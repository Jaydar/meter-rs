use slint::{ComponentHandle, ModelRc, RenderingState, Timer, TimerMode};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::{shared, tools, ui, view::app_view};

static LAST_MENU_SHOW_AT: OnceLock<Mutex<Instant>> = OnceLock::new();

pub struct MenuView {
    pub ui: ui::MenuWindow,
    close_timer: Timer,
}

pub struct SubmenuView {
    pub ui: ui::SubmenuWindow,
}

pub struct DiskMenuView {
    pub ui: ui::DiskMenuWindow,
}

unsafe impl Send for MenuView {}
unsafe impl Sync for MenuView {}
unsafe impl Send for SubmenuView {}
unsafe impl Sync for SubmenuView {}
unsafe impl Send for DiskMenuView {}
unsafe impl Sync for DiskMenuView {}

impl Default for MenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SubmenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DiskMenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuView {
    pub fn new() -> Self {
        let ui = ui::MenuWindow::new().unwrap();
        Self { ui, close_timer: Timer::default() }.setup()
    }

    fn setup(self) -> Self {
        self.sync_from_settings();
        let weak_handle = self.ui.as_weak();

        self.ui.on_close_menu(move || hide_all_menus());
        self.ui.on_close_app(move || {
            let _ = slint::quit_event_loop();
        });
        self.ui.on_set_auto_start(move |enable| {
            tools::set_auto_start(enable);
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.auto_start = enable;
            }
        });
        self.ui.on_set_mouse_passthrough(move |enable| {
            let hwnd = shared::win32_info.try_lock().unwrap().hwnd;
            if hwnd != 0 {
                tools::set_mouse_passthrough(hwnd, enable);
            }
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.mouse_passthrough = enable;
            }
        });
        self.ui.on_set_prevent_sleep(move |enable| {
            tools::set_prevent_sleep(enable);
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.prevent_sleep = enable;
            }
        });
        self.ui.on_turn_off_display(move || tools::turn_off_display());
        self.ui.on_restart_explorer(move || tools::restart_explorer());
        self.ui.on_show_theme_submenu(move |offset_y| show_submenu(ui::SubmenuKind::Theme, offset_y as i32));
        self.ui.on_show_display_submenu(move |offset_y| show_submenu(ui::SubmenuKind::Display, offset_y as i32));
        self.ui.on_show_window_submenu(move |offset_y| show_submenu(ui::SubmenuKind::Window, offset_y as i32));
        self.ui.on_hide_submenu(move || hide_secondary_menus());

        self.close_timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
            if let Some(menu) = weak_handle.upgrade() {
                if !menu.window().is_visible() {
                    return;
                }

                let shown_at = LAST_MENU_SHOW_AT
                    .get_or_init(|| Mutex::new(Instant::now()))
                    .lock()
                    .map(|instant| *instant)
                    .unwrap_or_else(|_| Instant::now());
                if shown_at.elapsed() < Duration::from_millis(500) {
                    return;
                }

                let submenu = ui::use_view::<crate::view::SubmenuView>();
                let disk_menu = ui::use_view::<crate::view::DiskMenuView>();
                let mouse = tools::get_current_mouse_position();
                let mouse_in_main = window_contains_point(&menu, mouse.x, mouse.y);
                let mouse_in_sub = submenu.ui.window().is_visible() && window_contains_point(&submenu.ui, mouse.x, mouse.y);
                let mouse_in_disk = disk_menu.ui.window().is_visible() && window_contains_point(&disk_menu.ui, mouse.x, mouse.y);

                let menu_hwnd = tools::get_hwnd_by_window_handle(&menu);
                let submenu_hwnd = if submenu.ui.window().is_visible() {
                    tools::get_hwnd_by_window_handle(&submenu.ui)
                } else {
                    None
                };
                let disk_hwnd = if disk_menu.ui.window().is_visible() {
                    tools::get_hwnd_by_window_handle(&disk_menu.ui)
                } else {
                    None
                };

                let main_active = menu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                let sub_active = submenu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                let disk_active = disk_hwnd.map(tools::is_window_foreground).unwrap_or(false);

                if !mouse_in_main && !mouse_in_sub && !mouse_in_disk && !main_active && !sub_active && !disk_active {
                    hide_all_menus();
                }
            }
        });

        self.set_position()
    }

    pub fn sync_from_settings(&self) {
        let settings = shared::app_settings.lock().unwrap().clone();
        self.ui.set_auto_start_state(settings.auto_start);
        self.ui.set_mouse_passthrough_state(settings.mouse_passthrough);
        self.ui.set_prevent_sleep_state(settings.prevent_sleep);
    }

    fn set_position(self) -> Self {
        let weak = self.ui.as_weak();
        self.ui
            .window()
            .set_rendering_notifier({
                let initialized = AtomicBool::new(false);
                move |state, _| match state {
                    RenderingState::BeforeRendering if !initialized.swap(true, Ordering::SeqCst) => {
                        if let Some(menu) = weak.upgrade() {
                            apply_main_menu_geometry(&menu);
                        }
                    }
                    _ => {}
                }
            })
            .expect("set_rendering_notifier error");
        self
    }
}

