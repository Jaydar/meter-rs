use anyhow::Context;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use tracing::error;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static _hook_handle: AtomicIsize = AtomicIsize::new(0);
static _on_event: OnceLock<Box<dyn Fn(i32, WPARAM, LPARAM) + Send + Sync>> = OnceLock::new();

// WH_CBT 的回调函数。这里用它监听当前线程里的窗口创建事件。
unsafe extern "system" fn call_back_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    let h_hook_ptr = _hook_handle.load(Ordering::SeqCst);
    let h_hook = HHOOK(h_hook_ptr as *mut _);

    if n_code == HCBT_CREATEWND as i32 {
        if let Some(callback) = _on_event.get() {
            // HCBT_CREATEWND 时 w_param 是正在创建的 HWND。
            callback(n_code, w_param, l_param);
        }
    }
    // 不拦截系统事件，只观察后继续交给后续 hook 和系统默认处理。
    unsafe { CallNextHookEx(Some(h_hook), n_code, w_param, l_param) }
}

// 给当前线程安装 CBT hook，用来在 Slint/Winit 创建窗口时拿到原生 HWND。
pub fn install_win32_hook<F>(callback: F)
where
    F: Fn(i32, WPARAM, LPARAM) + Send + Sync + 'static,
{
    if _on_event.set(Box::new(callback)).is_err() {
        error!("Win32 hook callback is already installed");
    }
    unsafe {
        // thread_id 指定为当前线程，所以这个 hook 只监听本 UI 线程，不注入其它进程。
        match SetWindowsHookExW(
            WH_CBT,
            Some(call_back_proc),
            None,
            windows::Win32::System::Threading::GetCurrentThreadId(),
        ).context("failed to install hook") {
            Ok(h_hook) => _hook_handle.store(h_hook.0 as isize, Ordering::SeqCst),
            Err(err) => error!("{}", err),
        }
    }
}

// HWND 捕获完成后及时卸载 hook，避免后续窗口创建继续触发回调。
pub fn uninstall_win32_hook() {
    let h_hook_ptr = _hook_handle.swap(0, Ordering::SeqCst);
    if h_hook_ptr != 0 {
        unsafe {
            if let Err(err) = UnhookWindowsHookEx(HHOOK(h_hook_ptr as *mut _)) {
                error!("uninstall Win32 hook failed: {}", err);
            }
        }
    }
}
