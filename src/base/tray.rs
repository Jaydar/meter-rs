use std::thread;

use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::{
            Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, LoadIconW,
                PostQuitMessage, RegisterClassW, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
                HMENU, IDI_APPLICATION, MSG, WINDOW_EX_STYLE, WM_CONTEXTMENU, WM_DESTROY,
                WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
};

const TRAY_ICON_ID: u32 = 1001;
const TRAY_MESSAGE: u32 = WM_USER + 1;

pub fn setup(_hwnd: usize) {
    thread::spawn(move || {
        let _ = run_tray_loop();
    });
}

fn run_tray_loop() -> windows::core::Result<()> {
    let class_name = w!("MonRsTrayWindow");
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        lpszClassName: class_name,
        ..Default::default()
    };

    unsafe {
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("MonRsTrayHost"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            Some(HWND::default()),
            Some(HMENU::default()),
            None,
            None,
        )?;

        add_tray_icon(hwnd)?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }

    Ok(())
}

fn add_tray_icon(hwnd: HWND) -> windows::core::Result<()> {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = TRAY_MESSAGE;
    nid.hIcon = unsafe { LoadIconW(None, IDI_APPLICATION)? };

    let tooltip = "Monitor RS".encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    nid.szTip[..tooltip.len()].copy_from_slice(&tooltip);

    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }

    Ok(())
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        TRAY_MESSAGE => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                let _ = slint::invoke_from_event_loop(|| {
                    let menu_view = crate::ui::use_view::<crate::view::MenuView>();
                    menu_view.show();
                });
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                let nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: TRAY_ICON_ID,
                    ..Default::default()
                };
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

