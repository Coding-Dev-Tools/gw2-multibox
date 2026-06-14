# AI Developer Handoff — gw2-multibox (Multisbox)

## Project Overview

**Multisbox** — Windows-only multiboxing launcher and window manager. Launches multiple game instances, positions their windows into a configured layout, and lets you switch between them with hotkeys (F1–F4). Phase 1 complete; Phase 2 (input broadcasting) pending.

**Repo:** `C:/Users/home/OneDrive/Documents/GitHub/gw2-multibox`  
**GitHub:** https://github.com/Coding-Dev-Tools/gw2-multibox  
**Binary:** `target/release/gw2-multibox.exe` (single file, no installer)

---

## Architecture

```
src/
├── lib.rs           # Library root, re-exports
├── main.rs          # Thin CLI (~400 lines)
├── config.rs        # YAML schema, validation, auto-detection (GW2 registry), templates
├── launcher.rs      # CreateProcessW wrapper
├── window.rs        # EnumWindows, SetWindowPos, monitor enum, rect helpers
├── hotkey.rs        # RegisterHotKey + GetMessageW loop, HotkeyManager (RAII)
├── log.rs           # File logging to %APPDATA%\Multisbox\multisbox.log
├── http.rs          # Embedded HTTP server (raw std::net), /api/wizard/create
└── ui/static/
    ├── index.html   # 5-tab editor + first-run wizard modal
    ├── app.js       # Vanilla JS, wizard logic, form→JSON sync
    └── style.css    # Dark theme
```

**Dependencies:** `anyhow`, `serde`/`serde_yaml`/`serde_json`, `winapi` (v0.3.9, GNU prebuilt libs — no `dlltool.exe` needed)

---

## Key Features (Done)

| Feature | Command / Access |
|---------|------------------|
| One-click GW2 setup | `gw2-init -c config.yaml` (auto-detects install, 4 accounts, 2×2 grid) |
| Validate before launch | `--dry-run` |
| Launch N instances | (default mode) |
| Web config editor | `--ui` → http://127.0.0.1:7878 |
| First-run wizard | Auto-shown in web UI if config empty |
| Switch windows | F1–F4 hotkeys |
| Structured logging | `%APPDATA%\Multisbox\multisbox.log` |
| ISBoxer layout replication | `config-isboxer.yaml` (scaled from 3440×1440 → 2752×1152) |

---

## How to Build & Test

```powershell
cd C:/Users/home/OneDrive/Documents/GitHub/gw2-multibox

# Build release
cargo build --release

# Run tests (16 passing)
cargo test

# Lint (must pass)
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# Quick GW2 setup
.\target\release\gw2-multibox.exe gw2-init -c my-config.yaml
.\target\release\gw2-multibox.exe --dry-run -c my-config.yaml
.\target\release\gw2-multibox.exe -c my-config.yaml
```

**Notepad smoke test** (no GW2 needed):
```powershell
.\target\release\gw2-multibox.exe -c config-test.yaml
```

---

## Config Schema (Key Types)

```yaml
game_profiles:
  - name: gw2
    exe_path: "C:/Games/GW2/Gw2-64.exe"
    args: ["-shareArchive"]
    window_ready_delay_ms: 5000

accounts:
  - name: Account1
    game_profile: gw2

layout:
  name: "2x2-grid"
  regions:
    - name: tl
      x: 0, y: 0, width: 1376, height: 576

team:
  name: "4box"
  slots:
    - index: 1, account: Account1, region: tl
  options:
    stagger_delay_ms: 3000
    window_timeout_ms: 60000
```

**Validation:** `config::resolve()` catches duplicates, dangling refs, non-positive regions, missing exe paths.

---

## Priority Improvements (Phase 2+)

### 1. Input Broadcasting (Core)
- Low-level keyboard hook (`SetWindowsHookEx` + `WH_KEYBOARD_LL`)
- Map keys per-slot (F1–F4 → target window)
- Optional: mouse click broadcasting
- **Constraints:** No game memory modification, no network interception

### 2. System Tray Icon
- Minimize to tray, show status (running/stopped)
- Right-click menu: Show UI, Reload Config, Quit
- Crate: `tray-icon` (check `dlltool` compatibility)

### 3. Windows Installer
- NSIS or WiX installer
- Code signing (EV cert)
- Auto-update via GitHub Releases

### 4. Multi-Monitor Improvements
- Per-monitor DPI awareness
- Drag-and-drop layout designer in web UI
- Save/load named layouts

### 5. Web UI Enhancements
- Real-time window preview (screenshot via `PrintWindow`)
- Drag-to-resize regions
- Account login helper (launch with `-email`/`-password` args)

### 6. Multi-Game Presets
- Add more `gw2_template()`-style presets (WoW, FFXIV, EVE, etc.)
- Auto-detect via registry for each

---

## Known Issues / Gotchas

