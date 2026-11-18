#!/usr/bin/env bash
# Additive Raspberry Pi installer. Does not change upstream's install.sh.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: install.sh [OPTIONS]
  --terminal foot|kitty|keep  Install Foot (Wayland), Kitty, or keep your terminal
  --download-dir /PATH       Optional directory for per-series offline downloads
  --usb-uuid UUID            Require that directory to reside on this USB filesystem
  --jobs N                   Build jobs (default: 1)
  --prefix /PATH             Installation prefix (default: ~/.local)
  --binary /PATH             Install an existing native ARM64 binary instead of building
  --skip-deps                Skip apt and ani-cli installation (dependencies already installed)
  -h, --help                 Show this help

Run as your regular user. sudo is used only to install Debian packages.
Without --terminal: prompt on a terminal; otherwise keep the existing terminal.
Existing Solus configuration is preserved. New configuration disables self-updates.
EOF
}
die() { printf 'Error: %s\n' "$*" >&2; exit 1; }
terminal=ask
download_dir=
download_dir_set=false
usb_uuid=
jobs=1
prefix=$HOME/.local
binary=
skip_deps=false
while (($#)); do
    case "$1" in
        --terminal|--download-dir|--usb-uuid|--jobs|--prefix|--binary)
            [[ $# -ge 2 && -n $2 ]] || die "$1 needs a value"
            case "$1" in
                --terminal) terminal=$2 ;;
                --download-dir) download_dir=$2; download_dir_set=true ;;
                --usb-uuid) usb_uuid=$2 ;;
                --jobs) jobs=$2 ;;
                --prefix) prefix=$2 ;;
                --binary) binary=$2 ;;
            esac
            shift 2 ;;
        --skip-deps) skip_deps=true; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done
