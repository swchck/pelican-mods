#!/usr/bin/env bash
# Creates a self-signed code-signing identity in your login keychain and trusts it for
# code signing. Release builds signed with it keep the same identity across rebuilds,
# so macOS stops asking for the Keychain password every time the app reads its secrets.
#
# Run it yourself, once: macOS asks for your password to trust the certificate.
set -euo pipefail

NAME="Pelican Mods Dev"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
  echo "\"$NAME\" already exists, nothing to do."
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Apple's LibreSSL lacks -addext; prefer a real OpenSSL when one is installed
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

PASS="$("$OPENSSL" rand -hex 16)"
"$OPENSSL" pkcs12 -export $LEGACY -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -name "$NAME" \
  -out "$TMP/identity.p12" -passout "pass:$PASS"

security import "$TMP/identity.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign
echo "Trusting the certificate for code signing; macOS will ask for your password."
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$TMP/cert.pem"

security find-identity -v -p codesigning | grep "$NAME"
echo "Done. Build with: npm run app:build"
