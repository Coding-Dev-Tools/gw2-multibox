//! System tray icon management using raw Win32 APIs.
//!
//! Provides a system tray icon with context menu for controlling the application.
//! Features:
//! - Show/hide main window
//! - Reload configuration
//! - Quit application
//!
//! Uses Shell_NotifyIconW directly via winapi (no external tray crate needed).

use anyhow::Result;
use std::mem;
use std::ptr;
use winapi::shared::minwindef::{DWORD, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, POINT};
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::shellapi::*;
use winapi::um::winuser::*;

/// Custom message for tray icon events.
const TRAY_ICON_MESSAGE: UINT = WM_USER + 1;

/// Tray icon ID.
const TRAY_ICON_ID: UINT = 1;

/// Context menu command IDs.
const CMD_SHOW_UI: UINT = 1001;
const CMD_RELOAD_CONFIG: UINT = 1002;
const CMD_QUIT: UINT = 1003;

/// Application status displayed in the tray tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    Running,
    Stopped,
}

impl TrayStatus {
    fn as_str(&self) -> &'static str {
        match self {
            TrayStatus::Running => "Running",
            TrayStatus::Stopped => "Stopped",
        }
    }
}

/// Tray icon manager using raw Win32 APIs.
pub struct TrayManager {
    hwnd: HWND,
    status: TrayStatus,
    initialized: bool,
}

impl TrayManager {
    /// Create a new tray manager. The hwnd must be a valid hidden window for receiving messages.
    pub fn new(hwnd: HWND) -> Self {
        Self {
            hwnd,
            status: TrayStatus::Running,
            initialized: false,
        }
    }

    /// Initialize the tray icon. Must be called after the message loop window is created.
    pub fn init(&mut self, version: &str) -> Result<()> {
        unsafe {
            let mut nid: NOTIFYICONDATAW = mem::zeroed();
            nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = TRAY_ICON_MESSAGE;

            // Set tooltip
            let tooltip = format!("Multisbox v{}", version);
            let wide_tooltip = to_wide(&tooltip);
            let copy_len = wide_tooltip.len().min(128);
            ptr::copy_nonoverlapping(wide_tooltip.as_ptr(), nid.szTip.as_mut_ptr(), copy_len);

            // Use a default application icon
            nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION);

            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                return Err(anyhow::anyhow!("Failed to add tray icon"));
            }

            self.initialized = true;
            crate::log::info("Tray icon initialized");
        }
        Ok(())
    }

    /// Update the tray icon status tooltip.
    pub fn set_status(&mut self, status: TrayStatus) {
        self.status = status;
        if !self.initialized {
            return;
        }

        unsafe {
            let mut nid: NOTIFYICONDATAW = mem::zeroed();
            nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_TIP;

            let tooltip = format!("Multisbox — {}", status.as_str());
            let wide_tooltip = to_wide(&tooltip);
            let copy_len = wide_tooltip.len().min(128);
            ptr::copy_nonoverlapping(wide_tooltip.as_ptr(), nid.szTip.as_mut_ptr(), copy_len);

            Shell_NotifyIconW(NIM_MODIFY, &mut nid);
        }
    }

    /// Handle a tray icon message. Returns the command ID if a menu item was clicked.
    pub fn handle_message(&self, _w_param: WPARAM, l_param: LPARAM) -> Option<TrayCommand> {
        if l_param as u32 == WM_LBUTTONDBLCLK {
            Some(TrayCommand::ShowUi)
        } else if l_param as u32 == WM_RBUTTONUP {
            self.show_context_menu();
            None
        } else {
            None
        }
    }

    /// Show the context menu at the tray icon position.
    fn show_context_menu(&self) {
        unsafe {
            let mut pt: POINT = mem::zeroed();
            GetCursorPos(&mut pt);

            let h_menu = CreatePopupMenu();
            if h_menu.is_null() {
                return;
            }

            // Add menu items
            let show_ui_text = to_wide("Show UI");
            AppendMenuW(
                h_menu,
                MF_STRING,
                CMD_SHOW_UI as usize,
                show_ui_text.as_ptr(),
            );

            let reload_text = to_wide("Reload Config");
            AppendMenuW(
                h_menu,
                MF_STRING,
                CMD_RELOAD_CONFIG as usize,
                reload_text.as_ptr(),
            );

            AppendMenuW(h_menu, MF_SEPARATOR, 0, ptr::null());

            let quit_text = to_wide("Quit");
            AppendMenuW(h_menu, MF_STRING, CMD_QUIT as usize, quit_text.as_ptr());

            // Required for tray icon context menus
            SetForegroundWindow(self.hwnd);

            let cmd = TrackPopupMenu(
                h_menu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                pt.x,
                pt.y,
                0,
                self.hwnd,
                ptr::null(),
            );

            DestroyMenu(h_menu);

            // Post the command to the window
            if cmd != 0 {
                PostMessageW(self.hwnd, TRAY_ICON_MESSAGE, cmd as WPARAM, 0);
            }
        }
    }

    /// Process a tray command.
    pub fn process_command(&self, cmd: UINT) -> Option<TrayCommand> {
        match cmd {
            CMD_SHOW_UI => Some(TrayCommand::ShowUi),
            CMD_RELOAD_CONFIG => Some(TrayCommand::ReloadConfig),
            CMD_QUIT => Some(TrayCommand::Quit),
            _ => None,
        }
    }

    /// Remove the tray icon.
    pub fn remove(&self) {
        if !self.initialized {
            return;
        }

        unsafe {
            let mut nid: NOTIFYICONDATAW = mem::zeroed();
            nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as DWORD;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;

            Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
    }

    /// Check if the tray icon is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Drop for TrayManager {
    fn drop(&mut self) {
        self.remove();
    }
}

