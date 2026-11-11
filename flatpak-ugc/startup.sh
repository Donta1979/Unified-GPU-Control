#!/usr/bin/env bash
set -e

# Unified GPU Control Flatpak launcher
# The LACT daemon is managed independently.
# Never install, replace, or restart system services.

# Include libraries installed under /app/lib64 by Flatpak modules.
export LD_LIBRARY_PATH="/app/lib64:/app/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

exec /app/bin/ugc gui "$@"
