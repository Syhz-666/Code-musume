use tauri::{AppHandle, Manager, WebviewWindow};

#[cfg(windows)]
pub fn restore_topmost(window: &WebviewWindow) {
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect, IsIconic,
            IsWindowVisible, SetWindowPos, GWL_EXSTYLE, GW_HWNDNEXT, HWND_TOPMOST, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, WS_EX_TOPMOST,
        },
    };
    let pet = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Ok(handle) = pet.hwnd() else { return };
        let hwnd = handle.0;
        // The HWND belongs to this window. Read and update it on its owning thread.
        unsafe {
            if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                return;
            }
            let topmost = |window| GetWindowLongW(window, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
            if topmost(hwnd) {
                let foreground = GetForegroundWindow();
                if foreground.is_null() || foreground == hwnd || topmost(foreground) {
                    return;
                }
                let mut pet_bounds = RECT::default();
                let mut foreground_bounds = RECT::default();
                if GetWindowRect(hwnd, &mut pet_bounds) == 0
                    || GetWindowRect(foreground, &mut foreground_bounds) == 0
                    || pet_bounds.left >= foreground_bounds.right
                    || pet_bounds.right <= foreground_bounds.left
                    || pet_bounds.top >= foreground_bounds.bottom
                    || pet_bounds.bottom <= foreground_bounds.top
                {
                    return;
                }
                // The style bit can remain set while a normal window sits above the pet.
                let mut next = GetWindow(foreground, GW_HWNDNEXT);
                for _ in 0..2048 {
                    if next.is_null() || next == hwnd {
                        break;
                    }
                    next = GetWindow(next, GW_HWNDNEXT);
                }
                if next != hwnd {
                    return;
                }
            }
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER,
            );
        }
    });
}

#[cfg(not(windows))]
pub fn restore_topmost(_: &WebviewWindow) {}
#[cfg(windows)]
pub fn fullscreen(app: &AppHandle) -> bool {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            GetDesktopWindow, GetForegroundWindow, GetShellWindow, GetWindowRect, IsIconic,
        },
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null()
            || hwnd == GetDesktopWindow()
            || hwnd == GetShellWindow()
            || IsIconic(hwnd) != 0
        {
            return false;
        }
        if app
            .get_webview_window("pet")
            .and_then(|w| w.hwnd().ok())
            .is_some_and(|h| h.0 == hwnd)
        {
            return false;
        }
        let mut rect: RECT = std::mem::zeroed();
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetWindowRect(hwnd, &mut rect) == 0
            || GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ) == 0
        {
            return false;
        }
        let m = monitor.rcMonitor;
        rect.left <= m.left + 2
            && rect.top <= m.top + 2
            && rect.right >= m.right - 2
            && rect.bottom >= m.bottom - 2
    }
}
#[cfg(not(windows))]
pub fn fullscreen(_: &AppHandle) -> bool {
    false
}
#[cfg(windows)]
pub fn autostart(enabled: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::*;
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    unsafe {
        let path = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
        let name = wide("CodeMusume");
        let mut key = std::ptr::null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != 0 {
            return Err("无法设置开机启动".into());
        }
        let result = if enabled {
            let value = wide(&format!("\"{}\"", exe.display()));
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        if result == 0 || !enabled && result == 2 {
            Ok(())
        } else {
            Err("无法修改开机启动".into())
        }
    }
}
#[cfg(not(windows))]
pub fn autostart(_: bool) -> Result<(), String> {
    Err("首版只支持 Windows".into())
}
