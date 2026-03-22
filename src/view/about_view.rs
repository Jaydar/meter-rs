use std::{os::windows::process::CommandExt, process::Command};

use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::{tools, trim_memory, ui, MAIN_HWND};

const GITHUB_URL: &str = "https://github.com/Jaydar/meter-rs";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct AboutView {
    pub ui: ui::AboutWindow,
}

impl Default for AboutView {
    fn default() -> Self {
        AboutView::new()
    }
}

impl AboutView {
    pub fn new() -> Self {
        let ui = ui::AboutWindow::new().unwrap();
        Self { ui }.setup()
    }

    fn setup(self) -> Self {
        self.ui
            .set_version_text(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        self.ui.set_github_url(GITHUB_URL.into());
        self.ui.on_close_about(|| {
            let about = ui::use_view::<AboutView>();
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

    pub fn show(&self) {
        let app_view = ui::use_view::<crate::view::AppView>();
        let app_store = app_view.ui.global::<ui::Store>();
        let theme_mode = app_store.get_theme_mode();
        self.ui.global::<ui::Store>().set_theme_mode(theme_mode);
        self.ui.global::<ui::Theme>().set_mode(theme_mode);
        self.sync_content();
        self.apply_geometry();
        let _ = self.ui.show();
    }

    pub fn hide(&self) {
        let _ = self.ui.hide();
        slint::Timer::single_shot(Duration::from_millis(200), trim_memory);
    }

    fn sync_content(&self) {
        self.ui
            .set_version_text(format!("v{}", env!("CARGO_PKG_VERSION")).into());
        self.ui.set_github_url(GITHUB_URL.into());
    }

    fn apply_geometry(&self) {
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
}
