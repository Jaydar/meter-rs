#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(non_upper_case_globals)]

use anyhow::Result;
use std::{
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
use tracing::error;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::Threading::{GetCurrentProcess, PROCESS_CREATION_FLAGS, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass, SetProcessInformation},
    UI::WindowsAndMessaging::GetClassNameW,
};

mod base;
mod ui;
mod view;

pub use base::{hook, log, task, tools, tray};
use crate::view::ViewTrait;

pub static _main_hwnd: AtomicUsize = AtomicUsize::new(0);

fn run_app() -> Result<()> {
    let open_mac_address = std::env::args().any(|arg| arg == "--open-mac-address");
    let open_route_manager = std::env::args().any(|arg| arg == "--open-route-manager");
    hook::install_win32_hook(move |_n_code: i32, w_param: WPARAM, _l_param: LPARAM| {
        // 获取窗口句柄
        let hwnd = HWND(w_param.0 as _);
        let mut class_name = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, &mut class_name) };
        let name = String::from_utf16_lossy(&class_name[..len as usize]);

        // 判断主窗口
        if name.contains("Window Class") {
            let hwnd = hwnd.0 as usize;
            hook::uninstall_win32_hook();
            _main_hwnd.store(hwnd, Ordering::Release);
            if !open_mac_address && !open_route_manager {
                tray::setup(hwnd);
            }

            unsafe {
                // 获取进程
                let handle = GetCurrentProcess();


                // 降低进程优先级，避免监控刷新影响前台程序。
                if let Err(err) = SetPriorityClass(handle, PROCESS_CREATION_FLAGS(0x40)) {
                    error!("Failed to set process priority: {}", err);
                }

                // 开启 Windows 进程电源节流，后台运行时减少 CPU 调度压力。
                let mut throttling = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                    StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                };

                let _ = SetProcessInformation(
                    handle,
                    ProcessPowerThrottling,
                    &mut throttling as *mut _ as *mut _,
                    std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                );
            }
        }
    });
    view::AppView::init_backend()?;
    if open_mac_address || open_route_manager {
        let app = ui::use_view::<view::AppView>();
        base::config::load(&app.ui);
        if open_mac_address {
            ui::use_view::<view::MacAddressView>().show(None)?;
        } else {
            ui::use_view::<view::RouteManagerView>().show(None)?;
        }
        slint::run_event_loop()?;
        return Ok(());
    }
    let app = ui::use_view::<view::AppView>();
    app.show(None)?;
    Ok(())
}

fn main() -> Result<()> {
    log::init();

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_app)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => {
            if std::env::var("SLINT_BACKEND").ok().as_deref() == Some("winit-software") {
                return Ok(());
            }

            let exe = std::env::current_exe()?;
            let args: Vec<String> = std::env::args().skip(1).collect();

            Command::new(exe)
                .args(args)
                .env("SLINT_BACKEND", "winit-software")
                .spawn()?;
            std::process::exit(0);
        }
    }
}
