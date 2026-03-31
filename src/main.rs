#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use tracing::{error,info,warn,debug,trace};
use std::{process::Command, sync::atomic::{AtomicUsize, Ordering}};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM}, System::Threading::{GetCurrentProcess, PROCESS_CREATION_FLAGS, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation}, UI::WindowsAndMessaging::GetClassNameW
};


mod base;
mod ui;
mod view;

pub use base::{hook, log, task, tools, tray};
use crate::view::ViewTrait;

pub static MAIN_HWND: AtomicUsize = AtomicUsize::new(0);


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


            unsafe {
                
                let handle = GetCurrentProcess();
                // 小绿叶
                SetPriorityClass(handle, PROCESS_CREATION_FLAGS(0x40)).unwrap();
                // 这一步才是“按出”绿叶的开关
                let mut throttling = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED, // 0x1
                    StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,   // 0x1
                };

                // 效能模式
                let _ = SetProcessInformation(
                    handle,
                    ProcessPowerThrottling, // 枚举值 4
                    &mut throttling as *mut _ as *mut _,
                    std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                );
            }

        }
    });
}

fn run_app() -> Result<()> {
    install_main_window_hook();
    view::AppView::init_backend()?;
    let app = ui::use_view::<view::AppView>();
    app.show(None)?;
    Ok(())
}


fn main() -> Result<()> {
    log::init();

    error!("Application started");
    warn!("This is a warning message");
    info!("This is an info message");
    debug!("This is a debug message");
    trace!("This is a trace message");

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_app())) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => {
            
            if std::env::var("SLINT_BACKEND").ok().as_deref() == Some("winit-software") {
                return  Ok(());
            }

            let exe = std::env::current_exe()?;
            let args: Vec<String> = std::env::args().skip(1).collect();

            Command::new(exe)
                .args(args)
                .env("SLINT_BACKEND", "winit-software")
                .spawn()?;
            std::process::exit(0);
        },
    }
}
