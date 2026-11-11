#!/usr/bin/env bash
# Wraps a built binary into "ZMK Battery.app" so macOS shows a Bluetooth permission prompt
# and hides the Dock icon.
#
# Usage: scripts/bundle-macos.sh [path/to/binary] [output-dir]
#   defaults: target/release/zmk-battery-applet, target/bundle
set -euo pipefail

cd "$(dirname "$0")/.."

BIN="${1:-target/release/zmk-battery-applet}"
OUT="${2:-target/bundle}"
APP="$OUT/ZMK Battery.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

if [[ ! -f "$BIN" ]]; then
    echo "binary not found: $BIN (run cargo build --release first)" >&2
    exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/zmk-battery-applet"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>ZMK Battery</string>
    <key>CFBundleDisplayName</key>
    <string>ZMK Battery</string>
    <key>CFBundleIdentifier</key>
    <string>de.aucubin.zmk-battery-applet</string>
    <key>CFBundleExecutable</key>
    <string>zmk-battery-applet</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>Reads the battery level of your connected ZMK keyboard halves.</string>
</dict>
</plist>
EOF

# Ad-hoc signature: enough for TCC to remember the Bluetooth permission locally.
codesign --force --sign - "$APP"

echo "$APP"
