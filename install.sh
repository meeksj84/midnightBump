#!/usr/bin/env bash
# Stable entrypoint: install Midnight Bump on an already bootable Arch system.
set -euo pipefail
REPO="$(cd -- "$(dirname -- "$0")" && pwd -P)"
exec bash "$REPO/scripts/install-minimal-arch.sh" "$@"
