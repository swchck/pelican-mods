#!/usr/bin/env bash
# Builds the companion mod into companion/dist/PelicanCompanion. References come from
# the game install, or from SMAPI_DIR (see smapi-refs.sh) where there is none.
set -euo pipefail
cd "$(dirname "$0")/.."
args=(-c Release --nologo -v quiet)
if [ -n "${SMAPI_DIR:-}" ]; then
  args+=("-p:SmapiDir=$SMAPI_DIR")
fi
export DOTNET_CLI_TELEMETRY_OPTOUT=1
dotnet build companion/PelicanCompanion "${args[@]}"
test -f companion/dist/PelicanCompanion/PelicanCompanion.dll
