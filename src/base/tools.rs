use std::os::windows::process::CommandExt;
use std::{env, process::Command, thread};

use anyhow::{Context, Result, bail};
use i_slint_backend_winit::WinitWindowAccessor;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use windows::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
        Power::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState},
        ProcessStatus::EmptyWorkingSet,
        Threading::{GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, SetProcessWorkingSetSize},
    },
    UI::{
        Shell::{IsUserAnAdmin, ShellExecuteW},
        WindowsAndMessaging::{FindWindowW, GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, GetWindowRect, HWND_BROADCAST, SC_MONITORPOWER, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNORMAL, SendMessageW, SetWindowLongPtrW, SetWindowPos, WM_SYSCOMMAND, WS_EX_LAYERED, WS_EX_TRANSPARENT},
    },
};
use windows::core::{HSTRING, w};

const _create_no_window: u32 = 0x0800_0000;

pub struct NetworkAdapter {
    pub id: String,
    pub name: String,
    pub current_mac: String,
    pub mac: String,
}

/// 从 Slint 窗口句柄中提取原生 HWND。
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

/// 获取窗口所在显示器的工作区域。
pub fn get_work_area(hwnd: usize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as _);
    let h_monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    get_work_area_from_monitor(h_monitor)
}

/// 获取窗口所在显示器的完整区域。
pub fn get_monitor_area(hwnd: usize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as _);
    let h_monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO::default();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(h_monitor, &mut mi).as_bool() } {
        return Some((mi.rcMonitor.left, mi.rcMonitor.top, mi.rcMonitor.right, mi.rcMonitor.bottom));
    }
    None
}

/// 获取窗口所在显示器的任务栏高度。
pub fn get_taskbar_height(hwnd: usize) -> Option<i32> {
    let mut tray_rect = RECT::default();
    let tray_hwnd = unsafe { FindWindowW(w!("Shell_TrayWnd"), None).ok()? };
    if unsafe { GetWindowRect(tray_hwnd, &mut tray_rect).is_ok() } {
        let height = tray_rect.bottom - tray_rect.top;
        if height > 0 {
            return Some(height);
        }
    }

    let hwnd = HWND(hwnd as _);
    let h_monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO::default();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(h_monitor, &mut mi).as_bool() } {
        let bottom_height = (mi.rcMonitor.bottom - mi.rcWork.bottom).max(0);
        let top_height = (mi.rcWork.top - mi.rcMonitor.top).max(0);
        return Some(bottom_height.max(top_height));
    }
    None
}

/// 获取窗口矩形位置和尺寸。
pub fn get_size(hwnd: usize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as _);
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect).is_ok() } {
        return Some((rect.left, rect.top, rect.right, rect.bottom));
    }
    None
}

/// 获取当前鼠标坐标。
pub fn get_current_mouse_position() -> POINT {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    point
}

/// 计算主菜单的弹出位置。
pub fn get_menu_position(size: (i32, i32), work_area: (i32, i32, i32, i32), total_width: i32) -> (i32, i32) {
    let (wa_left, wa_top, wa_right, wa_bottom) = work_area;
    let mouse = get_current_mouse_position();
    let center_y = wa_top + (wa_bottom - wa_top) / 2;

    let mut x = if mouse.x + total_width <= wa_right { mouse.x } else { mouse.x - size.0 };
    let mut y = mouse.y;

    if y > center_y {
        y -= size.1;
    }

    x = x.clamp(wa_left, wa_right - size.0);
    y = y.clamp(wa_top, wa_bottom - size.1);

    (x, y)
}

/// 计算子菜单的弹出位置。
pub fn get_submenu_position(main_pos: (i32, i32), main_size: (i32, i32), submenu_size: (i32, i32), item_offset_y: i32, work_area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (wa_left, wa_top, wa_right, wa_bottom) = work_area;
    let gap = 3;
    let open_left = main_pos.0 + main_size.0 + gap + submenu_size.0 > wa_right;
    let x = if open_left { (main_pos.0 - gap - submenu_size.0).max(wa_left) } else { (main_pos.0 + main_size.0 + gap).min(wa_right - submenu_size.0) };
    let y = (main_pos.1 + item_offset_y).clamp(wa_top, wa_bottom - submenu_size.1);
    (x, y)
}

