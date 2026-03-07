use std::sync::atomic::{AtomicBool, Ordering};

use slint::{ComponentHandle, RenderingState};

use crate::{base, shared, tools, ui, view};

pub struct AppView {
    pub ui: ui::AppWindow,
}

unsafe impl Send for AppView {}
unsafe impl Sync for AppView {}

impl Default for AppView {
    fn default() -> Self {
        Self::new()
    }
}

impl AppView {
    pub fn new() -> Self {
        Self { ui: ui::AppWindow::new().unwrap() }.setup()
    }

    pub fn setup(self) -> Self {
        self.setup_window_events().set_position()
    }

    fn set_position(self) -> Self {
        let weak = self.ui.as_weak();
        self.ui
            .window()
            .set_rendering_notifier({
                let initialized = AtomicBool::new(false);
                move |state, _graphics_api| match state {
                    RenderingState::BeforeRendering if !initialized.swap(true, Ordering::SeqCst) => {
                        if let Some(ui) = weak.upgrade() {
                            let settings = shared::app_settings.lock().unwrap().clone();
                            apply_theme(&ui, settings.theme);
                            apply_store_settings(&ui, &settings);
                            ui.set_always_on_top_state(settings.always_on_top);
                            tools::set_prevent_sleep(settings.prevent_sleep);

                            if let Ok(info) = shared::win32_info.try_lock() {
                                if info.hwnd != 0 {
                                    tools::set_window_opacity(info.hwnd, settings.opacity);
                                    tools::set_mouse_passthrough(info.hwnd, settings.mouse_passthrough);

                                    let size = ui.window().size();
                                    let width = size.width as i32;
                                    let height = size.height as i32;

                                    if let Some((wa_left, wa_top, wa_right, wa_bottom)) = tools::get_work_area(info.hwnd) {
                                        let x = (wa_right - width).max(wa_left);
                                        let y = (wa_bottom - height).max(wa_top);
                                        ui.window().set_position(slint::PhysicalPosition::new(x, y));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            })
            .expect("set_rendering_notifier error");
        self
    }

    fn setup_window_events(self) -> Self {
        let weak = self.ui.as_weak();

        self.ui.on_win_move({
            let weak = weak.clone();
            move |delta_x, delta_y| {
                let threshold = 20.0;
                if delta_x.abs() > threshold && delta_y.abs() > threshold {
                    return;
                }
                if let Some(view_inst) = weak.upgrade() {
                    let window = view_inst.window();
                    let scale_factor = window.scale_factor();
                    let logical_pos = window.position().to_logical(scale_factor);
                    window.set_position(slint::LogicalPosition::new(logical_pos.x + delta_x, logical_pos.y + delta_y));
                }
            }
        });

        self.ui.on_win_move_up({
            let weak = weak.clone();
            move || {
                let snap_enabled = shared::app_settings.lock().map(|settings| settings.snap_to_edge).unwrap_or(true);
                if !snap_enabled {
                    return;
                }

                if let Some(view_inst) = weak.upgrade() {
                    let window = view_inst.window();
                    let win32 = base::shared::win32_info.try_lock().expect("lock failed");
                    if let Some(size) = base::tools::get_size(win32.hwnd) {
                        let cur_x = size.0;
                        let cur_y = size.1;
                        let width = size.2 - size.0;
                        let height = size.3 - size.1;

                        if let Some((wa_left, wa_top, wa_right, wa_bottom)) = base::tools::get_work_area(win32.hwnd) {
                            let mut target_x = cur_x;
                            let mut target_y = cur_y;
                            let snap_dist = 50;

                            if cur_x < (wa_left + snap_dist) {
                                target_x = wa_left;
                            } else if (cur_x + width) > (wa_right - snap_dist) {
                                target_x = wa_right - width;
                            }

                            if cur_y < (wa_top + snap_dist) {
                                target_y = wa_top;
                            } else if (cur_y + height) > (wa_bottom - snap_dist) {
                                target_y = wa_bottom - height;
                            }

                            if target_x != cur_x || target_y != cur_y {
                                window.set_position(slint::PhysicalPosition::new(target_x, target_y));
                            }
                        }
                    }
                }
            }
        });

        self.ui.on_show_menu(move |_, _| {
            view::show_context_menu();
        });

        self
    }
}

pub fn apply_store_settings(view: &ui::AppWindow, settings: &shared::AppSettings) {
    let store = view.global::<ui::Store>();
    store.set_show_cpu(settings.show_cpu);
    store.set_show_memory(settings.show_memory);
    store.set_show_disk_total(settings.show_disk_total);
    store.set_show_disk_io(settings.show_disk_io);
    store.set_show_network(settings.show_network);
}

pub fn apply_theme(view: &ui::AppWindow, theme: shared::ThemeKind) {
    let theme_global = view.global::<ui::Theme>();
    theme_global.set_mode(to_ui_theme_mode(theme));
}

pub fn to_ui_theme_mode(theme: shared::ThemeKind) -> ui::ThemeMode {
    match theme {
        shared::ThemeKind::Dark => ui::ThemeMode::Dark,
        shared::ThemeKind::Light => ui::ThemeMode::Light,
    }
}

pub fn from_ui_theme_mode(theme: ui::ThemeMode) -> shared::ThemeKind {
    match theme {
        ui::ThemeMode::Dark => shared::ThemeKind::Dark,
        ui::ThemeMode::Light => shared::ThemeKind::Light,
    }
}
