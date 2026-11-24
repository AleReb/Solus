# Solus

**Solus is an anime theater for your terminal.** It makes it easy to discover anime, track episodes, save favorites, play video, and control playback from your phone or TV.

No ads. No browser tabs. No extra setup screens.

https://github.com/user-attachments/assets/a7c6c35f-ac28-49ef-bb0f-fbfc72bc4177

## What Solus includes

- Anime discovery and search
- AniList watch history and watchlist sync
- Episode streaming in `mpv`, `iina`, or `VLC`
- Subtitle and dub audio options
- Local mobile remote for playback and episode control
- Smart TV casting
- Watch queues and saved playlists
- Download management
- Anime4K upscaling and cover-adaptive colors
- Release calendar and viewing statistics
- Multiple user profiles

## Install

### Raspberry Pi ARM64: install a prebuilt release

Download the compiled executable from this fork's [latest ARM64 release](https://github.com/AleReb/Solus/releases/latest).
**Rust and local compilation are not required.** GitHub Actions builds the releases.

Requirements: **Debian 13 / Raspberry Pi OS based on Debian 13, 64-bit ARM64**
(`uname -m` must print `aarch64`), a graphical desktop, Internet access, and sudo
for system dependencies. These binaries are not for 32-bit Raspberry Pi OS or older
distributions. Foot requires Wayland; choose Kitty for an X11 desktop.

For a fresh installation:

```bash
sudo apt update
sudo apt install -y git curl ca-certificates
git clone --branch raspberry-pi-arm64 --single-branch https://github.com/AleReb/Solus.git
cd Solus
bash scripts/raspberry-pi/install-release.sh --terminal foot
```

Choose the terminal you prefer; run **one** of these alternatives:

```bash
bash scripts/raspberry-pi/install-release.sh --terminal foot
bash scripts/raspberry-pi/install-release.sh --terminal kitty
bash scripts/raspberry-pi/install-release.sh --terminal keep
```

The installer downloads `solus-linux-aarch64` and `SHA256SUMS-arm64.txt` from the
latest release, verifies SHA256, installs the required system packages, and preserves
existing preferences and history. It does not change desktop keyboard shortcuts.
Open your selected terminal and run:

```bash
solus
```

To update an existing checkout and installation, without compiling:

```bash
git pull --ff-only
bash scripts/raspberry-pi/install-release.sh --terminal keep
```

Use `--skip-deps` if the system dependencies are already installed. To select a
specific snapshot, add `--tag arm64-20261124-9969aa1`
([release details](https://github.com/AleReb/Solus/releases/tag/arm64-20261124-9969aa1)).

For downloads on a USB drive, find its filesystem UUID and mount point with
`lsblk -o NAME,LABEL,UUID,MOUNTPOINTS`, then substitute your own values:

```bash
bash scripts/raspberry-pi/install-release.sh --terminal foot \
  --download-dir /media/your-user/your-drive/Anime \
  --usb-uuid YOUR-FILESYSTEM-UUID
```

The configured USB directory and UUID are preserved on subsequent updates. Downloads
stay organized by series; the launcher checks the USB before starting Solus. See the
[Raspberry Pi guide](docs/raspberry-pi.md) for storage permissions, backups, and maintenance.

GitHub Actions builds subsequent ARM64 releases when application code changes on
this fork's `raspberry-pi-arm64` branch; the workflow also supports manual runs.
The [first automatic ARM64 release](https://github.com/AleReb/Solus/releases/tag/arm64-20261124-9969aa1)
was built and its installer checks passed on GitHub. These community builds retain
the original project's MIT license and attribution.

### macOS and Linux x86_64

Open a terminal and run:

```bash
curl -fsSL https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.sh | bash
```

To inspect the installer first:

```bash
curl -fsSLO https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.sh
less install.sh
bash install.sh
```

### Windows

Open PowerShell and run:

```powershell
irm https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.ps1 | iex
```

To inspect the installer first:

```powershell
irm https://raw.githubusercontent.com/abdullah-binmadhi/Solus/main/install.ps1 -OutFile install.ps1
notepad install.ps1
.\install.ps1
```

### Raspberry Pi: build from source (optional)

Use the prebuilt release above to avoid compilation. If you prefer to build the
application yourself, use a 64-bit ARM Linux system and the source installer below.
This option installs Rust and compiles locally, which takes time and memory.

Preview the installation:

```bash
bash scripts/raspberry-pi/install.sh --dry-run
```

Choose Foot, Kitty, or keep your current terminal:

```bash
bash scripts/raspberry-pi/install.sh --terminal foot
bash scripts/raspberry-pi/install.sh --terminal kitty
bash scripts/raspberry-pi/install.sh --terminal keep
```

See the [Raspberry Pi guide](docs/raspberry-pi.md) for requirements, USB downloads, and maintenance commands.

## Run Solus

Open an image-capable terminal such as Kitty, Foot, or WezTerm, then run:

```bash
solus
```

Use a large or full-screen terminal for the best experience.

## Basic controls

| Key | Action |
| --- | --- |
| `j` / `k` or arrows | Move through results |
| `h` / `l` or arrows | Move between cards |
| `Enter` | Open or play the selected item |
| `+` | Add or remove from watchlist |
| `d` | Change audio mode |
| `o` | Change video quality or subtitles |
| `Q` | Open the watch queue and playlists |
| `m` | Open the mobile remote QR code |
| `c` | Open calendar or TV casting |
| `s` | Open settings |
| `Esc` / `q` | Go back or quit |

Press `?` inside Solus for the full keyboard guide.

## AniList

Connect AniList from the login option in Solus to sync your history and watchlist. Solus can also be used without an account.

## Data and settings

Solus stores its settings and data locally:

- macOS: `~/Library/Application Support/solus`
- Linux: `~/.config/solus`
- Windows: `%APPDATA%\solus`

You normally do not need to edit these files directly.

## Requirements

- An image-capable terminal
- `mpv` for the recommended playback experience
- Internet access
- A 64-bit operating system for Raspberry Pi support

## Learn more

- [Raspberry Pi installation guide](docs/raspberry-pi.md)
- [Release checklist](RELEASE-CHECKLIST.md)
- [License](LICENSE)

## Credits

Solus is open source under the [MIT License](LICENSE).

It uses AniList, ani-cli, mpv, Ratatui, and other open-source projects.
