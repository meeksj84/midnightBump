#!/usr/bin/env bash
#
# Midnight Bump Pre-Upgrade Check
#
# Runs a complete read-only system health audit, then creates a
# timestamped pre-upgrade backup.
#
# Exit codes:
#   0 = health check clean and backup successful
#   1 = health warnings found, backup successful
#   2 = backup failed
#
set -uo pipefail

TIMESTAMP="$(date +%Y-%m-%d-%H%M%S)"
BACKUP_ROOT="${HOME}/Backups"
BACKUP_DIR="${BACKUP_ROOT}/system-update-${TIMESTAMP}"
REPORT_FILE="${BACKUP_DIR}/pre-upgrade-report.txt"
MIDNIGHT_BUMP_DIR="${HOME}/Projects/midnight-bump"

mkdir -p "$BACKUP_DIR"

# Save all output while still displaying it in the terminal.
exec > >(tee "$REPORT_FILE") 2>&1

printf 'Backup workspace: %s\n' "$BACKUP_DIR"
printf 'Health report:    %s\n' "$REPORT_FILE"

PASS=0
WARN=0
INFO=0

have() {
    command -v "$1" >/dev/null 2>&1
}

host_name() {
    if have hostnamectl; then
        hostnamectl --static 2>/dev/null && return
    fi
    if [[ -r /etc/hostname ]]; then
        cat /etc/hostname
        return
    fi
    uname -n 2>/dev/null || printf 'unknown\n'
}

heading() {
    printf '\n============================================================\n'
    printf '%s\n' "$1"
    printf '============================================================\n'
}

pass() {
    ((PASS+=1))
    printf '[PASS] %s\n' "$*"
}

warn() {
    ((WARN+=1))
    printf '[WARN] %s\n' "$*"
}

info() {
    ((INFO+=1))
    printf '[INFO] %s\n' "$*"
}

show_cmd() {
    local title="$1"
    shift
    printf '\n--- %s ---\n' "$title"
    "$@" 2>&1 || true
}

heading "MIDNIGHT BUMP SYSTEM HEALTH CHECK"
printf 'Host:       %s\n' "$(host_name)"
printf 'Date:       %s\n' "$(date)"
printf 'Kernel:     %s\n' "$(uname -r)"
printf 'Uptime:     %s\n' "$(uptime -p 2>/dev/null || true)"
printf 'Desktop:    %s\n' "${XDG_CURRENT_DESKTOP:-unknown}"
printf 'Session:    %s\n' "${XDG_SESSION_TYPE:-unknown}"

heading "1. FAILED SERVICES"

system_failed="$(systemctl --failed --no-legend --plain 2>/dev/null || true)"
if [[ -z "$system_failed" ]]; then
    pass "No failed system services."
else
    warn "Failed system services detected:"
    printf '%s\n' "$system_failed"
fi

user_failed="$(systemctl --user --failed --no-legend --plain 2>/dev/null || true)"
if [[ -z "$user_failed" ]]; then
    pass "No failed user services."
else
    warn "Failed user services detected:"
    printf '%s\n' "$user_failed"
fi

heading "2. CURRENT BOOT ERRORS"

boot_errors="$(journalctl -b -p err..alert --no-pager 2>/dev/null || true)"
if [[ -z "$boot_errors" || "$boot_errors" == "-- No entries --" ]]; then
    pass "No error-or-higher journal entries for this boot."
else
    known_boot_noise="$(grep -E 'virt/tdx: TDX not supported by the host platform|ACPI BIOS Error \(bug\): Could not resolve symbol \[\\_SB\.PCI0\.PB2\], AE_NOT_FOUND|ACPI Error: AE_NOT_FOUND, During name lookup/catalog|lenovo_wmi_gamezone .*platform_profile probe failed|nl80211: kernel reports: multicast RX registrations are not supported' <<<"$boot_errors" || true)"

    actionable_boot_errors="$(grep -Ev 'virt/tdx: TDX not supported by the host platform|ACPI BIOS Error \(bug\): Could not resolve symbol \[\\_SB\.PCI0\.PB2\], AE_NOT_FOUND|ACPI Error: AE_NOT_FOUND, During name lookup/catalog|lenovo_wmi_gamezone .*platform_profile probe failed|nl80211: kernel reports: multicast RX registrations are not supported' <<<"$boot_errors" | sed '/ kernel: *$/d' || true)"

    if [[ -n "$actionable_boot_errors" ]]; then
        warn "Current boot contains unrecognized error-or-higher journal entries:"
        printf '%s\n' "$actionable_boot_errors"
    else
        pass "No actionable error-or-higher journal entries detected."
    fi

    if [[ -n "$known_boot_noise" ]]; then
        info "Known platform/firmware boot messages were detected and not counted as warnings:"
        printf '%s\n' "$known_boot_noise"
    fi
