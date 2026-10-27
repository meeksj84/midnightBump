#!/usr/bin/env bash
# Compatibility entry point: the original default remains audit plus backup.
set -euo pipefail
SCRIPT_DIR="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
exec "$SCRIPT_DIR/pre-upgrade-check" "$@"
