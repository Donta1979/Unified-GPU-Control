#!/usr/bin/env bash
set -e

# Unified GPU Control Flatpak launcher
# The LACT daemon is managed independently.
# Never install, replace, or restart system services.

exec /app/bin/ugc gui "$@"
