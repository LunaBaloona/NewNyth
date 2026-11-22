# Changelog

All notable changes to NewNyth are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Signed apt and dnf package repositories served from GitHub Pages
  (`https://lunabaloona.github.io/NewNyth/`): every published release is
  mirrored automatically, so Debian/Ubuntu and Fedora/RHEL/openSUSE
  machines get NewNyth updates through `apt upgrade` / `dnf upgrade`.
  Repository metadata is GPG-signed; the signing key setup and rotation
  procedure live in `docs/RELEASING.md`.

### Changed

- The D-Bus transport in the daemon, GUI, and `nythctl` now uses the
  pure-Rust `zbus` crate instead of the C-based `dbus` bindings. Nothing
  links libdbus anymore, the daemon's 100 ms D-Bus pump is gone, and
  external profile changes and config nudges travel over the same
  connection as before.
- `nythd` now caps `zbus`'s and `tracing`'s log output at warn level
  unless `RUST_LOG` is set, so the systemd journal no longer fills with
  one full D-Bus message dump per window activation. Set `RUST_LOG`
  (e.g. `zbus=trace`) to capture library internals while debugging.
- GUI polish: alternating row bands in the Button Assignment grids now
  span the full card width instead of stopping at the row content, the
  Auto-Switch profile cards use the same card-width bands behind their
  application rows, and the Advanced tab arranges its four cards in a
  two-column grid when the window is wide enough (still stacked when
  narrow).
- GUI polish: the Apply/Save and Revert buttons now sit centred in
  their own row inside a pinned bottom strip on every section (the
  Buttons tab already had such a dedicated bottom section; Settings,
  Advanced and Lighting now share one, and the Auto-Switch save bar
  moved there from the top of its tab). Each Auto-Switch Mouse Profile
  card gained a second header: the "+ Add application" button centred
  directly below the profile title, closed off by a card-wide rule, so
  captured slots appear right underneath it. The placeholder note
  below the Button Assignment grids ("Click a button name above to
  configure it.") is gone; the panel stays blank until a button is
  picked.
- GUI polish: the Tracking and Distance Control Unit radio rows centre
  within their cards, and the small grey helper texts on the Advanced
  tab (polling rate, TCU and DCU descriptions plus the mouse-wide
  settings note) use the normal font size while staying grey. The
  centring was then finished properly: the Apply/Save and Revert
  buttons hug the centre line at any font scale, and horizontal rows
  (DPI levels, sensitivity, advanced sensitivity, lighting colours and
  effects, TCU and DCU radios) are measured and parked on the card's
  centre line instead of packing against a fixed-width strip's left
  edge. The "+ Add application" buttons and the Mouse Profile card
  header titles centre on their cards the same way. The Mouse Profile
  headers are now two lines tall: the fixed "Mouse Profile N" caption
  centred above the custom name, spanning the full width between the
  pencil button and the Default checkbox, each line ellipsised only
  if even it cannot fit.

### Removed

- The `libdbus-1-dev` build dependency and the `libdbus-1-3` /
  `dbus-libs` runtime package requirements, for .deb and .rpm alike.

### Fixed

- An application can now only be auto-switch-assigned to one mouse
  profile. Capturing a window whose class is already assigned elsewhere
  is refused with a status message naming the owning profile, instead of
  silently creating a duplicate whose winning profile was arbitrary.
  Duplicates left in hand-edited configs now resolve deterministically
  to the lowest-numbered profile that lists the app.

## [0.9.4] - 2026-11-07

### Added

- PC-side profile names: rename profiles inline from the profile card
  header (the device has no name field, names live in the config dir).
- Profile slot buttons and the five Mini window profile buttons now carry
  tooltips showing the profile names.
- Step-by-step wizard for the TCU surface calibration, driving the
  one-shot daemon handshake.
- A keep/discard dialog after a new DCU lift-off distance is measured.

### Fixed

- Mini window profile tooltips open above their buttons, including the
  buttons at the window edges.
- The rename pencil stays pinned to the left edge of the card header.

## [0.9.3] - 2026-11-07

### Added

- `.deb` and `.rpm` packages built automatically by the release workflow
  and attached to every GitHub Release.
- `install.sh` gained uninstall modes and now finishes by starting the
  user daemon.
- Buttons tab: the action bar stays pinned while scrolling, and the main
  button rows are numbered and left-aligned to match the mouse.

### Changed

- Workspace moved to Rust edition 2024.
- Wording pass over headings, card titles, log strings and documentation.

## [0.9.2] - 2026-11-05

### Added

- Button Assignment tab rebuilt around self-rendered flat-vector
  illustrations of the Nyth (top view plus side keypad), replacing the
  borrowed store photos. Three strips: standard combos, EasyShift combos
  and the numbered mouse art, height-aligned with the cards beside them.

### Changed

- Auto-switch cards are numbered row-major, matching the physical button
  order; compact combo columns separated by accent rules; the top-view
  mouse is pinned larger and scales with the window.

### Fixed

- Buttons tab scrolls exactly like every other tab (worked around the
  egui 0.37 scrollbar squeeze); the keypad art is capped in size and the
  strip is kept inside the viewport.

## [0.9.1] - 2026-11-05

### Added

- Advanced tab with split X/Y sensitivity, polling rate and the TCU/DCU
  controls.
- Lighting tab preview card showing the mouse zones.
- Swarm-style segmented slider; rebuilt DPI and sensitivity cards.

### Changed

- Hardware feedback polish: even segment gaps, calmer scrollbars, bigger
  Colours heading.

## [0.9.0] - 2026-11-04

### Changed

- Swarm-inspired dark theme and window chrome.
- Embedded UI font switched from Play to Ubuntu.
- Section cards, centred button quadrants and a reflowing auto-switch
  grid.

### Fixed

- Settings and Lighting contents really centred; accent on slider hover.

## [0.8.1] - 2026-11-04

### Added

- The GUI now follows the profile selected on the device.

### Fixed

- The auto-switch save bar is reachable again, and the daemon reliably
  picks up config changes.
- The xdotool bootstrap no longer overrides KWin window activations.
- Profile button row centred in the Mini window.

## [0.8.0] - 2026-10-29

### Changed

- GUI migrated from eframe 0.27 to 0.37; monochrome glyph icons keep
  rendering on the new egui.

### Removed

- The temporary RustSec audit ignore (fixed upstream).

## [0.7.2] - 2026-10-29

### Added

- CI: Dependabot config, lockfile-aware caching, `--locked` builds and a
  RustSec audit job.

### Changed

- Dependency versions and workspace build config refreshed.

### Fixed

- The daemon refuses to start when another instance is already running
  (instance lock).
- Install-time papercuts in the packaging scripts.

## [0.7.1] - 2026-04-19

### Added

- First tagged release (on Github): the `nythd` user daemon and the `newnyth` config
  GUI with profile, button, DPI, sensitivity and lighting support, KDE
  Wayland app auto-switch and Arch packaging. Earlier history was not
  tracked in a changelog.

[Unreleased]: https://github.com/LunaBaloona/NewNyth/compare/v0.9.4...HEAD
[0.9.4]: https://github.com/LunaBaloona/NewNyth/compare/v0.9.3...v0.9.4
[0.9.3]: https://github.com/LunaBaloona/NewNyth/compare/v0.9.2...v0.9.3
[0.9.2]: https://github.com/LunaBaloona/NewNyth/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/LunaBaloona/NewNyth/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/LunaBaloona/NewNyth/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/LunaBaloona/NewNyth/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/LunaBaloona/NewNyth/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/LunaBaloona/NewNyth/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/LunaBaloona/NewNyth/tree/v0.7.1
