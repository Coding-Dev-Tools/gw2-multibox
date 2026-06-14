//! Multisbox launcher with DLL injection for the bypass DLL.
//! Injects multisbox_bypass.dll into each Gw2-64.exe process to
//! redirect Gw2.dat → Gw2_N.dat based on the slot index.

use crate::config::GameProfile;
use crate::log;
use anyhow::Result;
use std::ffi::CString;
use std::mem;
use std::ptr;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::handleapi::CloseHandle;
use winapi::um::libloaderapi::GetModuleHandleA;
use winapi::um::processenv::SetEnvironmentVariableW;
use winapi::um::processthreadsapi::{
    CreateProcessW, PROCESS_INFORMATION, ResumeThread, STARTUPINFOW,
};
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winbase::CREATE_SUSPENDED;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Launch a process with the bypass DLL injected.
/// Sets MSBOX_INSTANCE env var, creates process suspended, injects DLL, resumes.
pub fn launch_with_inject(
    profile: &GameProfile,
    extra_args: Option<&Vec<String>>,
    instance: usize,
    bypass_dll_path: &str,
) -> Result<DWORD> {
    let mut args_str = profile.args.join(" ");
    if let Some(extra) = extra_args {
        args_str.push(' ');
        args_str.push_str(&extra.join(" "));
    }

    let wide_exe = to_wide(&profile.exe_path);
    // Quote the exe path in the command line so spaces are handled correctly
    let exe_quoted = format!("\"{}\"", profile.exe_path);
    let wide_cmd = if args_str.is_empty() {
        exe_quoted.clone()
    } else {
        format!("{} {}", exe_quoted, args_str)
    };
    let wide_cmd = to_wide(&wide_cmd);

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
        .unwrap_or(ptr::null_mut());

    // Set MSBOX_INSTANCE env var in the parent's environment so the
    // child process inherits it. (Custom env blocks REPLACE the entire
    // environment, which breaks loading of system DLLs — so we mutate
    // the parent env via SetEnvironmentVariableW and pass NULL.)
    // The DLL uses this as a fallback if it cannot determine the
    // instance from the exe name.
    let env_name = to_wide("MSBOX_INSTANCE");
    let env_value = to_wide(&instance.to_string());
    unsafe {
        SetEnvironmentVariableW(env_name.as_ptr(), env_value.as_ptr());
    }

    unsafe {
        let ok = CreateProcessW(
            wide_exe.as_ptr() as _,
            wide_cmd.as_ptr() as _,
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            CREATE_SUSPENDED,
            ptr::null_mut(),
            dir_ptr,
            &mut si,
            &mut pi,
        );
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            log::error(&format!(
                "CreateProcessW failed for '{}' (error {}: {})",
                profile.exe_path,
                err.raw_os_error().unwrap_or(0),
                err
            ));
            return Err(anyhow::anyhow!(
                "CreateProcessW failed for '{}' (error {})",
                profile.exe_path,
                err
            ));
        }
        log::info(&format!(
            "CreateProcessW succeeded for '{}', suspended with PID {}",
            profile.exe_path, pi.dwProcessId
        ));
    }

    // Inject the bypass DLL
    match unsafe { inject_dll(pi.hProcess, bypass_dll_path) } {
        Ok(()) => {
            log::info(&format!(
                "DLL injection succeeded: {} into PID {}",
                bypass_dll_path, pi.dwProcessId
            ));
        }
        Err(e) => {
            log::error(&format!(
                "DLL injection FAILED for PID {}: {}",
                pi.dwProcessId, e
            ));
            // Kill the suspended process since we can't inject
            unsafe {
                winapi::um::processthreadsapi::TerminateProcess(pi.hProcess, 1);
            }
            unsafe {
                CloseHandle(pi.hProcess);
                CloseHandle(pi.hThread);
            }
            return Err(e);
        }
    }

    // Resume the process
    unsafe {
        ResumeThread(pi.hThread);
    }

    let pid = pi.dwProcessId;

    unsafe {
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
    }

    Ok(pid)
}

/// Inject a DLL into a suspended process by writing the DLL path to
/// the process's address space and creating a remote thread to
/// call LoadLibraryA.
unsafe fn inject_dll(process: winapi::shared::ntdef::HANDLE, dll_path: &str) -> Result<()> {
    use winapi::um::memoryapi::{VirtualAllocEx, WriteProcessMemory};
    use winapi::um::processthreadsapi::CreateRemoteThread;

    // Get LoadLibraryA address
    let k32 = unsafe { GetModuleHandleA(c"kernel32.dll".as_ptr()) };
    if k32.is_null() {
        return Err(anyhow::anyhow!("Failed to get kernel32.dll handle"));
    }
    let load_library =
        unsafe { winapi::um::libloaderapi::GetProcAddress(k32, c"LoadLibraryA".as_ptr()) };
    if load_library.is_null() {
        return Err(anyhow::anyhow!("Failed to get LoadLibraryA address"));
    }

    // Allocate memory in the target process for the DLL path
    let path_bytes = CString::new(dll_path)?.into_bytes_with_nul();
    let path_len = path_bytes.len();

    let remote_mem = unsafe {
        VirtualAllocEx(
            process,
            ptr::null_mut(),
            path_len,
            winapi::um::winnt::MEM_COMMIT | winapi::um::winnt::MEM_RESERVE,
            winapi::um::winnt::PAGE_READWRITE,
        )
    };
    if remote_mem.is_null() {
        return Err(anyhow::anyhow!("VirtualAllocEx failed"));
    }

    // Write the DLL path to the remote process
    let mut written = 0;
    let write_result = unsafe {
        WriteProcessMemory(
            process,
            remote_mem,
            path_bytes.as_ptr() as LPVOID,
            path_len,
            &mut written,
        )
    };
    if write_result == 0 {
        return Err(anyhow::anyhow!("WriteProcessMemory failed"));
    }

    // Create a remote thread to call LoadLibraryA(dll_path)
    let mut thread_id: DWORD = 0;
    let thread = unsafe {
        CreateRemoteThread(
            process,
            ptr::null_mut(),
            0,
            Some(std::mem::transmute::<
                *mut _,
                unsafe extern "system" fn(*mut _) -> u32,
            >(load_library)),
            remote_mem,
            0,
            &mut thread_id,
        )
    };
    if thread.is_null() {
        return Err(anyhow::anyhow!("CreateRemoteThread failed"));
    }

    // Wait for the thread to finish
    unsafe {
        WaitForSingleObject(thread, 5000);
    }

    unsafe {
        winapi::um::handleapi::CloseHandle(thread);
    }

    Ok(())
}

/// Launch without injection (fallback for non-GW2 games)
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
        .unwrap_or(ptr::null_mut());

    unsafe {
        let ok = CreateProcessW(
            wide_exe.as_ptr() as _,
            wide_cmd.as_ptr() as _,
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            0,
            ptr::null_mut(),
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
