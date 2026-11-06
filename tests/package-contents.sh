#!/bin/sh
set -eu

if [ "$#" -eq 0 ]; then
    echo "Usage: sh tests/package-contents.sh <module.zip> [...]" >&2
    exit 2
fi

for archive in "$@"; do
    unzip -tq "$archive" >/dev/null
    contents=$(unzip -Z1 "$archive" | sed 's|^\./||')

    for required in \
        META-INF/com/google/android/update-binary \
        module.prop service.sh prop.sh common/prop-handler.sh webui/index.html \
        bin/arm64-v8a/ta-enhanced bin/arm64-v8a/resetprop-rs \
        bin/armeabi-v7a/ta-enhanced bin/armeabi-v7a/resetprop-rs \
        bin/x86/ta-enhanced bin/x86/resetprop-rs \
        bin/x86_64/ta-enhanced bin/x86_64/resetprop-rs; do
        if ! printf '%s\n' "$contents" | grep -Fxq "$required"; then
            printf 'Missing %s in %s\n' "$required" "$archive" >&2
            exit 1
        fi
    done
    if printf '%s\n' "$contents" | grep -Eq '^(\.git|\.github|\.omc|\.context|handoff|artifacts|release|rust|tests)/'; then
        printf 'Development or session files found in %s\n' "$archive" >&2
        exit 1
    fi
    printf 'Validated %s\n' "$archive"
done
