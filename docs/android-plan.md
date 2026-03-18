# Android Port Feasibility Analysis — mapxr

## Current Codebase State

| Component | File(s) | Android status |
|-----------|---------|----------------|
| mapping-core (engine) | `crates/mapping-core/` | Pure Rust — compiles to Android as-is |
| tap-ble (BLE layer) | `crates/tap-ble/` | btleplug 0.12 has Android backend; needs NDK config |
| Input injection | `src-tauri/` via `enigo 0.2` | enigo has no Android support — must be replaced |
| Tauri shell | `src-tauri/src/lib.rs` | `#[cfg_attr(mobile, tauri::mobile_entry_point)]` already present |
| Svelte UI | `src/` | WebView-based, works on Android unchanged |
| Platform paths | `src-tauri/src/platform.rs` | Android path needs adding |
| App state | `src-tauri/src/state.rs` | No platform assumptions |

The codebase is already partially structured for Android. The hard problems are **key injection** and **background persistence** — not the mapping engine or UI.

---

## Key Injection Options

### Option A — AccessibilityService (typing into the Android device itself)

Android API 28+ allows `AccessibilityService.dispatchKeyEvent()` to inject key events into whatever app is in the foreground.

Supports: letters, numbers, modifier keys (Ctrl/Shift/Alt), function keys, arrow keys.

**Limitations:**
- User must manually enable in Settings → Accessibility → mapxr (one-time friction)
- Blocked by some apps (banking apps, secure keyboards, lock screen)
- ~5% of Android OEMs interfere with Accessibility Services
- Requires a Tauri Kotlin plugin replacing enigo

### Option B — BluetoothHidDevice (phone relays keystrokes to a paired computer) — Recommended

Android API 28+ supports `BluetoothHidDevice` — the phone registers itself as a Bluetooth HID keyboard/mouse with a target computer.

**Architecture:** Tap Strap → (BLE) → Android phone running mapxr → (BT HID) → computer

The target computer sees a standard Bluetooth keyboard — no software needed on the computer. Works with Windows, macOS, Linux, iPadOS, another Android device.

**Advantages:**
- No Accessibility permission required (just standard Bluetooth)
- Full HID key report support: any key, any modifier combo, media keys, function keys
- The phone is the central hub — serves any paired target device
- Less OEM-specific friction than Accessibility Services

**Limitations:**
- Phone and computer must be Bluetooth paired (one-time setup)
- ~2-3m Bluetooth range between phone and computer (not a real constraint)
- The phone cannot simultaneously be the device you're typing INTO (requires Option A for that)

### Option C — Input Method Editor (IME / virtual keyboard)

Only works when a text field is focused and the user has selected mapxr as their keyboard. Cannot inject modifier keys, function keys, or non-text events. Not viable as the primary mechanism.

**Recommendation:** Build Option B (BT HID relay) as the primary mode. Option A (Accessibility) can be added as a secondary mode for phone-native typing.

---

## Background Persistence

For mapxr to work as a keyboard interface, it must stay alive in the background continuously.

### Stock Android (manageable)

- **Foreground Service** with persistent notification: keeps the process alive, immune to standard background process killing. Required since Android 8.0 (API 26).
- `FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE` permission (for BLE connections)
- Battery optimization exemption: request user grant "unrestricted battery usage" via `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`
- These two together handle stock Android (Google Pixel) reliably.

### OEM Customizations (the real problem, affects ~50% of Android users)

Chinese and Korean OEMs add aggressive app-killing on top of Android:

| OEM / OS | Issue | User-facing fix required |
|----------|-------|--------------------------|
| Xiaomi / MIUI | Kills apps without "Autostart" permission | Settings → Apps → mapxr → Autostart: ON |
| Samsung One UI | "Sleeping apps" feature | Battery → Background usage limits → Never sleeping |
| Huawei / EMUI | App launch manager | Settings → Battery → App launch: manual manage mapxr |
| Oppo / ColorOS | Similar to MIUI | Settings → Battery → App quick freeze: exclude mapxr |
| OnePlus (OxygenOS ≥ 12) | Uses ColorOS base | Same as Oppo |
| Stock Android (Pixel) | Standard Android only | Battery optimization exemption is sufficient |

The website dontkillmyapp.com documents all OEM-specific steps. The app should deep-link directly to the relevant settings screen using `ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS` or manufacturer-specific intent actions.

