//! Config schema, parsing, and validation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use winapi::shared::minwindef::*;
use winapi::um::winnt::*;
use winapi::um::winreg::*;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GameProfile {
    pub name: String,
    pub exe_path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    /// How long to wait after launch before polling for the window (ms).
    /// Lets the game initialize its renderer before we try to position it.
    #[serde(default)]
    pub window_ready_delay_ms: Option<u64>,
    /// If true, exe_path is a launcher (e.g. GW2Launcher.exe) that spawns
    /// the actual game processes. Multisbox will launch it once and then
    /// search for game windows by process name instead of by PID.
    #[serde(default)]
    pub launcher_mode: bool,
    /// When launcher_mode=true, the process name to search for (e.g. "Gw2-64").
    #[serde(default)]
    pub game_process_name: Option<String>,
    /// If set, after the process is launched the launcher will locate and
    /// close a named mutex in the process's handle table. Used to bypass
    /// GW2's single-instance check (`ANET-WIN32-MUTEX`). `None` (default)
    /// means no kill is attempted — safe for any game that doesn't enforce
    /// a mutex.
    #[serde(default)]
    pub kill_mutex: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Account {
    pub name: String,
    pub game_profile: String,
    #[serde(default)]
    pub extra_args: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Region {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Layout {
    pub name: String,
    pub regions: Vec<Region>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Slot {
    pub index: usize,
    pub account: String,
    pub region: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TeamOptions {
    /// Delay between launches in milliseconds. Default: 3000
    #[serde(default)]
    pub stagger_delay_ms: Option<u64>,
    /// Window discovery timeout in milliseconds. Default: 60000
    #[serde(default)]
    pub window_timeout_ms: Option<u64>,
    /// Base VK code for slot hotkeys. Default: 0x75 (F6).
    /// Slot 1 = base, slot 2 = base+1, etc.
    /// Set higher (e.g. 0x79 = F10) to avoid colliding with game hotkeys
    /// on F1-F5.
    #[serde(default)]
    pub hotkey_base: Option<u32>,
    /// Window layout mode. Default: tiled.
    /// "swap" = ISBoxer-style: focused window full-screen, others
    /// as small thumbnails at the bottom; clicking a thumbnail swaps it.
    #[serde(default)]
    pub layout_mode: Option<LayoutMode>,
}

pub fn default_hotkey_base() -> u32 {
    // F6 = 0x75 — skip F1-F5 which are commonly used by games
    0x75
}

/// Window layout mode.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    /// Classic tiled layout — each slot gets its own region, no overlap.
    #[default]
    Tiled,
    /// ISBoxer-style swap layout — focused window is full-screen,
    /// other windows are small thumbnails at the bottom. Clicking a
    /// thumbnail swaps it to become the focused window.
    Swap,
}

/// Broadcast delivery mode — how keys are sent to non-active windows.
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    /// PostMessage WM_KEYDOWN/WM_KEYUP — no focus switching required.
    /// Does NOT work with DirectInput/Raw Input games (GW2, many others).
    /// Only works with games that read keyboard input from the window message queue.
    PostMessage,
    /// Focus cycling — briefly SetForegroundWindow each target, SendInput,
    /// then restore the original foreground window. This is the ONLY reliable
    /// method for DirectInput/Raw Input games like GW2. The layout stays
    /// visually stable because the original foreground is restored after
    /// the key is delivered to each target.
    #[default]
    FocusCycle,
}

/// Input broadcasting configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BroadcastConfig {
    /// Whether broadcasting is enabled. Default: true
    #[serde(default = "default_broadcast_enabled")]
    pub enabled: bool,
    /// List of VK codes to forward. Empty = forward all except modifiers.
    #[serde(default)]
    pub keys: Vec<u32>,
    /// Toggle key VK code. Default: 0x78 (F9)
    #[serde(default = "default_toggle_key")]
    pub toggle_key: u32,
    /// If set, the broadcast manager will auto-discover windows of
    /// processes with this name (e.g. "Gw2-64") and add them to the
    /// target list. Use this when the launched EXE is a launcher
    /// (e.g. Gw2Launcher.exe) that spawns separate game processes
    /// the multibox tool doesn't directly know about.
    #[serde(default)]
    pub target_process: Option<String>,
    /// How keys are delivered to non-active windows.
    /// "postmessage" (default) = no focus switching, preferred for swap layout.
    /// "focus_cycle" = brief focus change per target, works with all games.
    #[serde(default)]
    pub delivery_mode: DeliveryMode,
}

impl Default for BroadcastConfig {
    fn default() -> Self {
        Self {
            enabled: default_broadcast_enabled(),
            keys: vec![],
            toggle_key: default_toggle_key(),
            target_process: None,
            delivery_mode: DeliveryMode::default(),
        }
    }
}

fn default_broadcast_enabled() -> bool {
    true
}

fn default_toggle_key() -> u32 {
    0x78 // F9
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Team {
    pub name: String,
    pub slots: Vec<Slot>,
    #[serde(default)]
    pub options: TeamOptions,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub game_profiles: Vec<GameProfile>,
    pub accounts: Vec<Account>,
    pub layout: Layout,
    pub team: Team,
    /// Input broadcasting settings.
    #[serde(default)]
    pub broadcast: BroadcastConfig,
    /// Named layouts that can be loaded via the web UI or CLI.
    #[serde(default)]
    pub named_layouts: Vec<Layout>,
}

impl Config {
    /// Returns a starter config with one profile, one account, one region,
    /// and one slot. Safe to write and edit.
    pub fn template() -> Self {
        Config {
            game_profiles: vec![GameProfile {
                name: "my-game".to_string(),
                exe_path: r"C:\Games\MyGame\game.exe".to_string(),
                args: vec![],
                working_dir: None,
                window_ready_delay_ms: None,
                launcher_mode: false,
                game_process_name: None,
                kill_mutex: None,
            }],
            accounts: vec![Account {
                name: "account-1".to_string(),
                game_profile: "my-game".to_string(),
                extra_args: None,
            }],
            layout: Layout {
                name: "single-monitor".to_string(),
                regions: vec![Region {
                    name: "fullscreen".to_string(),
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                }],
            },
            team: Team {
                name: "default".to_string(),
                slots: vec![Slot {
                    index: 1,
                    account: "account-1".to_string(),
                    region: "fullscreen".to_string(),
                }],
                options: TeamOptions {
                    stagger_delay_ms: Some(3000),
                    window_timeout_ms: Some(60000),
                    hotkey_base: None,
                    layout_mode: None,
                },
            },
            broadcast: BroadcastConfig::default(),
            named_layouts: vec![],
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config {:?}", path))?;
        let cfg: Config = serde_yaml::from_str(&s)
            .with_context(|| format!("Failed to parse config {:?}", path))?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let s = serde_yaml::to_string(self).context("Failed to serialize config")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, s).with_context(|| format!("Failed to write config {:?}", path))?;
        Ok(())
    }

    pub fn default_stagger_ms(&self) -> u64 {
        self.team.options.stagger_delay_ms.unwrap_or(3000)
    }

    pub fn default_timeout_ms(&self) -> u64 {
        self.team.options.window_timeout_ms.unwrap_or(60000)
    }

    pub fn layout_mode(&self) -> LayoutMode {
        self.team.options.layout_mode.clone().unwrap_or_default()
    }
}

/// Resolved references from a validated Config.
/// All HashMaps use the source Config's lifetime.
pub struct ResolvedConfig<'a> {
    pub accounts: HashMap<&'a str, &'a Account>,
    pub profiles: HashMap<&'a str, &'a GameProfile>,
    pub regions: HashMap<&'a str, &'a Region>,
    pub slot_to_profile: HashMap<usize, &'a GameProfile>,
    pub slot_to_region: HashMap<usize, &'a Region>,
}

pub fn resolve(config: &Config) -> Result<ResolvedConfig<'_>> {
    let mut accounts: HashMap<&str, &Account> = HashMap::new();
    for a in &config.accounts {
        if accounts.insert(a.name.as_str(), a).is_some() {
            return Err(anyhow::anyhow!("Duplicate account name: {}", a.name));
        }
    }

    let mut profiles: HashMap<&str, &GameProfile> = HashMap::new();
    for p in &config.game_profiles {
        if profiles.insert(p.name.as_str(), p).is_some() {
            return Err(anyhow::anyhow!("Duplicate game profile name: {}", p.name));
        }
    }

    let mut regions: HashMap<&str, &Region> = HashMap::new();
    for r in &config.layout.regions {
        if r.width <= 0 || r.height <= 0 {
            return Err(anyhow::anyhow!(
                "Region '{}' has non-positive size ({}x{})",
                r.name,
                r.width,
                r.height
            ));
        }
        if regions.insert(r.name.as_str(), r).is_some() {
            return Err(anyhow::anyhow!("Duplicate region name: {}", r.name));
        }
    }

    let mut slot_to_profile: HashMap<usize, &GameProfile> = HashMap::new();
    let mut slot_to_region: HashMap<usize, &Region> = HashMap::new();
    let mut seen_slots: HashSet<usize> = HashSet::new();
    for slot in &config.team.slots {
        if !seen_slots.insert(slot.index) {
            return Err(anyhow::anyhow!("Duplicate slot index: {}", slot.index));
        }

        let account = accounts.get(slot.account.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "Slot {} references unknown account '{}'",
                slot.index,
                slot.account
            )
        })?;

        let profile = profiles.get(account.game_profile.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "Account '{}' references unknown game profile '{}'",
                account.name,
                account.game_profile
            )
        })?;

        let region = regions.get(slot.region.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "Slot {} references unknown region '{}'",
                slot.index,
                slot.region
            )
        })?;

        slot_to_profile.insert(slot.index, profile);
        slot_to_region.insert(slot.index, region);
    }

    Ok(ResolvedConfig {
        accounts,
        profiles,
        regions,
        slot_to_profile,
        slot_to_region,
    })
}

