# NewNyth API Stability

NewNyth follows Semantic Versioning. This document records exactly
which surfaces are part of the public API, so everyone knows what a
1.x release promises to keep and what is free to change.

In short: within `1.x`, nothing described here is removed or changed in
a way that breaks existing integrations. Additions are allowed.
Breaking changes can only arrive with a new major version, and will be
listed in the changelog with migration notes.

## D-Bus interface

- Bus name: `org.roccat.Nyth`, object path `/org/roccat/Nyth`, on the
  session bus.
- Methods and signals:

| Type   | Name                          | Purpose                                        |
|--------|-------------------------------|------------------------------------------------|
| Method | `ProfileChangedOutside(byte)` | Another program switched the profile           |
| Method | `ConfigurationChangedOutside()` | Reload the JSON config files                 |
| Method | `OpenGui()`                   | Launch the NewNyth GUI                         |
| Method | `WindowActivated(string)`     | KWin script notifies the active window class   |
| Signal | `ProfileChanged(byte)`        | The active profile changed on the device       |

- Before 1.0, three never-implemented members (`StartCapture`, the
  `AppCaptured` signal and `ProfileDataChangedOutside`) were removed
  from the interface. They are not part of the promise because they
  never did anything.

- **Name clash:** roccat-tools uses the same bus name. If both are
  installed, only the service that starts first owns the name; the
  other logs a warning and continues without D-Bus. The name is kept
  deliberately for compatibility with existing integrations and the
  KWin script; do not rely on being the name owner unless roccat-tools
  is not installed.

## Command line

- The four programs and their names: `newnyth`, `newnyth-mini`,
  `nythd`, `nythctl`.
- Flags and subcommands as documented in the README and each program's
  `--help` output. `--version`/`-V` and `--help`/`-h` are always
  accepted, even when no mouse is connected.
- `--version` currently prints the version with the git commit
  (e.g. `nythd 0.9.5 #4d0e5a7`). The commit part is informational
  and may be absent in source tarball builds; parse only the version
  number.

## Config files

- Location: `$XDG_CONFIG_HOME/newnyth/` (default `~/.config/newnyth/`).
- Files: `openers.json`, `autoswitch.json`, `profile_names.json`.
- Every file carries a `"version"` field. A file without the field is
  read as version `1`. Readers must ignore fields they do not know
  (NewNyth itself does), so hand-added keys survive round trips only
  by accident and are not promised.
- Saves write a temporary file and `rename` it into place, so the
  daemon never sees half-written JSON. A file that fails to parse is
  copied to `<name>.bak` before anything replaces it.
- Downgrade note: `openers.json` written by 1.0 or later carries the
  `"version"` key at the top level, which pre-1.0 readers mistake for
  a profile entry; other files are tolerated unchanged by old readers.

## Not covered

These are internal and may change at any time, even within `1.x`:

- The Rust crate APIs (they are workspace-internal, not published).
- Temporary handoff files between the daemon and the KWin script.
- The KWin script installer's working details.
- Human-readable output wording, log messages and notifications.
- The packaging layout beyond the documented install paths
  (systemd user unit, udev rule, desktop entries).
