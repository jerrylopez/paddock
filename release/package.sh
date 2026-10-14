#!/usr/bin/env bash
# Assembles the files for one Paddock release:
#
#   release/package.sh <version> <base-url> <out-dir> <x86_64-binary> <aarch64-binary>
#
# <base-url> is where the files will be downloaded from, e.g.
# https://github.com/jerrylopez/paddock/releases/download/v0.1.0. Writes to <out-dir>:
#
#   paddock-x86_64-linux, paddock-aarch64-linux   the binaries
#   install.sh                                   release/install.sh, pinned to this release
#   SHA256SUMS                                   checksums of all of the above
set -euo pipefail

if [ $# -ne 5 ]; then
  sed -n '2,4p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
fi
version="$1" base_url="${2%/}" out="$3" x86_64="$4" aarch64="$5"

mkdir -p "$out"
cp "$x86_64" "$out/paddock-x86_64-linux"
cp "$aarch64" "$out/paddock-aarch64-linux"

sha256() { sha256sum "$1" | cut -d' ' -f1; }

sed -e "s|__VERSION__|$version|g" \
  -e "s|__BASE_URL__|$base_url|g" \
  -e "s|__SHA256_X86_64__|$(sha256 "$out/paddock-x86_64-linux")|g" \
  -e "s|__SHA256_AARCH64__|$(sha256 "$out/paddock-aarch64-linux")|g" \
  "$(dirname "$0")/install.sh" >"$out/install.sh"

if grep -n '__[A-Z0-9_]*__' "$out/install.sh" >&2; then
  echo "package.sh: placeholders left in install.sh" >&2
  exit 1
fi

(cd "$out" && sha256sum paddock-x86_64-linux paddock-aarch64-linux install.sh >SHA256SUMS)
