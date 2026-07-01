# Multisbox

A Windows-only multiboxing launcher and window manager. Launches multiple
game instances, positions their windows into a layout you configure, and lets
you switch between them with hotkeys.

> Compatible with ISBoxer/Inner Space — use whichever tool you prefer per
> session.

[![CI](https://github.com/Coding-Dev-Tools/gw2-multibox/actions/workflows/ci.yml/badge.svg)](https://github.com/Coding-Dev-Tools/gw2-multibox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)

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
- ✅ Bypasses GW2 instance mutex for true multi-account play
- ✅ 19 unit tests for config parsing and validation
- ✅ Single-file binary — no installer, no runtime dependencies

## Not in this release

- ❌ Input broadcasting (Phase 2)
- ❌ Mapped keys / macros (Phase 3)
- ❌ In-game overlay UI (Phase 4)
- ❌ System tray icon (Phase 2)
- ❌ Windows installer (Phase 2)
- ❌ Auto-update (Phase 2)

## Download

Grab the latest prebuilt binary from the
[Releases](https://github.com/Coding-Dev-Tools/gw2-multibox/releases) page.
Unzip anywhere and run — no installation required.

## Build from source

Requires Rust 1.85 or newer and the MSVC toolchain.

```bash
git clone https://github.com/Coding-Dev-Tools/gw2-multibox.git
cd gw2-multibox
cargo build --release
```

Output: `target/release/gw2-multibox.exe` (single file, no installer required).

## Quick start

```bash
# 1. Generate a starter config
multisbox init

# 2. Edit config.yaml — set your game path and accounts
notepad config.yaml

# 3. Validate the config without launching
multisbox --dry-run

# 4. Launch for real
multisbox
```

The default config targets a single instance in a 1920×1080 region. See
`examples/` for multi-account setups with Guild Wars 2, World of Warcraft,
and Final Fantasy XIV.

## Usage

```bash
multisbox [OPTIONS] [SUBCOMMAND]

Options:
  -c, --config <PATH>    Config YAML file [default: config.yaml]
      --dry-run          Validate config and print launch plan, then exit
      --list-windows     Enumerate all visible top-level windows, then exit
      --ui               Start the web config UI on http://127.0.0.1:7878
      --ui-port <PORT>   Override UI port [default: 7878]
      --debug            Enable debug logging
  -h, --help             Print help
  -v, --version          Print version

Subcommands:
  init            Write a starter config and exit
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
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to contribute

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
├── .github/                CI workflows, issue templates
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

[MIT](LICENSE)
