#!/usr/bin/env bash
# Signs with the stable "Pelican Mods Dev" identity when it exists (see
# make-signing-cert.sh), else falls back to ad-hoc signing from tauri.conf.json.
set -euo pipefail
NAME="Pelican Mods Dev"
if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
  export APPLE_SIGNING_IDENTITY="$NAME"
else
  echo "warning: \"$NAME\" not found, signing ad-hoc; Keychain will ask again after each rebuild" >&2
fi
exec npx tauri build "$@"
