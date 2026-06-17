//! Multisbox — multiboxing launcher and window manager.
//!
//! Modes:
//!   (default)       Launch instances from config, position windows, register hotkeys
//!   --dry-run       Validate config and print plan
//!   --list-windows  Enumerate visible windows
//!   --ui            Start the web config UI on http://127.0.0.1:7878
//!   --ui-port N     Override UI port
//!   -c PATH         Config YAML path
//!   -h, --help      Print help

use anyhow::Result;
use gw2_multibox::config::{self, Config, LayoutMode};
use gw2_multibox::{broadcast, hotkey, http, launcher, log, mutex_kill, tray, window};
use std::cell::Cell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread;
use std::time::Duration;
use winapi::shared::minwindef::DWORD;
use winapi::shared::windef::HWND;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(PartialEq)]
enum Mode {
    Run,
    DryRun,
    ListWindows,
    Ui,
    Init,
    Gw2Init,
    WowInit,
    FfxivInit,
    EveInit,
    Help,
    Version,
}

struct Args {
    config_path: PathBuf,
    mode: Mode,
    ui_port: u16,
    debug: bool,
}

fn parse_args() -> Result<Args> {
    let raw: Vec<String> = env::args().collect();
    let mut mode = Mode::Run;
    let mut ui_port: u16 = http::DEFAULT_PORT;
    let mut debug = false;

    for a in &raw[1..] {
        match a.as_str() {
            "--help" | "-h" => mode = Mode::Help,
            "--version" | "-v" => mode = Mode::Version,
            "--dry-run" => mode = Mode::DryRun,
            "--list-windows" => mode = Mode::ListWindows,
            "--ui" => mode = Mode::Ui,
            "init" => mode = Mode::Init,
            "gw2-init" => mode = Mode::Gw2Init,
            "wow-init" => mode = Mode::WowInit,
            "ffxiv-init" => mode = Mode::FfxivInit,
            "eve-init" => mode = Mode::EveInit,
            "--debug" => debug = true,
            "--ui-port" => {
                // Handled below
            }
            _ => {}
        }
    }

    if let Some(i) = raw.iter().position(|a| a == "--ui-port") {
        let v = raw
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--ui-port requires a value"))?;
        ui_port = v
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid port: {}", v))?;
    }

    let config_path = raw
        .iter()
        .position(|a| a == "--config" || a == "-c")
        .and_then(|i| raw.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.yaml"));

    Ok(Args {
        config_path,
        mode,
        ui_port,
        debug,
    })
}

fn print_help() {
    println!(
        "multisbox v{} — multiboxing launcher and window manager\n",
        VERSION
    );
    println!("USAGE:");
    println!("    multisbox [OPTIONS]");
    println!("    multisbox init [-c PATH]");
    println!("    multisbox gw2-init [-c PATH]");
    println!("    multisbox wow-init [-c PATH]");
    println!("    multisbox ffxiv-init [-c PATH]");
    println!("    multisbox eve-init [-c PATH]\n");
    println!("OPTIONS:");
    println!("    -c, --config <PATH>    Config YAML file [default: config.yaml]");
    println!("        --dry-run          Validate config and print launch plan, then exit");
    println!("        --list-windows     Enumerate all visible top-level windows, then exit");
    println!("        --ui               Start the web config UI on http://127.0.0.1:7878");
    println!("        --ui-port <PORT>   Override UI port [default: 7878]");
    println!("        --debug            Enable debug logging");
    println!("    -h, --help             Print this help");
    println!("    -v, --version          Print version");
    println!();
    println!("SUBCOMMANDS:");
    println!("    init         Write a generic starter config to PATH (or config.yaml) and exit");
    println!(
        "    gw2-init     Write a GW2-optimized config (auto-detects install, 4 accounts, 2x2 grid)"
    );
    println!(
        "    wow-init     Write a WoW-optimized config (auto-detects install, 4 accounts, 2x2 grid)"
    );
    println!(
        "    ffxiv-init   Write a FFXIV-optimized config (auto-detects install, 4 accounts, 2x2 grid)"
    );
    println!(
        "    eve-init     Write an EVE-optimized config (auto-detects install, 4 accounts, 2x2 grid)"
    );
    println!();
    println!("MODES:");
    println!(
        "    (default)    Launch instances from config, position windows, register F6+ hotkeys"
    );
    println!("    --dry-run    Validate config, print what would happen, exit");
    println!("    --list-windows  Print all visible top-level windows (debug aid)");
    println!("    --ui         Serve the config editor on http://127.0.0.1:7878 (no launching)");
    println!();
    println!("EXAMPLES:");
    println!("    multisbox gw2-init              # One-click GW2 setup");
    println!("    multisbox wow-init              # One-click WoW setup");
    println!("    multisbox ffxiv-init            # One-click FFXIV setup");
    println!("    multisbox eve-init              # One-click EVE setup");
    println!("    multisbox gw2-init -c my.yaml   # Custom path");
    println!("    multisbox -c config.yaml --dry-run");
    println!("    multisbox --ui --ui-port 9000");
    println!("    multisbox -c config.yaml");
}

fn run_list_windows() -> Result<()> {
    println!("=== Visible Windows ===\n");
    let windows = window::list_all_visible();
    if windows.is_empty() {
        println!("(no visible windows found)");
        return Ok(());
    }
    println!("{:<10} {:<10} TITLE", "HWND", "PID");
    println!("{}", "-".repeat(70));
    for w in &windows {
        println!("{:<10x} {:<10} {}", w.hwnd as usize, w.pid, w.title);
    }
    println!("\n{} window(s) found.", windows.len());
    Ok(())
}

fn run_dry_run(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    println!("=== Config Validated ===");
    println!("Config: {:?}", config_path);
    println!();
    let resolved = config::resolve(&config)?;
    println!(
        "OK — {} accounts, {} profiles, {} regions, {} slots",
        resolved.accounts.len(),
        resolved.profiles.len(),
        resolved.regions.len(),
        config.team.slots.len(),
    );
    print_launch_plan(&config, &resolved);

    for w in config::check_exe_paths(&config) {
        eprintln!("Warning: {}", w);
    }
    println!("\nDry run complete. No processes launched.");
    Ok(())
}

fn print_launch_plan(config: &Config, resolved: &config::ResolvedConfig) {
    println!("\n=== Launch Plan ===");
    println!("Team: {}", config.team.name);
    println!(
        "Layout: {} ({} regions)",
        config.layout.name,
        config.layout.regions.len()
    );
    println!(
        "Stagger: {}ms, Window timeout: {}ms",
        config.default_stagger_ms(),
        config.default_timeout_ms()
    );
    println!();
    for slot in &config.team.slots {
        let account = resolved.accounts.get(slot.account.as_str()).unwrap();
        let profile = resolved.slot_to_profile[&slot.index];
        let region = resolved.slot_to_region[&slot.index];
        let cmdline = launcher::build_command_line(profile, account.extra_args.as_ref());
        println!("Slot {} (account: {})", slot.index, slot.account);
        println!("  Profile: {} -> {}", profile.name, profile.exe_path);
        println!(
            "  Region:  {} -> ({},{} {}x{})",
            region.name, region.x, region.y, region.width, region.height
        );
        println!("  Cmd:     {}", cmdline);
        if let Some(dir) = &profile.working_dir {
            println!("  Cwd:     {}", dir);
        }
        if let Some(mutex_name) = &profile.kill_mutex {
            println!("  Mutex:   {} (will kill)", mutex_name);
        }
        println!();
    }
}

fn run_init(config_path: &PathBuf) -> Result<()> {
    if config_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {:?}\n\
             Use a different path with -c, or delete the file first.",
            config_path
        );
        std::process::exit(2);
    }
    let cfg = Config::template();
    cfg.save(config_path)?;
    println!("Wrote starter config to {:?}", config_path);
    println!();
    println!("Next steps:");
    println!("  1. Edit the file and set your game path under game_profiles[0].exe_path");
    println!(
        "  2. Run `multisbox -c {:?} --dry-run` to validate",
        config_path
    );
    println!("  3. Run `multisbox -c {:?}` to launch", config_path);
    Ok(())
}

