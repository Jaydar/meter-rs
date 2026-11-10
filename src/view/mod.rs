use slint::ComponentHandle;
use tracing::error;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{GWLP_HWNDPARENT, GWL_EXSTYLE, GetWindowLongPtrW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, LoadImageW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPos, WM_SETICON, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::core::HSTRING;

use crate::tools;
mod app;
use std::any::Any;

pub use app::AppView;

mod about;
pub use about::AboutView;

mod mac_address;
pub use mac_address::MacAddressView;

mod port_proxy;
pub use port_proxy::PortProxyView;

mod route_manager;
pub use route_manager::RouteManagerView;

pub trait ViewTrait {
    fn new() -> Self where Self: Sized;
    fn show(&self, extra: Option<&dyn Any>) -> anyhow::Result<()>;
    fn hide(&self);
    fn close(&self) {
        self.hide();
    }
    fn set_position(&self);
    fn bind_event(self) -> Self where Self: Sized;
    fn as_any(&self) -> &dyn Any;
}

pub(super) fn setup_page_window<C: ComponentHandle>(view: &C) {
    let Some(hwnd) = tools::get_hwnd_by_window_handle(view) else {
        return;
    };
    if let Some(icon) = page_window_icon() {
        unsafe {
            let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_BIG as usize)), Some(LPARAM(icon)));
            let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_SMALL as usize)), Some(LPARAM(icon)));
        }
    }
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        style &= !(WS_EX_TOOLWINDOW.0 as isize);
        style |= WS_EX_APPWINDOW.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        if let Err(err) = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED) {
            error!("show page window on taskbar failed: {}", err);
        }
    }
}

fn page_window_icon() -> Option<isize> {
    if let Ok(module) = unsafe { GetModuleHandleW(None) } {
        if let Ok(icon) = unsafe { LoadImageW(Some(module.into()), windows::core::PCWSTR(1 as *const u16), IMAGE_ICON, 0, 0, windows::Win32::UI::WindowsAndMessaging::LR_SHARED) } {
            if !icon.is_invalid() {
                return Some(icon.0 as isize);
            }
        }
    }
    let path = HSTRING::from(concat!(env!("CARGO_MANIFEST_DIR"), "\\ui\\assets\\icon.ico"));
    let icon = unsafe { LoadImageW(None, &path, IMAGE_ICON, 0, 0, LR_LOADFROMFILE | LR_DEFAULTSIZE) }.ok()?;
    if icon.is_invalid() { None } else { Some(icon.0 as isize) }
}
