#!/usr/bin/env bash
# Compatibility entry point for the older supplied setup script.
# The repository root install.sh retains the fuller desktop/SDDM setup.
set -euo pipefail
SCRIPT_DIR="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
exec "$SCRIPT_DIR/pre-upgrade-check" --install "$@"
