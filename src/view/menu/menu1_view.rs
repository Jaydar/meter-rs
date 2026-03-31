use anyhow::Ok;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use anyhow::Result;

use crate::{
    tools, ui::{self}, view::{AboutView, ViewTrait}, MAIN_HWND,
};

use super::{close_menus, listen_menu_close, Menu2View};



pub struct Menu1View {
    pub ui: ui::Menu1Window,
}
impl Menu1View {
    fn sync_store(&self) {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::Store>();
        self.ui.set_auto_start_state(app_store.get_auto_start());
        self.ui.set_mouse_passthrough_state(app_store.get_mouse_passthrough());
        self.ui.set_prevent_sleep_state(app_store.get_prevent_sleep());
    }
}

impl ViewTrait for Menu1View {
    fn new() -> Self {
        let ui = ui::Menu1Window::new().unwrap();
        Self { ui }.bind_event()
    }

    fn show(&self, _extra: Option<&dyn std::any::Any>) -> Result<()> {
        
        self.sync_store();
        
        let _ = self.ui.show();
        let next_height_bias = if self.ui.get_height_bias() == 0.0 { 0.1 } else { 0.0 };
        self.ui.set_height_bias(next_height_bias);
        self.set_position();
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {
        slint::spawn_local({
            let weak = self.ui.as_weak();
            
            async move {

                let Some(menu) = weak.upgrade() else {
                    return;
                };

                let hwnd = MAIN_HWND.load(Ordering::Relaxed);
                    if hwnd == 0 {
                    return;
                }

                if let Some(winit_win) = menu.window().winit_window().await.ok() {
                    winit_win.focus_window();
                    listen_menu_close();
                }
           
                if let Some(work_area) = tools::get_work_area(hwnd) {
                    let size = menu.window().size();
               
                    let position = tools::get_menu_position((size.width as i32, size.height as i32), work_area);
                    menu.window().set_position(slint::PhysicalPosition::new(position.0, position.1));
                }

            }
        }).unwrap();
    }

    fn bind_event(self) -> Self {
        let weak = self.ui.as_weak();

        self.ui.on_close_menu(|| {
            close_menus(1);
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
        self.ui.on_show_theme_submenu({
            let weak = weak.clone();
            move |_, offset_y| {
                if let Some(menu) = weak.upgrade() {
                    menu.set_active_submenu(0);
                    let pos = menu.window().position();
                    let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                    let menu2 = ui::use_view::<Menu2View>();
                    let extra = (ui::SubmenuKind::Theme, pos.x as f32, pos.y as f32, scaled_y);
                    let _ = menu2.show(Some(&extra));
                }
            }
        });
        self.ui.on_show_display_submenu({
            let weak = weak.clone();
            move |_, offset_y| {
                if let Some(menu) = weak.upgrade() {
                    menu.set_active_submenu(1);
                    let pos = menu.window().position();
                    let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                    let menu2 = ui::use_view::<Menu2View>();
                    let extra = (ui::SubmenuKind::Display, pos.x as f32, pos.y as f32, scaled_y);
                    let _ = menu2.show(Some(&extra));
                }
            }
        });
        self.ui.on_show_window_submenu({
            let weak = weak.clone();
            move |_, offset_y| {
                if let Some(menu) = weak.upgrade() {
                    menu.set_active_submenu(2);
                    let pos = menu.window().position();
                    let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                    let menu2 = ui::use_view::<Menu2View>();
                    let extra = (ui::SubmenuKind::Window, pos.x as f32, pos.y as f32, scaled_y);
                    let _ = menu2.show(Some(&extra));
                }
            }
        });
        self.ui.on_hide_submenu(|| {
            let menu_view = ui::use_view::<crate::view::Menu1View>();
            menu_view.ui.set_active_submenu(-1);
            close_menus(2);
        });

        self.ui.on_show_about(|| {
            let about = ui::use_view::<AboutView>();
            let _ = about.show(None);
        });

        self

    }



    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
