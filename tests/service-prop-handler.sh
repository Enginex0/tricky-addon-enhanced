#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM
mockbin="$tmpdir/mockbin"
mkdir -p "$mockbin"

cat > "$mockbin/resetprop" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$MOCK_STATE_DIR/calls"
case "$1" in
    -n)
        if [ "${MOCK_IGNORE_PROP:-}" != "$2" ]; then
            printf '%s' "$3" > "$MOCK_STATE_DIR/$2"
        fi
        ;;
    -d) rm -f "$MOCK_STATE_DIR/$2" ;;
    *) [ ! -f "$MOCK_STATE_DIR/$1" ] || cat "$MOCK_STATE_DIR/$1" ;;
esac
EOF
cat > "$mockbin/getprop" <<'EOF'
#!/bin/sh
if [ "$1" = sys.boot_completed ]; then
    printf '0\n'
elif [ -f "$MOCK_STATE_DIR/$1" ]; then
    cat "$MOCK_STATE_DIR/$1"
fi
EOF
cat > "$mockbin/cp" <<'EOF'
#!/bin/sh
if [ "${MOCK_FAIL_HIDE:-0}" = 1 ] && [ "$1" = -af ]; then
    exit 1
fi
exec /bin/cp "$@"
EOF
for command in busybox log pm setprop timeout; do
    printf '#!/bin/sh\nexit 0\n' > "$mockbin/$command"
done
chmod +x "$mockbin/"*

run_case() {
    local case_name="$1" ksu="$2" fail_hide="$3" ignored_prop="${4:-}"
    local case_dir="$tmpdir/$case_name" adb module state expected_prop_bin
    adb="$case_dir/adb"
    module="$adb/modules/TA_enhanced"
    state="$case_dir/state"
    mkdir -p "$module/common" "$module/bin/x86_64" "$adb/tricky_store" \
        "$adb/modules/tricky_store" "$adb/ksu/bin" "$state"
    for file in service.sh prop.sh common/common.sh common/prop-handler.sh; do
        sed "s|/data/adb|$adb|g; s|/storage/emulated/0|$case_dir/storage|g" \
            "$repo_dir/$file" > "$module/$file"
    done
    cp "$mockbin/resetprop" "$module/bin/x86_64/resetprop-rs"
    printf '#!/bin/sh\nexit 0\n' > "$module/bin/x86_64/ta-enhanced"
    chmod +x "$module/bin/x86_64/ta-enhanced"
    printf 'id=TA_enhanced\n' > "$module/module.prop"
    : > "$adb/tricky_store/keybox.xml"
    : > "$state/calls"
    printf '%s' orange > "$state/ro.boot.verifiedbootstate"

    if [ "$ksu" = true ]; then
        cp "$mockbin/resetprop" "$adb/ksu/bin/resetprop"
        expected_prop_bin="$adb/ksu/bin/resetprop"
    else
        : > "$module/action.sh"
        if [ "$fail_hide" = 1 ]; then
            expected_prop_bin="$module/bin/x86_64/resetprop-rs"
        else
            expected_prop_bin="$adb/modules/.TA_enhanced/bin/x86_64/resetprop-rs"
        fi
    fi

    PATH="$mockbin:$PATH" MOCK_STATE_DIR="$state" MOCK_FAIL_HIDE="$fail_hide" MOCK_IGNORE_PROP="$ignored_prop" \
        ARCH=x64 KSU="$ksu" APATCH="" sh "$module/service.sh" > "$case_dir/service-output" 2>&1

    local attempt=0
    while [ "$attempt" -lt 100 ]; do
        if grep -qx 'ro.boot.vbmeta.size' "$state/calls"; then
            break
        fi
        attempt=$((attempt + 1))
        sleep 0.1
    done
    if ! grep -qx 'ro.boot.vbmeta.size' "$state/calls"; then
        printf 'FAIL: property handler did not finish in %s\n' "$case_name" >&2
        cat "$case_dir/service-output" >&2
        cat "$adb/tricky_store/ta-enhanced/logs/main.log" >&2
        exit 1
    fi
    if [ -n "$ignored_prop" ]; then
        [ "$(cat "$state/ro.boot.verifiedbootstate")" = orange ]
        attempt=0
        while [ "$attempt" -lt 100 ]; do
            if grep -Fq "Property $ignored_prop did not retain the requested value" "$adb/tricky_store/ta-enhanced/logs/main.log"; then
                break
            fi
            attempt=$((attempt + 1))
            sleep 0.1
        done
        grep -Fq "Property $ignored_prop did not retain the requested value" "$adb/tricky_store/ta-enhanced/logs/main.log"
    else
        [ "$(cat "$state/ro.boot.verifiedbootstate")" = green ]
    fi
    grep -Fq "Property handler using $expected_prop_bin" "$adb/tricky_store/ta-enhanced/logs/main.log"

    if [ "$ksu" != true ] && [ "$fail_hide" = 0 ]; then
        [ ! -d "$module" ]
        [ -f "$adb/modules/.TA_enhanced/common/prop-handler.sh" ]
    else
        [ -d "$module" ]
    fi
    if [ "$fail_hide" = 1 ]; then
        grep -Fq 'Module hiding copy failed, using original path' "$adb/tricky_store/ta-enhanced/logs/main.log"
    fi
}

run_case magisk-fallback false 0
run_case magisk-copy-failure false 1
run_case kernelsu-native true 0
run_case kernelsu-write-failure true 0 ro.boot.verifiedbootstate
echo "Service property handler tests passed"