/// 设置窗口是否允许鼠标穿透。
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
        let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE);
    }
}

/// 设置是否阻止系统休眠。
pub fn set_prevent_sleep(enable: bool) {
    unsafe {
        let flags = if enable { ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED } else { ES_CONTINUOUS };
        let _ = SetThreadExecutionState(flags);
    }
}

/// 通过系统命令关闭显示器。
pub fn turn_off_display() {
    unsafe {
        let _ = SendMessageW(HWND_BROADCAST, WM_SYSCOMMAND, Some(WPARAM(SC_MONITORPOWER as usize)), Some(LPARAM(2)));
    }
}

/// 重启 Windows 资源管理器。
pub fn restart_explorer() {
    let _ = Command::new("taskkill").creation_flags(_create_no_window).args(["/F", "/IM", "explorer.exe"]).status();
    let _ = Command::new("explorer.exe").creation_flags(_create_no_window).spawn();
}

/// 在后台线程中尝试清理进程工作集。
pub fn clean_memory() {
    let _ = thread::Builder::new().name("meter-rs-memory-clean".to_string()).spawn(|| unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    if pid != 0 {
                        if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA, false, pid) {
                            let _ = SetProcessWorkingSetSize(process, usize::MAX, usize::MAX);
                            let _ = EmptyWorkingSet(process);
                            let _ = CloseHandle(process);
                        }
                    }

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
        }

        trim_memory();
    });
}

/// 清理当前进程工作集。
pub fn trim_memory() {
    unsafe {
        let handle = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(handle, usize::MAX, usize::MAX);
        let _ = EmptyWorkingSet(handle);
    }
}

