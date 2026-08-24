use anyhow::{Context, Result};
use slint::{ComponentHandle, ModelRc};
use std::sync::atomic::Ordering;
use tracing::error;

use crate::{_main_hwnd, tools, ui, view::ViewTrait};

pub struct PortProxyView {
    pub ui: ui::PortProxyWindow,
}

impl ViewTrait for PortProxyView {
    fn new() -> Self {
        let ui = match ui::PortProxyWindow::new().context("create PortProxyWindow failed") {
            Ok(ui) => ui,
            Err(err) => panic!("{}", err),
        };
        Self { ui }.bind_event()
    }

    fn show(&self, _extra: Option<&dyn std::any::Any>) -> Result<()> {
        let app_view = ui::use_view::<crate::view::AppView>();
        let theme_mode = app_view.ui.global::<ui::ConfigStore>().get_theme_mode();
        self.ui.global::<ui::ConfigStore>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        self.ui.set_loading_visible(true);
        let _ = self.ui.show();
        self.set_position();
        let weak = self.ui.as_weak();
        let _ = tokio::spawn(async move {
            let port_proxies = tools::port_proxies_async().await;
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                match port_proxies {
                    Ok(port_proxies) => ui.set_port_proxies(ModelRc::from(port_proxies.iter().map(|port_proxy| ui::PortProxyEntry { proxy_type: port_proxy.proxy_type.clone().into(), listen_address: port_proxy.listen_address.clone().into(), listen_port: port_proxy.listen_port.clone().into(), connect_address: port_proxy.connect_address.clone().into(), connect_port: port_proxy.connect_port.clone().into() }).collect::<Vec<_>>().as_slice())),
                    Err(err) => {
                        error!("load port proxies failed: {}", err);
                        ui.set_status_text(format!("读取端口转发失败: {err}").into());
                    }
                }
                ui.set_loading_visible(false);
            });
        });
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {
        let hwnd = _main_hwnd.load(Ordering::Relaxed);
        let hwnd = if hwnd == 0 { tools::get_hwnd_by_window_handle(&self.ui).map(|hwnd| hwnd.0 as usize).unwrap_or(0) } else { hwnd };
        let Some((wa_left, wa_top, wa_right, wa_bottom)) = tools::get_work_area(hwnd) else {
            return;
        };
        let size = self.ui.window().size();
        let width = size.width as i32;
        let height = size.height as i32;
        self.ui.window().set_position(slint::PhysicalPosition::new((wa_left + (wa_right - wa_left - width) / 2).clamp(wa_left, wa_right - width), (wa_top + (wa_bottom - wa_top - height) / 2).clamp(wa_top, wa_bottom - height)));
    }

    fn bind_event(self) -> Self {
        self.ui.on_win_move(|delta_x, delta_y| {
            let port_proxy_view = ui::use_view::<crate::view::PortProxyView>();
            let window = port_proxy_view.ui.window();
            let logical_pos = window.position().to_logical(window.scale_factor());
            window.set_position(slint::LogicalPosition::new(logical_pos.x + delta_x, logical_pos.y + delta_y));
        });
        self.ui.on_close_port_proxy(|| ui::use_view::<crate::view::PortProxyView>().hide());
        self.ui.on_add_port_proxy(|proxy_type, listen_address, listen_port, connect_address, connect_port| {
            let weak = ui::use_view::<crate::view::PortProxyView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::add_port_proxy_async(proxy_type.to_string(), listen_address.to_string(), listen_port.to_string(), connect_address.to_string(), connect_port.to_string()).await;
                let port_proxies = if result.is_ok() { tools::port_proxies_async().await } else { Ok(Vec::new()) };
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return; };
                    match result {
                        Ok(()) => {
                            PortProxyView::update_port_proxies(&ui, port_proxies);
                            ui.set_add_dialog_visible(false);
                            ui.set_add_listen_address("0.0.0.0".into());
                            ui.set_add_listen_port("".into());
                            ui.set_add_connect_address("".into());
                            ui.set_add_connect_port("".into());
                        }
                        Err(err) => {
                            error!("add port proxy failed: {}", err);
                            ui.set_status_text(format!("新增失败: {err}").into());
                        }
                    }
                });
            });
        });
        self.ui.on_edit_port_proxy(|old_proxy_type, old_listen_address, old_listen_port, proxy_type, listen_address, listen_port, connect_address, connect_port| {
            let weak = ui::use_view::<crate::view::PortProxyView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::delete_port_proxy_async(old_proxy_type.to_string(), old_listen_address.to_string(), old_listen_port.to_string()).await;
                let result = match result {
                    Ok(()) => tools::add_port_proxy_async(proxy_type.to_string(), listen_address.to_string(), listen_port.to_string(), connect_address.to_string(), connect_port.to_string()).await,
                    Err(err) => Err(err),
                };
                let port_proxies = if result.is_ok() { tools::port_proxies_async().await } else { Ok(Vec::new()) };
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return; };
                    match result {
                        Ok(()) => {
                            PortProxyView::update_port_proxies(&ui, port_proxies);
                            ui.set_add_dialog_visible(false);
                        }
                        Err(err) => {
                            error!("edit port proxy failed: {}", err);
                            ui.set_status_text(format!("修改失败: {err}").into());
                        }
                    }
                });
            });
        });
        self.ui.on_delete_port_proxy(|proxy_type, listen_address, listen_port| {
            let weak = ui::use_view::<crate::view::PortProxyView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::delete_port_proxy_async(proxy_type.to_string(), listen_address.to_string(), listen_port.to_string()).await;
                let port_proxies = if result.is_ok() { tools::port_proxies_async().await } else { Ok(Vec::new()) };
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return; };
                    match result {
                        Ok(()) => PortProxyView::update_port_proxies(&ui, port_proxies),
                        Err(err) => {
                            error!("delete port proxy failed: {}", err);
                            ui.set_status_text(format!("删除失败: {err}").into());
                        }
                    }
                });
            });
        });
        self.ui.on_reset_port_proxies(|| {
            let weak = ui::use_view::<crate::view::PortProxyView>().ui.as_weak();
            let _ = tokio::spawn(async move {
                let result = tools::reset_port_proxies_async().await;
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return; };
                    match result {
                        Ok(()) => ui.set_port_proxies(ModelRc::from([].as_slice())),
                        Err(err) => {
                            error!("reset port proxies failed: {}", err);
                            ui.set_status_text(format!("清理失败: {err}").into());
                        }
                    }
                });
            });
        });
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PortProxyView {
    fn update_port_proxies(ui: &ui::PortProxyWindow, port_proxies: Result<Vec<tools::PortProxyEntry>>) {
        match port_proxies {
            Ok(port_proxies) => ui.set_port_proxies(ModelRc::from(port_proxies.iter().map(|port_proxy| ui::PortProxyEntry { proxy_type: port_proxy.proxy_type.clone().into(), listen_address: port_proxy.listen_address.clone().into(), listen_port: port_proxy.listen_port.clone().into(), connect_address: port_proxy.connect_address.clone().into(), connect_port: port_proxy.connect_port.clone().into() }).collect::<Vec<_>>().as_slice())),
            Err(err) => {
                error!("load port proxies failed: {}", err);
                ui.set_port_proxies(ModelRc::from([].as_slice()));
                ui.set_status_text(format!("读取端口转发失败: {err}").into());
            }
        }
    }
}
