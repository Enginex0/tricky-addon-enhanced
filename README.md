<p align="center">
  <h1 align="center">⚡ Tricky Addon Enhanced</h1>
  <p align="center"><b>Automated TrickyStore & TEESimulator Management</b></p>
  <p align="center">Flash once. Forget forever.</p>
  <p align="center">
    <img src="https://img.shields.io/github/v/release/Enginex0/tricky-addon-enhanced?style=for-the-badge&color=orange" alt="Version">
    <img src="https://img.shields.io/github/actions/workflow/status/Enginex0/tricky-addon-enhanced/build.yml?style=for-the-badge&label=build" alt="Build">
    <img src="https://img.shields.io/badge/License-GPLv3-blue?style=for-the-badge" alt="License">
    <img src="https://img.shields.io/badge/Rust-native-B7410E?style=for-the-badge&logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/Telegram-community-blue?style=for-the-badge&logo=telegram" alt="Telegram">
  </p>
  <p align="center">
    <img src="https://img.shields.io/badge/KernelSU-32234%2B-green?style=for-the-badge" alt="KernelSU">
    <img src="https://img.shields.io/badge/APatch-11159%2B-purple?style=for-the-badge" alt="APatch">
    <img src="https://img.shields.io/badge/Magisk-20.4%2B-00AF9C?style=for-the-badge&logo=magisk" alt="Magisk">
  </p>
</p>

---

