//! Windows mutex-kill helper.
//!
//! Port of the technique used by `Healix/Gw2Launcher`'s `ProcessUtil` tool
//! (see <https://github.com/Healix/Gw2Launcher>). The game creates a named
//! mutex on startup to prevent a second instance; we enumerate that mutex
//! handle in the running process and close it via `DuplicateHandle` with
//! `DUPLICATE_CLOSE_SOURCE`. The game is no longer the owner, so a new
//! instance can start cleanly.
//!
//! This is safer than DLL injection (no AV heuristic on
//! `CREATE_SUSPENDED`+`LoadLibrary`), requires no extra binary, and matches
//! the upstream design 1:1.

#![cfg(windows)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::time::{Duration, Instant};

use winapi::shared::minwindef::FALSE;
use winapi::um::handleapi::{CloseHandle, DuplicateHandle};
use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcess};
use winapi::um::winnt::{HANDLE, PROCESS_DUP_HANDLE, PROCESS_QUERY_INFORMATION};

const ERROR_INVALID_PARAMETER: i32 = 87;

/// Canonical Guild Wars 2 instance mutex name. ArenaNet has used this for
/// years; override per-profile via the `kill_mutex` config field if it ever
/// changes.
pub const GW2_MUTEX_NAME: &str = "ANET-WIN32-MUTEX";

/// How long `kill_mutex_in_process` will keep searching before giving up.
/// GW2 usually creates the mutex within ~500ms of process start; 5s is a
/// generous upper bound.
const SEARCH_BUDGET: Duration = Duration::from_secs(5);

/// Result of a mutex search/kill.
#[derive(Debug)]
pub enum KillResult {
    /// The named mutex was found and successfully closed in the target.
    Killed,
    /// The named mutex was not found within the budget. Either the game
    /// hasn't created it yet, or it was already killed.
    NotFound,
}

