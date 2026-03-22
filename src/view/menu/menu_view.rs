use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, RenderingState, Timer, TimerMode};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use crate::{tools, trim_memory, ui, MAIN_HWND};

use super::{Menu2View, Menu3View};
use crate::view::AboutView;

static LAST_MENU_SHOW_AT: OnceLock<Mutex<Instant>> = OnceLock::new();
const MENU_HEIGHT_BIAS: f32 = 0.01;
const MENU_GEOMETRY_RETRY_DELAY: Duration = Duration::from_millis(16);
const MENU_GEOMETRY_RETRY_ATTEMPTS: u8 = 30;

fn mark_shown_now_inner() {
    if let Ok(mut shown_at) = LAST_MENU_SHOW_AT.get_or_init(|| Mutex::new(Instant::now())).lock() {
        *shown_at = Instant::now();
    }
}

fn shown_recently_inner() -> bool {
    let shown_at = LAST_MENU_SHOW_AT.get_or_init(|| Mutex::new(Instant::now())).lock().map(|instant| *instant).unwrap_or_else(|_| Instant::now());
    shown_at.elapsed() < Duration::from_millis(500)
}

fn window_contains_point_inner<C: ComponentHandle>(view: &C, x: i32, y: i32) -> bool {
    let pos = view.window().position();
    let size = view.window().size();
    x >= pos.x && x <= pos.x + size.width as i32 && y >= pos.y && y <= pos.y + size.height as i32
}

fn menu_has_valid_size_inner(menu: &ui::MenuWindow) -> bool {
    let size = menu.window().size();
    size.width > 0 && size.height > 0
}

fn apply_main_menu_geometry_inner(menu: &ui::MenuWindow) {
    let hwnd = MAIN_HWND.load(Ordering::Relaxed);
    if hwnd == 0 {
        return;
    }
    if let Some(work_area) = tools::get_work_area(hwnd) {
        let size = menu.window().size();
        let position = tools::get_menu_position((size.width as i32, size.height as i32), work_area);
        menu.window().set_position(slint::PhysicalPosition::new(position.0, position.1));
    }
}

fn ensure_main_menu_geometry_inner(menu: &ui::MenuWindow) {
    if menu_has_valid_size_inner(menu) {
        apply_main_menu_geometry_inner(menu);
        menu.window().request_redraw();
        return;
    }

    retry_main_menu_geometry_inner(menu.as_weak(), MENU_GEOMETRY_RETRY_ATTEMPTS);
}

fn retry_main_menu_geometry_inner(menu: slint::Weak<ui::MenuWindow>, remaining_attempts: u8) {
    Timer::single_shot(MENU_GEOMETRY_RETRY_DELAY, move || {
        let Some(menu) = menu.upgrade() else {
            return;
        };
        if !menu.window().is_visible() {
            return;
        }
        if menu_has_valid_size_inner(&menu) {
            apply_main_menu_geometry_inner(&menu);
            menu.window().request_redraw();
        } else if remaining_attempts > 0 {
            menu.window().request_redraw();
            retry_main_menu_geometry_inner(menu.as_weak(), remaining_attempts - 1);
        }
    });
}

fn hide_secondary_menus_inner() {
    let menu3 = ui::use_view::<Menu3View>();
    menu3.hide();
    let menu2 = ui::use_view::<Menu2View>();
    menu2.hide();
}

fn hide_all_menus_inner(menu: &ui::MenuWindow) {
    hide_secondary_menus_inner();
    let _ = menu.hide();
    Timer::single_shot(Duration::from_millis(200), trim_memory);
}

pub struct MenuView {
    pub ui: ui::MenuWindow,
    close_timer: Timer,
}

impl Default for MenuView {
    fn default() -> Self {
        MenuView::new()
    }
}

impl MenuView {
    pub fn new() -> Self {
        let ui = ui::MenuWindow::new().unwrap();
        Self { ui, close_timer: Timer::default() }.setup()
    }