pub fn check_exe_paths(config: &Config) -> Vec<String> {
    let mut warnings = Vec::new();
    for profile in &config.game_profiles {
        if !PathBuf::from(&profile.exe_path).exists() {
            warnings.push(format!(
                "Game profile '{}': exe_path does not exist: {}",
                profile.name, profile.exe_path
            ));
        }
    }
    warnings
}

/// Auto-detect Guild Wars 2 install path from registry and common locations.
pub fn detect_gw2_path() -> Option<String> {
    // 1. Check registry (Steam/standalone installs)
    if let Some(path) = read_gw2_from_registry() {
        return Some(path);
    }

    // 2. Check common install locations
    let common_paths = [
        r"C:\Program Files\Guild Wars 2\Gw2-64.exe",
        r"C:\Program Files (x86)\Guild Wars 2\Gw2-64.exe",
        r"C:\Games\Guild Wars 2\Gw2-64.exe",
        r"D:\Games\Guild Wars 2\Gw2-64.exe",
        r"E:\Games\Guild Wars 2\Gw2-64.exe",
    ];
    for p in &common_paths {
        if PathBuf::from(p).exists() {
            return Some(p.to_string());
        }
    }

    None
}

fn read_gw2_from_registry() -> Option<String> {
    unsafe {
        let hklm = HKEY_LOCAL_MACHINE;
        // Try Steam registry path
        let steam_paths = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 1284210",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 1284210",
        ];
        for subkey in &steam_paths {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(hklm, to_wide(subkey).as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32 * 2;
                let mut typ = 0;
                if RegGetValueW(
                    hkey,
                    std::ptr::null(),
                    to_wide("InstallLocation").as_ptr(),
                    RRF_RT_REG_SZ,
                    &mut typ,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) == 0
                {
                    RegCloseKey(hkey);
                    let loc = String::from_utf16_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    let exe = PathBuf::from(loc).join("Gw2-64.exe");
                    if exe.exists() {
                        return Some(exe.to_string_lossy().to_string());
                    }
                }
                RegCloseKey(hkey);
            }
        }

        // Try ArenaNet/Guild Wars 2 registry
        let anet_paths = [
            r"SOFTWARE\ArenaNet\Guild Wars 2",
            r"SOFTWARE\WOW6432Node\ArenaNet\Guild Wars 2",
        ];
        for subkey in &anet_paths {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(hklm, to_wide(subkey).as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32 * 2;
                let mut typ = 0;
                if RegGetValueW(
                    hkey,
                    std::ptr::null(),
                    to_wide("InstallPath").as_ptr(),
                    RRF_RT_REG_SZ,
                    &mut typ,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) == 0
                {
                    RegCloseKey(hkey);
                    let loc = String::from_utf16_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    let exe = PathBuf::from(loc).join("Gw2-64.exe");
                    if exe.exists() {
                        return Some(exe.to_string_lossy().to_string());
                    }
                }
                RegCloseKey(hkey);
            }
        }
    }
    None
}

