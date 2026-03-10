use slint::ComponentHandle;

use crate::{shared, tools, ui, view::AppView};

use super::{DiskMenuView, MenuView};

pub struct SubmenuView {
    pub ui: ui::SubmenuWindow,
}

impl Default for SubmenuView {
    fn default() -> Self {
        Self::new()
    }
}

impl SubmenuView {
    pub fn new() -> Self {
        let ui = ui::SubmenuWindow::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        self.sync_from_settings();
        self.ui.on_close_menu(MenuView::hide_all_menus);
        self.ui.on_set_theme(|theme_mode| {
            {
                let mut settings = shared::app_settings.lock().unwrap();
                settings.theme = AppView::from_ui_theme_mode(theme_mode);
            }
            let app = &ui::use_view::<crate::view::AppView>().ui;
            AppView::apply_theme(app, AppView::from_ui_theme_mode(theme_mode));
            let submenu = &ui::use_view::<SubmenuView>().ui;
            submenu.set_theme_state(theme_mode);
        });
        self.ui
            .on_set_show_hostname(|value| Self::update_visibility(|settings| settings.show_hostname = value));
        self.ui
            .on_set_show_cpu(|value| Self::update_visibility(|settings| settings.show_cpu = value));
        self.ui
            .on_set_show_memory(|value| Self::update_visibility(|settings| settings.show_memory = value));
        self.ui.on_set_show_disk_total(|value| {
            Self::update_visibility(|settings| settings.show_disk_total = value)
        });
        self.ui
            .on_set_show_disk_io(|value| Self::update_visibility(|settings| settings.show_disk_io = value));
        self.ui
            .on_set_show_network(|value| Self::update_visibility(|settings| settings.show_network = value));
        self.ui
            .on_show_disk_monitor_submenu(|offset_y| DiskMenuView::show(offset_y as i32));
        self.ui.on_hide_disk_monitor_submenu(DiskMenuView::hide);
        self.ui.on_set_always_on_top(|value| {
            Self::update_window_settings(|settings| settings.always_on_top = value)
        });
        self.ui
            .on_set_snap_to_edge(|value| Self::update_window_settings(|settings| settings.snap_to_edge = value));
        self.ui
            .on_set_opacity(|value| Self::update_window_settings(|settings| settings.opacity = value));
        self
    }

    pub fn sync_from_settings(&self) {
        Self::sync_window_from_settings(&self.ui);
    }

    pub(crate) fn sync_registered() {
        let submenu = &ui::use_view::<SubmenuView>().ui;
        Self::sync_window_from_settings(submenu);
    }

    fn sync_window_from_settings(submenu: &ui::SubmenuWindow) {
        let settings = shared::app_settings.lock().unwrap().clone();
        submenu.set_theme_state(AppView::to_ui_theme_mode(settings.theme));
        submenu.set_show_hostname_state(settings.show_hostname);
        submenu.set_show_cpu_state(settings.show_cpu);
        submenu.set_show_memory_state(settings.show_memory);
        submenu.set_show_disk_total_state(settings.show_disk_total);
        submenu.set_show_disk_io_state(settings.show_disk_io);
        submenu.set_show_network_state(settings.show_network);
        submenu.set_has_monitored_disks(!settings.monitored_disk_ids.is_empty());
        submenu.set_always_on_top_state(settings.always_on_top);
        submenu.set_snap_to_edge_state(settings.snap_to_edge);
        submenu.set_opacity_value(settings.opacity);
    }

    pub fn show(kind: ui::SubmenuKind, item_offset_y: i32) {
        let menu = &ui::use_view::<MenuView>().ui;
        if !menu.window().is_visible() {
            return;
        }

        let submenu = &ui::use_view::<SubmenuView>().ui;
        Self::sync_window_from_settings(submenu);
        submenu.set_kind(kind);

        let hwnd = shared::win32_info.try_lock().map(|info| info.hwnd).unwrap_or(0);
        if hwnd != 0 {
            if let Some(work_area) = tools::get_work_area(hwnd) {
                let main_pos = menu.window().position();
                let main_size = menu.window().size();
                let submenu_size = submenu.window().size();
                let (x, y) = tools::get_submenu_position(
                    (main_pos.x, main_pos.y),
                    (main_size.width as i32, main_size.height as i32),
                    (submenu_size.width as i32, submenu_size.height as i32),
                    item_offset_y,
                    work_area,
                );
                submenu
                    .window()
                    .set_position(slint::PhysicalPosition::new(x, y));
            }
        }
        DiskMenuView::hide();
        let _ = submenu.show();
    }

    pub fn hide() {
        let submenu = &ui::use_view::<SubmenuView>().ui;
        let _ = submenu.hide();
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
        let app = &ui::use_view::<crate::view::AppView>().ui;
        AppView::apply_store_settings(app, &settings);
        let submenu = &ui::use_view::<SubmenuView>().ui;
        Self::sync_window_from_settings(submenu);
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
        let app = &ui::use_view::<crate::view::AppView>().ui;
        app.set_always_on_top_state(settings.always_on_top);
        if let Ok(info) = shared::win32_info.try_lock() {
            if info.hwnd != 0 {
                tools::set_window_opacity(info.hwnd, settings.opacity);
            }
        }
        let submenu = &ui::use_view::<SubmenuView>().ui;
        Self::sync_window_from_settings(submenu);
    }
}
