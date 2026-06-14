//! Windows file-lock helper for multiboxing.
//!
//! Implements the GW2Launcher `FileManager_FileLocker` pattern.
//! Pre-opens `Gw2.dat` with `FileShare.Read | FileShare.Write | FileShare.Delete`
//! so that subsequent GW2 instances can also open the same file with
//! compatible share modes. Without this, the first GW2 instance opens the
//! file exclusively and subsequent instances get "Unable To Open Archive File".

#![cfg(windows)]

use std::io;
use std::path::Path;
use winapi::um::winnt::{
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, HANDLE,
};

#[cfg(windows)]
use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;

/// Open `Gw2.dat` (or any archive file) with FULL shared access.
/// Keep the returned handle open for as long as you need multiple
/// GW2 instances to coexist. Drop the handle to release the lock.
///
/// Returns `Ok(Some(handle))` on success, `Ok(None)` if the file doesn't exist,
/// or `Err` on hard OS errors.
pub fn pre_open_shared(path: &str) -> io::Result<Option<HANDLE>> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    if !Path::new(path).exists() {
        return Ok(None);
    }

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        );

        if handle.is_null() || handle == winapi::um::handleapi::INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(2) {
                // File not found
                return Ok(None);
            }
            return Err(err);
        }

        Ok(Some(handle))
    }
}

/// Close a handle opened by `pre_open_shared`.
///
/// # Safety
///
/// Caller must ensure `handle` is a valid handle returned by
/// [`pre_open_shared`] and has not already been closed.
pub unsafe fn close_shared(handle: HANDLE) {
    unsafe {
        CloseHandle(handle);
    }
}

/// RAII wrapper that opens the file on creation and closes on drop.
pub struct SharedFileLock {
    handle: HANDLE,
}

impl SharedFileLock {
    pub fn new(path: &str) -> io::Result<Self> {
        match pre_open_shared(path)? {
            Some(handle) => Ok(SharedFileLock { handle }),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path),
            )),
        }
    }
}

impl Drop for SharedFileLock {
    fn drop(&mut self) {
        unsafe { close_shared(self.handle) };
    }
}
