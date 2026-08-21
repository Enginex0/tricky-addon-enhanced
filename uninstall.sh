MODPATH=${0%/*}
TS="/data/adb/modules/tricky_store"
. "$MODPATH/common/detect_engine.sh"
SCRIPT_DIR="/data/adb/tricky_store"
AUTOMATION_DIR="$SCRIPT_DIR/.automation"
TA_DIR="$SCRIPT_DIR/ta-enhanced"
LOG_DIR="$TA_DIR/logs"
UNINSTALL_LOG="$LOG_DIR/uninstall.log"

# Minimal logger
_uninstall_log() {
    ts=$(date '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "unknown")
    echo "[$ts] [UNINSTALL] $1" >> "$UNINSTALL_LOG" 2>/dev/null || echo "[$ts] [UNINSTALL] $1" >&2
}

_uninstall_log "Uninstall started"

# Stop Daemon -- Method 1: Rust daemon PID file
DAEMON_PID="$SCRIPT_DIR/ta-enhanced/daemon.pid"
if [ -f "$DAEMON_PID" ]; then
    d_pid=$(cat "$DAEMON_PID" 2>/dev/null)
    if [ -n "$d_pid" ] && kill -0 "$d_pid" 2>/dev/null; then
        kill "$d_pid" 2>/dev/null
        _uninstall_log "Stopped Rust daemon (pid=$d_pid)"
        sleep 1
    fi
    rm -f "$DAEMON_PID"
fi

# Method 1b: Legacy supervisor PID file (fallback mode)
SUPERVISOR_PID="$AUTOMATION_DIR/supervisor.pid"
if [ -f "$SUPERVISOR_PID" ]; then
    sup_pid=$(cat "$SUPERVISOR_PID" 2>/dev/null)
    if [ -n "$sup_pid" ] && kill -0 "$sup_pid" 2>/dev/null; then
        kill "$sup_pid" 2>/dev/null
        _uninstall_log "Stopped supervisor (pid=$sup_pid)"
        sleep 1
    fi
    rm -f "$SUPERVISOR_PID"
fi

# Method 2: pidof fallback (handles stale PID file)
for proc_name in ta-enhanced supervisor; do
    pid=$(pidof "$proc_name" 2>/dev/null)
    if [ -n "$pid" ]; then
        kill "$pid" 2>/dev/null
        _uninstall_log "Stopped $proc_name via pidof (pid=$pid)"
    fi
done

# Method 3: Rust binary daemon-stop (if available)
if [ -n "$MODPATH" ]; then
    ABI=""
    case "$(uname -m)" in
        aarch64)       ABI=arm64-v8a ;;
        armv7*|armv8l) ABI=armeabi-v7a ;;
        x86_64)        ABI=x86_64 ;;
        i?86)          ABI=x86 ;;
    esac
    BIN="$MODPATH/bin/${ABI}/ta-enhanced"
    if [ -x "$BIN" ]; then
        "$BIN" daemon-stop 2>/dev/null
        _uninstall_log "Sent daemon-stop via binary"
    fi
fi

# Re-enable TSupport-A
[ -f "/storage/emulated/0/stop-tspa-auto-target" ] && rm -f "/storage/emulated/0/stop-tspa-auto-target"

# Remove module residue
rm -rf "/data/adb/modules/.TA_enhanced"
rm -f "/data/adb/boot_hash"
if [ "$ENGINE" = "tricky_store" ]; then
    rm -f "$SCRIPT_DIR/security_patch_auto_config"
    rm -f "$SCRIPT_DIR/target_from_denylist"
    rm -f "$SCRIPT_DIR/system_app"
    rm -f "$SCRIPT_DIR/enhanced.conf"
    rm -f "$SCRIPT_DIR/.verbose"
    rm -f "$SCRIPT_DIR/devconfig.toml"
fi
rm -rf "/data/adb/modules/TA_enhanced"

# Restore TrickyStore description
DESC_BAK="$TA_DIR/description.bak"
if [ "$ENGINE" = "tricky_store" ] && [ -f "$DESC_BAK" ] && [ -f "$TS/module.prop" ]; then
    orig=$(cat "$DESC_BAK" 2>/dev/null)
    if [ -n "$orig" ]; then
        sed -i "s|^description=.*|description=${orig}|" "$TS/module.prop" 2>/dev/null
        _uninstall_log "Restored original description"
    fi
fi

if [ "$ENGINE" = "tricky_store" ] && [ -d "$TS" ]; then
    [ -L "$TS/webroot" ] && rm -f "$TS/webroot"
    [ -L "$TS/action.sh" ] && rm -f "$TS/action.sh"
    [ -L "$TS/banner.png" ] && rm -f "$TS/banner.png"
    if [ -f "$TS/module.prop" ]; then
        sed -i '/^banner=banner\.png$/d' "$TS/module.prop" 2>/dev/null
    fi
fi

# Clean status files
rm -f "$SCRIPT_DIR/.health_state"
rm -f "$TA_DIR/description.bak"
rm -f "$SCRIPT_DIR/.status_installed"
rm -f "$SCRIPT_DIR/.status_targets"

# Preserve keybox.xml (user-selected, never auto-delete)
_uninstall_log "Keybox preserved"

# Clean up ta-enhanced state (config, PID, lock, status, logs)
rm -rf "$TA_DIR"

# Clean up legacy automation
rm -rf "$AUTOMATION_DIR"

_uninstall_log "Uninstall completed"
