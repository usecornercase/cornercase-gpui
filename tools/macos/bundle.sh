#!/bin/sh
set -eu

fail() { echo "::error::$*"; exit 1; }

[ "$#" -eq 3 ] || fail "usage: bundle.sh VERSION NOTICES OUT_DIR"
version=$1
notices=$2
out=$3
identity=${MACOS_SIGNING_IDENTITY:--}
notarize=${MACOS_NOTARIZE:-}
[ -z "$notarize" ] || [ "$identity" != - ] || fail "MACOS_NOTARIZE needs MACOS_SIGNING_IDENTITY: Apple notarizes only Developer ID signatures"

sign() {
  if [ "$identity" = - ]; then
    codesign --force --sign - "$@"
  else
    codesign --force --sign "$identity" --timestamp "$@"
  fi
}

root=$(cd "$(dirname "$0")/../.." && pwd)
arm=$root/target/aarch64-apple-darwin/release/cornercase-gpui
intel=$root/target/x86_64-apple-darwin/release/cornercase-gpui
[ -f "$arm" ] || fail "missing $arm"
[ -f "$intel" ] || fail "missing $intel"

mkdir -p "$out"
out=$(cd "$out" && pwd)
stage=$out/stage
rm -rf "$stage"
app=$stage/cornercase.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

lipo -create "$arm" "$intel" -output "$app/Contents/MacOS/cornercase-gpui"
chmod 755 "$app/Contents/MacOS/cornercase-gpui"
archs=$(lipo -archs "$app/Contents/MacOS/cornercase-gpui")
case "$archs" in *arm64*x86_64* | *x86_64*arm64*) ;; *) fail "expected both architectures, got: $archs" ;; esac
for arch in arm64 x86_64; do
  minos=$(vtool -arch "$arch" -show-build "$app/Contents/MacOS/cornercase-gpui" | awk '$1 == "minos" { print $2 }')
  [ "$minos" = "12.0" ] || fail "the $arch slice targets macOS $minos, not 12.0"
done

cp "$root/assets/macos/Info.plist" "$app/Contents/Info.plist"
plutil -replace CFBundleShortVersionString -string "${version%%-*}" "$app/Contents/Info.plist"
plutil -replace CFBundleVersion -string "${version%%-*}" "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
cp "$root/assets/macos/cornercase-gpui.icns" "$app/Contents/Resources/"
"$root/.github/release/release.sh" licences "$app/Contents/Resources" "$notices"

sign --options runtime --entitlements "$root/assets/macos/cornercase-gpui.entitlements" "$app"
codesign --verify --strict --verbose=2 "$app"
codesign --display --verbose=2 "$app"
[ -z "$notarize" ] || "$root/tools/macos/notarize.sh" "$app"

ln -s /Applications "$stage/Applications"
dmg=$out/cornercase-gpui-universal-apple-darwin.dmg
rm -f "$dmg"
attempt=1
until hdiutil create -volname cornercase -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg"; do
  [ "$attempt" -lt 3 ] || fail "hdiutil create failed three times"
  attempt=$((attempt + 1))
  rm -f "$dmg"
  sleep 5
done
if [ "$identity" != - ]; then
  sign "$dmg"
  codesign --verify --strict --verbose=2 "$dmg"
fi
[ -z "$notarize" ] || "$root/tools/macos/notarize.sh" "$dmg"
hdiutil verify "$dmg"
rm -rf "$stage"
