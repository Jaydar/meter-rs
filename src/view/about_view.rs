use std::{os::windows::process::CommandExt, process::Command};

use anyhow::{Context, Result};
use slint::ComponentHandle;
use std::sync::atomic::Ordering;

use crate::{tools, ui, view::ViewTrait, MAIN_HWND};

const GITHUB_URL: &str = "https://github.com/Jaydar/meter-rs";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct AboutView {
    pub ui: ui::AboutWindow,
}

impl ViewTrait for AboutView {
    fn new() -> Self {
        let ui = match ui::AboutWindow::new().context("create AboutWindow failed") {
            Ok(ui) => ui,
            Err(err) => panic!("{}", err),
        };
        ui.set_app_name(env!("CARGO_PKG_NAME").into());
        ui.set_github_url(env!("CARGO_PKG_REPOSITORY").into());
        ui.set_version_text(env!("CARGO_PKG_VERSION").into());
        Self { ui }.bind_event()
    }

    fn show(&self, _extra: Option<&dyn std::any::Any>) -> Result<()> {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::ConfigStore>();
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::ConfigStore>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        let _ = self.ui.show();
        self.set_position();
        Ok(())
    }

    fn hide(&self) {
        let _ = self.ui.hide();
    }

    fn set_position(&self) {
        let hwnd = MAIN_HWND.load(Ordering::Relaxed);
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
        self.ui.set_version_text(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        self.ui.set_github_url(env!("CARGO_PKG_REPOSITORY").into());
        self.ui.on_win_move(|delta_x, delta_y| {
            let about = ui::use_view::<crate::view::AboutView>();
            let window = about.ui.window();
            let scale_factor = window.scale_factor();
            let logical_pos = window.position().to_logical(scale_factor);
            window.set_position(slint::LogicalPosition::new(logical_pos.x + delta_x, logical_pos.y + delta_y));
        });
        self.ui.on_close_about(|| {
            let about = ui::use_view::<crate::view::AboutView>();
            about.hide();
        });
        self.ui.on_open_github(|| {
            let _ = Command::new("cmd")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/C", "start", "", GITHUB_URL])
                .spawn();
        });
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

