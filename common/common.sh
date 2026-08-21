# common.sh - Shared utilities for TA_enhanced module scripts
# Sourced by: service.sh, action.sh, uninstall.sh

# ABI Detection
# KSU/APatch set $ARCH during install; at runtime fall back to uname
if [ -n "$ARCH" ]; then
    case "$ARCH" in
        arm64)  ABI=arm64-v8a ;;
        arm)    ABI=armeabi-v7a ;;
        x64)    ABI=x86_64 ;;
        x86)    ABI=x86 ;;
        *)      ABI="" ;;
    esac
else
    case "$(uname -m)" in
        aarch64)       ABI=arm64-v8a ;;
        armv7*|armv8l) ABI=armeabi-v7a ;;
        x86_64)        ABI=x86_64 ;;
        i?86)          ABI=x86 ;;
        *)             ABI="" ;;
    esac
fi

# $MODDIR must be set by caller: MODDIR="${0%/*}" (standard KSU/Magisk convention)

# Binary Path
if [ -n "$MODDIR" ] && [ -n "$ABI" ]; then
    BIN="$MODDIR/bin/${ABI}/ta-enhanced"
fi
RP="/data/adb/tricky_store/ta-enhanced/bin/resetprop-rs"

# TrickyStore Paths
TS="/data/adb/modules/tricky_store"
TS_DIR="/data/adb/tricky_store"

# TEESimulator v4 owns /data/adb/teesim and its own WebUI.
. "$MODDIR/common/detect_engine.sh"

# Unified log directory -- shell and Rust daemon both log here
LOG_BASE_DIR="/data/adb/tricky_store/ta-enhanced/logs"
mkdir -p "$LOG_BASE_DIR" 2>/dev/null || true

# Simple Logger
# Writes to log file + logcat tag "TA_enhanced"
_log() {
    local level="$1" msg="$2"
    local ts
    ts=$(date '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "unknown")
    local line="[$ts] [$level] $msg"
    if [ -d "$LOG_BASE_DIR" ] && [ -w "$LOG_BASE_DIR" ]; then
        echo "$line" >> "$LOG_BASE_DIR/main.log" 2>/dev/null
    fi
    log -t "TA_enhanced" -p "${level%${level#?}}" "$msg" 2>/dev/null || true
}

# Root Manager Detection
# Sets MANAGER variable: "KSU", "APATCH", or "MAGISK"
detect_manager() {
    if [ "$KSU" = "true" ]; then
        MANAGER="KSU"
    elif [ "$APATCH" = "true" ]; then
        MANAGER="APATCH"
    else
        MANAGER="MAGISK"
    fi
}

# Config Reader (delegates to Rust binary)
read_config() {
    local key="$1" default="${2:-}"
    local val
    val=$("$BIN" config get "$key" 2>/dev/null)
    printf '%s' "${val:-$default}"
}

# Language Detection
# Read system locale, map to one of 23 supported locale codes
detect_language() {
    local device_lang lang_code

    device_lang=$(getprop ro.system.locale 2>/dev/null)
    [ -z "$device_lang" ] && device_lang=$(getprop persist.sys.locale 2>/dev/null)
    [ -z "$device_lang" ] && device_lang=$(getprop ro.product.locale 2>/dev/null)

    lang_code=$(printf '%s' "$device_lang" | sed 's/_/-/g')
    case "$lang_code" in
        zh-Hans*|zh-CN*) lang_code="zh-CN" ;;
        zh-Hant*|zh-TW*) lang_code="zh-TW" ;;
        pt-BR*) lang_code="pt-BR" ;;
        pt*) lang_code="pt-BR" ;;
        es-ES*|es*) lang_code="es-ES" ;;
        *-*) lang_code="${lang_code%%-*}" ;;
    esac
    case "$lang_code" in
        ar|az|bn|de|el|en|es-ES|fa|fr|id|it|ja|ko|pl|pt-BR|ru|th|tl|tr|uk|vi|zh-CN|zh-TW) ;;
        *) lang_code="en" ;;
    esac

    TA_LANG="$lang_code"
    export TA_LANG
}

# Property spoofing primitives (consumed by service.sh inline spoof block)
# Callers set _PROP_SPOOF_COUNT and _PROP_FAIL_COUNT before use

check_reset_prop() {
    [ -x "$RP" ] || { _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1)); return 1; }
    local name="$1" expected="$2"
    local val
    val=$(getprop "$name")
    [ -z "$val" ] && return 0
    [ "$val" = "$expected" ] && return 0
    if "$RP" "$name" "$expected" 2>/dev/null; then
        _PROP_SPOOF_COUNT=$((_PROP_SPOOF_COUNT + 1))
    else
        _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1))
        _log "ERROR" "Failed to spoof: $name"
    fi
}

contains_reset_prop() {
    [ -x "$RP" ] || { _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1)); return 1; }
    local name="$1" contains="$2" newval="$3"
    case "$(getprop "$name")" in
        *"$contains"*)
            if "$RP" "$name" "$newval" 2>/dev/null; then
                _PROP_SPOOF_COUNT=$((_PROP_SPOOF_COUNT + 1))
            else
                _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1))
                _log "ERROR" "Failed to spoof (contains): $name"
            fi
            ;;
    esac
}

replace_value_prop() {
    [ -x "$RP" ] || { _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1)); return 1; }
    local name="$1" search="$2" replace="$3"
    local val new_val
    val=$(getprop "$name")
    [ -z "$val" ] && return
    new_val=$(printf '%s' "$val" | sed "s|${search}|${replace}|g")
    [ "$val" = "$new_val" ] && return
    if "$RP" "$name" "$new_val" 2>/dev/null; then
        _PROP_SPOOF_COUNT=$((_PROP_SPOOF_COUNT + 1))
    else
        _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1))
        _log "ERROR" "Failed to replace in: $name"
    fi
}

ensure_prop() {
    [ -x "$RP" ] || { _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1)); return 1; }
    local name="$1" value="$2"
    [ -n "$(getprop "$name")" ] && return 0
    if "$RP" "$name" "$value" 2>/dev/null; then
        _PROP_SPOOF_COUNT=$((_PROP_SPOOF_COUNT + 1))
    else
        _PROP_FAIL_COUNT=$((_PROP_FAIL_COUNT + 1))
        _log "ERROR" "Failed to ensure: $name"
    fi
}
