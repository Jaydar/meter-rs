use std::cell::RefCell;

use muda::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use slint::{ComponentHandle, Model, ModelRc};
use tracing::error;
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::{
    Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA},
    core::s,
};

use crate::{
    _main_hwnd, task, tools, ui,
    view::{AboutView, AppView, MacAddressView, RouteManagerView, ViewTrait},
};

thread_local! {
    static _tray: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
    static _event_timer: RefCell<Option<slint::Timer>> = const { RefCell::new(None) };
}

pub fn setup(_hwnd: usize) {
    let result = slint::invoke_from_event_loop(|| {
        _tray.with(|tray| {
            if tray.borrow().is_some() {
                return;
            }

            apply_windows_menu_theme(ui::use_view::<AppView>().ui.global::<ui::ConfigStore>().get_theme_mode());
            let menu = build_menu();
            let icon = Icon::from_resource(1, Some((16, 16))).or_else(|_| tray_icon_image());
            let icon = match icon {
                Ok(icon) => icon,
                Err(err) => {
                    error!("create tray icon image failed: {}", err);
                    return;
                }
            };
            let tray_icon = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("Meter RS").with_icon(icon).with_menu_on_left_click(false).with_menu_on_right_click(true).build();
            let tray_icon = match tray_icon {
                Ok(tray_icon) => tray_icon,
                Err(err) => {
                    error!("create tray icon failed: {}", err);
                    return;
                }
            };
            *tray.borrow_mut() = Some(tray_icon);
            start_event_timer();
        });
    });

    if let Err(err) = result {
        error!("setup tray failed: {}", err);
    }
}

fn start_event_timer() {
    _event_timer.with(|timer| {
        if timer.borrow().is_some() {
            return;
        }
        let timer_inst = slint::Timer::default();
        timer_inst.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(80), || {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                handle_menu_event(event.id.as_ref());
            }
        });
        *timer.borrow_mut() = Some(timer_inst);
    });
}