/// Commands that can be triggered from the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ShowUi,
    ReloadConfig,
    Quit,
}

/// Convert a string to a null-terminated wide string.
fn to_wide(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Create a hidden window for tray icon message processing.
///
/// # Safety
///
/// The returned HWND must be used with the TrayManager and properly destroyed.
pub unsafe fn create_hidden_window() -> Result<HWND> {
    unsafe {
        let class_name = to_wide("MultisboxTray");

        let mut wc: WNDCLASSEXW = mem::zeroed();
        wc.cbSize = mem::size_of::<WNDCLASSEXW>() as UINT;
        wc.lpfnWndProc = Some(tray_wnd_proc);
        wc.hInstance = GetModuleHandleW(ptr::null());
        wc.lpszClassName = class_name.as_ptr();

        if RegisterClassExW(&wc) == 0 {
            return Err(anyhow::anyhow!(
                "Failed to register tray window class (error {})",
                std::io::Error::last_os_error()
            ));
        }

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            ptr::null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            ptr::null_mut(),
            wc.hInstance,
            ptr::null_mut(),
        );

        if hwnd.is_null() {
            return Err(anyhow::anyhow!(
                "Failed to create tray window (error {})",
                std::io::Error::last_os_error()
            ));
        }

        Ok(hwnd)
    }
}

/// Window procedure for the tray icon hidden window.
///
/// # Safety
///
/// This is a Windows callback. Must not panic.
unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == TRAY_ICON_MESSAGE {
            // Tray icon event - dispatch to context menu on right-click
            if l_param as u32 == WM_RBUTTONUP {
                // Show context menu at cursor position
                let mut pt: POINT = mem::zeroed();
                GetCursorPos(&mut pt);

                let h_menu = CreatePopupMenu();
                if !h_menu.is_null() {
                    let show_ui_text = to_wide("Show UI");
                    AppendMenuW(
                        h_menu,
                        MF_STRING,
                        CMD_SHOW_UI as usize,
                        show_ui_text.as_ptr(),
                    );

                    let reload_text = to_wide("Reload Config");
                    AppendMenuW(
                        h_menu,
                        MF_STRING,
                        CMD_RELOAD_CONFIG as usize,
                        reload_text.as_ptr(),
                    );

                    let quit_text = to_wide("Quit");
                    AppendMenuW(h_menu, MF_STRING, CMD_QUIT as usize, quit_text.as_ptr());

                    SetForegroundWindow(hwnd);
                    let cmd = TrackPopupMenu(
                        h_menu,
                        TPM_RETURNCMD | TPM_NONOTIFY,
                        pt.x,
                        pt.y,
                        0,
                        hwnd,
                        ptr::null(),
                    );
                    DestroyMenu(h_menu);

                    match cmd as UINT {
                        CMD_SHOW_UI => {
                            println!("Show UI requested (not yet implemented).");
                        }
                        CMD_RELOAD_CONFIG => {
                            println!("Reload config requested (not yet implemented).");
                        }
                        CMD_QUIT => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                }
            } else if l_param as u32 == WM_LBUTTONDBLCLK {
                println!("Show UI requested (not yet implemented).");
            }
            return 0;
        }

        DefWindowProcW(hwnd, msg, w_param, l_param)
    }
}
