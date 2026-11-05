#!/usr/bin/env bash
# NewNyth — prebuilt binary installer
# Generated into the release tarball by packaging/make-packages.sh.
# Run it from inside the extracted newnyth-<version>-x86_64-linux/ directory.
set -euo pipefail

cd "$(dirname "$0")"

echo "=== NewNyth installer (prebuilt binaries) ==="
echo "https://github.com/LunaBaloona/NewNyth"
echo ""

# Sanity check: make sure we're in the extracted archive
for bin in newnyth newnyth-mini nythd nythctl; do
    if [ ! -f "$bin" ] || [ ! -r "$bin" ]; then
        echo "Error: '$bin' not found next to this script."
        echo "Extract the whole archive first, then run ./install-prebuilt.sh from inside it."
        exit 1
    fi
done

# Warn early if the system C library is older than these binaries require
build_glibc="2.39"
sys_glibc="$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$' || echo 0)"
if [ "$(printf '%s\n%s\n' "$build_glibc" "$sys_glibc" | sort -V | head -1)" != "$build_glibc" ]; then
    echo "Warning: this system's glibc ($sys_glibc) is older than $build_glibc (minimum required"
    echo "by these binaries). The programs may fail to start. Build from source if that happens."
fi

# Binary permissions (tar preserves them, but be safe)
chmod +x newnyth newnyth-mini nythd nythctl

# Install binaries
echo "Installing binaries to /usr/local/bin..."
sudo install -m755 nythctl      /usr/local/bin/
sudo install -m755 nythd         /usr/local/bin/
sudo install -m755 newnyth       /usr/local/bin/
sudo install -m755 newnyth-mini  /usr/local/bin/

# Install udev rules
echo "Installing udev rules..."
sudo install -Dm644 udev/90-roccat-nyth.rules /etc/udev/rules.d/90-roccat-nyth.rules

# Create group
echo "Setting up roccat group..."
sudo groupadd -f roccat
if ! id -nG "$USER" | grep -qw roccat; then
    sudo usermod -aG roccat "$USER"
    echo "  Added $USER to roccat group (log out and back in to take effect)"
fi

# Reload udev
sudo udevadm control --reload-rules
sudo udevadm trigger

# Install icon
echo "Installing icon..."
if [ -f "assets/newnyth.png" ]; then
    sudo install -Dm644 assets/newnyth.png /usr/share/icons/hicolor/128x128/apps/newnyth.png
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
fi

# Install systemd user service
echo "Installing systemd user service..."
mkdir -p ~/.config/systemd/user
sed 's|/usr/bin/nythd|/usr/local/bin/nythd|' systemd/nythd.service > ~/.config/systemd/user/nythd.service
systemctl --user daemon-reload

# Install desktop entries
echo "Installing desktop entries..."
sudo install -Dm644 newnyth.desktop      /usr/share/applications/newnyth.desktop
sudo install -Dm644 newnyth-mini.desktop /usr/share/applications/newnyth-mini.desktop
update-desktop-database /usr/share/applications 2>/dev/null || true

echo ""
echo "=== NewNyth installation complete ==="
echo ""
echo "To start the background daemon:"
echo "  systemctl --user enable --now nythd"
echo ""
echo "To launch the GUI:"
echo "  newnyth"
echo ""
echo "CLI tool:"
echo "  nythctl --help"
