use slint::{ComponentHandle, ModelRc};

use crate::{shared, ui, view::AppView};

use super::{MenuView, SubmenuView};

pub struct DiskMenuView {
    pub ui: ui::DiskMenuWindow,
}

impl Default for DiskMenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskMenuView {
    pub fn new() -> Self {
        let ui = ui::DiskMenuWindow::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        self.sync_entries();
        self.ui.on_close_menu(MenuView::hide_all_menus);
        self.ui.on_toggle_disk(|disk_id| {
            {
                let mut settings = shared::app_settings.lock().unwrap();
                let disk_id = disk_id.to_string();
                if let Some(index) = settings
                    .monitored_disk_ids
                    .iter()
                    .position(|id| id == &disk_id)
                {
                    settings.monitored_disk_ids.remove(index);
                } else {
                    settings.monitored_disk_ids.push(disk_id);
                }
            }

            let settings = shared::app_settings.lock().unwrap().clone();
            let app = &ui::use_view::<crate::view::AppView>().ui;
            AppView::apply_store_settings(app, &settings);
            SubmenuView::sync_registered();
            let disk_menu = &ui::use_view::<DiskMenuView>().ui;
            Self::sync_entries_with_options(disk_menu, &shared::disk_catalog.lock().unwrap().clone());
        });
        self
    }

    pub fn sync_entries(&self) {
        let catalog = shared::disk_catalog.lock().unwrap().clone();
        Self::sync_entries_with_options(&self.ui, &catalog);
    }

    pub fn sync_entries_with_options(menu: &ui::DiskMenuWindow, options: &[shared::DiskOption]) {
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

    pub fn show(item_offset_y: i32) {
        let submenu = &ui::use_view::<SubmenuView>().ui;
        if !submenu.window().is_visible() || submenu.get_kind() != ui::SubmenuKind::Display {
            return;
        }

        let disk_menu = &ui::use_view::<DiskMenuView>().ui;
        let catalog = shared::disk_catalog.lock().unwrap().clone();
        Self::sync_entries_with_options(disk_menu, &catalog);

        let hwnd = shared::win32_info.try_lock().map(|info| info.hwnd).unwrap_or(0);
        if hwnd != 0 {
            if let Some(work_area) = crate::tools::get_work_area(hwnd) {
                let main_menu = &ui::use_view::<MenuView>().ui;
                let main_pos = main_menu.window().position();
                let main_size = main_menu.window().size();
                let sub_pos = submenu.window().position();
                let sub_size = submenu.window().size();
                let disk_size = disk_menu.window().size();
                let (x, y) = Self::get_third_menu_position(
                    (main_pos.x, main_pos.y),
                    (main_size.width as i32, main_size.height as i32),
                    (sub_pos.x, sub_pos.y),
                    (sub_size.width as i32, sub_size.height as i32),
                    (disk_size.width as i32, disk_size.height as i32),
                    item_offset_y,
                    work_area,
                );
                disk_menu
                    .window()
                    .set_position(slint::PhysicalPosition::new(x, y));
            }
        }

        let _ = disk_menu.show();
    }

    pub fn hide() {
        let disk_menu = &ui::use_view::<DiskMenuView>().ui;
        let _ = disk_menu.hide();
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
}
