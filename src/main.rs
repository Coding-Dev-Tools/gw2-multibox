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
use gw2_multibox::{hotkey, http, launcher, log, window};
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
    println!("    multisbox init [-c PATH]\n");
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
    println!("    init            Write a starter config to PATH (or config.yaml) and exit");
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
    println!("    multisbox init");
    println!("    multisbox init -c my-team.yaml");
    println!("    multisbox -c my-team.yaml --dry-run");
    println!("    multisbox --ui --ui-port 9000");
    println!("    multisbox -c my-team.yaml");
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

    // Launch instances
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
        let pid = launcher::launch(profile, account.extra_args.as_ref())?;
        log::info(&format!("Slot {} PID = {}", i + 1, pid));
        pids.push(pid);
        if i < config.team.slots.len() - 1 {
            thread::sleep(Duration::from_millis(stagger));
        }
    }
    println!("All {} instances launched.", pids.len());

    // Poll for windows
    let mut windows: Vec<HWND> = Vec::new();
    let poll_iterations = timeout / 100;
    for (i, slot) in config.team.slots.iter().enumerate() {
        let pid = pids[i];
        let region = resolved.slot_to_region[&slot.index];
        let mut found = false;
        for _ in 0..poll_iterations {
            if let Some(w) = window::find_primary_by_pid(pid) {
                windows.push(w.hwnd);
                unsafe {
                    window::apply_region(w.hwnd, region);
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
                    w.hwnd as usize
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

    // Register hotkeys
    let slot_count = config.team.slots.len();
    let mut hkm = hotkey::HotkeyManager::new();
    hkm.register(slot_count)?;
    println!(
        "\nHotkeys registered: F1..F{} to switch windows.",
        slot_count
    );
    println!("Press Ctrl+C to exit.\n");
    log::info(&format!(
        "Registered {} hotkeys (F1..F{})",
        slot_count, slot_count
    ));

    // Message loop
    hotkey::run_loop(slot_count, |idx| {
        if idx < windows.len() {
            let hwnd = windows[idx];
            if !hwnd.is_null() {
                unsafe {
                    window::activate(hwnd);
                }
                println!("Switched to slot {}.", idx + 1);
                log::info(&format!("Switched to slot {}", idx + 1));
            }
        }
    });

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

    match args.mode {
        Mode::Run => run_live(&args.config_path),
        Mode::DryRun => run_dry_run(&args.config_path),
        Mode::ListWindows => run_list_windows(),
        Mode::Ui => run_ui(args.config_path, args.ui_port),
        Mode::Init => run_init(&args.config_path),
        Mode::Help | Mode::Version => Ok(()), // handled above
    }
}
