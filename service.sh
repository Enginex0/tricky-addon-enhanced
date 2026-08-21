MODPATH=${0%/*}
MODDIR="$MODPATH"
PATH=$MODPATH/common/bin:/data/adb/ap/bin:/data/adb/ksu/bin:/data/adb/magisk:$PATH
HIDE_DIR="/data/adb/modules/.TA_enhanced"
TSPA="/data/adb/modules/tsupport-advance"

. "$MODPATH/common/common.sh"
detect_manager

_log "INFO" "Service started (manager=$MANAGER)"

# Denylist merge: Magisk reads from --denylist. KSU/APatch have no flat
# enumeration API (per-package umount profiles only), so skip with a log.
add_denylist_to_target() {
    local target_file="$TS_DIR/target.txt"
    local tmp_file="${target_file}.tmp"
    local exclamation_target question_target existing denylist

    exclamation_target=$(grep '!' "$target_file" | sed 's/!$//')
    question_target=$(grep '?' "$target_file" | sed 's/?$//')
    existing=$(sed 's/[!?]$//' "$target_file")
    denylist=$(magisk --denylist ls 2>/dev/null | awk -F'|' '{print $1}' | grep -v "isolated")

    if ! printf "%s\n" "$existing" "$denylist" | sort -u > "$tmp_file"; then
        _log "ERROR" "Failed to write target.txt from denylist"
        rm -f "$tmp_file"
        return 1
    fi

    for pkg in $exclamation_target; do
        sed -i "s/^${pkg}$/${pkg}!/" "$tmp_file"
    done
    for pkg in $question_target; do
        sed -i "s/^${pkg}$/${pkg}?/" "$tmp_file"
    done

    mv "$tmp_file" "$target_file"
    if [ "$ENGINE" = "teesim" ] && "$BIN" automation profile-ready >/dev/null 2>&1; then
        "$BIN" automation sync-target >/dev/null 2>&1
    fi
}

# Security patch is handled by the daemon's SecurityPatchTask (with retries + bulletin fetch).
# Running `set` here would overwrite bulletin-fetched dates with stale device props.

# prop.sh runs with RP_BIN pointed at the bundled resetprop-rs to keep the
# upstream calling pattern intact while sourcing the property setter from
# the module bundle.
RP_BIN="$MODPATH/bin/$ABI/resetprop-rs" sh "$MODPATH/prop.sh" &


# TSupport-A Interop
if [ -d "$TSPA" ]; then
    touch "/storage/emulated/0/stop-tspa-auto-target" 2>/dev/null || true
elif [ ! -d "$TSPA" ] && [ -f "/storage/emulated/0/stop-tspa-auto-target" ]; then
    rm -f "/storage/emulated/0/stop-tspa-auto-target"
fi

# Magisk Module Hiding
# Dot-prefix hides from Magisk's module list scan (stable since Magisk v24+).
# service.sh re-copies on every boot so the hidden copy is always fresh.
if [ "$MANAGER" = "MAGISK" ]; then
    if [ "$ENGINE" = "teesim" ] && [ "$MODPATH" = "$HIDE_DIR" ]; then
        VISIBLE_DIR="/data/adb/modules/TA_enhanced"
        rm -rf "$VISIBLE_DIR"
        mkdir -p "$VISIBLE_DIR"
        if cp -af "$HIDE_DIR/." "$VISIBLE_DIR/" \
            && [ -x "$VISIBLE_DIR/bin/${ABI}/ta-enhanced" ] \
            && [ -f "$VISIBLE_DIR/webui/index.html" ]; then
            MODPATH="$VISIBLE_DIR"
            MODDIR="$VISIBLE_DIR"
            BIN="$VISIBLE_DIR/bin/${ABI}/ta-enhanced"
        else
            _log "ERROR" "Failed to migrate hidden addon for TEESimulator; keeping current path"
            rm -rf "$VISIBLE_DIR"
        fi
    fi
    if [ "$ENGINE" = "tricky_store" ] && [ -f "$MODPATH/action.sh" ] && [ "$MODPATH" != "$HIDE_DIR" ]; then
        _log "INFO" "Module hiding (Magisk)"
        rm -rf "$HIDE_DIR"
        mkdir -p "$HIDE_DIR"
        busybox chcon --reference="$MODPATH" "$HIDE_DIR" 2>/dev/null || true
        if ! cp -af "$MODPATH/." "$HIDE_DIR/"; then
            _log "ERROR" "Module hiding copy failed, using original path"
            rm -rf "$HIDE_DIR"
        else
            MODPATH="$HIDE_DIR"
            MODDIR="$MODPATH"
            BIN="$MODPATH/bin/${ABI}/ta-enhanced"
        fi
    fi
    if [ "$ENGINE" = "teesim" ] && [ -d "$HIDE_DIR" ] && [ "$MODPATH" != "$HIDE_DIR" ]; then
        rm -rf "$HIDE_DIR"
    fi
    [ -f "$TS_DIR/target_from_denylist" ] && add_denylist_to_target
else
    _log "INFO" "Denylist merge skipped: $MANAGER has no flat denylist API"
    [ -d "$HIDE_DIR" ] && rm -rf "$HIDE_DIR"
fi

# Ensure system_app file exists for WebUI system app display
if [ ! -f "$TS_DIR/system_app" ]; then
    : > "$TS_DIR/system_app"
    for app in com.google.android.gms com.google.android.gsf com.android.vending \
               com.oplus.deepthinker com.heytap.speechassist com.coloros.sceneservice; do
        pm list packages -s 2>/dev/null | grep -q "package:$app" && echo "$app" >> "$TS_DIR/system_app"
    done
fi

# Snapshot module.prop for WebUI version display, then hide from manager UI.
# At boot the manager already registered the module before service.sh ran,
# so synchronous rm is safe. At hot-install the install transaction is still
# in flight, so defer rm until our installer parent exits (PPID polling
# matches upstream Tricky-Addon-Update-Target-List).
cp -f "$MODPATH/module.prop" "/data/adb/tricky_store/ta-enhanced/module.prop" 2>/dev/null || true
if [ "$ENGINE" = "teesim" ]; then
    : # Keep this addon's module metadata and WebUI separate from TEESimulator.
elif [ "$(getprop sys.boot_completed)" = "1" ]; then
    nohup sh -c "while kill -0 $PPID 2>/dev/null; do sleep 1; done; rm -f '$MODPATH/module.prop'" >/dev/null 2>&1 &
else
    rm -f "$MODPATH/module.prop"
fi

# Publish resetprop-rs to the daemon's expected path (rust/src/platform/props.rs:3).
# Re-copies on every boot so updates land.
mkdir -p /data/adb/tricky_store/ta-enhanced/bin
cp -f "$MODPATH/bin/$ABI/resetprop-rs" /data/adb/tricky_store/ta-enhanced/bin/resetprop-rs
chmod 755 /data/adb/tricky_store/ta-enhanced/bin/resetprop-rs

# TrickyStore can host this addon's UI. TEESimulator v4 already has its own
# webroot/action/banner, so leave all of those files untouched.
if [ "$ENGINE" = "tricky_store" ]; then
    if [ -f "$MODPATH/action.sh" ] && [ ! -e "$TS/action.sh" ]; then
        ln -s "$MODPATH/action.sh" "$TS/action.sh" 2>/dev/null || true
    fi
    if [ ! -e "$TS/webroot" ]; then
        ln -s "$MODPATH/webui" "$TS/webroot" 2>/dev/null || true
    fi
    if [ ! -e "$TS/banner.png" ] && [ -f "$MODPATH/banner.png" ]; then
        ln -s "$MODPATH/banner.png" "$TS/banner.png" 2>/dev/null || true
    fi
    if [ -f "$TS/module.prop" ] && ! grep -q "^banner=" "$TS/module.prop"; then
        sed -i '$ a\banner=banner.png' "$TS/module.prop" 2>/dev/null || true
    fi
elif [ ! -e "$MODPATH/webroot" ]; then
    # Publish the addon's WebUI under its own module ID.
    ln -s "$MODPATH/webui" "$MODPATH/webroot" 2>/dev/null || true
fi

if [ "$ENGINE" != "teesim" ] || "$BIN" automation profile-ready >/dev/null 2>&1; then
    "$BIN" automation export-target >/dev/null 2>&1 || true
fi

# Heavy support work waits for boot completion in a background subshell —
# pm and the rust binary are not prop-spoof critical, so backgrounding them
# never affects stealth.
(
    _log "INFO" "Waiting for boot completion"
    timeout 120 getprop -w sys.boot_completed 2>/dev/null || {
        until [ "$(getprop sys.boot_completed)" = "1" ]; do
            sleep 5
        done
    }
    _log "INFO" "Boot completed"

    pm list packages -s 2>/dev/null | sed 's/^package://' | sort > "/data/adb/tricky_store/ta-enhanced/system_packages.txt"

    vbhash_enabled=$(read_config vbhash.enabled true)
    if [ "$vbhash_enabled" = "true" ]; then
        _log "INFO" "Running VBHash extraction"
        "$BIN" vbhash extract 2>/dev/null || _log "WARN" "VBHash extraction failed"
    else
        _log "INFO" "VBHash extraction disabled"
    fi

    _log "INFO" "Checking for conflicts at boot"
    "$BIN" conflict check 2>/dev/null || _log "WARN" "Conflicts detected, check conflict.log"
) &

# Create tmp directory (needed by action.sh for KSU WebUI APK download)
mkdir -p "$MODPATH/common/tmp"

# Xposed Detection (background)
"$BIN" status xposed-scan >> "$LOG_BASE_DIR/main.log" 2>&1 &

# Magisk/TrickyStore legacy hosting cleans the visible addon copy. With
# TEESimulator this addon remains separate and keeps its own module WebUI.
if [ "$ENGINE" = "tricky_store" ] && [ -f "$MODPATH/action.sh" ]; then
    rm -rf "/data/adb/modules/TA_enhanced"
fi

# Launch Daemon
_log "INFO" "Starting ta-enhanced daemon"
"$BIN" daemon --manager "$MANAGER" &
_log "INFO" "Daemon launched"

# Keybox boot-time retry burst (background): exponential backoff, daemon
# takes over on the configured schedule once these attempts exhaust.
if [ "$ENGINE" = "teesim" ] && ! "$BIN" automation profile-ready >/dev/null 2>&1; then
    : # Profile-pending TEESimulator installs remain read-only.
elif { [ "$ENGINE" = "tricky_store" ] && [ ! -f "/data/adb/tricky_store/keybox.xml" ]; } \
    || { [ "$ENGINE" = "teesim" ] && ! "$BIN" keybox validate >/dev/null 2>&1; }; then
    (
        for _delay in 30 60 120 240; do
            sleep "$_delay"
            if [ "$ENGINE" = "tricky_store" ]; then
                [ -f "/data/adb/tricky_store/keybox.xml" ] && exit 0
            else
                "$BIN" keybox validate >/dev/null 2>&1 && exit 0
            fi
            timeout 10 "$BIN" keybox fetch 2>/dev/null && exit 0
        done
    ) &
fi

# Magisk also gets the dot-prefix copy at $HIDE_DIR for belt-and-braces hiding;
# KSU/APatch rely solely on the module.prop deletion above.
