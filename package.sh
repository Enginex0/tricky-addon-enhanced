#!/usr/bin/env bash
# Package TA_enhanced module ZIP
# Usage: ./package.sh [--no-build] [--debug] [--webui] [--no-bump] [--clean] [output_dir]
#
# Single entry point for producing a release:
#   1. Cross-compiles ta-enhanced for both ABIs (via rust/build.sh)
#   2. Bumps version (unless --no-bump)
#   3. Creates the installable ZIP
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
PROP="$REPO_DIR/module.prop"
UPDATE_JSON="$REPO_DIR/update.json"

DO_BUILD=true
DO_WEBUI=false
DO_BUMP=true
DO_CLEAN=false
BUILD_PROFILE=release
OUT_DIR=""

while [ $# -gt 0 ]; do
    case "$1" in
        --no-build) DO_BUILD=false ;;
        --debug)    BUILD_PROFILE=debug ;;
        --webui)    DO_WEBUI=true ;;
        --no-bump)  DO_BUMP=false ;;
        --clean)    DO_CLEAN=true ;;
        *)          OUT_DIR="$1" ;;
    esac
    shift
done

OUT_DIR="${OUT_DIR:-$REPO_DIR/release}"

MODULE_ID=$(grep '^id=' "$PROP" | cut -d= -f2)
OLD_VERSION=$(grep '^version=' "$PROP" | cut -d= -f2)
OLD_CODE=$(grep '^versionCode=' "$PROP" | cut -d= -f2)

if [ -z "$MODULE_ID" ] || [ -z "$OLD_VERSION" ] || [ -z "$OLD_CODE" ]; then
    echo "FATAL: Cannot parse module.prop" >&2
    exit 1
fi

if [ "$DO_BUMP" = true ]; then
    MAJOR=$(echo "$OLD_VERSION" | sed 's/^v//' | cut -d. -f1)
    MINOR=$(echo "$OLD_VERSION" | sed 's/^v//' | cut -d. -f2)

    NEW_MINOR=$((MINOR + 1))
    NEW_VERSION="v${MAJOR}.${NEW_MINOR}.0"
    NEW_CODE=$((OLD_CODE + 1))

    echo "Version: $OLD_VERSION -> $NEW_VERSION (code: $OLD_CODE -> $NEW_CODE)"

    sed -i "s/^version=.*/version=$NEW_VERSION/" "$PROP"
    sed -i "s/^versionCode=.*/versionCode=$NEW_CODE/" "$PROP"

    CARGO_VER=$(echo "$NEW_VERSION" | sed 's/^v//')
    sed -i "s/^version = \".*\"/version = \"$CARGO_VER\"/" "$REPO_DIR/rust/Cargo.toml"

    RELEASE_URL="https://github.com/Enginex0/tricky-addon-enhanced/releases/download/${NEW_VERSION}/${MODULE_ID}-${NEW_VERSION}.zip"
    sed -i "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" "$UPDATE_JSON"
    sed -i "s/\"versionCode\": [0-9]*/\"versionCode\": $NEW_CODE/" "$UPDATE_JSON"
    sed -i "s|\"zipUrl\": \".*\"|\"zipUrl\": \"$RELEASE_URL\"|" "$UPDATE_JSON"
else
    NEW_VERSION="$OLD_VERSION"
    NEW_CODE="$OLD_CODE"
    echo "Version: $NEW_VERSION (code: $NEW_CODE) [no bump]"
fi

