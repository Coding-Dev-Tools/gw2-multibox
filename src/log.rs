//! Lightweight file-based logging.
//!
//! Designed to have zero external deps (no log4rs, no env_logger).
//! Writes to `%APPDATA%\Multisbox\multisbox.log` by default.
//!
//! Use [`init`] once at startup. After that, use [`info`], [`warn`], [`error`],
//! and [`debug`] to log messages. The log file is line-flushed so users can
//! tail it during a session.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
static LOG_LEVEL: Mutex<Level> = Mutex::new(Level::Info);

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Copy, Clone)]
pub enum Level {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl Level {
    fn as_str(&self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

pub fn log_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Multisbox").join("multisbox.log")
}

pub fn init() {
    let path = log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    if let Some(f) = file {
        *LOG_FILE.lock().unwrap() = Some(f);
    }
    info(&format!("Logging initialized: {}", path.display()));
}

pub fn set_level(level: Level) {
    *LOG_LEVEL.lock().unwrap() = level;
}

pub fn debug(msg: &str) {
    write(Level::Debug, msg);
}

pub fn info(msg: &str) {
    write(Level::Info, msg);
}

pub fn warn(msg: &str) {
    write(Level::Warn, msg);
}

pub fn error(msg: &str) {
    write(Level::Error, msg);
}

fn write(level: Level, msg: &str) {
    if level < *LOG_LEVEL.lock().unwrap() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[{}] [{}] {}\n", now, level.as_str(), msg);
    if let Ok(mut guard) = LOG_FILE.lock()
        && let Some(f) = guard.as_mut()
    {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
    // Also print to stderr for console mode
    if level >= Level::Warn {
        eprint!("{}", line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ordering() {
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warn);
        assert!(Level::Warn < Level::Error);
    }

    #[test]
    fn log_path_under_appdata() {
        let p = log_path();
        assert!(p.to_string_lossy().contains("Multisbox"));
        assert!(p.to_string_lossy().ends_with("multisbox.log"));
    }
}
