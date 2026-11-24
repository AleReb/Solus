# Raspberry Pi / uConsole: Linux ARM64

## Prebuilt releases from this fork

For Debian 13 / Raspberry Pi OS based on Debian 13, use the ARM64 binary from
[AleReb/Solus releases](https://github.com/AleReb/Solus/releases) instead of compiling:

```sh
bash scripts/raspberry-pi/install-release.sh --terminal foot
```

Use `--terminal kitty` or `--terminal keep` if preferred. USB options are forwarded:

```sh
bash scripts/raspberry-pi/install-release.sh --terminal foot \
  --download-dir /media/your-user/your-drive/Anime --usb-uuid YOUR-FILESYSTEM-UUID
```

The script downloads `solus-linux-aarch64` and `SHA256SUMS-arm64.txt` over HTTPS,
checks the binary checksum, and calls the existing installer with `--binary` and
`--sha256`. It never installs Rust or compiles locally. Playback dependencies still
need to be installed; `--skip-deps` is available if they are already present.
Use `--tag arm64-YYYYMMDD-COMMIT` to pin a specific release, or `--repo OWNER/REPO`
to use another repository that publishes assets with these names.

These are dynamically linked Debian 13 binaries, not universal Linux executables.
Older Debian/Raspberry Pi OS releases and 32-bit systems are not supported by these
assets. `BUILD-INFO.txt` identifies the exact source commit. Original source and
the MIT license are available in the release and repository.

The fork's `ARM64 binary release` GitHub Actions workflow builds natively on an ARM64
runner inside Debian 13, tests the installer, and publishes a commit-specific release
with checksums. It runs on application changes to `raspberry-pi-arm64` and supports
manual runs. It does not change the upstream multi-platform release workflow.

These optional scripts build the existing Solus application natively. The original
`install.sh`, `install.ps1`, Cargo files, Rust sources, and release workflow are unchanged.
All project credit and the MIT license remain with the upstream authors.

## Requirements

- A 64-bit Raspberry Pi OS / Debian installation (`uname -m`: `aarch64`).
- A graphical session for mpv. Foot needs Wayland; Kitty can also run under X11.
- Internet access and sudo for Debian packages. Run the installer as your normal user.
- Rust stable >= 1.88. The installer installs the official minimal stable toolchain
  through rustup; it does not rely on an older distribution Rust package.
- Allow several GB of disk space for Rust, dependencies and build artifacts. A Pi with
  8 GB of RAM is recommended for building. Even an 8 GB device may need swap when
  browsers or other applications use substantial RAM. Smaller devices are unverified.

The build defaults to **one job** and optimization level **1 for the Solus package**
to reduce compilation pressure. This is a command-line Cargo setting; it does not
change upstream build profiles or dependency versions. The installer does not alter
swap, boot settings, Bluetooth, Wi-Fi, compositor bindings or the default terminal.

## Install

Use a checkout of this branch. Review the scripts before running them.

```sh
bash scripts/raspberry-pi/install.sh
```

Choose Foot, Kitty, or keep the terminal you already use. Explicit alternatives:

```sh
bash scripts/raspberry-pi/install.sh --terminal foot
bash scripts/raspberry-pi/install.sh --terminal kitty
bash scripts/raspberry-pi/install.sh --terminal keep
```

The installer uses apt for build/playback dependencies, installs ani-cli if missing,
builds with the committed Cargo.lock, and installs into `~/.local` by default:

| Path | Purpose |
| --- | --- |
| `~/.local/bin/solus` | Launcher, including optional USB checks |
| `~/.local/share/solus-rpi/solus` | Native ARM64 executable |
| `~/.local/share/solus-rpi/launcher.conf` | Download directory and optional USB UUID |
| `~/.config/solus/config.toml` | Solus preferences (respects XDG_CONFIG_HOME) |

An existing launcher is backed up before replacement. Existing Solus preferences,
history and credentials are preserved. Reinstalling without download options also
preserves a previously configured download directory and USB guard. For a new configuration the defaults are
mpv, 720p, Anime4K off, and self-updates disabled. For an existing configuration,
set these values manually to keep a local ARM build from being replaced:

```toml
[update]
check_on_startup = false
auto_install = false
```

Put `~/.local/bin` on your PATH if your shell does not already include it. Then run
`solus` from Foot or Kitty. Direct launch examples:

```sh
foot -e "$HOME/.local/bin/solus"
kitty "$HOME/.local/bin/solus"
```

Foot and Kitty use different image protocols; this application probes the terminal.
Cover art can take time to appear while the catalog and images load. Maximize the
window; upstream recommends at least 125 columns by 38 rows. The video opens in mpv,
so changing terminal does not change the video decoder.

## Download to a USB drive (optional)

Discover the filesystem UUID and its actual mount point:

```sh
lsblk -o NAME,TRAN,LABEL,UUID,MOUNTPOINTS
```

Pass **your own** values. A UUID identifies the filesystem rather than a changing
device name such as `/dev/sda1`:

```sh
bash scripts/raspberry-pi/install.sh --terminal foot \
  --download-dir /media/your-user/your-drive/Anime \
  --usb-uuid YOUR-FILESYSTEM-UUID
```

The installer links Solus's existing `~/Movies/Solus` vault to the chosen directory.
Downloads remain organized by series. Press **uppercase D** on a series' detail screen
to select an episode or range. The launcher checks that the download path is actually
backed by the configured UUID and attempts a normal udisks mount if needed. If the
USB is disconnected, mounted at a different path, or unwritable, it refuses to start
rather than silently create downloads on the system disk. Mount the drive at the
configured path or rerun the installer with the new path.

An existing download directory or different symlink at `~/Movies/Solus` is never
replaced automatically. Move existing downloads manually before configuring a new
destination. The chosen USB directory must be writable by your normal user. The
installer does not format drives or change their ownership.

For a regular folder without mount checks, omit `--usb-uuid`:

```sh
bash scripts/raspberry-pi/install.sh --terminal keep --download-dir "$HOME/Videos/Anime"
```

## Build or reuse a binary

```sh
bash scripts/raspberry-pi/build.sh --jobs 1
bash scripts/raspberry-pi/build.sh --dry-run --jobs 1
```

The build preview reports the locked Cargo command, job count, and low disk/memory warnings.

With dependencies already installed and a trusted native ARM64 executable:

```sh
bash scripts/raspberry-pi/install.sh --terminal keep --skip-deps \
  --binary /absolute/path/to/solus
```

`--prefix /absolute/path` allows installation under another user-owned prefix.
`--skip-deps` skips apt and ani-cli installation, not compilation. Use `--binary`
as well to skip building. Do not set root as the installation user.

Before making changes, preview the installation:

```sh
bash scripts/raspberry-pi/install.sh --dry-run --terminal keep
```

A supplied binary or downloaded helper can be checksum-verified:

```sh
bash scripts/raspberry-pi/install.sh --binary /absolute/path/to/solus \
  --sha256 <64-character-SHA256>
```

Use `--ani-cli-sha256` or `--rustup-sha256` when those helpers are downloaded.
No supplied checksum means the download is clearly warned rather than silently trusted.

Lifecycle commands:

```sh
bash scripts/raspberry-pi/install.sh --verify
bash scripts/raspberry-pi/install.sh --repair
bash scripts/raspberry-pi/install.sh --uninstall
bash scripts/raspberry-pi/install.sh --uninstall --purge-config
```

`--verify` checks the installed binary, launcher, launcher config, and download directory.
`--repair` follows the normal installer and preserves existing configuration and downloads.
`--uninstall` removes Solus's Raspberry Pi installation files but preserves user configuration,
profiles, and downloads. `--purge-config` is a separate explicit opt-in removal of the Solus config directory.

Existing Solus TOML files are preserved and only missing supported defaults are appended.
Existing keys, update preferences, comments, and unknown settings are not overwritten.
A backup named `config.toml.backup-<timestamp>` is created only when a merge changes the file.

If `~/Movies/Solus` is already a symlink to a different location, the installer refuses to replace it
unless `--replace-vault-link` is supplied. Real directories are never replaced automatically.

```sh
bash scripts/raspberry-pi/install.sh --replace-vault-link \
  --download-dir /absolute/path/to/downloads
```

## Validation and limits

The original application was built and launched on a Raspberry Pi Compute Module 5
uConsole with Debian 13 ARM64; catalog and cover art were checked in Foot and Kitty.
This does not establish compatibility with every Pi model or every video codec.
Some sources can contain a green strip or other defects; compare another quality,
provider or episode before changing the decoder. Hardware decoding depends on the
codec and installed driver and is not forced by these scripts.

Installer checks can be run without apt, recompiling, changing the real HOME or
opening a GUI. Supply a previously built ARM64 binary:

```sh
bash -n scripts/raspberry-pi/*.sh
shellcheck scripts/raspberry-pi/*.sh
python3 scripts/raspberry-pi/check_install.py /absolute/path/to/solus
```

These checks cover installation, reinstallation, preservation of configuration and
existing downloads, paths with spaces, and refusal to launch on the wrong USB.
They do not perform a fresh apt/rustup installation or download episodes.

Pull requests that touch this Raspberry Pi support run a dedicated `.github/workflows/raspberry-pi.yml`:
ShellCheck and bash syntax checks run on Linux, Python helpers are compiled and tested,
TOML merge behavior is checked, and a Debian 13 ARM64 container performs a native
release build plus the temporary-HOME installer regression script. Container emulation
adds coverage but does not replace a real Raspberry Pi smoke test.

## En español

Ejecuta `bash scripts/raspberry-pi/install.sh` desde esta copia del repositorio y
elige **Foot**, **Kitty** o conservar tu terminal. Se necesita un sistema Linux
ARM64 de 64 bits; la primera compilación tarda y consume bastante memoria.
El instalador no cambia tus atajos del escritorio.

Para guardar las series en un USB, utiliza `--download-dir` con la ruta real de tu
carpeta y `--usb-uuid` con el UUID mostrado por `lsblk`. No se borra ni formatea
el disco. Con el USB desconectado, el lanzador se detiene para evitar descargas en
el disco del sistema. Desde el terminal escribe `solus`; dentro de una serie,
**D mayúscula** abre las descargas.

## Upstream integration

This addition is deliberately opt-in: one small README section, this guide, and
`scripts/raspberry-pi/`. There are no hardware-specific constants, personal paths,
tokens, prebuilt binaries, changes to Cargo.lock, or automatic release changes.
The original installers retain their behavior on macOS, Windows and Linux x86_64.

To incorporate later upstream changes:

```sh
git fetch upstream
git merge upstream/main
```

To propose these scripts upstream, open a pull request from the
`raspberry-pi-arm64` branch. Thanks to Abdullah Binmadhi and the Solus contributors
for the original project.