/// 检查程序是否已注册开机自启。
pub fn is_auto_start() -> bool {
    let app_name = "Meter RS";
    let status = reg_command().args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", app_name]).output();
    match status {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// 设置程序的开机自启状态。
pub fn set_auto_start(enable: bool) {
    let app_name = "Meter RS";
    if enable {
        if let Ok(current_exe) = env::current_exe() {
            let exe_path = current_exe.to_str().unwrap_or("");
            let _ = reg_command().args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", app_name, "/t", "REG_SZ", "/d", exe_path, "/f"]).status();
        }
    } else {
        let _ = reg_command().args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", app_name, "/f"]).status();
    }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

pub fn run_as_admin_open_mac_address() -> Result<()> {
    let exe = env::current_exe().context("get current exe failed")?;
    let file = HSTRING::from(exe.to_string_lossy().as_ref());
    let operation = HSTRING::from("runas");
    let parameters = HSTRING::from("--open-mac-address");
    let result = unsafe { ShellExecuteW(None, &operation, &file, &parameters, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        bail!("请求管理员权限失败");
    }
    Ok(())
}

pub fn network_adapters() -> Result<Vec<NetworkAdapter>> {
    let script = "Get-NetAdapter | Where-Object { $_.InterfaceDescription -notmatch 'Bluetooth' -and $_.Name -notmatch 'Bluetooth|蓝牙' } | Sort-Object Name | ForEach-Object { $id = \"$($_.InterfaceGuid)\".ToUpper(); $path = Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}' | Where-Object { (Get-ItemProperty $_.PSPath -Name NetCfgInstanceId -ErrorAction SilentlyContinue).NetCfgInstanceId -eq $id } | Select-Object -First 1; $origin = if ($null -ne $path) { (Get-ItemProperty $path.PSPath -Name NetworrkAddressOrigin -ErrorAction SilentlyContinue).NetworrkAddressOrigin } else { $null }; if ([string]::IsNullOrWhiteSpace($origin)) { $origin = $_.MacAddress }; \"$id`t$($_.Name)`t$($_.MacAddress)`t$origin\" }";
    let output = powershell_command(script).output().context("run Get-NetAdapter failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_network_adapter).collect())
}

pub fn set_mac_address(adapter_id: &str, original_mac: &str, new_mac: &str) -> Result<()> {
    let original_mac = normalize_mac_address(original_mac)?;
    let new_mac = normalize_mac_address(new_mac)?;
    let adapter_id = ps_quote(adapter_id);
    let script = format!("$id = '{}'; $origin = '{}'; $mac = '{}'; $path = Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e972-e325-11ce-bfc1-08002be10318}}' | Where-Object {{ (Get-ItemProperty $_.PSPath -Name NetCfgInstanceId -ErrorAction SilentlyContinue).NetCfgInstanceId -eq $id }} | Select-Object -First 1; if ($null -eq $path) {{ throw '未找到网卡注册表项' }}; New-ItemProperty -Path $path.PSPath -Name NetworrkAddressOrigin -Value $origin -PropertyType String -Force | Out-Null; New-ItemProperty -Path $path.PSPath -Name NetworkAddress -Value $mac -PropertyType String -Force | Out-Null; $written_origin = (Get-ItemProperty -Path $path.PSPath -Name NetworrkAddressOrigin -ErrorAction Stop).NetworrkAddressOrigin; $written = (Get-ItemProperty -Path $path.PSPath -Name NetworkAddress -ErrorAction Stop).NetworkAddress; if ($written_origin -ne $origin) {{ throw \"写入失败: $($path.PSChildName) NetworrkAddressOrigin=$written_origin\" }}; if ($written -ne $mac) {{ throw \"写入失败: $($path.PSChildName) NetworkAddress=$written\" }}", adapter_id, original_mac, new_mac);
    let output = powershell_command(&script).output().context("run set mac address failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub fn restart_network_adapter(adapter_id: &str) -> Result<()> {
    let adapter_id = ps_quote(adapter_id);
    let script = format!("$id = '{}'; $adapter = Get-NetAdapter | Where-Object {{ $_.InterfaceGuid -eq $id }} | Select-Object -First 1; if ($null -eq $adapter) {{ throw '未找到网卡' }}; $adapter | Disable-NetAdapter -Confirm:$false; Start-Sleep -Seconds 1; $adapter | Enable-NetAdapter -Confirm:$false", adapter_id);
    let output = powershell_command(&script).output().context("run restart network adapter failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// 从显示器句柄中读取工作区域。
fn get_work_area_from_monitor(h_monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> Option<(i32, i32, i32, i32)> {
    let mut mi = MONITORINFO::default();
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(h_monitor, &mut mi).as_bool() } {
        return Some((mi.rcWork.left, mi.rcWork.top, mi.rcWork.right, mi.rcWork.bottom));
    }
    None
}

/// 创建隐藏窗口执行的 reg 命令。
fn reg_command() -> Command {
    let mut command = Command::new("reg");
    command.creation_flags(_create_no_window);
    command
}

fn powershell_command(script: &str) -> Command {
    let mut command = Command::new("pwsh");
    let script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {script}");
    command.creation_flags(_create_no_window);
    command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    command
}

fn parse_network_adapter(line: &str) -> Option<NetworkAdapter> {
    let parts = line.split('\t').collect::<Vec<_>>();
    if parts.len() < 4 || parts[0].trim().is_empty() || parts[1].trim().is_empty() {
        return None;
    }
    Some(NetworkAdapter { id: parts[0].trim().to_string(), name: parts[1].trim().to_string(), current_mac: format_mac_address(parts[2].trim()).unwrap_or_else(|_| parts[2].trim().to_string()), mac: format_mac_address(parts[3].trim()).unwrap_or_else(|_| parts[3].trim().to_string()) })
}

fn normalize_mac_address(mac: &str) -> Result<String> {
    let value = mac.trim().to_ascii_uppercase();
    let valid_plain = value.len() == 12 && value.chars().all(|c| c.is_ascii_hexdigit());
    let valid_dash = value.len() == 17 && value.split('-').count() == 6 && value.split('-').all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()));
    if !valid_plain && !valid_dash {
        bail!("MAC 地址格式错误");
    }
    let value = value.replace('-', "");
    if value.len() != 12 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("MAC 地址格式错误");
    }
    Ok(value)
}

fn format_mac_address(mac: &str) -> Result<String> {
    let value = normalize_mac_address(mac)?;
    Ok(value.as_bytes().chunks(2).map(|part| std::str::from_utf8(part).unwrap_or("")).collect::<Vec<_>>().join("-"))
}

fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
}
