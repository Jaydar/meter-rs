use anyhow::{Context, Result};
use slint::{ComponentHandle, ModelRc};
use std::sync::atomic::Ordering;
use tracing::error;

use crate::{_main_hwnd, tools, ui, view::ViewTrait};

pub struct MacAddressView {
    pub ui: ui::MacAddressWindow,
}

impl ViewTrait for MacAddressView {
    fn new() -> Self {
        let ui = match ui::MacAddressWindow::new().context("create MacAddressWindow failed") {
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
            let mac_view = ui::use_view::<crate::view::MacAddressView>();
            let window = mac_view.ui.window();
            let scale_factor = window.scale_factor();
            let logical_pos = window.position().to_logical(scale_factor);
            window.set_position(slint::LogicalPosition::new(logical_pos.x + delta_x, logical_pos.y + delta_y));
        });
        self.ui.on_close_mac_address(|| {
            let mac_view = ui::use_view::<crate::view::MacAddressView>();
            mac_view.hide();
        });
        self.ui.on_apply_mac_address(|adapter_id, original_mac, new_mac| {
            let mac_view = ui::use_view::<crate::view::MacAddressView>();
            match tools::set_mac_address(adapter_id.as_str(), original_mac.as_str(), new_mac.as_str()) {
                Ok(()) => {
                    mac_view.ui.set_status_text("".into());
                    mac_view.ui.set_new_mac("".into());
                    mac_view.refresh_adapters();
                    mac_view.ui.set_pending_adapter_id(adapter_id);
                    mac_view.ui.set_success_dialog_visible(true);
                }
                Err(err) => {
                    error!("set mac address failed: {}", err);
                    mac_view.ui.set_status_text(format!("修改失败: {err}").into());
                }
            }
        });
        self.ui.on_restart_adapter(|adapter_id| {
            let mac_view = ui::use_view::<crate::view::MacAddressView>();
            match tools::restart_network_adapter(adapter_id.as_str()) {
                Ok(()) => {
                    mac_view.ui.set_status_text("网卡已重启".into());
                    mac_view.refresh_adapters();
                }
                Err(err) => {
                    error!("restart network adapter failed: {}", err);
                    mac_view.ui.set_status_text(format!("重启失败: {err}").into());
                }
            }
        });
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl MacAddressView {
    fn refresh_adapters(&self) {
        match tools::network_adapters() {
            Ok(adapters) => {
                let entries = adapters
                    .iter()
                    .map(|adapter| ui::NetworkAdapterEntry {
                        id: adapter.id.clone().into(),
                        name: adapter.name.clone().into(),
                        current_mac: adapter.current_mac.clone().into(),
                        mac: adapter.mac.clone().into(),
                    })
                    .collect::<Vec<_>>();
                let names = adapters.iter().map(|adapter| adapter.name.clone().into()).collect::<Vec<slint::SharedString>>();
                self.ui.set_adapters(ModelRc::from(entries.as_slice()));
                self.ui.set_adapter_names(ModelRc::from(names.as_slice()));
                self.ui.set_selected_index(0);
                self.ui.set_original_mac(entries.first().map(|entry| entry.mac.clone()).unwrap_or_default());
                self.ui.set_status_text(if entries.is_empty() { "没有找到可用网卡".into() } else { "".into() });
            }
            Err(err) => {
                error!("load network adapters failed: {}", err);
                self.ui.set_adapters(ModelRc::from([].as_slice()));
                self.ui.set_adapter_names(ModelRc::from([].as_slice()));
                self.ui.set_selected_index(0);
                self.ui.set_status_text(format!("读取网卡失败: {err}").into());
            }
        }
    }
}
