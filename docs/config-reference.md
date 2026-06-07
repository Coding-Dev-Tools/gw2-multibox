# Config Reference

The config file is YAML. All fields are required unless marked optional.

## Top-level structure

```yaml
game_profiles: [...]   # required
accounts:      [...]   # required
layout:        {...}   # required
team:          {...}   # required
```

## game_profiles

A list of executable + launch-arg combinations. Each profile is a self-contained
"how to launch this game" definition. Multiple profiles allow multi-game setups
in the same config.

```yaml
game_profiles:
  - name: gw2              # required — used by accounts
    exe_path: "C:/Games/Guild Wars 2/Gw2-64.exe"   # required — absolute path
    args: ["-shareArchive"]    # optional — command-line args, space-separated
    working_dir: "C:/Games/Guild Wars 2"   # optional — defaults to ""
    window_ready_delay_ms: 0   # optional — wait this long after launch
                                 #            before polling for the window
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | Unique identifier. Referenced by `accounts[].game_profile`. |
| `exe_path` | string | yes | Absolute path to the executable. Use forward slashes. |
| `args` | list[string] | no | Command-line args passed to the process. |
| `working_dir` | string | no | Working directory. Empty = inherit from parent. |
| `window_ready_delay_ms` | integer | no | Delay after launch before window polling. |

## accounts

A list of named accounts. Each account is a label for a "thing to launch" — the
tool itself doesn't manage credentials.

```yaml
accounts:
  - { name: acc1, game_profile: gw2 }
  - { name: acc2, game_profile: gw2, extra_args: ["-debug"] }
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | Unique identifier. Referenced by `team.slots[].account`. |
| `game_profile` | string | yes | Must match a `game_profiles[].name`. |
| `extra_args` | list[string] | no | Per-account extra args appended to the profile args. |

## layout

Defines the screen regions where windows will be placed.

```yaml
layout:
  name: my-layout
  regions:
    - { name: main, x: 0, y: 0, width: 1920, height: 1080 }
    - { name: s2,   x: 1920, y: 0, width: 960, height: 540 }
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | Human label. |
| `regions` | list[Region] | yes | At least 1 required. |

### Region

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | Unique within layout. Referenced by `team.slots[].region`. |
| `x` | integer | yes | Top-left X in screen pixels. |
| `y` | integer | yes | Top-left Y in screen pixels. |
| `width` | integer | yes | Must be > 0. |
| `height` | integer | yes | Must be > 0. |

## team

Defines the multibox setup: which accounts are running, in what order, and where.

```yaml
team:
  name: my-team
  slots:
    - { index: 1, account: acc1, region: main }
    - { index: 2, account: acc2, region: s2 }
  options:
    stagger_delay_ms: 3000
    window_timeout_ms: 60000
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | yes | Human label. |
| `slots` | list[Slot] | yes | At least 1 required. |
| `options` | TeamOptions | no | Timing tuning — see below. |

### Slot

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `index` | integer | yes | Unique. Determines F-key assignment (index 1 → F1, etc). |
| `account` | string | yes | Must match an `accounts[].name`. |
| `region` | string | yes | Must match a `layout.regions[].name`. |

### TeamOptions

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `stagger_delay_ms` | integer | 3000 | Wait this long between launches. |
| `window_timeout_ms` | integer | 60000 | Wait this long for each window to appear. |

## Validation rules

`multisbox --dry-run` runs validation and reports errors. These are the rules:

- All `name` fields must be unique within their scope (profile names, account
  names, region names, slot indices).
- `accounts[].game_profile` must reference an existing profile.
- `team.slots[].account` must reference an existing account.
- `team.slots[].region` must reference an existing region.
- `layout.regions[].width` and `.height` must be > 0.
- `game_profiles[].exe_path` should exist (warning only, not an error — useful
  for testing configs on a different machine).

## Example: minimum viable

```yaml
game_profiles:
  - { name: g, exe_path: "C:/game.exe" }

accounts:
  - { name: a, game_profile: g }

layout:
  name: l
  regions:
    - { name: r, x: 0, y: 0, width: 1920, height: 1080 }

team:
  name: t
  slots:
    - { index: 1, account: a, region: r }
```

This launches 1 instance, places it at (0,0) with size 1920×1080, and registers
F1 to activate it.
