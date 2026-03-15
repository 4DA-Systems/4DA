# 4DA on Linux

## Supported Distributions

- Ubuntu 22.04+
- Debian 12+
- Fedora 38+
- Arch Linux (rolling)

Other distributions using GTK3 and WebKitGTK 4.1 should also work.

## Installation

### .deb (Ubuntu / Debian)

```bash
sudo apt install ./4da-home_*.deb
```

### .rpm (Fedora)

```bash
sudo dnf install ./4da-home-*.rpm
```

### AppImage

```bash
chmod +x 4DA-Home_*.AppImage
./4DA-Home_*.AppImage
```

AppImage requires FUSE2. On Ubuntu 22.04+ it is not installed by default:

```bash
sudo apt install libfuse2
```

## Runtime Dependencies

### Ubuntu / Debian

```bash
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1
```

### Fedora

```bash
sudo dnf install webkit2gtk4.1 gtk3 libappindicator-gtk3
```

### Arch

```bash
sudo pacman -S webkit2gtk-4.1 gtk3 libappindicator-gtk3
```

The `.deb` and `.rpm` packages declare these as dependencies, so your package manager will pull them automatically. Only the AppImage requires manual dependency management.

## Known Issues

### NVIDIA: Blank or white screen

WebKitGTK has a known issue with NVIDIA DMA-BUF rendering on some driver versions. 4DA auto-detects NVIDIA GPUs and applies the workaround. If you still see a blank screen, set the environment variable manually:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 ./4da-home
```

Or add to your `.desktop` file / shell profile for a permanent fix.

### GNOME system tray

GNOME does not natively support the StatusNotifierItem (SNI) protocol used for system tray icons. Install the AppIndicator extension:

```bash
# GNOME 45+
sudo apt install gnome-shell-extension-appindicator
```

Then enable it in GNOME Extensions and log out/in.

### Auto-updates

Auto-update only works with the AppImage format. The `.deb` and `.rpm` packages must be updated manually by downloading the new release.

## Data Paths

4DA follows the XDG Base Directory Specification:

| Data | Path |
|------|------|
| Application data | `~/.local/share/4da/` |
| Database | `~/.local/share/4da/data/4da.db` |
| Settings | `~/.local/share/4da/data/settings.json` |
| Logs | `~/.local/share/4da/logs/` |

These paths respect `$XDG_DATA_HOME` if set.

## Building from Source

Install build dependencies (Ubuntu/Debian):

```bash
sudo apt-get install \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev
```

Then build:

```bash
# Install Rust (if not present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js 20+ and pnpm
npm install -g pnpm

# Clone and build
git clone https://github.com/runyourempire/4da.git
cd 4da
pnpm install
pnpm run tauri build
```

Build outputs are in `src-tauri/target/release/bundle/`.

See `.github/workflows/release.yml` for the full CI build configuration.