impl SubmenuView {
    pub fn new() -> Self {
        let ui = ui::SubmenuWindow::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        self.sync_from_settings();
        self.ui.on_close_menu(move || hide_all_menus());
        self.ui.on_set_theme(move |theme_mode| {
            {
                let mut settings = shared::app_settings.lock().unwrap();
                settings.theme = app_view::from_ui_theme_mode(theme_mode);
            }
            let app = ui::use_view::<crate::view::AppView>();
            app_view::apply_theme(&app.ui, app_view::from_ui_theme_mode(theme_mode));
            let submenu = ui::use_view::<crate::view::SubmenuView>();
            submenu.ui.set_theme_state(theme_mode);
        });
        self.ui.on_set_show_hostname(move |value| update_visibility(|settings| settings.show_hostname = value));
        self.ui.on_set_show_cpu(move |value| update_visibility(|settings| settings.show_cpu = value));
        self.ui.on_set_show_memory(move |value| update_visibility(|settings| settings.show_memory = value));
        self.ui.on_set_show_disk_total(move |value| update_visibility(|settings| settings.show_disk_total = value));
        self.ui.on_set_show_disk_io(move |value| update_visibility(|settings| settings.show_disk_io = value));
        self.ui.on_set_show_network(move |value| update_visibility(|settings| settings.show_network = value));
        self.ui.on_show_disk_monitor_submenu(move |offset_y| show_disk_menu(offset_y as i32));
        self.ui.on_hide_disk_monitor_submenu(move || hide_disk_menu());
        self.ui.on_set_always_on_top(move |value| update_window_settings(|settings| settings.always_on_top = value));
        self.ui.on_set_snap_to_edge(move |value| update_window_settings(|settings| settings.snap_to_edge = value));
        self.ui.on_set_opacity(move |value| update_window_settings(|settings| settings.opacity = value));
        self
    }

    pub fn sync_from_settings(&self) {
        let settings = shared::app_settings.lock().unwrap().clone();
        self.ui.set_theme_state(app_view::to_ui_theme_mode(settings.theme));
        self.ui.set_show_hostname_state(settings.show_hostname);
        self.ui.set_show_cpu_state(settings.show_cpu);
        self.ui.set_show_memory_state(settings.show_memory);
        self.ui.set_show_disk_total_state(settings.show_disk_total);
        self.ui.set_show_disk_io_state(settings.show_disk_io);
        self.ui.set_show_network_state(settings.show_network);
        self.ui.set_has_monitored_disks(!settings.monitored_disk_ids.is_empty());
        self.ui.set_always_on_top_state(settings.always_on_top);
        self.ui.set_snap_to_edge_state(settings.snap_to_edge);
        self.ui.set_opacity_value(settings.opacity);
    }
}

