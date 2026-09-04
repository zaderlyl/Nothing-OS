#!/bin/bash
# Compile le pont et l'empaquette en .app — ScreenCaptureKit exige un
# bundle pour capturer l'écran de façon fiable, et l'autorisation
# « Enregistrement de l'écran » se rattache alors au bundle (elle survit
# aux recompilations).
set -e
cd "$(dirname "$0")"

APP="NothingBridge.app"
BIN="$APP/Contents/MacOS/NothingBridge"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

swiftc -O discord-bridge.swift -o "$BIN"

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>NothingBridge</string>
  <key>CFBundleDisplayName</key><string>Nothing OS — Pont Discord</string>
  <key>CFBundleIdentifier</key><string>os.nothing.bridge</string>
  <key>CFBundleExecutable</key><string>NothingBridge</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSScreenCaptureUsageDescription</key><string>Affiche la fenetre Discord dans Nothing OS.</string>
  <key>NSAppleEventsUsageDescription</key><string>Renvoie les clics et frappes vers Discord.</string>
</dict></plist>
EOF

# signature ad-hoc (stabilise l'identite pour TCC)
codesign --force --deep -s - "$APP" 2>/dev/null || true

echo "OK : $PWD/$APP"
echo "Lancer :  open $APP --args \"\$HOME/Documents\""
echo "Log    :  ~/Library/Logs/nothing-bridge.log"
