# Releasing NewNyth

How to cut a release. Everything here runs on a machine with push access
to the repository; the work leading up to it goes through pull requests
as usual.

## Versioning

- Fixes and hygiene work bump the patch version (0.9.4 to 0.9.5).
- Feature work bumps the minor version (0.9.x to 0.10.0).
- Tags are always `v` plus the version, e.g. `v0.9.5`.

## Before you release

1. `main` must be green: CI (format, clippy, tests, audit) passed on the
   latest merged pull request.
2. Run the same gates locally:

   ```bash
   cargo fmt --all --check
   cargo clippy --release --locked --workspace --all-targets -- -D warnings
   cargo test --release --locked --workspace
   ```

3. Hardware smoke test on a real Nyth (`./install.sh` from `main`):
   connect, switch profiles, trigger an auto-switch to a bound app,
   apply a profile, change DPI, run the TCU wizard once, open the Mini
   window. For a feature release, walk the matching test checklist
   instead.

## 1. Version bump (one commit)

Branch off the current `main`:

```bash
git checkout -b release/X.Y.Z main
```

Update three places:

1. `version` in the root `Cargo.toml` (the workspace package version).
2. `pkgver` in `PKGBUILD`.
3. `CHANGELOG.md`: rename the `Unreleased` heading to
   `## [X.Y.Z] - YYYY-MM-DD`, and add the new compare link at the bottom.

Then refresh the lockfile and sanity-check the build:

```bash
cargo update --workspace
cargo build --release --locked
git add -A
git commit -m "chore: release X.Y.Z"
```

`cargo update --workspace` should only touch the workspace crates' own
entries in `Cargo.lock`. If it changes third-party dependencies, stop
and look into it before committing.

## 2. Merge

Push the branch, open a pull request titled `chore: release X.Y.Z`
against `main`, wait for CI, merge it like any other pull request.

## 3. Tag and build

```bash
git checkout main
git pull
git tag -a vX.Y.Z -m "NewNyth vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag triggers the Release workflow
(`.github/workflows/release.yml`). It builds the release binaries on
ubuntu-24.04 (glibc 2.39 baseline) and opens a **draft** GitHub Release
with these assets:

- `newnyth-X.Y.Z-x86_64-linux.tar.gz` (with `install-prebuilt.sh`)
- `newnyth_X.Y.Z-1_amd64.deb` (Debian 13 / Ubuntu 24.04 and newer)
- `newnyth-X.Y.Z-1.fc-style.x86_64.rpm` (Fedora / RHEL / openSUSE)
- `SHA256SUMS.txt`

## 4. Publish the release

1. On GitHub: repository > **Releases** > the draft for `vX.Y.Z` >
   **Edit**.
2. Title: `NewNyth vX.Y.Z`.
3. Write notes aimed at users: what is new, improved and fixed; point at
   the matching `CHANGELOG.md` section for full detail; call out known
   issues if there are any.
4. **Publish release**.

Re-running the workflow for an existing tag refreshes the assets and
returns the release to draft state, so a failed or repeated run needs no
cleanup.

## If something goes wrong after publishing

- A typo in the notes: edit the release page directly.
- A broken artifact: fix it on `main` and cut the next patch version.
  Keep the old release visible with a short note pointing at the
  replacement, so nobody installs the broken package by accident.
