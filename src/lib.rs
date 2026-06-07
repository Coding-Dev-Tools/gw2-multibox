//! Multisbox — A multiboxing launcher and window manager for Windows.
//!
//! This crate exposes the core building blocks:
//! - [`config`] — YAML config schema, parsing, and validation
//! - [`launcher`] — process launching with staggered delays
//! - [`window`] — window discovery, positioning, and monitor detection
//! - [`hotkey`] — global hotkey registration and the Win32 message loop
//! - [`log`] — file-based structured logging
//! - [`http`] — embedded web server for the config UI
//!
//! The binary entry point in `main.rs` is a thin CLI wrapper around these.

pub mod config;
pub mod hotkey;
pub mod http;
pub mod launcher;
pub mod log;
pub mod window;

pub use config::{Account, Config, GameProfile, Layout, Region, Slot, Team, TeamOptions};