**Realistic reliability:**
- Google Pixel: ~99% background persistence
- Samsung (with user following setup): ~90%
- Xiaomi/MIUI (with user following setup): ~80%
- Without user setup on problematic OEMs: ~40%

---

## BLE Layer on Android

btleplug 0.12 has an Android backend via the `android` feature flag. Alternatively, a Kotlin Tauri plugin can expose Android's BLE API directly (more control, easier debugging, no NDK complexity).

**Recommended approach:** Write the BLE connection as a Kotlin Tauri plugin. The tap-ble packet parsing (`crates/tap-ble/src/packet_parser.rs`) and the full mapping engine (`crates/mapping-core/`) are pure Rust and compile to Android targets unchanged.

---

## Required Android Permissions

```xml
<uses-permission android:name="android.permission.BLUETOOTH" />
<uses-permission android:name="android.permission.BLUETOOTH_ADMIN" />
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />  <!-- API 31+ -->
<uses-permission android:name="android.permission.BLUETOOTH_SCAN" />     <!-- API 31+ -->
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" /> <!-- BLE scan pre-API31 -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />
<uses-permission android:name="android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS" />
<!-- If using AccessibilityService: -->
<uses-permission android:name="android.permission.BIND_ACCESSIBILITY_SERVICE" />
```

---

## Recommended Architecture

```
Android Foreground Service (always running, persistent notification)
│
├── Kotlin BLE Plugin
│   ├── BluetoothLeGatt connection to Tap Strap
│   ├── Keepalive + auto-reconnect
│   └── Raw tap_code events → Rust via JNI
│
├── Rust mapping-core (ComboEngine + LayerStack)
│   └── Processes tap events → resolved actions (unchanged from desktop)
│
├── Output Dispatcher
│   ├── BluetoothHidDevice plugin (primary — relay to paired computer)
│   └── AccessibilityService plugin (secondary — type into Android itself)
│
└── Tauri WebView UI (existing Svelte frontend, unchanged)
    ├── Profile editor
    ├── Device pairing (Tap Strap + HID target)
    └── Service management + OEM setup wizard
```

The Tauri `lib.rs` entry point is already annotated with `#[cfg_attr(mobile, tauri::mobile_entry_point)]`. The Svelte UI needs no changes. Custom Kotlin plugins communicate via Tauri's plugin event/invoke system.

---

## Phased Effort Estimate

| Phase | Work | Effort |
|-------|------|--------|
| 1 | Tauri Android project setup + UI running on phone | 1–2 days |
| 2 | Kotlin BLE plugin (scan + connect Tap Strap in foreground) | 3–5 days |
| 3 | Foreground Service + battery exemption + OEM setup wizard | 2–3 days |
| 4B | BluetoothHidDevice plugin (HID relay to computer) | 3–5 days |
| 4A | AccessibilityService plugin (type into Android itself) | 2–3 days |
| 5 | Android path handling in `src-tauri/src/platform.rs` | 0.5 days |
| 6 | Integration testing across devices | ongoing |

**Total to working Phase 4B (most compelling feature): ~10–15 days of focused work.**

---

## What Works, What Doesn't, What's Impossible

**Works well on Android:**
- Full mapping engine (layers, sequences, variables, conditionals) — zero changes
- Profile editor UI — zero changes
- BLE connection to Tap Strap — manageable with Kotlin plugin
- BluetoothHidDevice relay to a computer — full key support

**Genuinely hard:**
- Background persistence on OEM devices — requires user setup on ~50% of Android phones
- AccessibilityService — requires one-time user setup, blocked by some apps
- Play Store distribution — Google may flag an app using Accessibility + BLE keyboard as policy-sensitive; F-Droid or sideloaded APK is the safer path

**Impossible without root:**
- Injecting keys into the lock screen
- Bypassing OEM background-kill on devices that aggressively block all workarounds
- Intercepting other Bluetooth devices' input

---

## Verdict

**Yes, an Android port is worth building**, particularly with the BluetoothHidDevice relay model (Option B). It enables a compelling use case: phone-as-adapter — carry one phone and one Tap Strap and type on any Bluetooth-capable device. The OEM background-kill issue is real but manageable with a good setup wizard.

The biggest risk is Play Store policy. F-Droid or sideloaded APK distribution avoids this entirely and aligns with the open-source nature of the project.
