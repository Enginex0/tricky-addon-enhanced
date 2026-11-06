#!/bin/sh

RP="${RP_BIN:-resetprop}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
. "$SCRIPT_DIR/common/prop-handler.sh"

contains_reset_prop() {
    local NAME=$1
    local CONTAINS=$2
    local NEWVAL=$3
    local VALUE
    if ! VALUE=$("$RP" "$NAME"); then
        printf 'Failed to read property %s\n' "$NAME" >&2
        return 1
    fi
    case "$VALUE" in
        *"$CONTAINS"*) set_reset_prop "$NAME" "$NEWVAL" ;;
    esac
}

empty_reset_prop() {
    local NAME=$1
    local NEWVAL=$2
    local VALUE
    if ! VALUE=$(getprop "$NAME"); then
        printf 'Failed to read property %s\n' "$NAME" >&2
        return 1
    fi
    [ -n "$VALUE" ] && return 0
    set_reset_prop "$NAME" "$NEWVAL"
}

if [ -f "/data/adb/disable_prop_handler" ]; then
    exit 0
fi

if [ ! -f "/data/adb/disable_vbmeta_digest_spoof" ] && [ -f "/data/adb/boot_hash" ]; then
    hash_value=$(grep -v '^#' "/data/adb/boot_hash" | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')
    if [ -z "$hash_value" ]; then
        rm -f /data/adb/boot_hash || printf '%s\n' 'Failed to remove empty boot hash' >&2
    else
        set_reset_prop ro.boot.vbmeta.digest "$hash_value"
    fi
fi

check_reset_prop "ro.boot.vbmeta.device_state" "locked"
check_reset_prop "ro.boot.verifiedbootstate" "green"
check_reset_prop "ro.boot.flash.locked" "1"
check_reset_prop "ro.boot.veritymode" "enforcing"
check_reset_prop "ro.boot.warranty_bit" "0"
check_reset_prop "ro.warranty_bit" "0"
check_reset_prop "ro.debuggable" "0"
check_reset_prop "ro.force.debuggable" "0"
check_reset_prop "ro.secure" "1"
check_reset_prop "ro.adb.secure" "1"
check_reset_prop "ro.build.type" "user"
check_reset_prop "ro.build.tags" "release-keys"
check_reset_prop "ro.vendor.boot.warranty_bit" "0"
check_reset_prop "ro.vendor.warranty_bit" "0"
check_reset_prop "vendor.boot.vbmeta.device_state" "locked"
check_reset_prop "vendor.boot.verifiedbootstate" "green"
check_reset_prop "sys.oem_unlock_allowed" "0"

# MIUI specific
check_reset_prop "ro.secureboot.lockstate" "locked"

# Realme specific
check_reset_prop "ro.boot.realmebootstate" "green"
check_reset_prop "ro.boot.realme.lockstate" "1"

# Hide that we booted from recovery when magisk is in recovery mode
contains_reset_prop "ro.bootmode" "recovery" "unknown"
contains_reset_prop "ro.boot.bootmode" "recovery" "unknown"
contains_reset_prop "vendor.boot.bootmode" "recovery" "unknown"

# Custom ROM (LineageOS) identity scrub: only fires when LOS-only signatures are present
v=$("$RP" ro.product.vendor.name)
case "$v" in
    lineage_*) set_reset_prop ro.product.vendor.name "${v#lineage_}" ;;
esac

v=$("$RP" vendor.camera.aux.packagelist)
case "$v" in
    *org.lineageos*) set_reset_prop vendor.camera.aux.packagelist "com.android.camera" ;;
esac

v=$("$RP" persist.vendor.camera.privapp.list)
case "$v" in
    *org.lineageos*) set_reset_prop persist.vendor.camera.privapp.list "com.android.camera" ;;
esac

[ -n "$("$RP" init.svc.vendor.lineage_health)" ] && {
    setprop ctl.stop vendor.lineage_health || printf '%s\n' 'Failed to stop vendor.lineage_health' >&2
    "$RP" -d init.svc.vendor.lineage_health || printf '%s\n' 'Failed to delete init.svc.vendor.lineage_health' >&2
}

# Reset vbmeta related prop
empty_reset_prop "ro.boot.vbmeta.device_state" "locked"
empty_reset_prop "ro.boot.vbmeta.invalidate_on_error" "yes"
empty_reset_prop "ro.boot.vbmeta.avb_version" "1.0"
empty_reset_prop "ro.boot.vbmeta.hash_alg" "sha256"
empty_reset_prop "ro.boot.vbmeta.size" "4096"

exit 0
