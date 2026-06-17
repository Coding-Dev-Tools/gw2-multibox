# GW2 Multibox - Launch Options

This document describes the three ways to launch your 4 GW2 accounts for multiboxing.

## The Problem

Gw2Launcher has a known incompatibility with InnerSpace's process injection. The .NET STA
threading gets corrupted, causing "DragDrop registration did not succeed" errors and
"failed" status on profile launches. Gw2Launcher also rewrites the `.dat` files when you
click accounts, which can corrupt your account token data.

## The Solution: Three Options

### Option 1: Use gw2-multibox (Recommended, standalone)

Launches all 4 games at once. Handles the GW2 single-instance mutex (ANET-WIN32-MUTEX).
Does its own window layout and broadcasting (via the embedded web UI).

- **Shortcut**: Desktop `gw2-multibox` shortcut
- **Config**: `C:\Users\home\OneDrive\Documents\GitHub\gw2-multibox\config.yaml`
- **Pros**: One click, all 4 windows, no InnerSpace dependency
- **Cons**: Doesn't use ISBoxer's window layout or keymap system

### Option 2: Use InnerSpace/ISBoxer with per-account game entries (No Gw2Launcher)

ISBoxer launches 4 separate `Gw2-64.exe` instances, each from its own per-account subfolder
(`Gw2Launcher\1\`, `Gw2Launcher\2\`, `Gw2Launcher\3\`) with the correct `-l:id:N` parameter.
ISBoxer handles window layout and broadcasting.

- **How to run**:
  1. Open ISBoxer Toolkit
  2. Load the `4boxnew` character set
  3. Click the character set to launch all 4 slots
  4. Each character launches its own `Gw2-64.exe` from the per-account folder
- **Pros**: Uses ISBoxer's proven window layout and keymap system
- **Cons**: Each game's Local.dat must be set up correctly before launch
- **Note**: The game reads Local.dat from `%APPDATA%\Guild Wars 2\Local.dat`. You need to
  copy the right account's Local.dat to that location before launching each instance.
  Use the PowerShell script (Option 3) or the batch file wrappers for this.

### Option 3: Use the PowerShell launch script + ISBoxer "Detect Running Games"

This is the **most reliable** approach for ISBoxer-based multiboxing:

1. Run the PowerShell script `launch-gw2-multibox.ps1` (Desktop shortcut: "Launch GW2 (4 accounts)")
   - It copies each account's Local.dat to the standard location
   - Launches 4 `Gw2-64.exe` instances with correct `-l:id:N` args
2. Open ISBoxer Toolkit
3. Use **"Detect Running Games"** to find the 4 `Gw2-64.exe` windows
4. ISBoxer applies the `GW2614` window layout and broadcasting

- **Shortcut**: Desktop `Launch GW2 (4 accounts)` shortcut
- **Pros**: Bypasses Gw2Launcher entirely, uses ISBoxer for layout/broadcasting
- **Cons**: Two-step process (launch script, then ISBoxer)

## File Layout

```
C:\Program Files\Guild Wars 2\
├── Gw2-64.exe                      (game client, root)
├── Gw2.dat                         (87GB hardlink, shared)
├── Gw2Launcher\                    (per-account subfolders with hardlinks)
│   ├── 1\
│   │   ├── Gw2-64.exe (hardlink)
│   │   ├── Gw2.dat (hardlink)
│   │   ├── d3d11.dll (hardlink)
│   │   └── bin64\cef\ (hardlinks)
│   ├── 2\...
│   └── 3\...
├── InnerSpace\
│   ├── ISBoxerToolkitProfile.XML    (ISBoxer config)
│   └── GameConfiguration.XML       (game entries)

C:\Users\home\AppData\Roaming\Gw2Launcher\data\
├── 1.dat                           (Account 1 token - Jomie)
├── 2.dat                           (Account 2 token - Jaixi)
├── 3.dat                           (Account 3 token - adminatvpn)
├── 1\
│   ├── Local.dat                   (Account 1 Local.dat for game)
│   └── AppData\Roaming\Guild Wars 2\Local.dat
├── 2\...
└── 3\...

C:\Users\home\OneDrive\Documents\GitHub\gw2-multibox\
├── target\release\gw2-multibox.exe (custom multibox tool)
├── config.yaml                     (4-account config)
└── tools\
    ├── launch-gw2-multibox.ps1     (PowerShell launch script)
    └── gw2launcher-src\            (vendored Gw2Launcher source for reference)
```

## ISBoxer Character Mapping

| Character | Game Entry | Account | -l:id |
|-----------|------------|---------|-------|
| 1a | Gw2-64-Acct1 | Jomie | 1 |
| 2a | Gw2-64-Acct2 | Jaixi | 2 |
| 3a | Gw2-64-Acct3 | adminatvpn | 3 |
| 4a | Gw2-64-Acct1 | Jomie (alt) | 1 |

## Troubleshooting

### "Failed" error on profile launch
This was the original Gw2Launcher issue. With the current setup, you should NOT use
Gw2Launcher at all. Use Option 1, 2, or 3 above.

### "Unable To Open Archive File" on game start
The Gw2.dat hardlinks are broken or inaccessible. Check:
- `C:\Program Files\Guild Wars 2\Gw2.dat` exists and is 87GB
- `C:\Program Files\Guild Wars 2\Gw2Launcher\1\Gw2.dat` is a hardlink to the above
- Run: `fsutil hardlink list "C:\Program Files\Guild Wars 2\Gw2Launcher\1\Gw2.dat"`
  to verify hardlinks

### All 4 games log into the same account
The Local.dat in `%APPDATA%\Guild Wars 2\Local.dat` is wrong. Use the PowerShell
script (Option 3) which copies the right Local.dat for each account before launch.

### Games fail with mutex error
ANET-WIN32-MUTEX is still held by another process. Kill all Gw2-64.exe processes
and try again. The gw2-multibox tool automatically kills this mutex.
