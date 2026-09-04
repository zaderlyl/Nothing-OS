#!/bin/bash
# Compile le pont et l'empaquette en .app.
#
# ScreenCaptureKit ne capture de façon fiable que depuis un bundle, et
# l'autorisation « Enregistrement de l'écran » se rattache au bundle.
# ⚠️  Reconstruire l'app change sa signature → il faut re-cocher
#     l'autorisation.  Ce script ne reconstruit donc QUE si la source a
#     changé (forcer avec:  bridge/build.sh --force).
set -e
cd "$(dirname "$0")"

APP="NothingBridge.app"
BIN="$APP/Contents/MacOS/NothingBridge"

if [ "$1" != "--force" ] && [ -x "$BIN" ] && [ "$BIN" -nt discord-bridge.swift ]; then
    echo "à jour : $PWD/$APP  (bridge/build.sh --force pour reconstruire)"
    exit 0
fi

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

codesign --force -s - "$APP" 2>/dev/null || true

echo "construit : $PWD/$APP"
echo "→ lance :  bridge/run.sh"
