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
use crate::{base::cli::{self, PageEnum}, view::ViewTrait};

pub static _main_hwnd: AtomicUsize = AtomicUsize::new(0);

fn run_app(args: cli::Args) -> Result<()> {
    let is_app = args.page.is_none();
    if let Some(close_event) = args.close_event {
        tools::close_when_event_set(close_event);
    }
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
            if is_app {
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

                if let Err(err) = SetProcessInformation(
                    handle,
                    ProcessPowerThrottling,
                    &mut throttling as *mut _ as *mut _,
                    std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                ) {
                    error!("Failed to set process power throttling: {}", err);
                }
            }
        }
    });
    view::AppView::init_backend(is_app)?;
    if let Some(page) = args.page {
        let app = ui::use_view::<view::AppView>();
        base::config::load(&app.ui);
        match page {
            PageEnum::Mac => ui::use_view::<view::MacAddressView>().show(None)?,
            PageEnum::Route => ui::use_view::<view::RouteManagerView>().show(None)?,
            PageEnum::PortProxy => ui::use_view::<view::PortProxyView>().show(None)?,
            PageEnum::About => ui::use_view::<view::AboutView>().show(None)?,
        }
        slint::run_event_loop()?;
        return Ok(());
    }
    let app = ui::use_view::<view::AppView>();
    let result = app.show(None);
    tools::close_pages();
    result?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    log::init();
    let args = base::cli::parse(std::env::args().skip(1))?;

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_app(args))) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => {
            error!("application failed: {:#}", err);
            tools::close_pages();
            if std::env::var("SLINT_BACKEND").ok().as_deref() == Some("winit-software") {
                return Ok(());
            }

            let exe = std::env::current_exe()?;
            let args: Vec<String> = std::env::args().skip(1).collect();

            if let Err(err) = Command::new(exe)
                .args(args)
                .env("SLINT_BACKEND", "winit-software")
                .spawn()
            {
                error!("restart application failed: {}", err);
                return Err(err.into());
            }
            std::process::exit(0);
        }
        Err(panic) => {
            error!("application panicked: {:?}", panic);
            tools::close_pages();
            if std::env::var("SLINT_BACKEND").ok().as_deref() == Some("winit-software") {
                return Ok(());
            }

            let exe = std::env::current_exe()?;
            let args: Vec<String> = std::env::args().skip(1).collect();

            if let Err(err) = Command::new(exe)
                .args(args)
                .env("SLINT_BACKEND", "winit-software")
                .spawn()
            {
                error!("restart application after panic failed: {}", err);
                return Err(err.into());
            }
            std::process::exit(0);
        }
    }
}
