SKIPUNZIP=0
DEBUG=false
COMPATH="$MODPATH/common"
TS="/data/adb/modules/tricky_store"
. "$MODPATH/common/detect_engine.sh"
SCRIPT_DIR="/data/adb/tricky_store"
CONFIG_DIR="$SCRIPT_DIR/target_list_config"
MODID=$(grep_prop id "$TMPDIR/module.prop")
NEW_MODID=".TA_enhanced"
AUTOMATION_DIR="$SCRIPT_DIR/.automation"
ACTION=true

export MODULE_HOT_INSTALL_REQUEST="true"
export MODULE_HOT_RUN_SCRIPT="hotinstall.sh"

. "$MODPATH/install_i18n.sh"

ui_print " "
ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print "  ⚡ Tricky Addon Enhanced"
ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print " "

if [ "$APATCH" = "true" ]; then
    ui_print "  📱 APatch $APATCH_VER | $APATCH_VER_CODE"
    ACTION=false
elif [ "$KSU" = "true" ]; then
    if [ "$KSU_NEXT" ]; then
        ui_print "  📱 KernelSU Next $KSU_KERNEL_VER_CODE | $KSU_VER_CODE"
    else
        ui_print "  📱 KernelSU $KSU_KERNEL_VER_CODE | $KSU_VER_CODE"
    fi
    ACTION=false
elif [ "$MAGISK_VER_CODE" ]; then
    ui_print "  📱 Magisk $MAGISK_VER | $MAGISK_VER_CODE"
else
    ui_print " "
    ui_print "  ❌ Recovery is not supported"
    abort " "
fi

if [ "$ENGINE" = "teesim" ]; then
    engine_name=$(grep_prop name "$ENGINE_MODULE/module.prop")
    ui_print "  🔒 ${engine_name:-TEESimulator v4} detected"
elif [ -d "$TS" ]; then
    engine_name=""
    if [ -f "$TS/daemon" ]; then
        engine_name=$(grep -o '\-\-nice-name=[^ ]*' "$TS/daemon" 2>/dev/null | cut -d= -f2)
    fi
    engine_name=${engine_name:-"attestation engine"}
    ui_print "  🔒 $engine_name detected"
else
    ui_print "  ⚠️  No attestation engine module found"
fi

. "$MODPATH/install_func.sh"

ABI=$(getprop ro.product.cpu.abi)
case "$ABI" in
    arm64-v8a|armeabi-v7a|x86_64|x86) ;;
    *) abort "  ❌ Unsupported ABI: $ABI" ;;
esac
BIN="$MODPATH/bin/$ABI/ta-enhanced"
initialize

# Aggressive conflict purge. Hot-install means we cannot wait for the
# manager to process disable+remove on next boot, so rm -rf conflicting
# module directories from disk now. Active overlays linger until reboot
# but our module hot-installs at higher precedence.
PURGE_IDS="Yamabukiko TA_utl .TA_utl Yurikey xiaocaiye safetynet-fix \
vbmeta-fixer playintegrity integrity_box SukiSU_module Reset_BootHash \
Tricky_store-bm Hide_Bootloader ShamikoManager extreme_hide_root \
Tricky_Store-xiaoyi tricky_store_assistant extreme_hide_bootloader \
wjw_hiderootauxiliarymod PlayIntegrityFork"

ui_print "  🔍 $(_msg conflict_check)"
PURGED_COUNT=0
for mod in $PURGE_IDS; do
    [ -d "/data/adb/modules/$mod" ] || continue
    rm -rf "/data/adb/modules/$mod"
    ui_print "  🗑️  Purged: $mod"
    PURGED_COUNT=$((PURGED_COUNT + 1))
done

