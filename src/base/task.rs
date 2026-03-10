use std::{thread, time::Duration};

use crate::base::shared;
use crate::ui;
use slint::{ComponentHandle, ModelRc};
use sysinfo::System;
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};

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

pub async fn start_monitor(view: &ui::AppWindow) {
    let cpu_last = {
        let system = shared::info_system.lock().await;
        system.cpus().len().saturating_sub(1)
    };
    let cpu_last = cpu_last.min((usize::BITS as usize).saturating_sub(1));
    let mask: usize = 1usize << cpu_last;

    let settings = shared::app_settings.lock().unwrap().clone();
    let store = view.global::<ui::Store>();
    store.set_show_hostname(settings.show_hostname);
    store.set_show_cpu(settings.show_cpu);
    store.set_show_memory(settings.show_memory);
    store.set_show_disk_total(settings.show_disk_total);
    store.set_show_disk_io(settings.show_disk_io);
    store.set_show_network(settings.show_network);
    store.set_computer_name(get_computer_name().into());

    let weak = view.as_weak();

    let _ = thread::Builder::new()
        .name("meter-rs".to_string())
        .spawn(move || {
            unsafe {
                let thread = GetCurrentThread();
                let _ = SetThreadAffinityMask(thread, mask);
            }

            loop {
                let (
                    cpu_usage,
                    memory_usage,
                    disk_usage,
                    network_rx,
                    network_tx,
                    disk_total_read,
                    disk_total_write,
                    monitored_disks,
                    disk_options,
                ) = {
                    let mut system = shared::info_system.blocking_lock();
                    let mut disks = shared::info_disks.blocking_lock();
                    let mut networks = shared::info_networks.blocking_lock();

                    system.refresh_cpu_all();
                    system.refresh_memory();
                    disks.refresh(true);
                    networks.refresh(true);

                    let cpu = system.global_cpu_usage();
                    let mem =
                        (system.used_memory() as f32 / system.total_memory().max(1) as f32) * 100.0;
                    let (total_space, used_space) =
                        disks.iter().fold((0u64, 0u64), |(t, u), disk| {
                            (
                                t + disk.total_space(),
                                u + disk.total_space().saturating_sub(disk.available_space()),
                            )
                        });
                    let disk_usage = if total_space == 0 {
                        0.0
                    } else {
                        (used_space as f32 / total_space as f32) * 100.0
                    };

                    let (rx, tx) = networks.iter().fold((0u64, 0u64), |(rx, tx), (_, data)| {
                        (rx + data.received(), tx + data.transmitted())
                    });

                    let disk_options = disks
                        .iter()
                        .map(|disk| shared::DiskOption {
                            id: disk_id(disk),
                            name: disk_name(disk),
                        })
                        .collect::<Vec<_>>();

                    if let Ok(mut catalog) = shared::disk_catalog.lock() {
                        *catalog = disk_options.clone();
                    }

                    let selected_ids = {
                        let mut settings = shared::app_settings.lock().unwrap();
                        settings
                            .monitored_disk_ids
                            .retain(|id| disk_options.iter().any(|disk| disk.id == *id));
                        settings.monitored_disk_ids.clone()
                    };

                    let (total_read, total_write, monitored) = disks.iter().fold(
                        (0u64, 0u64, Vec::new()),
                        |(read_sum, write_sum, mut entries), disk| {
                            let usage = disk.usage();
                            let id = disk_id(disk);
                            if selected_ids.iter().any(|selected| selected == &id) {
                                let percent = if disk.total_space() == 0 {
                                    0.0
                                } else {
                                    ((disk.total_space().saturating_sub(disk.available_space())) as f32
                                        / disk.total_space() as f32)
                                        * 100.0
                                };
                                entries.push(ui::DiskIoEntry {
                                    id: id.into(),
                                    name: disk_name(disk).into(),
                                    usage: percent,
                                    read: format_rate(usage.read_bytes).into(),
                                    write: format_rate(usage.written_bytes).into(),
                                });
                            }
                            (
                                read_sum + usage.read_bytes,
                                write_sum + usage.written_bytes,
                                entries,
                            )
                        },
                    );

                    (
                        cpu,
                        mem,
                        disk_usage,
                        format_rate(rx),
                        format_rate(tx),
                        format_rate(total_read),
                        format_rate(total_write),
                        monitored,
                        disk_options,
                    )
                };

                let weak = weak.clone();
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let store = ui.global::<ui::Store>();
                    store.set_cpu_usage(cpu_usage);
                    store.set_memory_usage(memory_usage);
                    store.set_disk_usage(disk_usage);
                    store.set_network_rx(network_rx.into());
                    store.set_network_tx(network_tx.into());
                    store.set_disk_total_read(disk_total_read.into());
                    store.set_disk_total_write(disk_total_write.into());
                    store.set_monitored_disks(ModelRc::from(monitored_disks.as_slice()));

                    let disk_menu = &ui::use_view::<crate::view::DiskMenuView>().ui;
                    crate::view::DiskMenuView::sync_entries_with_options(disk_menu, &disk_options);

                    let submenu = &ui::use_view::<crate::view::SubmenuView>().ui;
                    submenu.set_has_monitored_disks(!monitored_disks.is_empty());
                });

                thread::sleep(Duration::from_millis(1500));
            }
        });
}
