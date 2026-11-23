#!/usr/bin/env bash
# NewNyth — build all release artifacts from a release build.
#
#   packaging/make-packages.sh <version> [binary-dir] [repo-root]
#
#   <version>      e.g. 0.9.2 (must match Cargo.toml; tags are vX.Y.Z)
#   [binary-dir]   directory with the four release binaries
#                  (default: <repo-root>/target/release)
#   [repo-root]    checkout to take udev/systemd/assets from
#                  (default: parent directory of this script)
#
# Produces in <repo-root>/dist/:
#   newnyth-<v>-x86_64-linux.tar.gz   self-contained tarball (+ installer)
#   newnyth_<v>-1_amd64.deb           Debian 13 / Ubuntu 24.04+   (dpkg-deb)
#   newnyth-<v>-1.x86_64.rpm          Fedora / RHEL / openSUSE    (rpmbuild)
#   SHA256SUMS.txt                    checksums of all of the above
#
# The deb/rpm ship the *same* binaries as the tarball; runtime dependencies
# are declared manually in packaging/debian/control and packaging/rpm/*.spec
# because the GUI libraries are dlopen()'ed and appear in no NEEDED entries.
set -euo pipefail

VERSION="${1:?usage: make-packages.sh <version> [binary-dir] [repo-root]}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "${3:-$(dirname "$SCRIPT_DIR")}" && pwd)"
BINDIR="$(cd "${2:-$REPO/target/release}" && pwd)"
OUT="$REPO/dist"
NAME="newnyth-$VERSION-x86_64-linux"
STAGE="$OUT/stage/$NAME"

for b in newnyth newnyth-mini nythd nythctl; do
    [ -f "$BINDIR/$b" ] || { echo "error: $BINDIR/$b not found — run: cargo build --release --locked" >&2; exit 1; }
    [ -x "$BINDIR/$b" ] || chmod +x "$BINDIR/$b"
done
[ -f "$REPO/LICENSE" ] || { echo "error: $REPO/LICENSE not found" >&2; exit 1; }

command -v dpkg-deb >/dev/null || { echo "error: dpkg-deb not found" >&2; exit 1; }
HAVE_RPM=1
command -v rpmbuild >/dev/null || HAVE_RPM=0

rm -rf "$OUT/stage" "$OUT"/*.deb "$OUT"/*.rpm "$OUT/$NAME.tar.gz" "$OUT/SHA256SUMS.txt"
mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/assets"

echo "==> staging $NAME"
install -m755 "$BINDIR/newnyth"      "$STAGE/"
install -m755 "$BINDIR/newnyth-mini" "$STAGE/"
install -m755 "$BINDIR/nythd"        "$STAGE/"
install -m755 "$BINDIR/nythctl"      "$STAGE/"
install -m644 "$REPO/udev/70-roccat-nyth.rules" "$STAGE/udev/"
install -m644 "$REPO/systemd/nythd.service"     "$STAGE/systemd/"
install -m644 "$REPO/systemd/roccat.sysusers"   "$STAGE/systemd/"
install -m644 "$REPO/assets/newnyth.png"        "$STAGE/assets/"
install -m644 "$SCRIPT_DIR/README-INSTALL.md"   "$STAGE/README.md"
install -m644 "$REPO/LICENSE"                   "$STAGE/"
install -m755 "$SCRIPT_DIR/install-prebuilt.sh" "$STAGE/"

# Desktop entries and AppStream metadata: packaging/ holds the one
# canonical copy every packaging path installs from; staged at the top
# level for install-prebuilt.sh.
install -m644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNyth.desktop"         "$STAGE/"
install -m644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNythMini.desktop"     "$STAGE/"
install -m644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNyth.appdata.xml"     "$STAGE/"
install -m644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNythMini.appdata.xml" "$STAGE/"

echo "==> tarball"
tar -czf "$OUT/$NAME.tar.gz" -C "$OUT/stage" "$NAME"

echo "==> deb"
DBASE="$OUT/stage/deb-root"
for f in usr/bin/newnyth usr/bin/newnyth-mini usr/bin/nythd usr/bin/nythctl; do
    install -Dm755 "$BINDIR/$(basename "$f")" "$DBASE/$f"
done
install -Dm644 "$REPO/udev/70-roccat-nyth.rules" "$DBASE/usr/lib/udev/rules.d/70-roccat-nyth.rules"
install -Dm644 "$REPO/systemd/nythd.service"     "$DBASE/usr/lib/systemd/user/nythd.service"
install -Dm644 "$REPO/assets/newnyth.png"        "$DBASE/usr/share/icons/hicolor/128x128/apps/newnyth.png"
install -Dm644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNyth.desktop" \
    "$DBASE/usr/share/applications/io.github.LunaBaloona.NewNyth.desktop"
install -Dm644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNythMini.desktop" \
    "$DBASE/usr/share/applications/io.github.LunaBaloona.NewNythMini.desktop"
install -Dm644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNyth.appdata.xml" \
    "$DBASE/usr/share/metainfo/io.github.LunaBaloona.NewNyth.appdata.xml"
install -Dm644 "$SCRIPT_DIR/io.github.LunaBaloona.NewNythMini.appdata.xml" \
    "$DBASE/usr/share/metainfo/io.github.LunaBaloona.NewNythMini.appdata.xml"
install -d -m755 "$DBASE/DEBIAN"
sed "s/^Version: .*/Version: ${VERSION}-1/" "$SCRIPT_DIR/debian/control" > "$DBASE/DEBIAN/control"
install -m755 "$SCRIPT_DIR/debian/postinst" "$DBASE/DEBIAN/postinst"
install -m755 "$SCRIPT_DIR/debian/postrm"   "$DBASE/DEBIAN/postrm"
dpkg-deb --root-owner-group --build "$DBASE" "$OUT" >/dev/null
rm -rf "$OUT/stage/deb-root"

if [ "$HAVE_RPM" = 1 ]; then
    echo "==> rpm"
    RTOP="$OUT/stage/rtop"
    rpmbuild -bb "$SCRIPT_DIR/rpm/newnyth.spec" \
        --define "_topdir $RTOP" \
        --define "newnyth_version $VERSION" \
        --define "newnyth_bindir $BINDIR" \
        --define "newnyth_srcdir $REPO" \
        --target x86_64 >/dev/null
    find "$RTOP/RPMS" -name "*.rpm" -exec cp -t "$OUT" {} +
    rm -rf "$RTOP"
else
    echo "==> rpm: rpmbuild not found, skipping .rpm" >&2
fi

echo "==> SHA256SUMS.txt"
( cd "$OUT" && sha256sum -- *.tar.gz *.deb *.rpm 2>/dev/null > SHA256SUMS.txt )
rm -rf "$OUT/stage"

echo "--- artifacts in $OUT ---"
ls -lh "$OUT"
