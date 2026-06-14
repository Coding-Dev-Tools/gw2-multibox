//! Input broadcasting via low-level keyboard hook.
//!
//! Uses `SetWindowsHookEx` with `WH_KEYBOARD_LL` to intercept keyboard input
//! and forward it to the active game window. Keys are forwarded only when
//! broadcasting is enabled (toggled via hotkey, default F9).
//!
//! Constraints:
//! - No game memory modification
//! - No network interception
//! - No combat automation
//! - Only forwards to active slot window

use anyhow::Result;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::{HHOOK, HWND};
use winapi::um::winuser::*;

use crate::config::BroadcastConfig;

/// Modifier VK codes that should not be forwarded.
const MODIFIER_VKS: &[u32] = &[
    VK_LWIN as u32,
    VK_RWIN as u32,
    VK_LSHIFT as u32,
    VK_RSHIFT as u32,
    VK_LCONTROL as u32,
    VK_RCONTROL as u32,
    VK_LMENU as u32,
    VK_RMENU as u32,
];

/// Thread-safe wrapper for mutable state accessed by the hook callback.
struct HookState {
    enabled: AtomicBool,
    active_slot: AtomicUsize,
    windows: UnsafeCell<Vec<HWND>>,
    hook: UnsafeCell<HHOOK>,
}

// Safety: The hook callback runs on a single thread and we ensure
// proper synchronization through atomic operations.
unsafe impl Sync for HookState {}

static STATE: HookState = HookState {
    enabled: AtomicBool::new(false),
    active_slot: AtomicUsize::new(0),
    windows: UnsafeCell::new(Vec::new()),
    hook: UnsafeCell::new(std::ptr::null_mut()),
};

/// State for broadcast management.
pub struct BroadcastManager {
    enabled: bool,
    hook: Option<HHOOK>,
    windows: Vec<HWND>,
    slot_count: usize,
    #[allow(dead_code)]
    config: BroadcastConfig,
}

impl BroadcastManager {
    /// Create a new broadcast manager.
    pub fn new(config: BroadcastConfig, windows: Vec<HWND>) -> Self {
        let slot_count = windows.len();
        Self {
            enabled: false,
            hook: None,
            windows,
            slot_count,
            config,
        }
    }

    /// Enable broadcasting. Installs the keyboard hook.
    pub fn enable(&mut self) -> Result<()> {
        if self.enabled {
            return Ok(());
        }

        // Store windows in static for the hook callback
        unsafe {
            *STATE.windows.get() = self.windows.clone();
        }

        // Install the keyboard hook
        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                std::ptr::null_mut(),
                0,
            );

            if hook.is_null() {
                return Err(anyhow::anyhow!(
                    "Failed to install keyboard hook (error {})",
                    std::io::Error::last_os_error()
                ));
            }

            *STATE.hook.get() = hook;
            self.hook = Some(hook);
        }

        STATE.enabled.store(true, Ordering::SeqCst);
        self.enabled = true;
        crate::log::info("Input broadcasting enabled");
        Ok(())
    }

    /// Disable broadcasting. Removes the keyboard hook.
    pub fn disable(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        unsafe {
            if let Some(hook) = self.hook.take() {
                UnhookWindowsHookEx(hook);
                *STATE.hook.get() = std::ptr::null_mut();
            }
        }

        STATE.enabled.store(false, Ordering::SeqCst);
        self.enabled = false;
        crate::log::info("Input broadcasting disabled");
        Ok(())
    }

    /// Toggle broadcasting on/off.
    pub fn toggle(&mut self) -> Result<bool> {
        if self.enabled {
            self.disable()?;
            Ok(false)
        } else {
            self.enable()?;
            Ok(true)
        }
    }

    /// Check if broadcasting is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the active slot index (0-based).
    pub fn set_active_slot(&mut self, index: usize) {
        if index < self.slot_count {
            STATE.active_slot.store(index, Ordering::SeqCst);
            crate::log::debug(&format!("Active slot set to {}", index));
        }
    }

    /// Get the active slot index.
    pub fn active_slot(&self) -> usize {
        STATE.active_slot.load(Ordering::SeqCst)
    }

    /// Check if a key should be forwarded based on config.
    #[allow(dead_code)]
    fn should_forward(&self, vk_code: u32) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Don't forward modifier keys
        if MODIFIER_VKS.contains(&vk_code) {
            return false;
        }

        // If no specific keys configured, forward all except modifiers
        if self.config.keys.is_empty() {
            true
        } else {
            // Forward only configured keys
            self.config.keys.contains(&vk_code)
        }
    }

    /// Update the window list (e.g., after window resize/reposition).
    pub fn update_windows(&mut self, windows: Vec<HWND>) {
        self.slot_count = windows.len();
        self.windows = windows;
        unsafe {
            *STATE.windows.get() = self.windows.clone();
        }
    }

    /// Get the target window for the active slot.
    #[allow(dead_code)]
    fn target_window(&self) -> Option<HWND> {
        let idx = STATE.active_slot.load(Ordering::SeqCst);
        let windows = unsafe { &*STATE.windows.get() };
        if idx < windows.len() {
            let hwnd = windows[idx];
            if !hwnd.is_null() {
                return Some(hwnd);
            }
        }
        None
    }
}

