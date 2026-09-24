#!/usr/bin/env bash
set -Eeuo pipefail

# Midnight Bump pre-upgrade backup
# Creates a dated backup bundle before running a system upgrade.

DATE="$(date +%F)"
BACKUP_ROOT="${HOME}/Backups"
BACKUP_DIR="${BACKUP_ROOT}/system-update-${DATE}"
MIDNIGHT_BUMP_DIR="${HOME}/Projects/midnight-bump"

echo "== Midnight Bump pre-upgrade backup =="
echo "Backup destination: ${BACKUP_DIR}"
echo

mkdir -p "${BACKUP_DIR}"

echo "[1/4] Saving official package list..."
pacman -Qqen > "${BACKUP_DIR}/official-packages.txt"

echo "[2/4] Saving AUR/foreign package list..."
pacman -Qqem > "${BACKUP_DIR}/aur-packages.txt"

echo "[3/4] Backing up user configuration..."
HOME_ITEMS=()

for item in \
    "${HOME}/.config" \
    "${HOME}/.local" \
    "${HOME}/.bashrc" \
    "${HOME}/.profile"
do
    if [[ -e "${item}" ]]; then
        HOME_ITEMS+=("${item}")
    fi
done

if (( ${#HOME_ITEMS[@]} > 0 )); then
    tar -czf "${BACKUP_DIR}/home-config.tar.gz" \
        --absolute-names \
        "${HOME_ITEMS[@]}"
else
    echo "WARNING: No expected home configuration paths were found."
fi

echo "[4/4] Backing up Midnight Bump..."
if [[ -d "${MIDNIGHT_BUMP_DIR}" ]]; then
    tar -czf "${BACKUP_DIR}/midnight-bump.tar.gz" \
        --absolute-names \
        "${MIDNIGHT_BUMP_DIR}"
else
    echo "WARNING: ${MIDNIGHT_BUMP_DIR} was not found."
fi

echo
echo "Backup complete."
echo
ls -lh "${BACKUP_DIR}"
echo
du -sh "${BACKUP_DIR}"
