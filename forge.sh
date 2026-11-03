#!/bin/sh
set -eu

if [ $# -lt 1 ]; then
    echo "Uzycie: ./forge.sh <nazwa_pluginu> [crate1 crate2 ...]"
    echo "Przyklad: ./forge.sh recon reqwest scraper"
    exit 1
fi

NAME="$1"
shift
DIR="plugin_${NAME}"
ROOT="$(cd "$(dirname "$0")" && pwd)"

if [ -d "$ROOT/$DIR" ]; then
    echo "BLAD: $DIR juz istnieje."
    exit 1
fi

echo ">>> Kopiuje szablon do $DIR..."
cp -r "$ROOT/plugin_template" "$ROOT/$DIR"
sed -i "s/plugin_NAZWA/plugin_${NAME}/g" "$ROOT/$DIR/Cargo.toml"

if ! grep -q "\"$DIR\"" "$ROOT/Cargo.toml"; then
    echo ">>> Dodaje $DIR do workspace..."
    sed -i "/^]$/i\\    \"$DIR\"," "$ROOT/Cargo.toml"
fi

for CRATE in "$@"; do
    echo ">>> cargo add $CRATE..."
    cargo add "$CRATE" --manifest-path "$ROOT/$DIR/Cargo.toml"
done

echo ">>> Buduje $DIR..."
cargo build -p "plugin_${NAME}" --manifest-path "$ROOT/Cargo.toml"

NEXT_KEY=$(grep -c '^\[\[slot\]\]' "$ROOT/workspace.toml" 2>/dev/null || echo "0")
NEXT_KEY=$((NEXT_KEY + 1))
LABEL=$(echo "$NAME" | tr '[:lower:]' '[:upper:]')

if ! grep -q "binary = \"plugin_${NAME}\"" "$ROOT/workspace.toml"; then
    echo ">>> Rejestruje slot w workspace.toml..."
    cat >> "$ROOT/workspace.toml" <<SLOT

[[slot]]
key = "${NEXT_KEY}"
label = "${LABEL}"
binary = "plugin_${NAME}"
description = "Wygenerowany przez forge.sh"
SLOT
fi

echo ""
echo "=== GOTOWE ==="
echo "Plugin: $DIR"
echo "Slot:   ${NEXT_KEY}:${LABEL}"
echo "Wcisnij 'r' w TUI aby przeladowac workspace.toml"
