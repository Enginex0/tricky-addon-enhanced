MODPATH=${0%/*}
TS="/data/adb/modules/tricky_store"
LOG_BASE_DIR="/data/adb/tricky_store/ta-enhanced/logs"
BOOT_LOG="$LOG_BASE_DIR/boot.log"

# Defensive logger -- /data may not be fully decrypted
_pfd_log() {
    local ts msg
    ts=$(date '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "unknown")
    msg="[$ts] [POST-FS-DATA] $1"
    if [ -d "$LOG_BASE_DIR" ] && [ -w "$LOG_BASE_DIR" ]; then
        echo "$msg" >> "$BOOT_LOG" 2>/dev/null && return 0
    fi
    echo "$msg" >&2
}

_pfd_log "post-fs-data started"

# Wait for modules directory -- 9s cap (KSU/APatch hard 10s limit)
_wait_count=0
while [ -z "$(ls -A /data/adb/modules/ 2>/dev/null)" ]; do
    _wait_count=$((_wait_count + 1))
    [ "$_wait_count" -ge 18 ] && break
    sleep 0.5
done
_pfd_log "Modules directory ready (waited ${_wait_count} iterations)"
. "$MODPATH/common/detect_engine.sh"

# Self-removal only when neither supported engine is present.
if { [ ! -d "$TS" ] || [ -f "$TS/remove" ]; } && [ "$ENGINE" != "teesim" ]; then
    _pfd_log "No supported attestation engine - marking self for removal"
    if [ -f "$MODPATH/action.sh" ]; then
        # Magisk hidden module: recreate stub at real ID
        rm -rf "/data/adb/modules/TA_enhanced" 2>/dev/null
        mkdir -p "/data/adb/modules/TA_enhanced"
        touch "/data/adb/modules/TA_enhanced/remove"
    else
        touch "$MODPATH/remove"
    fi
fi

# Clean stale links only from TrickyStore. Never touch TEESimulator's WebUI.
if [ -d "$TS" ] && [ "$ENGINE" != "teesim" ]; then
    [ -L "$TS/webroot" ] && rm -f "$TS/webroot"
    [ -L "$TS/action.sh" ] && rm -f "$TS/action.sh"
    [ -L "$TS/banner.png" ] && rm -f "$TS/banner.png"
fi

# Root Manager Detection
if [ -n "$APATCH" ]; then
    MANAGER="APATCH"
elif [ -n "$KSU" ]; then
    MANAGER="KSU"
else
    MANAGER="MAGISK"
fi

# Persist manager for service.sh
echo "MANAGER=$MANAGER" > "$MODPATH/common/manager.sh"
chmod 755 "$MODPATH/common/manager.sh"
_pfd_log "Root manager detected: $MANAGER"

_pfd_log "post-fs-data completed"