if [ "$DO_BUILD" = true ]; then
    echo ""
    echo "=== Cross-compiling ta-enhanced ==="
    bash "$REPO_DIR/rust/build.sh" "$BUILD_PROFILE"

    echo ""
    echo "=== Cross-compiling resetprop-rs ==="
    RP_SRC="${RESETPROP_RS_SRC:-/home/president/Git-repo-success/resetprop-rs}"
    if [ ! -f "$RP_SRC/Cargo.toml" ]; then
        echo "FATAL: resetprop-rs source not found at $RP_SRC" >&2
        echo "Set RESETPROP_RS_SRC to override." >&2
        exit 1
    fi
    NDK_HOME=$(
        if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "$ANDROID_NDK_HOME" ]; then
            echo "$ANDROID_NDK_HOME"
        elif [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
            ls -1 "$ANDROID_HOME/ndk" | sort -V | tail -1 | xargs -I{} echo "$ANDROID_HOME/ndk/{}"
        else
            ls -1 "$HOME/Android/Sdk/ndk" 2>/dev/null | sort -V | tail -1 | xargs -I{} echo "$HOME/Android/Sdk/ndk/{}"
        fi
    )
    if [ -z "$NDK_HOME" ] || [ ! -d "$NDK_HOME" ]; then
        echo "FATAL: Android NDK not found for resetprop-rs build" >&2
        exit 1
    fi
    NDK_BIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
    declare -A ABI_TARGET=(
        [arm64-v8a]=aarch64-linux-android
        [armeabi-v7a]=armv7-linux-androideabi
        [x86_64]=x86_64-linux-android
        [x86]=i686-linux-android
    )
    declare -A TARGET_CC=(
        [aarch64-linux-android]=aarch64-linux-android26-clang
        [armv7-linux-androideabi]=armv7a-linux-androideabi26-clang
        [x86_64-linux-android]=x86_64-linux-android26-clang
        [i686-linux-android]=i686-linux-android26-clang
    )
    export PATH="$NDK_BIN:$PATH"
    for abi in "${!ABI_TARGET[@]}"; do
        target="${ABI_TARGET[$abi]}"
        cc="$NDK_BIN/${TARGET_CC[$target]}"
        target_upper=$(echo "$target" | tr '[:lower:]-' '[:upper:]_')
        target_under=$(echo "$target" | tr - _)
        declare -x "CARGO_TARGET_${target_upper}_LINKER=$cc"
        declare -x "CC_${target_under}=$cc"
        declare -x "AR_${target_under}=$NDK_BIN/llvm-ar"
        echo "  -> $abi ($target)"
        cargo build --release --manifest-path "$RP_SRC/Cargo.toml" \
            --bin resetprop --target "$target" \
            --target-dir "/tmp/resetprop-rs-build-$abi" >/dev/null
        src="/tmp/resetprop-rs-build-$abi/$target/release/resetprop"
        dst="$REPO_DIR/bin/$abi/resetprop-rs"
        mkdir -p "$REPO_DIR/bin/$abi"
        cp -f "$src" "$dst"
        "$NDK_BIN/llvm-strip" "$dst" 2>/dev/null || true
        chmod 755 "$dst"
    done
    echo "resetprop-rs built for all ABIs"
fi

if [ "$DO_WEBUI" = true ]; then
    echo ""
    echo "=== Building WebUI ==="
    if [ -f "$REPO_DIR/webui/package.json" ]; then
        cd "$REPO_DIR/webui"
        pnpm install && pnpm build
        cd "$REPO_DIR"
    else
        echo "WebUI: no package.json found, skipping WebUI build"
    fi
fi

echo ""
echo "=== Validating module contents ==="

REQUIRED_FILES=(
    customize.sh
    post-fs-data.sh
    service.sh
    prop.sh
    uninstall.sh
    install_func.sh
    hotinstall.sh
    module.prop
    update.json
    banner.png
    more-exclude.json
)

for f in "${REQUIRED_FILES[@]}"; do
    if [ ! -f "$REPO_DIR/$f" ]; then
        echo "FATAL: Required file missing: $f" >&2
        exit 1
    fi
done

