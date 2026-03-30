use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use winit::platform::windows::WindowAttributesExtWindows;
use std::sync::atomic::Ordering;
use anyhow::Result;

use crate::{
    MAIN_HWND, base, task, tools, ui, view::{Menu1View, ViewTrait}
};

pub struct AppView {
    pub ui: ui::AppWindow,
}



impl AppView {

    pub fn init_backend() -> Result<()>{
        println!("11111111111");
        i_slint_backend_selector::api::BackendSelector::new()
        .with_winit_window_attributes_hook(|attr| attr.with_skip_taskbar(true))
        .select()?;
        
        Ok(())
    }
}

impl ViewTrait for AppView {
    fn new() -> Self {
        Self { ui: ui::AppWindow::new().unwrap() }.bind_event()
    }

    fn show(&self) -> Result<()> {
        self.ui.show()?;
        self.set_position();
        task::start_monitor(&self.ui);
        slint::run_event_loop()?;
        Ok(())
    }

    fn hide(&self) {
        let _ =self.ui.hide();
    }

    fn set_position(&self) {
        let weak = self.ui.as_weak();

        slint::spawn_local(async move {

            let Some(ui) = weak.upgrade() else {
                return;
            };

            if ui.window().winit_window().await.is_ok() {

                let hwnd = MAIN_HWND.load(Ordering::Relaxed);
                if hwnd == 0 {
                    return;
                }

                let size = ui.window().size();
                let width = size.width as i32;
                let height = size.height as i32;

                if let Some((wa_left, wa_top, wa_right, wa_bottom)) = tools::get_work_area(hwnd) {
                    let x = (wa_right - width).max(wa_left);
                    let y = (wa_bottom - height).max(wa_top);
                    ui.window().set_position(slint::PhysicalPosition::new(x, y));
                }
            }

        }).unwrap();

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
                    if !view_inst.global::<ui::Store>().get_snap_to_edge() {
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

                        if let Some((wa_left, wa_top, wa_right, wa_bottom)) = base::tools::get_work_area(hwnd) {
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
            let menu_view = ui::use_view::<Menu1View>();
            menu_view.show();
        });

        self
    }

    fn sync_store(&self) {

    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
