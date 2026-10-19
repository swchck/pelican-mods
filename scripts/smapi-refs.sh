#!/usr/bin/env bash
# Unpacks the assemblies the companion compiles against from a SMAPI release into $1,
# for machines without the game (CI). Usage: scripts/smapi-refs.sh <dir> [version]
set -euo pipefail
out="$1"
version="${2:-4.5.2}"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "https://github.com/Pathoschild/SMAPI/releases/download/$version/SMAPI-$version-installer.zip" -o "$tmp/smapi.zip"
unzip -q -j "$tmp/smapi.zip" "SMAPI $version installer/internal/macOS/install.dat" -d "$tmp"
mkdir -p "$out"
unzip -q -o "$tmp/install.dat" StardewModdingAPI.dll \
  smapi-internal/Newtonsoft.Json.dll smapi-internal/SMAPI.Toolkit.CoreInterfaces.dll -d "$out"
