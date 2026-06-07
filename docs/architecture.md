# Multisbox Architecture

This document describes the internal architecture of multisbox for contributors.

## High-level shape

```
src/
├── lib.rs              # library root, module declarations, re-exports
├── main.rs             # thin CLI wrapper around the library
├── config.rs           # config schema, parsing, validation, save/load
├── launcher.rs         # Win32 CreateProcessW wrapper
├── window.rs           # window discovery, positioning, monitor enumeration
├── hotkey.rs           # global hotkey registration, message loop
├── log.rs              # file-based logging
├── http.rs             # embedded HTTP server (no external crate)
└── ui/static/          # embedded web UI (HTML, JS, CSS)
    ├── index.html      # include_str!'d into the binary
    ├── app.js          # vanilla JS config editor
    └── style.css       # dark theme
```

The split between `lib.rs` and `main.rs` is intentional. The library contains
all the logic; the binary is a thin CLI that parses args and dispatches to
library functions. This makes the code testable (unit tests in `lib.rs`) and
reusable (a future Tauri/Egui GUI would import the same library).

## Module responsibilities

### config

Pure data + validation. No Win32 calls, no I/O except file read/write.

- Schema types: `Config`, `GameProfile`, `Account`, `Region`, `Layout`, `Slot`,
  `Team`, `TeamOptions`
- `Config::load(path)` — read YAML from disk
- `Config::save(path)` — write YAML to disk
- `resolve(&Config) -> Result<ResolvedConfig>` — validate references and build
  the lookup tables used at runtime
- `check_exe_paths(&Config) -> Vec<String>` — non-fatal warnings for missing
  exes (useful for cross-machine config testing)

### launcher

Wraps Win32 `CreateProcessW`. Single function: `launch(profile, extra_args) -> Result<DWORD>`.

The `to_wide` helper converts Rust strings to UTF-16 with null terminator,
which is what Win32 APIs expect.

### window

Window enumeration and manipulation.

- `list_all_visible()` — `EnumWindows` callback to collect all visible windows
- `find_by_pid(pid)` — filter by PID
- `find_primary_by_pid(pid)` — first match (for a single-windowed game)
- `apply_region(hwnd, &Region)` — `SetWindowPos` to move+resize
- `activate(hwnd)` — `SetForegroundWindow`
- `list_monitors()` — `EnumDisplayMonitors` for multi-monitor coordinate info

### hotkey

Global hotkeys via `RegisterHotKey` + a Win32 message loop.

- `HotkeyManager` — RAII wrapper. Auto-unregisters on drop.
- `run_loop<F: FnMut(usize)>(slot_count, on_hotkey)` — runs `GetMessageW` and
  dispatches `WM_HOTKEY` to the callback.

### log

Zero-dep logging. Writes to `%APPDATA%\Multisbox\multisbox.log`. Mirrors warn
and error to stderr. Single global `Mutex<File>` for the log handle.

### http

Embedded HTTP server. Pure `std::net::TcpListener` + manual HTTP parsing. No
external crate to avoid transitive `windows-sys` deps (which need `dlltool`).

Routes:
- `GET /` — serve `index.html`
- `GET /app.js` — serve `app.js`
- `GET /style.css` — serve `style.css`
- `GET /api/config` — return current config as JSON
- `POST /api/config` — save new config (runs validation server-side)
- `GET /api/status` — version + health check

The web UI assets are `include_str!`'d at compile time, so the binary is
self-contained — no separate files to ship.

## Win32 dependency strategy

The `winapi` crate (v0.3.9) is used for Win32 bindings. It pulls in
`winapi-x86_64-pc-windows-gnu` which provides pre-built import libraries for
the GNU toolchain — no `dlltool` needed. The MSVC toolchain would also work
but the import libraries are not bundled by default.

**We do NOT use `windows` or `windows-sys`** because they require `dlltool.exe`
to generate import libraries, which is not installed on this build environment.

**We do NOT use `clap`, `ctrlc`, or `anyhow`-with-backtrace** because they
transitively depend on `windows-sys` and trigger the same `dlltool` failure.

## Testing

`cargo test` runs the unit tests in `config.rs` and `log.rs`. These cover:

- Resolving a minimal valid config
- Detecting duplicate account/profile/region/slot names
- Detecting dangling references (unknown account, profile, region)
- Detecting invalid region dimensions
- Default vs. custom team options
- Log level ordering
- Log path construction

## What's NOT in Phase 1

- No input broadcasting (that's Phase 2)
- No mapped keys / macros (Phase 3)
- No overlay UI (Phase 4)
- No auto-update mechanism
- No tray icon (planned for Phase 2)
- No system installer (planned for Phase 2)
- No code signing (planned for distribution)

## Future: multi-game scheduling

The current design supports multiple games via separate config files. A future
enhancement could add a top-level `schedule` that runs different configs at
different times (e.g., "WoW on weekdays, GW2 on weekends"). Not in scope for
Phase 1.
