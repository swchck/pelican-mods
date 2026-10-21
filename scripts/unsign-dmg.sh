#!/usr/bin/env bash
# Rewrites the disk image at $1 without its code signature, keeping the window layout.
#
# Tauri signs the .dmg with the same identity as the app. Gatekeeper checks a signed
# image as soon as it's opened and rejects one signed by a certificate Apple didn't issue,
# while an unsigned image just mounts. The app inside keeps its self-signed signature,
# which the Keychain needs to trust every version alike. codesign --remove-signature
# doesn't handle disk images, so the image is converted into a fresh one.
set -euo pipefail
dmg="$1"
tmp="$(mktemp -d)/unsigned.dmg"
hdiutil convert "$dmg" -format UDZO -imagekey zlib-level=9 -o "$tmp" -quiet
mv "$tmp" "$dmg"
if codesign -dv "$dmg" 2>/dev/null; then
  echo "error: $dmg is still signed" >&2
  exit 1
fi