fn run_gw2_init(config_path: &PathBuf) -> Result<()> {
    if config_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {:?}\n\
             Use a different path with -c, or delete the file first.",
            config_path
        );
        std::process::exit(2);
    }
    let cfg = config::gw2_template();
    cfg.save(config_path)?;
    println!("✓ GW2 config written to {:?}", config_path);
    println!();
    println!("Auto-detected:");
    println!("  Game path: {}", cfg.game_profiles[0].exe_path);
    println!(
        "  Layout:    {} ({} regions)",
        cfg.layout.name,
        cfg.layout.regions.len()
    );
    println!("  Accounts:  {}", cfg.accounts.len());
    println!();
    println!("Next steps:");
    println!("  1. (Optional) Edit account names in config if different");
    println!(
        "  2. Run `multisbox -c {:?} --dry-run` to validate",
        config_path
    );
    println!(
        "  3. Run `multisbox -c {:?}` to launch 4 GW2 windows",
        config_path
    );
    println!();
    println!("Hotkeys: F1=Account1, F2=Account2, F3=Account3, F4=Account4");
    Ok(())
}

fn run_wow_init(config_path: &PathBuf) -> Result<()> {
    if config_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {:?}\n\
             Use a different path with -c, or delete the file first.",
            config_path
        );
        std::process::exit(2);
    }
    let cfg = config::wow_template();
    cfg.save(config_path)?;
    println!("✓ WoW config written to {:?}", config_path);
    println!();
    println!("Auto-detected:");
    println!("  Game path: {}", cfg.game_profiles[0].exe_path);
    println!(
        "  Layout:    {} ({} regions)",
        cfg.layout.name,
        cfg.layout.regions.len()
    );
    println!("  Accounts:  {}", cfg.accounts.len());
    println!();
    println!("Next steps:");
    println!("  1. (Optional) Edit account names in config if different");
    println!(
        "  2. Run `multisbox -c {:?} --dry-run` to validate",
        config_path
    );
    println!(
        "  3. Run `multisbox -c {:?}` to launch 4 WoW windows",
        config_path
    );
    println!();
    println!("Hotkeys: F1=Account1, F2=Account2, F3=Account3, F4=Account4");
    Ok(())
}

fn run_ffxiv_init(config_path: &PathBuf) -> Result<()> {
    if config_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {:?}\n\
             Use a different path with -c, or delete the file first.",
            config_path
        );
        std::process::exit(2);
    }
    let cfg = config::ffxiv_template();
    cfg.save(config_path)?;
    println!("✓ FFXIV config written to {:?}", config_path);
    println!();
    println!("Auto-detected:");
    println!("  Game path: {}", cfg.game_profiles[0].exe_path);
    println!(
        "  Layout:    {} ({} regions)",
        cfg.layout.name,
        cfg.layout.regions.len()
    );
    println!("  Accounts:  {}", cfg.accounts.len());
    println!();
    println!("Next steps:");
    println!("  1. (Optional) Edit account names in config if different");
    println!(
        "  2. Run `multisbox -c {:?} --dry-run` to validate",
        config_path
    );
    println!(
        "  3. Run `multisbox -c {:?}` to launch 4 FFXIV windows",
        config_path
    );
    println!();
    println!("Hotkeys: F1=Account1, F2=Account2, F3=Account3, F4=Account4");
    Ok(())
}

