use crate::base::{shared, tools};
use crate::ui;
use slint::ComponentHandle;

fn format_rate(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB/s", "MB/s", "GB/s", "TB/s"];
    let mut value = (bytes as f64 / 1024.0).max(0.01);
    let mut idx = 0usize;

    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }

    format!("{value:.2} {}", UNITS[idx])
}

fn fit_disk_name(name: &str) -> String {
    let truncated = name.chars().take(8).collect::<String>();
    format!("{truncated:<8}")
}

pub async fn start_monitor(view: &ui::AppWindow) {
    let system = shared::info_system.lock().await;
    let cpu_last = system.cpus().len().saturating_sub(1);
    let mask: usize = 1usize << cpu_last;
    drop(system);

    let settings = shared::app_settings.lock().unwrap().clone();
    let store = view.global::<ui::Store>();
    store.set_show_cpu(settings.show_cpu);
    store.set_show_memory(settings.show_memory);
    store.set_show_disk_usage(settings.show_disk_usage);
    store.set_show_network(settings.show_network);
    store.set_show_disk_io(settings.show_disk_io);

    let pool = tools::get_tokio_runtime(mask, 1);
    let weak = view.as_weak();

    pool.spawn(async move {
        loop {
            let (cpu_usage, memory_usage, disk_usage, network_rx, network_tx, disk_io_summary) = {
                let mut system = shared::info_system.lock().await;
                let mut disks = shared::info_disks.lock().await;
                let mut networks = shared::info_networks.lock().await;

                system.refresh_cpu_all();
                system.refresh_memory();
                disks.refresh(true);
                networks.refresh(true);

                let cpu = system.global_cpu_usage();
                let mem = (system.used_memory() as f32 / system.total_memory().max(1) as f32) * 100.0;
                let (total, used) = disks.iter().fold((0u64, 0u64), |(t, u), d| {
                    (t + d.total_space(), u + d.total_space().saturating_sub(d.available_space()))
                });
                let disk = if total == 0 { 0.0 } else { (used as f32 / total as f32) * 100.0 };

                let (rx, tx) = networks.iter().fold((0u64, 0u64), |(rx, tx), (_, data)| {
                    (rx + data.received(), tx + data.transmitted())
                });

                let mut disk_io_lines = vec![format!("{:<8} {:>10} {:>10}", "磁盘", "读取", "写入")];
                disk_io_lines.extend(disks.iter().map(|disk| {
                    let usage = disk.usage();
                    format!(
                        "{} {:>10} {:>10}",
                        fit_disk_name(&disk.name().to_string_lossy()),
                        format_rate(usage.read_bytes),
                        format_rate(usage.written_bytes)
                    )
                }));

                (cpu, mem, disk, format_rate(rx), format_rate(tx), disk_io_lines.join("\n"))
            };

            let weak = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let store = ui.global::<ui::Store>();
                store.set_cpu_usage(cpu_usage);
                store.set_memory_usage(memory_usage);
                store.set_disk_usage(disk_usage);
                store.set_network_rx(network_rx.into());
                store.set_network_tx(network_tx.into());
                store.set_disk_io_summary(disk_io_summary.into());
            });

            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
    });

    std::mem::forget(pool);
}