for abi in arm64-v8a armeabi-v7a x86_64 x86; do
    if [ ! -f "$REPO_DIR/bin/${abi}/ta-enhanced" ]; then
        echo "FATAL: bin/${abi}/ta-enhanced not found. Run without --no-build or build manually." >&2
        exit 1
    fi
    if [ ! -f "$REPO_DIR/bin/${abi}/resetprop-rs" ]; then
        echo "FATAL: bin/${abi}/resetprop-rs not found. Run without --no-build or build manually." >&2
        exit 1
    fi
done

if [ ! -f "$REPO_DIR/webui/index.html" ]; then
    echo "FATAL: webui/index.html missing" >&2
    exit 1
fi

echo "Validation passed"

# Dead script guard: warn if deprecated scripts still exist in tree
ELIMINATED=(
    "common/logging.sh"
    "common/utils.sh"
    "common/conflict_manager.sh"
    "common/security_patch_manager.sh"
    "common/keybox_manager.sh"
    "common/vbhash_manager.sh"
    "common/health_check.sh"
    "common/automation.sh"
    "common/status_monitor.sh"
    "common/get_extra.sh"
    "common/run_keybox.sh"
    "common/run_secpatch.sh"
    "common/run_daemon.sh"
    "common/run_health.sh"
    "common/run_status.sh"
    "common/test_logging.sh"
)
for dead in "${ELIMINATED[@]}"; do
    if [ -f "$REPO_DIR/$dead" ]; then
        echo "WARNING: Deprecated script in tree: $dead (excluded from ZIP)"
    fi
done

echo ""
echo "=== Packaging ==="

mkdir -p "$OUT_DIR"
ZIP_NAME="${MODULE_ID}-${NEW_VERSION}.zip"
ZIP_PATH="$OUT_DIR/$ZIP_NAME"

if [ "$DO_CLEAN" = true ]; then
    rm -f "$OUT_DIR"/${MODULE_ID}-*.zip
    echo "Cleaned old ZIPs"
fi

rm -f "$ZIP_PATH"

cd "$REPO_DIR"
zip -r9 "$ZIP_PATH" . \
    -x ".git/*" \
    -x ".amp/*" \
    -x ".claude/*" \
    -x ".mcp-vector-search/*" \
    -x ".mcp.json" \
    -x ".gitignore" \
    -x ".gitmodules" \
    -x ".github/*" \
    -x ".omc/*" \
    -x ".context/*" \
    -x "handoff/*" \
    -x "external/*" \
    -x "artifacts/*" \
    -x "CLAUDE.md" \
    -x "*.zip" \
    -x "*.db" -x "*.db-shm" -x "*.db-wal" \
    -x "logs_llm/*" \
    -x "evidence_*.png" \
    -x "*.swp" -x "*~" \
    -x "release/*" \
    -x "package.sh" \
    -x "rust/*" \
    -x "node_modules/*" \
    -x "webui/src/*" \
    -x "webui/node_modules/*" \
    -x "*.map" \
    -x ".git" \
    -x "webui/dist/*" \
    -x "webui/src/*" \
    -x "webui/public/*" \
    -x "webui/node_modules/*" \
    -x "webui/package.json" \
    -x "webui/package-lock.json" \
    -x "webui/pnpm-lock.yaml" \
    -x "webui/.npmrc" \
    -x "webui/vite.config.ts" \
    -x "webui/tsconfig.json" \
    -x "common/archive/*" \
    -x "bin/archive/*" \
    -x "webui-mockup/*" \
    -x "bin/*/supervisor" \
    -x "bin/*/keygen" \
    -x "config/*" \
    -x "glob" -x "os" \
    -x "*.new" \
    -x "vectors.db*" \
    -x "webui/assets/index-CExZ91Qz.js.bak" \
    -x "webui/material-symbols-outlined.woff2" \
    -x "*.md"

echo ""
echo "=== Summary ==="
echo "Output:  $ZIP_PATH"
echo "Size:    $(du -h "$ZIP_PATH" | cut -f1)"
echo "Version: $NEW_VERSION (code: $NEW_CODE)"
echo "Files:   $(unzip -l "$ZIP_PATH" | tail -1)"
