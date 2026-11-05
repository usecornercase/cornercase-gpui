#!/bin/sh
set -eu

fail() { echo "::error::$*"; exit 1; }

[ "$#" -eq 1 ] || fail "usage: notarize.sh DMG"
dmg=$1
[ -f "$dmg" ] || fail "missing $dmg"

work=$(mktemp -d)
mount=$work/mount
cleanup() {
  hdiutil detach -quiet "$mount" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

if [ -n "${NOTARY_PROFILE:-}" ]; then
  set -- --keychain-profile "$NOTARY_PROFILE"
else
  [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_KEY_ID:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ] \
    || fail "set NOTARY_PROFILE, or APPLE_API_KEY, APPLE_API_KEY_ID and APPLE_API_ISSUER"
  (umask 077 && printf '%s\n' "$APPLE_API_KEY" > "$work/key.p8")
  set -- --key "$work/key.p8" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER"
fi

assess() {
  verdict=$(spctl --assess --verbose=2 "$@" 2>&1) || fail "Gatekeeper refuses it: $verdict"
  echo "$verdict"
  case "$verdict" in *"source=Notarized Developer ID"*) ;; *) fail "not notarized: $verdict" ;; esac
}

result=$(xcrun notarytool submit "$dmg" "$@" --wait --timeout 45m --output-format json) || true
echo "$result"
id=$(printf '%s' "$result" | plutil -extract id raw -o - - 2>/dev/null) || fail "notarytool did not take the submission"
status=$(printf '%s' "$result" | plutil -extract status raw -o - -)
if [ "$status" != Accepted ]; then
  xcrun notarytool log "$id" "$@" || true
  fail "notarization ended $status"
fi

xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
assess --type open --context context:primary-signature "$dmg"
mkdir "$mount"
hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mount" "$dmg"
assess --type execute "$mount/cornercase.app"
