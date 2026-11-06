#!/bin/sh

select_prop_resetprop() {
    local manager="$1" bundled="$2" adb_dir="${3:-/data/adb}" native=""

    case "$manager" in
        KSU)    native="$adb_dir/ksu/bin/resetprop" ;;
        APATCH) native="$adb_dir/ap/bin/resetprop" ;;
        MAGISK) native="$adb_dir/magisk/resetprop" ;;
    esac

    if [ -n "$native" ] && [ -x "$native" ]; then
        printf '%s\n' "$native"
    elif [ -x "$bundled" ]; then
        printf '%s\n' "$bundled"
    else
        printf 'No executable resetprop found for %s (bundled: %s)\n' "$manager" "$bundled" >&2
        return 1
    fi
}

set_reset_prop() {
    local name="$1" expected="$2" value

    if ! "$RP" -n "$name" "$expected"; then
        printf 'Failed to set property %s\n' "$name" >&2
        return 1
    fi
    if ! value=$("$RP" "$name"); then
        printf 'Failed to verify property %s\n' "$name" >&2
        return 1
    fi
    if [ "$value" != "$expected" ]; then
        printf 'Property %s did not retain the requested value\n' "$name" >&2
        return 1
    fi
}

check_reset_prop() {
    local name="$1" expected="$2" value

    if ! value=$("$RP" "$name"); then
        printf 'Failed to read property %s\n' "$name" >&2
        return 1
    fi
    if [ -z "$value" ] || [ "$value" = "$expected" ]; then
        return 0
    fi
    set_reset_prop "$name" "$expected"
}
