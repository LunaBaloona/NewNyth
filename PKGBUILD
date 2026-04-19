# Maintainer: NewNyth Contributors
# https://github.com/LunaBaloona/NewNyth
pkgname=newnyth
pkgver=0.7.1
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
)
makedepends=(
    'rust'
    'cargo'
    'linux-headers'
)
backup=('etc/udev/rules.d/90-roccat-nyth.rules')

# Build from the local directory — no source tarball needed.
# Run makepkg from inside the newnyth project directory.
source=()
sha256sums=()

build() {
    cd "$startdir"
    cargo build --release --locked 2>/dev/null || cargo build --release
}

check() {
    cd "$startdir"
    cargo test --release
}

package() {
    cd "$startdir"

    # Binaries
    install -Dm755 "target/release/nythctl"       "${pkgdir}/usr/bin/nythctl"
    install -Dm755 "target/release/nythd"          "${pkgdir}/usr/bin/nythd"
    install -Dm755 "target/release/newnyth"        "${pkgdir}/usr/bin/newnyth"
    install -Dm755 "target/release/newnyth-mini"   "${pkgdir}/usr/bin/newnyth-mini"

    # udev rules
    install -Dm644 "udev/90-roccat-nyth.rules" \
        "${pkgdir}/etc/udev/rules.d/90-roccat-nyth.rules"

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

post_install() {
    echo ">>> Creating roccat group (if it doesn't exist)..."
    groupadd -f roccat
    echo ">>> Reloading udev rules..."
    udevadm control --reload-rules 2>/dev/null || true
    udevadm trigger 2>/dev/null || true
    echo ">>> Updating icon cache..."
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    echo ""
    echo ">>> IMPORTANT: Add your user to the roccat group:"
    echo ">>>   sudo usermod -aG roccat \$USER"
    echo ">>> Then log out and back in."
    echo ""
    echo ">>> To start the event daemon:"
    echo ">>>   systemctl --user enable --now nythd"
}

post_upgrade() {
    post_install
}