impl Drop for BroadcastManager {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

/// Low-level keyboard hook callback.
///
/// # Safety
///
/// This is a Windows callback. Must not panic or allocate.
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    unsafe {
        if n_code >= 0 && STATE.enabled.load(Ordering::SeqCst) {
            let kbd_struct = *(l_param as *const KBDLLHOOKSTRUCT);
            let vk_code = kbd_struct.vkCode;
            let msg = w_param as u32;

            // Don't forward modifier keys
            if !MODIFIER_VKS.contains(&vk_code)
                && let Some(hwnd) = get_active_window()
            {
                let (key_down, key_up) = match msg {
                    WM_KEYDOWN => (true, false),
                    WM_KEYUP => (false, true),
                    WM_SYSKEYDOWN => (true, false),
                    WM_SYSKEYUP => (false, true),
                    _ => (false, false),
                };

                if key_down {
                    let scan_code = kbd_struct.scanCode;
                    let lparam = make_lparam(1, scan_code as u16, 0, 0, 0, 0);
                    PostMessageW(hwnd, WM_KEYDOWN, vk_code as WPARAM, lparam as LPARAM);
                }

                if key_up {
                    let scan_code = kbd_struct.scanCode;
                    let lparam = make_lparam(1, scan_code as u16, 0, 0, 1, 1);
                    PostMessageW(hwnd, WM_KEYUP, vk_code as WPARAM, lparam as LPARAM);
                }
            }
        }

        let hook = *STATE.hook.get();
        CallNextHookEx(hook, n_code, w_param, l_param)
    }
}

/// Get the active window handle from the static windows array.
unsafe fn get_active_window() -> Option<HWND> {
    unsafe {
        let idx = STATE.active_slot.load(Ordering::SeqCst);
        let windows = &*STATE.windows.get();
        if idx < windows.len() {
            let hwnd = windows[idx];
            if !hwnd.is_null() {
                return Some(hwnd);
            }
        }
        None
    }
}

/// Create an lParam value for keyboard messages.
fn make_lparam(
    repeat_count: u16,
    scan_code: u16,
    extended: u8,
    _context_code: u8,
    previous_state: u8,
    transition_state: u8,
) -> u32 {
    (repeat_count as u32)
        | ((scan_code as u32) << 16)
        | ((extended as u32) << 24)
        | ((previous_state as u32) << 30)
        | ((transition_state as u32) << 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_lparam_repeat_and_scan() {
        let lp = make_lparam(1, 0x1E, 0, 0, 0, 0);
        assert_eq!(lp & 0xFFFF, 1); // repeat count
        assert_eq!((lp >> 16) & 0xFF, 0x1E); // scan code
    }

    #[test]
    fn make_lparam_extended_flag() {
        let lp = make_lparam(1, 0x1E, 1, 0, 0, 0);
        assert_eq!((lp >> 24) & 1, 1); // extended key flag
    }

    #[test]
    fn make_lparam_previous_and_transition() {
        let lp = make_lparam(1, 0x1E, 0, 0, 1, 1);
        assert_eq!((lp >> 30) & 1, 1); // previous state
        assert_eq!((lp >> 31) & 1, 1); // transition state
    }

    #[test]
    fn broadcast_manager_initial_state() {
        let config = BroadcastConfig {
            enabled: false,
            keys: vec![],
            toggle_key: 0x78, // F9
        };
        let mgr = BroadcastManager::new(config, vec![]);
        assert!(!mgr.is_enabled());
        assert_eq!(mgr.active_slot(), 0);
    }
}
