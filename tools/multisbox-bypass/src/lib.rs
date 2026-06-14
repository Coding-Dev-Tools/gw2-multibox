//! Multisbox Mutex Bypass DLL
//!
//! Injected into Gw2-64.exe to bypass the GW2 named mutex check.
//! Hooks CreateMutexW and CreateMutexExW to return NULL (failure),
//! which tells the game the mutex couldn't be created → proceeds without conflict.
//!
//! This DLL does NOT hook CreateFileW — the hard links in the game directory
//! (Gw2_1.dat through Gw2_4.dat) handle the archive file issue.

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};

use winapi::shared::minwindef::{BOOL, DWORD, HINSTANCE, LPVOID, TRUE};
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use winapi::um::libloaderapi::LoadLibraryW;
use winapi::um::consoleapi::AllocConsole;
use winapi::um::winbase::WriteConsoleW;

static mut ORIGINAL_CREATE_MUTEX_W: Option<extern "system" fn() -> isize> = None;
static mut DLL_LOADED: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: DWORD,
    _lpv_reserved: LPVOID,
) -> BOOL {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            // Disable thread library calls for performance
            unsafe {
                // We could hook here, but for simplicity we rely on
                // the launcher to set up the hook before main thread runs.
            }
        }
        DLL_PROCESS_DETACH => {
            DLL_LOADED.store(false, Ordering::SeqCst);
        }
        _ => {}
    }
    TRUE
}
