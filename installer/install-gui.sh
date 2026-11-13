#!/usr/bin/env bash
# KDE double-click entry: starts the existing installer in a persistent Konsole.
# Never requests root for the whole installer; individual operations ask when needed.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if ! command -v konsole >/dev/null 2>&1; then
    echo "Konsole is required for the KDE installer launcher." >&2
    exit 1
fi
exec konsole --hold -e /usr/bin/bash "$SCRIPT_DIR/install.sh"
