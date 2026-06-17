//! Global hotkey registration and the Win32 message loop.

use anyhow::Result;
use std::mem;
use winapi::shared::minwindef::UINT;
use winapi::shared::windef::HWND;
use winapi::um::winuser::*;

pub const HOTKEY_BASE_ID: i32 = 100;
pub const BROADCAST_TOGGLE_ID: i32 = 200;

/// Hotkey manager that registers window-switching hotkeys for slots
/// plus an optional broadcast toggle hotkey.
pub struct HotkeyManager {
    slot_count: usize,
    base_vk: u32,
    broadcast_toggle_vk: Option<u32>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            slot_count: 0,
            base_vk: 0x75, // F6 default
            broadcast_toggle_vk: None,
        }
    }

    /// `base_vk` is the VK code for slot 1 (e.g. 0x75 = F6). Subsequent
    /// slots use base_vk+1, base_vk+2, etc. The previous default of F1
    /// (0x70) collided with GW2's own in-game hotkeys on F1-F5, so
    /// callers should pass a base of F6 or higher unless the user has
    /// explicitly opted in to lower keys.
    pub fn register(&mut self, slot_count: usize, base_vk: u32) -> Result<()> {
        for i in 0..slot_count {
            unsafe {
                let result =
                    RegisterHotKey(0 as HWND, HOTKEY_BASE_ID + i as i32, 0, base_vk + i as UINT);
                if result == 0 {
                    eprintln!(
                        "Warning: Failed to register slot {} hotkey (VK 0x{:X}, error {})",
                        i + 1,
                        base_vk + i as u32,
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
        self.slot_count = slot_count;
        self.base_vk = base_vk;
        Ok(())
    }

    /// Register the broadcast toggle hotkey.
    pub fn register_broadcast_toggle(&mut self, vk: u32) -> Result<()> {
        unsafe {
            let result = RegisterHotKey(0 as HWND, BROADCAST_TOGGLE_ID, 0, vk as UINT);
            if result == 0 {
                return Err(anyhow::anyhow!(
                    "Failed to register broadcast toggle hotkey (VK 0x{:X}, error {})",
                    vk,
                    std::io::Error::last_os_error()
                ));
            }
        }
        self.broadcast_toggle_vk = Some(vk);
        Ok(())
    }

    pub fn unregister_all(&self) {
        unsafe {
            for i in 0..self.slot_count {
                UnregisterHotKey(0 as HWND, HOTKEY_BASE_ID + i as i32);
            }
            if self.broadcast_toggle_vk.is_some() {
                UnregisterHotKey(0 as HWND, BROADCAST_TOGGLE_ID);
            }
        }
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

/// Hotkey event kinds passed to the single run_loop callback.
#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    /// A slot hotkey was pressed (idx is 0-based).
    Slot(usize),
    /// The broadcast toggle hotkey was pressed.
    BroadcastToggle,
}

const TIMER_FOCUS_POLL: usize = 1;
/// Poll interval in milliseconds for detecting focus changes (alt+Tab, clicks).
/// 250ms provides responsive detection without excessive CPU usage.
const FOCUS_POLL_INTERVAL_MS: u32 = 250;

/// Run the message loop and dispatch hotkey events to the callback.
/// The callback receives a [`HotkeyEvent`] for each registered hotkey.
/// If `poll_fn` is Some, a 500ms timer is set and `poll_fn()` is called
/// on each WM_TIMER tick — used for focus-polling in swap mode.
pub fn run_loop<F, P>(
    slot_count: usize,
    mut on_event: F,
    tray_hwnd: Option<HWND>,
    mut poll_fn: Option<P>,
) where
    F: FnMut(HotkeyEvent),
    P: FnMut(),
{
    unsafe {
        // Set up a repeating timer for focus polling
        if poll_fn.is_some() {
            SetTimer(0 as HWND, TIMER_FOCUS_POLL, FOCUS_POLL_INTERVAL_MS, None);
        }

        let mut msg: MSG = mem::zeroed();
        let _ = tray_hwnd;
        while GetMessageW(&mut msg, 0 as HWND, 0, 0) != 0 {
            if msg.message == WM_HOTKEY {
                let id = msg.wParam as i32;
                if id >= HOTKEY_BASE_ID && id < HOTKEY_BASE_ID + slot_count as i32 {
                    let idx = (id - HOTKEY_BASE_ID) as usize;
                    if idx < slot_count {
                        on_event(HotkeyEvent::Slot(idx));
                    }
                } else if id == BROADCAST_TOGGLE_ID {
                    on_event(HotkeyEvent::BroadcastToggle);
                }
            } else if msg.message == WM_TIMER && msg.wParam == TIMER_FOCUS_POLL {
                #[allow(clippy::collapsible_if)]
                if let Some(ref mut f) = poll_fn {
                    f();
                }
            }
            DispatchMessageW(&msg);
        }

        // Clean up the timer
        if poll_fn.is_some() {
            KillTimer(0 as HWND, TIMER_FOCUS_POLL);
        }
    }
}