fi

heading "3. KERNEL WARNINGS"

journalctl -k -b -p warning..alert --no-pager 2>/dev/null || true

heading "4. BOOT PERFORMANCE"

if have systemd-analyze; then
    systemd-analyze 2>/dev/null || true
    printf '\nSlowest services:\n'
    systemd-analyze blame 2>/dev/null | head -20 || true
else
    info "systemd-analyze not available."
fi

heading "5. STORAGE"

df -hT / /boot 2>/dev/null || df -hT / 2>/dev/null || true

if have findmnt; then
    boot_source="$(findmnt -n -o SOURCE /boot 2>/dev/null || true)"
    boot_opts="$(findmnt -n -o OPTIONS /boot 2>/dev/null || true)"

    if [[ -n "$boot_source" ]]; then
        info "/boot source: $boot_source"
        printf 'Options: %s\n' "$boot_opts"

        if [[ "$boot_opts" == *"fmask=0077"* && "$boot_opts" == *"dmask=0077"* ]]; then
            pass "/boot uses restrictive FAT masks (0077/0077)."
        elif [[ "$boot_opts" == *"vfat"* || "$(findmnt -n -o FSTYPE /boot 2>/dev/null)" == "vfat" ]]; then
            warn "/boot is FAT but does not appear to use fmask=0077,dmask=0077."
        fi
    else
        info "No separate /boot mount detected."
    fi
fi

if have btrfs && findmnt -n -o FSTYPE / 2>/dev/null | grep -qx btrfs; then
    printf '\nBtrfs usage:\n'
    btrfs filesystem usage / 2>/dev/null || true
fi

heading "6. ARCH PACKAGE HEALTH"

if have pacman; then
    printf 'Pacman database check:\n'
    pacman -Dk 2>&1 || true

    orphans="$(pacman -Qtdq 2>/dev/null || true)"
    if [[ -z "$orphans" ]]; then
        pass "No orphan packages."
    else
        info "Orphan packages:"
        printf '%s\n' "$orphans"
    fi

    if have checkupdates; then
        updates="$(checkupdates 2>/dev/null || true)"
        if [[ -z "$updates" ]]; then
            pass "No pending repository package updates detected."
        else
            info "Pending repository updates:"
            printf '%s\n' "$updates"
        fi
    else
        info "checkupdates not installed; skipping non-invasive update check."
    fi
fi

heading "7. HYPRLAND"

