//! Process launching with Win32 CreateProcessW.

use crate::config::GameProfile;
use anyhow::Result;
use std::mem;
use winapi::shared::minwindef::DWORD;
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
use winapi::um::synchapi::WaitForSingleObject;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Quote a single command-line argument using the standard Windows
/// (MSVCRT `CommandLineToArgvW`) convention, so that arguments containing
/// spaces, tabs or quotes survive as ONE argument instead of being split.
///
/// Plain arguments without whitespace or quotes are returned unchanged.
pub fn quote_win_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.bytes().any(|b| b == b' ' || b == b'\t' || b == b'"') {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                // Escape all pending backslashes plus the quote itself.
                for _ in 0..(backslashes * 2 + 1) {
                    out.push('\\');
                }
                out.push('"');
                backslashes = 0;
            }
            _ => {
                for _ in 0..backslashes {
                    out.push('\\');
                }
                out.push(c);
                backslashes = 0;
            }
        }
    }
    // Double any trailing backslashes so they do not escape our closing quote.
    for _ in 0..(backslashes * 2) {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Join profile args and per-account extra args into one command-line tail,
/// quoting each argument individually. Returns an empty string when there
/// are no arguments at all (no stray trailing space).
fn build_args_string(args: &[String], extra: Option<&Vec<String>>) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(args.len() + extra.map_or(0, |e| e.len()));
    parts.extend(args.iter().map(|a| quote_win_arg(a)));
    if let Some(extra) = extra {
        parts.extend(extra.iter().map(|a| quote_win_arg(a)));
    }
    parts.join(" ")
}

/// Build the full human-readable command line for a launch (also used by
/// the dry-run output): quoted exe path followed by quoted arguments.
pub fn build_command_line(profile: &GameProfile, extra_args: Option<&Vec<String>>) -> String {
    let exe_quoted = format!("\"{}\"", profile.exe_path);
    let args_str = build_args_string(&profile.args, extra_args);
    if args_str.is_empty() {
        exe_quoted
    } else {
        format!("{} {}", exe_quoted, args_str)
    }
}

pub fn launch(profile: &GameProfile, extra_args: Option<&Vec<String>>) -> Result<DWORD> {
    let args_str = build_args_string(&profile.args, extra_args);

    let wide_exe = to_wide(&profile.exe_path);
    // lpCommandLine's first token should be the quoted application path so
    // that the child's own argv[0] parses correctly even when exe_path
    // contains spaces (e.g. the default C:\Program Files\Guild Wars 2\
    // install location). CreateProcessW uses lpApplicationName for the
    // actual binary; everything else reads the raw command line.
    let wide_cmd = if args_str.is_empty() {
        to_wide(&format!("\"{}\"", profile.exe_path))
    } else {
        to_wide(&format!("\"{}\" {}", profile.exe_path, args_str))
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
        let pid = pi.dwProcessId;
        // Best-effort: give the process a moment to initialize before we
        // return (callers start mutex-kill / window discovery right after).
        let _ = WaitForSingleObject(pi.hProcess, 500);
        // Both child handles MUST be closed - previously they leaked on
        // every launch (two handles per game instance).
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
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
    fn build_command_line_empty_extra_args_no_trailing_space() {
        let profile = test_profile();
        let extra: Vec<String> = vec![];
        let cmd = build_command_line(&profile, Some(&extra));
        assert!(cmd.starts_with(r#""C:\Games\gw2\Gw2-64.exe""#));
        assert!(cmd.contains("-autologin"));
        assert!(cmd.contains("-windowed"));
        assert!(!cmd.ends_with(' '));
    }

    #[test]
    fn quote_win_arg_plain_unchanged() {
        assert_eq!(quote_win_arg("-autologin"), "-autologin");
        assert_eq!(quote_win_arg("C:\\temp"), "C:\\temp");
    }

    #[test]
    fn quote_win_arg_spaces_are_one_argument() {
        assert_eq!(quote_win_arg("test map"), "\"test map\"");
        assert_eq!(quote_win_arg(""), "\"\"");
    }

    #[test]
    fn quote_win_arg_embedded_quotes_escaped() {
        assert_eq!(quote_win_arg("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn quote_win_arg_trailing_backslash_does_not_escape_closing_quote() {
        // A literal trailing backslash must not eat the closing quote when
        // the command line is parsed by CommandLineToArgvW. Only quoted
        // args are affected; a bare path with no whitespace stays unquoted.
        assert_eq!(quote_win_arg("C:\\dir"), "C:\\dir");
        assert_eq!(quote_win_arg("C:\\my dir\\"), "\"C:\\my dir\\\\\"");
    }

    #[test]
    fn args_with_spaces_survive_as_single_arguments() {
        let profile = GameProfile {
            exe_path: r"C:\Program Files\Guild Wars 2\Gw2-64.exe".into(),
            args: vec!["-custpath".into(), r"C:\My Games\data".into()],
            ..test_profile()
        };
        let cmd = build_command_line(&profile, None);
        assert!(cmd.starts_with(r#""C:\Program Files\Guild Wars 2\Gw2-64.exe""#));
        // The spaced value stays one quoted token, not two bare ones.
        assert!(cmd.contains(r#""C:\My Games\data""#));
    }
}
