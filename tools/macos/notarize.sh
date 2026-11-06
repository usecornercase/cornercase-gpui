#!/bin/sh
set -eu

fail() { echo "::error::$*"; exit 1; }

[ "$#" -eq 1 ] || fail "usage: notarize.sh APP_OR_DMG"
target=$1
[ -e "$target" ] || fail "missing $target"

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

stapler() {
  tries=1
  until xcrun stapler "$@"; do
    [ "$tries" -lt 5 ] || fail "stapler $1 kept failing"
    sleep $((tries * 20))
    tries=$((tries + 1))
  done
}

assess() {
  verdict=$(spctl --assess --verbose=2 "$@" 2>&1) || fail "Gatekeeper refuses it: $verdict"
  echo "$verdict"
  case "$verdict" in *"source=Notarized Developer ID"*) ;; *) fail "not notarized: $verdict" ;; esac
}

case "$target" in
  *.app)
    upload=$work/$(basename "$target").zip
    ditto -c -k --keepParent "$target" "$upload"
    ;;
  *.dmg) upload=$target ;;
  *) fail "notarize.sh takes an .app or a .dmg, not $target" ;;
esac

result=$(xcrun notarytool submit "$upload" "$@" --wait --timeout 45m --output-format json) || true
echo "$result"
id=$(printf '%s' "$result" | plutil -extract id raw -o - - 2>/dev/null) || fail "notarytool did not take the submission"
status=$(printf '%s' "$result" | plutil -extract status raw -o - -)
xcrun notarytool log "$id" "$@" || true
[ "$status" = Accepted ] || fail "notarization ended $status"

stapler staple "$target"
stapler validate "$target"
case "$target" in
  *.app) assess --type execute "$target" ;;
  *.dmg)
    assess --type open --context context:primary-signature "$target"
    mkdir "$mount"
    hdiutil attach -quiet -nobrowse -readonly -mountpoint "$mount" "$target"
    stapler validate "$mount/cornercase.app"
    assess --type execute "$mount/cornercase.app"
    ;;
esac
