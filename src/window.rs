//! Window discovery, positioning, and monitor detection.

use crate::config::Region;
use std::mem;
use winapi::shared::minwindef::{BOOL, DWORD, LPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::*;

pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub pid: DWORD,
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

pub fn get_window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let mut rect = winapi::shared::windef::RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut rect) != 0 {
            Some((rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top))
        } else {
            None
        }
    }
}

pub fn list_all_windows_with_rect() -> Vec<(WindowInfo, (i32, i32, i32, i32))> {
    list_all_visible()
        .into_iter()
        .filter_map(|w| {
            get_window_rect(w.hwnd).map(|r| (w, r))
        })
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

#[derive(Debug, Clone)]
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
