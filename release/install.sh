#!/usr/bin/env bash
# Installs Paddock __VERSION__.
#
#   curl -fsSL __BASE_URL__/install.sh | bash
#
# This file is generated for a single release by release/package.sh: the version, the
# download URL and the SHA-256 of each binary are written in below, so it only ever installs
# that one release. If the same binary is already installed it does nothing, so it's safe to
# re-run from .agents/setup.
#
# Set PADDOCK_INSTALL_DIR to install somewhere other than /usr/local/bin.
set -euo pipefail

# Everything runs from main, called on the last line, so a partly downloaded script never runs.
main() {
  local version="__VERSION__"
  local base_url="__BASE_URL__"
  local dir="${PADDOCK_INSTALL_DIR:-/usr/local/bin}"
  local target="$dir/paddock"

  [ "$(uname -s)" = Linux ] || die "Paddock runs on Linux; this is $(uname -s)"

  local arch sha256
  case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 sha256="__SHA256_X86_64__" ;;
    aarch64 | arm64) arch=aarch64 sha256="__SHA256_AARCH64__" ;;
    *) die "no Paddock build for $(uname -m) (available: x86_64, aarch64)" ;;
  esac

  if [ -f "$target" ] && [ "$(sha256 "$target")" = "$sha256" ]; then
    echo "paddock: $version already installed at $target"
    return
  fi

  local tmp
  tmp="$(mktemp)"
  # Expanded now on purpose: $tmp is local to main and gone by the time the trap runs.
  # shellcheck disable=SC2064
  trap "rm -f '$tmp'" EXIT
  curl -fsSL --retry 3 -o "$tmp" "$base_url/paddock-$arch-linux" ||
    die "could not download $base_url/paddock-$arch-linux"
  [ "$(sha256 "$tmp")" = "$sha256" ] || die "checksum mismatch for paddock-$arch-linux"

  local sudo=""
  if ! mkdir -p "$dir" 2>/dev/null || [ ! -w "$dir" ]; then
    command -v sudo >/dev/null || die "$dir is not writable and sudo is not available"
    sudo="sudo"
  fi
  $sudo mkdir -p "$dir"
  $sudo install -m 755 "$tmp" "$target"

  echo "paddock: installed $version at $target"
  case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "paddock: note: $dir is not on PATH" >&2 ;;
  esac
}

sha256() {
  sha256sum "$1" | cut -d' ' -f1
}

die() {
  echo "paddock: install failed: $*" >&2
  exit 1
}

main
