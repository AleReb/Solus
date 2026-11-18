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

### macOS and Linux

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

### Raspberry Pi and uConsole

Raspberry Pi support is available for 64-bit ARM Linux systems.

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
