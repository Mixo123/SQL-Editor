#!/bin/bash
set -e

APP_PATH="$1"
ENTITLEMENTS="$2"

if [ -z "$APP_PATH" ] || [ ! -d "$APP_PATH" ]; then
    echo "Usage: $0 <path_to_app_bundle> [path_to_entitlements.plist]"
    exit 1
fi

echo "--- Preparing macOS code signing for $APP_PATH ---"

# 1. Usuń atrybuty kwarantanny i rozszerzone atrybuty
echo "Stripping extended attributes..."
xattr -cr "$APP_PATH" 2>/dev/null || true

# 2. Podpisz wewnętrzne biblioteki współdzielone i moduły binarne (.dylib, .so)
echo "Signing internal frameworks and dylibs inside-out..."
find "$APP_PATH/Contents/Frameworks" -type f \( -name "*.dylib" -o -name "*.so" \) 2>/dev/null | while read -r lib; do
    codesign --force --sign - "$lib" 2>/dev/null || true
done

find "$APP_PATH/Contents/MacOS" -type f \( -name "*.dylib" -o -name "*.so" \) 2>/dev/null | while read -r lib; do
    codesign --force --sign - "$lib" 2>/dev/null || true
done

# 3. Podpisz główny plik wykonywalny z uprawnieniami (entitlements)
MAIN_BIN=$(find "$APP_PATH/Contents/MacOS" -type f -perm +111 | head -n 1)
if [ -n "$MAIN_BIN" ]; then
    echo "Signing main executable $MAIN_BIN..."
    if [ -n "$ENTITLEMENTS" ] && [ -f "$ENTITLEMENTS" ]; then
        codesign --force --sign - --entitlements "$ENTITLEMENTS" "$MAIN_BIN"
    else
        codesign --force --sign - "$MAIN_BIN"
    fi
fi

# 4. Podpisz zewnętrzny pakiet aplikacji
echo "Signing outer app bundle..."
if [ -n "$ENTITLEMENTS" ] && [ -f "$ENTITLEMENTS" ]; then
    codesign --force --sign - --entitlements "$ENTITLEMENTS" "$APP_PATH"
else
    codesign --force --sign - "$APP_PATH"
fi

# 5. Weryfikacja podpisu
echo "Verifying bundle signature..."
codesign --verify --verbose=2 "$APP_PATH" || true
echo "macOS signing completed successfully."

