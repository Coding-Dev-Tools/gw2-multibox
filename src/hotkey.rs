//! Global hotkey registration and the Win32 message loop.

use anyhow::Result;
use std::mem;
use winapi::shared::minwindef::UINT;
use winapi::shared::windef::HWND;
use winapi::um::winuser::*;

pub const HOTKEY_BASE_ID: i32 = 100;
pub const VK_F1: u32 = 0x70;

pub struct HotkeyManager {
    count: usize,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self { count: 0 }
    }

    pub fn register(&mut self, slot_count: usize) -> Result<()> {
        for i in 0..slot_count {
            unsafe {
                let result =
                    RegisterHotKey(0 as HWND, HOTKEY_BASE_ID + i as i32, 0, VK_F1 + i as UINT);
                if result == 0 {
                    eprintln!(
                        "Warning: Failed to register F{} hotkey (error {})",
                        i + 1,
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
        self.count = slot_count;
        Ok(())
    }

    pub fn unregister_all(&self) {
        for i in 0..self.count {
            unsafe {
                UnregisterHotKey(0 as HWND, HOTKEY_BASE_ID + i as i32);
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

/// Run the message loop and dispatch hotkey events to the callback.
/// Returns when GetMessageW returns 0 (typically only on WM_QUIT).
/// If `tray_hwnd` is provided, it is passed to GetMessageW so tray messages arrive.
pub fn run_loop<F: FnMut(usize)>(slot_count: usize, mut on_hotkey: F, tray_hwnd: Option<HWND>) {
    unsafe {
        let mut msg: MSG = mem::zeroed();
        let tray = tray_hwnd.unwrap_or(0 as HWND);
        // GetMessageW with a specific HWND only retrieves messages for that window
        // plus thread messages (hotkeys). We use 0 to get all thread messages
        // and filter tray messages by hwnd.
        while GetMessageW(&mut msg, 0 as HWND, 0, 0) != 0 {
            if msg.message == WM_HOTKEY {
                let idx = (msg.wParam as i32).wrapping_sub(HOTKEY_BASE_ID) as usize;
                if idx < slot_count {
                    on_hotkey(idx);
                }
            }
            // Window messages (tray events etc.) are dispatched by DispatchMessageW
            // to the window's WndProc automatically.
            DispatchMessageW(&msg);
            let _ = tray; // tray messages handled by WndProc
        }
    }
}
