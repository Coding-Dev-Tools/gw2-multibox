//! Process launching with Win32 CreateProcessW.

use crate::config::GameProfile;
use anyhow::Result;
use std::mem;
use winapi::shared::minwindef::DWORD;
use winapi::um::processthreadsapi::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
use winapi::um::synchapi::WaitForSingleObject;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn build_command_line(profile: &GameProfile, extra_args: Option<&Vec<String>>) -> String {
    let mut args_str = profile.args.join(" ");
    if let Some(extra) = extra_args {
        args_str.push(' ');
        args_str.push_str(&extra.join(" "));
    }
    format!("{} {}", profile.exe_path, args_str)
}

pub fn launch(profile: &GameProfile, extra_args: Option<&Vec<String>>) -> Result<DWORD> {
    let full_cmd = build_command_line(profile, extra_args);
    let working_dir = profile.working_dir.as_deref().unwrap_or("");

    let wide_cmd = to_wide(&full_cmd);
    let wide_dir = to_wide(working_dir);

    let mut si: STARTUPINFOW = unsafe { mem::zeroed() };
    si.cb = mem::size_of::<STARTUPINFOW>() as DWORD;
    let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

    unsafe {
        let ok = CreateProcessW(
            std::ptr::null(),
            wide_cmd.as_ptr() as _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            0,
            std::ptr::null_mut(),
            wide_dir.as_ptr() as _,
            &mut si,
            &mut pi,
        );
        if ok == 0 {
            return Err(anyhow::anyhow!(
                "CreateProcessW failed for '{}' (error {})",
                full_cmd, std::io::Error::last_os_error()
            ));
        }
        let _ = WaitForSingleObject(pi.hProcess, 500);
        let pid = pi.dwProcessId;
        Ok(pid)
    }
}
