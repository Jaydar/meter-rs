#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use std::{process::Command, sync::atomic::{AtomicUsize, Ordering}};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::GetClassNameW,
};


mod base;
mod ui;
mod view;

pub use base::{hook, task, tools, tray};
use crate::view::ViewTrait;

pub static MAIN_HWND: AtomicUsize = AtomicUsize::new(0);
const SOFTWARE_FALLBACK_ENV: &str = "METER_FORCE_SOFTWARE_RENDERER";

pub fn trim_memory() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows::Win32::System::Threading::GetCurrentProcess;

        let handle = GetCurrentProcess();
        let _ = EmptyWorkingSet(handle);
    }
}

fn install_main_window_hook() {
    hook::install_win32_hook(|_n_code: i32, w_param: WPARAM, _l_param: LPARAM| {
        let hwnd = HWND(w_param.0 as _);
        let mut class_name = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
        let name = String::from_utf16_lossy(&class_name[..len as usize]);

        if name.contains("Window Class") {
            let hwnd = hwnd.0 as usize;
            hook::uninstall_win32_hook();
            MAIN_HWND.store(hwnd, Ordering::Release);
            tray::setup(hwnd);
        }
    });
}

fn run_app(renderer_name: &str) -> Result<()> {
    install_main_window_hook();
    view::AppView::init_backend(renderer_name)?;
    let app = ui::use_view::<view::AppView>();
    app.show()?;

    Ok(())
}

fn relaunch_with_software_renderer() -> Result<()> {
    let exe = std::env::current_exe()?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    Command::new(exe)
        .args(args)
        .env(SOFTWARE_FALLBACK_ENV, "1")
        .spawn()?;
    std::process::exit(0);
}

fn main() -> Result<()> {
    if std::env::var_os(SOFTWARE_FALLBACK_ENV).is_some() {
        return run_app("software");
    }

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_app("femtovg"))) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => relaunch_with_software_renderer(),
    }
}
