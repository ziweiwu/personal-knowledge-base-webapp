#!/usr/bin/env bash
# Builds KBView.app into target/macos/.
#
# The npm build must precede the cargo build: the server embeds web/dist with
# rust-embed, and the derive fails outright if that folder is not already there.

set -euo pipefail

# rustup lives in Homebrew's prefix and is not on PATH by default here.
export PATH="$HOME/.cargo/bin:$PATH"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BUILD_DIR="$REPO/target/macos"
APP="$BUILD_DIR/KBView.app"
BUNDLE_ID="com.ziweiwu.kbview"

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
cargo build --release -p kbview-server

step "Compiling the app wrapper"
mkdir -p "$BUILD_DIR"
swiftc -O -o "$BUILD_DIR/KBView" "$HERE"/Sources/*.swift
swiftc -O -o "$BUILD_DIR/make-icon" "$HERE/make-icon.swift"

step "Assembling the bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/Cargo.toml" | head -1)"
: "${version:=0.1.0}"
sed "s/__VERSION__/$version/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" > /dev/null

mv "$BUILD_DIR/KBView" "$APP/Contents/MacOS/KBView"
# Not "kbview": this filesystem is case insensitive, so that name is the same file as
# the wrapper's own executable and one silently overwrites the other.
cp "$REPO/target/release/kbview" "$APP/Contents/MacOS/kbview-server"
chmod +x "$APP/Contents/MacOS/KBView" "$APP/Contents/MacOS/kbview-server"

# That collision leaves one working binary under the surviving name and no error, so
# it is asserted rather than trusted.
if [ "$(stat -f '%i' "$APP/Contents/MacOS/KBView")" \
	= "$(stat -f '%i' "$APP/Contents/MacOS/kbview-server")" ]; then
	echo "build-app.sh: the wrapper and the server collided into one file" >&2
	exit 1
fi

"$BUILD_DIR/make-icon" "$BUILD_DIR/KBView.iconset"
iconutil -c icns "$BUILD_DIR/KBView.iconset" -o "$APP/Contents/Resources/KBView.icns"
rm -rf "$BUILD_DIR/KBView.iconset" "$BUILD_DIR/make-icon"

step "Signing"
# Ad-hoc. The inner binary is signed first: signing the bundle seals it, so doing it
# the other way round invalidates the outer signature immediately.
codesign --force --sign - --identifier "$BUNDLE_ID.server" "$APP/Contents/MacOS/kbview-server"
codesign --force --sign - --identifier "$BUNDLE_ID" "$APP"
codesign --verify --strict "$APP"

if [ "$package_zip" -eq 1 ]; then
	step "Packing for release"
	# ditto rather than `zip -r`: it preserves symlinks and extended attributes, which
	# this flat bundle does not yet need but would the moment it gained a framework, and
	# it is the form Apple's notarisation flow takes if a Developer ID is ever added.
	# The architecture is in the name because the bundled server is built for this
	# machine alone.
	archive="$BUILD_DIR/KBView-$version-$(uname -m).zip"
	rm -f "$archive"
	ditto -c -k --keepParent "$APP" "$archive"
	echo "$archive"
fi

if [ "$install_to_applications" -eq 1 ]; then
	step "Installing to /Applications"
	rm -rf "/Applications/KBView.app"
	cp -R "$APP" "/Applications/KBView.app"
	echo "Installed /Applications/KBView.app"
fi

step "Done"
echo "$APP"
echo "Open it with:  open '$APP'"
