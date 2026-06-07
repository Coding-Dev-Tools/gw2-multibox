# Multisbox

A Windows-only multiboxing launcher and window manager. Launches multiple
game instances, positions their windows into a layout you configure, and lets
you switch between them with hotkeys.

> Compatible with ISBoxer/Inner Space — use whichever tool you prefer per
> session.

## Features

- ✅ YAML config for game profiles, accounts, layout, and team
- ✅ Multi-game support (one config can include multiple game profiles)
- ✅ Multi-monitor support with absolute pixel coordinates
- ✅ Global hotkeys (F1–F4) to switch between windows
- ✅ Staggered launches with configurable delay
- ✅ Web-based config editor (no YAML editing required)
- ✅ File-based structured logging
- ✅ Validation mode (`--dry-run`) catches errors before launch
- ✅ Window enumeration (`--list-windows`) for debugging
- ✅ Unit tests for config parsing and validation

## Not in this release

- ❌ Input broadcasting (Phase 2)
- ❌ Mapped keys / macros (Phase 3)
- ❌ In-game overlay UI (Phase 4)
- ❌ System tray icon (Phase 2)
- ❌ Windows installer (Phase 2)
- ❌ Auto-update (Phase 2)

## Build

```bash
cargo build --release
```

Output: `target/release/multisbox.exe` (single file, no installer required)

## Usage

```bash
# Run with default config
multisbox

# Run with specific config
multisbox --config my-team.yaml

# Validate config without launching
multisbox --dry-run

# Enumerate visible windows (debug)
multisbox --list-windows

# Start the web config editor
multisbox --ui
# Then open http://127.0.0.1:7878
```

See `docs/user-guide.md` for the full guide.

## Examples

The `examples/` directory contains ready-to-use configs:

| File | Game | Accounts |
|------|------|----------|
| `config.yaml` | Guild Wars 2 | 4 |
| `config-wow.yaml` | World of Warcraft | 3 |
| `config-ffxiv.yaml` | Final Fantasy XIV | 2 |
| `config-test.yaml` | Notepad (smoke test) | 4 |

## Documentation

- `docs/user-guide.md` — for end users
- `docs/config-reference.md` — full YAML field reference
- `docs/architecture.md` — for contributors

## Project structure

```
gw2-multibox/
├── src/
│   ├── lib.rs              library root
│   ├── main.rs             thin CLI
│   ├── config.rs           YAML schema + validation
│   ├── launcher.rs         Win32 process launching
│   ├── window.rs           window discovery + positioning
│   ├── hotkey.rs           global hotkeys + message loop
│   ├── log.rs              file logging
│   ├── http.rs             embedded HTTP server
│   └── ui/static/          embedded web UI assets
├── examples/               sample configs for several games
├── docs/                   user guide, config reference, architecture
├── Cargo.toml
├── config.yaml             default 4-account GW2 setup
└── config-test.yaml        notepad test rig
```

## Safety

Multisbox is a process launcher and window manager. It does not:

- Modify game process memory
- Intercept or send network traffic
- Automate gameplay
- Implement input broadcasting (Phase 2+)

The tool launches the executable you point it at and arranges its window.
Anything beyond that is your responsibility and may violate your game's ToS.

## Dependencies

- `serde` + `serde_yaml` — config parsing
- `serde_json` — web UI API
- `anyhow` — error handling
- `winapi` — Win32 FFI bindings

Zero telemetry. Zero network calls. The HTTP server is bound to
`127.0.0.1` (localhost only) and only serves the embedded UI assets.

## License

TBD