fn run_eve_init(config_path: &PathBuf) -> Result<()> {
    if config_path.exists() {
        eprintln!(
            "Refusing to overwrite existing file: {:?}\n\
             Use a different path with -c, or delete the file first.",
            config_path
        );
        std::process::exit(2);
    }
    let cfg = config::eve_template();
    cfg.save(config_path)?;
    println!("✓ EVE config written to {:?}", config_path);
    println!();
    println!("Auto-detected:");
    println!("  Game path: {}", cfg.game_profiles[0].exe_path);
    println!(
        "  Layout:    {} ({} regions)",
        cfg.layout.name,
        cfg.layout.regions.len()
    );
    println!("  Accounts:  {}", cfg.accounts.len());
    println!();
    println!("Next steps:");
    println!("  1. (Optional) Edit account names in config if different");
    println!(
        "  2. Run `multisbox -c {:?} --dry-run` to validate",
        config_path
    );
    println!(
        "  3. Run `multisbox -c {:?}` to launch 4 EVE windows",
        config_path
    );
    println!();
    println!("Hotkeys: F1=Account1, F2=Account2, F3=Account3, F4=Account4");
    Ok(())
}

fn run_ui(config_path: PathBuf, port: u16) -> Result<()> {
    println!("=== Multisbox Config UI ===");
    println!("Config: {:?}", config_path);
    println!("URL:    http://127.0.0.1:{}", port);
    println!();
    let server = http::Server::new(config_path)?;
    log::info(&format!("UI server starting on port {}", port));

    // Try to open the browser
    let url = format!("http://127.0.0.1:{}", port);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }
    println!("Open {} in your browser. Ctrl+C to exit.", url);
    server.serve(port)
}

/// Reposition all windows for the swap layout (ISBoxer-style).
/// Active slot gets full screen on top; others become thumbnails at the bottom.
///
/// In revision 2 we stopped promoting any window to HWND_TOPMOST. The old
/// behavior caused two real problems: (1) games like GW2 detect a
/// persistent TOPMOST window and re-anchor to their saved position, undoing
/// our layout within a second; (2) the active game window would stay above
/// every other app (browser, chat, etc.) even after the user alt-tabbed
/// away. Both are fixed by using HWND_TOP for all four windows and calling
/// `SetForegroundWindow` separately for focus.
fn reposition_swap_layout(windows: &[HWND], active_idx: usize, monitor: (i32, i32, i32, i32)) {
    let regions = window::swap_layout_positions(monitor, active_idx, windows.len());
    for (i, hwnd) in windows.iter().enumerate() {
        if hwnd.is_null() || i >= regions.len() {
            continue;
        }
        unsafe {
            // All slots use HWND_TOP (not TOPMOST) so the game doesn't
            // re-anchor, and so alt-tab works to leave the team.
            window::apply_region_zorder(*hwnd, &regions[i], false);
        }
    }
    // Separately hand focus to the newly active slot. SetWindowPos with
    // SWP_NOACTIVATE (used inside apply_region_zorder) deliberately
    // suppresses focus, so we need this second call to make the swap
    // "stick" for keyboard input.
    if let Some(&hwnd) = windows.get(active_idx)
        && !hwnd.is_null()
    {
        unsafe {
            window::activate(hwnd);
        }
    }
}

/// Continue polling for any missing slot windows until `target` is met
/// or `overall_timeout_ms` elapses. Returns the additional HWNDs found.
/// Used as a second pass after the initial launch wait, so the hotkey
/// manager can be registered with the full slot count.
fn discover_remaining_windows(
    pids: &[DWORD],
    already_known: &[HWND],
    overall_timeout_ms: u64,
) -> Vec<HWND> {
    let mut found: Vec<HWND> = Vec::new();
    let start = std::time::Instant::now();
    let mut still_missing: Vec<usize> = (0..pids.len())
        .filter(|i| pids[*i] != 0 && !already_known.iter().any(|h| !h.is_null()))
        .collect();
    let _ = &mut still_missing; // suppress unused-mut if Vec is empty

    while !still_missing.is_empty() && start.elapsed() < Duration::from_millis(overall_timeout_ms) {
        for &slot_idx in still_missing.iter() {
            let pid = pids[slot_idx];
            if pid == 0 {
                continue;
            }
            let hwnd = window::find_primary_by_pid(pid)
                .map(|w| w.hwnd)
                .or_else(|| window::find_any_window_by_pid(pid));
            if let Some(h) = hwnd
                && !already_known.contains(&h)
                && !found.contains(&h)
            {
                found.push(h);
                log::info(&format!(
                    "Second-pass: found slot {} window (hwnd {:x}, pid {})",
                    slot_idx + 1,
                    h as usize,
                    pid
                ));
            }
        }
        still_missing.retain(|&slot_idx| {
            let pid = pids[slot_idx];
            pid != 0
                && !found.iter().chain(already_known.iter()).any(|h| {
                    if h.is_null() {
                        return false;
                    }
                    let mut owner_pid: DWORD = 0;
                    unsafe {
                        winapi::um::winuser::GetWindowThreadProcessId(*h, &mut owner_pid);
                    }
                    owner_pid == pid
                })
        });
        if !still_missing.is_empty() {
            thread::sleep(Duration::from_millis(250));
        }
    }

    if !still_missing.is_empty() {
        log::warn(&format!(
            "Second-pass timed out: {} slot(s) still without a window: {:?}",
            still_missing.len(),
            still_missing.iter().map(|i| i + 1).collect::<Vec<_>>()
        ));
    }
    found
}

