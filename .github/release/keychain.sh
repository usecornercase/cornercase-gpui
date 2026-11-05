#!/bin/sh
set -eu

fail() { echo "::error::$*"; exit 1; }

keychain=$RUNNER_TEMP/signing.keychain-db

create() {
  if [ -z "${MACOS_CERTIFICATE:-}" ]; then
    [ "$GITHUB_EVENT_NAME" != push ] || fail "a release is signed, and the release environment has no MACOS_CERTIFICATE"
    echo "no certificate: the bundle is signed ad hoc"
    return
  fi
  password=$(openssl rand -hex 24)
  security create-keychain -p "$password" "$keychain"
  security set-keychain-settings -lut 3600 "$keychain"
  security unlock-keychain -p "$password" "$keychain"
  certificate=$RUNNER_TEMP/certificate.p12
  (umask 077 && printf '%s' "$MACOS_CERTIFICATE" | base64 --decode > "$certificate")
  security import "$certificate" -k "$keychain" -f pkcs12 -P "${MACOS_CERTIFICATE_PASSWORD:-}" -T /usr/bin/codesign
  rm -f "$certificate"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" > /dev/null
  security list-keychains -d user -s "$keychain" "$HOME/Library/Keychains/login.keychain-db"
  identity=$(security find-identity -v -p codesigning "$keychain" \
    | sed -n 's/.*"\(Developer ID Application: .*\)"$/\1/p' | head -n 1)
  [ -n "$identity" ] || fail "MACOS_CERTIFICATE holds no valid Developer ID Application identity"
  echo "MACOS_SIGNING_IDENTITY=$identity" >> "$GITHUB_ENV"
}

delete() {
  [ ! -f "$keychain" ] || security delete-keychain "$keychain"
}

case "${1:-}" in
  create) create ;;
  delete) delete ;;
  *) fail "usage: keychain.sh create|delete" ;;
esac
