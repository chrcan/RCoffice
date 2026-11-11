#!/bin/bash
# Baut RC office als macOS-App ("RC office.app") als Universal Binary
# (Apple Silicon + Intel). Auf einem Mac im Projektordner ausfuehren: ./build-mac.sh
set -euo pipefail
cd "$(dirname "$0")/app"

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

# Fehlende Rust-Targets einmalig nachinstallieren
for t in "${TARGETS[@]}"; do
  if ! rustup target list --installed | grep -q "^$t$"; then
    echo "Installiere Rust-Target: $t"
    rustup target add "$t"
  fi
done

# Beide Architekturen bauen
for t in "${TARGETS[@]}"; do
  echo "Baue fuer $t ..."
  cargo build --release --target "$t"
done

# Universal Binary verschmelzen
mkdir -p target/universal-apple-darwin/release
lipo -create \
  "target/aarch64-apple-darwin/release/rcoffice" \
  "target/x86_64-apple-darwin/release/rcoffice" \
  -output "target/universal-apple-darwin/release/rcoffice"

# App-Bundle aufbauen
APP="../RC office.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "target/universal-apple-darwin/release/rcoffice" "$APP/Contents/MacOS/rcoffice"
if [ -f assets/AppIcon.icns ]; then
  cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
fi
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>RC office</string>
  <key>CFBundleDisplayName</key><string>RC office</string>
  <key>CFBundleIdentifier</key><string>at.rc-design.rcoffice</string>
  <key>CFBundleExecutable</key><string>rcoffice</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST

# Ad-hoc-Signatur: unter Apple Silicon zwingend notwendig, damit das Binary
# ueberhaupt startet. Ersetzt keine Developer-ID-Signatur fuer Gatekeeper.
codesign --force --deep --sign - "$APP" >/dev/null

echo "Fertig: $(cd .. && pwd)/RC office.app"
echo "Architekturen: $(lipo -archs "$APP/Contents/MacOS/rcoffice")"
echo "Beim ersten Start: Rechtsklick -> Öffnen (die App ist nicht von Apple signiert)."
