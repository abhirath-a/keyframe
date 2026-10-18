#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

binary=${1:?Usage: package-macos.sh BINARY arm64|x64}
architecture=${2:?Specify arm64 or x64}
case "$architecture" in arm64|x64) ;; *) echo 'Unsupported macOS architecture' >&2; exit 1 ;; esac
[[ $(uname -s) == Darwin ]] || { echo 'Build macOS packages on macOS.' >&2; exit 1; }
[[ -f "$binary" ]] || { echo "Missing binary: $binary" >&2; exit 1; }
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/Keyframe.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" dist
cp "$binary" "$app/Contents/MacOS/keyframe"
chmod 755 "$app/Contents/MacOS/keyframe"
cp packaging/macos/Info.plist "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"

# Generate a native icon from the shared artwork, using only macOS tools.
mkdir "$stage/Keyframe.iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" packaging/keyframe.png --out "$stage/Keyframe.iconset/icon_${size}x${size}.png" >/dev/null
    doubled=$((size * 2))
    sips -z "$doubled" "$doubled" packaging/keyframe.png --out "$stage/Keyframe.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$stage/Keyframe.iconset" -o "$app/Contents/Resources/Keyframe.icns"
plutil -lint "$app/Contents/Info.plist"
# Ad-hoc signing allows native Apple Silicon execution without CI secrets.
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
ditto -c -k --keepParent "$app" "dist/keyframe-macos-$architecture-app.zip"
pkgbuild --component "$app" --install-location /Applications \
    --identifier dev.keyframe.keyframe --version "$version" \
    "dist/keyframe-macos-$architecture.pkg"
pkgutil --payload-files "dist/keyframe-macos-$architecture.pkg" > "$stage/payload.txt"
grep -q 'Contents/MacOS/keyframe' "$stage/payload.txt"
