use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);
static ON_EVENT: OnceLock<Box<dyn Fn(i32, WPARAM, LPARAM) + Send + Sync>> = OnceLock::new();


// 核心：回调函数
unsafe extern "system" fn call_back_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    let h_hook_ptr = HOOK_HANDLE.load(Ordering::SeqCst);
    let h_hook = HHOOK(h_hook_ptr as *mut _);

    if n_code == HCBT_CREATEWND as i32 {
        if let Some(callback) = ON_EVENT.get() {
            // 执行外部传入的闭包
            callback(n_code, w_param, l_param);
        }
    }
    
    unsafe { CallNextHookEx(Some(h_hook), n_code, w_param, l_param) }
}

// 安装钩子
pub fn install_win32_hook<F>(callback: F) 
where F: Fn(i32, WPARAM, LPARAM) + Send + Sync + 'static 
{
    
    let _ = ON_EVENT.set(Box::new(callback));
    unsafe {
        let h_hook = SetWindowsHookExW(
            WH_CBT,
            Some(call_back_proc),
            None,
            windows::Win32::System::Threading::GetCurrentThreadId(),
        ).expect("Failed to install hook");
        
        HOOK_HANDLE.store(h_hook.0 as isize, Ordering::SeqCst);
    }
}


// 提供一个显式的卸载函数
pub fn uninstall_win32_hook() {
    let h_hook_ptr = HOOK_HANDLE.swap(0, Ordering::SeqCst);
    if h_hook_ptr != 0 {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(h_hook_ptr as *mut _));
        }
    }
}