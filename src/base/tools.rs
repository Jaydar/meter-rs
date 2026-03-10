use std::os::windows::process::CommandExt;
use std::{env, process::Command};

use i_slint_backend_winit::WinitWindowAccessor;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use tokio::runtime::{Builder, Runtime};
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    System::{
        Power::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState},
        Threading::{GetCurrentThread, SetThreadAffinityMask},
    },
    UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
        HWND_BROADCAST, LWA_ALPHA, SC_MONITORPOWER, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetLayeredWindowAttributes, SetWindowLongPtrW,
        SetWindowPos, WM_SYSCOMMAND, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    },
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn get_tokio_runtime(mask: usize, thread_num: usize) -> Runtime {
    Builder::new_multi_thread()
        .worker_threads(thread_num)
        .on_thread_start(move || unsafe {
            let thread = GetCurrentThread();
            SetThreadAffinityMask(thread, mask);
        })
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime")
}

pub fn get_hwnd_by_window_handle<C: ComponentHandle>(view: &C) -> Option<HWND> {
    let mut hwnd_res = None;
    view.window().with_winit_window(|winit_win| {
        if let Ok(handle) = winit_win.window_handle() {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                hwnd_res = Some(HWND(h.hwnd.get() as _));
            }
        }
    });
    hwnd_res
}

pub fn get_work_area(hwnd: usize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as _);
    let h_monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    get_work_area_from_monitor(h_monitor)
}

pub fn get_size(hwnd: usize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as _);
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect).is_ok() } {
        return Some((rect.left, rect.top, rect.right, rect.bottom));
    }
    None
}

pub fn get_current_mouse_position() -> POINT {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    point
}

pub fn get_menu_position(size: (i32, i32), work_area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (wa_left, wa_top, wa_right, wa_bottom) = work_area;
    let mouse = get_current_mouse_position();
    let center_x = wa_left + (wa_right - wa_left) / 2;
    let center_y = wa_top + (wa_bottom - wa_top) / 2;

    let mut x = mouse.x;
    let mut y = mouse.y;

    if x > center_x {
        x -= size.0;
    }
    if y > center_y {
        y -= size.1;
    }

    x = x.clamp(wa_left, wa_right - size.0);
    y = y.clamp(wa_top, wa_bottom - size.1);

    (x, y)
}

pub fn get_submenu_position(
    main_pos: (i32, i32),
    main_size: (i32, i32),
    submenu_size: (i32, i32),
    item_offset_y: i32,
    work_area: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (wa_left, wa_top, wa_right, wa_bottom) = work_area;
    let gap = 6;
    let open_left = main_pos.0 + main_size.0 + gap + submenu_size.0 > wa_right;
    let x = if open_left {
        (main_pos.0 - gap - submenu_size.0).max(wa_left)
    } else {
        (main_pos.0 + main_size.0 + gap).min(wa_right - submenu_size.0)
    };
    let y = (main_pos.1 + item_offset_y).clamp(wa_top, wa_bottom - submenu_size.1);
    (x, y)
}

pub fn is_window_foreground(hwnd: HWND) -> bool {
    unsafe { GetForegroundWindow() == hwnd }
}

pub fn set_mouse_passthrough(hwnd: usize, enable: bool) {
    let hwnd = HWND(hwnd as _);
    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        style |= WS_EX_LAYERED.0 as isize;
        if enable {
            style |= WS_EX_TRANSPARENT.0 as isize;
        } else {
            style &= !(WS_EX_TRANSPARENT.0 as isize);
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
    }
}

pub fn set_window_opacity(hwnd: usize, opacity: f32) {
    let hwnd = HWND(hwnd as _);
    let alpha = (opacity.clamp(0.2, 1.0) * 255.0).round() as u8;
    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        style |= WS_EX_LAYERED.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

pub fn set_prevent_sleep(enable: bool) {
    unsafe {
        let flags = if enable {
            ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
        } else {
            ES_CONTINUOUS
        };
        let _ = SetThreadExecutionState(flags);
    }
}

pub fn turn_off_display() {
    unsafe {
        let _ = SendMessageW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            Some(WPARAM(SC_MONITORPOWER as usize)),
            Some(LPARAM(2)),
        );
    }
}

pub fn restart_explorer() {
    let _ = Command::new("taskkill")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/F", "/IM", "explorer.exe"])
        .status();
    let _ = Command::new("explorer.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
}

pub fn is_auto_start() -> bool {
    let app_name = "Meter RS";
    let status = reg_command()
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            app_name,
        ])
        .output();
    match status {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

pub fn set_auto_start(enable: bool) {
    let app_name = "Meter RS";
    if enable {
        if let Ok(current_exe) = env::current_exe() {
            let exe_path = current_exe.to_str().unwrap_or("");
            let _ = reg_command()
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    app_name,
                    "/t",
                    "REG_SZ",
                    "/d",
                    exe_path,
                    "/f",
                ])
                .status();
        }
    } else {
        let _ = reg_command()
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                app_name,
                "/f",
            ])
            .status();
    }
}

fn get_work_area_from_monitor(
    h_monitor: windows::Win32::Graphics::Gdi::HMONITOR,
) -> Option<(i32, i32, i32, i32)> {
    let mut mi = MONITORINFO::default();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(h_monitor, &mut mi).as_bool() } {
        return Some((mi.rcWork.left, mi.rcWork.top, mi.rcWork.right, mi.rcWork.bottom));
    }
    None
}

fn reg_command() -> Command {
    let mut command = Command::new("reg");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}
