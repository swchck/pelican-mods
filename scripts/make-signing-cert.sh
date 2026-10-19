#!/usr/bin/env bash
# Creates a self-signed code-signing identity "Pelican Mods Self-Signed" in your login
# keychain. Builds signed with it keep the same designated requirement across versions,
# so the Keychain stops asking for your password after every rebuild or update.
#
# It also leaves ~/.tauri/pelican-mods-signing.p12 and its password next to it; put
# them into the GitHub secrets MACOS_CERTIFICATE (base64 of the .p12) and
# MACOS_CERTIFICATE_PASSWORD so release builds are signed with the same identity.
#
# codesign signs with an untrusted self-signed certificate just fine, so nothing here
# touches trust settings. Run it yourself, once.
set -euo pipefail

NAME="Pelican Mods Self-Signed"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
OUT="$HOME/.tauri"

if security find-identity -p codesigning | grep -q "\"$NAME\""; then
  echo "\"$NAME\" already exists, nothing to do."
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Apple's LibreSSL can't write the extensions below; prefer a real OpenSSL
OPENSSL="$(command -v /opt/homebrew/bin/openssl || command -v openssl)"
LEGACY=""
if "$OPENSSL" version | grep -qE '^OpenSSL [3-9]'; then
  # macOS `security import` only reads the old PKCS#12 ciphers
  LEGACY="-legacy"
fi

cat > "$TMP/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

"$OPENSSL" req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$TMP/cert.cnf" \
  -keyout "$TMP/key.pem" -out "$TMP/cert.pem" 2>/dev/null

mkdir -p "$OUT"
chmod 700 "$OUT"
PASS="$("$OPENSSL" rand -hex 16)"
"$OPENSSL" pkcs12 -export $LEGACY -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -name "$NAME" \
  -out "$OUT/pelican-mods-signing.p12" -passout "pass:$PASS"
printf '%s' "$PASS" > "$OUT/pelican-mods-signing.p12.password"
chmod 600 "$OUT/pelican-mods-signing.p12" "$OUT/pelican-mods-signing.p12.password"

security import "$OUT/pelican-mods-signing.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign

security find-identity -p codesigning | grep "$NAME"
echo
echo "Done. Local builds: npm run app:build"
echo "GitHub secrets:"
echo "  MACOS_CERTIFICATE          = output of: base64 -i $OUT/pelican-mods-signing.p12"
echo "  MACOS_CERTIFICATE_PASSWORD = contents of $OUT/pelican-mods-signing.p12.password"
