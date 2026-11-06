# NewNyth - prebuilt binaries (x86_64 Linux)

Linux driver, daemon, CLI and GUI for the Roccat Nyth mouse.

## Install

```bash
./install-prebuilt.sh
systemctl --user enable --now nythd
newnyth
```

The installer copies the four binaries to `/usr/local/bin`, installs the udev
rule, creates the `roccat` group and adds your user to it (log out and back in),
installs the `nythd` systemd **user** service, the app icon and the launcher
entries.

Prefer native packages? This release also ships a `.deb` (Debian 13 / Ubuntu
24.04+) and an `.rpm` (Fedora) that do the same setup through your package
manager. If your distro is older than those, build from source instead:
https://github.com/LunaBaloona/NewNyth

## Contents

| File | Purpose |
|------|---------|
| `newnyth` | Full configuration GUI |
| `newnyth-mini` | Minimal floating window for quick profile switching |
| `nythd` | Background daemon (hardware events, D-Bus, auto-switch) |
| `nythctl` | CLI for scripts and automation |
| `udev/90-roccat-nyth.rules` | Device permissions |
| `systemd/nythd.service` | Daemon user service |
| `newnyth.desktop`, `newnyth-mini.desktop` | Launcher entries |
| `install-prebuilt.sh` | One-shot installer (uses sudo) |

Licence: GPL-2.0-or-later.
