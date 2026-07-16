use anyhow::{Context, Result};
use slint::{ComponentHandle, Model, ModelRc};
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
        self.ui.set_loading_visible(true);
        let _ = self.ui.show();
        self.set_position();
        self.reload_data();
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
        self.ui.on_add_route(|destination, next_hop, address_family, policy_store, interface_index, metric| {
            let weak = ui::use_view::<crate::view::RouteManagerView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::add_route_async(destination.to_string(), next_hop.to_string(), address_family.to_string(), policy_store.to_string(), interface_index.to_string(), metric.to_string()).await;
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(_ui) = weak.upgrade() else {
                        return;
                    };
                    let route_view = ui::use_view::<crate::view::RouteManagerView>();
                    match result {
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
            route_view.reload_data();
                });
            });
        });
        self.ui.on_delete_route(|destination, interface_index, address_family, source| {
            let weak = ui::use_view::<crate::view::RouteManagerView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::delete_route_async(destination.to_string(), interface_index.to_string(), address_family.to_string(), source.to_string()).await;
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(_ui) = weak.upgrade() else {
                        return;
                    };
                    let route_view = ui::use_view::<crate::view::RouteManagerView>();
                    match result {
                Ok(()) => {
                    route_view.ui.set_status_text("".into());
                    route_view.reload_data();
                }
                Err(err) => {
                    error!("delete route failed: {}", err);
                    route_view.ui.set_status_text(format!("删除失败: {err}").into());
                }
            }
                });
            });
        });
        self.ui.on_select_route_tab(|index| {
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            route_view.ui.set_route_tab_index(index);
            route_view.show_routes();
        });
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl RouteManagerView {
    fn reload_data(&self) {
        let weak = self.ui.as_weak();
        let _ = tokio::spawn(async move {
            let (adapters, routes) = tokio::join!(tools::network_adapters_async(), tools::routes_async());
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                RouteManagerView::update_adapters(&ui, adapters);
                RouteManagerView::update_routes(&ui, routes);
                ui.set_loading_visible(false);
            });
        });
    }

    fn show_routes(&self) {
        let address_family = self.ui.get_route_address_family();
        let policy_store = self.ui.get_route_policy_store();
        let routes = self.ui.get_all_routes();
        let entries = (0..routes.row_count()).filter_map(|index| routes.row_data(index)).filter(|route| route.address_family == address_family && route.source == policy_store).collect::<Vec<_>>();
        self.ui.set_routes(ModelRc::from(entries.as_slice()));
    }

    fn update_adapters(ui: &ui::RouteManagerWindow, adapters: Result<Vec<tools::NetworkAdapter>>) {
    match adapters {
        Ok(adapters) => {
            let entries = adapters.iter().map(|adapter| ui::NetworkAdapterEntry { id: adapter.id.clone().into(), name: adapter.name.clone().into(), interface_index: adapter.interface_index.clone().into(), gateway: adapter.gateway.clone().into(), current_mac: adapter.current_mac.clone().into(), mac: adapter.mac.clone().into() }).collect::<Vec<_>>();
            let names = adapters.iter().map(|adapter| format!("{}({})", adapter.name, adapter.gateway).into()).collect::<Vec<slint::SharedString>>();
            ui.set_adapters(ModelRc::from(entries.as_slice()));
            ui.set_adapter_names(ModelRc::from(names.as_slice()));
            ui.set_add_adapter_index(0);
            ui.set_add_next_hop(entries.first().map(|entry| entry.gateway.clone()).unwrap_or_default());
        }
        Err(err) => {
            error!("load network adapters failed: {}", err);
            ui.set_adapters(ModelRc::from([].as_slice()));
            ui.set_adapter_names(ModelRc::from([].as_slice()));
            ui.set_status_text(format!("读取网卡失败: {err}").into());
        }
    }
}

    fn update_routes(ui: &ui::RouteManagerWindow, routes: Result<Vec<tools::RouteEntry>>) {
    match routes {
        Ok(routes) => {
            let entries = routes.iter().map(|route| ui::RouteEntry { address_family: route.address_family.clone().into(), destination: route.destination.clone().into(), adapter_id: route.adapter_id.clone().into(), interface_index: route.interface_index.clone().into(), adapter: format!("{}({})", route.adapter, route.gateway).into(), gateway: route.gateway.clone().into(), metric: route.metric.clone().into(), source: route.source.clone().into() }).collect::<Vec<_>>();
            ui.set_all_routes(ModelRc::from(entries.as_slice()));
            let route_view = ui::use_view::<crate::view::RouteManagerView>();
            route_view.show_routes();
        }
        Err(err) => {
            error!("load routes failed: {}", err);
            ui.set_all_routes(ModelRc::from([].as_slice()));
            ui.set_routes(ModelRc::from([].as_slice()));
            ui.set_status_text(format!("读取路由失败: {err}").into());
        }
    }
    }
}
