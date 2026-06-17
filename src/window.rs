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

/// Find all main (top-level, visible, with a title) windows owned by
/// processes whose image name matches `process_name` (case-insensitive,
/// without the .exe suffix). Returns one HWND per matching process,
/// preferring the largest window for each PID (the main game window
/// rather than splash or helper windows).
///
/// Uses the Toolhelp32 snapshot API to enumerate processes — works
/// without needing PROCESS_QUERY_INFORMATION on every process.
pub fn find_windows_by_process_name(process_name: &str) -> Vec<HWND> {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::tlhelp32::{PROCESSENTRY32W, Process32FirstW, Process32NextW};
    use winapi::um::winnt::HANDLE;

    let needle = process_name.to_ascii_lowercase();
    let needle = needle.strip_suffix(".exe").unwrap_or(&needle).to_string();

    unsafe {
        let snap: HANDLE = winapi::um::tlhelp32::CreateToolhelp32Snapshot(
            winapi::um::tlhelp32::TH32CS_SNAPPROCESS,
            0,
        );
        if snap.is_null() {
            return Vec::new();
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;

        let mut pids: Vec<DWORD> = Vec::new();
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                // szExeFile is a wide null-terminated string
                let name_w = &entry.szExeFile;
                let name_len = name_w.iter().position(|&c| c == 0).unwrap_or(name_w.len());
                let name = String::from_utf16_lossy(&name_w[..name_len]).to_ascii_lowercase();
                let name_no_exe = name.strip_suffix(".exe").unwrap_or(&name).to_string();
                if name_no_exe == needle {
                    pids.push(entry.th32ProcessID);
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);

        // For each PID, find the main window (largest visible with title)
        let mut results: Vec<HWND> = Vec::new();
        for pid in pids {
            if let Some(info) = find_primary_by_pid(pid) {
                results.push(info.hwnd);
            }
        }
        results
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

/// Position a window at the given region, making it topmost.
///
/// # Safety
///
/// Caller must ensure `hwnd` is a valid window handle.
pub unsafe fn apply_region(hwnd: HWND, region: &Region) {
    unsafe {
        apply_region_zorder(hwnd, region, true);
    }
}

/// Position a window at the given region with explicit z-order control.
///
/// When `topmost` is true, the window is placed above all non-topmost windows.
/// When false, it's placed at the top of the non-topmost z-order. This is
/// important for swap layouts where only the active window should be topmost —
/// using HWND_TOPMOST for all windows can cause them to not maintain proper
/// z-order between each other.
///
/// Uses SWP_NOACTIVATE to prevent stealing focus, and omits SWP_FRAMECHANGED
/// to prevent the window from recalculating its frame (which can cause games
/// like GW2 to reposition themselves to their saved position).
///
/// # Safety
///
/// Caller must ensure `hwnd` is a valid window handle.
pub unsafe fn apply_region_zorder(hwnd: HWND, region: &Region, topmost: bool) {
    unsafe {
        let zorder = if topmost {
            HWND_TOPMOST as _
        } else {
            HWND_TOP as _
        };
        SetWindowPos(
            hwnd,
            zorder,
            region.x,
            region.y,
            region.width,
            region.height,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
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
/// Compute ISBoxer-style swap-layout positions.
///
/// The focused slot gets the full screen. The other slots are
/// positioned as small thumbnails along the bottom edge.
///
/// # Arguments
/// * `monitor` — the monitor rectangle (x, y, w, h)
/// * `active_slot` — 0-based index of the focused slot
/// * `total_slots` — total number of windows (including active)
pub fn swap_layout_positions(
    monitor: (i32, i32, i32, i32),
    active_slot: usize,
    total_slots: usize,
) -> Vec<Region> {
    let (mx, my, mw, mh) = monitor;
    let thumb_h = (mh as f64 * 0.20) as i32; // 20% for thumbnails
    let main_h = mh - thumb_h;
    let thumb_count = total_slots.saturating_sub(1);

    let mut regions = Vec::with_capacity(total_slots);
    for i in 0..total_slots {
        if i == active_slot {
            // Active slot: full screen
            regions.push(Region {
                name: format!("slot-{}", i + 1),
                x: mx,
                y: my,
                width: mw,
                height: main_h,
            });
        } else {
            // Small thumbnails: evenly spaced along the bottom
            let mut thumb_idx = 0;
            for j in 0..total_slots {
                if j == active_slot {
                    continue;
                }
                if j == i {
                    break;
                }
                thumb_idx += 1;
            }
            let thumb_w = mw / thumb_count as i32;
            regions.push(Region {
                name: format!("slot-{}", i + 1),
                x: mx + thumb_idx * thumb_w,
                y: my + main_h,
                width: thumb_w,
                height: thumb_h,
            });
        }
    }
    regions
}

/// Get the active slot index by checking which window is in the
/// foreground. Returns None if the foreground window isn't one of ours.
pub fn get_foreground_slot(windows: &[HWND]) -> Option<usize> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return None;
        }
        windows.iter().position(|&h| h == fg)
    }
}

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

    // ── swap layout tests ─────────────────────────────────────────

    #[test]
    fn swap_layout_4_slots_active_0() {
        // 4 slots, active=0 (first), monitor 1920x1080
        let regions = swap_layout_positions((0, 0, 1920, 1080), 0, 4);
        assert_eq!(regions.len(), 4);

        // Active slot: full screen
        assert_eq!(regions[0].x, 0);
        assert_eq!(regions[0].y, 0);
        assert_eq!(regions[0].width, 1920);
        assert_eq!(regions[0].height, 864); // 1080 * 0.8

        // Thumbnails: bottom row, 20% height
        for (i, region) in regions[1..4].iter().enumerate() {
            let slot = i + 1;
            assert_eq!(region.y, 864, "slot {} y", slot);
            assert_eq!(region.height, 216, "slot {} height", slot);
            assert_eq!(region.width, 640, "slot {} width", slot); // 1920/3
        }
        assert_eq!(regions[1].x, 0);
        assert_eq!(regions[2].x, 640);
        assert_eq!(regions[3].x, 1280);
    }

    #[test]
    fn swap_layout_4_slots_active_2() {
        // Active slot 2 (0-indexed) should be the full-screen one
        let regions = swap_layout_positions((0, 0, 1920, 1080), 2, 4);
        assert_eq!(regions.len(), 4);

        // Slot 2 is active: full screen
        assert_eq!(regions[2].height, 864);

        // Slots 0,1,3 are thumbnails
        for i in [0, 1, 3] {
            assert_eq!(regions[i].y, 864, "slot {} y", i);
            assert_eq!(regions[i].height, 216, "slot {} height", i);
        }
    }

    #[test]
    fn swap_layout_3_slots() {
        let regions = swap_layout_positions((0, 0, 1920, 1080), 0, 3);
        assert_eq!(regions.len(), 3);

        // Active: full screen
        assert_eq!(regions[0].height, 864);

        // 2 thumbnails, each 960 wide
        assert_eq!(regions[1].width, 960);
        assert_eq!(regions[2].width, 960);
        assert_eq!(regions[1].x, 0);
        assert_eq!(regions[2].x, 960);
    }

    #[test]
    fn swap_layout_single_slot() {
        let regions = swap_layout_positions((0, 0, 1920, 1080), 0, 1);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].width, 1920);
        assert_eq!(regions[0].height, 864);
    }

    #[test]
    fn swap_layout_2_slots() {
        let regions = swap_layout_positions((0, 0, 1920, 1080), 0, 2);
        assert_eq!(regions.len(), 2);

        // Active: full screen top
        assert_eq!(regions[0].height, 864);

        // 1 thumbnail: full width bottom
        assert_eq!(regions[1].width, 1920);
        assert_eq!(regions[1].height, 216);
        assert_eq!(regions[1].y, 864);
    }

    #[test]
    fn swap_layout_multi_monitor_offset() {
        // Monitor at (1920,0) — second monitor
        let regions = swap_layout_positions((1920, 0, 1920, 1080), 0, 2);
        assert_eq!(regions[0].x, 1920);
        assert_eq!(regions[1].x, 1920);
    }

    #[test]
    fn swap_layout_all_thumbnails_same_y() {
        // All non-active slots share the same y position
        let regions = swap_layout_positions((0, 0, 1920, 1080), 0, 4);
        let thumb_y = regions[1].y;
        for region in &regions[2..4] {
            assert_eq!(region.y, thumb_y);
        }
    }

    #[test]
    fn swap_layout_active_slot_full_screen() {
        // The active slot always occupies the full width
        for active in 0..4 {
            let regions = swap_layout_positions((0, 0, 1920, 1080), active, 4);
            assert_eq!(
                regions[active].width, 1920,
                "active slot {} full width",
                active
            );
            assert_eq!(regions[active].x, 0, "active slot {} at x=0", active);
        }
    }

    #[test]
    fn swap_layout_inactive_below_active() {
        // Inactive slots are always positioned below the active slot's bottom edge
        for active in 0..4 {
            let regions = swap_layout_positions((0, 0, 1920, 1080), active, 4);
            let active_bottom = regions[active].y + regions[active].height;
            for (i, region) in regions.iter().enumerate() {
                if i != active {
                    assert_eq!(
                        region.y, active_bottom,
                        "inactive slot {} y should equal active bottom",
                        i
                    );
                }
            }
        }
    }

    #[test]
    fn swap_layout_thumbnails_have_aspect_ratio_for_clicking() {
        // Sanity check: in the canonical 1920x1080 monitor case, each
        // thumbnail must be wide enough to be clickable as a target
        // (the user clicks on the bottom strip to swap). At least
        // 200px wide and 100px tall is the ISBoxer convention.
        for active in 0..4 {
            let regions = swap_layout_positions((0, 0, 1920, 1080), active, 4);
            for (i, r) in regions.iter().enumerate() {
                if i != active {
                    assert!(r.width >= 200, "slot {} thumb width {} < 200", i, r.width);
                    assert!(
                        r.height >= 100,
                        "slot {} thumb height {} < 100",
                        i,
                        r.height
                    );
                }
            }
        }
    }

    #[test]
    fn swap_layout_region_count_matches_slot_count() {
        // The runtime calls swap_layout_positions(monitor, active, n)
        // and then iterates windows.len() times. If n == 0 the function
        // must return an empty Vec, not panic. This is the contract
        // that fixes the post-discovery hotkey indexing path.
        assert_eq!(swap_layout_positions((0, 0, 1920, 1080), 0, 0).len(), 0);
        for n in 1..=8 {
            let r = swap_layout_positions((0, 0, 1920, 1080), 0, n);
            assert_eq!(r.len(), n, "for n={}", n);
        }
    }
}
