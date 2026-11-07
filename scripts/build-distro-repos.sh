#!/usr/bin/env bash
#
# Build the signed apt and dnf distribution channels that are published
# to the gh-pages branch (https://lunabaloona.github.io/NewNyth/).
#
# usage: scripts/build-distro-repos.sh <packages-dir> <pages-dir>
#
#   packages-dir  input directory holding release assets: files named
#                 newnyth_<v>-1_amd64.deb and newnyth-<v>-1.fc-style.x86_64.rpm
#                 for one or more releases (all published releases at once;
#                 both repository formats are rebuilt from scratch).
#   pages-dir     output directory for the pages tree. Any existing apt/
#                 and dnf/ content in it is replaced, so the result always
#                 mirrors exactly the packages handed in.
#
# Required environment:
#   GNUPGHOME       GnuPG home directory holding the repo signing key
#   NEWNYTH_GPG_FPR fingerprint of that key (sign-capable, imported)
# Optional:
#   NEWNYTH_GPG_PASSPHRASE_FILE
#                   file containing the key passphrase. When set, the
#                   gpg-agent cache is primed from it so signing tools
#                   (reprepro) run unattended.
#   NEWNYTH_PAGES_URL
#                   base URL of the published pages site.
set -euo pipefail

PKG_DIR=${1:?usage: build-distro-repos.sh <packages-dir> <pages-dir>}
PAGES_DIR=${2:?usage: build-distro-repos.sh <packages-dir> <pages-dir>}
KEY_FPR=${NEWNYTH_GPG_FPR:?set NEWNYTH_GPG_FPR to the repo signing key fingerprint}
: "${GNUPGHOME:?set GNUPGHOME to the GnuPG home directory holding the private key}"
PAGES_URL=${NEWNYTH_PAGES_URL:-https://lunabaloona.github.io/NewNyth}
PAGES_URL=${PAGES_URL%/}

mkdir -p "$PAGES_DIR"
PAGES_DIR=$(cd "$PAGES_DIR" && pwd)
PKG_DIR=$(cd "$PKG_DIR" && pwd)

# --- unattended signing -----------------------------------------------------
# reprepro shells out to a plain gpg, so the key passphrase must be living
# in the agent cache for the duration of this script. Prime that cache with
# one loopback signature; everything signed afterwards is a cache hit.
if [ -n "${NEWNYTH_GPG_PASSPHRASE_FILE:-}" ]; then
  touch "$GNUPGHOME/gpg-agent.conf"
  grep -qxF "allow-loopback-pinentry" "$GNUPGHOME/gpg-agent.conf" ||
    echo "allow-loopback-pinentry" >> "$GNUPGHOME/gpg-agent.conf"
  grep -qxF "default-cache-ttl 1800" "$GNUPGHOME/gpg-agent.conf" ||
    echo "default-cache-ttl 1800" >> "$GNUPGHOME/gpg-agent.conf"
  grep -qxF "max-cache-ttl 1800" "$GNUPGHOME/gpg-agent.conf" ||
    echo "max-cache-ttl 1800" >> "$GNUPGHOME/gpg-agent.conf"
  gpgconf --kill gpg-agent 2>/dev/null || true
  PRIME=$(mktemp)
  echo prime | gpg --batch --yes --pinentry-mode loopback \
    --passphrase-file "$NEWNYTH_GPG_PASSPHRASE_FILE" \
    --local-user "$KEY_FPR" --sign -o "$PRIME" -
  rm -f "$PRIME"
fi

KEY_FPR_SHORT=$(gpg --list-keys --with-colons "$KEY_FPR" 2>/dev/null |
  awk -F: '/^pub/ {print substr($5, length($5)-15); exit}')

# --- public key files ---------------------------------------------------------
gpg --batch --yes --armor --export "$KEY_FPR" > "$PAGES_DIR/newnyth.asc"
gpg --batch --yes --export "$KEY_FPR" > "$PAGES_DIR/newnyth.gpg"

shopt -s nullglob
DEBS=("$PKG_DIR"/newnyth_*_amd64.deb)
RPMS=("$PKG_DIR"/newnyth-*.x86_64.rpm)
if [ ${#DEBS[@]} -eq 0 ] && [ ${#RPMS[@]} -eq 0 ]; then
  echo "build-distro-repos: no .deb or .rpm assets found in $PKG_DIR" >&2
  exit 1
fi

# --- apt repository (reprepro) -------------------------------------------------
APT_DIR=$PAGES_DIR/apt
rm -rf "$APT_DIR"
if [ ${#DEBS[@]} -gt 0 ]; then
  mkdir -p "$APT_DIR/conf"
  cat > "$APT_DIR/conf/distributions" <<EOF
Origin: NewNyth
Label: NewNyth
Suite: stable
Codename: stable
Architectures: amd64
Components: main
Description: NewNyth Linux driver packages for Debian and Ubuntu
SignWith: $KEY_FPR
EOF
  # One package per version; if the same version ever appears twice the
  # last file wins, so re-published releases replace cleanly.
  declare -A deb_for_version=()
  for deb in "${DEBS[@]}"; do
    ver=$(dpkg-deb --field "$deb" Version)
    deb_for_version[$ver]=$deb
  done
  for ver in $(printf '%s\n' "${!deb_for_version[@]}" | sort -V); do
    reprepro --basedir "$APT_DIR" includedeb stable "${deb_for_version[$ver]}"
  done
  # reprepro keeps its own bookkeeping databases and run logs beside the
  # published files; the tree is rebuilt from scratch anyway.
  rm -rf "$APT_DIR/db" "$APT_DIR/logs"
fi

# --- dnf repository (createrepo_c) ---------------------------------------------
DNF_DIR=$PAGES_DIR/dnf/x86_64
rm -rf "$PAGES_DIR/dnf"
if [ ${#RPMS[@]} -gt 0 ]; then
  mkdir -p "$DNF_DIR"
  declare -A rpm_for_version=()
  for rpm_file in "${RPMS[@]}"; do
    base=$(basename "$rpm_file")
    key=${base#newnyth-}
    key=${key%%.fc-style*}
    rpm_for_version[$key]=$rpm_file
  done
  for key in "${!rpm_for_version[@]}"; do
    cp "${rpm_for_version[$key]}" "$DNF_DIR/"
  done
  createrepo_c --general-compress-type=gz --outputdir "$DNF_DIR" "$DNF_DIR"
  gpg --batch --yes --pinentry-mode loopback \
    ${NEWNYTH_GPG_PASSPHRASE_FILE:+--passphrase-file "$NEWNYTH_GPG_PASSPHRASE_FILE"} \
    --local-user "$KEY_FPR" --armor --detach-sign \
    --output "$DNF_DIR/repodata/repomd.xml.asc" "$DNF_DIR/repodata/repomd.xml"

  cat > "$PAGES_DIR/dnf/newnyth.repo" <<EOF
# NewNyth package repository for Fedora, RHEL and openSUSE.
# Install with:
#   sudo curl -fsSL $PAGES_URL/dnf/newnyth.repo -o /etc/yum.repos.d/newnyth.repo
# or drop this file into /etc/yum.repos.d/ yourself.
#
# repo_gpgcheck=1 verifies the signed repomd.xml (repomd.xml.asc); its
# checksums authenticate every package in the repository.
[newnyth]
name=NewNyth
baseurl=$PAGES_URL/dnf/\$basearch/
enabled=1
gpgcheck=0
repo_gpgcheck=1
gpgkey=$PAGES_URL/newnyth.asc
EOF
fi

# --- landing page ---------------------------------------------------------------
cat > "$PAGES_DIR/index.html" <<EOF
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>NewNyth package repositories</title>
<style>
  body { font-family: system-ui, sans-serif; max-width: 46rem; margin: 3rem auto;
         padding: 0 1rem; line-height: 1.5; }
  pre  { background: #f2f2f4; padding: .75rem 1rem; overflow-x: auto; }
  a    { color: #2668c9; }
</style>
</head>
<body>
<h1>NewNyth package repositories</h1>
<p>
  apt and dnf channels for the <a href="https://github.com/LunaBaloona/NewNyth">NewNyth</a>
  Roccat Nyth driver. Arch Linux uses the PKGBUILD from the repository;
  every release also ships standalone
  <a href="https://github.com/LunaBaloona/NewNyth/releases/latest">packages</a>.
</p>

<h2>Debian and Ubuntu (apt)</h2>
<pre>
sudo curl -fsSL $PAGES_URL/newnyth.gpg -o /usr/share/keyrings/newnyth.gpg
echo "deb [signed-by=/usr/share/keyrings/newnyth.gpg] $PAGES_URL/apt stable main" |
  sudo tee /etc/apt/sources.list.d/newnyth.list
sudo apt update &amp;&amp; sudo apt install newnyth
</pre>

<h2>Fedora, RHEL and openSUSE (dnf)</h2>
<pre>
sudo curl -fsSL $PAGES_URL/dnf/newnyth.repo -o /etc/yum.repos.d/newnyth.repo
sudo dnf install newnyth
</pre>

<p>
  Repository metadata is GPG-signed; packages are authenticated by the
  checksums inside the signed manifests. Signing key fingerprint:
  <code>$KEY_FPR_SHORT</code>
  (<a href="$PAGES_URL/newnyth.asc">public key</a>).
</p>
</body>
</html>
EOF

echo "build-distro-repos: done"
if [ ${#DEBS[@]} -gt 0 ]; then
  echo "  apt: $(find "$APT_DIR/pool" -name '*.deb' | wc -l) package(s) in stable"
fi
if [ ${#RPMS[@]} -gt 0 ]; then
  echo "  dnf: $(find "$DNF_DIR" -maxdepth 1 -name '*.rpm' | wc -l) package(s) in $DNF_DIR"
fi