[[ $jobs =~ ^[1-9][0-9]*$ ]] || die '--jobs must be a positive integer'
[[ $prefix == /* ]] || die '--prefix must be an absolute path'
[[ -z $download_dir || $download_dir == /* ]] || die '--download-dir must be an absolute path'
[[ -z $usb_uuid || $usb_uuid =~ ^[[:alnum:]-]+$ ]] || die 'invalid UUID'
[[ -z $usb_uuid || -n $download_dir ]] || die '--usb-uuid requires --download-dir'
[[ $(uname -s) == Linux && $(uname -m) == aarch64 ]] || die '64-bit Linux ARM64 (aarch64) is required'
((EUID != 0)) || die 'run as your regular user, without sudo'

if [[ $terminal == ask ]]; then
    if [[ -t 0 ]]; then
        printf 'Terminal: 1) Foot (Wayland)  2) Kitty  3) Keep current [3]: '
        read -r answer || answer=
        case "$answer" in 1) terminal=foot ;; 2) terminal=kitty ;; 3|'') terminal=keep ;; *) die 'choose 1, 2 or 3' ;; esac
    else
        terminal=keep
    fi
fi
case "$terminal" in foot|kitty|keep) ;; *) die '--terminal must be foot, kitty or keep' ;; esac
if [[ $terminal == foot && -z ${WAYLAND_DISPLAY:-} ]]; then
    echo 'Foot requires a Wayland session; it will not run in a plain X11 session.'
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
prefix=$(realpath -m -- "$prefix")
settings_file=$prefix/share/solus-rpi/launcher.conf
if ! $download_dir_set && [[ -f $settings_file ]]; then
    # Preserve USB guards when reinstalling without download options.
    # shellcheck source=/dev/null
    source "$settings_file"
    download_dir=${SOLUS_RPI_DOWNLOAD_DIR:-}
    [[ -n $usb_uuid ]] || usb_uuid=${SOLUS_RPI_USB_UUID:-}
fi

# Validate the download destination before installing packages or compiling.
if [[ -n $download_dir ]]; then
    download_dir=$(realpath -m -- "$download_dir")
    vault=$HOME/Movies/Solus
    if [[ -e $vault || -L $vault ]]; then
        [[ -L $vault && $(readlink -f -- "$vault") == "$download_dir" ]] ||
            die "$HOME/Movies/Solus already exists; move your downloads manually before changing it"
    fi
    if [[ -n $usb_uuid ]]; then
        if ! findmnt -rn -S "UUID=$usb_uuid" >/dev/null; then
            udisksctl mount -b "/dev/disk/by-uuid/$usb_uuid" || die 'connect and mount the USB first'
        fi
        probe=$download_dir
        while [[ ! -d $probe && $probe != / ]]; do probe=$(dirname -- "$probe"); done
        [[ $(findmnt -rno UUID -T "$probe" 2>/dev/null || true) == "$usb_uuid" ]] ||
            die 'download directory is not on the specified USB filesystem'
    fi
    mkdir -p -- "$download_dir"
    [[ -w $download_dir ]] || die 'download directory is not writable by your user'
fi

if ! $skip_deps; then
    command -v apt-get >/dev/null || die 'this installer supports Debian / Raspberry Pi OS; install dependencies manually and use --skip-deps'
    packages=(build-essential pkg-config libssl-dev ca-certificates curl git fzf mpv ffmpeg yt-dlp file)
    [[ $terminal == keep ]] || packages+=("$terminal")
    [[ -z $usb_uuid ]] || packages+=(udisks2)
    sudo apt-get update
    sudo apt-get install -y "${packages[@]}"
    if ! command -v ani-cli >/dev/null; then
        tmp_ani=$(mktemp)
        trap 'rm -f -- "${tmp_ani:-}" "${tmp_rustup:-}"' EXIT
        curl --fail --location --silent --show-error --retry 3 \
            https://raw.githubusercontent.com/pystardust/ani-cli/master/ani-cli -o "$tmp_ani"
        install -Dm755 "$tmp_ani" "$prefix/bin/ani-cli"
    fi
fi

if [[ -z $binary ]]; then
    rustup_bin=$(command -v rustup || true)
    [[ -n $rustup_bin || ! -x $HOME/.cargo/bin/rustup ]] || rustup_bin=$HOME/.cargo/bin/rustup
    if [[ -z $rustup_bin ]]; then
        tmp_rustup=$(mktemp)
        trap 'rm -f -- "${tmp_ani:-}" "${tmp_rustup:-}"' EXIT
        curl --fail --location --silent --show-error --retry 3 https://sh.rustup.rs -o "$tmp_rustup"
        sh "$tmp_rustup" -y --profile minimal --default-toolchain none --no-modify-path
        rustup_bin=$HOME/.cargo/bin/rustup
    fi
    "$rustup_bin" toolchain install stable --profile minimal
    bash "$root/scripts/raspberry-pi/build.sh" --jobs "$jobs"
    binary=$root/target/release/solus
fi
[[ -f $binary && -x $binary ]] || die 'binary must be an executable file'
file --brief -- "$binary" | grep -q 'ELF 64-bit.*ARM aarch64' || die 'binary is not a Linux ARM64 executable'

install -d "$prefix/bin" "$prefix/share/solus-rpi"
install -m755 "$binary" "$prefix/share/solus-rpi/solus"
if [[ -e $prefix/bin/solus || -L $prefix/bin/solus ]]; then
    backup=$prefix/bin/solus.backup-$(date +%Y%m%d-%H%M%S)-$$
    cp -Pp -- "$prefix/bin/solus" "$backup"
    printf 'Previous launcher saved as %s\n' "$backup"
fi
{
    printf 'SOLUS_RPI_DOWNLOAD_DIR=%q\n' "$download_dir"
    printf 'SOLUS_RPI_USB_UUID=%q\n' "$usb_uuid"
} > "$prefix/share/solus-rpi/launcher.conf"
chmod 600 "$prefix/share/solus-rpi/launcher.conf"
install -m755 "$root/scripts/raspberry-pi/launcher.sh" "$prefix/bin/solus"

if [[ -n $download_dir ]]; then
    mkdir -p "$HOME/Movies"
    [[ -L $HOME/Movies/Solus ]] || ln -s -- "$download_dir" "$HOME/Movies/Solus"
fi
config_dir=${XDG_CONFIG_HOME:-$HOME/.config}/solus
if [[ ! -e $config_dir/config.toml ]]; then
    mkdir -p "$config_dir"
    cat > "$config_dir/config.toml" <<'EOF'
quality = "720p"
audio_mode = "sub"
player = "mpv"
anime4k_mode = "off"
config_version = 2

[update]
check_on_startup = false
auto_install = false

[cache]
trending_ttl = 86400
stable_ttl = 604800
detail_ttl = 172800
EOF
else
    echo 'Existing config preserved. Disable [update].check_on_startup and auto_install for local ARM builds.'
fi
printf '\nInstalled: %s/bin/solus\nTerminal choice: %s\n' "$prefix" "$terminal"
case ":$PATH:" in
    *":$prefix/bin:"*) echo 'Run: solus' ;;
    *) printf 'Add this directory to your PATH: %s/bin\nOr run: %q\n' "$prefix" "$prefix/bin/solus" ;;
esac
echo 'No desktop shortcuts, terminal defaults, or compositor settings were changed.'