/// Search for `mutex_name` in the handle table of process `pid` and close it.
///
/// This mirrors `Win32Handles.GetHandle` in Healix's `ProcessUtil`: it
/// enumerates all system handles via `NtQuerySystemInformation(64)`, walks
/// the entries belonging to the target pid, reads each handle's object
/// name via `NtQueryObject(1)`, and on match calls
/// `DuplicateHandle(... DUPLICATE_CLOSE_SOURCE)` so the target process
/// loses the handle and a second instance can be launched.
///
/// Returns `Ok(KillResult::Killed)` when the mutex is closed,
/// `Ok(KillResult::NotFound)` if the budget elapses without a match, and
/// `Err(_)` only on hard OS errors (e.g. access denied, invalid pid).
pub fn kill_mutex_in_process(pid: u32, mutex_name: &str) -> io::Result<KillResult> {
    if pid == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid 0 is the system idle process; refusing to touch it",
        ));
    }

    let target_wide = to_wide(mutex_name);
    let deadline = Instant::now() + SEARCH_BUDGET;

    loop {
        if let Some(handle) = find_mutex_handle(pid, &target_wide)? {
            close_handle_in_target(pid, handle)?;
            return Ok(KillResult::Killed);
        }
        if Instant::now() >= deadline {
            return Ok(KillResult::NotFound);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Convenience: kill the canonical GW2 mutex in `pid`. Logs nothing; the
/// caller decides what to do with the result.
pub fn kill_gw2_mutex(pid: u32) -> io::Result<KillResult> {
    kill_mutex_in_process(pid, GW2_MUTEX_NAME)
}

// ---------------------------------------------------------------------------
// ntdll FFI — `winapi` 0.3 doesn't ship these, so we declare them locally
// and link ntdll explicitly. The signatures are stable (used by every
// process-explorer-style tool on Windows for 20+ years).
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SystemHandleTableEntryInfoEx {
    object: *mut core::ffi::c_void,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

const SYSTEM_EXTENDED_HANDLE_INFORMATION: u32 = 64;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
const DUPLICATE_CLOSE_SOURCE: u32 = 0x0000_0001;
const OBJECT_NAME_INFORMATION: u32 = 1;
const STATUS_SUCCESS: i32 = 0;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(
        system_information_class: u32,
        system_information: *mut core::ffi::c_void,
        system_information_length: u32,
        return_length: *mut u32,
    ) -> i32;

    fn NtQueryObject(
        object_handle: HANDLE,
        object_information_class: u32,
        object_information: *mut core::ffi::c_void,
        object_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn find_mutex_handle(pid: u32, needle_wide: &[u16]) -> io::Result<Option<usize>> {
    // Start with a reasonable buffer, grow on STATUS_INFO_LENGTH_MISMATCH
    let mut buf_len: u32 = 0x40000;
    let mut buffer: Vec<u8>;
    let mut info_ptr: *mut core::ffi::c_void;

    loop {
        buffer = vec![0u8; buf_len as usize];
        info_ptr = buffer.as_mut_ptr() as *mut _;
        let mut return_length: u32 = 0;
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_EXTENDED_HANDLE_INFORMATION,
                info_ptr,
                buf_len,
                &mut return_length,
            )
        };
        if status == STATUS_SUCCESS {
            break;
        }
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf_len = return_length.max(buf_len).saturating_add(0x1000);
            continue;
        }
        return Err(io::Error::other(format!(
            "NtQuerySystemInformation failed: status=0x{:x}",
            status
        )));
    }

    // First 8 bytes = handle count (ULONG_PTR; 4 on 32-bit, 8 on 64-bit)
    let handle_count = unsafe {
        if (mem::size_of::<usize>()) == 8 {
            *(info_ptr as *const u64)
        } else {
            *(info_ptr as *const u32) as u64
        }
    };

    // Handle entries start at offset 16 (64-bit) or 8 (32-bit)
    let entry_size = core::mem::size_of::<SystemHandleTableEntryInfoEx>();
    let entries_start = unsafe {
        if (mem::size_of::<usize>()) == 8 {
            info_ptr.add(16)
        } else {
            info_ptr.add(8)
        }
    };

    for i in 0..handle_count {
        let entry_ptr = unsafe { entries_start.add((i as usize) * entry_size) };
        let entry: SystemHandleTableEntryInfoEx =
            unsafe { core::ptr::read_unaligned(entry_ptr as *const _) };

        if entry.unique_process_id as u32 != pid {
            continue;
        }

        let handle_value = entry.handle_value as HANDLE;
        if handle_value.is_null() {
            continue;
        }

        if let Some(name) = query_object_name(handle_value) {
            // Compare case-insensitive, full-match. GW2's mutex shows up as
            // e.g. "\Sessions\1\BaseNamedObjects\ANET-WIN32-MUTEX"; we want
            // the suffix match because the session prefix varies by login
            // session.
            if wide_ends_with_ignore_ascii_case(&name, needle_wide) {
                return Ok(Some(entry.handle_value));
            }
        }
    }

    Ok(None)
}

fn query_object_name(handle: HANDLE) -> Option<Vec<u16>> {
    // Step 1: ask for the size with a small buffer (returns the real size)
    let mut needed: u32 = 0;
    let _ = unsafe {
        NtQueryObject(
            handle,
            OBJECT_NAME_INFORMATION,
            core::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if needed == 0 || needed < 8 {
        // Most Mutant (mutex) objects in user sessions are nameable, so a
        // zero-length result means we can't query (e.g. an unnamed object).
        return None;
    }

    // The UNICODE_STRING header (8 bytes on 32-bit, 16 bytes on 64-bit —
    // actually always 8 for Length+MaximumLength + 4/8 padding for Buffer
    // ptr; on 64-bit it's 8+8=16) is written into the start of our buffer.
    // The `Buffer` pointer field points into the TARGET process's address
    // space — we must NOT deref it directly. We need a second call with
    // a buffer sized for (header + name data) so the API writes the data
    // into our buffer too.
    //
    // Allocate header + up to 32768 UTF-16 chars (64 KB) of name data,
    // which is far more than any real object name. Loop with a bigger
    // buffer if STATUS_INFO_LENGTH_MISMATCH.
    let mut buf_len: u32 = 0x10000;
    let mut status: i32;
    let mut actual: u32;
    let mut data: Vec<u8>;

    loop {
        data = vec![0u8; buf_len as usize];
        actual = 0;
        status = unsafe {
            NtQueryObject(
                handle,
                OBJECT_NAME_INFORMATION,
                data.as_mut_ptr() as *mut _,
                buf_len,
                &mut actual,
            )
        };
        if status == STATUS_SUCCESS {
            break;
        }
        if status == STATUS_INFO_LENGTH_MISMATCH {
            buf_len = actual.max(buf_len).saturating_add(0x1000);
            if buf_len > 0x100000 {
                // Cap at 1 MB; real object names are never this long
                return None;
            }
            continue;
        }
        return None;
    }

    // On 64-bit, the UNICODE_STRING is 16 bytes (USHORT Length,
    // USHORT MaximumLength, then 4 bytes of padding, then a pointer).
    // On 32-bit it's 8 bytes. Read it carefully.
    #[cfg(target_pointer_width = "64")]
    let unicode: (u16, u16, u32, *mut u16) = unsafe {
        let p = data.as_ptr() as *const (u16, u16, u32, *mut u16);
        core::ptr::read_unaligned(p)
    };
    #[cfg(target_pointer_width = "32")]
    let unicode: (u16, u16, *mut u16) = unsafe {
        let p = data.as_ptr() as *const (u16, u16, *mut u16);
        core::ptr::read_unaligned(p)
    };

    #[cfg(target_pointer_width = "64")]
    let (length, _max_length, _pad, _buffer_ptr) = unicode;
    #[cfg(target_pointer_width = "32")]
    let (length, _max_length, _buffer_ptr) = unicode;

    if length == 0 {
        return None;
    }
    let char_count = (length / 2) as usize;
    if char_count * 2 + 16 > data.len() {
        return None;
    }

    // The string data follows the UNICODE_STRING header in our buffer.
    let data_offset = if cfg!(target_pointer_width = "64") {
        16
    } else {
        8
    };
    let name_bytes = &data[data_offset..data_offset + char_count * 2];
    let mut out = vec![0u16; char_count];
    unsafe {
        core::ptr::copy_nonoverlapping(
            name_bytes.as_ptr(),
            out.as_mut_ptr() as *mut u8,
            char_count * 2,
        );
    }
    Some(out)
}

fn close_handle_in_target(pid: u32, handle_value: usize) -> io::Result<()> {
    let desired = PROCESS_DUP_HANDLE | PROCESS_QUERY_INFORMATION;
    let target = unsafe { OpenProcess(desired, FALSE, pid) };
    if target.is_null() {
        return Err(io::Error::last_os_error());
    }

    let me = unsafe { GetCurrentProcess() };
    let mut duped: HANDLE = core::ptr::null_mut();
    let handle_as = handle_value as HANDLE;
    let ok = unsafe {
        DuplicateHandle(
            target,
            handle_as,
            me,
            &mut duped,
            0,
            FALSE,
            DUPLICATE_CLOSE_SOURCE,
        )
    };
    unsafe {
        CloseHandle(target);
        if !duped.is_null() {
            CloseHandle(duped);
        }
    }
    if ok == 0 {
        let err = io::Error::last_os_error();
        // ERROR_INVALID_PARAMETER (87) often means the handle was already
        // closed by the game itself. Treat as success.
        if err.raw_os_error() == Some(ERROR_INVALID_PARAMETER) {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// True if `haystack` ends with `suffix`, both being UTF-16 (low-byte
/// ASCII), compared case-insensitively. Windows object paths are uppercase
/// for the namespace prefix and the object name is uppercase by convention
/// for named mutexes from ArenaNet.
fn wide_ends_with_ignore_ascii_case(haystack: &[u16], suffix: &[u16]) -> bool {
    if suffix.is_empty() || haystack.len() < suffix.len() {
        return false;
    }
    let h = &haystack[haystack.len() - suffix.len()..];
    h.iter().zip(suffix.iter()).all(|(a, b)| {
        // Windows object paths and named mutexes are pure ASCII
        let la = (*a as u8).to_ascii_lowercase() as u16;
        let lb = (*b as u8).to_ascii_lowercase() as u16;
        la == lb
    })
}

use core::mem;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutex_kill_name_constant_is_not_empty() {
        assert!(!GW2_MUTEX_NAME.is_empty());
        assert!(GW2_MUTEX_NAME.contains("ANET"));
    }

    #[test]
    fn mutex_kill_returns_err_for_zero_pid() {
        let r = kill_mutex_in_process(0, GW2_MUTEX_NAME);
        assert!(r.is_err());
    }
}
