# Shared attestation-engine detection. Keep this in sync with rust/src/engine.rs.
ENGINE="tricky_store"
ENGINE_MODULE="/data/adb/modules/tricky_store"

for _tee_candidate in /data/adb/modules/teesim /data/adb/modules_update/teesim; do
    [ -d "$_tee_candidate" ] || continue
    [ -f "$_tee_candidate/remove" ] && continue
    [ -f "$_tee_candidate/module.prop" ] || continue
    grep -q '^id=teesim$' "$_tee_candidate/module.prop" 2>/dev/null || continue
    ENGINE="teesim"
    ENGINE_MODULE="$_tee_candidate"
    break
done

unset _tee_candidate
