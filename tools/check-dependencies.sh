#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Checks the dependencies against the licence policy (docs/development.md):
#   1. The Rust dependencies against core/deny.toml: known vulnerabilities
#      (fetches the RustSec advisory database), licences, banned crates and
#      sources.
#   2. The native libraries the build fetches from their pinned sources and
#      links in (cmake/Keychain.cmake: QtKeychain, BSD-3-Clause): their
#      licence texts in the build's fetched sources (MITCAD_BUILD_DIR,
#      default build/dev; skipped before a configure has fetched them).
#   3. The HEIF decoder of image references (mitcad#850) in the build's
#      vcpkg tree: libheif and libde265 (LGPL) as shared libraries only,
#      no GPL encoder (x265, x264: libheif's encoder features stay off),
#      and the LGPL and GPL texts in their copyright files.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
(cd "$ROOT/core" && cargo deny --locked check)

BUILD=${MITCAD_BUILD_DIR:-$ROOT/build/dev}
# licence name file phrases...: the file has every phrase and no GPL text.
licence() {
  local name=$1 file=$2
  shift 2
  if [ ! -f "$file" ]; then
    echo "native: $name: $file not there (configure the build first): skipped"
    return
  fi
  for phrase in "$@"; do
    grep -qF -- "$phrase" "$file" || { echo "native: $name: '$phrase' is not in $file" >&2; exit 1; }
  done
  if grep -qiE 'GNU (Lesser |Library )?General Public License' "$file"; then
    echo "native: $name: $file is a GPL licence" >&2
    exit 1
  fi
  echo "native: $name: ok"
}
licence "QtKeychain (BSD-3-Clause)" "$BUILD/_deps/qtkeychain-src/COPYING" \
  "Redistribution and use in source and binary forms" \
  "Redistributions in binary form must reproduce the above copyright" \
  "may not be used to" \
  "endorse or promote products derived from this software without"

heif_checked=""
for tree in "$BUILD"/vcpkg_installed/*/; do
  [ -d "$tree/share/libheif" ] || continue
  triplet=$(basename "$tree")
  for encoder in x265 x264; do
    if [ -e "$tree/share/$encoder" ]; then
      echo "native: $triplet: the GPL encoder $encoder is installed; libheif's encoder features must stay off" >&2
      exit 1
    fi
  done
  for library in libheif libde265 heif de265; do
    if [ -e "$tree/lib/$library.a" ]; then
      echo "native: $triplet: $tree/lib/$library.a is a static library; libheif and libde265 (LGPL) are linked dynamically" >&2
      exit 1
    fi
  done
  # The licence texts the packages ship from these files: the LGPL, and
  # the GPL it builds on (LGPL-3.0 section 4b).
  for port in libheif libde265; do
    if ! grep -q "GNU LESSER GENERAL PUBLIC LICENSE" "$tree/share/$port/copyright"; then
      echo "native: $triplet: $port's licence text (share/$port/copyright) has no LGPL" >&2
      exit 1
    fi
  done
  if ! grep -q "GNU GENERAL PUBLIC LICENSE" "$tree/share/libheif/copyright" "$tree/share/libde265/copyright"; then
    echo "native: $triplet: neither libheif's nor libde265's licence text has the GPL the LGPL builds on; ship it with them" >&2
    exit 1
  fi
  echo "native: libheif and libde265 ($triplet): shared libraries, no GPL encoder, LGPL and GPL texts: ok"
  heif_checked=1
done
[ -n "$heif_checked" ] || echo "native: libheif: not in $BUILD/vcpkg_installed (not configured, or MITCAD_HEIF off): skipped"