| Issue | Workaround |
|-------|------------|
| `dlltool.exe` missing on GNU toolchain | Use `winapi` v0.3.9 (prebuilt) — **do not add `windows`/`windows-sys` crates** |
| OAuth token lacks `workflow` scope | Push workflows via GitHub Desktop, not CLI |
| GW2 window appears ~5s after launch | `window_ready_delay_ms: 5000` in config |
| `-shareArchive` required for multi-instance | Enforced in `gw2_template()` |
| GW2 `ANET-WIN32-MUTEX` blocks 2nd instance | Auto-killed by `src/mutex_kill.rs` (set `kill_mutex: ANET-WIN32-MUTEX` in profile — the GW2 template does this by default) |
| High-DPI scaling | Set `dpiAwareness` in manifest or `SetProcessDpiAwarenessContext` |

### Mutex bypass technique

`src/mutex_kill.rs` is a direct port of the technique from `Healix/Gw2Launcher`'s `ProcessUtil` (see https://github.com/Healix/Gw2Launcher — `ProcessUtil/Program.cs` and `Win32.cs`). It uses no DLL injection. After `CreateProcessW` returns a PID:

1. `NtQuerySystemInformation(SystemExtendedHandleInformation)` enumerates all system handles
2. Entries are filtered to the target PID and each handle's name is read with `NtQueryObject(ObjectNameInformation)`
3. On a name match, `DuplicateHandle(... DUPLICATE_CLOSE_SOURCE)` closes the mutex in the target process so the game no longer owns it
4. A second GW2 instance can now be launched without the "ANET is already running" error

Opt out per-profile with `kill_mutex: null` (or omit the key). Search budget is 5s. On miss/error the launcher logs a warning and continues — never aborts.

---

## Testing Checklist

```powershell
# 1. All tests pass
cargo test

# 2. Lint clean
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# 3. GW2 config generates & validates
.\target\release\gw2-multibox.exe gw2-init -c test.yaml
.\target\release\gw2-multibox.exe --dry-run -c test.yaml

# 3. Notepad smoke test launches 4 windows
.\target\release\gw2-multibox.exe -c config-test.yaml

# 4. Web UI loads
.\target\release\gw2-multibox.exe --ui
# Open http://127.0.0.1:7878
```

---

## Release Process

1. Update `Cargo.toml` version
2. `git tag v0.X.Y`
3. `git push origin main --tags`
4. GitHub Action `release.yml` builds binary + SHA256SUMS
5. Verify release at https://github.com/Coding-Dev-Tools/gw2-multibox/releases

---

## File Locations

| File | Purpose |
|------|---------|
| `config-isboxer.yaml` | ISBoxer "4boxnew" layout replicated for 2752×1152 |
| `config-test.yaml` | Notepad smoke test |
| `config.yaml` | User's GW2 config (gitignored) |
| `docs/user-guide.md` | End-user guide |
| `docs/config-reference.md` | Full YAML field reference |
| `docs/architecture.md` | Contributor architecture doc |
| `CONTRIBUTING.md` | Dev setup, style, PR process |

---

## Contact / Context

- **Principal:** Coding-Dev-Tools (algorithmictradingsolutions@gmail.com)
- **Goal:** Platform usable by thousands for multiple games
- **Constraints:** No game memory modification, no network interception, no combat automation
- **Compatible with:** ISBoxer/Inner Space (use either per session)

---

**Start here:** Run `.\target\release\gw2-multibox.exe gw2-init -c my.yaml` → edit if needed → `--dry-run` → launch. The web UI (`--ui`) is the primary interface for non-technical users.

### Live-test gotchas (discovered during multi-account testing, 2026-06-14)

- **GW2 exits without a logged-in account.** `Gw2-64.exe` is a non-interactive launcher; the actual game requires a login session driven by ISBoxer / InnerSpace. So a bare `multisbox -c config-test.yaml` will launch 4 `Gw2-64.exe` processes but they exit after a few seconds because no account is logged in. To test the full launch-and-position pipeline without a real game, use a notepad config: 4 notepad windows, 100ms stagger, 15s window timeout. Verified: all 4 windows positioned, 4 hotkeys (F1–F4) registered, tray initialized.
- **Memory-safety bug found and fixed** in `src/mutex_kill.rs:query_object_name()`. The original Healix port was reading `UNICODE_STRING.Buffer` (a pointer in the target process's address space) directly from our own buffer — a use-after-free / cross-process-memory-read bug. When the target GW2 process exits between `OpenProcess` and `DuplicateHandle`, the read segfaults. Fix: do a second `NtQueryObject` call with a buffer sized for header + name data so the API writes both into our own buffer.
- **DLL injection is disabled by default** — `main.rs:580` has `if false {` gating `launcher_inject::launch_with_inject`. The user found the bypass DLL "interferes with GW2 startup". The mutex-kill path is the alternative. To re-enable injection, change `if false` to `if true`.