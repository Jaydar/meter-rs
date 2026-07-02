use anyhow::{Context, Result};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use tracing::error;
use winit::platform::windows::WindowAttributesExtWindows;

use crate::{MAIN_HWND, base, task, tools, ui, view::ViewTrait};

pub struct AppView {
    pub ui: ui::AppWindow,
}

impl AppView {
    pub fn init_backend() -> Result<()> {
        i_slint_backend_selector::api::BackendSelector::new().with_winit_window_attributes_hook(|attr| attr.with_skip_taskbar(true)).select()?;
        Ok(())
    }
}

impl ViewTrait for AppView {
    fn new() -> Self {
        let ui = match ui::AppWindow::new().context("create AppWindow failed") {
            Ok(ui) => ui,
            Err(err) => panic!("{}", err),
        };
        Self { ui }.bind_event()
    }

    fn show(&self, _extra: Option<&dyn std::any::Any>) -> Result<()> {
        crate::base::config::load(&self.ui);
        self.ui.show()?;
        self.set_position();
        let hwnd = MAIN_HWND.load(Ordering::Relaxed);
        if hwnd != 0 && self.ui.global::<ui::ConfigStore>().get_mouse_passthrough() {
            tools::set_mouse_passthrough(hwnd, true);
        }
        self.ui.global::<ui::ConfigStore>().set_auto_start(tools::is_auto_start());
        task::start_monitor(&self.ui);
        slint::run_event_loop()?;
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {
        let weak = self.ui.as_weak();

        let result = slint::spawn_local(async move {
            let Some(ui) = weak.upgrade() else {
                return;
            };

            if ui.window().winit_window().await.is_ok() {
                let hwnd = MAIN_HWND.load(Ordering::Relaxed);
                if hwnd == 0 {
                    return;
                }

                if let Some((wa_left, wa_top, wa_right, wa_bottom)) = tools::get_work_area(hwnd) {
                    if let Some(taskbar_height) = tools::get_taskbar_height(hwnd) {
                        let scale_factor = ui.window().scale_factor();
                        ui.global::<ui::RuntimeStore>().set_taskbar_height(((taskbar_height as f32 - 6.0).max(1.0)) / scale_factor);
                    }
                    let size = ui.window().size();
                    let width = size.width as i32;
                    let height = size.height as i32;
                    let x = (wa_right - width).max(wa_left);
                    let y = (wa_bottom - height).max(wa_top);
                    ui.window().set_position(slint::PhysicalPosition::new(x, y));
                }
            }
        });
        if let Err(err) = result {
            error!("spawn app position task failed: {}", err);
        }
    }

    fn bind_event(self) -> Self {
        let weak = self.ui.as_weak();

        self.ui.on_win_move({
            let weak = weak.clone();
            move |delta_x, delta_y| {
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
                if let Some(view_inst) = weak.upgrade() {
                    if !view_inst.global::<ui::ConfigStore>().get_snap_to_edge() {
                        return;
                    }
                    let window = view_inst.window();
                    let hwnd = MAIN_HWND.load(Ordering::Relaxed);
                    if hwnd == 0 {
                        return;
                    }
                    if let Some(size) = base::tools::get_size(hwnd) {
                        let cur_x = size.0;
                        let cur_y = size.1;
                        let width = size.2 - size.0;
                        let height = size.3 - size.1;

                        let snap_area = if view_inst.global::<ui::ConfigStore>().get_snap_mode() == ui::SnapMode::FullScreen { base::tools::get_monitor_area(hwnd) } else { base::tools::get_work_area(hwnd) };
                        if let Some((wa_left, wa_top, wa_right, wa_bottom)) = snap_area {
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

        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

