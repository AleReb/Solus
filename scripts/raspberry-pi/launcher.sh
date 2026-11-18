#!/usr/bin/env bash
set -euo pipefail

prefix=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# Generated with shell quoting by install.sh, and owned by the installing user.
# shellcheck source=/dev/null
source "$prefix/share/solus-rpi/launcher.conf"
export PATH="$prefix/bin:$PATH"

if [[ -n $SOLUS_RPI_USB_UUID ]]; then
    if ! findmnt -rn -S "UUID=$SOLUS_RPI_USB_UUID" >/dev/null; then
        udisksctl mount -b "/dev/disk/by-uuid/$SOLUS_RPI_USB_UUID" || {
            echo 'Connect the configured USB drive before starting Solus.' >&2; exit 1;
        }
    fi
    # Check the filesystem actually backing the path, not just any mounted USB.
    probe=$SOLUS_RPI_DOWNLOAD_DIR
    while [[ ! -d $probe && $probe != / ]]; do probe=$(dirname -- "$probe"); done
    actual_uuid=$(findmnt -rno UUID -T "$probe" 2>/dev/null || true)
    [[ $actual_uuid == "$SOLUS_RPI_USB_UUID" ]] || {
        echo 'Download path is not on the configured USB. Refusing to use the system disk.' >&2; exit 1;
    }
fi

if [[ -n $SOLUS_RPI_DOWNLOAD_DIR ]]; then
    vault=$HOME/Movies/Solus
    [[ -L $vault && $(readlink -f -- "$vault") == $(realpath -m -- "$SOLUS_RPI_DOWNLOAD_DIR") ]] || {
        echo 'The ~/Movies/Solus download link changed. Re-run the installer or restore the link.' >&2; exit 1;
    }
    [[ -d $SOLUS_RPI_DOWNLOAD_DIR && -w $SOLUS_RPI_DOWNLOAD_DIR ]] || {
        echo 'The download directory is unavailable or not writable.' >&2; exit 1;
    }
    export ANI_CLI_DOWNLOAD_DIR=$SOLUS_RPI_DOWNLOAD_DIR
fi
exec "$prefix/share/solus-rpi/solus" "$@"
