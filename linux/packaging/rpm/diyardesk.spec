# RPM spec for DiyarDesk (independent GPL-3.0 fork of Deskland Preview).
# Build from this directory:
#   rpmbuild -ba diyardesk.spec
# or with the source tarball alongside:
#   rpmbuild -ba --define "_sourcedir $PWD" diyardesk.spec
#
# Requires/BuildRequires use Fedora / RHEL / CentOS Stream package names.
# For openSUSE, map the GTK/WebKit typelib packages accordingly.

Name:           diyardesk
Version:        0.3.0
Release:        1%{?dist}
Summary:        Adaptive animated wallpaper that follows desktop icons

License:        GPL-3.0-only
URL:            https://github.com/adam0307a/DiyarDesk
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz

BuildArch:      noarch
BuildRequires:  gettext

# Drop-in replacement for the upstream Deskland Preview package,
# matching the Replaces/Conflicts fields of the .deb and Arch packages.
Provides:       deskland-preview
Conflicts:      deskland-preview
Obsoletes:      deskland-preview < %{version}-%{release}

# Nothing is compiled; assets are copied into the runtime layout.
%global debug_package %{nil}
# Python sources live outside the standard site-packages paths.
%global __brp_python_bytecompile %{nil}

Requires:       python3 >= 3.10
Requires:       python3-gobject
Requires:       python3-cairo
Requires:       gtk3
Requires:       webkit2gtk4.1
Requires:       libayatana-appindicator-gtk3
Requires:       xdg-utils

Recommends:     gtk-layer-shell
Recommends:     xdotool
Recommends:     gstreamer1-plugins-good

%description
DiyarDesk (based on Deskland Preview) renders a WebGL/Canvas scene behind the
desktop and reshapes it around the real positions of desktop icons. Includes
adaptive themes, custom theme import, tray controls, session autostart, KDE
Plasma 6 integration and privacy-safe sharing.

Runs entirely locally: no accounts, telemetry, store APIs or update endpoints.

%prep
%autosetup -n DiyarDesk-%{version}

%build
# Compile the gettext catalogs from the PO sources instead of relying on
# the prebuilt .mo files checked into the extension directory.
rm -rf locale-build
sh linux/gnome-extension/build-mo.sh locale-build

%install
install -d %{buildroot}%{_prefix}/lib/deskland/linux/deskland_linux \
           %{buildroot}%{_prefix}/lib/deskland/prototype \
           %{buildroot}%{_bindir} \
           %{buildroot}%{_datadir}/applications \
           %{buildroot}%{_datadir}/icons/hicolor/scalable/apps \
           %{buildroot}%{_datadir}/plasma/wallpapers \
           %{buildroot}%{_datadir}/plasma/plasmoids \
           %{buildroot}%{_datadir}/gnome-shell/extensions

cp -a linux/deskland_linux/. %{buildroot}%{_prefix}/lib/deskland/linux/deskland_linux/
cp -a prototype/. %{buildroot}%{_prefix}/lib/deskland/prototype/
cp -a linux/plasma-wallpaper/org.deskland.preview %{buildroot}%{_datadir}/plasma/wallpapers/
cp -a linux/kde-iconspy/org.deskland.iconspy %{buildroot}%{_datadir}/plasma/plasmoids/
cp -a linux/gnome-extension/deskland@deskland.app %{buildroot}%{_datadir}/gnome-shell/extensions/

# System-installed GNOME extensions have no locale/ subfolder; GNOME Shell
# then binds the gettext domain to Config.LOCALEDIR, so ship the compiled
# catalogs in the system locale tree and let %find_lang tag them.
install -d %{buildroot}%{_datadir}/locale
cp -a locale-build/. %{buildroot}%{_datadir}/locale/
rm -rf %{buildroot}%{_datadir}/gnome-shell/extensions/deskland@deskland.app/locale \
       %{buildroot}%{_datadir}/gnome-shell/extensions/deskland@deskland.app/po

install -m644 linux/packaging/deskland.desktop %{buildroot}%{_datadir}/applications/deskland.desktop
install -m644 linux/packaging/deskland.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/deskland.svg

# Never ship interpreter caches.
find %{buildroot}%{_prefix}/lib/deskland -type d -name __pycache__ -prune -exec rm -rf {} +

cat > %{buildroot}%{_bindir}/deskland-linux <<'EOF'
#!/bin/sh
export PYTHONPATH="/usr/lib/deskland/linux${PYTHONPATH:+:$PYTHONPATH}"
export DESKLAND_PROTOTYPE="${DESKLAND_PROTOTYPE:-/usr/lib/deskland/prototype}"
exec python3 -m deskland_linux "$@"
EOF
chmod 755 %{buildroot}%{_bindir}/deskland-linux
install -m 755 linux/bin/diyardesk-ctl %{buildroot}%{_bindir}/diyardesk-ctl

%find_lang deskland

%files -f deskland.lang
%license LICENSE NOTICE THIRD_PARTY_NOTICES.md TRADEMARKS.md
%doc README.md SECURITY.md

%{_bindir}/deskland-linux
%{_bindir}/diyardesk-ctl
%{_datadir}/applications/deskland.desktop
%{_datadir}/icons/hicolor/scalable/apps/deskland.svg
%{_datadir}/plasma/wallpapers/org.deskland.preview
%{_datadir}/plasma/plasmoids/org.deskland.iconspy
%{_datadir}/gnome-shell/extensions/deskland@deskland.app

%dir %{_prefix}/lib/deskland
%dir %{_prefix}/lib/deskland/linux
%{_prefix}/lib/deskland/linux/deskland_linux
%{_prefix}/lib/deskland/prototype

%changelog
* Sun Nov 22 2026 DiyarDesk contributors <adam0307a@users.noreply.github.com> - 0.2.12-1
- Initial RPM package for DiyarDesk 0.2.12 (Linux host, themes, Plasma/GNOME integration).
