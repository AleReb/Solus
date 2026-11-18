#!/usr/bin/env bash
# Native Linux ARM64 build; upstream sources and Cargo.lock stay unchanged.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: build.sh [--jobs N] [--help]
Build Solus natively on 64-bit Raspberry Pi OS / Debian ARM64.
Defaults to one build job and opt-level=1 for the Solus package to reduce RAM use.
Requires Rust >= 1.88, a C compiler, pkg-config and OpenSSL development headers.
EOF
}

jobs=1
while (($#)); do
    case "$1" in
        --jobs) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; jobs=$2; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
    esac
done
[[ $jobs =~ ^[1-9][0-9]*$ ]] || { echo '--jobs must be a positive integer.' >&2; exit 2; }
[[ $(uname -s) == Linux && $(uname -m) == aarch64 ]] || {
    echo 'Use a 64-bit Linux ARM64 OS (aarch64), not 32-bit Raspberry Pi OS.' >&2; exit 1;
}

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
rustup_bin=$(command -v rustup || true)
[[ -n $rustup_bin || ! -x $HOME/.cargo/bin/rustup ]] || rustup_bin=$HOME/.cargo/bin/rustup
if [[ -n $rustup_bin ]]; then
    cargo_cmd=("$rustup_bin" run stable cargo)
    rustc_cmd=("$rustup_bin" run stable rustc)
else
    cargo_cmd=(cargo)
    rustc_cmd=(rustc)
fi
version=$("${rustc_cmd[@]}" --version) || {
    echo 'Install Rust stable first, or run scripts/raspberry-pi/install.sh.' >&2; exit 1;
}
read -r _ version_number _ <<< "$version"
IFS=. read -r major minor _ <<< "$version_number"
((major > 1 || (major == 1 && minor >= 88))) || {
    echo 'Rust >= 1.88 is required by the locked dependencies. Run the installer to install stable.' >&2; exit 1;
}
command -v cc >/dev/null
pkg-config --exists openssl || { echo 'Install pkg-config and libssl-dev.' >&2; exit 1; }

cd -- "$root"
# Only this invocation uses the lower optimization level; no Cargo.toml changes.
"${cargo_cmd[@]}" build --release --locked --jobs "$jobs" --target-dir "$root/target" \
    --config 'profile.release.package.solus.opt-level=1'
printf '\nBuilt: %s/target/release/solus\n' "$root"
