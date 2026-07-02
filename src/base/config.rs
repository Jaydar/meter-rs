use std::{fs, path::PathBuf};

use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model, ModelRc};
use tracing::error;

use crate::ui::{self, ThemeMode};

#[derive(Clone, Deserialize, Serialize)]
struct Settings {
    theme_mode: String,
    mouse_passthrough: bool,
    prevent_sleep: bool,
    always_on_top: bool,
    snap_to_edge: bool,
    snap_mode: String,
    window_opacity: f32,
    simple_mode: bool,
    show_hostname: bool,
    show_cpu: bool,
    show_memory: bool,
    show_disk_total: bool,
    show_disk_io: bool,
    show_network: bool,
    selected_disk_ids: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: "system".to_string(),
            mouse_passthrough: false,
            prevent_sleep: false,
            always_on_top: true,
            snap_to_edge: true,
            snap_mode: "work_area".to_string(),
            window_opacity: 0.9,
            simple_mode: false,
            show_hostname: false,
            show_cpu: true,
            show_memory: true,
            show_disk_total: true,
            show_disk_io: false,
            show_network: false,
            selected_disk_ids: Vec::new(),
        }
    }
}

#[derive(Default, Serialize)]
struct PartialSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    theme_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_passthrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prevent_sleep: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    always_on_top: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snap_to_edge: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snap_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    simple_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_hostname: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_cpu: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_memory: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_disk_total: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_disk_io: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_network: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    selected_disk_ids: Vec<String>,
}

fn path() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("meter-rs.yml")))
}

fn settings() -> Settings {
    let defaults = Settings::default();
    let mut builder = Config::builder()
        .set_default("theme_mode", defaults.theme_mode)
        .unwrap()
        .set_default("mouse_passthrough", defaults.mouse_passthrough)
        .unwrap()
        .set_default("prevent_sleep", defaults.prevent_sleep)
        .unwrap()
        .set_default("always_on_top", defaults.always_on_top)
        .unwrap()
        .set_default("snap_to_edge", defaults.snap_to_edge)
        .unwrap()
        .set_default("snap_mode", defaults.snap_mode)
        .unwrap()
        .set_default("window_opacity", defaults.window_opacity as f64)
        .unwrap()
        .set_default("simple_mode", defaults.simple_mode)
        .unwrap()
        .set_default("show_hostname", defaults.show_hostname)
        .unwrap()
        .set_default("show_cpu", defaults.show_cpu)
        .unwrap()
        .set_default("show_memory", defaults.show_memory)
        .unwrap()
        .set_default("show_disk_total", defaults.show_disk_total)
        .unwrap()
        .set_default("show_disk_io", defaults.show_disk_io)
        .unwrap()
        .set_default("show_network", defaults.show_network)
        .unwrap()
        .set_default("selected_disk_ids", Vec::<String>::new())
        .unwrap();
    if let Some(path) = path() {
        builder = builder.add_source(File::from(path).required(false));
    }
    builder.add_source(Environment::with_prefix("METER_RS").separator("__")).build().and_then(|config| config.try_deserialize()).unwrap_or_else(|err| {
        error!("load config failed: {}", err);
        Settings::default()
    })
}

pub fn load(view: &ui::AppWindow) {
    let settings = settings();
    let theme_mode = ThemeMode::from_str(&settings.theme_mode);
    let config_store = view.global::<ui::ConfigStore>();
    let runtime_store = view.global::<ui::RuntimeStore>();
    config_store.set_theme_mode(theme_mode);
    view.global::<ui::Theme>().set_mode(theme_mode);
    config_store.set_mouse_passthrough(settings.mouse_passthrough);
    config_store.set_prevent_sleep(settings.prevent_sleep);
    crate::tools::set_prevent_sleep(settings.prevent_sleep);
    config_store.set_always_on_top(settings.always_on_top);
    view.set_always_on_top_state(settings.always_on_top);
    config_store.set_snap_to_edge(settings.snap_to_edge);
    config_store.set_snap_mode(ui::SnapMode::from_str(&settings.snap_mode));
    config_store.set_window_opacity(settings.window_opacity);
    config_store.set_simple_mode(settings.simple_mode);
    config_store.set_show_hostname(settings.show_hostname);
    config_store.set_show_cpu(settings.show_cpu);
    config_store.set_show_memory(settings.show_memory);
    config_store.set_show_disk_total(settings.show_disk_total);
    config_store.set_show_disk_io(settings.show_disk_io);
    config_store.set_show_network(settings.show_network);
    if !settings.selected_disk_ids.is_empty() {
        let entries = settings.selected_disk_ids.iter().map(|id| ui::DiskMenuEntry { id: id.clone().into(), name: id.clone().into(), checked: true }).collect::<Vec<_>>();
        runtime_store.set_disk_menu_entries(ModelRc::from(entries.as_slice()));
        runtime_store.set_has_monitored_disks(true);
    }
}

pub fn save(view: &ui::AppWindow) {
    let config_store = view.global::<ui::ConfigStore>();
    let runtime_store = view.global::<ui::RuntimeStore>();
    let defaults = Settings::default();
    let model = runtime_store.get_disk_menu_entries();
    let mut selected_disk_ids = Vec::new();
    for i in 0..model.row_count() {
        if let Some(entry) = model.row_data(i) {
            if entry.checked {
                selected_disk_ids.push(entry.id.to_string());
            }
        }
    }
    let settings = PartialSettings {
        theme_mode: (config_store.get_theme_mode() != ThemeMode::System).then(|| config_store.get_theme_mode().as_str().to_string()),
        mouse_passthrough: (config_store.get_mouse_passthrough() != defaults.mouse_passthrough).then_some(config_store.get_mouse_passthrough()),
        prevent_sleep: (config_store.get_prevent_sleep() != defaults.prevent_sleep).then_some(config_store.get_prevent_sleep()),
        always_on_top: (config_store.get_always_on_top() != defaults.always_on_top).then_some(config_store.get_always_on_top()),
        snap_to_edge: (config_store.get_snap_to_edge() != defaults.snap_to_edge).then_some(config_store.get_snap_to_edge()),
        snap_mode: (config_store.get_snap_mode() != ui::SnapMode::WorkArea).then(|| config_store.get_snap_mode().as_str().to_string()),
        window_opacity: ((config_store.get_window_opacity() - defaults.window_opacity).abs() > f32::EPSILON).then_some(config_store.get_window_opacity()),
        simple_mode: (config_store.get_simple_mode() != defaults.simple_mode).then_some(config_store.get_simple_mode()),
        show_hostname: (config_store.get_show_hostname() != defaults.show_hostname).then_some(config_store.get_show_hostname()),
        show_cpu: (config_store.get_show_cpu() != defaults.show_cpu).then_some(config_store.get_show_cpu()),
        show_memory: (config_store.get_show_memory() != defaults.show_memory).then_some(config_store.get_show_memory()),
        show_disk_total: (config_store.get_show_disk_total() != defaults.show_disk_total).then_some(config_store.get_show_disk_total()),
        show_disk_io: (config_store.get_show_disk_io() != defaults.show_disk_io).then_some(config_store.get_show_disk_io()),
        show_network: (config_store.get_show_network() != defaults.show_network).then_some(config_store.get_show_network()),
        selected_disk_ids,
    };
    let Some(path) = path() else {
        return;
    };
    let Ok(text) = serde_yaml::to_string(&settings) else {
        return;
    };
    if text.trim() == "{}" {
        let _ = fs::remove_file(path);
        return;
    }
    if let Err(err) = fs::write(path, text) {
        error!("save config failed: {}", err);
    }
}
