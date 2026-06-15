# gw2-multibox — Agent Operating Manual

## What this is

`gw2-multibox` is a **Windows-only multiboxing launcher and window manager**
written in Rust (edition 2024, Rust 1.85+). It launches multiple instances of
a game (Guild Wars 2 by default; also WoW, FFXIV, EVE), positions the
windows into a layout from a YAML config, and lets you switch between them
with global hotkeys (F1-Fn).

It is **not** an automation, input-broadcasting, or memory-modification tool
in the default configuration. The mutex-kill path is a process-external
handle-closure technique (port of Healix/Gw2Launcher's `ProcessUtil`) that
does not inject a DLL into the target game.

## Critical constraints

### 1. Guild Wars 2 Terms of Service

This project is intended to operate **within** the GW2 Terms of Service
when used with the **canonical** configuration. Specifically:

- The `kill_mutex` field in `game_profiles[].kill_mutex` (default
  `ANET-WIN32-MUTEX`) is what allows multiple GW2 clients to coexist. This
  is a process-external handle closure, not a memory edit. It mirrors the
  technique used by the upstream `Healix/Gw2Launcher` tool
  (<https://github.com/Healix/Gw2Launcher>) and is **not** a DLL injection.
- Input broadcasting (`broadcast.enabled: true`) and the F9 broadcast toggle
  are opt-in and disabled by default. Per the README, enabling broadcasting
  is the user's responsibility with respect to their game's ToS.
- The README explicitly states: *"It does not modify game process memory,
  intercept or send network traffic, or automate gameplay."*

**Do not** add features that:
- Inject DLLs into the game process at runtime
- Read or write game process memory
- Intercept or replay network traffic to/from the game
- Automate gameplay (input timing, anti-idle, route following)
- Bypass Warden anti-cheat signatures

The `tools/multisbox-bypass/` crate is a **superseded stub** retained as
historical reference. It was an IAT-hooking DLL approach. The approach in
production today is `src/mutex_kill.rs` (process-external
`DuplicateHandle(... DUPLICATE_CLOSE_SOURCE)`). **Do not** reintroduce
`launcher_inject` as an active path — the `if false { ... }` branch in
`src/main.rs:585` exists to keep the code out of the build while
documenting why the technique was abandoned.

### 2. Windows-only

This codebase uses `winapi` and direct FFI to `ntdll`, `kernel32`, `user32`,
`shellapi`, `winscard`, and `msi`. The whole tree is gated on `#![cfg(windows)]`
where appropriate and will not compile on macOS/Linux. **Do not** add
non-Windows dependencies or platform-specific code paths.

## Repository layout

```
gw2-multibox/
├── src/
│   ├── lib.rs              # library root, module wiring, re-exports
│   ├── main.rs             # thin CLI (~800 lines, single binary)
│   ├── config.rs           # YAML schema, validation, templates, auto-detect
│   ├── launcher.rs         # Win32 CreateProcessW wrapper
│   ├── window.rs           # EnumWindows, SetWindowPos, monitor enum
│   ├── hotkey.rs           # RegisterHotKey + GetMessageW loop
│   ├── broadcast.rs        # SetWindowsHookEx WH_KEYBOARD_LL (opt-in)
│   ├── tray.rs             # Shell_NotifyIconW + hidden window for events
│   ├── log.rs              # file logging to %APPDATA%\Multisbox\
│   ├── http.rs             # embedded HTTP server for the web UI
│   ├── file_lock.rs        # CreateFileW with FILE_SHARE_*|DELETE
│   ├── junction.rs         # mklink /J wrapper for per-account appdata
│   ├── mutex_kill.rs       # NtQuerySystemInformation + DuplicateHandle
│   ├── launcher_inject.rs  # DISABLED — see Critical Constraints above
│   └── ui/static/          # embedded web UI assets (HTML/JS/CSS)
├── examples/               # sample configs (gw2, wow, ffxiv, notepad)
├── docs/                   # user-guide.md, config-reference.md, architecture.md
├── installer/              # NSIS script for the Windows installer
├── tools/                  # supporting tools (see below)
├── .github/workflows/      # CI and release workflows
├── Cargo.toml
└── AGENTS.md               # this file
```

### `tools/`

- `tools/Gw2LauncherStarter/` — small C# helper that launches the GW2 launcher
  in STA mode (fixes a DragDrop registration bug when launching from
  non-STA contexts like InnerSpace). Compiles independently.
- `tools/gw2launcher-src/` — vendored copy of the upstream
  `Healix/Gw2Launcher` C# source (`fc530bb` commit). **Gitignored**; not
  built in-repo. Kept locally for reference. The zip source is also ignored.

The `tools/gw2launcher-src/`, all `*.ps1`, all `*.bat`, all build artifacts
(`target/`, `*.exe`, `*.dll`, `*.pdb`), and the source archive are
**gitignored** to keep the working tree clean.

## Build, test, lint

```bash
# Build (Windows, MSVC)
cargo build --release

# Run all unit tests (config, mutex_kill, broadcast, window, hotkey, log)
cargo test --lib

# Lint (must pass for CI)
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

CI runs on Windows-latest with stable-x86_64-pc-windows-msvc.

## How to make changes

### Adding a config field

1. Add the field to the struct in `src/config.rs` with `#[serde(default)]`.
2. Add a test case in the `mod tests` block at the bottom of `config.rs`
   that round-trips the field through YAML.
3. Update `docs/config-reference.md` (if it exists) and `docs/user-guide.md`.
4. Bump the `## [Unreleased]` section in `CHANGELOG.md`.

### Adding a new game template (e.g. NewGame)

1. Add a `detect_newgame_path()` function in `src/config.rs` that reads the
   Windows registry and falls back to common install locations.
2. Add a `newgame_template()` function returning a starter `Config`.
3. Wire it into the `Mode` enum in `src/main.rs` and the `print_help()` text.
4. Wire it into the `gw2_template`/`wow_template`/`ffxiv_template`/`eve_template`
   pattern in `src/http.rs` (the wizard endpoint).
5. Add a sample config to `examples/`.

### Modifying the mutex-kill technique

`src/mutex_kill.rs` is the only place where the in-process handle table is
walked. The `wide_ends_with_ignore_ascii_case` and `query_object_name` helpers
encode the Microsoft documented layout of the
`SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX` structure. If you change the FFI, run
the existing tests (`cargo test --lib mutex_kill`) and add a new test for
the new behavior.

## Don't

- Don't reintroduce the `launcher_inject` path as active. The `if false`
  branch in `main.rs` documents why the technique was abandoned (AV
  heuristics on `CREATE_SUSPENDED`+`LoadLibrary`).
- Don't add `unsafe` blocks outside of explicit FFI bindings. Every `unsafe`
  in the codebase has a `# Safety` doc comment.
- Don't commit `target/`, `Cargo.lock` changes that don't correspond to a
  dependency change, vendored C# source, or binary build outputs.
- Don't bump the `version` in `Cargo.toml` without updating `CHANGELOG.md`.
- Don't merge without `cargo test --lib` and `cargo clippy --all-targets
  -- -D warnings` passing.
