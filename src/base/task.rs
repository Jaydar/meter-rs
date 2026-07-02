use std::{sync::mpsc, thread, time::Duration};

use crate::ui;
use slint::{ComponentHandle, Model, ModelRc};
use sysinfo::{CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, System};
use windows::Win32::{
    System::{
        ProcessStatus::EmptyWorkingSet,
        Threading::{GetCurrentProcess, GetCurrentThread, SetThreadAffinityMask},
    },
};

const ZERO_RATE: &str = "0.00 KB";

struct MonitorRequest {
    show_disk_total: bool,
    show_disk_io: bool,
    show_network: bool,
    selected_disk_ids: Vec<String>,
}

fn format_rate(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    let mut value = bytes as f64 / 1024.0;
    let mut idx = 0usize;

    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }

    format!("{value:.2} {}", UNITS[idx])
}

fn get_computer_name() -> String {
    System::host_name().unwrap_or_else(|| "Unknown".to_string())
}

fn disk_id(disk: &sysinfo::Disk) -> String {
    disk.mount_point().to_string_lossy().to_string()
}

fn disk_letter(disk: &sysinfo::Disk) -> String {
    disk.mount_point().to_string_lossy().trim_end_matches('\\').to_string()
}

fn disk_name(disk: &sysinfo::Disk) -> String {
    let letter = disk_letter(disk);
    let name = disk.name().to_string_lossy().trim().to_string();
    if name.is_empty() || name.eq_ignore_ascii_case(&letter) {
        letter
    } else {
        format!("{letter} {name}")
    }
}

fn retain_selected_disk_ids(selected_ids: &[String], valid_ids: &[String]) -> Vec<String> {
    selected_ids
        .iter()
        .filter(|id| valid_ids.iter().any(|valid_id| valid_id == *id))
        .cloned()
        .collect()
}

fn trim_working_set() {
    unsafe {
        let handle = GetCurrentProcess();
        let _ = EmptyWorkingSet(handle);
    }
}

pub fn refresh_disk_menu(view: &ui::AppWindow) {
    let mut disks = Disks::new();
    disks.refresh_specifics(true, DiskRefreshKind::nothing().with_storage());

    let runtime_store = view.global::<ui::RuntimeStore>();
    let model = runtime_store.get_disk_menu_entries();
    let mut selected_ids = Vec::new();
    for i in 0..model.row_count() {
        if let Some(entry) = model.row_data(i) {
            if entry.checked {
                selected_ids.push(entry.id.to_string());
            }
        }
    }

    let disk_options = {
        disks
            .iter()
            .map(|disk| (disk_id(disk), disk_name(disk)))
            .collect::<Vec<_>>()
    };

    let valid_ids = disk_options
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let selected_ids = retain_selected_disk_ids(&selected_ids, &valid_ids);
    let disk_menu_entries = disk_options
        .iter()
        .map(|(id, name)| ui::DiskMenuEntry {
            id: id.clone().into(),
            name: name.clone().into(),
            checked: selected_ids.iter().any(|selected_id| selected_id == id),
        })
        .collect::<Vec<_>>();
    let has_monitored_disks = !selected_ids.is_empty();

    runtime_store.set_disk_menu_entries(ModelRc::from(disk_menu_entries.as_slice()));
    runtime_store.set_has_monitored_disks(has_monitored_disks);
}

