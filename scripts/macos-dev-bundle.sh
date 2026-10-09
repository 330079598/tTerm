#!/bin/sh
# Cargo runner on macOS (see src-tauri/.cargo/config.toml). Runs a development
# build of tTerm from inside an app bundle: macOS only shows notifications
# from a bundle, and only leads a click on one back to it. Run as a bare
# binary, tTerm can only post them through AppleScript, and clicking those
# opens Script Editor. Anything else Cargo runs (tests) runs as is.
set -e

binary=$1
case $binary in
*/deps/* | */build/*) exec "$@" ;;
esac
[ "$(basename "$binary")" = tTerm ] || exec "$@"
shift

src_tauri=$(cd "$(dirname "$0")/../src-tauri" && pwd)

# `tauri dev` passes the merged config (tauri.dev.conf.json) in TAURI_CONFIG.
config_value() {
  printf "%s" "${TAURI_CONFIG:-"{}"}" | /usr/bin/plutil -extract "$1" raw -o - - 2>/dev/null || printf '%s' "$2"
}
identifier=$(config_value identifier com.stone.tTerm.dev)
name=$(config_value productName "tTerm Dev")

app="$(dirname "$binary")/bundle/dev/$name.app"
contents="$app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources"

plist="$contents/Info.plist"
cp "$src_tauri/Info.plist" "$plist"
set_key() {
  /usr/bin/plutil -replace "$1" "-$2" "$3" "$plist"
}
set_key CFBundleIdentifier string "$identifier"
set_key CFBundleName string "$name"
set_key CFBundleDisplayName string "$name"
set_key CFBundleExecutable string tTerm
set_key CFBundlePackageType string APPL
set_key CFBundleIconFile string icon
set_key NSHighResolutionCapable bool YES
cp "$src_tauri/icons/icon.icns" "$contents/Resources/icon.icns"

# A clone, so the copy costs nothing. Signing it ad hoc names the signature
# after the bundle identifier: the linker's signature is named after the
# binary, and the notification daemon refuses that.
rm -f "$contents/MacOS/tTerm"
cp -c "$binary" "$contents/MacOS/tTerm"
/usr/bin/codesign --force --sign - "$app" 2>/dev/null
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"

exec "$contents/MacOS/tTerm" "$@"
