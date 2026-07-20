use std::os::windows::process::CommandExt;
use std::{cell::RefCell, env, process::Command};

use anyhow::{Context, Result, bail};
use i_slint_backend_winit::WinitWindowAccessor;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::ComponentHandle;
use tokio::process::Command as TokioCommand;
use windows::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    System::{
        Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS},
        Power::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState},
        ProcessStatus::EmptyWorkingSet,
        Threading::{CreateEventW, EVENT_MODIFY_STATE, GetCurrentProcess, INFINITE, OpenEventW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, SYNCHRONIZATION_SYNCHRONIZE, SetEvent, SetProcessWorkingSetSize, WaitForSingleObject},
    },
    UI::{
        Shell::{IsUserAnAdmin, ShellExecuteW},
        WindowsAndMessaging::{FindWindowW, GWL_EXSTYLE, GetCursorPos, GetWindowLongPtrW, GetWindowRect, HWND_BROADCAST, SC_MONITORPOWER, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNORMAL, SendMessageW, SetWindowLongPtrW, SetWindowPos, WM_SYSCOMMAND, WS_EX_LAYERED, WS_EX_TRANSPARENT},
    },
};
use windows::core::{HSTRING, w};

const _create_no_window: u32 = 0x0800_0000;
const _synchronize: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS = windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS(0x0010_0000);

thread_local! {
    static _admin_page_close_events: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub struct NetworkAdapter {
    pub id: String,
    pub name: String,
    pub interface_index: String,
    pub current_mac: String,
    pub mac: String,
    pub gateway: String,
}

pub struct RouteEntry {
    pub address_family: String,
    pub destination: String,
    pub adapter_id: String,
    pub interface_index: String,
    pub adapter: String,
    pub gateway: String,
    pub metric: String,
    pub source: String,
}

pub struct PortProxyEntry {
    pub proxy_type: String,
    pub listen_address: String,
    pub listen_port: String,
    pub connect_address: String,
    pub connect_port: String,
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

/// 在 Tokio 阻塞任务中尝试清理进程工作集。
pub fn clean_memory() {
    let _ = tokio::task::spawn_blocking(|| unsafe {
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

pub fn run_as_admin_open_page(page: &str) -> Result<()> {
    let exe = env::current_exe().context("get current exe failed")?;
    let file = HSTRING::from(exe.to_string_lossy().as_ref());
    let operation = HSTRING::from("runas");
    let close_event = format!("Local\\MeterRsAdminPageClose_{}_{}", std::process::id(), uuid_tick());
    let _ = unsafe { CreateEventW(None, true, false, &HSTRING::from(close_event.as_str())) }.context("create close event failed")?;
    let parameters = HSTRING::from(format!("--page {} --close-event {}", quote_arg(page), quote_arg(&close_event)));
    let result = unsafe { ShellExecuteW(None, &operation, &file, &parameters, None, SW_SHOWNORMAL) };
    if result.0 as isize <= 32 {
        bail!("请求管理员权限失败");
    }
    _admin_page_close_events.with(|events| events.borrow_mut().push(close_event));
    Ok(())
}

fn uuid_tick() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|time| time.as_nanos()).unwrap_or_default()
}

fn quote_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

pub fn close_when_parent_exit(parent_pid: u32) {
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(process) = (unsafe { OpenProcess(_synchronize, false, parent_pid) }) else {
            return;
        };
        unsafe {
            let _ = WaitForSingleObject(process, INFINITE);
            let _ = CloseHandle(process);
        }
        let _ = slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        });
    });
}

pub fn close_when_event_set(close_event: String) {
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(event) = (unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, &HSTRING::from(close_event.as_str())) }) else {
            return;
        };
        unsafe {
            let _ = WaitForSingleObject(event, INFINITE);
            let _ = CloseHandle(event);
        }
        let _ = slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        });
    });
}

