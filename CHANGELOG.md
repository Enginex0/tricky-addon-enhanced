# Changelog

## Unreleased

### Features
- Added TEESimulator v4 support for profile apps, keyboxes, and patch levels without replacing its WebUI. Multi-profile setups can select the managed profile in this addon's WebUI.

## v5.53.1 (2026-05-01)

### Bug Fixes
- WebUI engine detector (`_c()` in `webui/assets/index-migrated.min.js`) scans `/data/adb/modules/*/module.prop` for `name=TEESimulator*` before falling back to the daemon nice-name and module.prop lookups. Fixes TEESimulator-rs forks displaying as upstream `TEESimulator` on the badge.
- Save toast pill anchored at `bottom-inset + 8px` with 11px font and 13px icon. `pointer-events:none` retained in the `.show` state so taps pass through to the package list. Default duration reduced to 1200ms.
- Card-body taps trigger autosave. Programmatic `md-checkbox.checked` toggles do not dispatch `change`, so a sibling click listener on `.card[data-package]` calls `scheduleFlush()` directly. Coalesced with the existing change-event path via `saveScheduled`.

## v5.53.0 (2026-05-01)

### Bug Fixes
- Keybox badge no longer reports `Revoked` for working keyboxes. Revocation is now informational metadata. Badge states: `OK`, `Invalid`, `No Keybox`.
- `keybox fetch` install gate checks structural validity only: chain integrity, validity window, recognized root, leaf-to-private-key match. Stops rejecting revoked keyboxes from public sources.
- `lookup_revocation` (`rust/src/keybox/validate.rs`) inspects the leaf only. Removes false positives from intermediate-CA entries in unrelated chains of a multi-Key keybox.
- `rust/src/keybox/roots/status.json` refreshed from `https://android.googleapis.com/attestation/status`. 1660 hex entries replace the prior 517-entry mixed-format snapshot.
- `window.y` bound to the global so the inline-script autosave, flag-toggle, and reboot-required prompts produce visible toasts. The bundled `y()` is module-scoped and not reachable from `webui/index.html`.

### Features
- AOSP keybox badge state. Health banner shows a blue `AOSP` pill when an AOSP-rooted keybox (`aosp_ec` or `aosp_rsa`) is loaded on a non-AOSP device. AOSP detection reads `ro.build.tags` from `/system/build.prop`.
- `detect_engine()` (`rust/src/health/mod.rs`) scans installed modules for `name=TEESimulator*` and returns the exact fork name. The badge now shows `TEESimulator-rs` for the Rust fork and `TEESimulator` for upstream.
- Automatic-mode `target.txt` seed includes `com.google.android.gms!`, `com.google.android.gsf!`, and `com.android.vending!`. Applied in both `generate_initial_target()` and `generate_minimal_target()` in `install_func.sh`.

### Changed
- `ValidationReport.ok` aggregates with `any` instead of `all`. A multi-Key keybox installs when at least one Key is structurally valid.
- `KeyboxInfo` JSON drops the `revoked` field. Revocation metadata remains as `revocation_serial` and `revocation_reason` per Key.
- `StatusInfo` JSON adds `is_aosp_device: bool`.
- `health_keybox_aosp` i18n key added to `webui/locales/template.xml` and 24 per-language strings files.
- Manual install (vol-down) preserves any existing `target.txt` verbatim. Removed the unconditional GMS/GSF/Vending/Oplus/Coloros append loop in `customize.sh`.

## v5.52.0 (2026-05-01)

### Features
- **Keybox status badge in the health banner.** The WebUI status row now renders a colored pill next to the engine state showing the active keybox condition: green `OK` when valid, red `Revoked` when the leaf appears in Google's attestation status list, amber `Invalid` for any other validation failure, and gray `No Keybox` when the file is absent. Driven by the existing `webui-init` JSON cached in `localStorage`, so the badge adds no extra `ksu.exec` call. Hover surfaces the full validation error list when the keybox is degraded.
- **Custom ROM identity scrub at boot.** `prop.sh` now strips the `lineage_` prefix from `ro.product.vendor.name`, rewrites `vendor.camera.aux.packagelist` and `persist.vendor.camera.privapp.list` to `com.android.camera` whenever they reference `org.lineageos`, and stops the `vendor.lineage_health` service before deleting its `init.svc.*` status prop. Every block is gated on the LineageOS-only signature being present, so stock devices match nothing and write nothing.

