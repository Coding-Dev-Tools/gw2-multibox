# Changelog

## [Unreleased]

### Added
- Input Broadcasting (keyboard only, disabled by default, F9 toggle)
- System tray icon with context menu
- Windows installer (NSIS)
- Multi-monitor improvements (per-monitor DPI awareness)
- Web UI enhancements (layout preview, saved layouts, first-run wizard)
- Multi-game presets (WoW, FFXIV, EVE Online) with registry auto-detection
- Named layouts support in config
- `kill_mutex` per-profile field (process-external handle closure, no DLL injection)
- `launcher_mode` and `game_process_name` for orchestrating an external launcher
- Pre-opened `Gw2.dat` shared file lock (CreateFileW with FILE_SHARE_*|DELETE)
- `AGENTS.md` describing architecture, ToS constraints, and contribution rules

### Fixed
- Compilation errors in multi-game preset path strings (raw strings)
- `launcher.rs` was passing the full command line as the executable parameter
  to `CreateProcessW`; now correctly passes `exe_path` as the application
  and quotes the exe path in the command line so paths with spaces work
- Working tree: `.gitignore` now covers `target/`, vendored C# source, build
  artifacts, and personal-environment scripts (608 untracked items → 0)

## [0.1.0] - Initial Release

### Added
- Initial release