pub fn start_monitor(view: &ui::AppWindow) {
    let cpu_last = thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(1))
        .unwrap_or(0);
    let cpu_last = cpu_last.min((usize::BITS as usize).saturating_sub(1));
    let mask: usize = 1usize << cpu_last;

    let runtime_store = view.global::<ui::RuntimeStore>();
    runtime_store.set_computer_name(get_computer_name().into());
    let model = runtime_store.get_disk_menu_entries();
    let mut has_monitored_disks = false;
    for i in 0..model.row_count() {
        if let Some(entry) = model.row_data(i) {
            if entry.checked {
                has_monitored_disks = true;
                break;
            }
        }
    }
    runtime_store.set_has_monitored_disks(has_monitored_disks);

    let weak = view.as_weak();

    let _ = thread::Builder::new()
        .name("meter-rs".to_string())
        .spawn(move || {
            let mut system = System::new();
            let mut disks = Disks::new();
            let mut networks = Networks::new();
            let mut trimmed_after_start = false;

            unsafe {
                let thread = GetCurrentThread();
                let _ = SetThreadAffinityMask(thread, mask);
            }

            loop {
                let (request_tx, request_rx) = mpsc::sync_channel(1);
                let weak_request = weak.clone();
                if weak_request
                    .upgrade_in_event_loop(move |ui| {
                        let config_store = ui.global::<ui::ConfigStore>();
                        let runtime_store = ui.global::<ui::RuntimeStore>();
                        let model = runtime_store.get_disk_menu_entries();
                        let mut selected_disk_ids = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(entry) = model.row_data(i) {
                                if entry.checked {
                                    selected_disk_ids.push(entry.id.to_string());
                                }
                            }
                        }

                        let _ = request_tx.send(MonitorRequest {
                            show_disk_total: config_store.get_show_disk_total(),
                            show_disk_io: config_store.get_show_disk_io(),
                            show_network: config_store.get_show_network(),
                            selected_disk_ids,
                        });
                    })
                    .is_err()
                {
                    break;
                }

                let Ok(mut request) = request_rx.recv() else {
                    break;
                };
                let had_monitored_disks = !request.selected_disk_ids.is_empty();

                let (cpu_usage, memory_usage) = {
                    system.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());
                    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

                    let cpu = system.global_cpu_usage();
                    let mem =
                        (system.used_memory() as f32 / system.total_memory().max(1) as f32) * 100.0;
                    (cpu, mem)
                };

                let mut disk_usage = 0.0;
                let mut disk_total_read = ZERO_RATE.to_string();
                let mut disk_total_write = ZERO_RATE.to_string();
                let mut monitored_disks = Vec::new();
                let mut has_monitored_disks = had_monitored_disks;

                if request.show_disk_total || request.show_disk_io || had_monitored_disks {
                    let mut disk_refresh_kind = DiskRefreshKind::nothing();
                    if request.show_disk_total || had_monitored_disks {
                        disk_refresh_kind = disk_refresh_kind.with_storage();
                    }
                    if request.show_disk_io || had_monitored_disks {
                        disk_refresh_kind = disk_refresh_kind.with_io_usage();
                    }

                    let (
                        next_disk_usage,
                        next_disk_total_read,
                        next_disk_total_write,
                        next_monitored_disks,
                        next_has_monitored_disks,
                        next_selected_disk_ids,
                    ) = {
                        disks.refresh_specifics(true, disk_refresh_kind);

                        let selected_ids = if had_monitored_disks {
                            let valid_ids = disks.iter().map(disk_id).collect::<Vec<_>>();
                            retain_selected_disk_ids(&request.selected_disk_ids, &valid_ids)
                        } else {
                            Vec::new()
                        };
                        let has_monitored_disks = !selected_ids.is_empty();

                        let disk_usage = if request.show_disk_total {
                            let (total_space, used_space) =
                                disks.iter().fold((0u64, 0u64), |(total, used), disk| {
                                    (
                                        total + disk.total_space(),
                                        used + disk.total_space().saturating_sub(disk.available_space()),
                                    )
                                });
                            if total_space == 0 {
                                0.0
                            } else {
                                (used_space as f32 / total_space as f32) * 100.0
                            }
                        } else {
                            0.0
                        };

                        let mut total_read = 0u64;
                        let mut total_write = 0u64;
                        let mut monitored = Vec::new();
                        if request.show_disk_io || has_monitored_disks {
                            for disk in disks.iter() {
                                let usage = disk.usage();
                                if request.show_disk_io {
                                    total_read += usage.read_bytes;
                                    total_write += usage.written_bytes;
                                }

                                let id = disk_id(disk);
                                if selected_ids.iter().any(|selected| selected == &id) {
                                    let percent = if disk.total_space() == 0 {
                                        0.0
                                    } else {
                                        ((disk.total_space().saturating_sub(disk.available_space()))
                                            as f32
                                            / disk.total_space() as f32)
                                            * 100.0
                                    };
                                    monitored.push(ui::DiskIoEntry {
                                        id: id.into(),
                                        name: disk_name(disk).into(),
                                        usage: percent,
                                        read: format_rate(usage.read_bytes).into(),
                                        write: format_rate(usage.written_bytes).into(),
                                    });
                                }
                            }
                        }

                        (
                            disk_usage,
                            if request.show_disk_io {
                                format_rate(total_read)
                            } else {
                                ZERO_RATE.to_string()
                            },
                            if request.show_disk_io {
                                format_rate(total_write)
                            } else {
                                ZERO_RATE.to_string()
                            },
                            monitored,
                            has_monitored_disks,
                            selected_ids,
                        )
                    };

                    disk_usage = next_disk_usage;
                    disk_total_read = next_disk_total_read;
                    disk_total_write = next_disk_total_write;
                    monitored_disks = next_monitored_disks;
                    has_monitored_disks = next_has_monitored_disks;
                    request.selected_disk_ids = next_selected_disk_ids;
                }

                let (network_rx, network_tx) = if request.show_network {
                    networks.refresh(true);

                    let (rx, tx) = networks.iter().fold((0u64, 0u64), |(received, sent), (_, data)| {
                        (received + data.received(), sent + data.transmitted())
                    });
                    (format_rate(rx), format_rate(tx))
                } else {
                    (ZERO_RATE.to_string(), ZERO_RATE.to_string())
                };

                let weak = weak.clone();
                let selected_disk_ids = request.selected_disk_ids;
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let runtime_store = ui.global::<ui::RuntimeStore>();
                    runtime_store.set_cpu_usage(cpu_usage);
                    runtime_store.set_memory_usage(memory_usage);
                    runtime_store.set_disk_usage(disk_usage);
                    runtime_store.set_network_rx(network_rx.into());
                    runtime_store.set_network_tx(network_tx.into());
                    runtime_store.set_disk_total_read(disk_total_read.into());
                    runtime_store.set_disk_total_write(disk_total_write.into());
                    runtime_store.set_monitored_disks(ModelRc::from(monitored_disks.as_slice()));

                    if had_monitored_disks {
                        let model = runtime_store.get_disk_menu_entries();
                        let mut entries = Vec::new();
                        for i in 0..model.row_count() {
                            if let Some(mut entry) = model.row_data(i) {
                                entry.checked = selected_disk_ids
                                    .iter()
                                    .any(|selected_id| selected_id == &entry.id.to_string());
                                entries.push(entry);
                            }
                        }
                    }

                    runtime_store.set_has_monitored_disks(has_monitored_disks);
                });

                if !trimmed_after_start {
                    trimmed_after_start = true;
                    trim_working_set();
                }

                thread::sleep(Duration::from_millis(1500));
            }
        });
}