### Changed
- **`KeyboxInfo` payload exposes `revoked: bool`.** `webui-init` JSON now carries an explicit revocation flag derived from `keybox::validate::ValidationReport.keys[].revocation_reason`, so the WebUI does not need to substring-match error text to detect a revoked keybox.
- **i18n adds four health-banner keys.** `health_keybox_ok`, `health_keybox_revoked`, `health_keybox_invalid`, and `health_keybox_missing` land in `webui/locales/template.xml` and the 23 per-language strings files. Non-English locales carry the English literal as a placeholder pending translator contributions.

## v5.51.0 (2026-05-01)

### Features
- **Full keybox validator** — `keybox::validate` is now a faithful Rust port of the `purainity/keybox-tools` Python `check.py` reference. Replaces the previous 39-line XML-tag-presence check with a complete cryptographic pipeline: certificate chain signature verification (RSA-PKCS1 SHA-1/256/384/512, ECDSA P-256/SHA-256, ECDSA P-384/SHA-384), leaf validity window check, leaf-to-private-key match across RSA/P-256/P-384 PKCS#8 and PKCS#1 PEM, root-type detection against four embedded trust anchors (Google hardware, AOSP-EC, AOSP-RSA, Samsung Knox), and live Google revocation check at `https://android.googleapis.com/attestation/status` with a baked-in `status.json` snapshot for offline fallback
- **Rich validation report** — new `validate_full` API returns a per-`Key` `KeyReport` carrying root type, leaf serial, leaf subject, validity window, chain status, key-match verdict, and revocation reason. Aggregate `ValidationReport` exposes the revocation source (online vs. embedded), online error string when fallback fired, and overall `ok` flag computed as `chain_valid && validity_ok && root_type != Unknown && !revoked`
- **`ta-enhanced keybox validate` emits JSON** — CLI handler now serializes the full report to stdout and returns exit code 1 when `report.ok` is false, suitable for shell scripts and the WebUI
- **WebUI keybox `rootType` field** — `webui-init` JSON now carries the active keybox's root type (`google`, `aosp_ec`, `aosp_rsa`, `knox`, `unknown`) so the UI can render hardware vs. software attestation badges. WebUI rendering work left for follow-up

### Bug Fixes
- **Keybox fetch silently accepted broken keyboxes** — `fetch()` previously passed any keybox whose XML contained the right substrings, including expired, revoked, key-mismatched, chain-broken, and unknown-root payloads. The new validator rejects all of these with explicit log lines naming the failure mode, then continues to the next configured source
- **`keybox set-custom` accepted invalid keyboxes** — install path now bails with a per-`Key` failure summary if any structural or cryptographic check fails

### Changed
- **New dependencies** — `x509-parser = "0.18.1"` for X.509 ASN.1 parsing, `p256 = "0.13.2"` and `p384 = "0.13.1"` for EC private-key public-side derivation, plus the `serialize` feature on `quick-xml` for serde-driven XML deserialization. `ring`, `rsa`, `ureq`, `base64`, and `serde` are reused; no new HTTP stack added
- **Embedded resources** — four trust-anchor PEMs and the Google revocation snapshot now live under `rust/src/keybox/roots/` and are baked into the binary via `include_bytes!`
- **Single-cert chain semantics** — matches `check.py`: chains shorter than two certificates pass `chain_valid` vacuously. Software keyboxes with a single self-signed cert still fail the overall `ok` check via `root_type == Unknown`

## v5.48.0 (2026-04-30)

