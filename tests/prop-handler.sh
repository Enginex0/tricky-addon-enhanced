#!/bin/sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
. "$repo_dir/common/prop-handler.sh"

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM
adb_dir="$tmpdir/adb"
bundled="$tmpdir/bundled resetprop"
mkdir -p "$adb_dir/ksu/bin" "$adb_dir/ap/bin" "$adb_dir/magisk" "$tmpdir/state"

assert_equal() {
    if [ "$1" != "$2" ]; then
        printf 'FAIL: %s: expected "%s", got "%s"\n' "$3" "$2" "$1" >&2
        exit 1
    fi
}

for native in "$adb_dir/ksu/bin/resetprop" "$adb_dir/ap/bin/resetprop" "$adb_dir/magisk/resetprop"; do
    : > "$native"
    chmod +x "$native"
done

cat > "$bundled" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$MOCK_STATE_DIR/calls"
if [ "$1" = "-n" ]; then
    [ "${MOCK_FAIL_SET:-0}" = 0 ] || exit 1
    if [ "${MOCK_IGNORE_SET:-0}" = 0 ]; then
        printf '%s' "$3" > "$MOCK_STATE_DIR/$2"
    fi
elif [ "$1" = "-d" ]; then
    rm -f "$MOCK_STATE_DIR/$2"
else
    [ "${MOCK_FAIL_GET:-0}" = 0 ] || exit 1
    if [ -f "$MOCK_STATE_DIR/$1" ]; then
        cat "$MOCK_STATE_DIR/$1"
    fi
fi
EOF
chmod +x "$bundled"

assert_equal "$(select_prop_resetprop KSU "$bundled" "$adb_dir")" "$adb_dir/ksu/bin/resetprop" "KernelSU native"
assert_equal "$(select_prop_resetprop APATCH "$bundled" "$adb_dir")" "$adb_dir/ap/bin/resetprop" "APatch native"
assert_equal "$(select_prop_resetprop MAGISK "$bundled" "$adb_dir")" "$adb_dir/magisk/resetprop" "Magisk native"
chmod -x "$adb_dir/ksu/bin/resetprop"
assert_equal "$(select_prop_resetprop KSU "$bundled" "$adb_dir")" "$bundled" "fallback ignores other managers"
assert_equal "$(select_prop_resetprop UNKNOWN "$bundled" "$adb_dir")" "$bundled" "unknown manager fallback"
if select_prop_resetprop KSU "$tmpdir/missing" "$adb_dir" 2>"$tmpdir/error"; then
    echo "FAIL: missing binaries were accepted" >&2
    exit 1
fi
grep -q 'No executable resetprop found' "$tmpdir/error"

RP="$bundled"
MOCK_STATE_DIR="$tmpdir/state"
export MOCK_STATE_DIR

printf '%s' unlocked > "$MOCK_STATE_DIR/ro.boot.vbmeta.device_state"
check_reset_prop ro.boot.vbmeta.device_state locked
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.vbmeta.device_state")" locked "property updated"
assert_equal "$(wc -l < "$MOCK_STATE_DIR/calls" | tr -d ' ')" 3 "write verified"

: > "$MOCK_STATE_DIR/calls"
check_reset_prop ro.boot.vbmeta.device_state locked
check_reset_prop ro.boot.missing locked
assert_equal "$(wc -l < "$MOCK_STATE_DIR/calls" | tr -d ' ')" 2 "unchanged and missing properties are not written"
[ ! -e "$MOCK_STATE_DIR/ro.boot.missing" ]

printf '%s' unlocked > "$MOCK_STATE_DIR/ro.boot.vbmeta.device_state"
export MOCK_FAIL_SET=1
if check_reset_prop ro.boot.vbmeta.device_state locked 2>"$tmpdir/error"; then
    echo "FAIL: failed write was accepted" >&2
    exit 1
fi
grep -q 'Failed to set property ro.boot.vbmeta.device_state' "$tmpdir/error"
unset MOCK_FAIL_SET

export MOCK_IGNORE_SET=1
if check_reset_prop ro.boot.vbmeta.device_state locked 2>"$tmpdir/error"; then
    echo "FAIL: ineffective write was accepted" >&2
    exit 1