fn to_wide(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Generate a GW2-optimized starter config with auto-detected path and 4-account layout.
/// Uses InnerSpace-style swap layout: 1 large window on top, 3 thumbnails on bottom.
pub fn gw2_template() -> Config {
    let exe_path =
        detect_gw2_path().unwrap_or_else(|| r"C:\Games\Guild Wars 2\Gw2-64.exe".to_string());

    // Get monitor info for the placeholder region. In swap mode the
    // runtime computes real geometry from this monitor at reposition
    // time; in tiled mode users edit layout.regions by hand.
    let (mon_w, mon_h) = get_primary_monitor_size();

    Config {
        game_profiles: vec![GameProfile {
            name: "gw2".to_string(),
            exe_path,
            args: vec!["-shareArchive".to_string()],
            working_dir: None,
            window_ready_delay_ms: Some(5000),
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: Some(crate::mutex_kill::GW2_MUTEX_NAME.to_string()),
        }],
        accounts: vec![
            Account {
                name: "Account1".to_string(),
                game_profile: "gw2".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account2".to_string(),
                game_profile: "gw2".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account3".to_string(),
                game_profile: "gw2".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account4".to_string(),
                game_profile: "gw2".to_string(),
                extra_args: None,
            },
        ],
        // In swap mode the runtime computes regions from the monitor at
        // reposition time (see window::swap_layout_positions). The `layout`
        // field is still required by the schema, so we ship a single
        // placeholder region that documents the swap contract. The values
        // are ignored when team.options.layout_mode = swap; if you ever
        // switch back to tiled mode, these are the regions that will be
        // applied.
        layout: Layout {
            name: "innerspace-swap".to_string(),
            regions: vec![
                // Placeholder for swap mode. The real geometry is computed
                // live from the primary monitor; the active slot gets the
                // full width × 80% height, the remaining slots are evenly
                // sized thumbnails along the bottom 20%.
                Region {
                    name: "swap-auto".to_string(),
                    x: 0,
                    y: 0,
                    width: mon_w as i32,
                    height: mon_h as i32,
                },
            ],
        },
        team: Team {
            name: "4box".to_string(),
            slots: vec![
                Slot {
                    index: 1,
                    account: "Account1".to_string(),
                    region: "swap-auto".to_string(),
                },
                Slot {
                    index: 2,
                    account: "Account2".to_string(),
                    region: "swap-auto".to_string(),
                },
                Slot {
                    index: 3,
                    account: "Account3".to_string(),
                    region: "swap-auto".to_string(),
                },
                Slot {
                    index: 4,
                    account: "Account4".to_string(),
                    region: "swap-auto".to_string(),
                },
            ],
            options: TeamOptions {
                stagger_delay_ms: Some(3000),
                window_timeout_ms: Some(60000),
                hotkey_base: None,
                layout_mode: Some(LayoutMode::Swap),
            },
        },
        broadcast: BroadcastConfig::default(),
        named_layouts: vec![],
    }
}

fn get_primary_monitor_size() -> (u32, u32) {
    unsafe {
        let hdc = winapi::um::winuser::GetDC(std::ptr::null_mut());
        let w = winapi::um::wingdi::GetDeviceCaps(hdc, winapi::um::wingdi::HORZRES);
        let h = winapi::um::wingdi::GetDeviceCaps(hdc, winapi::um::wingdi::VERTRES);
        winapi::um::winuser::ReleaseDC(std::ptr::null_mut(), hdc);
        (w.max(1920) as u32, h.max(1080) as u32)
    }
}

/// Auto-detect World of Warcraft install path from registry.
pub fn detect_wow_path() -> Option<String> {
    unsafe {
        let hklm = HKEY_LOCAL_MACHINE;
        let paths = [
            r"SOFTWARE\Blizzard Entertainment\World of Warcraft",
            r"SOFTWARE\WOW6432Node\Blizzard Entertainment\World of Warcraft",
        ];
        for subkey in &paths {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(hklm, to_wide(subkey).as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32 * 2;
                let mut typ = 0;
                if RegGetValueW(
                    hkey,
                    std::ptr::null(),
                    to_wide("InstallPath").as_ptr(),
                    RRF_RT_REG_SZ,
                    &mut typ,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) == 0
                {
                    RegCloseKey(hkey);
                    let loc = String::from_utf16_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    let exe = PathBuf::from(loc.clone()).join(r"_retail_\WorldOf Warcraft.exe");
                    if exe.exists() {
                        return Some(exe.to_string_lossy().to_string());
                    }
                    // Try classic
                    let exe_classic = PathBuf::from(loc).join(r"_classic_\WorldOf Warcraft.exe");
                    if exe_classic.exists() {
                        return Some(exe_classic.to_string_lossy().to_string());
                    }
                }
                RegCloseKey(hkey);
            }
        }
    }
    None
}

/// Auto-detect Final Fantasy XIV install path from registry.
pub fn detect_ffxiv_path() -> Option<String> {
    unsafe {
        let hklm = HKEY_LOCAL_MACHINE;
        let paths = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 39210",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Steam App 39210",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\FFXIV",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\FFXIV",
        ];
        for subkey in &paths {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(hklm, to_wide(subkey).as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32 * 2;
                let mut typ = 0;
                if RegGetValueW(
                    hkey,
                    std::ptr::null(),
                    to_wide("InstallLocation").as_ptr(),
                    RRF_RT_REG_SZ,
                    &mut typ,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) == 0
                {
                    RegCloseKey(hkey);
                    let loc = String::from_utf16_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    let exe = PathBuf::from(loc).join(r"game\ffxivboot.exe");
                    if exe.exists() {
                        return Some(exe.to_string_lossy().to_string());
                    }
                }
                RegCloseKey(hkey);
            }
        }
    }
    None
}

