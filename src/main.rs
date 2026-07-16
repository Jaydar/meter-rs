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

struct AdminPage {
    name: &'static str,
    show: fn() -> Result<()>,
}

const _admin_pages: &[AdminPage] = &[
    AdminPage { name: "mac", show: || ui::use_view::<view::MacAddressView>().show(None) },
    AdminPage { name: "route", show: || ui::use_view::<view::RouteManagerView>().show(None) },
    AdminPage { name: "port-proxy", show: || ui::use_view::<view::PortProxyView>().show(None) },
];

fn run_app() -> Result<()> {
    let admin_page = current_admin_page();
    let parent_pid = parent_pid_arg();
    let close_event = arg_value("--close-event");
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
            if admin_page.is_none() {
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
    if let Some(admin_page) = admin_page {
        if let Some(parent_pid) = parent_pid {
            tools::close_when_parent_exit(parent_pid);
        }
        if let Some(close_event) = close_event {
            tools::close_when_event_set(close_event);
        }
        let app = ui::use_view::<view::AppView>();
        base::config::load(&app.ui);
        (admin_page.show)()?;
        slint::run_event_loop()?;
        return Ok(());
    }
    let app = ui::use_view::<view::AppView>();
    let result = app.show(None);
    tools::close_pages();
    result?;
    Ok(())
}

fn current_admin_page() -> Option<&'static AdminPage> {
    let page_name = arg_value("--page")?;
    _admin_pages.iter().find(|page| page.name == page_name)
}

fn parent_pid_arg() -> Option<u32> {
    arg_value("--parent-pid")?.parse().ok()
}

fn arg_value(name: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    log::init();

    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run_app)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => {
            tools::close_pages();
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
