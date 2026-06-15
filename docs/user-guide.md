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

   Or use a game-specific template:
   ```bash
   # Guild Wars 2
   multisbox gw2-init

   # World of Warcraft
   multisbox wow-init

   # Final Fantasy XIV
   multisbox ffxiv-init

   # EVE Online
   multisbox eve-init
   ```

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
multisbox [OPTIONS] [SUBCOMMAND]

OPTIONS:
  -c, --config <PATH>    Config YAML file [default: config.yaml]
      --dry-run          Validate config, print plan, exit
      --list-windows     Print all visible windows (debug aid)
      --ui               Start the web config UI on http://127.0.0.1:7878
      --ui-port <PORT>   Override UI port [default: 7878]
      --debug            Enable debug logging
  -h, --help             Print help
  -v, --version          Print version

SUBCOMMANDS:
  gw2-init       Generate a Guild Wars 2 starter config
  wow-init       Generate a World of Warcraft starter config
  ffxiv-init     Generate a Final Fantasy XIV starter config
  eve-init       Generate an EVE Online starter config
  help           Print help (or a subcommand's help)
```

## Hotkeys

- **F1** through **F(N)** — switch to slot N's window
- **F9** — toggle input broadcasting (keyboard only, disabled by default)
- **Ctrl+C** — exit the launcher (closes all hotkeys cleanly)

Hotkeys are global (work regardless of which window is focused).

Input broadcasting forwards keyboard events from the active window to all
other windows. Enable it in your config:

```yaml
broadcast:
  enabled: true
  toggle_key: 0x78  # F9 (default)
```

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
    working_dir: "C:/Games/Guild Wars 2"
    kill_mutex: ANET-WIN32-MUTEX   # required for multi-instance
```

Guild Wars 2 enforces a single-instance check via the mutex
`ANET-WIN32-MUTEX`. Multisbox closes that mutex in the running game
process by walking the process handle table via `NtQuerySystemInformation`
and calling `DuplicateHandle` with `DUPLICATE_CLOSE_SOURCE` — the same
technique used by the popular third-party `Gw2Launcher` tool. This is
process-external: no DLL is injected into the game. If the kill fails
for any reason, the launch continues (the error is logged but not fatal).

**Note on simultaneous multi-instance:** GW2 also opens its `Gw2.dat`
archive with a non-shared handle, so even after the mutex is removed
the second instance will fail to open `Gw2.dat` if the first instance
is still running. Multisbox pre-opens `Gw2.dat` with
`FILE_SHARE_READ|WRITE|DELETE` for the entire launch session
(this is the same `FileLocker` pattern Gw2Launcher uses), and launches
slots sequentially — each slot's window must be ready before the next
slot is launched. With this combination, multi-instance launch works
when `Gw2.dat` is opened by the game with `FILE_SHARE_READ`. If your
game version opens `Gw2.dat` with no sharing flags, the only fully
compatible solutions are: (1) per-account Windows user profiles, or
(2) IAT-patched copies of the game EXE (the InnerSpace/ISBoxer
virtual-files approach, which is outside the scope of this tool's
Terms-of-Service constraints). See `docs/architecture.md` for details.

To opt out of the mutex kill, set `kill_mutex: null` in your profile, or
delete the line. The mutex kill only runs when `kill_mutex` is set to a
non-null string, so any other game profile is unaffected.

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

Input broadcasting (keyboard only) is implemented but disabled by default.
Enable it only if you understand the implications for your game's terms of service.