impl DiskMenuView {
    pub fn new() -> Self {
        let ui = ui::DiskMenuWindow::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        self.sync_entries();
        self.ui.on_close_menu(move || hide_all_menus());
        self.ui.on_toggle_disk(move |disk_id| {
            {
                let mut settings = shared::app_settings.lock().unwrap();
                let disk_id = disk_id.to_string();
                if let Some(index) = settings.monitored_disk_ids.iter().position(|id| id == &disk_id) {
                    settings.monitored_disk_ids.remove(index);
                } else {
                    settings.monitored_disk_ids.push(disk_id);
                }
            }
            let app = ui::use_view::<crate::view::AppView>();
            let settings = shared::app_settings.lock().unwrap().clone();
            app_view::apply_store_settings(&app.ui, &settings);
            let submenu = ui::use_view::<crate::view::SubmenuView>();
            submenu.sync_from_settings();
            let disk_menu = ui::use_view::<crate::view::DiskMenuView>();
            disk_menu.sync_entries();
        });
        self
    }

    pub fn sync_entries(&self) {
        let catalog = shared::disk_catalog.lock().unwrap().clone();
        sync_disk_menu_entries(&self.ui, &catalog);
    }
}

pub fn show_context_menu() {
    if let Ok(mut shown_at) = LAST_MENU_SHOW_AT.get_or_init(|| Mutex::new(Instant::now())).lock() {
        *shown_at = Instant::now();
    }
    let menu = ui::use_view::<crate::view::MenuView>();
    menu.sync_from_settings();
    apply_main_menu_geometry(&menu.ui);
    let _ = menu.ui.show();
    hide_secondary_menus();
}

pub fn sync_disk_menu_entries(menu: &ui::DiskMenuWindow, options: &[shared::DiskOption]) {
    let selected = shared::app_settings.lock().unwrap().monitored_disk_ids.clone();
    let entries = options
        .iter()
        .map(|disk| ui::DiskMenuEntry {
            id: disk.id.clone().into(),
            name: disk.name.clone().into(),
            checked: selected.iter().any(|selected_id| selected_id == &disk.id),
        })
        .collect::<Vec<_>>();
    menu.set_entries(ModelRc::from(entries.as_slice()));
}

fn show_submenu(kind: ui::SubmenuKind, item_offset_y: i32) {
    let menu = ui::use_view::<crate::view::MenuView>();
    if !menu.ui.window().is_visible() {
        return;
    }

    let submenu = ui::use_view::<crate::view::SubmenuView>();
    submenu.sync_from_settings();
    submenu.ui.set_kind(kind);

    let hwnd = shared::win32_info.try_lock().unwrap().hwnd;
    if let Some(work_area) = tools::get_work_area(hwnd) {
        let main_pos = menu.ui.window().position();
        let main_size = menu.ui.window().size();
        let submenu_size = submenu.ui.window().size();
        let (x, y) = tools::get_submenu_position(
            (main_pos.x, main_pos.y),
            (main_size.width as i32, main_size.height as i32),
            (submenu_size.width as i32, submenu_size.height as i32),
            item_offset_y,
            work_area,
        );
        submenu.ui.window().set_position(slint::PhysicalPosition::new(x, y));
    }
    hide_disk_menu();
    let _ = submenu.ui.show();
}

fn show_disk_menu(item_offset_y: i32) {
    let submenu = ui::use_view::<crate::view::SubmenuView>();
    if !submenu.ui.window().is_visible() || submenu.ui.get_kind() != ui::SubmenuKind::Display {
        return;
    }

    let disk_menu = ui::use_view::<crate::view::DiskMenuView>();
    disk_menu.sync_entries();

    let hwnd = shared::win32_info.try_lock().unwrap().hwnd;
    if let Some(work_area) = tools::get_work_area(hwnd) {
        let main_menu = ui::use_view::<crate::view::MenuView>();
        let main_pos = main_menu.ui.window().position();
        let main_size = main_menu.ui.window().size();
        let sub_pos = submenu.ui.window().position();
        let sub_size = submenu.ui.window().size();
        let disk_size = disk_menu.ui.window().size();
        let (x, y) = get_third_menu_position(
            (main_pos.x, main_pos.y),
            (main_size.width as i32, main_size.height as i32),
            (sub_pos.x, sub_pos.y),
            (sub_size.width as i32, sub_size.height as i32),
            (disk_size.width as i32, disk_size.height as i32),
            item_offset_y,
            work_area,
        );
        disk_menu.ui.window().set_position(slint::PhysicalPosition::new(x, y));
    }

    let _ = disk_menu.ui.show();
}

