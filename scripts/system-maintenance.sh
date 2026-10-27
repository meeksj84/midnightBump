#!/usr/bin/env bash
# Compatibility entry point: no mode still means a read-only audit.
set -euo pipefail
SCRIPT_DIR="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"

has_mode=false
for argument in "$@"; do
    case "$argument" in
        --audit|--review|--backup|--maintain|--clean|--install) has_mode=true ;;
        --help|-h) printf 'Compatibility command: system-maintenance.sh defaults to --audit.\n\n' ;;
    esac
done
if [[ "$has_mode" == false ]]; then
    set -- --audit "$@"
fi
exec "$SCRIPT_DIR/pre-upgrade-check" "$@"
