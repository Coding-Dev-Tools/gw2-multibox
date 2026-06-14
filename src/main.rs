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
use gw2_multibox::config::{self, Config};
use gw2_multibox::{broadcast, hotkey, http, launcher, log, mutex_kill, tray, window};
use std::env;
use std::path::PathBuf;
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
        "    (default)    Launch instances from config, position windows, register F1-FN hotkeys"
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
                unsafe {
                    window::apply_region(hwnd, region);
                }
                let mut pid: DWORD = 0;
                unsafe {
                    winapi::um::winuser::GetWindowThreadProcessId(hwnd, &mut pid);
                }
                positioned += 1;
                println!(
                    "  Slot {} ({}) -> {} ({},{} {}x{}) [pid {}]",
                    positioned,
                    slot.account,
                    region.name,
                    region.x,
                    region.y,
                    region.width,
                    region.height,
                    pid
                );
                log::info(&format!(
                    "Slot {} positioned at {} (hwnd {:x}, pid {})",
                    positioned, region.name, hwnd as usize, pid
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
            println!(
                "Found {}/{} windows. Hotkeys active for those slots.",
                positioned, slot_count
            );
            log::info(&format!(
                "Found {}/{} windows initially",
                positioned, slot_count
            ));
        }
    } else {
        // Direct mode: launch each slot's exe and find by PID
        // (file_lock is already held from the top of run_live)

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

        // Poll for windows by PID
        let poll_iterations = timeout / 100;
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
                    unsafe {
                        window::apply_region(hwnd, region);
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
                eprintln!(
                    "WARNING: Could not find window for slot {} (PID {}) within {}ms",
                    i + 1,
                    pid,
                    timeout
                );
                log::warn(&format!(
                    "Slot {} (PID {}) window not found within {}ms",
                    i + 1,
                    pid,
                    timeout
                ));
                windows.push(std::ptr::null_mut());
            }
        }
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

    // Register hotkeys — only for windows we actually found
    let active_count = windows.len();
    let mut hkm = hotkey::HotkeyManager::new();
    hkm.register(active_count)?;
    if active_count > 0 {
        println!(
            "\nHotkeys registered: F1..F{} to switch windows.",
            active_count
        );
        println!("Press Ctrl+C to exit.\n");
    }
    log::info(&format!(
        "Registered {} hotkeys (F1..F{})",
        active_count, active_count
    ));

    // Initialize input broadcasting
    let mut broadcast_mgr =
        broadcast::BroadcastManager::new(config.broadcast.clone(), windows.clone());
    let _broadcast_toggle_key = config.broadcast.toggle_key;
    if config.broadcast.enabled {
        if let Err(e) = broadcast_mgr.enable() {
            eprintln!("Warning: Failed to enable input broadcasting: {}", e);
            log::warn(&format!("Failed to enable broadcasting: {}", e));
        } else {
            println!("Input broadcasting enabled (toggle: F9).");
            log::info("Input broadcasting enabled");
        }
    } else {
        println!("Input broadcasting disabled (press F9 to toggle).");
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

    hotkey::run_loop(
        active_count,
        |idx| {
            if idx < windows.len() {
                let hwnd = windows[idx];
                if !hwnd.is_null() {
                    unsafe {
                        window::activate(hwnd);
                    }
                    broadcast_mgr.set_active_slot(idx);
                    println!("Switched to slot {}.", idx + 1);
                    log::info(&format!("Switched to slot {}", idx + 1));
                }
            }
        },
        tray_for_loop,
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
