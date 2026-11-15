#!/usr/bin/env bash
set -euo pipefail

REPO="abdullah-binmadhi/Solus"
TAG="v1.0.7-rolling"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"

info()  { printf "\033[1;35m▸\033[0m %s\n" "$*"; }
ok()    { printf "\033[1;32m✓\033[0m %s\n" "$*"; }
warn()  { printf "\033[1;33m!\033[0m %s\n" "$*"; }
fail()  { printf "\033[1;31m✗\033[0m %s\n" "$*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}:${ARCH}" in
  Darwin:arm64|Darwin:aarch64) ASSET="solus-macos-aarch64" ;;
  Darwin:x86_64)               ASSET="solus-macos-x86_64" ;;
  Linux:x86_64|Linux:amd64)    ASSET="solus-linux-x86_64" ;;
  *)
    fail "Unsupported platform (${OS}/${ARCH}). Build from source with: cargo install --git https://github.com/${REPO}.git"
    ;;
esac

install_macos_deps() {
  if ! need_cmd brew; then
    info "Installing Homebrew..."
    if [[ "${EUID:-$(id -u)}" -ne 0 ]] && need_cmd sudo && [[ -e /dev/tty ]] && ! sudo -n true 2>/dev/null; then
      sudo -v < /dev/tty || true
    fi
    NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    if [[ -x /opt/homebrew/bin/brew ]]; then
      eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -x /usr/local/bin/brew ]]; then
      eval "$(/usr/local/bin/brew shellenv)"
    fi
  fi

  local formulas=()
  need_cmd mpv     || formulas+=(mpv)
  need_cmd ani-cli || formulas+=(ani-cli)
  need_cmd ffmpeg  || formulas+=(ffmpeg)
  need_cmd yt-dlp  || formulas+=(yt-dlp)

  if (( ${#formulas[@]} > 0 )); then
    info "Installing playback dependencies (${formulas[*]})..."
    brew install "${formulas[@]}"
  fi

  if ! need_cmd kitty && [[ ! -d "/Applications/kitty.app" ]]; then
    info "Installing Kitty terminal for full HD cover art..."
    brew install --cask kitty || warn "Could not auto-install Kitty (Solus will still run in your current terminal)."
  fi
}

install_ani_cli_from_git() {
  local bin_dir="$1"
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  info "Installing ani-cli..."
  git clone --depth 1 https://github.com/pystardust/ani-cli.git "${tmp_dir}/ani-cli" >/dev/null 2>&1
  install -m 755 "${tmp_dir}/ani-cli/ani-cli" "${bin_dir}/ani-cli"
  rm -rf "${tmp_dir}"
}

install_linux_deps() {
  local sudo_cmd=""
  if [[ "${EUID:-$(id -u)}" -ne 0 ]] && need_cmd sudo; then
    if ! sudo -n true 2>/dev/null; then
      if [[ -e /dev/tty ]]; then
        info "Administrator privileges required to install system packages (mpv, ffmpeg, yt-dlp, kitty)..."
        sudo -v < /dev/tty
      else
        fail "sudo requires an interactive terminal. Download and run ./install.sh locally instead."
      fi
    fi
    sudo_cmd="sudo"
  fi

  if need_cmd apt-get; then
    info "Installing dependencies via apt..."
    ${sudo_cmd} apt-get update -qq
    ${sudo_cmd} apt-get install -y -qq mpv ffmpeg curl git fzf yt-dlp kitty || true
  elif need_cmd pacman; then
    info "Installing dependencies via pacman..."
    ${sudo_cmd} pacman -Sy --needed --noconfirm mpv ffmpeg curl git fzf yt-dlp kitty ani-cli || true
  elif need_cmd dnf; then
    info "Installing dependencies via dnf..."
    ${sudo_cmd} dnf install -y -q mpv ffmpeg curl git fzf yt-dlp kitty || true
  elif need_cmd zypper; then
    info "Installing dependencies via zypper..."
    ${sudo_cmd} zypper install -y mpv ffmpeg curl git fzf yt-dlp kitty || true
  else
    warn "Unknown Linux package manager — please ensure mpv, ani-cli, ffmpeg, curl, and yt-dlp are installed."
  fi

  if ! need_cmd ani-cli && need_cmd git; then
    mkdir -p "${HOME}/.local/bin"
    install_ani_cli_from_git "${HOME}/.local/bin"
  fi
}

pick_install_dir() {
  if [[ -d "/opt/homebrew/bin" && -w "/opt/homebrew/bin" ]]; then
    echo "/opt/homebrew/bin"
  elif [[ -d "/usr/local/bin" && -w "/usr/local/bin" ]]; then
    echo "/usr/local/bin"
  else
    mkdir -p "${HOME}/.local/bin"
    echo "${HOME}/.local/bin"
  fi
}

ensure_path_rc() {
  local dir="$1"
  case ":${PATH}:" in
    *":${dir}:"*) return 0 ;;
  esac

  local line="export PATH=\"${dir}:\$PATH\""
  for rc in "${HOME}/.zshrc" "${HOME}/.bashrc" "${HOME}/.profile"; do
    if [[ -f "${rc}" ]] && ! grep -Fq "${dir}" "${rc}"; then
      printf '\n%s\n' "${line}" >> "${rc}"
    fi
  done
  export PATH="${dir}:${PATH}"
}

main() {
  info "Setting up Solus (${OS} ${ARCH})..."

  if [[ "${OS}" == "Darwin" ]]; then
    install_macos_deps
  else
    install_linux_deps
  fi

  local install_dir
  install_dir="$(pick_install_dir)"
  local target="${install_dir}/solus"
  local tmp_bin
  tmp_bin="$(mktemp)"

  info "Downloading ${ASSET} (${TAG})..."
  curl -fsSL "${BASE_URL}/${ASSET}" -o "${tmp_bin}"
  chmod +x "${tmp_bin}"
  if [[ "${OS}" == "Darwin" ]]; then
    xattr -d com.apple.quarantine "${tmp_bin}" >/dev/null 2>&1 || true
  fi
  mv -f "${tmp_bin}" "${target}"

  ensure_path_rc "${install_dir}"

  ok "Solus installed to ${target}"
  printf "\n  Run \033[1;36msolus\033[0m to launch (for HD artwork, open inside \033[1mKitty\033[0m).\n\n"
}

main "$@"
