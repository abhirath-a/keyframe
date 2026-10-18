#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

binary=${1:?Usage: package-linux.sh BINARY x64|arm64}
architecture=${2:?Specify x64 or arm64}
case "$architecture" in
    x64) tool_arch=x86_64 ;;
    arm64) tool_arch=aarch64 ;;
    *) echo 'Unsupported Linux architecture' >&2; exit 1 ;;
esac
[[ $(uname -s) == Linux ]] || { echo 'Build AppImages on Linux.' >&2; exit 1; }
[[ -f "$binary" ]] || { echo "Missing binary: $binary" >&2; exit 1; }
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
mkdir -p dist target/packaging-tools
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
appdir="$stage/Keyframe.AppDir"
install -Dm755 "$binary" "$appdir/usr/bin/keyframe"
install -Dm644 packaging/linux/dev.keyframe.Keyframe.desktop "$appdir/usr/share/applications/dev.keyframe.Keyframe.desktop"
install -Dm644 packaging/keyframe.svg "$appdir/usr/share/icons/hicolor/scalable/apps/dev.keyframe.Keyframe.svg"
desktop-file-validate "$appdir/usr/share/applications/dev.keyframe.Keyframe.desktop"

tool="$PWD/target/packaging-tools/linuxdeploy-$tool_arch.AppImage"
curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$tool_arch.AppImage" -o "$tool"
chmod +x "$tool"

# winit loads these at runtime, so automatic ELF dependency discovery misses them.
libraries=()
for name in libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1 libxkbcommon.so.0 libxkbcommon-x11.so.0 libX11.so.6 libXcursor.so.1 libXrandr.so.2 libXi.so.6; do
    library=$(ldconfig -p | awk -v name="$name" '$1 == name && !found { print $NF; found=1 }')
    [[ -n "$library" ]] || { echo "Missing runtime library: $name" >&2; exit 1; }
    libraries+=(--library "$library")
done
# Extract-and-run works on build runners without FUSE. GPU drivers stay on the host.
export APPIMAGE_EXTRACT_AND_RUN=1
export LDAI_OUTPUT="$PWD/dist/keyframe-linux-$architecture.AppImage"
export LDAI_VERSION="$version"
"$tool" --appdir "$appdir" --executable "$appdir/usr/bin/keyframe" \
    --desktop-file "$appdir/usr/share/applications/dev.keyframe.Keyframe.desktop" \
    --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/dev.keyframe.Keyframe.svg" \
    "${libraries[@]}" --output appimage
test -s "$LDAI_OUTPUT"
chmod +x "$LDAI_OUTPUT"

# Keep a conventional desktop archive as well.
tar -czf "dist/keyframe-linux-$architecture-desktop.tar.gz" -C "$appdir/usr" .