    fn setup(self) -> Self {
        let weak_handle = self.ui.as_weak();
        let weak_close = self.ui.as_weak();
        let weak_theme = self.ui.as_weak();
        let weak_display = self.ui.as_weak();
        let weak_window = self.ui.as_weak();

        self.ui.on_close_menu(move || {
            if let Some(menu) = weak_close.upgrade() {
                hide_all_menus_inner(&menu);
            } else {
                hide_secondary_menus_inner();
            }
        });
        self.ui.on_close_app(|| {
            let _ = slint::quit_event_loop();
        });
        self.ui.on_set_auto_start(|enable| {
            tools::set_auto_start(enable);
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_auto_start(enable);
        });
        self.ui.on_set_mouse_passthrough(|enable| {
            let hwnd = MAIN_HWND.load(Ordering::Relaxed);
            if hwnd != 0 {
                tools::set_mouse_passthrough(hwnd, enable);
            }
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_mouse_passthrough(enable);
        });
        self.ui.on_set_prevent_sleep(|enable| {
            tools::set_prevent_sleep(enable);
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_prevent_sleep(enable);
        });
        self.ui.on_turn_off_display(tools::turn_off_display);
        self.ui.on_restart_explorer(tools::restart_explorer);
        self.ui.on_clean_memory(tools::clean_memory);
        self.ui.on_show_theme_submenu(move |_, offset_y| {
            if let Some(menu) = weak_theme.upgrade() {
                menu.set_active_submenu(0);
                let pos = menu.window().position();
                let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                let menu2 = ui::use_view::<Menu2View>();
                menu2.show(ui::SubmenuKind::Theme, pos.x as f32, pos.y as f32, scaled_y);
            }
        });
        self.ui.on_show_display_submenu(move |_, offset_y| {
            if let Some(menu) = weak_display.upgrade() {
                menu.set_active_submenu(1);
                let pos = menu.window().position();
                let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                let menu2 = ui::use_view::<Menu2View>();
                menu2.show(ui::SubmenuKind::Display, pos.x as f32, pos.y as f32, scaled_y);
            }
        });
        self.ui.on_show_window_submenu(move |_, offset_y| {
            if let Some(menu) = weak_window.upgrade() {
                menu.set_active_submenu(2);
                let pos = menu.window().position();
                let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                let menu2 = ui::use_view::<Menu2View>();
                menu2.show(ui::SubmenuKind::Window, pos.x as f32, pos.y as f32, scaled_y);
            }
        });
        self.ui.on_hide_submenu(|| {
            let menu_view = ui::use_view::<MenuView>();
            menu_view.ui.set_active_submenu(-1);
            hide_secondary_menus_inner();
        });
        self.ui.on_show_about(|| {
            let about = ui::use_view::<AboutView>();
            about.show();
        });

        self.close_timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
            if let Some(menu) = weak_handle.upgrade() {
                if !menu.window().is_visible() {
                    return;
                }
                if shown_recently_inner() {
                    return;
                }

                let mouse = tools::get_current_mouse_position();

                let submenu = &ui::use_view::<Menu2View>().ui;
                let disk_menu = &ui::use_view::<Menu3View>().ui;

                let submenu_visible = submenu.window().is_visible();
                let disk_visible = disk_menu.window().is_visible();

                let mouse_in_main = window_contains_point_inner(&menu, mouse.x, mouse.y);
                let mouse_in_sub = submenu_visible && window_contains_point_inner(submenu, mouse.x, mouse.y);
                let mouse_in_disk = disk_visible && window_contains_point_inner(disk_menu, mouse.x, mouse.y);

                let menu_hwnd = tools::get_hwnd_by_window_handle(&menu);
                let submenu_hwnd = if submenu_visible { tools::get_hwnd_by_window_handle(submenu) } else { None };
                let disk_hwnd = if disk_visible { tools::get_hwnd_by_window_handle(disk_menu) } else { None };

                let main_active = menu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                let sub_active = submenu_hwnd.map(tools::is_window_foreground).unwrap_or(false);
                let disk_active = disk_hwnd.map(tools::is_window_foreground).unwrap_or(false);

                if !mouse_in_main && !mouse_in_sub && !mouse_in_disk && !main_active && !sub_active && !disk_active {
                    hide_all_menus_inner(&menu);
                }
            }
        });

        self.set_position()
    }

    pub fn show(&self) {
        mark_shown_now_inner();
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::Store>();
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::Store>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        self.ui.set_auto_start_state(app_store.get_auto_start());
        self.ui.set_mouse_passthrough_state(app_store.get_mouse_passthrough());
        self.ui.set_prevent_sleep_state(app_store.get_prevent_sleep());
        self.ui.set_active_submenu(-1);
        hide_secondary_menus_inner();
        let next_height_bias = if self.ui.get_height_bias() == 0.0 { MENU_HEIGHT_BIAS } else { 0.0 };
        self.ui.set_height_bias(next_height_bias);
        let _ = self.ui.show();
        let weak = self.ui.as_weak();
        slint::spawn_local(async move {
            let Some(menu) = weak.upgrade() else {
                return;
            };
            let _ = menu.window().winit_window().await.ok();
            ensure_main_menu_geometry_inner(&menu);
        })
        .expect("failed to await menu winit window");
    }

    pub fn hide_all_menus(&self) {
        hide_all_menus_inner(&self.ui);
    }

    fn set_position(self) -> Self {
        let weak = self.ui.as_weak();
        let result = self.ui.window().set_rendering_notifier({
            let weak = weak.clone();
            let initialized = AtomicBool::new(false);
            move |state, _| match state {
                RenderingState::BeforeRendering if !initialized.swap(true, Ordering::SeqCst) => {
                    if let Some(menu) = weak.upgrade() {
                        ensure_main_menu_geometry_inner(&menu);
                    }
                }
                _ => {}
            }
        });
        if let Err(err) = result {
            match err {
                slint::SetRenderingNotifierError::Unsupported => {
                    let weak = weak.clone();
                    slint::spawn_local(async move {
                        let Some(menu) = weak.upgrade() else {
                            return;
                        };
                        let _ = menu.window().winit_window().await.ok();
                        Timer::single_shot(Duration::ZERO, move || {
                            ensure_main_menu_geometry_inner(&menu);
                        });
                    })
                    .expect("failed to await menu winit window for unsupported notifier");
                }
                _ => panic!("set_rendering_notifier error: {err}"),
            }
        }
        self
    }
}