> [!NOTE]
> **This is a personal project built for automation enthusiasts — flash and forget.**
>
> It was never planned for public release, but enough people wanted it, so here it is. It's open-source because sharing is good — not because anyone is owed support, features, or responses on a timeline. If something breaks, report it on [Telegram](https://t.me/superpowers9). PRs are welcome. Entitlement is not.
>
> **Do NOT report issues to TrickyStore, TEESimulator, or any upstream project.** They have nothing to do with this module. All support goes through one place: the [SuperPowers Telegram](https://t.me/superpowers9).

---

## 🧬 What is Tricky Addon Enhanced?

A native Rust daemon that manages TrickyStore and TEESimulator silently in the background — keybox rotation, security patch spoofing, VBHash injection, attestation engine monitoring, and target list management — all running as a single process with zero user intervention after install.

> **This is a clean-room rewrite.** The original concept drew from several community projects, but every line of backend logic has been rebuilt from scratch in Rust with a completely different architecture — a single native daemon replacing 6 shell processes, inotify replacing poll loops, in-memory config replacing fork-per-read patterns. No commits were squashed or rebased from any other project. This is new code.

---

## 🔥 Why Rust?

The previous shell-based approach worked, but it was fragile. Config reads forked a new process every time. App detection polled in a sleep loop. Six separate scripts competed for resources. The result was hundreds of thousands of unnecessary wakeups per day, constant JVM spawns, and race conditions that caused intermittent failures users couldn't diagnose.

The Rust rewrite eliminates all of that:

| Metric | Shell | Rust |
|---|---|---|
| **Wakeups/day** | 884,449 | ~100 (timer fires only) |
| **JVM spawns/day** | 20,170 | ~200 (only for `pm` when inotify can't detect) |
| **Processes** | 6 | 1 |
| **Background CPU** | ~28 min/day | <1 min/day |
| **Config reads** | 43,200 forks/day | 0 (in-memory struct) |
| **App detection latency** | 10s–minutes | Instant (inotify) |

One binary. One process. One config file. No shell scripts in the hot path.

---

## ✨ Features

**Keybox Management**
- [x] **3-source failover** — Yurikey → KOW → Custom, with automatic rotation
- [x] **XML validation** before applying — never installs a broken keybox
- [x] **Automatic backup** — existing keybox backed up before every replacement
- [x] **Custom keybox protection** — set source to `custom` and it stays untouched
- [x] **Device keybox generation** — ECDSA P-256 + RSA-2048 keygen for AOSP-level attestation
- [x] **Configurable fetch interval** — preset chips (1h–7d) or custom value with min/hr/day unit toggle from WebUI

**Security Patch Automation**
- [x] **Engine-aware patching** — auto-detects James Fork, standard TrickyStore, or TEESimulator
- [x] **All three dates** — system, boot, and vendor patch levels set on boot and daily
- [x] **Latest patch fetch** — pulls current dates from Google's Pixel bulletin

**VBHash Spoofing**
- [x] **15 properties spoofed** — `vbmeta.digest`, `device_state`, `verifiedbootstate`, and 12 more
- [x] **Captured at install time** — reads `ro.boot.vbmeta.digest` before any modules are active
- [x] **Fallback chain** — persisted file → bootloader property → APK extraction (last resort)

**Custom ROM Identity Hiding**
- [x] **LineageOS prop scrub on boot:** strips the `lineage_` prefix from `ro.product.vendor.name`, replaces `org.lineageos.aperture` in `vendor.camera.aux.packagelist` and `persist.vendor.camera.privapp.list`, stops `vendor.lineage_health` and deletes its `init.svc.*` status prop
- [x] **Stock-safe gating:** every block matches a LineageOS-only signature before writing, so stock ROMs trigger nothing

**Attestation Engine Health Monitor**
- [x] **Auto-restart on crash** — polls every 10s, detects TEESimulator or TrickyStore, restarts if dead
- [x] **Grace period** — 5s window for the engine's internal restart loop before intervening
- [x] **Restart tracking** — count persisted to `.health_state`, visible in WebUI

**Target List Automation**
- [x] **inotify-based app detection** — new installs added to `target.txt` instantly
- [x] **Xposed exclusion** — auto-detects and excludes Xposed modules
- [x] **Denylist merge** — optionally pulls from Magisk denylist

**Conflict Detection**
- [x] **19 conflicting modules** detected and handled (auto-remove, block, or warn)

**Live Status Monitor**
- [x] **Module description updates** every 30s — app count, keybox source, patch level, VBHash state

```
⚡ 37 Apps │ 🔑 Yurikey │ 🛡️ 2026-01-05 │ 🔒 VBHash
```

Real-time status directly in your module manager — no need to open anything.

**WebUI**
- [x] **Glass morphism design** — AMOLED dark gradient (`#0F0F1A` → `#1A1A2E`), 6 accent color presets with random selection on launch
- [x] **Health status banner** — live engine state (green/red/orange)
- [x] **Keybox status pill:** green `OK`, blue `AOSP` when an AOSP-rooted keybox is loaded on a non-AOSP device, amber `Invalid` for structural failures, gray `No Keybox` when absent. Hover surfaces the full validation error list. Revocation is exposed as JSON metadata but no longer drives the badge color since TEESimulator routes around Google's attestation status list at runtime
- [x] **Keybox automation panel** — 6 source cards, interval scheduler with preset chips (1h–7d) and custom input with min/hr/day toggle, manual fetch
- [x] **Target list auto-refresh** — every 3s, with search and per-app mode control
- [x] **23 languages** with RTL support
- [x] **Single batched init** — one shell call loads the entire UI state

**Set It and Forget It**

A single native daemon manages all background tasks — if anything dies, it restarts within 1s:

- [x] **App Watcher** — auto-adds new installations to `target.txt` via inotify
- [x] **Xposed Detection** — auto-excludes Xposed modules from targeting
- [x] **Health Monitor** — auto-restarts attestation engine on crash
- [x] **Status Monitor** — live module description updates every 30s
- [x] **Conflict Detection** — warns about 19 conflicting modules on boot
- [x] **Log Rotation** — 1MB limit with automatic cleanup

---

## 📱 Compatibility

| Root Manager | Minimum Version | WebUI Support |
|---|---|---|
| **KernelSU** | 32234+ | Built-in |
| **APatch** | 11159+ | Built-in |
| **Magisk** | 20.4+ | [KSUWebUIStandalone](https://github.com/5ec1cff/KSUWebUIStandalone) or [WebUI-X](https://github.com/5ec1cff/WebUI-X) required |

**Requires:** [TEESimulator v4](https://github.com/JingMatrix/TEESimulator) or [TrickyStore](https://github.com/5ec1cff/TrickyStore) as the attestation engine.

---

## 🚀 Installation

1. Download the [latest release](https://github.com/Enginex0/tricky-addon-enhanced/releases/latest)
2. Install via your root manager
3. **No reboot required** on KSU and APatch (hot install). Magisk applies on the next boot.

During install, press **Vol−** for manual target mode (GMS/GSF only) or **Vol+** / wait 10s for full automation.

Conflicting modules are detected and `rm -rf`'d at install time, so an old TA fork or competing keybox/VBHash module is removed automatically.

The module captures VBHash, configures protected apps, manages the keybox and patch levels, and starts the daemon. With TEESimulator v4, changes are applied only to the selected native profile.

TEESimulator and Tricky Addon Enhanced keep separate WebUIs.

One TEESimulator profile is selected automatically. With multiple profiles, choose one under **Tricky Addon Enhanced → Automation Settings → TEESimulator Profile**, or run:

```sh
ta-enhanced automation select-profile PROFILE_NAME
```

---

## 🔨 Building from Source

Requires Rust toolchain and Android NDK.

```bash
git clone https://github.com/Enginex0/tricky-addon-enhanced.git
cd tricky-addon-enhanced
bash package.sh --no-bump
```

The ZIP lands in `release/`. CI also builds on every push — grab artifacts from the [Actions tab](https://github.com/Enginex0/tricky-addon-enhanced/actions/workflows/build.yml).

<details>
<summary><b>Build options</b></summary>

| Flag | Effect |
|---|---|
| *(none)* | Bump version, cross-compile, package |
| `--no-bump` | Build without incrementing version |
| `--no-build` | Package only (skip Rust compile) |
| `--clean` | Remove old ZIPs before packaging |

</details>

---

## ⚙️ Configuration

All settings are configurable from the **WebUI** (open from your root manager) or via CLI:

```bash
ta-enhanced config get keybox.source
ta-enhanced config set keybox.interval 3600
```

Addon config lives at `/data/adb/tricky_store/ta-enhanced/config.toml` and is preserved across reinstalls.

<details>
<summary><b>Config Reference</b></summary>

| Key | Default | Description |
|---|---|---|
| `keybox.enabled` | `true` | Auto keybox fetching |
| `keybox.source` | `yurikey` | Primary source (`yurikey`, `upstream`, `custom`) |
| `keybox.interval` | `300` | Seconds between fetch attempts |
| `security_patch.auto_update` | `true` | Auto patch date updates |
| `security_patch.interval` | `86400` | Seconds between patch checks |
| `automation.enabled` | `true` | Auto-populate TrickyStore targets or TEESimulator profile apps |
| `automation.use_inotify` | `true` | Use inotify for instant app detection |
| `health.enabled` | `true` | Attestation engine health monitor |
| `health.interval` | `10` | Seconds between health checks |
| `conflict.enabled` | `true` | Conflicting module detection |
| `vbhash.enabled` | `true` | VBHash spoofing |

</details>

<details>
<summary><b>File Locations</b></summary>

```
/data/adb/teesim/              # TEESimulator-owned files
├── config.json               # Profiles, apps, and patch levels
└── *.xml                     # Profile keyboxes

/data/adb/tricky_store/
├── target.txt                 # TrickyStore target list / TEESimulator UI mirror
├── keybox.xml                 # TrickyStore current keybox
├── keybox.xml.bak             # TrickyStore keybox backup
├── security_patch.txt         # TrickyStore patch dates
├── .health_state              # Health monitor state
└── ta-enhanced/
    └── config.toml            # Addon configuration

/data/adb/Tricky-addon-enhanced/logs/
├── daemon.log                 # Unified daemon log
└── conflict.log               # Conflict detection

/data/adb/boot_hash            # Persisted VBHash
```

</details>

---

## 🛠️ Troubleshooting

<details>
<summary><b>Fingerprint enrollment fails after install</b></summary>

On Snapdragon-class devices, the runtime VBMeta digest spoof can prevent enrollment of new fingerprints. Existing prints keep working; only adding new ones fails. Two escape hatches, available from the WebUI under **Automation → Compatibility**, or via terminal:

- `touch /data/adb/disable_vbmeta_digest_spoof` — keeps Play Integrity spoofing, drops only the digest rewrite. Try this first.
- `touch /data/adb/disable_prop_handler` — disables the entire boot-state spoof. Use only if the first doesn't help.

Reboot after either change. Remove the file to restore the spoof.

</details>

---

## 💬 Community

```
$ ta-enhanced --connect

 ███████╗██╗   ██╗██████╗ ███████╗██████╗
 ██╔════╝██║   ██║██╔══██╗██╔════╝██╔══██╗
 ███████╗██║   ██║██████╔╝█████╗  ██████╔╝
 ╚════██║██║   ██║██╔═══╝ ██╔══╝  ██╔══██╗
 ███████║╚██████╔╝██║     ███████╗██║  ██║
 ╚══════╝ ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═╝
              POWERS

 [✓] SIGNAL    ──→  t.me/superpowers9
 [✓] UPLINK    ──→  bug reports · feature drops · dev updates
 [✓] STATUS    ──→  OPEN — all operators welcome
```

<p align="center">
  <a href="https://t.me/superpowers9">
    <img src="https://img.shields.io/badge/⚡_JOIN_THE_GRID-SuperPowers_Telegram-black?style=for-the-badge&logo=telegram&logoColor=cyan&labelColor=0d1117&color=00d4ff" alt="Telegram">
  </a>
</p>

---

## 🙏 Credits

- **[KOWX712](https://github.com/KOWX712)** — original Tricky Addon concept and WebUI foundation
- **[JingMatrix](https://github.com/JingMatrix)** — TEESimulator
- **[5ec1cff](https://github.com/5ec1cff/TrickyStore)** — TrickyStore attestation module
- **[XtrLumen/TS-Enhancer-Extreme](https://github.com/XtrLumen/TS-Enhancer-Extreme)** — VBHash extraction concept
- **[Yurikey](https://github.com/Yurii0307/yurikey)** — primary keybox source
- **[Zero-Mount](https://github.com/Enginex0/zeromount)** — WebUI design inspiration
- **[j-hc/zygisk-detach](https://github.com/nickcao/zygisk-detach)** — WebUI template

---

## 📄 License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

---

<p align="center">
  <b>⚡ Flash once. Forget forever.</b>
</p>
