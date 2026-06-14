//! Multisbox — A multiboxing launcher and window manager for Windows.
//!
//! This crate exposes the core building blocks:
//! - [`config`] — YAML config schema, parsing, and validation
//! - [`launcher`] — process launching with staggered delays
//! - [`window`] — window discovery, positioning, and monitor detection
//! - [`hotkey`] — global hotkey registration and the Win32 message loop
//! - [`broadcast`] — input broadcasting via low-level keyboard hook
//! - [`tray`] — system tray icon and context menu
//! - [`log`] — file-based structured logging
//! - [`http`] — embedded web server for the config UI
//!
//! The binary entry point in `main.rs` is a thin CLI wrapper around these.

pub mod broadcast;
pub mod config;
pub mod file_lock;
pub mod hotkey;
pub mod http;
pub mod launcher;
pub mod launcher_inject;
pub mod log;
pub mod mutex_kill;
pub mod tray;
pub mod window;

pub use config::{
    Account, BroadcastConfig, Config, GameProfile, Layout, Region, Slot, Team, TeamOptions,
    detect_gw2_path, gw2_template,
};
pub use mutex_kill::{GW2_MUTEX_NAME, KillResult, kill_gw2_mutex, kill_mutex_in_process};
