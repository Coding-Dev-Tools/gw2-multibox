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
                },
            },
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
pub fn gw2_template() -> Config {
    let exe_path =
        detect_gw2_path().unwrap_or_else(|| r"C:\Games\Guild Wars 2\Gw2-64.exe".to_string());

    // Get monitor info for smart layout
    let (mon_w, mon_h) = get_primary_monitor_size();
    let (region_w, region_h) = (mon_w / 2, mon_h / 2);

    Config {
        game_profiles: vec![GameProfile {
            name: "gw2".to_string(),
            exe_path,
            args: vec!["-shareArchive".to_string()],
            working_dir: None,
            window_ready_delay_ms: Some(5000),
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
                stagger_delay_ms: Some(3000),
                window_timeout_ms: Some(60000),
            },
        },
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
        });
        assert!(resolve(&cfg).is_err());
    }
}
