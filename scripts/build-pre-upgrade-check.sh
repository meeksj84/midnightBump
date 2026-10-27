#!/usr/bin/env bash
# Build only: no package installation, configuration changes, commit or push.
set -euo pipefail

usage() {
    printf 'Usage: %s [--help]\nBuild the Rust pre-upgrade-check executable offline for this checkout.\n' "$0"
}
if (( $# > 0 )); then
    if (( $# == 1 )) && [[ "$1" == --help || "$1" == -h ]]; then
        usage
        exit 0
    fi
    usage >&2
    exit 2
fi

SCRIPT_PATH="$(readlink -f -- "${BASH_SOURCE[0]}")"
PROJECT_DIR="$(dirname -- "$(dirname -- "$SCRIPT_PATH")")"
CRATE_DIR="$PROJECT_DIR/tools/pre-upgrade-check"

if ! command -v cargo >/dev/null 2>&1; then
    printf 'Rust/Cargo 1.70 or newer is required. Install or activate your toolchain, then rerun this command.\n' >&2
    exit 2
fi

cargo build --manifest-path "$CRATE_DIR/Cargo.toml" \
    --release --locked --offline --target-dir "$CRATE_DIR/target"
printf 'Built pre-upgrade-check. Run:\n  %q --help\n' "$PROJECT_DIR/scripts/pre-upgrade-check"
