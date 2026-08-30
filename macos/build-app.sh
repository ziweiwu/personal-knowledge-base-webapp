#!/usr/bin/env bash
# Builds KBViewer.app into target/macos/.
#
# The npm build must precede the cargo build: the server embeds web/dist with
# rust-embed, and the derive fails outright if that folder is not already there.

set -euo pipefail

# rustup lives in Homebrew's prefix and is not on PATH by default here.
export PATH="$HOME/.cargo/bin:$PATH"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BUILD_DIR="$REPO/target/macos"
APP="$BUILD_DIR/KBViewer.app"
BUNDLE_ID="com.ziweiwu.kbviewer"

install_to_applications=0
package_zip=0
for argument in "$@"; do
	case "$argument" in
		--install) install_to_applications=1 ;;
		--zip) package_zip=1 ;;
		-h|--help)
			echo "usage: build-app.sh [--install] [--zip]"
			echo "  --install   also copy the built app to /Applications"
			echo "  --zip       also pack the signed bundle for a GitHub release"
			exit 0
			;;
		*) echo "build-app.sh: unknown option $argument" >&2; exit 2 ;;
	esac
done

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

step "Building the frontend"
cd "$REPO/web"
if [ ! -d node_modules ]; then
	npm ci
fi
npm run build

step "Building the server"
cd "$REPO"
cargo build --release -p kbviewer-server

step "Compiling the app wrapper"
mkdir -p "$BUILD_DIR"
swiftc -O -o "$BUILD_DIR/KBViewer" "$HERE"/Sources/*.swift
swiftc -O -o "$BUILD_DIR/make-icon" "$HERE/make-icon.swift"

step "Assembling the bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/Cargo.toml" | head -1)"
: "${version:=0.1.0}"
sed "s/__VERSION__/$version/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" > /dev/null

mv "$BUILD_DIR/KBViewer" "$APP/Contents/MacOS/KBViewer"
# Not "kbviewer": this filesystem is case insensitive, so that name is the same file as
# the wrapper's own executable and one silently overwrites the other.
cp "$REPO/target/release/kbviewer" "$APP/Contents/MacOS/kbviewer-server"
chmod +x "$APP/Contents/MacOS/KBViewer" "$APP/Contents/MacOS/kbviewer-server"

# That collision leaves one working binary under the surviving name and no error, so
# it is asserted rather than trusted.
if [ "$(stat -f '%i' "$APP/Contents/MacOS/KBViewer")" \
	= "$(stat -f '%i' "$APP/Contents/MacOS/kbviewer-server")" ]; then
	echo "build-app.sh: the wrapper and the server collided into one file" >&2
	exit 1
fi

"$BUILD_DIR/make-icon" "$BUILD_DIR/KBViewer.iconset"
iconutil -c icns "$BUILD_DIR/KBViewer.iconset" -o "$APP/Contents/Resources/KBViewer.icns"
rm -rf "$BUILD_DIR/KBViewer.iconset" "$BUILD_DIR/make-icon"

step "Signing"
# Ad-hoc. The inner binary is signed first: signing the bundle seals it, so doing it
# the other way round invalidates the outer signature immediately.
codesign --force --sign - --identifier "$BUNDLE_ID.server" "$APP/Contents/MacOS/kbviewer-server"
codesign --force --sign - --identifier "$BUNDLE_ID" "$APP"
codesign --verify --strict "$APP"

if [ "$package_zip" -eq 1 ]; then
	step "Packing for release"
	# ditto rather than `zip -r`: it preserves symlinks and extended attributes, which
	# this flat bundle does not yet need but would the moment it gained a framework, and
	# it is the form Apple's notarisation flow takes if a Developer ID is ever added.
	# The architecture is in the name because the bundled server is built for this
	# machine alone.
	archive="$BUILD_DIR/KBViewer-$version-$(uname -m).zip"
	rm -f "$archive"
	ditto -c -k --keepParent "$APP" "$archive"
	echo "$archive"
fi

if [ "$install_to_applications" -eq 1 ]; then
	step "Installing to /Applications"
	rm -rf "/Applications/KBViewer.app"
	cp -R "$APP" "/Applications/KBViewer.app"
	echo "Installed /Applications/KBViewer.app"
fi

step "Done"
echo "$APP"
echo "Open it with:  open '$APP'"