pub fn close_pages() {
    _admin_page_close_events.with(|events| {
        for close_event in events.borrow().iter() {
            let Ok(event) = (unsafe { OpenEventW(EVENT_MODIFY_STATE, false, &HSTRING::from(close_event.as_str())) }) else {
                continue;
            };
            unsafe {
                let _ = SetEvent(event);
                let _ = CloseHandle(event);
            }
        }
        events.borrow_mut().clear();
    });
}

pub fn network_adapters() -> Result<Vec<NetworkAdapter>> {
    let script = "Get-NetAdapter | Where-Object { $_.InterfaceDescription -notmatch 'Bluetooth' -and $_.Name -notmatch 'Bluetooth|蓝牙' } | Sort-Object Name | ForEach-Object { $id = \"$($_.InterfaceGuid)\".ToUpper(); $path = Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}' | Where-Object { (Get-ItemProperty $_.PSPath -Name NetCfgInstanceId -ErrorAction SilentlyContinue).NetCfgInstanceId -eq $id } | Select-Object -First 1; $origin = if ($null -ne $path) { (Get-ItemProperty $path.PSPath -Name NetworrkAddressOrigin -ErrorAction SilentlyContinue).NetworrkAddressOrigin } else { $null }; $config = Get-NetIPConfiguration -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue; $gateway = if ($null -ne $config -and $null -ne $config.IPv4DefaultGateway) { ($config.IPv4DefaultGateway | Select-Object -First 1).NextHop } else { '' }; if ([string]::IsNullOrWhiteSpace($origin)) { $origin = $_.MacAddress }; \"$id`t$($_.Name)`t$($_.InterfaceIndex)`t$($_.MacAddress)`t$origin`t$gateway\" }";
    let output = powershell_command(script).output().context("run Get-NetAdapter failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_network_adapter).collect())
}

pub async fn network_adapters_async() -> Result<Vec<NetworkAdapter>> {
    let script = "Get-NetAdapter | Where-Object { $_.InterfaceDescription -notmatch 'Bluetooth' -and $_.Name -notmatch 'Bluetooth|蓝牙' } | Sort-Object Name | ForEach-Object { $id = \"$($_.InterfaceGuid)\".ToUpper(); $path = Get-ChildItem 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e972-e325-11ce-bfc1-08002be10318}' | Where-Object { (Get-ItemProperty $_.PSPath -Name NetCfgInstanceId -ErrorAction SilentlyContinue).NetCfgInstanceId -eq $id } | Select-Object -First 1; $origin = if ($null -ne $path) { (Get-ItemProperty $path.PSPath -Name NetworrkAddressOrigin -ErrorAction SilentlyContinue).NetworrkAddressOrigin } else { $null }; $config = Get-NetIPConfiguration -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue; $gateway = if ($null -ne $config -and $null -ne $config.IPv4DefaultGateway) { ($config.IPv4DefaultGateway | Select-Object -First 1).NextHop } else { '' }; if ([string]::IsNullOrWhiteSpace($origin)) { $origin = $_.MacAddress }; \"$id`t$($_.Name)`t$($_.InterfaceIndex)`t$($_.MacAddress)`t$origin`t$gateway\" }";
    let output = powershell_async_command(script).output().await.context("run Get-NetAdapter failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_network_adapter).collect())
}

pub fn routes() -> Result<Vec<RouteEntry>> {
    let script = "$adapters = @{}; Get-NetAdapter -IncludeHidden | ForEach-Object { $adapters[[int]$_.InterfaceIndex] = @{ Id = \"$($_.InterfaceGuid)\".ToUpper(); Name = $_.Name } }; 'IPv4', 'IPv6' | ForEach-Object { $family = $_; 'PersistentStore', 'ActiveStore' | ForEach-Object { $store = $_; Get-NetRoute -AddressFamily $family -PolicyStore $store -ErrorAction SilentlyContinue | Sort-Object DestinationPrefix, InterfaceIndex, RouteMetric | ForEach-Object { $route = $_; $index = [int]$route.InterfaceIndex; $adapter = $adapters[$index]; $id = if ($null -ne $adapter) { $adapter.Id } else { '' }; $name = if ($index -eq 0) { 'None' } elseif ($null -ne $adapter) { \"$($adapter.Name)($index)\" } else { \"接口 $index\" }; $gateway = if ($route.NextHop -eq '0.0.0.0' -or $route.NextHop -eq '::') { '在链路上' } else { $route.NextHop }; \"$family`t$($route.DestinationPrefix)`t$id`t$index`t$name`t$gateway`t$($route.RouteMetric)`t$store\" } } }";
    let output = powershell_command(&script).output().context("run Get-NetRoute failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_route_entry).collect())
}

pub async fn routes_async() -> Result<Vec<RouteEntry>> {
    let script = "$adapters = @{}; Get-NetAdapter -IncludeHidden | ForEach-Object { $adapters[[int]$_.InterfaceIndex] = @{ Id = \"$($_.InterfaceGuid)\".ToUpper(); Name = $_.Name } }; 'IPv4', 'IPv6' | ForEach-Object { $family = $_; 'PersistentStore', 'ActiveStore' | ForEach-Object { $store = $_; Get-NetRoute -AddressFamily $family -PolicyStore $store -ErrorAction SilentlyContinue | Sort-Object DestinationPrefix, InterfaceIndex, RouteMetric | ForEach-Object { $route = $_; $index = [int]$route.InterfaceIndex; $adapter = $adapters[$index]; $id = if ($null -ne $adapter) { $adapter.Id } else { '' }; $name = if ($index -eq 0) { 'None' } elseif ($null -ne $adapter) { \"$($adapter.Name)($index)\" } else { \"接口 $index\" }; $gateway = if ($route.NextHop -eq '0.0.0.0' -or $route.NextHop -eq '::') { '在链路上' } else { $route.NextHop }; \"$family`t$($route.DestinationPrefix)`t$id`t$index`t$name`t$gateway`t$($route.RouteMetric)`t$store\" } } }";
    let output = powershell_async_command(script).output().await.context("run Get-NetRoute failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_route_entry).collect())
}

pub fn add_route(destination: &str, next_hop: &str, address_family: &str, policy_store: &str, interface_index: &str, metric: &str) -> Result<()> {
    let address_family = normalize_address_family(address_family)?;
    let destination = normalize_route_destination(destination, address_family)?;
    let next_hop = normalize_next_hop(next_hop, address_family)?;
    let policy_store = normalize_policy_store(policy_store)?;
    let interface_index = normalize_interface_index(interface_index)?;
    let metric = normalize_route_metric(metric)?;
    let output = if address_family == "IPv4" && policy_store == "PersistentStore" {
        let (address, prefix) = destination.split_once('/').unwrap();
        let mask = std::net::Ipv4Addr::from(u32::MAX.checked_shl(32 - prefix.parse::<u32>().unwrap()).unwrap_or(0)).to_string();
        let metric = metric.unwrap_or(1).to_string();
        let interface_arg = interface_index.map(|interface_index| format!(" if {interface_index}")).unwrap_or_default();
        tracing::info!("add route command: route -p add {address} mask {mask} {next_hop} metric {metric}{interface_arg}");
        let mut command = Command::new("route");
        command.creation_flags(_create_no_window);
        command.args(["-p", "add", address, "mask", mask.as_str(), next_hop.as_str(), "metric", metric.as_str()]);
        if let Some(interface_index) = interface_index {
            command.args(["if", &interface_index.to_string()]);
        }
        command.output().context("run add persistent route failed")?
    } else {
        let interface_arg = interface_index.map(|interface_index| format!(" -InterfaceIndex {interface_index}")).unwrap_or_default();
        let metric_arg = metric.map(|metric| format!(" -RouteMetric {metric}")).unwrap_or_default();
        let script = format!("New-NetRoute -AddressFamily {address_family} -DestinationPrefix '{}' -NextHop '{}' -PolicyStore {}{}{} -ErrorAction Stop | Out-Null", destination, next_hop, policy_store, interface_arg, metric_arg);
        tracing::info!("add route command: {script}");
        powershell_command(&script).output().context("run add route failed")?
    };
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn add_route_async(destination: String, next_hop: String, address_family: String, policy_store: String, interface_index: String, metric: String) -> Result<()> {
    tokio::task::spawn_blocking(move || add_route(&destination, &next_hop, &address_family, &policy_store, &interface_index, &metric)).await.context("wait add route task failed")?
}

pub fn delete_route(destination: &str, interface_index: &str, address_family: &str, source: &str) -> Result<()> {
    let destination = ps_quote(destination);
    let interface_index = ps_quote(interface_index);
    let source = ps_quote(source);
    let address_family = normalize_address_family(address_family)?;
    let script = format!("$destination = '{}'; $index = '{}'; $source = '{}'; if ($index -eq '') {{ throw '未找到接口' }}; Get-NetRoute -DestinationPrefix $destination -InterfaceIndex ([int]$index) -AddressFamily {address_family} -PolicyStore $source -ErrorAction SilentlyContinue | Remove-NetRoute -Confirm:$false", destination, interface_index, source);
    let output = powershell_command(&script).output().context("run delete route failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn delete_route_async(destination: String, interface_index: String, address_family: String, source: String) -> Result<()> {
    tokio::task::spawn_blocking(move || delete_route(&destination, &interface_index, &address_family, &source)).await.context("wait delete route task failed")?
}

pub fn port_proxies() -> Result<Vec<PortProxyEntry>> {
    let script = "'v4tov4', 'v4tov6', 'v6tov4', 'v6tov6' | ForEach-Object { $type = $_; netsh interface portproxy show $type | Select-Object -Skip 5 | ForEach-Object { $parts = $_.Trim() -split '\\s+'; if ($parts.Count -eq 4) { \"$type`t$($parts[0])`t$($parts[1])`t$($parts[2])`t$($parts[3])\" } } }";
    let output = powershell_command(script).output().context("run netsh portproxy show failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_port_proxy_entry).collect())
}

pub async fn port_proxies_async() -> Result<Vec<PortProxyEntry>> {
    let script = "'v4tov4', 'v4tov6', 'v6tov4', 'v6tov6' | ForEach-Object { $type = $_; netsh interface portproxy show $type | Select-Object -Skip 5 | ForEach-Object { $parts = $_.Trim() -split '\\s+'; if ($parts.Count -eq 4) { \"$type`t$($parts[0])`t$($parts[1])`t$($parts[2])`t$($parts[3])\" } } }";
    let output = powershell_async_command(script).output().await.context("run netsh portproxy show failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).lines().filter_map(parse_port_proxy_entry).collect())
}

pub fn add_port_proxy(proxy_type: &str, listen_address: &str, listen_port: &str, connect_address: &str, connect_port: &str) -> Result<()> {
    let proxy_type = normalize_port_proxy_type(proxy_type)?;
    let listen_address = normalize_ip_address(listen_address, "监听地址", proxy_type.starts_with("v4"))?;
    let listen_port = normalize_port(listen_port, "监听端口")?;
    let connect_address = normalize_ip_address(connect_address, "目标地址", proxy_type.ends_with("v4"))?;
    let connect_port = normalize_port(connect_port, "目标端口")?;
    let output = Command::new("netsh").creation_flags(_create_no_window).args(["interface", "portproxy", "add", proxy_type, &format!("listenaddress={listen_address}"), &format!("listenport={listen_port}"), &format!("connectaddress={connect_address}"), &format!("connectport={connect_port}")]).output().context("run netsh portproxy add failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn add_port_proxy_async(proxy_type: String, listen_address: String, listen_port: String, connect_address: String, connect_port: String) -> Result<()> {
    tokio::task::spawn_blocking(move || add_port_proxy(&proxy_type, &listen_address, &listen_port, &connect_address, &connect_port)).await.context("wait add port proxy task failed")?
}

pub fn delete_port_proxy(proxy_type: &str, listen_address: &str, listen_port: &str) -> Result<()> {
    let proxy_type = normalize_port_proxy_type(proxy_type)?;
    let listen_address = normalize_ip_address(listen_address, "监听地址", proxy_type.starts_with("v4"))?;
    let listen_port = normalize_port(listen_port, "监听端口")?;
    let output = Command::new("netsh").creation_flags(_create_no_window).args(["interface", "portproxy", "delete", proxy_type, &format!("listenport={listen_port}"), &format!("listenaddress={listen_address}")]).output().context("run netsh portproxy delete failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn delete_port_proxy_async(proxy_type: String, listen_address: String, listen_port: String) -> Result<()> {
    tokio::task::spawn_blocking(move || delete_port_proxy(&proxy_type, &listen_address, &listen_port)).await.context("wait delete port proxy task failed")?
}

pub fn reset_port_proxies() -> Result<()> {
    let output = Command::new("netsh").creation_flags(_create_no_window).args(["interface", "portproxy", "reset"]).output().context("run netsh portproxy reset failed")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

pub async fn reset_port_proxies_async() -> Result<()> {
    tokio::task::spawn_blocking(reset_port_proxies).await.context("wait reset port proxies task failed")?
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

pub async fn set_mac_address_async(adapter_id: String, original_mac: String, new_mac: String) -> Result<()> {
    tokio::task::spawn_blocking(move || set_mac_address(&adapter_id, &original_mac, &new_mac)).await.context("wait set mac address task failed")?
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
pub async fn restart_network_adapter_async(adapter_id: String) -> Result<()> {
    tokio::task::spawn_blocking(move || restart_network_adapter(&adapter_id)).await.context("wait restart adapter task failed")?
}

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
    let mut command = Command::new("powershell.exe");
    let script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {script}");
    command.creation_flags(_create_no_window);
    command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    command
}

fn powershell_async_command(script: &str) -> TokioCommand {
    let mut command = TokioCommand::new("powershell.exe");
    let script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; {script}");
    command.creation_flags(_create_no_window);
    command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    command
}

fn parse_network_adapter(line: &str) -> Option<NetworkAdapter> {
    let parts = line.split('\t').collect::<Vec<_>>();
    if parts.len() < 5 || parts[0].trim().is_empty() || parts[1].trim().is_empty() {
        return None;
    }
    Some(NetworkAdapter { id: parts[0].trim().to_string(), name: parts[1].trim().to_string(), interface_index: parts[2].trim().to_string(), current_mac: format_mac_address(parts[3].trim()).unwrap_or_else(|_| parts[3].trim().to_string()), mac: format_mac_address(parts[4].trim()).unwrap_or_else(|_| parts[4].trim().to_string()), gateway: parts.get(5).map(|part| part.trim().to_string()).unwrap_or_default() })
}

fn parse_route_entry(line: &str) -> Option<RouteEntry> {
    let parts = line.split('\t').collect::<Vec<_>>();
    if parts.len() < 8 || parts[1].trim().is_empty() || parts[3].trim().is_empty() {
        return None;
    }
    Some(RouteEntry { address_family: parts[0].trim().to_string(), destination: parts[1].trim().to_string(), adapter_id: parts[2].trim().to_string(), interface_index: parts[3].trim().to_string(), adapter: parts[4].trim().to_string(), gateway: parts[5].trim().to_string(), metric: parts[6].trim().to_string(), source: parts[7].trim().to_string() })
}

fn parse_port_proxy_entry(line: &str) -> Option<PortProxyEntry> {
    let parts = line.split('\t').collect::<Vec<_>>();
    if parts.len() != 5 {
        return None;
    }
    Some(PortProxyEntry { proxy_type: format_port_proxy_type(parts[0]), listen_address: parts[1].to_string(), listen_port: parts[2].to_string(), connect_address: parts[3].to_string(), connect_port: parts[4].to_string() })
}

fn normalize_route_destination(destination: &str, address_family: &str) -> Result<String> {
    let value = destination.trim();
    let parts = value.split('/').collect::<Vec<_>>();
    if parts.len() != 2 || (address_family == "IPv4" && parts[0].parse::<std::net::Ipv4Addr>().is_err()) || (address_family == "IPv6" && parts[0].parse::<std::net::Ipv6Addr>().is_err()) {
        bail!("路由格式错误");
    }
    let prefix = parts[1].parse::<u8>().context("路由掩码错误")?;
    if prefix > if address_family == "IPv4" { 32 } else { 128 } {
        bail!("路由掩码错误");
    }
    Ok(format!("{}/{}", parts[0], prefix))
}

fn normalize_route_metric(metric: &str) -> Result<Option<u32>> {
    let value = metric.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let metric = value.parse::<u32>().context("跃点格式错误")?;
    Ok(Some(metric))
}

fn normalize_next_hop(next_hop: &str, address_family: &str) -> Result<String> {
    let value = next_hop.trim();
    if (address_family == "IPv4" && value.parse::<std::net::Ipv4Addr>().is_err()) || (address_family == "IPv6" && value.parse::<std::net::Ipv6Addr>().is_err()) {
        bail!("网关格式错误");
    }
    Ok(value.to_string())
}

fn normalize_address_family(address_family: &str) -> Result<&'static str> {
    match address_family {
        "IPv4" => Ok("IPv4"),
        "IPv6" => Ok("IPv6"),
        _ => bail!("地址类型错误"),
    }
}

fn normalize_ip_address(address: &str, name: &str, ipv4: bool) -> Result<String> {
    let address = address.trim();
    if (ipv4 && address.parse::<std::net::Ipv4Addr>().is_err()) || (!ipv4 && address.parse::<std::net::Ipv6Addr>().is_err()) {
        bail!("{name}格式错误");
    }
    Ok(address.to_string())
}

fn normalize_port_proxy_type(proxy_type: &str) -> Result<&'static str> {
    match proxy_type {
        "v4tov4" | "V4 to V4" => Ok("v4tov4"),
        "v4tov6" | "V4 to V6" => Ok("v4tov6"),
        "v6tov4" | "V6 to V4" => Ok("v6tov4"),
        "v6tov6" | "V6 to V6" => Ok("v6tov6"),
        _ => bail!("转发类型错误"),
    }
}

fn format_port_proxy_type(proxy_type: &str) -> String {
    match proxy_type {
        "v4tov4" => "V4 to V4",
        "v4tov6" => "V4 to V6",
        "v6tov4" => "V6 to V4",
        "v6tov6" => "V6 to V6",
        _ => proxy_type,
    }
    .to_string()
}

fn normalize_port(port: &str, name: &str) -> Result<u16> {
    let port = port.trim().parse::<u16>().with_context(|| format!("{name}格式错误"))?;
    if port == 0 {
        bail!("{name}格式错误");
    }
    Ok(port)
}

fn normalize_policy_store(policy_store: &str) -> Result<&'static str> {
    match policy_store.trim() {
        "ActiveStore" => Ok("ActiveStore"),
        "PersistentStore" => Ok("PersistentStore"),
        _ => bail!("路由类型错误"),
    }
}

fn normalize_interface_index(interface_index: &str) -> Result<Option<u32>> {
    let value = interface_index.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let index = value.parse::<u32>().context("接口编号格式错误")?;
    Ok(Some(index))
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


pub fn svg_to_ico(svg_path: &str, ico_path: &str) {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&std::fs::read(svg_path).unwrap(), &options).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    let image = pixmap.encode_png().unwrap();
    let mut icon = vec![0, 0, 1, 0, 1, 0, 64, 64, 0, 0, 1, 0, 32, 0];
    icon.extend_from_slice(&(image.len() as u32).to_le_bytes());
    icon.extend_from_slice(&22u32.to_le_bytes());
    icon.extend_from_slice(&image);
    std::fs::write(ico_path, icon).unwrap();
}
