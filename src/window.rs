//! Window discovery, positioning, and monitor detection.

use crate::config::Region;
use std::mem;
use winapi::shared::minwindef::{BOOL, DWORD, LPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::*;

/// Set per-monitor DPI awareness. This allows accurate window positioning
/// on multi-monitor setups with different DPI scales.
pub fn set_dpi_awareness() {
    unsafe {
        // Try SetProcessDpiAwarenessContext (Windows 10 1607+)
        // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
        let success = SetProcessDpiAwarenessContext(-4isize as _);
        if success == 0 {
            // Fall back to SetProcessDPIAware (Windows Vista+)
            SetProcessDPIAware();
        }
        crate::log::info("DPI awareness set");
    }
}

/// Serializable window information for the web UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfoJson {
    pub hwnd: usize,
    pub title: String,
    pub pid: DWORD,
}

pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub pid: DWORD,
}

impl WindowInfo {
    pub fn to_json(&self) -> WindowInfoJson {
        WindowInfoJson {
            hwnd: self.hwnd as usize,
            title: self.title.clone(),
            pid: self.pid,
        }
    }
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let results = &mut *(lparam as *mut Vec<WindowInfo>);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let title_len = GetWindowTextLengthW(hwnd);
        if title_len == 0 {
            return 1;
        }
        let mut buf = vec![0u16; (title_len + 1) as usize];
        GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let title = String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string();
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        results.push(WindowInfo { hwnd, title, pid });
        1
    }
}

pub fn list_all_visible() -> Vec<WindowInfo> {
    let mut results: Vec<WindowInfo> = Vec::new();
    unsafe {
        EnumWindows(
            Some(enum_windows_callback),
            &mut results as *mut _ as LPARAM,
        );
    }
    results
}

pub fn find_by_pid(target_pid: DWORD) -> Vec<WindowInfo> {
    list_all_visible()
        .into_iter()
        .filter(|w| w.pid == target_pid)
        .collect()
}

pub fn find_primary_by_pid(target_pid: DWORD) -> Option<WindowInfo> {
    find_by_pid(target_pid).into_iter().next()
}

/// Like `find_by_pid` but with no visibility/title filters. Walks ALL
/// top-level windows owned by `target_pid`, including invisible or
/// titleless ones. Required for games like GW2 whose main window is
/// hidden or has an empty title during early startup (Coherent UI
/// init, splash, or the window is owned by a renderer subprocess that
/// `EnumWindows` doesn't surface through the normal visible-only path).
pub fn find_all_by_pid(target_pid: DWORD) -> Vec<HWND> {
    let mut results: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(
            Some(enum_all_windows_for_pid_callback),
            &mut results as *mut _ as LPARAM,
        );
    }
    results
        .into_iter()
        .filter(|hwnd| {
            let mut pid: DWORD = 0;
            unsafe {
                GetWindowThreadProcessId(*hwnd, &mut pid);
            }
            pid == target_pid
        })
        .collect()
}

unsafe extern "system" fn enum_all_windows_for_pid_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let results = &mut *(lparam as *mut Vec<HWND>);
        results.push(hwnd);
        1
    }
}

/// Returns the first window for `target_pid` with a non-empty
/// bounding rectangle, falling back to ANY top-level window owned by
/// that pid. Prefers larger windows (skips tiny utility windows
/// like the Coherent GPU helper).
pub fn find_any_window_by_pid(target_pid: DWORD) -> Option<HWND> {
    let hwnds = find_all_by_pid(target_pid);
    if hwnds.is_empty() {
        return None;
    }

    // Pick the largest window by area — that's almost always the main
    // game window, not a tiny helper/overlay window
    let mut best: Option<(HWND, u64)> = None;
    for hwnd in hwnds {
        unsafe {
            let mut rect = winapi::shared::windef::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) == 0 {
                continue;
            }
            let w = (rect.right - rect.left).max(0) as u64;
            let h = (rect.bottom - rect.top).max(0) as u64;
            let area = w * h;
            // Ignore tiny windows (< 100x100 = 10000 px) — those are
            // always utility/helper windows, not the main game window
            if area < 10_000 {
                continue;
            }
            match best {
                None => best = Some((hwnd, area)),
                Some((_, prev)) if area > prev => best = Some((hwnd, area)),
                _ => {}
            }
        }
    }
    best.map(|(h, _)| h)
}

