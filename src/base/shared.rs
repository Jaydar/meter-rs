#![allow(non_upper_case_globals)]

use std::sync::{LazyLock, Mutex as StdMutex};

use sysinfo::{Disks, Networks, System};
use tokio::sync::Mutex;

pub static info_system: LazyLock<Mutex<System>> = LazyLock::new(|| Mutex::new(System::new_all()));
pub static info_disks: LazyLock<Mutex<Disks>> = LazyLock::new(|| Mutex::new(Disks::new_with_refreshed_list()));
pub static info_networks: LazyLock<Mutex<Networks>> = LazyLock::new(|| Mutex::new(Networks::new_with_refreshed_list()));
pub static win32_info: LazyLock<StdMutex<Win32Info>> = LazyLock::new(|| StdMutex::new(Win32Info::default()));
pub static app_settings: LazyLock<StdMutex<AppSettings>> = LazyLock::new(|| StdMutex::new(AppSettings::default()));
pub static disk_catalog: LazyLock<StdMutex<Vec<DiskOption>>> = LazyLock::new(|| StdMutex::new(Vec::new()));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeKind {
    #[default]
    Dark,
    Light,
}

#[derive(Default, Debug)]
pub struct Win32Info {
    pub hwnd: usize,
    pub monitor_width: f32,
    pub monitor_height: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskOption {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub theme: ThemeKind,
    pub opacity: f32,
    pub auto_start: bool,
    pub show_hostname: bool,
    pub show_cpu: bool,
    pub show_memory: bool,
    pub show_disk_total: bool,
    pub show_disk_io: bool,
    pub show_network: bool,
    pub mouse_passthrough: bool,
    pub prevent_sleep: bool,
    pub always_on_top: bool,
    pub snap_to_edge: bool,
    pub monitored_disk_ids: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeKind::Dark,
            opacity: 0.9,
            auto_start: false,
            show_hostname: false,
            show_cpu: true,
            show_memory: true,
            show_disk_total: true,
            show_disk_io: false,
            show_network: false,
            mouse_passthrough: false,
            prevent_sleep: false,
            always_on_top: true,
            snap_to_edge: true,
            monitored_disk_ids: Vec::new(),
        }
    }
}