if have hyprctl && [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
    config_errors="$(hyprctl configerrors 2>/dev/null || true)"
    if [[ -z "$config_errors" ]]; then
        pass "Hyprland reports no configuration errors."
    else
        warn "Hyprland configuration errors:"
        printf '%s\n' "$config_errors"
    fi

    hyprctl version 2>/dev/null | head -12 || true
else
    info "Hyprland IPC unavailable in this shell/session."
fi

if systemctl --user cat hypridle.service >/dev/null 2>&1; then
    if systemctl --user is-active --quiet hypridle.service; then
        pass "hypridle.service is active."
    else
        warn "hypridle.service is installed but not active."
    fi

    hypridle_recent="$(journalctl --user -b -u hypridle.service --no-pager 2>/dev/null | grep -E 'No rules configured|Config has errors|found [0-9]+ rules' | tail -1 || true)"
    if [[ -n "$hypridle_recent" ]]; then
        printf '%s\n' "$hypridle_recent"
        if grep -qE 'No rules configured|Config has errors' <<<"$hypridle_recent"; then
            warn "Hypridle has reported configuration problems this boot."
        elif grep -qE 'found [1-9][0-9]* rules' <<<"$hypridle_recent"; then
            pass "Hypridle has active listener rules."
        fi
    fi
fi

heading "8. AUDIO / REALTIME"

if systemctl --user is-active --quiet pipewire.service; then
    pass "pipewire.service is active."
else
    warn "pipewire.service is not active."
fi

if systemctl --user is-active --quiet wireplumber.service; then
    pass "wireplumber.service is active."
else
    warn "wireplumber.service is not active."
fi

if pacman -Q pipewire-pulse >/dev/null 2>&1; then
    if systemctl --user is-active --quiet pipewire-pulse.socket || systemctl --user is-active --quiet pipewire-pulse.service; then
        pass "PipeWire PulseAudio compatibility layer is available."
    else
        warn "pipewire-pulse is installed but its user socket/service is not active."
    fi
else
    warn "pipewire-pulse is not installed; PulseAudio-compatible applications may not connect."
fi

if have wpctl; then
    default_sink="$(wpctl status 2>/dev/null | sed -n '/Sinks:/,/Sources:/p' | grep -m1 -E '^[[:space:]│├└]*\*[[:space:]]+[0-9]+\.' || true)"
    if [[ -n "$default_sink" ]]; then
        info "Default audio sink: $(sed 's/^[[:space:]│├└]*\*[[:space:]]*//' <<<"$default_sink")"
    else
        info "No active default audio sink was detected by wpctl."
    fi
fi

if pacman -Q rtkit >/dev/null 2>&1; then
    pass "rtkit package is installed."

    if systemctl is-active --quiet rtkit-daemon.service; then
        pass "rtkit-daemon is active."
    else
        info "rtkit-daemon is inactive; it may start on demand through D-Bus."
    fi
else
    warn "rtkit is not installed; PipeWire/WirePlumber may log RealtimeKit errors."
fi

rt_errors="$(journalctl --user --since "-10 minutes" --no-pager 2>/dev/null | grep -Ei 'RTKit error|RealtimeKit.*ServiceUnknown' | tail -10 || true)"
if [[ -z "$rt_errors" ]]; then
    pass "No RTKit ServiceUnknown errors found in the current user boot journal."
else
    warn "RTKit-related errors were logged this boot:"
    printf '%s\n' "$rt_errors"
fi

heading "9. GRAPHICS"

if have lspci; then
    lspci -k 2>/dev/null | grep -EA4 'VGA|3D|Display' || true
fi

amd_errors="$(journalctl -k -b --no-pager 2>/dev/null | grep -Ei 'amdgpu.*error|DMCUB.*error|Error getting DMUB|dpia_query.*fail' || true)"
if [[ -z "$amd_errors" ]]; then
    pass "No AMDGPU/DMUB/DMCUB errors detected."
else
    warn "AMDGPU errors detected:"
    printf '%s\n' "$amd_errors"
fi

if have nvidia-smi; then
    if nvidia-smi >/dev/null 2>&1; then
        pass "NVIDIA driver responds to nvidia-smi."
        nvidia-smi --query-gpu=name,driver_version,temperature.gpu,power.draw --format=csv,noheader 2>/dev/null || true
    else
        warn "nvidia-smi is installed but could not query the GPU."
    fi

    nvidia_serious="$(journalctl -k -b --no-pager 2>/dev/null | grep -Ei 'NVRM: Xid|GPU has fallen off the bus' || true)"
    if [[ -z "$nvidia_serious" ]]; then
        pass "No NVIDIA Xid/fallen-off-bus errors detected."
    else
        warn "Serious NVIDIA errors detected:"
        printf '%s\n' "$nvidia_serious"
    fi
fi

heading "10. LAPTOP / BATTERY"

battery_found=0
for bat in /sys/class/power_supply/BAT*; do
    [[ -d "$bat" ]] || continue
    battery_found=1

    printf '\nBattery: %s\n' "$(basename "$bat")"
    for f in status capacity cycle_count energy_full energy_full_design; do
        if [[ -r "$bat/$f" ]]; then
            printf '  %-20s %s\n' "$f:" "$(cat "$bat/$f")"
        fi
    done

    if [[ -r "$bat/charge_types" ]]; then
        charge_types="$(cat "$bat/charge_types")"
        printf '  %-20s %s\n' "charge_types:" "$charge_types"
        if [[ "$charge_types" == *"[Standard]"* ]]; then
            pass "Battery charge mode is Standard."
        elif [[ "$charge_types" == *"[Long_Life]"* ]]; then
            info "Battery charge mode is Long_Life."
        elif [[ "$charge_types" == *"[Fast]"* ]]; then
            info "Battery charge mode is Fast."
        fi
    fi
done

if (( ! battery_found )); then
    info "No battery detected; treating this as a desktop system."
fi

charge_warnings="$(journalctl -k -b --no-pager 2>/dev/null | grep -F 'unexpected charge_types' | tail -5 || true)"
if [[ -n "$charge_warnings" ]]; then
    current_standard=0
    for bat in /sys/class/power_supply/BAT*; do
        [[ -r "$bat/charge_types" ]] || continue
        [[ "$(cat "$bat/charge_types")" == *"[Standard]"* ]] && current_standard=1
    done

    if (( current_standard )); then
        info "Lenovo charge-mode warnings occurred earlier this boot, but the current mode is Standard."
    else
        warn "Lenovo charge-mode warnings were logged this boot:"
        printf '%s\n' "$charge_warnings"
    fi
fi

heading "11. NETWORK / FIRMWARE"

if [[ -r /usr/lib/firmware/regulatory.db ]]; then
    pass "Wireless regulatory database is present."
else
    warn "/usr/lib/firmware/regulatory.db is missing."
fi

nm_errors="$(journalctl -b --no-pager 2>/dev/null | grep -E 'NetworkManager.*error|wpa_supplicant.*error' | tail -10 || true)"
if [[ -n "$nm_errors" ]]; then
    info "Recent network messages containing 'error':"
    printf '%s\n' "$nm_errors"
fi

heading "12. SUMMARY"

printf '[PASS] %d checks\n' "$PASS"
printf '[WARN] %d checks\n' "$WARN"
printf '[INFO] %d notes\n' "$INFO"

HEALTH_WARNINGS="$WARN"

printf '\nHealth report saved to: %s\n' "$REPORT_FILE"

heading "13. PRE-UPGRADE BACKUP"

BACKUP_FAILED=0

printf '[1/5] Saving official package list...\n'
if pacman -Qqen > "$BACKUP_DIR/official-packages.txt"; then
    printf '[OK] Official package list saved.\n'
else
    printf '[ERROR] Could not save official package list.\n'
    BACKUP_FAILED=1
fi

printf '\n[2/5] Saving AUR/foreign package list...\n'
if pacman -Qqem > "$BACKUP_DIR/aur-packages.txt"; then
    printf '[OK] AUR/foreign package list saved.\n'
else
    printf '[ERROR] Could not save AUR/foreign package list.\n'
    BACKUP_FAILED=1
fi

printf '\n[3/5] Backing up user configuration...\n'

HOME_ITEMS=()

for item in \
    ".config" \
    ".local/bin" \
    ".local/share/applications" \
    ".local/share/fonts" \
    ".local/share/icons" \
    ".local/share/themes" \
    ".bashrc" \
    ".profile"
do
    if [[ -e "$HOME/$item" ]]; then
        HOME_ITEMS+=("$item")
    fi
done

if (( ${#HOME_ITEMS[@]} > 0 )); then
    if tar -C "$HOME" -czf "$BACKUP_DIR/home-config.tar.gz" \
        -- "${HOME_ITEMS[@]}"; then
        printf '[OK] User configuration backed up.\n'
    else
        printf '[ERROR] User configuration backup failed.\n'
        BACKUP_FAILED=1
    fi
else
    printf '[WARN] No expected home configuration paths were found.\n'
fi

printf '\n[4/5] Backing up Midnight Bump repository...\n'

if [[ -d "$MIDNIGHT_BUMP_DIR" ]]; then
    repo_parent="$(dirname "$MIDNIGHT_BUMP_DIR")"
    repo_name="$(basename "$MIDNIGHT_BUMP_DIR")"

    if tar -C "$repo_parent" -czf \
        "$BACKUP_DIR/midnight-bump.tar.gz" \
        "$repo_name"; then
        printf '[OK] Midnight Bump repository backed up.\n'
    else
        printf '[ERROR] Midnight Bump repository backup failed.\n'
        BACKUP_FAILED=1
    fi
else
    printf '[WARN] %s was not found.\n' "$MIDNIGHT_BUMP_DIR"
fi

printf '\n[5/5] Recording Midnight Bump Git state...\n'

if [[ -d "$MIDNIGHT_BUMP_DIR/.git" ]] && command -v git >/dev/null 2>&1; then
    git -C "$MIDNIGHT_BUMP_DIR" status --short --branch \
        > "$BACKUP_DIR/midnight-bump-git-status.txt" 2>&1 || true

    git -C "$MIDNIGHT_BUMP_DIR" rev-parse HEAD \
        > "$BACKUP_DIR/midnight-bump-commit.txt" 2>&1 || true

    printf '[OK] Git state recorded.\n'
else
    printf '[WARN] Midnight Bump Git metadata unavailable.\n'
fi

heading "14. BACKUP CONTENTS"

ls -lh "$BACKUP_DIR"

printf '\nTotal backup size:\n'
du -sh "$BACKUP_DIR"

heading "15. FINAL STATUS"

if (( BACKUP_FAILED )); then
    printf '[FAILED] One or more backup operations failed.\n'
    printf 'Do not perform the system upgrade until the backup problem is resolved.\n'
    exit 2
fi

if (( HEALTH_WARNINGS > 0 )); then
    printf '[REVIEW] Backup completed successfully, but %d health warning(s) were detected.\n' \
        "$HEALTH_WARNINGS"
    printf 'Review the health report before performing the system upgrade.\n'
    printf 'Backup: %s\n' "$BACKUP_DIR"
    exit 1
fi

printf '[READY] Health check passed and backup completed successfully.\n'
printf 'Backup: %s\n' "$BACKUP_DIR"
printf 'System is ready for the upgrade step.\n'

exit 0
