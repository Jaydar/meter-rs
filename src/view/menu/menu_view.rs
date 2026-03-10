use slint::{ComponentHandle, RenderingState, Timer, TimerMode};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::{shared, tools, ui};

use super::{DiskMenuView, SubmenuView};
use crate::view::AboutView;

static LAST_MENU_SHOW_AT: OnceLock<Mutex<Instant>> = OnceLock::new();

pub struct MenuView {
    pub ui: ui::MenuWindow,
    close_timer: Timer,
}

impl Default for MenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuView {
    pub fn new() -> Self {
        let ui = ui::MenuWindow::new().unwrap();
        Self {
            ui,
            close_timer: Timer::default(),
        }
        .setup()
    }

    fn setup(self) -> Self {
        self.sync_from_settings();
        let weak_handle = self.ui.as_weak();

        self.ui.on_close_menu(Self::hide_all_menus);
        self.ui.on_close_app(|| {
            let _ = slint::quit_event_loop();
        });
        self.ui.on_set_auto_start(|enable| {
            tools::set_auto_start(enable);
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.auto_start = enable;
            }
        });
        self.ui.on_set_mouse_passthrough(|enable| {
            let hwnd = shared::win32_info.try_lock().map(|info| info.hwnd).unwrap_or(0);
            if hwnd != 0 {
                tools::set_mouse_passthrough(hwnd, enable);
            }
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.mouse_passthrough = enable;
            }
        });
        self.ui.on_set_prevent_sleep(|enable| {
            tools::set_prevent_sleep(enable);
            if let Ok(mut settings) = shared::app_settings.lock() {
                settings.prevent_sleep = enable;
            }
        });
        self.ui.on_turn_off_display(tools::turn_off_display);
        self.ui.on_restart_explorer(tools::restart_explorer);
        self.ui
            .on_show_theme_submenu(|offset_y| SubmenuView::show(ui::SubmenuKind::Theme, offset_y as i32));
        self.ui
            .on_show_display_submenu(|offset_y| SubmenuView::show(ui::SubmenuKind::Display, offset_y as i32));
        self.ui
            .on_show_window_submenu(|offset_y| SubmenuView::show(ui::SubmenuKind::Window, offset_y as i32));
        self.ui.on_hide_submenu(Self::hide_secondary_menus);
        self.ui.on_show_about(AboutView::show);

        self.close_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(150),
            move || {
                if let Some(menu) = weak_handle.upgrade() {
                    if !menu.window().is_visible() {
                        return;
                    }
                    if Self::shown_recently() {
                        return;
                    }

                    let submenu = &ui::use_view::<SubmenuView>().ui;
                    let disk_menu = &ui::use_view::<DiskMenuView>().ui;
                    let mouse = tools::get_current_mouse_position();

                    let submenu_visible = submenu.window().is_visible();
                    let disk_visible = disk_menu.window().is_visible();

                    let mouse_in_main = Self::window_contains_point(&menu, mouse.x, mouse.y);
                    let mouse_in_sub =
                        submenu_visible && Self::window_contains_point(submenu, mouse.x, mouse.y);
                    let mouse_in_disk =
                        disk_visible && Self::window_contains_point(disk_menu, mouse.x, mouse.y);

                    let menu_hwnd = tools::get_hwnd_by_window_handle(&menu);
                    let submenu_hwnd = if submenu_visible {
                        tools::get_hwnd_by_window_handle(submenu)
                    } else {
                        None
                    };
                    let disk_hwnd = if disk_visible {
                        tools::get_hwnd_by_window_handle(disk_menu)
                    } else {
                        None
                    };

                    let main_active = menu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                    let sub_active = submenu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                    let disk_active = disk_hwnd.map(tools::is_window_foreground).unwrap_or(false);

                    if !mouse_in_main
                        && !mouse_in_sub
                        && !mouse_in_disk
                        && !main_active
                        && !sub_active
                        && !disk_active
                    {
                        Self::hide_all_menus();
                    }
                }
            },
        );

        self.set_position()
    }

    pub fn sync_from_settings(&self) {
        Self::sync_window_from_settings(&self.ui);
    }

    fn sync_window_from_settings(menu: &ui::MenuWindow) {
        let settings = shared::app_settings.lock().unwrap().clone();
        menu.set_auto_start_state(settings.auto_start);
        menu.set_mouse_passthrough_state(settings.mouse_passthrough);
        menu.set_prevent_sleep_state(settings.prevent_sleep);
    }

    pub fn show_context_menu() {
        Self::mark_shown_now();
        let menu = &ui::use_view::<MenuView>().ui;
        Self::sync_window_from_settings(&menu);
        Self::apply_main_menu_geometry(&menu);
        let _ = menu.show();
        Self::hide_secondary_menus();
    }

    pub fn hide_all_menus() {
        let disk_menu = &ui::use_view::<DiskMenuView>().ui;
        let _ = disk_menu.hide();
        let submenu = &ui::use_view::<SubmenuView>().ui;
        let _ = submenu.hide();
        let menu = &ui::use_view::<MenuView>().ui;
        let _ = menu.hide();
    }

    pub(crate) fn hide_secondary_menus() {
        DiskMenuView::hide();
        SubmenuView::hide();
    }

    pub(crate) fn apply_main_menu_geometry(menu: &ui::MenuWindow) {
        let hwnd = shared::win32_info.try_lock().map(|info| info.hwnd).unwrap_or(0);
        if hwnd == 0 {
            return;
        }

        if let Some(work_area) = tools::get_work_area(hwnd) {
            let size = menu.window().size();
            let position =
                tools::get_menu_position((size.width as i32, size.height as i32), work_area);
            menu.window()
                .set_position(slint::PhysicalPosition::new(position.0, position.1));
        }
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
                            Self::apply_main_menu_geometry(&menu);
                        }
                    }
                    _ => {}
                }
            })
            .expect("set_rendering_notifier error");
        self
    }

    fn mark_shown_now() {
        if let Ok(mut shown_at) = LAST_MENU_SHOW_AT
            .get_or_init(|| Mutex::new(Instant::now()))
            .lock()
        {
            *shown_at = Instant::now();
        }
    }

    fn shown_recently() -> bool {
        let shown_at = LAST_MENU_SHOW_AT
            .get_or_init(|| Mutex::new(Instant::now()))
            .lock()
            .map(|instant| *instant)
            .unwrap_or_else(|_| Instant::now());
        shown_at.elapsed() < Duration::from_millis(500)
    }

    fn window_contains_point<C: ComponentHandle>(view: &C, x: i32, y: i32) -> bool {
        let pos = view.window().position();
        let size = view.window().size();
        x >= pos.x && x <= pos.x + size.width as i32 && y >= pos.y && y <= pos.y + size.height as i32
    }
}
