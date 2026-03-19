#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use anyhow::Result;
use windows::Win32::{Foundation::{HWND, LPARAM, WPARAM}, UI::WindowsAndMessaging::GetClassNameW};
use winit::platform::windows::WindowAttributesExtWindows;

mod view;
mod base;
mod ui;

pub use base::{hook, task, tools, tray};

pub static MAIN_HWND: AtomicUsize = AtomicUsize::new(0);

fn trim_memory() {
    unsafe {
        use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows::Win32::System::Threading::GetCurrentProcess;

        let handle = GetCurrentProcess();
        let _ = EmptyWorkingSet(handle);
    }
}

fn main() -> Result<()> {
  

    hook::install_win32_hook(|_n_code: i32, w_param: WPARAM, _l_param: LPARAM| {
        let hwnd = HWND(w_param.0 as _);
        let mut class_name = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
        let name = String::from_utf16_lossy(&class_name[..len as usize]);

        if name.contains("Window Class") {
            let hwnd = hwnd.0 as usize;
            hook::uninstall_win32_hook();
            MAIN_HWND.store(hwnd, Ordering::Relaxed);
            tray::setup(hwnd);
        }
    });

    let mut backend = i_slint_backend_winit::Backend::new().unwrap();
    backend.window_attributes_hook = Some(Box::new(|attr| {
        attr.with_skip_taskbar(true)
    }));

    slint::platform::set_platform(Box::new(backend)).expect("Failed to set platform");

    let app = ui::use_view::<view::AppView>();
    task::start_monitor(&app.ui);

    #[cfg(windows)]
    trim_memory();
    app.run()?;
    Ok(())
}
