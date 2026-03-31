use slint::{ComponentHandle, Model, ModelRc};
use anyhow::{Context, Ok, Result};
use crate::{task, ui, view::ViewTrait};

use super::{calc_menu_show_position, close_menus};

pub struct Menu3View {
    pub ui: ui::Menu3Window,
}

impl ViewTrait for Menu3View {
    fn new() -> Self {
        let ui = ui::Menu3Window::new().unwrap();
        Self { ui }.bind_event()
    }

    fn show(&self, extra: Option<&dyn std::any::Any>) -> Result<()> {
        let (parent_pos_x, parent_pos_y, offset_y) = extra.and_then(|e| e.downcast_ref::<(f32, f32, f32)>()).context("extra error")?;
        let app_view = ui::use_view::<crate::view::AppView>();
        task::refresh_disk_menu(&app_view.ui);
        let app_store = app_view.ui.global::<ui::Store>();
        let model = app_store.get_disk_menu_entries();
        let mut entries = Vec::new();
        for i in 0..model.row_count() {
            if let Some(entry) = model.row_data(i) {
                entries.push(entry);
            }
        }
        let store = self.ui.global::<ui::Store>();
        store.set_disk_menu_entries(ModelRc::from(entries.as_slice()));
        store.set_has_monitored_disks(app_store.get_has_monitored_disks());
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        store.set_theme_mode(theme_mode);
        let Some((x, y)) = calc_menu_show_position(*parent_pos_x, *parent_pos_y, *offset_y) else {
            return Ok(());
        };
        self.ui.window().set_position(slint::PhysicalPosition::new(x, y));
        let _ = self.ui.show();
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {}

    fn bind_event(self) -> Self {
        let weak = self.ui.as_weak();
        self.ui.on_close_menu(|| {
            close_menus(1);
        });
        self.ui.on_toggle_disk({
            let weak = weak.clone();
            move |disk_id| {
                let Some(menu3) = weak.upgrade() else {
                    return;
                };

                let menu3_store = menu3.global::<ui::Store>();
                let model = menu3_store.get_disk_menu_entries();
                let mut entries = Vec::new();
                for i in 0..model.row_count() {
                    if let Some(mut entry) = model.row_data(i) {
                        if entry.id == disk_id {
                            entry.checked = !entry.checked;
                        }
                        entries.push(entry);
                    }
                }
                let has_monitored = entries.iter().any(|entry| entry.checked);
                menu3_store.set_disk_menu_entries(ModelRc::from(entries.as_slice()));
                menu3_store.set_has_monitored_disks(has_monitored);

                let app_view = ui::use_view::<crate::view::AppView>();
                let app_store = app_view.ui.global::<ui::Store>();
                app_store.set_disk_menu_entries(ModelRc::from(entries.as_slice()));
                app_store.set_has_monitored_disks(has_monitored);
            }
        });
        self
    }


    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
