# install_func.sh - Installation functions for Tricky Addon Enhanced
# Sourced by customize.sh

AUTOMATION_DIR="/data/adb/tricky_store/.automation"
EXCLUDE_FILE="$AUTOMATION_DIR/exclude_patterns.txt"
KNOWN_PACKAGES="$AUTOMATION_DIR/known_packages.txt"
TARGET_FILE="/data/adb/tricky_store/target.txt"

initialize() {
    # Cleanup leftover from previous installs
    if [ -d "/data/adb/modules/$NEW_MODID" ]; then
        rm -rf "/data/adb/modules/$NEW_MODID"
    fi

    if [ "$ACTION" = "false" ]; then
        rm -f "$MODPATH/action.sh"
        NEW_MODID="$MODID"
    else
        mkdir -p "$COMPATH/update/common"
        cp "$COMPATH/.default" "$COMPATH/update/common/.default" 2>/dev/null || true
        cp "$MODPATH/uninstall.sh" "$COMPATH/update/uninstall.sh" 2>/dev/null || true
    fi

    cp "$MODPATH/module.prop" "$COMPATH/update/module.prop" 2>/dev/null || true

    # Set binary permissions (binaries stay at bin/$abi/)
    local abi
    abi=$(getprop ro.product.cpu.abi)
    if [ -d "$MODPATH/bin/$abi" ]; then
        set_perm_recursive "$MODPATH/bin/$abi" 0 2000 0755 0755
    fi

    # Remove the other ABI directory to save space
    for d in "$MODPATH/bin/"*; do
        [ -d "$d" ] || continue
        case "$d" in
            *"$abi") ;;
            *) rm -rf "$d" ;;
        esac
    done

    mkdir -p "$AUTOMATION_DIR"
}

choose_automation() {
    local vol_tmp="$TMPDIR/vol_key"
    local seconds="${1:-10}"
    local ge_pid=""

    : > "$vol_tmp"
    getevent -qlc 1 > "$vol_tmp" 2>/dev/null &
    ge_pid=$!

    while [ "$seconds" -gt 0 ] || [ "$1" = "0" ]; do
        sleep 1
        if ! kill -0 "$ge_pid" 2>/dev/null; then
            local key
            key=$(awk '/KEY_/{print $3}' "$vol_tmp" 2>/dev/null)
            case "$key" in
                KEY_VOLUMEUP)
                    rm -f "$vol_tmp"
                    return 0
                    ;;
                KEY_VOLUMEDOWN)
                    rm -f "$vol_tmp"
                    return 1
                    ;;
            esac
            : > "$vol_tmp"
            getevent -qlc 1 > "$vol_tmp" 2>/dev/null &
            ge_pid=$!
        fi
        [ "$1" = "0" ] || seconds=$((seconds - 1))
    done

    kill "$ge_pid" 2>/dev/null
    wait "$ge_pid" 2>/dev/null
    rm -f "$vol_tmp"
    return 0
}

# Populate system_app file for WebUI display (matches upstream behavior)
populate_system_app() {
    local system_app_file="/data/adb/tricky_store/system_app"
    [ -f "$system_app_file" ] && return

    local candidates="com.google.android.gms
com.google.android.gsf
com.android.vending
com.oplus.deepthinker
com.heytap.speechassist
com.coloros.sceneservice"

    : > "$system_app_file"
    for app in $candidates; do
        pm list packages -s 2>/dev/null | grep -q "package:$app" && echo "$app" >> "$system_app_file"
    done
}

# System-apps-only target.txt for manual mode
generate_minimal_target() {
    local system_apps="com.google.android.gms
com.google.android.gsf
com.android.vending"

    if [ "$(getprop ro.product.brand)" = "OnePlus" ]; then
        system_apps="$system_apps
com.oplus.engineermode"
    fi

    echo "$system_apps" | sort -u > "$TARGET_FILE"

    # Seed known_packages so daemon works if re-enabled later
    pm list packages -3 2>/dev/null | sed 's/^package://' | sort > "$KNOWN_PACKAGES"

    local count
    count=$(wc -l < "$TARGET_FILE" 2>/dev/null || echo 0)
    ui_print "  Target.txt: $count system apps"
}

# Build exclude list from more-exclude.json
build_exclude_list() {
    mkdir -p "$AUTOMATION_DIR"

    if [ -f "$MODPATH/more-exclude.json" ]; then
        grep '"package-name"' "$MODPATH/more-exclude.json" \
            | sed 's/.*"package-name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/' > "$EXCLUDE_FILE"
    else
        : > "$EXCLUDE_FILE"
    fi

    cat >> "$EXCLUDE_FILE" << 'EOF'
com.topjohnwu.magisk
io.github.vvb2060.magisk
io.github.huskydg.magisk
me.weishu.kernelsu
com.rifsxd.ksunext
com.sukisu.ultra
com.resukisu.resukisu
me.bmax.apatch
me.garfieldhan.apatch.next
com.android.patch
org.lsposed.manager
EOF

    ui_print "  Scanning for Xposed modules..."
    local xposed_tmp="$AUTOMATION_DIR/xposed_detected.tmp"
    : > "$xposed_tmp"

    for pkg in $(pm list packages -3 2>/dev/null | sed 's/^package://'); do
        [ -z "$pkg" ] && continue
        apk_path=$(pm path "$pkg" 2>/dev/null | head -n1 | cut -d: -f2)
        [ -z "$apk_path" ] && continue

        if unzip -l "$apk_path" 2>/dev/null | grep -q "assets/xposed_init"; then
            echo "$pkg" >> "$xposed_tmp"
        elif unzip -p "$apk_path" AndroidManifest.xml 2>/dev/null | tr -d '\0' | grep -q "xposedmodule"; then
            echo "$pkg" >> "$xposed_tmp"
        fi
    done

    [ -s "$xposed_tmp" ] && cat "$xposed_tmp" >> "$EXCLUDE_FILE"
    local xposed_count
    xposed_count=$(wc -l < "$xposed_tmp" 2>/dev/null || echo 0)
    ui_print "  Found $xposed_count Xposed modules"
    rm -f "$xposed_tmp"

    sort -u "$EXCLUDE_FILE" -o "$EXCLUDE_FILE"

    local count
    count=$(wc -l < "$EXCLUDE_FILE" 2>/dev/null || echo 0)
    ui_print "  Exclude list: $count patterns"
}

# Generate initial target.txt from installed packages
generate_initial_target() {
    local user_packages
    user_packages=$(pm list packages -3 2>/dev/null | sed 's/^package://' | sort)

    local system_apps="com.google.android.gms
com.google.android.gsf
com.android.vending"

    if [ "$(getprop ro.product.brand)" = "OnePlus" ]; then
        system_apps="$system_apps
com.oplus.engineermode"
    fi

    {
        if [ -s "$EXCLUDE_FILE" ]; then
            echo "$user_packages" | grep -vxFf "$EXCLUDE_FILE" 2>/dev/null
        else
            echo "$user_packages"
        fi
        echo "$system_apps"
    } | sort -u > "$TARGET_FILE"

    echo "$user_packages" > "$KNOWN_PACKAGES"

    local count
    count=$(wc -l < "$TARGET_FILE" 2>/dev/null || echo 0)
    ui_print "  Target.txt: $count apps"
}
