#!/bin/sh
set -eu

if [ $# -lt 1 ]; then
    echo "Uzycie: ./forge.sh <nazwa_pluginu> [crate1 crate2 ...]"
    echo "Przyklad: ./forge.sh recon reqwest scraper"
    exit 1
fi

NAME="$1"
shift

if ! echo "$NAME" | grep -qE '^[a-z0-9_]+$'; then
    echo "BLAD: Nazwa pluginu moze zawierac wylacznie male litery, cyfry i podkreslenia (np. net_diag)."
    exit 1
fi

DIR="plugin_${NAME}"
ROOT="$(cd "$(dirname "$0")" && pwd)"

if [ -d "$ROOT/$DIR" ]; then
    echo "BLAD: $DIR juz istnieje."
    exit 1
fi

# Validate every argument and allocate a shortcut before changing files.
for CRATE in "$@"; do
    if ! echo "$CRATE" | grep -qE '^[a-zA-Z][a-zA-Z0-9_-]*$'; then
        echo "Invalid crate name: $CRATE" >&2
        exit 1
    fi
done
NEXT_KEY=""
for KEY in 1 2 3 4 5 6 7 8 9 a b c d e f g i j k l m n o s t u v w x y z; do
    if ! grep -qE "^key[[:space:]]*=[[:space:]]*\"$KEY\"" "$ROOT/workspace.toml"; then
        NEXT_KEY="$KEY"
        break
    fi
done
if [ -z "$NEXT_KEY" ]; then
    echo "No free plugin shortcut" >&2
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
    if ! echo "$CRATE" | grep -qE '^[a-zA-Z0-9_\-]+$'; then
        echo "BLAD: Nieprawidlowa nazwa crate: $CRATE"
        exit 1
    fi
    echo ">>> cargo add $CRATE..."
    cargo add "$CRATE" --manifest-path "$ROOT/$DIR/Cargo.toml"
done

echo ">>> Buduje $DIR..."
cargo build -p "plugin_${NAME}" --manifest-path "$ROOT/Cargo.toml"

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
