use slint::{ComponentHandle, Model, ModelRc};

use crate::ui;
use crate::view::AboutView;

use super::{calc_menu_show_position, Menu3View, MenuView};

pub struct Menu2View {
    pub ui: ui::Menu2Window,
}

impl Default for Menu2View {
    fn default() -> Self {
        Menu2View::new()
    }
}

impl Menu2View {
    pub fn new() -> Self {
        let ui = ui::Menu2Window::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        let weak = self.ui.as_weak();
        self.ui.on_close_menu(|| {
            let menu_view = ui::use_view::<MenuView>();
            menu_view.hide_all_menus();
        });
        self.ui.on_set_theme(|theme_mode| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            app_view.ui.global::<ui::Theme>().set_mode(theme_mode);
            let menu_view = ui::use_view::<MenuView>();
            menu_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu_view.ui.global::<ui::Theme>().set_mode(theme_mode);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu2_view.ui.global::<ui::Theme>().set_mode(theme_mode);
            menu2_view.ui.set_theme_state(theme_mode);
            let menu3_view = ui::use_view::<Menu3View>();
            menu3_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu3_view.ui.global::<ui::Theme>().set_mode(theme_mode);
            let about_view = ui::use_view::<AboutView>();
            about_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            about_view.ui.global::<ui::Theme>().set_mode(theme_mode);
        });
        self.ui.on_set_show_hostname(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_hostname(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_hostname_state(value);
        });
        self.ui.on_set_show_cpu(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_cpu(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_cpu_state(value);
        });
        self.ui.on_set_show_memory(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_memory(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_memory_state(value);
        });
        self.ui.on_set_show_disk_total(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_disk_total(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_disk_total_state(value);
        });
        self.ui.on_set_show_disk_io(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_disk_io(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_disk_io_state(value);
        });
        self.ui.on_set_show_network(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_show_network(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_show_network_state(value);
        });
        self.ui.on_show_disk_monitor_submenu(move |_, offset_y| {
            if let Some(weak) = weak.upgrade() {
                let pos = weak.window().position();
                let scaled_y = (offset_y * weak.window().scale_factor()).round() as f32;
                let menu3_view = ui::use_view::<Menu3View>();
                menu3_view.show(pos.x as f32, pos.y as f32, scaled_y);
            }
        });
        self.ui.on_hide_disk_monitor_submenu(|| {
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_disk_submenu_active(false);
            let menu3_view = ui::use_view::<Menu3View>();
            menu3_view.hide();
        });
        self.ui.on_set_always_on_top(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_always_on_top(value);
            app_view.ui.set_always_on_top_state(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_always_on_top_state(value);
        });
        self.ui.on_set_snap_to_edge(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_snap_to_edge(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_snap_to_edge_state(value);
        });
        self.ui.on_set_opacity(|value| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_window_opacity(value);
            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.set_opacity_value(value);
        });
        self
    }

    pub fn show(&self, kind: ui::SubmenuKind, parent_pos_x: f32, parent_pos_y: f32, offset_y: f32) {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::Store>();
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::Store>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        self.ui.set_theme_state(theme_mode);
        if kind == ui::SubmenuKind::Display {
            self.ui.set_show_hostname_state(app_store.get_show_hostname());
            self.ui.set_show_cpu_state(app_store.get_show_cpu());
            self.ui.set_show_memory_state(app_store.get_show_memory());
            self.ui.set_show_disk_total_state(app_store.get_show_disk_total());
            self.ui.set_show_disk_io_state(app_store.get_show_disk_io());
            self.ui.set_show_network_state(app_store.get_show_network());

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
        } else if kind == ui::SubmenuKind::Window {
            self.ui.set_always_on_top_state(app_store.get_always_on_top());
            self.ui.set_snap_to_edge_state(app_store.get_snap_to_edge());
            self.ui.set_opacity_value(app_store.get_window_opacity());
        }
        self.ui.set_kind(kind);
        self.ui.set_disk_submenu_active(false);
        let Some((x, y)) = calc_menu_show_position(parent_pos_x, parent_pos_y, offset_y) else {
            return;
        };

        self.ui.window().set_position(slint::PhysicalPosition::new(x, y));
        let menu3_view = ui::use_view::<Menu3View>();
        menu3_view.hide();
        let _ = self.ui.show();
    }

    pub fn hide(&self) {
        let _ = self.ui.hide();
    }
}
