#!/usr/bin/env bash
set -euo pipefail

echo "=== NewNyth Installer==="
echo "https://github.com/LunaBaloona/NewNyth"
echo ""

# Check for Rust
if ! command -v cargo &>/dev/null; then
    echo "Error: Rust/Cargo not found. Install with: sudo pacman -S rust"
    exit 1
fi

# Now build
echo "Building (release mode)..."
cargo build --release

# Install binaries
echo "Installing binaries to /usr/local/bin..."
sudo install -m755 target/release/nythctl       /usr/local/bin/
sudo install -m755 target/release/nythd          /usr/local/bin/
sudo install -m755 target/release/newnyth        /usr/local/bin/
sudo install -m755 target/release/newnyth-mini   /usr/local/bin/

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

# Install desktop entry
# NB include remove old RustyNyth entry and icon for NewNyth
echo "Installing desktop entries..."
sudo install -Dm644 /dev/stdin /usr/share/applications/newnyth.desktop <<EOF
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

sudo install -Dm644 /dev/stdin /usr/share/applications/newnyth-mini.desktop <<EOF
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
