#![allow(non_upper_case_globals)]

use std::sync::{LazyLock, Mutex as StdMutex};

use sysinfo::{Disks, Networks, System};
use tokio::sync::Mutex;

pub static info_system: LazyLock<Mutex<System>> = LazyLock::new(|| Mutex::new(System::new_all()));
pub static info_disks: LazyLock<Mutex<Disks>> = LazyLock::new(|| Mutex::new(Disks::new_with_refreshed_list()));
pub static info_networks: LazyLock<Mutex<Networks>> = LazyLock::new(|| Mutex::new(Networks::new_with_refreshed_list()));
pub static win32_info: LazyLock<StdMutex<Win32Info>> = LazyLock::new(|| StdMutex::new(Win32Info::default()));
pub static app_settings: LazyLock<StdMutex<AppSettings>> = LazyLock::new(|| StdMutex::new(AppSettings::default()));

#[derive(Default, Debug)]
pub struct Win32Info {
    pub hwnd: usize,
    pub monitor_width: f32,
    pub monitor_height: f32,
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub theme: i32,
    pub opacity: f32,
    pub auto_start: bool,
    pub show_cpu: bool,
    pub show_memory: bool,
    pub show_disk_usage: bool,
    pub show_network: bool,
    pub show_disk_io: bool,
    pub mouse_passthrough: bool,
    pub prevent_sleep: bool,
    pub always_on_top: bool,
    pub snap_to_edge: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: 0,
            opacity: 0.9,
            auto_start: false,
            show_cpu: true,
            show_memory: true,
            show_disk_usage: true,
            show_network: true,
            show_disk_io: true,
            mouse_passthrough: false,
            prevent_sleep: false,
            always_on_top: true,
            snap_to_edge: true,
        }
    }
}