fn run_live(config_path: &PathBuf) -> Result<()> {
    let config = Config::load(config_path)?;
    log::info(&format!("Loaded config from {:?}", config_path));
    println!("=== Multisbox Launcher ===");
    println!("Config: {:?}", config_path);
    println!(
        "Team: {} ({} slots)",
        config.team.name,
        config.team.slots.len()
    );

    let resolved = config::resolve(&config)?;
    log::info("Config validated");

    for w in config::check_exe_paths(&config) {
        log::warn(&w);
        eprintln!("Warning: {}", w);
    }

    let stagger = config.default_stagger_ms();
    let timeout = config.default_timeout_ms();

    // Pre-open Gw2.dat with FULL shared access for the entire run.
    // This replicates GW2Launcher's FileManager_FileLocker pattern:
    // the file is held open with FILE_SHARE_READ|WRITE|DELETE so that
    // all GW2 instances we launch can open the same file.
    // The handle is held in `_file_lock` for the entire function scope
    // (dropped only when run_live returns).
    let gw2_dat_path = "C:\\Program Files\\Guild Wars 2\\Gw2.dat";
    let _file_lock = match gw2_multibox::file_lock::SharedFileLock::new(gw2_dat_path) {
        Ok(lock) => {
            log::info(&format!(
                "Pre-opened {} with FILE_SHARE_READ|WRITE|DELETE — multiple instances can now share",
                gw2_dat_path
            ));
            println!("Pre-opened Gw2.dat with shared access (FileLocker).");
            Some(lock)
        }
        Err(e) => {
            log::warn(&format!(
                "Could not pre-open {}: {} — second instance may fail",
                gw2_dat_path, e
            ));
            eprintln!("Warning: Could not pre-open Gw2.dat: {}", e);
            None
        }
    };

    // Check if we're in launcher mode (e.g. GW2Launcher.exe spawns game processes)
    let first_profile = resolved.slot_to_profile[&config.team.slots[0].index];
    let is_launcher_mode = first_profile.launcher_mode;
    let slot_count = config.team.slots.len();
    let is_swap = config.layout_mode() == LayoutMode::Swap;
    let monitor = {
        let monitors = window::list_monitors();
        if let Some(m) = monitors.first() {
            (m.x, m.y, m.width, m.height)
        } else {
            (0, 0, 1920, 1080)
        }
    };

    let mut windows: Vec<HWND> = Vec::new();

    if is_launcher_mode {
        // Launcher mode: launch the launcher once, then find game windows
        // Position each window as it appears — don't block waiting for all.
        let game_name = first_profile
            .game_process_name
            .as_deref()
            .unwrap_or("Gw2-64");

        println!(
            "Launcher mode: launching {} — positioning windows as they appear...",
            first_profile.exe_path
        );
        log::info(&format!(
            "Launcher mode: exe={} game_process={} max_slots={}",
            first_profile.exe_path, game_name, slot_count
        ));

        match launcher::launch(first_profile, None) {
            Ok(pid) => {
                log::info(&format!("Launcher PID = {}", pid));
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to launch {}: {}",
                    first_profile.exe_path, e
                );
                eprintln!("Continuing — looking for already-running game windows...");
                log::warn(&format!("Failed to launch launcher: {}", e));
            }
        }

        // Phase 1: Wait for first window (up to 30s), then continue immediately
        let first_timeout = 30_000 / 500;
        let mut positioned = 0usize;
        for _ in 0..first_timeout {
            let new_hwnds = window::collect_new_windows(game_name, &windows);
            for hwnd in new_hwnds {
                if positioned >= slot_count {
                    break;
                }
                windows.push(hwnd);
                let slot = &config.team.slots[positioned];
                let region = resolved.slot_to_region[&slot.index];
                // In swap mode, all windows overlap — reposition_swap_layout
                // handles it after the discovery loop. Skip raw positioning.
                if config.layout_mode() != LayoutMode::Swap {
                    unsafe {
                        window::apply_region(hwnd, region);
                    }
                }
                let mut pid: DWORD = 0;
                unsafe {
                    winapi::um::winuser::GetWindowThreadProcessId(hwnd, &mut pid);
                }
                positioned += 1;
                println!("  Slot {} ({}) [pid {}]", positioned, slot.account, pid);
                log::info(&format!(
                    "Slot {} discovered (hwnd {:x}, pid {})",
                    positioned, hwnd as usize, pid
                ));
            }
            if positioned > 0 {
                // Got at least 1 window — break out, set up hotkeys, continue
                break;
            }
            if positioned == 0 {
                println!("  Waiting for game windows...");
            }
            thread::sleep(Duration::from_millis(500));
        }

        if positioned == 0 {
            eprintln!(
                "ERROR: No {} windows found within 30s. Is the launcher running?",
                game_name
            );
            log::warn(&format!("No {} windows found within 30s", game_name));
        } else {
            // Second pass: keep polling for game windows by process
            // name until we have slot_count or overall_timeout_ms
            // elapses. In launcher mode the children are spawned by
            // Gw2Launcher.exe and we don't have their PIDs directly,
            // so we just keep asking the system for top-level windows
            // of `game_name` until the count matches what we need.
            let start = std::time::Instant::now();
            let max_extra_ms = timeout.saturating_sub(30_000);
            while positioned < slot_count && start.elapsed() < Duration::from_millis(max_extra_ms) {
                let new_hwnds = window::collect_new_windows(game_name, &windows);
                for hwnd in new_hwnds {
                    if positioned >= slot_count {
                        break;
                    }
                    windows.push(hwnd);
                    positioned += 1;
                    println!("  Slot {} (second pass) found", positioned);
                    log::info(&format!(
                        "Launcher-mode second pass: slot {} found (hwnd {:x})",
                        positioned, hwnd as usize
                    ));
                }
                if positioned < slot_count {
                    thread::sleep(Duration::from_millis(500));
                }
            }
            println!(
                "Found {}/{} windows. Hotkeys active for all {} slots.",
                positioned, slot_count, slot_count
            );
            log::info(&format!(
                "Found {}/{} windows after launcher-mode second pass",
                positioned, slot_count
            ));
            // Pad with nulls so window indices align with slot indices
            // (the hotkey handler reads `windows[idx]` and checks
            // `hwnd.is_null()`).
            while windows.len() < slot_count {
                windows.push(std::ptr::null_mut());
            }
        }
    } else {
        // Direct mode: launch each slot's exe and find by PID
        // (file_lock is already held from the top of run_live)

        // Per-account user data: build a junction at the standard
        // GW2 appdata path pointing to a per-account folder, for each
        // account. This is the same technique Gw2Launcher uses — it
        // gives each account its own Local.dat, GFXSettings.xml,
        // screenshots, and addons, while sharing the read-only
        // C:\Program Files\Guild Wars 2 directory.
        //
        // The junction is created ONCE before the loop and points at
        // the LAST account's folder. For multi-instance, only one
        // account's data can be active at the standard path at a time
        // (Windows junctions are not multi-target). The Gw2Launcher
        // model is: launch one account, when it closes swap the
        // junction to the next, etc.
        let standard_appdata = std::env::var("APPDATA")
            .map(|p| std::path::PathBuf::from(p).join("Guild Wars 2"))
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(r"C:\Users\home\AppData\Roaming\Guild Wars 2")
            });
        let multibox_data_root = std::env::var("LOCALAPPDATA")
            .map(|p| std::path::PathBuf::from(p).join("Multisbox").join("data"))
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(r"C:\Users\home\AppData\Local\Multisbox\data")
            });

        let mut junctions_created: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        for slot in &config.team.slots {
            let _account = resolved.accounts.get(slot.account.as_str()).unwrap();
            let profile = resolved.slot_to_profile[&slot.index];
            // Only set up junctions for GW2-like profiles (those with
            // a kill_mutex set, indicating they need per-account data).
            if profile.kill_mutex.is_none() {
                continue;
            }
            let per_account = multibox_data_root.join(&slot.account);
            if let Err(e) = std::fs::create_dir_all(&per_account) {
                log::warn(&format!(
                    "Could not create per-account dir {}: {}",
                    per_account.display(),
                    e
                ));
                continue;
            }
            // Track so we can clean up at the end.
            junctions_created.push((standard_appdata.clone(), per_account.clone()));
            // For multi-instance under a single user, only the LAST
            // account's junction is active. Document this and don't
            // try to swap during the loop (would require closing
            // accounts sequentially).
            log::info(&format!(
                "Per-account data dir for {}: {}",
                slot.account,
                per_account.display()
            ));
        }
        if !junctions_created.is_empty() {
            let (last_link, last_target) = junctions_created.last().unwrap().clone();
            // Remove the existing standard path (only if it's empty
            // or already a junction — never delete user data).
            if last_link.exists() {
                // Try removing as a junction first (rmdir on a junction
                // removes the junction without touching the target).
                let remove_result = gw2_multibox::junction::remove_junction(&last_link);
                if remove_result.is_err() {
                    // Not a junction or rmdir failed. Check if empty
                    // and only then remove the real directory.
                    let is_empty = std::fs::read_dir(&last_link)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false);
                    if is_empty {
                        let _ = std::fs::remove_dir(&last_link);
                    } else {
                        log::warn(&format!(
                            "Standard appdata path {} is not empty and not a junction; not modifying",
                            last_link.display()
                        ));
                    }
                }
            }
            match gw2_multibox::junction::create_junction(&last_link, &last_target) {
                Ok(()) => {
                    println!(
                        "Per-account data: junction at {} -> {}",
                        last_link.display(),
                        last_target.display()
                    );
                    log::info(&format!(
                        "Junction created: {} -> {}",
                        last_link.display(),
                        last_target.display()
                    ));
                }
                Err(e) => {
                    log::warn(&format!(
                        "Could not create per-account junction: {} (continuing without)",
                        e
                    ));
                }
            }
        }

        let mut pids: Vec<DWORD> = Vec::new();
        for (i, slot) in config.team.slots.iter().enumerate() {
            let account = resolved.accounts.get(slot.account.as_str()).unwrap();
            let profile = resolved.slot_to_profile[&slot.index];
            println!(
                "Launching slot {} (account: {}, profile: {})...",
                i + 1,
                slot.account,
                profile.name
            );
            log::info(&format!(
                "Launching slot {} (account={}, profile={})",
                i + 1,
                slot.account,
                profile.name
            ));
            let pid = if false {
                // Injection disabled — bypass DLL interferes with GW2 startup
                let bypass_dll = r"C:\Program Files\Guild Wars 2\multisbox_bypass_v2.dll";
                if std::path::Path::new(bypass_dll).exists() {
                    log::info(&format!(
                        "Slot {}: launching with bypass DLL injection (instance={})",
                        i + 1,
                        i + 1
                    ));
                    gw2_multibox::launcher_inject::launch_with_inject(
                        profile,
                        account.extra_args.as_ref(),
                        i + 1,
                        bypass_dll,
                    )?
                } else {
                    log::warn(&format!(
                        "Bypass DLL not found at {} — using direct launch (may fail)",
                        bypass_dll
                    ));
                    launcher::launch(profile, account.extra_args.as_ref())?
                }
            } else {
                launcher::launch(profile, account.extra_args.as_ref())?
            };
            log::info(&format!("Slot {} PID = {}", i + 1, pid));
            pids.push(pid);
            if let Some(mutex_name) = &profile.kill_mutex {
                // Wait briefly so the game has time to call CreateMutex
                thread::sleep(Duration::from_millis(1500));
                log::info(&format!(
                    "Slot {}: attempting to kill mutex '{}' in pid {}",
                    i + 1,
                    mutex_name,
                    pid
                ));
                match mutex_kill::kill_mutex_in_process(pid, mutex_name) {
                    Ok(mutex_kill::KillResult::Killed) => {
                        println!("  Slot {} mutex '{}' killed.", i + 1, mutex_name);
                        log::info(&format!("Slot {} mutex '{}' killed", i + 1, mutex_name));
                    }
                    Ok(mutex_kill::KillResult::NotFound) => {
                        println!(
                            "  Slot {} mutex '{}' not found (already killed or not created yet).",
                            i + 1,
                            mutex_name
                        );
                        log::warn(&format!(
                            "Slot {} mutex '{}' not found within budget",
                            i + 1,
                            mutex_name
                        ));
                    }
                    Err(e) => {
                        eprintln!("  Slot {} mutex kill failed: {} (continuing)", i + 1, e);
                        log::warn(&format!("Slot {} mutex kill failed: {}", i + 1, e));
                    }
                }
            }
            if i < config.team.slots.len() - 1 {
                thread::sleep(Duration::from_millis(stagger));
            }
        }
        println!("All {} instances launched.", pids.len());

        // First pass: quick poll for each slot's window. This is per-slot
        // with a per-slot budget of (timeout / 100) * 100ms, but the
        // total runtime is bounded by `timeout`. Most games that take
        // a long time to render their first frame (GW2, FFXIV) will
        // need the second pass below.
        let poll_iterations = timeout / 100;
        let mut missing_pids: Vec<(usize, DWORD)> = Vec::new();
        for (i, slot) in config.team.slots.iter().enumerate() {
            let pid = pids[i];
            let region = resolved.slot_to_region[&slot.index];
            let mut found = false;
            for _ in 0..poll_iterations {
                // First try the standard "visible + titled" path (works
                // for most games and matches what the web UI/debug
                // --list-windows shows). Fall back to the permissive
                // "any window owned by pid" path for games like GW2
                // whose main window is briefly hidden or has an empty
                // title during early startup.
                let primary = window::find_primary_by_pid(pid).map(|w| w.hwnd);
                let any = window::find_any_window_by_pid(pid);
                if let Some(hwnd) = primary.or(any) {
                    windows.push(hwnd);
                    // In tiled mode, position the window at its configured
                    // region. In swap mode, the second pass + final
                    // reposition_swap_layout below does the work, so we
                    // skip the raw positioning here to avoid a one-frame
                    // flicker at the wrong location.
                    if !is_swap {
                        unsafe {
                            window::apply_region_zorder(hwnd, region, false);
                        }
                    }
                    println!(
                        "Slot {} ({}) -> {} ({},{} {}x{}) [pid {}]",
                        i + 1,
                        slot.account,
                        region.name,
                        region.x,
                        region.y,
                        region.width,
                        region.height,
                        pid
                    );
                    log::info(&format!(
                        "Slot {} positioned at {} (hwnd {:x})",
                        i + 1,
                        region.name,
                        hwnd as usize
                    ));
                    found = true;
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
            if !found {
                missing_pids.push((i, pid));
                eprintln!(
                    "Slot {} ({}) first-pass miss; entering second pass",
                    i + 1,
                    slot.account
                );
            }
        }

        // Second pass: keep polling for any slot that didn't show a
        // window in the first pass, until we either find it or hit
        // another `timeout`. The previous code bailed out after the
        // first pass and registered hotkeys against only the slots
        // that succeeded, which left the rest of the team stranded at
        // GW2's default window position.
        if !missing_pids.is_empty() {
            let second_pass_pids: Vec<DWORD> = missing_pids.iter().map(|(_, pid)| *pid).collect();
            let extra = discover_remaining_windows(&second_pass_pids, &windows, timeout);
            for (hwnd, (slot_idx, pid)) in extra.iter().zip(missing_pids.iter()) {
                let slot = &config.team.slots[*slot_idx];
                let region = resolved.slot_to_region[&slot.index];
                if !is_swap {
                    unsafe {
                        window::apply_region_zorder(*hwnd, region, false);
                    }
                }
                windows.push(*hwnd);
                println!(
                    "Slot {} ({}) [second pass] -> {} (hwnd {:x}, pid {})",
                    slot_idx + 1,
                    slot.account,
                    region.name,
                    *hwnd as usize,
                    pid
                );
            }
            for (slot_idx, pid) in missing_pids.iter() {
                let _slot = &config.team.slots[*slot_idx];
                if !windows.iter().any(|h| {
                    if h.is_null() {
                        return false;
                    }
                    let mut owner: DWORD = 0;
                    unsafe {
                        winapi::um::winuser::GetWindowThreadProcessId(*h, &mut owner);
                    }
                    owner == *pid
                }) {
                    eprintln!(
                        "WARNING: Could not find window for slot {} (PID {}) after second pass",
                        slot_idx + 1,
                        pid
                    );
                    log::warn(&format!(
                        "Slot {} (PID {}) window not found after second pass",
                        slot_idx + 1,
                        pid
                    ));
                    // Pad with a null so the slot index in `windows` stays
                    // aligned with `config.team.slots` (used by hotkey
                    // indexing later).
                    windows.push(std::ptr::null_mut());
                }
            }
        }
    }

    // (is_swap and monitor are computed above, before the launch branch
    // that needs them)

    // In swap mode, reposition all discovered windows using ISBoxer layout
    if is_swap && !windows.is_empty() {
        reposition_swap_layout(&windows, 0, monitor);
        println!("Swap layout: slot 1 = full screen, others = bottom thumbnails.");
        log::info("Swap layout applied");
    }

    // Activate first slot
    if let Some(&hwnd) = windows.first()
        && !hwnd.is_null()
    {
        unsafe {
            window::activate(hwnd);
        }
        println!("Activated slot 1.");
        log::info("Activated slot 1");
    }

    // Register hotkeys for every configured slot, not just the ones we
    // successfully discovered. The previous behavior only registered
    // hotkeys for discovered windows, which meant slots that took longer
    // to spawn (typical for GW2 in launcher mode) had no hotkey at all —
    // even if their window showed up 5 seconds later. The hotkey handler
    // already guards `idx < windows.len()` and the slot's HWND is
    // non-null check, so missing slots just no-op with a friendly log.
    let active_count = slot_count;
    let hotkey_base = config
        .team
        .options
        .hotkey_base
        .unwrap_or_else(gw2_multibox::config::default_hotkey_base);
    let mut hkm = hotkey::HotkeyManager::new();
    hkm.register(active_count, hotkey_base)?;
    if let Err(e) = hkm.register_broadcast_toggle(config.broadcast.toggle_key) {
        eprintln!(
            "Warning: could not register broadcast toggle (VK 0x{:X}): {}",
            config.broadcast.toggle_key, e
        );
    }
    if active_count > 0 {
        // Build a friendly label like F6, F7, F8 for the base VK
        let label = |vk: u32| -> String {
            if (0x70..0x7B).contains(&vk) {
                format!("F{}", vk - 0x70 + 1)
            } else {
                format!("VK 0x{:X}", vk)
            }
        };
        let first = label(hotkey_base);
        let last = label(hotkey_base + active_count as u32 - 1);
        let found_now = windows.iter().filter(|h| !h.is_null()).count();
        println!(
            "\nHotkeys registered: {}..{} to switch windows (F1-F5 are reserved for your game).",
            first, last
        );
        if found_now < active_count {
            println!(
                "Note: {}/{} slot windows are visible right now. Hotkeys for the missing slots will activate once their windows appear.",
                found_now, active_count
            );
        }
        println!("Press Ctrl+C to exit.\n");
    }
    log::info(&format!(
        "Registered {} hotkeys (base VK 0x{:X}); {} window(s) discovered so far",
        active_count,
        hotkey_base,
        windows.iter().filter(|h| !h.is_null()).count()
    ));

    // Initialize input broadcasting
    let mut broadcast_mgr =
        broadcast::BroadcastManager::new(config.broadcast.clone(), windows.clone());

    // In swap mode, default to PostMessage delivery (no focus switching).
    // Users can override with broadcast.delivery_mode: focus_cycle in config.
    if is_swap && config.broadcast.delivery_mode == config::DeliveryMode::PostMessage {
        // Already the default, but log it for clarity
        log::info("Swap mode: using PostMessage delivery (no focus switching during broadcast)");
    }

    // If the user specified a target process (e.g. "Gw2-64"), discover
    // its windows and add them to the broadcast target list. In swap
    // mode, we also position them as they appear. We do a quick
    // non-blocking scan first, then rely on F9 refresh and hotkey
    // handlers to pick up new windows.
    let mut target_windows: Vec<HWND> = windows.clone();
    if let Some(proc) = &config.broadcast.target_process {
        let discovered = window::find_windows_by_process_name(proc);
        if !discovered.is_empty() {
            if is_swap {
                for hwnd in &discovered {
                    if !windows.contains(hwnd) {
                        windows.push(*hwnd);
                    }
                }
                reposition_swap_layout(&windows, 0, monitor);
            }
            println!(
                "Broadcast: discovered {} '{}' window(s)",
                discovered.len(),
                proc,
            );
            log::info(&format!(
                "Broadcast: discovered {} '{}' window(s)",
                discovered.len(),
                proc
            ));
            target_windows = discovered;
        } else {
            log::info(&format!(
                "Broadcast: no '{}' windows found yet — will discover on F9 refresh",
                proc
            ));
        }
    }
    broadcast_mgr.update_windows(target_windows);

    if config.broadcast.enabled {
        if let Err(e) = broadcast_mgr.enable() {
            eprintln!("Warning: Failed to enable input broadcasting: {}", e);
            log::warn(&format!("Failed to enable broadcasting: {}", e));
        } else {
            println!(
                "Input broadcasting enabled (toggle: VK 0x{:X}).",
                config.broadcast.toggle_key
            );
            log::info("Input broadcasting enabled");
        }
    } else {
        println!(
            "Input broadcasting disabled (press VK 0x{:X} to toggle).",
            config.broadcast.toggle_key
        );
    }

    // Initialize tray icon
    let tray_hwnd_result = unsafe { tray::create_hidden_window() };
    let mut tray_icon_hwnd: HWND = std::ptr::null_mut();
    let _tray_mgr = match tray_hwnd_result {
        Ok(hwnd) => {
            tray_icon_hwnd = hwnd;
            let mut mgr = tray::TrayManager::new(hwnd);
            if let Err(e) = mgr.init(VERSION) {
                eprintln!("Warning: Failed to initialize tray icon: {}", e);
                log::warn(&format!("Failed to initialize tray icon: {}", e));
            } else {
                println!("System tray icon initialized.");
                log::info("System tray icon initialized");
            }
            Some(mgr)
        }
        Err(e) => {
            eprintln!("Warning: Failed to create tray window: {}", e);
            log::warn(&format!("Failed to create tray window: {}", e));
            None
        }
    };
    // Message loop — passes tray HWND so tray events are dispatched
    let tray_for_loop = if tray_icon_hwnd.is_null() {
        None
    } else {
        Some(tray_icon_hwnd)
    };

    // Shared active slot index for focus polling
    let active_slot = Rc::new(Cell::new(0usize));

    // Build focus poll callback for swap mode: every 250ms, detect
    // focus changes and reposition.
    //
    // The previous revision required 2 polls of the same foreground
    // (~500ms) plus a 1-second cooldown before acting. That was tuned
    // for transient focus flicker (context menus, tooltips), but the
    // cost was a 1.5–2 second delay between clicking a thumbnail and
    // seeing the swap happen. In practice Windows delivers
    // WM_SETFOCUS within ~50ms of a real click, so a single
    // confirmation poll (~250ms) plus a short cooldown is sufficient
    // to reject transient events while keeping the swap feeling
    // instantaneous.
    let poll_fn = if is_swap {
        let windows_for_poll = windows.clone();
        let active_slot_clone = active_slot.clone();
        let broadcast_mgr_ptr = &broadcast_mgr as *const broadcast::BroadcastManager;
        let pending_fg = Rc::new(Cell::new(Option::<usize>::None));
        let last_reposition = Rc::new(Cell::new(
            std::time::Instant::now() - std::time::Duration::from_secs(2),
        ));
        let pending_fg_clone = pending_fg.clone();
        let last_reposition_clone = last_reposition.clone();
        Some(move || {
            if let Some(new_fg) = window::get_foreground_slot(&windows_for_poll) {
                let current = active_slot_clone.get();
                if new_fg != current && new_fg < windows_for_poll.len() {
                    let pending = pending_fg_clone.get();
                    if pending == Some(new_fg) {
                        // Cooldown: 250ms minimum between repositions.
                        let elapsed = last_reposition_clone.get().elapsed();
                        if elapsed >= std::time::Duration::from_millis(250) {
                            reposition_swap_layout(&windows_for_poll, new_fg, monitor);
                            active_slot_clone.set(new_fg);
                            pending_fg_clone.set(None);
                            last_reposition_clone.set(std::time::Instant::now());
                            // SAFETY: broadcast_mgr is alive for the entire
                            // message loop (held in scope above the run_loop
                            // call).
                            unsafe {
                                (*broadcast_mgr_ptr).set_active_slot(new_fg);
                            }
                            println!("Swapped to slot {} (focus stable).", new_fg + 1);
                            log::info(&format!("Focus swap to slot {}", new_fg + 1));
                        }
                    } else {
                        // First poll seeing this focus change — require
                        // it to be confirmed on the next poll (250ms
                        // later) before acting. This is what filters
                        // out transient foreground changes from context
                        // menus, popups, and tooltip hovers.
                        pending_fg_clone.set(Some(new_fg));
                    }
                } else {
                    // Focus is back to current or unknown — clear pending.
                    pending_fg_clone.set(None);
                }
            } else {
                // Foreground is not one of our windows (external app,
                // child window, etc.). Clear any pending change and
                // don't reposition.
                pending_fg_clone.set(None);
            }
        })
    } else {
        None
    };

    hotkey::run_loop(
        active_count,
        |event| match event {
            hotkey::HotkeyEvent::Slot(idx) => {
                if let Some(proc) = &config.broadcast.target_process {
                    let discovered = window::find_windows_by_process_name(proc);
                    if !discovered.is_empty() {
                        if is_swap {
                            let mut changed = false;
                            for hwnd in &discovered {
                                if !windows.contains(hwnd) {
                                    windows.push(*hwnd);
                                    changed = true;
                                }
                            }
                            if changed {
                                reposition_swap_layout(&windows, idx, monitor);
                            }
                        }
                        broadcast_mgr.update_windows(discovered);
                    }
                }
                if idx < windows.len() {
                    let hwnd = windows[idx];
                    if !hwnd.is_null() {
                        if is_swap {
                            // In swap mode, hotkeys do NOT switch windows.
                            // Only the broadcast active_slot is updated so keys
                            // are forwarded to the correct non-active windows.
                            // Window switching happens only via alt+Tab or mouse click
                            // (detected by the focus poll callback).
                            active_slot.set(idx);
                            broadcast_mgr.set_active_slot(idx);
                            println!(
                                "Broadcast target: slot {} (use alt+Tab or click to switch main window).",
                                idx + 1
                            );
                            log::info(&format!(
                                "Broadcast target set to slot {} (no window switch)",
                                idx + 1
                            ));
                        } else {
                            // Tiled mode: hotkeys switch windows as before
                            unsafe {
                                window::activate(hwnd);
                            }
                            active_slot.set(idx);
                            broadcast_mgr.set_active_slot(idx);
                            println!("Switched to slot {}.", idx + 1);
                            log::info(&format!("Switched to slot {}", idx + 1));
                        }
                    }
                }
            }
            hotkey::HotkeyEvent::BroadcastToggle => {
                if let Some(proc) = &config.broadcast.target_process {
                    let discovered = window::find_windows_by_process_name(proc);
                    if !discovered.is_empty() {
                        // In swap mode, add any new windows and reposition
                        if is_swap {
                            let mut changed = false;
                            for hwnd in &discovered {
                                if !windows.contains(hwnd) {
                                    windows.push(*hwnd);
                                    changed = true;
                                }
                            }
                            if changed {
                                reposition_swap_layout(&windows, 0, monitor);
                                println!("Swap layout: repositioned after refresh.");
                            }
                        }
                        println!(
                            "Broadcast: refreshed, found {} '{}' window(s)",
                            discovered.len(),
                            proc
                        );
                        log::info(&format!(
                            "Broadcast: refreshed, found {} '{}' window(s)",
                            discovered.len(),
                            proc
                        ));
                        broadcast_mgr.update_windows(discovered);
                    }
                }
                match broadcast_mgr.toggle() {
                    Ok(true) => println!("Input broadcasting: ON"),
                    Ok(false) => println!("Input broadcasting: OFF"),
                    Err(e) => eprintln!("Failed to toggle broadcasting: {}", e),
                }
            }
        },
        tray_for_loop,
        poll_fn,
    );

    // hkm auto-unregisters on drop
    log::info("Exited");
    println!("Exited.");
    Ok(())
}

fn main() -> Result<()> {
    let args = parse_args()?;

    if args.mode == Mode::Help {
        print_help();
        return Ok(());
    }
    if args.mode == Mode::Version {
        println!("multisbox v{}", VERSION);
        return Ok(());
    }

    log::init();
    if args.debug {
        log::set_level(log::Level::Debug);
    }

    // Enable per-monitor DPI awareness
    window::set_dpi_awareness();

    match args.mode {
        Mode::Run => run_live(&args.config_path),
        Mode::DryRun => run_dry_run(&args.config_path),
        Mode::ListWindows => run_list_windows(),
        Mode::Ui => run_ui(args.config_path, args.ui_port),
        Mode::Init => run_init(&args.config_path),
        Mode::Gw2Init => run_gw2_init(&args.config_path),
        Mode::WowInit => run_wow_init(&args.config_path),
        Mode::FfxivInit => run_ffxiv_init(&args.config_path),
        Mode::EveInit => run_eve_init(&args.config_path),
        Mode::Help | Mode::Version => Ok(()), // handled above
    }
}
