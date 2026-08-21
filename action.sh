MODPATH="/data/adb/modules/.TA_enhanced"
[ -d "$MODPATH" ] || MODPATH="/data/adb/modules/TA_enhanced"
MODDIR="$MODPATH"
ORG_PATH="$PATH"
TMP_DIR="$MODPATH/common/tmp"
APK_PATH="$TMP_DIR/base.apk"

. "$MODPATH/common/common.sh"

WEBUI_MODULE_ID="tricky_store"
[ "$ENGINE" = "teesim" ] && WEBUI_MODULE_ID="TA_enhanced"

download() {
    PATH=/data/adb/magisk:/data/data/com.termux/files/usr/bin:$PATH
    if command -v curl >/dev/null 2>&1; then
        curl --connect-timeout 10 -Ls "$1"
    else
        busybox wget -T 10 -qO- "$1"
    fi
    PATH="$ORG_PATH"
}

manual_download() {
    echo "$1"
    sleep 3
    am start -a android.intent.action.VIEW \
        -d "https://github.com/KOWX712/KsuWebUIStandalone/releases"
    exit 1
}

get_webui() {
    echo "- Downloading KSU WebUI Standalone..."
    API="https://api.github.com/repos/KOWX712/KsuWebUIStandalone/releases/latest"
    ping -c 1 -w 5 api.github.com >/dev/null 2>&1 \
        || manual_download "Error: No network"

    URL=$(download "$API" \
        | grep -o '"browser_download_url": "[^"]*"' \
        | cut -d '"' -f 4) \
        || manual_download "Error: Cannot get latest version"

    download "$URL" > "$APK_PATH" \
        || manual_download "Error: APK download failed"

    echo "- Installing..."
    pm install -r "$APK_PATH" || {
        rm -f "$APK_PATH"
        manual_download "Error: APK install failed"
    }
    rm -f "$APK_PATH"

    echo "- Launching WebUI..."
    am start -n "io.github.a13e300.ksuwebui/.WebUIActivity" -e id "$WEBUI_MODULE_ID"
}

if pm path io.github.a13e300.ksuwebui >/dev/null 2>&1; then
    echo "- Launching WebUI in KSUWebUIStandalone..."
    am start -n "io.github.a13e300.ksuwebui/.WebUIActivity" -e id "$WEBUI_MODULE_ID"
elif pm path com.dergoogler.mmrl.wx > /dev/null 2>&1; then
    echo "- Launching WebUI in WebUI X..."
    am start -n "com.dergoogler.mmrl.wx/.ui.activity.webui.WebUIActivity" \
        -e MOD_ID "$WEBUI_MODULE_ID"
else
    echo "! No WebUI app found"
    get_webui
fi

echo "- WebUI launched successfully."
