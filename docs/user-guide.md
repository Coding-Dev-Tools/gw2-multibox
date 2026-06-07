# Multisbox User Guide

Multisbox is a Windows-only multiboxing launcher. It launches multiple instances
of a game, positions their windows into a layout you configure, and lets you
switch between them with hotkeys.

This guide assumes you already have a working game install and the ability to
run multiple instances of the game (most modern MMOs allow this with the right
launch flags; see the per-game notes below).

## Quick start

1. **Download or build** the binary.
   - Pre-built: `multisbox.exe` (single file, no installer needed)
   - Build from source: `cargo build --release` → `target/release/multisbox.exe`

2. **Create a config file.** Start by copying one of the examples in
   `examples/` to your working directory and renaming it `config.yaml`.
   Edit the `exe_path` to point at your game.

3. **Validate:** `multisbox --dry-run`
   This parses your config and prints what would happen, without launching
   anything. Use it to catch typos and config errors.

4. **Launch:** `multisbox`
   This launches your instances, positions the windows, and registers F1–F4
   to switch between them. Press Ctrl+C to exit (or close the console window).

5. **(Optional) Use the UI:** `multisbox --ui`
   Opens a web-based config editor at http://127.0.0.1:7878. Use it instead of
   hand-editing YAML.

## Config file

A minimal config has four parts: game profiles, accounts, layout, and team.

```yaml
game_profiles:
  - name: mygame
    exe_path: "C:/Games/MyGame/game.exe"
    args: ["-windowed", "-novideo"]    # optional
    working_dir: "C:/Games/MyGame"      # optional

accounts:
  - { name: char1, game_profile: mygame }
  - { name: char2, game_profile: mygame }

layout:
  name: my-layout
  regions:
    - { name: left,  x: 0,    y: 0, width: 1920, height: 1080 }
    - { name: right, x: 1920, y: 0, width: 1920, height: 1080 }

team:
  name: my-team
  slots:
    - { index: 1, account: char1, region: left }
    - { index: 2, account: char2, region: right }
  options:
    stagger_delay_ms: 3000
    window_timeout_ms: 60000
```

The full reference is in `docs/config-reference.md`.

## CLI reference

```
multisbox [OPTIONS]

  -c, --config <PATH>    Config YAML file [default: config.yaml]
      --dry-run          Validate config, print plan, exit
      --list-windows     Print all visible windows (debug aid)
      --ui               Start the web config UI on http://127.0.0.1:7878
      --ui-port <PORT>   Override UI port [default: 7878]
      --debug            Enable debug logging
  -h, --help             Print help
  -v, --version          Print version
```

## Hotkeys

- **F1** through **F(N)** — switch to slot N's window
- **Ctrl+C** — exit the launcher (closes all hotkeys cleanly)

Hotkeys are global (work regardless of which window is focused).

## Logs

When the tool runs, it writes a log to:
```
%APPDATA%\Multisbox\multisbox.log
```

Use `--debug` to enable debug-level logging (more verbose).

## Per-game notes

### Guild Wars 2

```yaml
game_profiles:
  - name: gw2
    exe_path: "C:/Games/Guild Wars 2/Gw2-64.exe"
    args: ["-shareArchive"]
    working_dir: "C:/Games/Guild Wars 2"
```

The `-shareArchive` flag lets multiple instances share the asset cache.
If the second instance fails to launch, you may need to kill the GW2
mutex first. The tool does NOT do this automatically — it's the user's
responsibility per the spec (avoiding memory modification).

### World of Warcraft

```yaml
game_profiles:
  - name: wow
    exe_path: "C:/Program Files (x86)/World of Warcraft/World of Warcraft.exe"
    args: []
```

WoW supports multiple instances by default. Just launch and position.

### Final Fantasy XIV

```yaml
game_profiles:
  - name: ffxiv
    exe_path: "C:/Program Files (x86)/SquareEnix/FINAL FANTASY XIV - A Realm Reborn/ffxiv.exe"
    args: []
```

FFXIV supports multiple instances out of the box. Each account must
be logged in separately.

## Troubleshooting

**Window not found after launch.** The game might be slow to render its
window. Increase `window_timeout_ms` (default 60s) to give it more time.

**Window found but positioned wrong.** Check that your monitor coordinates
match the actual layout. The web UI's `--list-windows` mode shows all
visible windows with HWND, PID, and title.

**Config won't parse.** Run `multisbox --dry-run` for a detailed error
message. Common issues: duplicate names, references to non-existent
accounts/regions, non-positive region dimensions.

**Hotkey doesn't fire.** Some games (fullscreen mode especially) capture
input at a low level. Run the game in borderless-windowed or windowed mode.

**Tool won't start at all.** Run from a command prompt to see the error
output. Check the log at `%APPDATA%\Multisbox\multisbox.log`.

## Safety

Multisbox is a **window manager and process launcher**. It does NOT:

- Modify game process memory
- Intercept or send network traffic
- Automate gameplay
- Implement any kind of input broadcasting (Phase 2+)

All it does is launch your game executable multiple times and arrange the
windows. Anything more is your responsibility and may violate your game's
terms of service.
