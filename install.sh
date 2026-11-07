#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<EOF
Usage: ./install.sh [-u|--uninstall | -fu|--fulluninstall]

Without arguments: build NewNyth from source and install it (binaries,
udev rule, icon, systemd user service, desktop entries).

-u, --uninstall: remove everything this script installs. Your
configuration in ~/.config/newnyth and the roccat group are left in
place.

-fu, --fulluninstall: uninstall plus a full cleanup: also delete your
~/.config/newnyth configuration (profiles, openers, auto-switch) and
remove the roccat group and your membership in it. Log out and back in
afterwards so the group removal takes effect in your session.
EOF
}

MODE=install
for arg in "$@"; do
    case "$arg" in
        -h|--help) usage; exit 0 ;;
        -u|--uninstall) MODE=uninstall ;;
        -fu|--fulluninstall) MODE=fulluninstall ;;
        *) echo "Error: unknown argument '$arg'"; echo; usage; exit 1 ;;
    esac
done

if [ "$MODE" = uninstall ] || [ "$MODE" = fulluninstall ]; then
    echo "=== NewNyth Uninstaller ==="
    echo "https://github.com/LunaBaloona/NewNyth"
    echo ""

    # Nothing below removes package-manager state; a package-managed
    # newnyth must go back out the way it came in.
    if command -v pacman >/dev/null 2>&1 && pacman -Q newnyth >/dev/null 2>&1; then
        echo "Error: newnyth is installed via pacman. Remove it with your"
        echo "package manager (e.g. 'sudo pacman -Rns newnyth') instead of this script."
        exit 1
    fi
    if command -v dpkg-query >/dev/null 2>&1 && dpkg-query -W newnyth >/dev/null 2>&1; then
        echo "Error: newnyth is installed via dpkg/apt. Remove it with"
        echo "'sudo apt purge newnyth' instead of this script."
        exit 1
    fi
    if command -v rpm >/dev/null 2>&1 && rpm -q newnyth >/dev/null 2>&1; then
        echo "Error: newnyth is installed via rpm. Remove it with"
        echo "'sudo dnf remove newnyth' instead of this script."
        exit 1
    fi

    if [ "$MODE" = fulluninstall ]; then
        echo "This will remove your Nyth profiles."
        echo "(Full uninstall also deletes ~/.config/newnyth and the roccat group.)"
        printf "Proceed? (y/N): "
        read -r reply || reply=""
        case "$reply" in
            [yY]*) ;;
            *) echo "Aborted; nothing was removed."; exit 1 ;;
        esac
        echo ""
    fi

    echo "Stopping the daemon and removing the systemd user service..."
    systemctl --user disable --now nythd.service >/dev/null 2>&1 || true
    rm -f ~/.config/systemd/user/nythd.service
    systemctl --user daemon-reload >/dev/null 2>&1 || true

    echo "Removing binaries from /usr/local/bin..."
    sudo rm -f /usr/local/bin/nythctl /usr/local/bin/nythd \
        /usr/local/bin/newnyth /usr/local/bin/newnyth-mini

    echo "Removing desktop entries and icon..."
    sudo rm -f /usr/share/applications/newnyth.desktop \
        /usr/share/applications/newnyth-mini.desktop \
        /usr/share/icons/hicolor/128x128/apps/newnyth.png
    gtk-update-icon-cache -f -t /usr/share/icons/hicolor >/dev/null 2>&1 || true

    if [ -e /etc/udev/rules.d/90-roccat-nyth.rules ]; then
        echo "Removing udev rule..."
        sudo rm -f /etc/udev/rules.d/90-roccat-nyth.rules
        sudo udevadm control --reload-rules
        sudo udevadm trigger
    fi

    if [ "$MODE" = fulluninstall ]; then
        echo "Removing your roccat group membership and the group..."
        sudo gpasswd -d "$USER" roccat >/dev/null 2>&1 || true
        if ! sudo groupdel roccat >/dev/null 2>&1; then
            echo "  Note: the 'roccat' group could not be deleted. Other users"
            echo "  may still be members; remove it manually when safe:"
            echo "      sudo groupdel roccat"
        fi
        echo "Deleting your configuration in ~/.config/newnyth..."
        rm -rf ~/.config/newnyth
    fi

    echo ""
    echo "=== NewNyth uninstalled ==="
    echo ""
    if [ "$MODE" = fulluninstall ]; then
        echo "Log out and back in so the roccat group membership removal takes"
        echo "effect in existing sessions."
    else
        echo "Left in place on purpose:"
        echo "  - ~/.config/newnyth (profiles, openers, auto-switch); delete it to reset"
        echo "  - the 'roccat' group and your membership; clean up with:"
        echo "      ./install.sh --fulluninstall"
        echo "    or by hand:"
        echo "      sudo gpasswd -d \"$USER\" roccat && sudo groupdel roccat"
    fi
    exit 0
fi

echo "=== NewNyth Installer ==="
echo "https://github.com/LunaBaloona/NewNyth"
echo ""

# Don't fight an installed package manager — updates would silently diverge.
if command -v pacman >/dev/null 2>&1 && pacman -Q newnyth >/dev/null 2>&1; then
    echo "Error: newnyth is installed via pacman. Update it with your package manager"
    echo "(e.g. 'yay -Syu newnyth' for AUR) instead of running this script."
    exit 1
fi
if command -v dpkg-query >/dev/null 2>&1 && dpkg-query -W newnyth >/dev/null 2>&1; then
    echo "Error: newnyth is installed via dpkg/apt. Update it with 'apt install --only-upgrade newnyth'"
    echo "or reinstall the newest .deb from the GitHub Releases page instead."
    exit 1
fi
if command -v rpm >/dev/null 2>&1 && rpm -q newnyth >/dev/null 2>&1; then
    echo "Error: newnyth is installed via rpm. Update it with 'dnf upgrade newnyth' (or reinstall"
    echo "the newest .rpm from the GitHub Releases page) instead of running this script."
    exit 1
fi

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
GROUP_JUST_ADDED=0
sudo groupadd -f roccat
if ! id -nG "$USER" | grep -qw roccat; then
    sudo usermod -aG roccat "$USER"
    GROUP_JUST_ADDED=1
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

# Finish with the daemon enabled, and running whenever this session can
# already access the device. A freshly granted roccat group is not active
# until re-login, so in that case only enable: starting now would give the
# daemon no permission to open the mouse.
echo "Enabling the background daemon..."
DAEMON_OK=0
if [ "$GROUP_JUST_ADDED" = 1 ]; then
    if systemctl --user enable nythd 2>/dev/null; then
        DAEMON_OK=1
    fi
elif systemctl --user enable --now nythd 2>/dev/null; then
    DAEMON_OK=1
fi

echo ""
echo "=== NewNyth installation complete ==="
echo ""
if [ "$GROUP_JUST_ADDED" = 1 ] && [ "$DAEMON_OK" = 1 ]; then
    echo "The daemon is enabled but not started yet: your roccat group membership"
    echo "only takes effect after logging out and back in. From then on the daemon"
    echo "starts automatically on login."
elif [ "$DAEMON_OK" = 1 ]; then
    echo "The daemon is running and will start automatically on login."
else
    echo "Could not reach the systemd user session. Enable and start the daemon"
    echo "yourself:"
    echo "  systemctl --user enable --now nythd"
fi
echo ""
echo "To launch the GUI:"
echo "  newnyth"
echo ""
echo "CLI tool:"
echo "  nythctl --help"
