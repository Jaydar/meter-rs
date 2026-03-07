#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use windows::Win32::{Foundation::{HWND, LPARAM, WPARAM}, UI::WindowsAndMessaging::GetClassNameW};
use winit::platform::windows::WindowAttributesExtWindows;
use slint::ComponentHandle;

mod view;
mod base;
mod ui;

pub use base::{hook, shared, task, tools, tray};

fn trim_memory() {
    unsafe {
        use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows::Win32::System::Threading::GetCurrentProcess;
        
        let handle = GetCurrentProcess();
        let _ = EmptyWorkingSet(handle);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(windows)] trim_memory();

    if let Ok(mut settings) = shared::app_settings.lock() {
        settings.auto_start = tools::is_auto_start();
    }
    
    let mut backend = i_slint_backend_winit::Backend::new().unwrap();
    backend.window_attributes_hook = Some(Box::new(|attr| {
        attr.with_skip_taskbar(true)
    }));

    slint::platform::set_platform(Box::new(backend)).expect("Failed to set platform");

    let _ = ui::use_view::<view::MenuView>();
    let _ = ui::use_view::<view::SubmenuView>();

    hook::install_win32_hook(|_n_code: i32, w_param: WPARAM, _l_param: LPARAM|{
        let hwnd = HWND(w_param.0 as _);
        let mut class_name = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
        let name = String::from_utf16_lossy(&class_name[..len as usize]);

        if name.contains("Window Class") {
            let hwnd = hwnd.0 as usize;
            hook::uninstall_win32_hook();
            tray::setup(hwnd); 

            if let Ok(mut info) = shared::win32_info.try_lock() {
                info.hwnd = hwnd;
            }
        }
    });

    let app = ui::use_view::<view::AppView>();
    task::start_monitor(&app.ui).await;
    app.ui.run().unwrap();

    Ok(())
}