fn tray_icon_image() -> Result<Icon, tray_icon::BadIcon> {
    let mut rgba = vec![0u8; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let i = (y * 16 + x) * 4;
            let active = (3..=5).contains(&x) && y >= 7 || (7..=9).contains(&x) && y >= 4 || (11..=13).contains(&x) && y >= 9;
            let border = x == 1 || x == 14 || y == 1 || y == 14;
            if active {
                rgba[i..i + 4].copy_from_slice(&[74, 222, 128, 255]);
            } else if border {
                rgba[i..i + 4].copy_from_slice(&[148, 163, 184, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, 16, 16)
}

fn apply_windows_menu_theme(theme_mode: ui::ThemeMode) {
    unsafe {
        let Ok(module) = LoadLibraryA(s!("uxtheme.dll")) else {
            return;
        };
        type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
        type FlushMenuThemes = unsafe extern "system" fn();
        let mode = match theme_mode {
            ui::ThemeMode::System => 1,
            ui::ThemeMode::Dark => 2,
            ui::ThemeMode::Light => 3,
        };
        if let Some(handle) = GetProcAddress(module, windows::core::PCSTR(135usize as _)) {
            let set_preferred_app_mode: SetPreferredAppMode = std::mem::transmute(handle);
            set_preferred_app_mode(mode);
        }
        if let Some(handle) = GetProcAddress(module, windows::core::PCSTR(136usize as _)) {
            let flush_menu_themes: FlushMenuThemes = std::mem::transmute(handle);
            flush_menu_themes();
        }
    }
}

fn handle_menu_event(id: &str) {
    if let Some(disk_id) = id.strip_prefix("disk:") {
        toggle_disk(disk_id.into());
        return;
    }

    let app_view = ui::use_view::<AppView>();
    let config_store = app_view.ui.global::<ui::ConfigStore>();
    match id {
        "theme:system" => set_theme(ui::ThemeMode::System),
        "theme:dark" => set_theme(ui::ThemeMode::Dark),
        "theme:light" => set_theme(ui::ThemeMode::Light),
        "display_mode:normal" => set_display_mode(ui::DisplayMode::Normal),
        "display_mode:simple" => set_display_mode(ui::DisplayMode::Simple),
        "display_mode:taskbar" => set_display_mode(ui::DisplayMode::Taskbar),
        "show_hostname" => set_show_hostname(!config_store.get_show_hostname()),
        "show_cpu" => set_show_cpu(!config_store.get_show_cpu()),
        "show_memory" => set_show_memory(!config_store.get_show_memory()),
        "show_disk_total" => set_show_disk_total(!config_store.get_show_disk_total()),
        "show_disk_io" => set_show_disk_io(!config_store.get_show_disk_io()),
        "show_network" => set_show_network(!config_store.get_show_network()),
        "always_on_top" => set_always_on_top(!config_store.get_always_on_top()),
        "snap_to_edge" => set_snap_to_edge(!config_store.get_snap_to_edge()),
        "snap_mode:work_area" => set_snap_mode(ui::SnapMode::WorkArea),
        "snap_mode:full_screen" => set_snap_mode(ui::SnapMode::FullScreen),
        "opacity:1.0" => set_opacity(1.0),
        "opacity:0.9" => set_opacity(0.9),
        "opacity:0.75" => set_opacity(0.75),
        "opacity:0.6" => set_opacity(0.6),
        "opacity:0.45" => set_opacity(0.45),
        "mouse_passthrough" => set_mouse_passthrough(!config_store.get_mouse_passthrough()),
        "prevent_sleep" => set_prevent_sleep(!config_store.get_prevent_sleep()),
        "auto_start" => set_auto_start(!config_store.get_auto_start()),
        "turn_off_display" => tools::turn_off_display(),
        "restart_explorer" => tools::restart_explorer(),
        "clean_memory" => tools::clean_memory(),
        "mac_address" => {
            if tools::is_admin() {
                if let Err(err) = ui::open_view::<MacAddressView>() {
                    error!("{}", err);
                }
            } else if let Err(err) = tools::run_as_admin_open_page("mac") {
                error!("{}", err);
            }
        }
        "route_manager" => {
            if tools::is_admin() {
                if let Err(err) = ui::open_view::<RouteManagerView>() {
                    error!("{}", err);
                }
            } else if let Err(err) = tools::run_as_admin_open_page("route") {
                error!("{}", err);
            }
        }
        "about" => {
            if let Err(err) = ui::open_view::<AboutView>() {
                error!("{}", err);
            }
        }
        "quit" => {
            tools::close_pages();
            let _ = slint::quit_event_loop();
        }
        _ => {}
    }
}

fn build_menu() -> Menu {
    let app_view = ui::use_view::<AppView>();
    task::refresh_disk_menu(&app_view.ui);
    let config_store = app_view.ui.global::<ui::ConfigStore>();
    let runtime_store = app_view.ui.global::<ui::RuntimeStore>();
    let menu = Menu::new();
    let theme_menu = Submenu::with_items("主题", true, &[&check_item("theme:system", "系统", config_store.get_theme_mode() == ui::ThemeMode::System), &check_item("theme:dark", "深色", config_store.get_theme_mode() == ui::ThemeMode::Dark), &check_item("theme:light", "浅色", config_store.get_theme_mode() == ui::ThemeMode::Light)]).unwrap();
    let display_mode_menu = Submenu::with_items(
        "显示模式",
        true,
        &[
            &check_item("display_mode:normal", "正常模式", config_store.get_display_mode() == ui::DisplayMode::Normal),
            &check_item("display_mode:simple", "简洁模式", config_store.get_display_mode() == ui::DisplayMode::Simple),
            &check_item("display_mode:taskbar", "任务栏模式", config_store.get_display_mode() == ui::DisplayMode::Taskbar),
        ],
    )
    .unwrap();
    let display_items = vec![
        check_item("show_hostname", "主机名", config_store.get_show_hostname()),
        check_item("show_cpu", "CPU", config_store.get_show_cpu()),
        check_item("show_memory", "内存", config_store.get_show_memory()),
        check_item("show_disk_total", "磁盘", config_store.get_show_disk_total()),
    ];
    let display_menu = Submenu::with_items("显示设置", true, &[&display_mode_menu, &display_items[0], &display_items[1], &display_items[2], &display_items[3], &disk_menu(&runtime_store), &check_item("show_disk_io", "磁盘IO", config_store.get_show_disk_io()), &check_item("show_network", "流量", config_store.get_show_network())]).unwrap();
    let window_menu = Submenu::with_items(
        "窗口",
        true,
        &[
            &check_item("always_on_top", "总是置顶", config_store.get_always_on_top()),
            &check_item("snap_to_edge", "靠边吸附", config_store.get_snap_to_edge()),
            &check_item("snap_mode:work_area", "工作区吸附", config_store.get_snap_mode() == ui::SnapMode::WorkArea),
            &check_item("snap_mode:full_screen", "全屏吸附", config_store.get_snap_mode() == ui::SnapMode::FullScreen),
            &PredefinedMenuItem::separator(),
            &check_item("opacity:1.0", "透明度 100%", config_store.get_window_opacity() == 1.0),
            &check_item("opacity:0.9", "透明度 90%", config_store.get_window_opacity() == 0.9),
            &check_item("opacity:0.75", "透明度 75%", config_store.get_window_opacity() == 0.75),
            &check_item("opacity:0.6", "透明度 60%", config_store.get_window_opacity() == 0.6),
            &check_item("opacity:0.45", "透明度 45%", config_store.get_window_opacity() == 0.45),
        ],
    )
    .unwrap();

    append_items(
        &menu,
        &[
            &theme_menu,
            &display_menu,
            &window_menu,
            &PredefinedMenuItem::separator(),
            &check_item("mouse_passthrough", "鼠标穿透", config_store.get_mouse_passthrough()),
            &check_item("prevent_sleep", "禁止休眠", config_store.get_prevent_sleep()),
            &check_item("auto_start", "开机自启", config_store.get_auto_start()),
            &item("turn_off_display", "关闭显示器"),
            &item("restart_explorer", "重启资源管理器"),
            &item("clean_memory", "内存清理"),
            &item("mac_address", "修改 MAC 地址"),
            &item("route_manager", "路由管理"),
            &PredefinedMenuItem::separator(),
            &item("about", "关于 Meter RS"),
            &PredefinedMenuItem::separator(),
            &item("quit", "退出程序"),
        ],
    );
    menu
}

fn append_items(menu: &Menu, items: &[&dyn IsMenuItem]) {
    if let Err(err) = menu.append_items(items) {
        error!("append tray menu items failed: {}", err);
    }
}

fn check_item(id: &str, text: &str, checked: bool) -> CheckMenuItem {
    CheckMenuItem::with_id(id, text, true, checked, None)
}

fn item(id: &str, text: &str) -> MenuItem {
    MenuItem::with_id(id, text, true, None)
}

fn disk_menu(runtime_store: &ui::RuntimeStore) -> Submenu {
    let model = runtime_store.get_disk_menu_entries();
    let mut items = Vec::new();
    for i in 0..model.row_count() {
        if let Some(entry) = model.row_data(i) {
            items.push(CheckMenuItem::with_id(format!("disk:{}", entry.id), entry.name, true, entry.checked, None));
        }
    }
    let refs = items.iter().map(|item| item as &dyn IsMenuItem).collect::<Vec<_>>();
    Submenu::with_items("磁盘监控", true, &refs).unwrap()
}

fn sync_tray() {
    _tray.with(|tray| {
        if let Some(tray_icon) = tray.borrow().as_ref() {
            apply_windows_menu_theme(ui::use_view::<AppView>().ui.global::<ui::ConfigStore>().get_theme_mode());
            tray_icon.set_menu(Some(Box::new(build_menu())));
        }
    });
}

pub(crate) fn set_theme(theme_mode: ui::ThemeMode) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_theme_mode(theme_mode);
    app_view.ui.global::<ui::Theme>().set_mode(theme_mode);
    apply_windows_menu_theme(theme_mode);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_hostname(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_hostname(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_cpu(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_cpu(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_memory(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_memory(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_disk_total(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_disk_total(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_disk_io(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_disk_io(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_show_network(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_show_network(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_display_mode(value: ui::DisplayMode) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_display_mode(value);
    app_view.set_position();
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_mouse_passthrough(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_mouse_passthrough(value);
    let hwnd = _main_hwnd.load(std::sync::atomic::Ordering::Relaxed);
    if hwnd != 0 {
        tools::set_mouse_passthrough(hwnd, value);
    }
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_prevent_sleep(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_prevent_sleep(value);
    tools::set_prevent_sleep(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_auto_start(value: bool) {
    let app_view = ui::use_view::<AppView>();
    tools::set_auto_start(value);
    app_view.ui.global::<ui::ConfigStore>().set_auto_start(value);
    sync_tray();
}

pub(crate) fn set_always_on_top(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_always_on_top(value);
    app_view.ui.set_always_on_top_state(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_snap_to_edge(value: bool) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_snap_to_edge(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_snap_mode(value: ui::SnapMode) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_snap_mode(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn set_opacity(value: f32) {
    let app_view = ui::use_view::<AppView>();
    app_view.ui.global::<ui::ConfigStore>().set_window_opacity(value);
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

pub(crate) fn toggle_disk(disk_id: slint::SharedString) {
    let app_view = ui::use_view::<AppView>();
    let runtime_store = app_view.ui.global::<ui::RuntimeStore>();
    let model = runtime_store.get_disk_menu_entries();
    let mut entries = Vec::new();
    for i in 0..model.row_count() {
        if let Some(mut entry) = model.row_data(i) {
            if entry.id == disk_id {
                entry.checked = !entry.checked;
            }
            entries.push(entry);
        }
    }
    runtime_store.set_has_monitored_disks(entries.iter().any(|entry| entry.checked));
    runtime_store.set_disk_menu_entries(ModelRc::from(entries.as_slice()));
    crate::base::config::save(&app_view.ui);
    sync_tray();
}