### Features
- **Hot install** — module takes effect without reboot on KSU, APatch, and Magisk hot-paths. Conflicting modules are now purged via `rm -rf` at install time instead of flag-file disable, closing the boot-delay window where overlays kept mounting. Heuristic pass also removes any third-party module referencing known WebUI-companion conflict packages
- **HMA bypass** — package enumeration and APK path resolution read `/data/system/packages.list` and scan `/data/app/` directly, so HideMyAppList no longer hides apps from the target picker or the daemon. Falls back to `dumpsys` for label resolution
- **Default target.txt seeds** — `com.facebook.appmanager`, `com.facebook.services`, `com.facebook.system`, and `com.tencent.soter.soterserver` are now seeded unconditionally alongside GMS/GSF/Vending. Covers Samsung/OEM-preinstalled Facebook system apps and the WeChat biometric/payment auth bridge on Chinese ROMs
- **`target.txt` autosave with delta writes** — every checkbox change auto-persists; rapid toggles coalesce into a single write per microtask. Hidden system entries are preserved verbatim. Save FAB is gone (#23)
- **WebUI compatibility toggles** — Automation → Compatibility now exposes `disable_vbmeta_digest_spoof` and `disable_prop_handler` flags from a single dialog (#21)
- **Inotify-driven status refresh** — module description now updates the moment a status-relevant event fires instead of waiting for the 30s poll
- **Task list in `daemon-status`** — CLI now reports the active scheduler task list (Status, Automation, Health, Keybox, SecurityPatch)
- **`config props-custom` CLI getter** — prints user-defined `props.custom_props` as `name<TAB>value` lines for shell consumption

### Bug Fixes
- **Fingerprint enrollment failure** — boot-time `ro.boot.vbmeta.digest` rewrite is now gated behind `/data/adb/disable_vbmeta_digest_spoof`, fixing enrollment on Snapdragon-class devices without losing Play Integrity. The `/data/adb/disable_prop_handler` flag also short-circuits the digest rewrite (#21)
- **Keybox source rotation** — KOW upstream URL repointed from the dead `main/.extra` to the live `keybox/.extra` branch (#22)
- **Variant-aware boot hash** — `prop.sh` previously trusted `boot_hash.bin` from any TrickyStore variant, including forks without boothash persistence. Now checks `module.prop` for TEESimulator-RS before deferring, validates the file, and falls back to `/data/adb/boot_hash` if invalid
- **Spoof cleanup ordering** — split into early and late phases so prop normalization runs before the VBMeta block, fixing edge cases where ordering left stale build-string values
- **`custom_props` and test-keys loops** — switched to heredoc-fed loops so values containing whitespace or shell metacharacters are applied verbatim
- **x86 and x86_64 uninstall** — uninstall script now cleans the additional ABI binary directories shipped in v5.24.0
- **Light-mode WebUI** — full light-mode pass: text colors, hover surfaces, gradient-text fills, glass-surface backgrounds, source-card body, icon tile, pale Unknown/AOSP icons, and the FETCH INTERVAL caption all have proper light-mode contrast. Mode picker rows get per-row color stripes so Auto, Generated, and Custom stay distinguishable on any accent or theme (#20)
- **IntegrityBox config migration** — legacy `keybox.source = "integritybox"` values now persist their migration to `yurikey` on disk (previously rewritten in memory only)
- **Bundled JS null-deref on FAB removal** — minified bundle hard-references `#save` and `.floating-btn` at module init; an invisible inert stub is restored at the original location so the import resolves while autosave remains the sole save path

### Changed
- **Boot-time spoofing consolidated into `service.sh`** — replaces the backgrounded `prop.sh` with a synchronous inline block gated on `--wait sys.boot_completed 0`, mirroring the susfs4ksu pattern. ZeroMount gating is no longer required for prop application
- **Property cleanup runs once inline at boot** — folded `propclean.sh` into the consolidated spoof block. `--hexpatch-delete` replaced with `--nuke` everywhere; the hexpatch fallback is gone since propdetect heuristics flag hexpatch artifacts (count anomalies, name destruction)
- **Standalone `resetprop-rs` CLI** — props now go through the standalone resetprop binary instead of the in-process Rust API
- **`pm list` for package enumeration** — replaces direct `packages.list` parsing where appropriate
- **`resetprop-rs` as cargo git dependency** — switched from a vendored submodule so builds pull the latest commit
- **Denylist merge** — gracefully no-ops on KSU/APatch (no upstream denylist enumeration API); Magisk path unchanged

### Removed
- **Rust `props` module** — boot-time spoofing now lives entirely in `service.sh`. `Commands::Props` CLI variant, `props/mod.rs`, `PropsConfig.enabled`, and `PropCleanConfig` are gone. `platform/props.rs` trimmed to `RP_PATH` + `getprop` + `set`
- **`PropCleanTask`** — daemon no longer schedules a recurring property-cleanup timer. Cleanup is a one-shot inline boot step. Scheduler now tracks five tasks instead of six
- **`init.svc.*` spoofing** — dropped from the prop set; the entries created more attestation noise than they prevented
- **`propclean.sh` and `prop.sh`** — both deleted; their logic is inlined in `service.sh`. `hexpatch_deleteprop` removed from `common.sh`
- **IntegrityBox keybox source** — upstream MeowDump replaced their artifact with an anti-fork taunt that decodes to "NICE TRY DIDDY". Existing configs auto-migrate to `yurikey` on next boot
- **Save FAB** — replaced by autosave; the floating button is no longer rendered

---

## v5.27.0 (2026-03-18)

### Bug Fixes
- **Accent color not persisting** — manually picking an accent color from the picker didn't disable randomization, so the next page load or tab switch would randomize over the user's choice. Picker now auto-disables randomization and syncs the toggle state
- **Apps silently removed from target.txt** — `cleanup_dead_apps` relied solely on `pm list packages -3` which can be filtered by HideMyAppList or miss apps in other user profiles. Now cross-checks `/data/data/<pkg>` existence and requires 3 consecutive misses before removing, preventing false removals during app updates or when HMA is active
- **WebUI save race with daemon** — `target.txt` was written non-atomically (`echo > file`), allowing the daemon to read a truncated file mid-write. Now uses temp-file-then-rename

---

## v5.26.0 (2026-03-17)

### Bug Fixes
- **Custom ROM version wiped by propclean** — `hexpatch_deleteprop` was destroying all properties matching ROM fingerprint substrings (e.g. `crdroid`), including `ro.crdroid.build.version` which crDroid needs for its About screen and OTA updater. Now auto-detects the running ROM by checking for `ro.<fingerprint>*` properties before wiping — if the device owns those props, they're preserved. Generic fix that works for any ROM in the fingerprint list (LineageOS, EvolutionX, PixelOS, etc.)

---

## v5.25.0 (2026-03-14)

### Bug Fixes
- **`marketname` false positive in ROM fingerprint detection** — stock Xiaomi `marketname` props were being scrubbed as custom ROM indicators, breaking Xiaomi Share device naming and iOS Interconnectivity (#16, #17)
- **ReSukiSU excluded from target list** — added `com.resukisu.resukisu` to the manager exclusion list alongside existing SukiSU Ultra entry (#18)

---

## v5.24.0 (2026-03-11)

### Features
- **x86_64 and x86 ABI support** — module now ships binaries for all four Android ABIs (arm64-v8a, armeabi-v7a, x86_64, x86), enabling Waydroid and emulator installs

### Bug Fixes
- **Bootloader detection by TrustAttestor** — `check_reset_prop` was creating props on devices where they don't naturally exist (e.g. Realme/OnePlus-specific props on Xiaomi), giving attestor apps a clear tampering signal. Now skips non-existent props instead of blindly injecting them, matching stock behavior
- **AVB version mismatch** — `ro.boot.vbmeta.avb_version` was set to `1.3` (non-standard) instead of `1.0`, creating a detectable inconsistency with the actual AVB stack
- **Extra props in Rust backend** — removed `ro.bootimage.build.tags`, `ro.boot.verifiedbooterror`, and `ro.boot.veritymode.managed` from the Rust prop list since they don't exist on most devices and would be created unnecessarily
- **Unconditional prop injection** — `ro.oem_unlock_supported` and `ro.secureboot.devicelock` were set outside the ZeroMount guard, now properly gated
- **Stale description on uninstall** — WebUI uninstall now immediately restores the original TrickyStore/TEESimulator description instead of relying on daemon timing; also fixed path mismatch between Rust (`description.bak`) and shell (`.original_description`) backup files

---

## v5.23.0 (2026-03-11)

### Features
- **`ro.boot.product.hardware.sku` support** — fourth region prop added across the full stack (config, install snapshot, boot enforcement, WebUI)
- **Collapsible region UI** — `ro.boot.hwc` always visible, remaining 3 fields behind a chevron expand to reduce clutter
- **GitHub Actions CI** — build workflow cross-compiles both ABIs and uploads the module ZIP as artifact on every push

### Improvements
- **Region i18n** — all 3 region strings translated across 22 locales (was English-only fallback)
- **Dynamic README badges** — version badge pulls from GitHub releases, build status badge from CI

---

## v5.22.0 (2026-03-10)

### Features
- **Config-driven MIUI region props** — snapshots `ro.boot.hwc`, `ro.boot.hwcountry`, `ro.product.mod_device` at install time instead of blind CN→GLOBAL spoofing. Supports Global/CN/India variants without breaking Xiaomi.eu or stock Indian ROMs
- **Region WebUI controls** — toggle and text fields inside Keybox Automation settings for per-device override of region props
- **Boot hash priority chain** — prefers TEESimulator's persisted `boot_hash.bin` when present (cert-chain consistent), falls back to our install-time capture. No gap if TEESimulator is absent or uses upstream without persistence

### Improvements
- **Uninstall completeness** — cleans up `.verbose`, `devconfig.toml`, `TA_enhanced` module dir, `banner.png` symlink, and banner line from TrickyStore's `module.prop`

---

## v5.8.0 (2026-03-09)

### Features
- **Inotify-based app detection** — daemon watches `/data/app/` for installs and uninstalls via inotify, gated on `automation.use_inotify` config. Two-stage scan at 3s and 8s after directory event handles the PM registration race where the package manager hasn't finished registering the app when the directory first appears
- **On-demand automation trigger** — scheduler exposes `run_automation_now()` so inotify events bypass the regular 10s polling interval

### Bug Fixes
- **Module version not shown in WebUI** — `service.sh` deletes `module.prop` to hide from manager UI, but `webui_init.rs` couldn't read it. Now preserves a copy to `/data/adb/tricky_store/ta-enhanced/module.prop` before deletion, added as first lookup path
- **Version fallback missing `v` prefix** — `CARGO_PKG_VERSION` fallback now formats as `v5.8.0` instead of `5.8.0`

### Performance
- **WebUI init unblocked** — `webui-init` no longer waits for binary path resolution (`kc()`); path is resolved inline within the shell call, saving one sequential round trip on page load

---

## v5.7.0 (2026-03-08)

### Bug Fixes
- **KSU theme colors** — use KSU native theme system for Material surface colors instead of hardcoded values

---

## v5.6.0 (2026-03-07)

### Bug Fixes
- **Security patch not applied on reinstall** — `security-patch set` wrote device ROM dates (e.g. 2025-xx) and `update` silently skipped when `auto_update` was previously disabled; install now uses `--force` flag to always fetch latest bulletin date regardless of config, with built-in fallback table for offline installs
- **VBMeta hash not refreshed on reinstall** — installer skipped capture when `boot_hash` already existed; now compares live `ro.boot.vbmeta.digest` against stored value and updates if changed

### Enhancements
- **Keybox interval badge** — Automation settings now shows the current fetch interval as a badge next to the section header, updated in real-time when changed via chips or custom input

---

## v5.5.0 (2026-03-07)

### Bug Fixes
- **Security patch dialog empty on open** — dialog always showed blank fields because auto-mode config (default true) short-circuited file reading; now always reads and displays current values from `security_patch.txt`
- **Get patch date failing** — fetching latest Pixel bulletin date failed due to exec buffer limits with full HTML response; now pipes through `sed` on shell side (matching upstream), with `busybox wget` fallback
- **Boot script overwriting bulletin dates** — `service.sh` ran `security-patch set` on boot which overwrote daemon-fetched dates with stale device props; removed in favor of daemon's `SecurityPatchTask`
- **Security patch validator rejecting valid dates** — save dialog rejected `YYYY-MM-DD` format written by Rust; validator now accepts both `YYYYMM` and `YYYY-MM-DD`
- **Auto-set not detected in dialog** — dialog checked for legacy flag file instead of Rust config value
- **Security patch toggle resetting** — `hydrateUI()` used camelCase config keys but Rust serializes snake_case
- **Custom interval input not discoverable** — added pulse animation so users notice the editable field
- **VBMeta hash stale after reboot** — `extract()` returned the persisted file without checking the live `ro.boot.vbmeta.digest` property; now always reads the property first and updates the file if the digest has changed

### Install
- **Legacy module auto-removal** — `TA_utl` and `.TA_utl` are now detected and tagged for removal during install before conflict checks run
- **Version gate removal** — removed minimum APatch/KSU version checks from `customize.sh`

---

## v5.3.0 (2026-03-06)

### Ground-Up Rust Rewrite

The entire backend has been rewritten from scratch in Rust. Shell scripts were too fragile — hundreds of thousands of wakeups per day, fork-per-config-read, race conditions, and cascading failures. All eliminated.

| Metric | Shell (v4.x) | Rust (v5.x) |
|---|---|---|
| Wakeups/day | 884,449 | ~100 |
| JVM spawns/day | 20,170 | ~200 |
| Processes | 6 | 1 |
| Background CPU | ~28 min/day | <1 min/day |
| Config reads | 43,200 forks/day | 0 (in-memory) |
| App detection | 10s–minutes | Instant (inotify) |

### Native Daemon
- Single `ta-enhanced` binary (4.0MB arm64, 2.7MB armv7) replaces 16 shell scripts
- Unified scheduler — keybox, security patch, health, status, automation in one process
- inotify app detection — new installs targeted instantly
- In-memory TOML config — zero fork overhead
- Proper signal handling, graceful shutdown

### CLI
- `ta-enhanced config get/set` — type-safe config with validation and clamping
- `ta-enhanced keybox fetch/validate/generate/backup` — full keybox pipeline
- `ta-enhanced security-patch set/get-latest` — engine-aware patch management
- `ta-enhanced webui-init` — batched JSON endpoint (replaces ~14 shell calls)
- `ta-enhanced conflict check` — structured conflict detection
- `ta-enhanced applist` — package enumeration with labels

### Device Keybox Generation
- ECDSA P-256 + RSA-2048 via `ring` — valid AOSP-level keybox without remote sources

### WebUI
- Batched init — single shell call, <300ms cold start
- Cache-first hydration from localStorage
- Font subset 287KB → 5KB (28 icons)
- Non-blocking external CSS via media swap
- Keybox automation panel — 6 source cards, interval chips (1h–7d), custom input with min/hr/day toggle
- About dialog rebranded — Enginex0 author, SuperPowers Telegram, full credits
- Debounced health refresh (30s throttle)
- SukiSU theme caching

### Shell Elimination
- 16 scripts deleted, `service.sh` reduced to daemon launcher
- Config migrated from `enhanced.conf` to `config.toml` (auto-migration on first boot)
- Module ID: `TA_utl` → `TA_enhanced`

### Bug Fixes
- Daemon double-init panic after daemonize fork (tracing subscriber inherited by child)
- All 16 audit bugs from v4.9 eliminated by architectural change

---

## v4.9-auto (2026-02-13)

### Features
- **Volume key install-time selection** — press Vol- during flash to disable automatic target.txt population. Manual mode seeds only GMS/GSF/Vending so attestation works out of the box; Vol+ or 10s timeout defaults to full automation. Respects `automation_target_enabled` in config (`2661584`)
- **Build script** — `package.sh` auto-bumps versionCode and builds the release ZIP in one command (`8648fa7`)

### Fixes
- **78-bug codebase audit** — supervisor kill chain (PID tracking, SIGTERM cascade via process group, exponential backoff 1-60s), property race elimination (prop.sh sole VBMeta authority), keybox pipeline hardening (staged decode, 6-point validation, TOCTOU fix, rotating backup, 60s minimum interval), health monitor circuit breaker (10 max restarts, resets on recovery), atomic PID files, TLS enforced on wget, POSIX bashism cleanup (`8a8ada9`)
- **Keybox preserved on failed fetch** — no-network boot was falling through to bundled AOSP keybox, overwriting a valid device-specific one. Guard now preserves existing keybox; bundled fallback only fires on fresh install (`3089524`)
- **WebUI CLI sourcing** — manual keybox fetch was hanging because keybox_manager.sh lost logging.sh/utils.sh sourcing, causing `read_config` undefined. Restored sourcing for all CLI entry points (`3089524`)
- **MODPATH collision** — get_extra.sh manager scripts inherited `common/` subdirectory path instead of module root, causing `common/common/` sourcing failures on `--fetch-keybox-now`, `--set-security-patch-now`, `--check-conflicts` (`3089524`)
- **WebUI uninstall** — module.prop was restored to `MODPATH` (common/) instead of `MODDIR` (module root), so ksud/apd couldn't find the module to remove (`4367b7b`)
- **Bounded post-fs-data wait** — KernelSU kills post-fs-data after 10s; unbounded while loop on slow /data mounts capped at 8 iterations (`ae03ca4`)
- **Strict manager detection** — loose `[ "$APATCH" ]` matched any non-empty string; aligned to `[ "$APATCH" = "true" ]` (`ae03ca4`)
- **WebUI input validation** — innerHTML replaced with textContent for app names, hex validation on boot_hash input, shell metacharacter stripping on security patch values, config key/value whitelisting against sed injection (`08a004a`, `8a8ada9`)

### Performance
- **Cache-first app list** — localStorage renders instantly on subsequent opens with stale-while-revalidate background refresh. `fetchAppList`/`loadTranslations`/`getBasePath` fire in parallel instead of waterfall. Generation counter prevents stale writes (`3e9063c`)

### Refactoring
- **Consolidated read_config()** — was defined in 5 files with subtly different behavior; unified in utils.sh with `cut -f2-` (handles values containing `=`), trim, and file existence check. Removed duplicates from keybox_manager.sh, security_patch_manager.sh, status_monitor.sh, health_check.sh (`4a00a14`)

## v4.8-auto (2026-02-07)

### Fixes
- **VBHash extraction broken on some devices** — APK-based attestation extraction fails silently on certain OEM/Android combos (reported on OnePlus 12 / ColorOS 16 / Android 16). VBHash now captured directly from `ro.boot.vbmeta.digest` at install time — instant, zero dependencies, works on all AVB-enabled devices. APK method retained as last-resort fallback (#2) (`3ed7269`)
- **VBHash hex validation** — all extraction paths regex-validate the hash as exactly 64 lowercase hex characters, rejecting malformed or empty values before persisting

### Improvements
- **Discoverable log directory** — logs moved from hidden `/data/adb/tricky_store/.automation/` to `/data/adb/Tricky-addon-enhanced/logs/`
- **Separated concerns** — log files and automation state (exclude patterns, known packages, daemon PID) no longer share the same directory

### Internals
- VBHash extraction refactored into three composable functions: `extract_from_property()`, `extract_from_apk()`, `persist_and_apply_hash()`
- `LOG_BASE_DIR` updated across 8 files
- Uninstall cleanup covers both old and new directory paths

## v4.7-supervisor (2026-02-07)

### Features
- **Native process supervisor** — single compiled binary (5.6KB ARM64, 3.9KB ARM32) manages all 5 background processes via fork/wait/restart, based on TEESimulator's supervisor.cpp pattern (`022ee63`)
- **Full self-healing** — daemon, health monitor, status monitor, keybox loop, and security patch loop all restart within 1s of silent death
- **PR_SET_PDEATHSIG** — child processes auto-killed if supervisor dies, preventing orphaned processes
- **Root manager cache refresh** — force-stops KSU/Magisk/APatch manager after new app is added to target.txt, ensuring WebUI shows fresh package list immediately
- **Uninstall-aware exit** — supervised scripts exit with code 42 on module removal; supervisor stops cleanly without restart loops

### Internals
- Replaced shell supervisor (supervisor.sh) with native ARM64/ARM32 binary
- Extracted shared utility functions to `common/utils.sh`
- service.sh reduced from 257 to 141 lines
- Uninstall signal touches both `/data/adb/modules/TA_utl/remove` and `.TA_utl/remove` paths

## v4.6-auto (2026-02-06)

### Features
- **4-source keybox failover** — Yurikey → Upstream → IntegrityBox → bundled backup, with XML validation and custom keybox protection
- **Aggressive boot-time fetch** — keybox and security patch retry up to 10 times with 3s backoff after network ready
- **Install-time patch upgrade** — security patch dates fetched from Google during module flash, not just at boot
- **Automatic security patch updates** — detects attestation engine variant (James, Standard, Legacy) and sets system/boot/vendor dates at configurable intervals
- **VBHash spoofing** — one-time extraction via camouflaged APK, persists to `/data/adb/boot_hash`, 15 properties spoofed as locked bootloader
- **TEESimulator health monitor** — polls attestation engine every 10s with 5s grace period, auto-restarts on crash
- **Live status monitor** — module description updates every 30s with real-time app count, keybox source, patch level, VBHash state
- **Live target watcher** — WebUI polls target.txt every 3s and hot-inserts app cards without full reload
- **Dynamic engine detection** — auto-detects TEESimulator vs TrickyStore from daemon `--nice-name` parameter
- **Module conflict detection** — 19 conflicting modules detected at install and boot
- **Unified logging** — centralized 1MB rotation across 13 scripts

### Fixes
- **Status monitor app count** — whitespace/CR normalization; count now matches WebUI (`8c5dbf5`)
- **Monitor cleanup on uninstall** — status monitor and health check detect the `remove` flag within 30s, restore original description, exit cleanly (`8c5dbf5`)
- **TEESimulator variant misdetection** — versionCode-based detection now correctly identifies standard variant (`f14b5b4`)
- **Security patch fallback regression** — auto-update no longer overwrites valid dates with stale ROM values when Google fetch fails
- **Config preservation** — keybox and `enhanced.conf` survive reinstalls and engine switches (`0f6a581`)

### WebUI
- Modern glass morphism design with AMOLED-friendly dark gradients
- Health status banner with live attestation engine state
- Redesigned Save FAB and Uninstall button with glass morphism
- Scroll indicator in settings panel
- Automation settings bottom sheet
- 6 accent color presets with random selection
- 23 languages with RTL support

## v4.5-auto (2026-02-03)

### Features
- **Live status monitor** — module.prop description updates every 30s with active app count, keybox source, and patch level (`999e8a8`)
- **Live target watcher** — WebUI watches target.txt changes with event delegation and exec timeout (`54b50ab`)
- **Dynamic engine detection** — health monitor auto-detects TEESimulator vs TrickyStore at runtime (`ab23edd`)
- **Config preservation** — keybox and enhanced.conf preserved across reinstalls and engine switches (`0f6a581`)
- **Playwright test suite** — dynamic mock environment for WebUI testing (`db790de`)

### Fixes
- **TEESimulator misdetected as legacy** in security patch pipeline (`f14b5b4`)

## v4.4-auto (2026-02-02)

### Features
- **VBHash one-time persistence** — extracted hash persisted to `/data/adb/boot_hash`, APK camouflaged as `com.ceco.gravitybox.unlocker` (`e934494`, `6ca7ab7`)
- **TEESimulator health monitor** — background supervisor polls attestation engine, auto-restarts on crash (`a465142`)
- **WebUI automation integration** — automation.js backend integration, elite toast notifications with glass morphism (`92dc9cd`)
- **VBHash extraction** — extracts `verifiedBootHash` from KeyStore attestation extension (OID `1.3.6.1.4.1.11129.2.1.17`) via temp APK (`92dc9cd`)

### Fixes
- **Boot hash display** — sed filter was inverted, showing PEM comments instead of hash (`ed9aa68`)
- **GPU performance** — removed 12 backdrop-filter blur() calls and 3 infinite CSS animations (`ed9aa68`)

### WebUI
- Modern glass morphism overhaul with AMOLED dark gradients (`c3b5ab6`)
- 29 bug fixes including bounds checks, null guards, race conditions (`97d2b97`)
- 6 accent color presets with randomization toggle
- Signature rotating rainbow ring logo
- Light/dark mode, RTL, accessibility (focus states, ARIA)

## v4.3-auto (2026-01-20)

### Initial Release
- **Dual-source keybox** with 4-source failover (Yurikey → Upstream → IntegrityBox → Bundled)
- **Auto security patch updates** — system/boot/vendor dates from Google
- **Module conflict detection** — 16 regular + 1 aggressive + 2 app conflicts
- **WebUI** with automation settings
- **POSIX sh / BusyBox ash** compatible — no bashisms, properly quoted, injection-safe
- Supports Magisk, KernelSU (32234+), APatch (11159+)

---