/// Auto-detect EVE Online install path from registry.
pub fn detect_eve_path() -> Option<String> {
    unsafe {
        let hklm = HKEY_LOCAL_MACHINE;
        let paths = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\EVE Online",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\EVE Online",
        ];
        for subkey in &paths {
            let mut hkey: HKEY = std::ptr::null_mut();
            if RegOpenKeyExW(hklm, to_wide(subkey).as_ptr(), 0, KEY_READ, &mut hkey) == 0 {
                let mut buf = [0u16; 512];
                let mut len = buf.len() as u32 * 2;
                let mut typ = 0;
                if RegGetValueW(
                    hkey,
                    std::ptr::null(),
                    to_wide("InstallLocation").as_ptr(),
                    RRF_RT_REG_SZ,
                    &mut typ,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) == 0
                {
                    RegCloseKey(hkey);
                    let loc = String::from_utf16_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    let exe = PathBuf::from(loc).join(r"ExeFile\eve.exe");
                    if exe.exists() {
                        return Some(exe.to_string_lossy().to_string());
                    }
                }
                RegCloseKey(hkey);
            }
        }
    }
    None
}

/// Generate a WoW-optimized starter config.
pub fn wow_template() -> Config {
    let exe_path = detect_wow_path().unwrap_or_else(|| {
        r"C:\Program Files\World of Warcraft\_retail_\WorldOf Warcraft.exe".to_string()
    });

    let (mon_w, mon_h) = get_primary_monitor_size();
    let (region_w, region_h) = (mon_w / 2, mon_h / 2);

    Config {
        game_profiles: vec![GameProfile {
            name: "wow".to_string(),
            exe_path,
            args: vec![],
            working_dir: None,
            window_ready_delay_ms: Some(8000),
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: None,
        }],
        accounts: vec![
            Account {
                name: "Account1".to_string(),
                game_profile: "wow".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account2".to_string(),
                game_profile: "wow".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account3".to_string(),
                game_profile: "wow".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account4".to_string(),
                game_profile: "wow".to_string(),
                extra_args: None,
            },
        ],
        layout: Layout {
            name: "2x2-grid".to_string(),
            regions: vec![
                Region {
                    name: "tl".to_string(),
                    x: 0,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "tr".to_string(),
                    x: region_w as i32,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "bl".to_string(),
                    x: 0,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "br".to_string(),
                    x: region_w as i32,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
            ],
        },
        team: Team {
            name: "4box".to_string(),
            slots: vec![
                Slot {
                    index: 1,
                    account: "Account1".to_string(),
                    region: "tl".to_string(),
                },
                Slot {
                    index: 2,
                    account: "Account2".to_string(),
                    region: "tr".to_string(),
                },
                Slot {
                    index: 3,
                    account: "Account3".to_string(),
                    region: "bl".to_string(),
                },
                Slot {
                    index: 4,
                    account: "Account4".to_string(),
                    region: "br".to_string(),
                },
            ],
            options: TeamOptions {
                stagger_delay_ms: Some(5000),
                window_timeout_ms: Some(120000),
                hotkey_base: None,
                layout_mode: None,
            },
        },
        broadcast: BroadcastConfig::default(),

        named_layouts: vec![],
    }
}

/// Generate a FFXIV-optimized starter config.
pub fn ffxiv_template() -> Config {
    let exe_path = detect_ffxiv_path().unwrap_or_else(|| {
        r"C:\Program Files (x86)\Square Enix\FINAL FANTASY XIV\boot\ffxivboot.exe".to_string()
    });

    let (mon_w, mon_h) = get_primary_monitor_size();
    let (region_w, region_h) = (mon_w / 2, mon_h / 2);

    Config {
        game_profiles: vec![GameProfile {
            name: "ffxiv".to_string(),
            exe_path,
            args: vec![],
            working_dir: None,
            window_ready_delay_ms: Some(10000),
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: None,
        }],
        accounts: vec![
            Account {
                name: "Account1".to_string(),
                game_profile: "ffxiv".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account2".to_string(),
                game_profile: "ffxiv".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account3".to_string(),
                game_profile: "ffxiv".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account4".to_string(),
                game_profile: "ffxiv".to_string(),
                extra_args: None,
            },
        ],
        layout: Layout {
            name: "2x2-grid".to_string(),
            regions: vec![
                Region {
                    name: "tl".to_string(),
                    x: 0,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "tr".to_string(),
                    x: region_w as i32,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "bl".to_string(),
                    x: 0,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "br".to_string(),
                    x: region_w as i32,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
            ],
        },
        team: Team {
            name: "4box".to_string(),
            slots: vec![
                Slot {
                    index: 1,
                    account: "Account1".to_string(),
                    region: "tl".to_string(),
                },
                Slot {
                    index: 2,
                    account: "Account2".to_string(),
                    region: "tr".to_string(),
                },
                Slot {
                    index: 3,
                    account: "Account3".to_string(),
                    region: "bl".to_string(),
                },
                Slot {
                    index: 4,
                    account: "Account4".to_string(),
                    region: "br".to_string(),
                },
            ],
            options: TeamOptions {
                stagger_delay_ms: Some(5000),
                window_timeout_ms: Some(120000),
                hotkey_base: None,
                layout_mode: None,
            },
        },
        broadcast: BroadcastConfig::default(),

        named_layouts: vec![],
    }
}

/// Generate an EVE Online-optimized starter config.
pub fn eve_template() -> Config {
    let exe_path = detect_eve_path()
        .unwrap_or_else(|| r"C:\Program Files (x86)\CCP\EVE\ExeFile\eve.exe".to_string());

    let (mon_w, mon_h) = get_primary_monitor_size();
    let (region_w, region_h) = (mon_w / 2, mon_h / 2);

    Config {
        game_profiles: vec![GameProfile {
            name: "eve".to_string(),
            exe_path,
            args: vec![],
            working_dir: None,
            window_ready_delay_ms: Some(8000),
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: None,
        }],
        accounts: vec![
            Account {
                name: "Account1".to_string(),
                game_profile: "eve".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account2".to_string(),
                game_profile: "eve".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account3".to_string(),
                game_profile: "eve".to_string(),
                extra_args: None,
            },
            Account {
                name: "Account4".to_string(),
                game_profile: "eve".to_string(),
                extra_args: None,
            },
        ],
        layout: Layout {
            name: "2x2-grid".to_string(),
            regions: vec![
                Region {
                    name: "tl".to_string(),
                    x: 0,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "tr".to_string(),
                    x: region_w as i32,
                    y: 0,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "bl".to_string(),
                    x: 0,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
                Region {
                    name: "br".to_string(),
                    x: region_w as i32,
                    y: region_h as i32,
                    width: region_w as i32,
                    height: region_h as i32,
                },
            ],
        },
        team: Team {
            name: "4box".to_string(),
            slots: vec![
                Slot {
                    index: 1,
                    account: "Account1".to_string(),
                    region: "tl".to_string(),
                },
                Slot {
                    index: 2,
                    account: "Account2".to_string(),
                    region: "tr".to_string(),
                },
                Slot {
                    index: 3,
                    account: "Account3".to_string(),
                    region: "bl".to_string(),
                },
                Slot {
                    index: 4,
                    account: "Account4".to_string(),
                    region: "br".to_string(),
                },
            ],
            options: TeamOptions {
                stagger_delay_ms: Some(5000),
                window_timeout_ms: Some(120000),
                hotkey_base: None,
                layout_mode: None,
            },
        },
        broadcast: BroadcastConfig::default(),

        named_layouts: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config() -> Config {
        Config {
            game_profiles: vec![GameProfile {
                name: "test".to_string(),
                exe_path: "C:/test.exe".to_string(),
                args: vec!["-foo".to_string()],
                working_dir: None,
                window_ready_delay_ms: None,
                launcher_mode: false,
                game_process_name: None,
                kill_mutex: None,
            }],
            accounts: vec![Account {
                name: "a1".to_string(),
                game_profile: "test".to_string(),
                extra_args: None,
            }],
            layout: Layout {
                name: "default".to_string(),
                regions: vec![Region {
                    name: "main".to_string(),
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                }],
            },
            team: Team {
                name: "team1".to_string(),
                slots: vec![Slot {
                    index: 1,
                    account: "a1".to_string(),
                    region: "main".to_string(),
                }],
                options: TeamOptions::default(),
            },
            broadcast: BroadcastConfig::default(),
            named_layouts: vec![],
        }
    }

    #[test]
    fn resolve_minimal_ok() {
        let cfg = minimal_config();
        let r = resolve(&cfg).unwrap();
        assert_eq!(r.accounts.len(), 1);
        assert_eq!(r.profiles.len(), 1);
        assert_eq!(r.regions.len(), 1);
        assert_eq!(r.slot_to_profile.len(), 1);
        assert_eq!(r.slot_to_region.len(), 1);
    }

    #[test]
    fn resolve_duplicate_account_fails() {
        let mut cfg = minimal_config();
        cfg.accounts.push(Account {
            name: "a1".to_string(),
            game_profile: "test".to_string(),
            extra_args: None,
        });
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_duplicate_slot_fails() {
        let mut cfg = minimal_config();
        cfg.team.slots.push(Slot {
            index: 1,
            account: "a1".to_string(),
            region: "main".to_string(),
        });
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_unknown_account_fails() {
        let mut cfg = minimal_config();
        cfg.team.slots[0].account = "ghost".to_string();
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_unknown_profile_fails() {
        let mut cfg = minimal_config();
        cfg.accounts[0].game_profile = "ghost".to_string();
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_unknown_region_fails() {
        let mut cfg = minimal_config();
        cfg.team.slots[0].region = "ghost".to_string();
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_zero_width_region_fails() {
        let mut cfg = minimal_config();
        cfg.layout.regions[0].width = 0;
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn defaults() {
        let cfg = minimal_config();
        assert_eq!(cfg.default_stagger_ms(), 3000);
        assert_eq!(cfg.default_timeout_ms(), 60000);
    }

    #[test]
    fn custom_options() {
        let mut cfg = minimal_config();
        cfg.team.options = TeamOptions {
            stagger_delay_ms: Some(1500),
            window_timeout_ms: Some(20000),
            hotkey_base: None,
            layout_mode: None,
        };
        assert_eq!(cfg.default_stagger_ms(), 1500);
        assert_eq!(cfg.default_timeout_ms(), 20000);
    }

    #[test]
    fn template_resolves() {
        let cfg = Config::template();
        let r = resolve(&cfg).expect("template should resolve");
        assert_eq!(r.accounts.len(), 1);
        assert_eq!(r.profiles.len(), 1);
        assert_eq!(r.regions.len(), 1);
        assert_eq!(r.slot_to_profile.len(), 1);
        assert_eq!(r.slot_to_region.len(), 1);
        assert_eq!(cfg.default_stagger_ms(), 3000);
        assert_eq!(cfg.default_timeout_ms(), 60000);
    }

    #[test]
    fn template_roundtrips_yaml() {
        let cfg = Config::template();
        let s = serde_yaml::to_string(&cfg).expect("serialize");
        let back: Config = serde_yaml::from_str(&s).expect("deserialize");
        assert_eq!(back.game_profiles.len(), 1);
        assert_eq!(back.accounts[0].name, "account-1");
        assert_eq!(back.layout.name, "single-monitor");
        assert_eq!(back.team.slots[0].index, 1);
    }

    #[test]
    fn save_load_roundtrip() {
        let cfg = Config::template();
        let dir = std::env::temp_dir().join("multisbox_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip.yaml");
        cfg.save(&path).expect("save");
        let back = Config::load(&path).expect("load");
        assert_eq!(back.accounts.len(), 1);
        assert_eq!(back.layout.regions[0].width, 1920);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolve_negative_height_fails() {
        let mut cfg = minimal_config();
        cfg.layout.regions[0].height = -100;
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn resolve_duplicate_profile_fails() {
        let mut cfg = minimal_config();
        cfg.game_profiles.push(GameProfile {
            name: "test".to_string(),
            exe_path: "C:/other.exe".to_string(),
            args: vec![],
            working_dir: None,
            window_ready_delay_ms: None,
            launcher_mode: false,
            game_process_name: None,
            kill_mutex: None,
        });
        assert!(resolve(&cfg).is_err());
    }

    #[test]
    fn kill_mutex_field_default_is_none() {
        let cfg = minimal_config();
        assert!(cfg.game_profiles[0].kill_mutex.is_none());
    }

    #[test]
    fn kill_mutex_field_parses_when_set() {
        let mut cfg = minimal_config();
        cfg.game_profiles[0].kill_mutex = Some("FOO-MUTEX".to_string());
        let s = serde_yaml::to_string(&cfg).expect("serialize");
        let back: Config = serde_yaml::from_str(&s).expect("deserialize");
        assert_eq!(
            back.game_profiles[0].kill_mutex.as_deref(),
            Some("FOO-MUTEX")
        );
    }

    #[test]
    fn gw2_template_sets_kill_mutex() {
        let cfg = gw2_template();
        assert_eq!(cfg.game_profiles.len(), 1);
        assert_eq!(
            cfg.game_profiles[0].kill_mutex.as_deref(),
            Some(crate::mutex_kill::GW2_MUTEX_NAME)
        );
    }

    #[test]
    fn default_hotkey_base_skips_f1_through_f5() {
        // F1=0x70, F5=0x74, F6=0x75. Default must be 0x75 or higher
        // so we don't steal GW2's in-game F1-F5 hotkeys.
        let base = default_hotkey_base();
        assert!(
            base >= 0x75,
            "default hotkey base 0x{:X} is below F6 (0x75); would collide with game hotkeys",
            base
        );
    }

    #[test]
    fn broadcast_default_is_enabled() {
        // Broadcasting is opt-out (on by default) so the tool is
        // useful out of the box; users can disable it in their YAML.
        let cfg = BroadcastConfig::default();
        assert!(cfg.enabled);
    }

    #[test]
    fn layout_mode_default_is_tiled() {
        let cfg = minimal_config();
        assert_eq!(cfg.layout_mode(), LayoutMode::Tiled);
    }

    #[test]
    fn layout_mode_swap_parses() {
        let yaml = r#"
game_profiles:
  - name: test
    exe_path: C:/test.exe
accounts:
  - name: a1
    game_profile: test
layout:
  name: default
  regions:
    - name: main
      x: 0
      y: 0
      width: 1920
      height: 1080
team:
  name: team1
  slots:
    - index: 1
      account: a1
      region: main
  options:
    layout_mode: swap
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.layout_mode(), LayoutMode::Swap);
    }

    #[test]
    fn delivery_mode_default_is_focus_cycle() {
        // Focus cycling is the default because it's the ONLY method that
        // works with DirectInput/Raw Input games (GW2, etc.). PostMessage
        // does not deliver keys to these games.
        let cfg = BroadcastConfig::default();
        assert_eq!(cfg.delivery_mode, DeliveryMode::FocusCycle);
    }

    #[test]
    fn delivery_mode_postmessage_parses() {
        let yaml = r#"
game_profiles:
  - name: test
    exe_path: C:/test.exe
accounts:
  - name: a1
    game_profile: test
layout:
  name: default
  regions:
    - name: main
      x: 0
      y: 0
      width: 1920
      height: 1080
team:
  name: team1
  slots:
    - index: 1
      account: a1
      region: main
broadcast:
  delivery_mode: post_message
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.broadcast.delivery_mode, DeliveryMode::PostMessage);
    }

    #[test]
    fn delivery_mode_focus_cycle_parses() {
        let yaml = r#"
game_profiles:
  - name: test
    exe_path: C:/test.exe
accounts:
  - name: a1
    game_profile: test
layout:
  name: default
  regions:
    - name: main
      x: 0
      y: 0
      width: 1920
      height: 1080
team:
  name: team1
  slots:
    - index: 1
      account: a1
      region: main
broadcast:
  delivery_mode: focus_cycle
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.broadcast.delivery_mode, DeliveryMode::FocusCycle);
    }

    #[test]
    fn gw2_template_uses_swap_layout() {
        let cfg = gw2_template();
        assert_eq!(cfg.layout_mode(), LayoutMode::Swap);
        // The template ships a single placeholder region in swap mode;
        // the runtime computes real geometry from the primary monitor.
        // This is a regression test for the design where the template
        // accidentally shipped 4 hard-coded regions that were ignored
        // by the swap layout at runtime (see CHANGELOG / PR notes).
        assert_eq!(cfg.layout.regions.len(), 1);
        assert_eq!(cfg.layout.regions[0].name, "swap-auto");
        assert_eq!(cfg.team.slots.len(), 4);
    }
}
