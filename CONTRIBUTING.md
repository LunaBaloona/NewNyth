# Contributing to NewNyth

Bug reports, feature requests and pull requests are all welcome. The
mouse is rare hardware, so do not worry if you cannot test everything:
say what you could and could not test in the pull request, and the
maintainer will cover the rest.

## Dev setup

Rust 1.95 or newer (required by `eframe`). On Debian/Ubuntu the CI
dependency set is:

```bash
sudo apt-get install -y pkg-config libxkbcommon-dev libwayland-dev \
  libx11-dev libfontconfig1-dev mesa-common-dev
```

Arch equivalents: `rust base-devel pkg-config libxkbcommon wayland libx11
fontconfig mesa`.

Build, test and run everything without a mouse attached:

```bash
cargo build --release
cargo test --release --locked --workspace
```

Most of the code does not need the device: the config format handling,
the GUI layout suite (`cargo test -p nyth-config`) and the CLI argument
parsing all run headless. The device paths live in
`crates/roccat-nyth-hid`; changes there ideally get a hardware check
before merge, but CI cannot do that part.

## Gates CI runs (run these before pushing)

```bash
cargo fmt --all --check
cargo build --release --all --locked
cargo test --release --locked --workspace
cargo clippy --release --locked --workspace --all-targets -- -D warnings
```

Clippy warnings are errors here, on all targets including tests.

## Conventions

- Commits are titled `area: imperative summary`, e.g.
  `packaging: label the tarball install step`.
- User-visible strings, comments and docs use British spelling
  (colour, centre, behaviour).
- No em dashes or emoji in code, comments or user-visible text; a plain
  hyphen or rephrased sentence is preferred.
- GUI changes must not move the layout on their own: read
  [docs/GUI-LAYOUT.md](docs/GUI-LAYOUT.md) first and keep the headless
  layout suite (`mod layout_tests` in `crates/nyth-config/src/theme.rs`)
  honest in the same pull request.
- D-Bus interface, command lines and config formats are frozen from
  1.0: see [docs/API-STABILITY.md](docs/API-STABILITY.md). Additions are
  fine; anything breaking needs a very good reason and a deprecation
  path.

## Reporting a bug on real hardware

Open a [bug report](https://github.com/LunaBaloona/NewNyth/issues/new/choose)
and include:

- the app/daemon version stamp (`newnyth --version`, shown as
  `X.Y.Z #<commit>`);
- whether you run Wayland or X11, and which compositor;
- what happened versus what you expected;
- the daemon log around the time of the problem:

```bash
journalctl --user -u nythd --since "-10 min" --no-pager
```

If the device behaved oddly, also `lsusb | grep 1e7d` and
`ls -la /dev/hidraw*`. Logs are preferred over screenshots of errors.
