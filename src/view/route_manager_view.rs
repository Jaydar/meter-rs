use anyhow::{Context, Result};
use slint::{ComponentHandle, ModelRc};
use std::sync::atomic::Ordering;
use tracing::error;

use crate::{_main_hwnd, tools, ui, view::ViewTrait};

pub struct RouteManagerView {
    pub ui: ui::RouteManagerWindow,
}

impl ViewTrait for RouteManagerView {
    fn new() -> Self {
        let ui = match ui::RouteManagerWindow::new().context("create RouteManagerWindow failed") {
            Ok(ui) => ui,
            Err(err) => panic!("{}", err),
        };
        Self { ui }.bind_event()
    }

    fn show(&self, _extra: Option<&dyn std::any::Any>) -> Result<()> {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::ConfigStore>();
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::ConfigStore>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        self.refresh_adapters();
        self.refresh_routes();
        let _ = self.ui.show();
        self.set_position();
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {
        let hwnd = _main_hwnd.load(Ordering::Relaxed);
        let hwnd = if hwnd == 0 { tools::get_hwnd_by_window_handle(&self.ui).map(|hwnd| hwnd.0 as usize).unwrap_or(0) } else { hwnd };
        if hwnd == 0 {
            return;
        }
        let Some((wa_left, wa_top, wa_right, wa_bottom)) = tools::get_work_area(hwnd) else {
            return;
        };

        let size = self.ui.window().size();
        let width = size.width as i32;
        let height = size.height as i32;
        let x = (wa_left + (wa_right - wa_left - width) / 2).clamp(wa_left, wa_right - width);
        let y = (wa_top + (wa_bottom - wa_top - height) / 2).clamp(wa_top, wa_bottom - height);
        self.ui.window().set_position(slint::PhysicalPosition::new(x, y));
    }

    fn bind_event(self) -> Self {
        self.ui.on_win_move(|delta_x, delta_y| {
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            let window = route_view.ui.window();
            let scale_factor = window.scale_factor();
            let logical_pos = window.position().to_logical(scale_factor);
            window.set_position(slint::LogicalPosition::new(logical_pos.x + delta_x, logical_pos.y + delta_y));
        });
        self.ui.on_close_route_manager(|| {
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            route_view.hide();
        });
        self.ui.on_add_route(|destination, next_hop, policy_store, interface_index, metric| {
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            match tools::add_route(destination.as_str(), next_hop.as_str(), policy_store.as_str(), interface_index.as_str(), metric.as_str()) {
                Ok(()) => {
                    route_view.ui.set_add_dialog_visible(false);
                    route_view.ui.set_status_text("新增成功".into());
                    route_view.ui.set_add_destination("".into());
                    route_view.ui.set_add_metric("".into());
                }
                Err(err) => {
                    error!("add route failed: {}", err);
                    route_view.ui.set_status_text(format!("新增失败: {err}").into());
                }
            }
            route_view.refresh_routes();
        });
        self.ui.on_delete_route(|destination, interface_index, source| {
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            match tools::delete_route(destination.as_str(), interface_index.as_str(), source.as_str()) {
                Ok(()) => {
                    route_view.ui.set_status_text("".into());
                    route_view.refresh_routes();
                }
                Err(err) => {
                    error!("delete route failed: {}", err);
                    route_view.ui.set_status_text(format!("删除失败: {err}").into());
                }
            }
        });
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl RouteManagerView {
    fn refresh_adapters(&self) {
        match tools::network_adapters() {
            Ok(adapters) => {
                let entries = adapters
                    .iter()
                    .map(|adapter| ui::NetworkAdapterEntry {
                        id: adapter.id.clone().into(),
                        name: adapter.name.clone().into(),
                        interface_index: adapter.interface_index.clone().into(),
                        gateway: adapter.gateway.clone().into(),
                        current_mac: adapter.current_mac.clone().into(),
                        mac: adapter.mac.clone().into(),
                    })
                    .collect::<Vec<_>>();
                let names = adapters.iter().map(|adapter| format!("{}({})", adapter.name, adapter.gateway).into()).collect::<Vec<slint::SharedString>>();
                self.ui.set_adapters(ModelRc::from(entries.as_slice()));
                self.ui.set_adapter_names(ModelRc::from(names.as_slice()));
                self.ui.set_add_adapter_index(0);
                self.ui.set_add_next_hop(entries.first().map(|entry| entry.gateway.clone()).unwrap_or_default());
                self.ui.set_status_text(if entries.is_empty() { "没有找到可用网卡".into() } else { "".into() });
            }
            Err(err) => {
                error!("load network adapters failed: {}", err);
                self.ui.set_adapters(ModelRc::from([].as_slice()));
                self.ui.set_adapter_names(ModelRc::from([].as_slice()));
                self.ui.set_add_adapter_index(0);
                self.ui.set_status_text(format!("读取网卡失败: {err}").into());
            }
        }
    }

    fn refresh_routes(&self) {
        match tools::routes() {
            Ok(routes) => {
                let entries = routes
                    .iter()
                    .map(|route| ui::RouteEntry {
                        destination: route.destination.clone().into(),
                        adapter_id: route.adapter_id.clone().into(),
                        interface_index: route.interface_index.clone().into(),
                        adapter: format!("{}({})", route.adapter, route.gateway).into(),
                        gateway: route.gateway.clone().into(),
                        metric: route.metric.clone().into(),
                        source: route.source.clone().into(),
                    })
                    .collect::<Vec<_>>();
                self.ui.set_routes(ModelRc::from(entries.as_slice()));
            }
            Err(err) => {
                error!("load routes failed: {}", err);
                self.ui.set_routes(ModelRc::from([].as_slice()));
                self.ui.set_status_text(format!("读取路由失败: {err}").into());
            }
        }
    }
}