/// Find visible windows whose process name matches `name` (case-insensitive).
pub fn find_by_process_name(name: &str) -> Vec<WindowInfo> {
    let name_lower = name.to_lowercase();
    list_all_visible()
        .into_iter()
        .filter(|w| {
            let mut exe_buf = [0u16; 260];
            let size = 260u32;
            unsafe {
                let handle = winapi::um::processthreadsapi::OpenProcess(
                    winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    w.pid,
                );
                if handle.is_null() {
                    return false;
                }
                let ok = winapi::um::psapi::GetModuleFileNameExW(
                    handle,
                    std::ptr::null_mut(),
                    exe_buf.as_mut_ptr(),
                    size,
                );
                winapi::um::handleapi::CloseHandle(handle);
                if ok == 0 {
                    return false;
                }
                let path = String::from_utf16_lossy(&exe_buf)
                    .trim_end_matches('\0')
                    .to_string();
                let path_lower = path.to_lowercase();
                path_lower.ends_with(&name_lower)
                    || path_lower.contains(&format!("\\{}", name_lower))
            }
        })
        .collect()
}

/// Collect HWNDs for windows matching `name`, excluding those already in `exclude`.
pub fn collect_new_windows(name: &str, exclude: &[HWND]) -> Vec<HWND> {
    find_by_process_name(name)
        .into_iter()
        .filter(|w| !exclude.contains(&w.hwnd))
        .map(|w| w.hwnd)
        .collect()
}

/// Get the bounding rectangle of a window.
///
/// # Safety
///
/// Caller must ensure `hwnd` is a valid window handle.
pub unsafe fn get_window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let mut rect = winapi::shared::windef::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut rect) != 0 {
            Some((
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
            ))
        } else {
            None
        }
    }
}

pub fn list_all_windows_with_rect() -> Vec<(WindowInfo, (i32, i32, i32, i32))> {
    list_all_visible()
        .into_iter()
        .filter_map(|w| unsafe { get_window_rect(w.hwnd) }.map(|r| (w, r)))
        .collect()
}

pub fn find_windows_by_title_pattern(pattern: &str) -> Vec<(WindowInfo, (i32, i32, i32, i32))> {
    list_all_windows_with_rect()
        .into_iter()
        .filter(|(w, _)| w.title.to_lowercase().contains(&pattern.to_lowercase()))
        .collect()
}

/// Position a window at the given region.
///
/// # Safety
///
/// Caller must ensure `hwnd` is a valid window handle.
pub unsafe fn apply_region(hwnd: HWND, region: &Region) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST as _,
            region.x,
            region.y,
            region.width,
            region.height,
            SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );
    }
}

/// Bring a window to the foreground.
///
/// # Safety
///
/// Caller must ensure `hwnd` is a valid window handle.
pub unsafe fn activate(hwnd: HWND) {
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Monitor {
    pub index: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Enumerate attached monitors. Used by the web UI to show
/// coordinate info and help users position regions correctly.
pub fn list_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(monitor_enum_proc),
            &mut monitors as *mut _ as LPARAM,
        );
    }
    monitors
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: winapi::shared::windef::HMONITOR,
    _hdc: winapi::shared::windef::HDC,
    _rect: *mut winapi::shared::windef::RECT,
    lparam: LPARAM,
) -> BOOL {
    use winapi::um::winuser::{GetMonitorInfoW, MONITORINFOEXW};

    unsafe {
        let monitors = &mut *(lparam as *mut Vec<Monitor>);
        let mut info: MONITORINFOEXW = mem::zeroed();
        info.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _) != 0 {
            let rect = info.szDevice.as_ptr();
            let name_len = (0..).take_while(|&i| *rect.offset(i) != 0).count();
            let name_slice = std::slice::from_raw_parts(rect, name_len);
            let name = String::from_utf16_lossy(name_slice);

            monitors.push(Monitor {
                index: monitors.len() as u32,
                name,
                x: info.rcMonitor.left,
                y: info.rcMonitor.top,
                width: info.rcMonitor.right - info.rcMonitor.left,
                height: info.rcMonitor.bottom - info.rcMonitor.top,
            });
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_any_window_by_pid_zero_returns_none() {
        assert!(find_any_window_by_pid(0).is_none());
    }

    #[test]
    fn find_all_by_pid_zero_returns_empty() {
        // pid 0 is the System Idle Process and has no top-level windows
        assert!(find_all_by_pid(0).is_empty());
    }
}
