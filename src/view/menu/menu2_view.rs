use anyhow::{Context, Result};
use slint::{ComponentHandle, Model, ModelRc};
use tracing::{error, info};

use crate::{
    ui,
    view::{ViewTrait, app_view},
};

use super::{Menu3View, calc_menu_show_position, close_menus};

pub struct Menu2View {
    pub ui: ui::Menu2Window,
}

impl Menu2View {
    pub fn sync_store(&self) {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::Store>();

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

        self.ui.set_always_on_top_state(app_store.get_always_on_top());
        self.ui.set_snap_to_edge_state(app_store.get_snap_to_edge());
        self.ui.set_opacity_value(app_store.get_window_opacity());
    }
}

impl ViewTrait for Menu2View {
    fn new() -> Self {
        let ui = match ui::Menu2Window::new().context("create Menu2Window failed") {
            Ok(ui) => ui,
            Err(err) => panic!("{}", err),
        };
        Self { ui }.bind_event()
    }

    fn show(&self, extra: Option<&dyn std::any::Any>) -> Result<()> {
        info!("show menu2");
        let (kind, _parent_pos_x, parent_pos_y, offset_y, root_pos_x) =
            extra
                .and_then(|e| e.downcast_ref::<(ui::SubmenuKind, f32, f32, f32, f32)>())
                .context("menu2 show extra error")?;

        self.sync_store();
        self.ui.set_kind(*kind);
        self.ui.set_disk_submenu_active(false);
        let Some((x, y)) = calc_menu_show_position(2, *root_pos_x, *parent_pos_y, *offset_y) else {
            return Ok(());
        };
        self.ui.window().set_position(slint::PhysicalPosition::new(x, y));
        let menu3_view = ui::use_view::<Menu3View>();
        menu3_view.hide();
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
        self.ui.on_set_theme(|theme_mode| {
            let app_view = ui::use_view::<crate::view::AppView>();
            app_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            app_view.ui.global::<ui::Theme>().set_mode(theme_mode);

            let menu1_view = ui::use_view::<crate::view::Menu1View>();
            menu1_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu1_view.ui.global::<ui::Theme>().set_mode(theme_mode);

            let menu2_view = ui::use_view::<Menu2View>();
            menu2_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu2_view.ui.global::<ui::Theme>().set_mode(theme_mode);

            let menu3_view = ui::use_view::<Menu3View>();
            menu3_view.ui.global::<ui::Store>().set_theme_mode(theme_mode);
            menu3_view.ui.global::<ui::Theme>().set_mode(theme_mode);

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
        self.ui.on_show_disk_monitor_submenu({
            let weak = weak.clone();
            move |_, offset_y| {
                if let Some(menu) = weak.upgrade() {
                    let pos = menu.window().position();
                    let scaled_y = (offset_y * menu.window().scale_factor()).round() as f32;
                    let menu1 = ui::use_view::<crate::view::Menu1View>();
                    let root_x = menu1.ui.window().position().x as f32;
                    let menu3_view = ui::use_view::<Menu3View>();
                    let extra = (pos.x as f32, pos.y as f32, scaled_y, root_x);
                    if let Err(err) = menu3_view.show(Some(&extra)) {
                        error!("{}", err);
                    }
                }
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
