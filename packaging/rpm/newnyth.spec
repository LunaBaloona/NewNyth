# NewNyth — RPM spec for prebuilt binaries
# Used by packaging/make-packages.sh (and the release workflow) via rpmbuild
# with -E 'newnyth_bindir <dir of release binaries>'. The binaries are built
# by the caller; this spec only lays them out, so it works on any RPM host.
#
# The upstream PKGBUILD is the reference for file layout.

%global pkgname newnyth
%global debug_package %{nil}
%global _binaries_in_noarch_packages_terminate_build 0

Name:           %{pkgname}
Version:        %{?newnyth_version}%{!?newnyth_version:0.0.0}
Release:        1%{?dist}
Summary:        Linux driver, daemon, CLI and GUI for the Roccat Nyth mouse
License:        GPL-2.0-or-later
URL:            https://github.com/LunaBaloona/NewNyth
Source:         dummy
BuildArch:      x86_64

# dlopen()'ed at runtime (the binaries carry no NEEDED entries for them),
# so they must be declared explicitly:
Requires:       glibc >= 2.39, dbus-libs, fontconfig, libX11, libXi, libXcursor
Requires:       libxkbcommon >= 1.6.0, libxkbcommon-x11
Requires:       libwayland-client, libwayland-egl, libEGL.so.1()(64bit), libglvnd-glx
Recommends:     playerctl, pulseaudio-utils, libdrm, mesa-filesystem

%description
Modern Linux driver and configuration tools for the Roccat Nyth gaming
mouse, rewritten in Rust from the original roccat-tools C codebase.
Ships the full configuration GUI (newnyth), a floating profile switcher
(newnyth-mini), a background daemon with D-Bus interface and profile
auto-switch support (nythd), and a CLI for scripts and automation
(nythctl). Includes the udev rule for device access and a systemd user
service for the daemon.

%pre
# Create the device-access group before the udev rule lands (mirrors the
# PKGBUILD post_install; 'roccat' group is intentionally kept on removal).
if [ $1 -eq 1 ] && ! getent group roccat >/dev/null 2>&1; then
    groupadd -f roccat
fi

%install
# _pkgroot = repo root; absolute fallbacks because rpmbuild changes CWD.
%global _pkgroot %{expand:%(echo %{_topdir}/../..)}
BINDIR="%{?newnyth_bindir}%{!?newnyth_bindir:%{_pkgroot}/target/release}"
SRCDIR="%{?newnyth_srcdir}%{!?newnyth_srcdir:%{_pkgroot}}"

install -Dm755 "$BINDIR/newnyth"       "%{buildroot}/usr/bin/newnyth"
install -Dm755 "$BINDIR/newnyth-mini"  "%{buildroot}/usr/bin/newnyth-mini"
install -Dm755 "$BINDIR/nythd"         "%{buildroot}/usr/bin/nythd"
install -Dm755 "$BINDIR/nythctl"       "%{buildroot}/usr/bin/nythctl"

install -Dm644 "$SRCDIR/udev/90-roccat-nyth.rules" \
    "%{buildroot}/usr/lib/udev/rules.d/90-roccat-nyth.rules"
install -Dm644 "$SRCDIR/systemd/nythd.service" \
    "%{buildroot}/usr/lib/systemd/user/nythd.service"
install -Dm644 "$SRCDIR/assets/newnyth.png" \
    "%{buildroot}/usr/share/icons/hicolor/128x128/apps/newnyth.png"
install -Dm644 "$SRCDIR/LICENSE" \
    "%{buildroot}/usr/share/licenses/%{pkgname}/LICENSE"

mkdir -p "%{buildroot}/usr/share/applications"
cat > "%{buildroot}/usr/share/applications/newnyth.desktop" <<'EOF'
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
cat > "%{buildroot}/usr/share/applications/newnyth-mini.desktop" <<'EOF'
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

%post
/usr/bin/udevadm control --reload-rules 2>/dev/null || :
/usr/bin/udevadm trigger 2>/dev/null || :
touch --no-create /usr/share/icons/hicolor 2>/dev/null || :
gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || :
if [ $1 -eq 1 ]; then
    echo ">>> IMPORTANT: give your user device access:"
    echo ">>>   sudo usermod -aG roccat \$USER"
    echo ">>> then log out and back in."
    echo ""
    echo ">>> Start the event daemon:"
    echo ">>>   systemctl --user enable --now nythd"
fi

%postun
/usr/bin/udevadm control --reload-rules 2>/dev/null || :
/usr/bin/udevadm trigger 2>/dev/null || :
gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || :

%files
# NB: plain path, not %license — Ubuntu's rpmbuild relocates %license files
# under <name>-<version>/ and then rejects the spec's literal path.
/usr/share/licenses/newnyth/LICENSE
/usr/bin/newnyth
/usr/bin/newnyth-mini
/usr/bin/nythd
/usr/bin/nythctl
/usr/lib/udev/rules.d/90-roccat-nyth.rules
/usr/lib/systemd/user/nythd.service
/usr/share/applications/newnyth.desktop
/usr/share/applications/newnyth-mini.desktop
/usr/share/icons/hicolor/128x128/apps/newnyth.png

%changelog
* Thu Nov 05 2026 NewNyth Contributors - 0.9.2-1
- Prebuilt-binary packaging: .deb + .rpm alongside the release tarball
- udev rule, roccat group creation, systemd user service, desktop entries
  (file layout matches the Arch PKGBUILD)
