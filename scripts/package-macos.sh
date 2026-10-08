#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script packages a macOS app bundle." >&2
  exit 1
fi
cargo build --release --locked
bundle="dist/PDFPilot.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp target/release/pdfpilot "$bundle/Contents/MacOS/PDFPilot"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>PDFPilot</string>
  <key>CFBundleDisplayName</key><string>PDFPilot</string>
  <key>CFBundleIdentifier</key><string>dev.pdfpilot.desktop</string>
  <key>CFBundleVersion</key><string>4</string>
  <key>CFBundleShortVersionString</key><string>0.2.2</string>
  <key>CFBundleExecutable</key><string>PDFPilot</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST
echo "Built $bundle"