fn get_third_menu_position(
    main_pos: (i32, i32),
    main_size: (i32, i32),
    sub_pos: (i32, i32),
    sub_size: (i32, i32),
    menu_size: (i32, i32),
    item_offset_y: i32,
    work_area: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (wa_left, wa_top, wa_right, wa_bottom) = work_area;
    let gap = 6;
    let submenu_is_right = sub_pos.0 >= main_pos.0 + main_size.0;
    let try_right = submenu_is_right;

    let candidate_right = sub_pos.0 + sub_size.0 + gap;
    let candidate_left = sub_pos.0 - menu_size.0 - gap;

    let x = if try_right {
        if candidate_right + menu_size.0 <= wa_right {
            candidate_right
        } else {
            candidate_left.max(wa_left)
        }
    } else if candidate_left >= wa_left {
        candidate_left
    } else {
        candidate_right.min(wa_right - menu_size.0)
    };

    let y = (sub_pos.1 + item_offset_y).clamp(wa_top, wa_bottom - menu_size.1);
    (x, y)
}

fn hide_secondary_menus() {
    hide_disk_menu();
    hide_submenu();
}

fn hide_submenu() {
    let submenu = ui::use_view::<crate::view::SubmenuView>();
    let _ = submenu.ui.hide();
}

fn hide_disk_menu() {
    let disk_menu = ui::use_view::<crate::view::DiskMenuView>();
    let _ = disk_menu.ui.hide();
}

fn hide_all_menus() {
    let menu = ui::use_view::<crate::view::MenuView>();
    let submenu = ui::use_view::<crate::view::SubmenuView>();
    let disk_menu = ui::use_view::<crate::view::DiskMenuView>();
    let _ = disk_menu.ui.hide();
    let _ = submenu.ui.hide();
    let _ = menu.ui.hide();
}

fn apply_main_menu_geometry(menu: &ui::MenuWindow) {
    let hwnd = shared::win32_info.try_lock().unwrap().hwnd;
    if let Some(work_area) = tools::get_work_area(hwnd) {
        let size = menu.window().size();
        let position = tools::get_menu_position((size.width as i32, size.height as i32), work_area);
        menu.window().set_position(slint::PhysicalPosition::new(position.0, position.1));
    }
}

fn window_contains_point<C: ComponentHandle>(view: &C, x: i32, y: i32) -> bool {
    let pos = view.window().position();
    let size = view.window().size();
    x >= pos.x && x <= pos.x + size.width as i32 && y >= pos.y && y <= pos.y + size.height as i32
}

fn update_visibility<F>(mutator: F)
where
    F: FnOnce(&mut shared::AppSettings),
{
    let settings = {
        let mut settings = shared::app_settings.lock().unwrap();
        mutator(&mut settings);
        settings.clone()
    };
    let app = ui::use_view::<crate::view::AppView>();
    app_view::apply_store_settings(&app.ui, &settings);
    let submenu = ui::use_view::<crate::view::SubmenuView>();
    submenu.sync_from_settings();
}

fn update_window_settings<F>(mutator: F)
where
    F: FnOnce(&mut shared::AppSettings),
{
    let settings = {
        let mut settings = shared::app_settings.lock().unwrap();
        mutator(&mut settings);
        settings.clone()
    };
    let app = ui::use_view::<crate::view::AppView>();
    app.ui.set_always_on_top_state(settings.always_on_top);
    if let Ok(info) = shared::win32_info.try_lock() {
        if info.hwnd != 0 {
            tools::set_window_opacity(info.hwnd, settings.opacity);
        }
    }
    let submenu = ui::use_view::<crate::view::SubmenuView>();
    submenu.sync_from_settings();
}
