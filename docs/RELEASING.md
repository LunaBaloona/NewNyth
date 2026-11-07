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

## Maintaining the package repositories

Published releases are mirrored into signed apt and dnf repositories
served from the `gh-pages` branch at
`https://lunabaloona.github.io/NewNyth/` (the landing page there shows
the setup commands for each distro family). The `Package repositories`
workflow reruns automatically whenever a release is published, edited,
unpublished or deleted, and each run rebuilds both repositories from
scratch out of every published non-prerelease release, so nothing needs
manual upkeep beyond the secrets below.

The trust model: all repository metadata (apt `Release`, dnf
`repomd.xml`) is GPG-signed with the repo key, and the signed manifests
pin the checksum of every package. Individual packages are not
separately signed (matching how Debian ships `.deb`s).

### One-time setup: the signing key

Generate a throwaway key dedicated to repo signing (no personal
identity), then store its private half in the repository secrets.

```bash
# Pick a strong passphrase first and keep it in your password manager.
read -rsp "Passphrase for the NewNyth repo key: " PASS; echo
gpg --batch --pinentry-mode loopback --passphrase "$PASS" --gen-key <<EOF
%echo Generating the NewNyth repository signing key
Key-Type: RSA
Key-Length: 3072
Key-Usage: sign
Name-Real: NewNyth Repo Signing
Name-Email: repo@newnyth.local
Expire-Date: 2y
Passphrase: $PASS
%commit
EOF
unset PASS

# Note the fingerprint, then export the armored private key.
gpg --list-secret-keys "NewNyth Repo Signing"
gpg --armor --export-secret-keys FINGERPRINT > newnyth-repo-key.asc
```

On GitHub: repository > **Settings** > **Secrets and variables** >
**Actions** > **New repository secret**, twice:

| Name                     | Value                              |
| ------------------------ | ---------------------------------- |
| `NEWNYTH_GPG_PRIVATE_KEY`| entire contents of the `.asc` file |
| `NEWNYTH_GPG_PASSPHRASE` | the passphrase                     |

Then remove the export: `shred -u newnyth-repo-key.asc`. Keep the public
half anywhere you like; the workflow publishes it onto the pages site.

### First-time enablement

1. Merge the workflow, then trigger it once by hand: **Actions** >
   **Package repositories** > **Run workflow**. This creates the
   `gh-pages` branch with both repositories.
2. Enable Pages: repository > **Settings** > **Pages** > Source:
   **Deploy from a branch** > branch `gh-pages`, folder `/ (root)` >
   **Save**.
3. Smoke test: open `https://lunabaloona.github.io/NewNyth/` and follow
   the apt and dnf snippets on a clean VM or container; installing
   `newnyth` should offer the version from the pages repo.

### Rotating the key

Regenerate the key (same commands) and replace both secrets. Rotation
is breaking for existing installs: their pinned key can no longer verify
the re-signed metadata, so every user must re-download the new
`newnyth.gpg` / `newnyth.asc`. Only rotate if the key actually leaked
or expires, and announce it in the next release's notes.