# Heuristic: any module shipping a known conflicting WebUI package APK
# under its system/ tree gets purged regardless of the module's own ID.
for mod_dir in /data/adb/modules/*/; do
    [ -d "$mod_dir" ] || continue
    mod_id=$(basename "$mod_dir")
    case "$mod_id" in
        TA_enhanced|.TA_enhanced) continue ;;
    esac
    if find "$mod_dir" -maxdepth 6 \
        \( -path "*com.lingqian.appbl*" -o -path "*com.topmiaohan.hidebllist*" \) \
        2>/dev/null | head -n1 | grep -q .; then
        rm -rf "$mod_dir"
        ui_print "  🗑️  Purged WebUI conflict: $mod_id"
        PURGED_COUNT=$((PURGED_COUNT + 1))
    fi
done

[ "$PURGED_COUNT" -eq 0 ] && ui_print "  ✅ $(_msg no_conflicts)"

HAS_TARGET=0
TEESIM_READY=1
if [ "$ENGINE" = "teesim" ]; then
    if "$BIN" automation profile-ready >/dev/null 2>&1; then
        "$BIN" automation export-target >/dev/null 2>&1 \
            || abort "  ❌ Failed to read TEESimulator config.json"
    elif PROFILE_ERROR=$("$BIN" automation profiles 2>&1); then
        TEESIM_READY=0
        ui_print "  ⚠️  Select a TEESimulator profile in this addon's WebUI"
        ui_print "  ℹ️  TEESimulator will remain unchanged until then"
    elif [ -f /data/adb/teesim/config.json ]; then
        abort "  ❌ Invalid TEESimulator config: $PROFILE_ERROR"
    else
        TEESIM_READY=0
        ui_print "  ⚠️  TEESimulator config is not available yet"
    fi
fi
if [ -f "/data/adb/tricky_store/target.txt" ] && [ -s "/data/adb/tricky_store/target.txt" ]; then
    HAS_TARGET=1
fi

ui_print " "
ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print "  🎯 $(_msg automation_title)"
ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print " "
ui_print "  🔊 $(_msg vol_up)"
ui_print "  🔉 $(_msg vol_down)"
ui_print " "
if [ "$HAS_TARGET" -eq 1 ]; then
    ui_print "  📋 $(_msg has_target)"
    ui_print " "
    choose_automation 0
else
    ui_print "  ⏱️  $(_msg auto_select)"
    ui_print " "
    choose_automation
fi
auto_mode=$?

if [ "$auto_mode" -eq 0 ]; then
    ui_print "  ✅ $(_msg auto_selected)"
    AUTOMATION_ENABLED=1
else
    ui_print "  🔧 $(_msg manual_selected)"
    AUTOMATION_ENABLED=0
fi

ui_print " "
ui_print "  📦 $(_msg installing)"

populate_system_app

if [ -x "$BIN" ]; then
    if ! "$BIN" version >/dev/null 2>&1; then
        abort "  ❌ Binary validation failed -- ta-enhanced does not run on this device"
    fi
else
    abort "  ❌ Binary not found at $BIN"
fi

_vbhash=$(getprop ro.boot.vbmeta.digest 2>/dev/null \
    | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]' \
    | grep -oE '^[a-f0-9]{64}$')
if [ -n "$_vbhash" ]; then
    _old_hash=""
    [ -f "/data/adb/boot_hash" ] && _old_hash=$(cat /data/adb/boot_hash 2>/dev/null)
    if [ "$_vbhash" != "$_old_hash" ]; then
        echo "$_vbhash" > /data/adb/boot_hash.tmp
        mv -f /data/adb/boot_hash.tmp /data/adb/boot_hash
        chmod 644 /data/adb/boot_hash
        ui_print "  🔐 VBHash captured from bootloader"
    else
        ui_print "  🔐 VBHash unchanged"
    fi
fi

if [ "$AUTOMATION_ENABLED" -eq 1 ]; then
    ui_print "  📋 $(_msg building_config)"
    build_exclude_list
    generate_initial_target
elif [ "$HAS_TARGET" -eq 1 ]; then
    ui_print "  📋 $(_msg target_preserved)"
    pm list packages -3 2>/dev/null | sed 's/^package://' | sort > "$AUTOMATION_DIR/known_packages.txt"
else
    generate_minimal_target
fi

if [ "$ENGINE" = "teesim" ] && [ "$TEESIM_READY" = "1" ]; then
    "$BIN" automation sync-target >/dev/null 2>&1 \
        || abort "  ❌ Failed to update TEESimulator config.json"
fi

TA_DIR="$SCRIPT_DIR/ta-enhanced"
mkdir -p "$TA_DIR/logs"

# PM can be sluggish during install
_try=0
while [ "$_try" -lt 3 ]; do
    _pkgs=$(pm list packages -s 2>/dev/null)
    [ -n "$_pkgs" ] && break
    _try=$((_try + 1))
    sleep 1
done
[ -n "$_pkgs" ] && echo "$_pkgs" | sed 's/^package://' | sort > "$TA_DIR/system_packages.txt"

if [ ! -f "$TA_DIR/config.toml" ]; then
    "$BIN" config init --automation="$AUTOMATION_ENABLED" 2>/dev/null \
        || ui_print "  ⚠️  Config init failed, daemon will create defaults at first run"

    LANG_CODE="$INSTALL_LANG"
    "$BIN" config set ui.language "$LANG_CODE" 2>/dev/null || true
    ui_print "  ⚙️  Language: $LANG_CODE"
else
    "$BIN" config set automation.enabled "$AUTOMATION_ENABLED" 2>/dev/null || true
    ui_print "  ⚙️  Configuration preserved (automation=$AUTOMATION_ENABLED)"
fi

# Snapshot device region props (only on fresh install — don't overwrite user overrides)
_cur_hwc=$("$BIN" config get region.hwc 2>/dev/null)
if [ -z "$_cur_hwc" ]; then
    _hwc=$(getprop ro.boot.hwc 2>/dev/null)
    _hwcountry=$(getprop ro.boot.hwcountry 2>/dev/null)
    _mod_device=$(getprop ro.product.mod_device 2>/dev/null)
    _hw_sku=$(getprop ro.boot.product.hardware.sku 2>/dev/null)
    [ -n "$_hwc" ] && "$BIN" config set region.hwc "$_hwc" 2>/dev/null
    [ -n "$_hwcountry" ] && "$BIN" config set region.hwcountry "$_hwcountry" 2>/dev/null
    [ -n "$_mod_device" ] && "$BIN" config set region.mod_device "$_mod_device" 2>/dev/null
    [ -n "$_hw_sku" ] && "$BIN" config set region.hardware_sku "$_hw_sku" 2>/dev/null
    ui_print "  🌐 Region: hwc=${_hwc:-n/a} mod_device=${_mod_device:-n/a} sku=${_hw_sku:-n/a}"
fi

if [ -f "$SCRIPT_DIR/enhanced.conf" ]; then
    "$BIN" config migrate 2>/dev/null \
        || ui_print "  ⚠️  Legacy config migration failed"
fi

if [ "$ENGINE" = "teesim" ] && [ "$TEESIM_READY" = "0" ]; then
    ui_print "  ⚠️  TEESimulator updates paused: no profile selected"
    ui_print "  ℹ️  CLI: ta-enhanced automation select-profile PROFILE_NAME"
else
    ui_print "  🛡️  Setting security patch dates..."
    if "$BIN" security-patch update --force 2>/dev/null; then
        ui_print "  ✅ $(_msg sec_patch_ok)"
    else
        ui_print "  ⚠️  $(_msg sec_patch_fail)"
    fi
fi

if [ "$ENGINE" = "teesim" ] && [ "$TEESIM_READY" = "0" ]; then
    : # Do not read or replace a keybox until the user selects its owning profile.
elif { [ "$ENGINE" = "tricky_store" ] && [ -f "$SCRIPT_DIR/keybox.xml" ]; } \
    || { [ "$ENGINE" = "teesim" ] && "$BIN" keybox validate >/dev/null 2>&1; }; then
    ui_print "  🔑 $(_msg keybox_kept)"
elif timeout 3 ping -c 1 -W 2 1.1.1.1 >/dev/null 2>&1; then
    ui_print "  🔑 $(_msg keybox_fetch)"
    _keybox_ok=0
    for _attempt in 1 2 3; do
        if timeout 5 "$BIN" keybox fetch 2>/dev/null; then
            _keybox_ok=1
            break
        fi
        sleep 1
    done
    if [ "$_keybox_ok" = "1" ]; then
        ui_print "  ✅ $(_msg keybox_ok)"
    else
        ui_print "  ⚠️  $(_msg keybox_fail) (will retry at boot)"
    fi
else
    ui_print "  🌐 No internet, keybox fetch deferred to boot/daemon"
fi

rm -f "$MODPATH/install_func.sh"

ui_print " "
ui_print "  📌 $(_msg not_tricky_store)"
ui_print "  📌 $(_msg no_report)"
ui_print " "

ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print "  ✨ $(_msg completed)"
ui_print "  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
ui_print " "
