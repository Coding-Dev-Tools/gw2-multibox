# Contributing to multisbox

Thanks for your interest in contributing! This project aims to be a small,
focused, and reliable multiboxing tool for Windows. Below is everything you
need to get started.

## Code of conduct

Be respectful. Assume good faith. No harassment.

## What to work on

Good first contributions:

- Bug reports with a config and log file
- New example configs (different games, different layouts)
- Documentation fixes and clarifications
- Additional unit tests for `config::resolve`
- Cross-platform checks in the `winapi` wrappers (returns `Result` where it
  currently panics, etc.)

Larger changes — input broadcasting, system tray, installer — should be
discussed in an issue first. Phase 2 features touch the Windows API surface
deeply and benefit from design review.

## Development setup

You need:

- Windows 10 or 11
- Rust 1.85 or newer (`rustup default stable-x86_64-pc-windows-msvc`)
- A C/C++ build environment (Visual Studio Build Tools or the MSVC workload
  from Visual Studio)

Clone the repo and build:

```powershell
git clone https://github.com/Coding-Dev-Tools/gw2-multibox.git
cd gw2-multibox
cargo build --release
```

Run the notepad smoke test (no game needed):

```powershell
.\target\release\gw2-multibox.exe -c config-test.yaml --dry-run
```

This will validate the config and print what would happen. To actually launch
four notepad windows and exercise the full launch + window positioning +
hotkey code path, run without `--dry-run`.

## Project layout

```
src/
  lib.rs        — library root, re-exports
  main.rs       — CLI, mode dispatch
  config.rs     — YAML schema, validation, save/load
  launcher.rs   — CreateProcessW wrapper
  window.rs     — EnumWindows, SetWindowPos, monitor enumeration
  hotkey.rs     — Global hotkeys + message loop
  log.rs        — file-based structured logging
  http.rs       — embedded HTTP server (zero external deps)
  ui/static/    — web UI assets (HTML, JS, CSS)
```

The `config` module is intentionally permissive — anything that can be
expressed in YAML is fair game. Validation happens in `config::resolve()`.

## Coding style

- `cargo fmt` for formatting
- `cargo clippy --all-targets -- -D warnings` should be clean before pushing
- Prefer returning `Result` over panicking
- Module-level doc comments for any new public function
- No `unwrap()` in non-test code unless it's a static invariant
- No new dependencies that require `dlltool.exe` (the Rust GNU toolchain on
  Windows is missing it). If you need a new crate, check its build
  dependencies.

## Testing

```powershell
cargo test
```

The project has unit tests for config parsing and validation. Tests do not
require a real game install — they construct configs in-memory.

## Submitting a pull request

1. Fork the repo
2. Create a feature branch (`git checkout -b feature/my-change`)
3. Make your changes
4. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
5. Push to your fork
6. Open a pull request against `main`
7. Fill in the PR template

PRs that don't pass CI will not be merged. The CI runs on `windows-latest`
with the MSVC toolchain.

## Commit messages

Short and descriptive. Imperative mood. Examples:

- `Fix duplicate region validation when width is negative`
- `Add --ui-port validation`
- `Document env var %APPDATA% in user guide`

## Releasing

Maintainers only. Cut a tag of the form `v0.X.Y`. The release workflow builds
the binary and creates a GitHub release with the `.exe` and a SHA256SUMS
file attached. Pre-1.0 we follow semver loosely — breaking changes bump the
minor version, fixes bump the patch.
