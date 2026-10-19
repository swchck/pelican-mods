#!/usr/bin/env bash
# Signs with the stable "Pelican Mods Self-Signed" identity when it exists (see
# make-signing-cert.sh), else falls back to ad-hoc signing from tauri.conf.json.
set -euo pipefail
NAME="Pelican Mods Self-Signed"
if security find-identity -p codesigning | grep -q "\"$NAME\""; then
  export APPLE_SIGNING_IDENTITY="$NAME"
else
  echo "warning: \"$NAME\" not found, signing ad-hoc; the Keychain asks again after each rebuild" >&2
fi
# the updater archive must be signed; local builds use the key from ~/.tauri, CI its secret
KEY="$HOME/.tauri/pelican-mods.key"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -f "$KEY" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$KEY"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$KEY.password")"
fi
"$(dirname "$0")/build-companion.sh"
# the companion ships as a resource only in real builds: tauri dev and cargo test would
# otherwise need a .NET toolchain just to compile
exec npx tauri build --config src-tauri/tauri.bundle.conf.json "$@"
