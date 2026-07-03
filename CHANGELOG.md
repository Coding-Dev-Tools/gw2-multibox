# Changelog

## [Unreleased]

### Added
- Input Broadcasting (keyboard only, ENABLED by default, F9 toggle)
- `team.options.hotkey_base` config field (default 0x75 = F6). The
  default skips F1-F5 so they remain available for the game's own
  in-game hotkeys. Set to a lower value to use F1-F5 if your game
  doesn't reserve them.
- Broadcast toggle hotkey (F9 by default) is now actually registered
  and toggles broadcasting on/off from the message loop
- `junction` module: per-account Windows directory junctions via `mklink /J`
- Sequential slot launch: wait for each slot's window to appear before
  launching the next (improves archive-file compatibility for multi-instance)
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
- `config::resolve()` now rejects a team with zero slots instead of
  succeeding silently; such a config would launch nothing (silent no-op).
- Corrected the `broadcast.delivery_mode` doc comment: the actual default
  is `focus_cycle` (required for DirectInput/Raw Input games like GW2),
  not `postmessage` as the comment previously stated.
- Compilation errors in multi-game preset path strings (raw strings)
- `launcher.rs` was passing the full command line as the executable parameter
  to `CreateProcessW`; now correctly passes `exe_path` as the application
  and quotes the exe path in the command line so paths with spaces work
- Working tree: `.gitignore` now covers `target/`, vendored C# source, build
  artifacts, and personal-environment scripts (608 untracked items → 0)

## [0.1.0] - Initial Release

### Added
- Initial release
