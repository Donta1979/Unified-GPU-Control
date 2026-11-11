#!/usr/bin/env bash
set -euo pipefail

APP_ID="io.github.Donta1979.UnifiedGPUControl"
LACT_ID="io.github.ilya_zlobintsev.LACT"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE="${1:-$SCRIPT_DIR/Unified-GPU-Control-v1.8-x86_64.flatpak}"

ask() {
    local reply
    read -r -p "$1 [y/N] " reply
    [[ "$reply" =~ ^[Yy]$ ]]
}

installed() {
    flatpak info --user "$1" >/dev/null 2>&1 ||
    flatpak info --system "$1" >/dev/null 2>&1
}

echo "======================================"
echo " Unified GPU Control v1.8 Installer"
echo "======================================"
echo

if [[ $EUID -eq 0 ]]; then
    echo "ERROR: Run as your normal user, not root."
    exit 1
fi

if ! command -v flatpak >/dev/null 2>&1; then
    echo "ERROR: Flatpak is required."
    exit 1
fi

if [[ ! -f "$BUNDLE" ]]; then
    echo "ERROR: UGC bundle not found: $BUNDLE"
    exit 1
fi

echo "[1/5] LACT daemon"

if systemctl is-active --quiet lactd.service &&
   [[ -S /run/lactd.sock ]]; then
    echo "OK: LACT daemon is running."

elif systemctl cat lactd.service >/dev/null 2>&1; then
    echo "WARNING: Existing LACT service is not fully operational."
    echo "Its configuration will not be replaced."
    echo "Check: systemctl status lactd.service"

else
    if ! installed "$LACT_ID"; then
        echo "LACT is not installed."

        if ask "Install LACT from Flathub?"; then
            flatpak install --system flathub "$LACT_ID"
        fi
    fi

    if installed "$LACT_ID"; then
        if ask "Launch LACT's official service setup?"; then
            flatpak run "$LACT_ID"
        fi
    fi
fi

echo
echo "[2/5] MangoHud"

if command -v mangohud >/dev/null 2>&1; then
    echo "OK: MangoHud is installed."
else
    echo "MangoHud is not installed."

    if ask "Open Bazzite's package installer instructions?"; then
        echo "MangoHud installation depends on the host distribution."
        echo "For Bazzite, consult its supported package installation"
        echo "method before layering additional packages."
    fi
fi

echo
echo "[3/5] Astral Guard"

if systemctl is-active --quiet astral-guard.timer; then
    echo "OK: Astral Guard timer is active."
else
    echo "NOTICE: Astral Guard is not active."
    echo "It is optional and requires supported ASUS Astral hardware."
    echo "Automatic installation is not enabled in this release."
fi

echo
echo "[4/5] Unified GPU Control"

if flatpak info --user "$APP_ID" >/dev/null 2>&1; then
    echo "UGC is already installed for this user."

    echo "Checking bundled version..."

    TEMP_REPO="$(mktemp -d)"
    trap 'rm -rf "$TEMP_REPO"' EXIT

    ostree init --repo="$TEMP_REPO" --mode=archive-z2
    flatpak build-import-bundle "$TEMP_REPO" "$BUNDLE" >/dev/null

    INSTALLED_COMMIT="$(flatpak info --user --show-commit "$APP_ID")"
    BUNDLE_REF="app/$APP_ID/x86_64/master"
BUNDLE_COMMIT="$(ostree --repo="$TEMP_REPO" rev-parse "$BUNDLE_REF")"

    if [[ "$INSTALLED_COMMIT" == "$BUNDLE_COMMIT" ]]; then
        echo "OK: This exact UGC version is already installed."
        echo "No update necessary."
    elif ask "A different bundled version was detected. Install/update UGC?"; then
        flatpak install --user --or-update --bundle "$BUNDLE"
    else
        echo "Existing installation preserved."
    fi
elif flatpak info --system "$APP_ID" >/dev/null 2>&1; then
    echo "UGC is installed system-wide."
    echo "The existing system installation will be preserved."
    echo "No separate user installation will be created."
else
    if ask "Install Unified GPU Control?"; then
        flatpak install --user --or-update --bundle "$BUNDLE"
    else
        echo "UGC installation skipped."
    fi
fi

echo
echo "===== UGC Astral status integration ====="

HELPER_DIR="$SCRIPT_DIR/host-helper"

if [[ -f "$HELPER_DIR/astral-status-publisher.py" ]] &&
   [[ -f "$HELPER_DIR/systemd/ugc-astral-status.service" ]] &&
   [[ -f "$HELPER_DIR/systemd/ugc-astral-status.timer" ]]; then

    mkdir -p "$HOME/.local/libexec" "$HOME/.config/systemd/user"

    install -m 755 \
        "$HELPER_DIR/astral-status-publisher.py" \
        "$HOME/.local/libexec/ugc-astral-status-publisher"

    install -m 644 \
        "$HELPER_DIR/systemd/ugc-astral-status.service" \
        "$HOME/.config/systemd/user/ugc-astral-status.service"

    install -m 644 \
        "$HELPER_DIR/systemd/ugc-astral-status.timer" \
        "$HOME/.config/systemd/user/ugc-astral-status.timer"

    systemctl --user daemon-reload
    systemctl --user enable --now ugc-astral-status.timer
    systemctl --user start ugc-astral-status.service

    echo "OK: UGC Astral status publisher installed."
else
    echo "NOTICE: UGC Astral status publisher not configured."
    echo "The existing Astral Guard Manager and packaged helper files are required."
fi

echo
echo "[5/5] Final verification"

if flatpak info --user "$APP_ID" >/dev/null 2>&1; then
    echo "OK: Unified GPU Control installed."
else
    echo "WARNING: Unified GPU Control not installed for this user."
fi

if systemctl is-active --quiet lactd.service &&
   [[ -S /run/lactd.sock ]]; then
    echo "OK: LACT daemon operational."
else
    echo "WARNING: LACT daemon needs attention."
fi

if command -v mangohud >/dev/null 2>&1; then
    echo "OK: MangoHud available."
else
    echo "NOTICE: MangoHud unavailable."
fi

if systemctl is-active --quiet astral-guard.timer; then
    echo "OK: Astral Guard timer active."
else
    echo "NOTICE: Astral Guard not active."
fi

echo
echo "Installation checks completed."
echo "Existing GPU safety services were not deliberately changed."
