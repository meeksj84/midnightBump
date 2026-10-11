#!/usr/bin/env bash

set -u

MAINTAIN_MODE=false

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"
PRE_UPGRADE_SCRIPT="$SCRIPT_DIR/pre-upgrade-check.sh"

show_help() {
    cat <<EOF
Midnight Bump System Maintenance

Usage:
  $0              Run read-only system audit
  $0 --maintain   Run backup, update, cleanup, and scrub
  $0 --clean      Same as --maintain
  $0 --help       Show this help
EOF
}

case "${1:-}" in
    "")
        ;;
    --maintain|--clean)
        MAINTAIN_MODE=true
        ;;
    --help|-h)
        show_help
        exit 0
        ;;
    *)
        printf 'Unknown option: %s\n\n' "$1" >&2
        show_help >&2
        exit 2
        ;;
esac

section() {
    echo
    echo "============================================================"
    echo "$1"
    echo "============================================================"
}

section "SYSTEM INFO"
echo "Host:   $(hostname)"
echo "Kernel: $(uname -r)"
echo "Date:   $(date)"

section "FAILED SYSTEM SERVICES"
systemctl --failed --no-pager

section "FAILED USER SERVICES"
systemctl --user --failed --no-pager

section "AVAILABLE PACKAGE UPDATES"
if command -v checkupdates >/dev/null 2>&1; then
    if ! checkupdates; then
        echo "No package updates found."
    fi
else
    echo "checkupdates not installed."
    echo "Install pacman-contrib if you want update checks without syncing."
fi

section "PACMAN ORPHANS"
orphans="$(pacman -Qtdq 2>/dev/null || true)"

if [[ -n "$orphans" ]]; then
    echo "$orphans"
    echo
    echo "Orphans are NOT automatically removed."
else
    echo "No orphaned packages."
fi

section "PACMAN CACHE"
sudo du -sh /var/cache/pacman/pkg 2>/dev/null || true

section "JOURNAL SIZE"
journalctl --disk-usage

section "ROOT FILESYSTEM"
df -h /

section "BOOT FILESYSTEM"
df -h /boot

if command -v btrfs >/dev/null 2>&1; then
    section "BTRFS FILESYSTEM USAGE"
    sudo btrfs filesystem usage /

    section "BTRFS DEVICE STATS"
    sudo btrfs device stats /

    section "BTRFS SCRUB STATUS"
    sudo btrfs scrub status / || true
fi

if [[ "$MAINTAIN_MODE" == true ]]; then
    section "PRE-UPGRADE HEALTH CHECK + BACKUP"

    if [[ ! -x "$PRE_UPGRADE_SCRIPT" ]]; then
        echo "ERROR: Pre-upgrade script not found or not executable:"
        echo "  $PRE_UPGRADE_SCRIPT"
        exit 1
    fi

    "$PRE_UPGRADE_SCRIPT"
    PRECHECK_RESULT=$?

    case "$PRECHECK_RESULT" in
        0)
            echo
            echo "Pre-upgrade check passed."
            ;;

        1)
            echo
            echo "The pre-upgrade check completed with warnings."

            if [[ ! -t 0 ]]; then
                echo "Non-interactive shell detected."
                echo "Maintenance cancelled so warnings can be reviewed first."
                exit 1
            fi

            read -r -p "Continue with system maintenance anyway? [y/N] " answer

            if [[ ! "$answer" =~ ^[Yy]$ ]]; then
                echo "Maintenance cancelled."
                exit 1
            fi
            ;;

        *)
            echo
            echo "Pre-upgrade backup or health check failed."
            echo "Maintenance cancelled."
            exit 1
            ;;
    esac

    section "SYSTEM UPDATE"
    sudo pacman -Syu

    if command -v paccache >/dev/null 2>&1; then
        section "PACMAN CACHE CLEANUP"
        sudo paccache -r
    else
        echo
        echo "paccache not found."
        echo "Install pacman-contrib to enable package-cache cleanup."
    fi

    if command -v btrfs >/dev/null 2>&1; then
        section "BTRFS SCRUB"

        if sudo btrfs scrub status / 2>/dev/null | grep -qi "running"; then
            echo "A Btrfs scrub is already running."
        else
            sudo btrfs scrub start /
            echo "Btrfs scrub started."
        fi
    fi

    section "POST-MAINTENANCE CHECK"
    systemctl --failed --no-pager

    echo
    systemctl --user --failed --no-pager

    echo
    echo "Maintenance complete."
else
    section "AUDIT COMPLETE"

    echo "No system changes were made."
    echo
    echo "To perform full maintenance, run:"
    echo "  $0 --maintain"
    echo
    echo "For all options:"
    echo "  $0 --help"
fi
