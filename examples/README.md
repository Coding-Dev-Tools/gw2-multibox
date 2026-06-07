# Multisbox Examples

This directory contains sample configs demonstrating different game setups and
layout strategies. Each is a standalone YAML file you point the binary at via
`--config`. The same `multisbox.exe` binary works with all of them.

## Files

| File | Game | Accounts | Layout |
|------|------|----------|--------|
| `config.yaml` | Guild Wars 2 | 4 | 1 main + 3 small |
| `config-wow.yaml` | World of Warcraft Classic | 3 | 3-wide horizontal |
| `config-ffxiv.yaml` | Final Fantasy XIV | 2 | 2x1 vertical |
| `config-test.yaml` | Notepad (smoke test) | 4 | 1 main + 3 small |

## Switching between them

```bash
multisbox --config config.yaml          # GW2 4-account
multisbox --config config-wow.yaml      # WoW 3-account
multisbox --config config-ffxiv.yaml    # FFXIV 2-account
```

Each config is fully self-contained — accounts, profiles, layout, team, and
options are all in one file. Edit one without affecting the others.

## Using the UI to edit

For a friendlier editing experience, run the web UI and pick your config:

```bash
multisbox --ui --config config.yaml
```

Then open `http://127.0.0.1:7878` in your browser. The UI loads the active
config, lets you edit it in a form, validates on save, and reloads automatically.

## Multi-monitor layouts

Coordinates are absolute screen pixels with (0,0) at the top-left of the
primary monitor. Negative X/Y values place regions on monitors to the
left/above the primary.

**Three-monitor horizontal (5760×1080):**
```yaml
regions:
  - { name: left,   x: 0,     y: 0, width: 1920, height: 1080 }
  - { name: center, x: 1920,  y: 0, width: 1920, height: 1080 }
  - { name: right,  x: 3840,  y: 0, width: 1920, height: 1080 }
```

**Two-monitor horizontal (3840×1080):**
```yaml
regions:
  - { name: left,  x: 0,    y: 0, width: 1920, height: 1080 }
  - { name: right, x: 1920, y: 0, width: 1920, height: 1080 }
```

**Single-monitor 1+3 (1920×1080 with 3 thumbnails below):**
```yaml
regions:
  - { name: main, x: 0,    y: 0,  width: 1920, height: 1080 }
  - { name: s2,   x: 1920, y: 0,  width: 960,  height: 540  }
  - { name: s3,   x: 1920, y: 540, width: 960, height: 540  }
  - { name: s4,   x: 0,    y: 1080, width: 960, height: 540  }
```

(Note: the 1+3 layout overflows the primary monitor — it works if you have
a second monitor or if you accept that the small windows extend off-screen.
Use a true 2x2 grid on a single ultrawide if you want everything visible.)

**Single-monitor 2x2 (3840×2160 ultrawide or two stacked 1080s):**
```yaml
regions:
  - { name: tl, x: 0,    y: 0,    width: 1920, height: 1080 }
  - { name: tr, x: 1920, y: 0,    width: 1920, height: 1080 }
  - { name: bl, x: 0,    y: 1080, width: 1920, height: 1080 }
  - { name: br, x: 1920, y: 1080, width: 1920, height: 1080 }
```

## Adding a new game

1. Copy any of the existing YAML files
2. Update `game_profiles[].exe_path` to your game
3. Update `accounts[]` with one entry per multiboxed character
4. Update `layout.regions[]` to match your monitor arrangement
5. Update `team.slots[]` to map accounts to regions
6. Run `multisbox --dry-run --config your-new-file.yaml` to validate
7. Run `multisbox --config your-new-file.yaml` to launch

The tool doesn't care what game you're running — it's just launching executables
and positioning their windows.
