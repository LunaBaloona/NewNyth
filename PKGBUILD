# Maintainer: NewNyth Contributors
# https://github.com/LunaBaloona/NewNyth
pkgname=newnyth
pkgver=0.9.5
pkgrel=1
pkgdesc="NewNyth — Linux driver, daemon, CLI and GUI for the Roccat Nyth mouse"
arch=('x86_64')
url="https://github.com/LunaBaloona/NewNyth"
license=('GPL-2.0-or-later')
depends=(
    'dbus'
    'libxkbcommon'
    'wayland'
    'libx11'
    'mesa'
    'fontconfig'
    # Runtime helpers invoked by the daemon (kept in sync with
    # packaging/debian/control and packaging/rpm/newnyth.spec):
    'libnotify'       # notify-send, profile-change notifications
    'xdg-user-dirs'   # xdg-user-dir, folder buttons
    'xdg-utils'       # xdg-open, folder buttons
)
makedepends=(
    'rust'
    'cargo'
)
optdepends=(
    'playerctl: mouse media keys (play/pause, next/previous)'
    'libpulse: mouse volume keys via pactl'
    'xdotool: window tracking fallback for XWayland apps'
)
install=newnyth.install

# Build from the local directory — no source tarball needed.
# Run makepkg from inside the newnyth project directory.
source=()
sha256sums=()

build() {
    cd "$startdir"
    cargo build --release --locked
}

check() {
    cd "$startdir"
    cargo test --release --locked
}

package() {
    cd "$startdir"

    # Binaries
    install -Dm755 "target/release/nythctl"       "${pkgdir}/usr/bin/nythctl"
    install -Dm755 "target/release/nythd"          "${pkgdir}/usr/bin/nythd"
    install -Dm755 "target/release/newnyth"        "${pkgdir}/usr/bin/newnyth"
    install -Dm755 "target/release/newnyth-mini"   "${pkgdir}/usr/bin/newnyth-mini"

    # udev rules (packaged rules belong under /usr/lib; /etc is for admins)
    install -Dm644 "udev/70-roccat-nyth.rules" \
        "${pkgdir}/usr/lib/udev/rules.d/70-roccat-nyth.rules"

    # roccat group, created by systemd-sysusers via pacman's systemd hook
    install -Dm644 "systemd/roccat.sysusers" \
        "${pkgdir}/usr/lib/sysusers.d/roccat.conf"

    # systemd user service
    install -Dm644 "systemd/nythd.service" \
        "${pkgdir}/usr/lib/systemd/user/nythd.service"

    # Application icon
    # Installs assets/newnyth.png to the hicolor icon theme.
    # For best results, provide a 256x256 or 128x128 PNG.
    if [ -f "assets/newnyth.png" ]; then
        install -Dm644 "assets/newnyth.png" \
            "${pkgdir}/usr/share/icons/hicolor/128x128/apps/newnyth.png"
        echo "  -> Icon installed from assets/newnyth.png"
    else
        echo "  WARNING: No icon found at assets/newnyth.png"
        echo "  The application will work but won't have a custom icon."
    fi

    # Desktop entry — main app
    install -Dm644 /dev/stdin "${pkgdir}/usr/share/applications/newnyth.desktop" <<EOF
[Desktop Entry]
Name=NewNyth
Comment=Configure your Roccat Nyth mouse
Exec=newnyth
Icon=newnyth
Terminal=false
Type=Application
Categories=Settings;HardwareSettings;
Keywords=mouse;roccat;nyth;gaming;
EOF

    # Desktop entry — mini app
    install -Dm644 /dev/stdin "${pkgdir}/usr/share/applications/newnyth-mini.desktop" <<EOF
[Desktop Entry]
Name=NewNyth Mini
Comment=Quick profile switcher for the Roccat Nyth mouse
Exec=newnyth-mini
Icon=newnyth
Terminal=false
Type=Application
Categories=Settings;HardwareSettings;
Keywords=mouse;roccat;nyth;gaming;profile;
EOF

    # Licence
    install -Dm644 /dev/stdin "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE" <<'EOF'
This software is licensed under the GNU General Public License v2.0 or later.
See https://www.gnu.org/licenses/old-licenses/gpl-2.0.html
EOF
}

# Install-time messages and the obsolete-rule cleanup live in
# newnyth.install (pacman only runs scriptlets from that separate file).