fi
grep -q 'did not retain the requested value' "$tmpdir/error"
unset MOCK_IGNORE_SET

export MOCK_FAIL_GET=1
if check_reset_prop ro.boot.vbmeta.device_state locked 2>"$tmpdir/error"; then
    echo "FAIL: failed read was accepted" >&2
    exit 1
fi
grep -q 'Failed to read property ro.boot.vbmeta.device_state' "$tmpdir/error"
unset MOCK_FAIL_GET

mkdir -p "$tmpdir/common" "$tmpdir/mockbin"
cp "$repo_dir/common/prop-handler.sh" "$tmpdir/common/prop-handler.sh"
sed "s|/data/adb|$adb_dir|g" "$repo_dir/prop.sh" > "$tmpdir/prop.sh"
cat > "$tmpdir/mockbin/getprop" <<'EOF'
#!/bin/sh
if [ -f "$MOCK_STATE_DIR/$1" ]; then
    cat "$MOCK_STATE_DIR/$1"
fi
EOF
cat > "$tmpdir/mockbin/setprop" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$MOCK_STATE_DIR/setprop-calls"
EOF
chmod +x "$tmpdir/mockbin/getprop" "$tmpdir/mockbin/setprop"

printf '%s' unlocked > "$MOCK_STATE_DIR/ro.boot.vbmeta.device_state"
printf '%s' orange > "$MOCK_STATE_DIR/ro.boot.verifiedbootstate"
printf '%s' 0 > "$MOCK_STATE_DIR/ro.boot.flash.locked"
printf '%s' lineage_device > "$MOCK_STATE_DIR/ro.product.vendor.name"
printf '%s' org.lineageos.aperture > "$MOCK_STATE_DIR/vendor.camera.aux.packagelist"
printf '%s' running > "$MOCK_STATE_DIR/init.svc.vendor.lineage_health"
printf '%s\n' ABC123 > "$adb_dir/boot_hash"
: > "$MOCK_STATE_DIR/calls"
(cd "$tmpdir" && PATH="$tmpdir/mockbin:$PATH" RP_BIN="$bundled" sh prop.sh) 2>"$tmpdir/error"
[ ! -s "$tmpdir/error" ]
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.vbmeta.device_state")" locked "boot device state"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.verifiedbootstate")" green "verified boot state"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.flash.locked")" 1 "flash lock state"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.vbmeta.digest")" abc123 "boot hash"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.product.vendor.name")" device "LineageOS property"
assert_equal "$(cat "$MOCK_STATE_DIR/vendor.camera.aux.packagelist")" com.android.camera "LineageOS camera property"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.vbmeta.avb_version")" 1.0 "missing vbmeta property"
assert_equal "$(cat "$MOCK_STATE_DIR/setprop-calls")" "ctl.stop vendor.lineage_health" "LineageOS service"
[ ! -e "$MOCK_STATE_DIR/init.svc.vendor.lineage_health" ]
if grep -Eq '^(-w|-c)( |$)' "$MOCK_STATE_DIR/calls"; then
    echo "FAIL: property handler used incompatible resetprop options" >&2
    exit 1
fi

: > "$adb_dir/disable_vbmeta_digest_spoof"
printf '%s\n' DEF456 > "$adb_dir/boot_hash"
PATH="$tmpdir/mockbin:$PATH" RP_BIN="$bundled" sh "$tmpdir/prop.sh" 2>"$tmpdir/error"
assert_equal "$(cat "$MOCK_STATE_DIR/ro.boot.vbmeta.digest")" abc123 "boot hash spoof disabled"
[ ! -s "$tmpdir/error" ]

: > "$adb_dir/disable_prop_handler"
: > "$MOCK_STATE_DIR/calls"
PATH="$tmpdir/mockbin:$PATH" RP_BIN="$bundled" sh "$tmpdir/prop.sh" 2>"$tmpdir/error"
[ ! -s "$MOCK_STATE_DIR/calls" ]
[ ! -s "$tmpdir/error" ]

echo "Property handler tests passed"
