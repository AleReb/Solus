#!/usr/bin/env bash
# Download an already-built ARM64 release, then use the existing installer.
set -euo pipefail
repo=AleReb/Solus
tag=latest
args=()
while (($#)); do
    case "$1" in
        --repo|--tag)
            [[ $# -ge 2 && -n $2 ]] || { echo "$1 requires a value" >&2; exit 2; }
            case "$1" in --repo) repo=$2 ;; --tag) tag=$2 ;; esac
            shift 2 ;;
        --binary|--sha256) echo 'The release supplies --binary and --sha256.' >&2; exit 2 ;;
        -h|--help)
            echo 'Usage: install-release.sh [--repo OWNER/REPO] [--tag TAG] [installer options]'
            echo 'Default: latest ARM64 release from AleReb/Solus. No Rust or local build required.'
            echo 'Example: bash scripts/raspberry-pi/install-release.sh --terminal foot'
            exit 0 ;;
        *) args+=("$1"); shift ;;
    esac
done
[[ $(uname -s) == Linux && $(uname -m) == aarch64 ]] || {
    echo 'Requires 64-bit Linux ARM64.' >&2; exit 1;
}
[[ $repo =~ ^[[:alnum:]_.-]+/[[:alnum:]_.-]+$ && $tag =~ ^[[:alnum:]_.-]+$ ]] || {
    echo 'Invalid repository or release tag.' >&2; exit 2;
}
command -v curl >/dev/null
if [[ $tag == latest ]]; then
    base=https://github.com/$repo/releases/latest/download
else
    base=https://github.com/$repo/releases/download/$tag
fi
tmp=$(mktemp -d)
trap 'rm -rf -- "$tmp"' EXIT
for asset in solus-linux-aarch64 SHA256SUMS-arm64.txt; do
    curl --fail --location --silent --show-error --retry 3 --connect-timeout 15 --max-time 180 \
        "$base/$asset" -o "$tmp/$asset"
done
sha=$(awk '$2 == "solus-linux-aarch64" {print $1}' "$tmp/SHA256SUMS-arm64.txt")
[[ $sha =~ ^[[:xdigit:]]{64}$ ]] || { echo 'Missing or invalid binary checksum.' >&2; exit 1; }
printf '%s  %s\n' "$sha" "$tmp/solus-linux-aarch64" | sha256sum --check --status
chmod 755 "$tmp/solus-linux-aarch64"
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
bash "$root/install.sh" --binary "$tmp/solus-linux-aarch64" --sha256 "$sha" "${args[@]}"
