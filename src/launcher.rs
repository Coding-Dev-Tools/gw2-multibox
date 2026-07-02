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
    let exe_quoted = format!("\"{}\"", profile.exe_path);
    if args_str.is_empty() {
        exe_quoted
    } else {
        format!("{} {}", exe_quoted, args_str)
    }
}

pub fn launch(profile: &GameProfile, extra_args: Option<&Vec<String>>) -> Result<DWORD> {
    let mut args_str = profile.args.join(" ");
    if let Some(extra) = extra_args {
        args_str.push(' ');
        args_str.push_str(&extra.join(" "));
    }

    let wide_exe = to_wide(&profile.exe_path);
    let wide_cmd = if args_str.is_empty() {
        to_wide(&profile.exe_path)
    } else {
        to_wide(&format!("{} {}", profile.exe_path, args_str))
    };

    let mut si: STARTUPINFOW = unsafe { mem::zeroed() };
    si.cb = mem::size_of::<STARTUPINFOW>() as DWORD;
    let mut pi: PROCESS_INFORMATION = unsafe { mem::zeroed() };

    let dir_wide = profile
        .working_dir
        .as_deref()
        .filter(|d| !d.is_empty())
        .map(to_wide);

    let dir_ptr = dir_wide
        .as_ref()
        .map(|v| v.as_ptr() as _)
        .unwrap_or(std::ptr::null_mut());

    unsafe {
        let ok = CreateProcessW(
            wide_exe.as_ptr() as _,
            wide_cmd.as_ptr() as _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            0,
            std::ptr::null_mut(),
            dir_ptr,
            &mut si,
            &mut pi,
        );
        if ok == 0 {
            return Err(anyhow::anyhow!(
                "CreateProcessW failed for '{}' (error {})",
                profile.exe_path,
                std::io::Error::last_os_error()
            ));
        }
        let _ = WaitForSingleObject(pi.hProcess, 500);
        let pid = pi.dwProcessId;
        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> GameProfile {
        GameProfile {
            name: "test".into(),
            exe_path: r"C:\Games\gw2\Gw2-64.exe".into(),
            args: vec!["-autologin".into(), "-windowed".into()],
            working_dir: None,
            window_ready_delay_ms: None,
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: None,
        }
    }

    #[test]
    fn build_command_line_no_extra_args() {
        let profile = test_profile();
        let cmd = build_command_line(&profile, None);
        assert!(cmd.starts_with(r#""C:\Games\gw2\Gw2-64.exe""#));
        assert!(cmd.contains("-autologin"));
        assert!(cmd.contains("-windowed"));
    }

    #[test]
    fn build_command_line_with_extra_args() {
        let profile = test_profile();
        let extra = vec!["-mapload".to_string(), "test_map".to_string()];
        let cmd = build_command_line(&profile, Some(&extra));
        assert!(cmd.contains("-mapload"));
        assert!(cmd.contains("test_map"));
    }

    #[test]
    fn build_command_line_no_args() {
        let profile = GameProfile {
            args: vec![],
            ..test_profile()
        };
        let cmd = build_command_line(&profile, None);
        // Should return just the quoted exe path
        assert_eq!(cmd, r#""C:\Games\gw2\Gw2-64.exe""#);
    }

    #[test]
    fn build_command_line_empty_extra_args() {
        let profile = test_profile();
        let extra: Vec<String> = vec![];
        let cmd = build_command_line(&profile, Some(&extra));
        // Should be same as no extra args
        assert!(cmd.starts_with(r#""C:\Games\gw2\Gw2-64.exe""#));
        assert!(cmd.contains("-autologin"));
        assert!(cmd.contains("-windowed"));
    }
}
