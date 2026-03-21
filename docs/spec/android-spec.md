---
covers: Epic 15 (Android port — Phase 1) and Phase 2 (BluetoothHidDevice relay, post-release)
status: Draft — awaiting approval
last-updated: 2026-03-20
---

# Android Port — Specification

## Table of contents

1. [Overview and scope](#1-overview-and-scope)
2. [Architecture](#2-architecture)
3. [Project setup — Tauri Android](#3-project-setup--tauri-android)
4. [Kotlin BLE plugin](#4-kotlin-ble-plugin)
5. [Android Foreground Service](#5-android-foreground-service)
6. [OEM battery setup wizard](#6-oem-battery-setup-wizard)
7. [Phase 1 — AccessibilityService key injection](#7-phase-1--accessibilityservice-key-injection)
8. [Platform paths and preferences storage](#8-platform-paths-and-preferences-storage)
9. [UI adaptations](#9-ui-adaptations)
10. [Permissions manifest](#10-permissions-manifest)
11. [Phase 2 — BluetoothHidDevice relay](#11-phase-2--bluetoothhiddevice-relay)
12. [Tauri command and event surface](#12-tauri-command-and-event-surface)
13. [Testing strategy](#13-testing-strategy)
14. [Distribution](#14-distribution)
15. [Known limitations and non-goals](#15-known-limitations-and-non-goals)

---

## 1. Overview and scope

The Android port brings the full mapxr experience to Android phones and tablets. The Tap
wearable device connects to the phone over BLE; the phone runs the same mapping engine
(`mapping-core`) and the same Svelte profile editor UI as the desktop app. Mapped tap actions
are dispatched as real key/mouse events into whatever app is in the foreground.

The port is implemented in two phases with a release between them:

### Phase 1 — Direct input into Android (Epic 15, this spec)

The phone is the target device. Tap actions are injected into the currently focused Android
app via an `AccessibilityService`. This mirrors the desktop app's behaviour exactly: connect
a Tap Strap, load a profile, and the Tap becomes a keyboard/mouse for the phone.

**Deliverable:** A published APK (GitHub Releases and/or F-Droid) that lets users use their
Tap device as a keyboard for Android apps.

### Phase 2 — BluetoothHidDevice relay (separate release, post Phase 1)

The phone acts as a Bluetooth HID keyboard adapter. Tap actions are relayed from the phone
to a paired computer via Bluetooth HID. The target computer sees a standard Bluetooth keyboard
with no software required. Phase 2 is specified in §11 but **must not be implemented until
Phase 1 is released**.

---

## 2. Architecture

```
Android Foreground Service (persistent notification, immune to background kill)
│
├── Kotlin BLE Plugin                    [§4]
│   ├── BluetoothLeGatt → Tap Strap
│   ├── Keepalive + auto-reconnect
│   └── Raw tap bytes → Rust JNI bridge
│
├── Rust mapping-core (unchanged)        [crates/mapping-core/]
│   └── ComboEngine + LayerStack → resolved Actions
│
├── Phase 1: AccessibilityService plugin [§7]
│   └── dispatchKeyEvent() into foreground app
│
├── Phase 2: BluetoothHidDevice plugin   [§11]
│   └── HID key reports → paired computer
│
└── Tauri WebView UI                     [apps/desktop/src/]
    ├── Profile editor (unchanged)
    ├── Device pairing screen
    ├── Accessibility permission prompt  [Phase 1]
    ├── HID target pairing screen        [Phase 2]
    └── OEM battery setup wizard
```

### Component reuse summary

| Component | Reuse status |
|-----------|-------------|
| `crates/mapping-core/` | Unchanged — pure Rust, compiles to Android as-is |
| `apps/desktop/src/` (Svelte UI) | Unchanged for Phase 1; minor additions for Phase 2 |
| `apps/desktop/src-tauri/src/commands.rs` | Android-specific commands added via `#[cfg(target_os = "android")]` |
| `crates/tap-ble/` | **Not used on Android** — replaced by Kotlin BLE plugin (§4) |
| `apps/desktop/src-tauri/src/platform.rs` | Android path added (§8) |
| `apps/desktop/src-tauri/src/pump.rs` | Adapted: tap events arrive from Kotlin plugin instead of `tap-ble` |

---

## 3. Project setup — Tauri Android

### 3.1 Tauri mobile target

The `apps/desktop/src-tauri/src/lib.rs` entry point already has:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() { ... }
```

No changes are required to this entry point.

Enable the Android target in `apps/desktop/src-tauri/Cargo.toml`:

```toml
[target.'cfg(target_os = "android")'.dependencies]
tauri = { version = "2", features = ["mobile"] }
```

### 3.2 Android project initialisation

Run `cargo tauri android init` to generate the Android project under
`apps/desktop/src-tauri/gen/android/`. This directory is committed to the repository.

The generated Android project structure:

```
apps/desktop/src-tauri/gen/android/
├── app/
│   ├── src/main/
│   │   ├── AndroidManifest.xml       ← permissions (§10)
│   │   ├── java/com/mapxr/app/
│   │   │   ├── MainActivity.kt       ← Tauri entry point (generated)
│   │   │   ├── BlePlugin.kt          ← BLE plugin (§4)
│   │   │   ├── MapxrForegroundService.kt  ← Foreground Service (§5)
│   │   │   ├── AccessibilityPlugin.kt     ← Accessibility plugin (§7)
│   │   │   └── HidPlugin.kt              ← HID relay plugin (§11, Phase 2)
│   │   └── res/
│   │       ├── drawable/             ← app icons
│   │       └── xml/
│   │           └── accessibility_service_config.xml  ← (§7)
│   └── build.gradle
└── build.gradle
```

### 3.3 Minimum SDK version

- `minSdkVersion`: **26** (Android 8.0 Oreo) — required for `FOREGROUND_SERVICE`
- `targetSdkVersion`: **34** (Android 14) — current Play Store requirement
- `compileSdkVersion`: **34**

AccessibilityService key injection requires API 28+. On API 26–27 devices, the
AccessibilityService will still initialise but `dispatchKeyEvent()` silently does nothing;
a UI warning must be shown to users on these older OS versions indicating that direct key
injection is unavailable.

### 3.4 Build variants

| Variant | Description |
|---------|-------------|
| `debug` | Standard debug build; used during development |
| `release` | Signed APK for distribution; signing config from `ANDROID_KEYSTORE_*` environment variables |

The existing GitHub Actions release workflow (`release.yml`) gains an Android job in Phase 1
(see §14).

---

## 4. Kotlin BLE plugin

The `crates/tap-ble/` crate is **not used on Android**. The Tap BLE protocol is implemented
entirely in Kotlin as a Tauri plugin. The tap packet parser and `mapping-core` engine are
called via JNI.

### 4.1 Plugin registration

`BlePlugin` is a Tauri Kotlin plugin registered in `MainActivity.kt`:

```kotlin
class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        registerPlugin(BlePlugin::class.java)
        registerPlugin(AccessibilityPlugin::class.java)
        // Phase 2: registerPlugin(HidPlugin::class.java)
    }
}
```

### 4.2 BLE scanning

`BlePlugin` exposes a `startScan()` plugin command. Scanning uses Android's
`BluetoothLeScanner` API. Only devices advertising the Tap GATT service UUID
(`0000FEE0-0000-1000-8000-00805F9B34FB`) are surfaced.

When a device is found, the plugin emits a `ble-device-found` event to the WebView with:

```typescript
interface BleDeviceFoundPayload {
  address: string;   // MAC address (Android uses address, not UUID)
  name: string | null;
  rssi: number;
}
```

Scanning stops automatically after 30 seconds or when `stopScan()` is called.

### 4.3 Connection and GATT setup

`connect(address: string)` connects to the named device and performs the same GATT setup
sequence as `crates/tap-ble/src/tap_device.rs`:

1. Discover services
2. Find the Tap data characteristic (`0000FEE1-0000-1000-8000-00805F9B34FB`)
3. Enable notifications on the characteristic
4. Enter controller mode by writing the controller mode byte (`0x0C 0x00`)
5. Emit `ble-device-connected` event to the WebView

Connection state changes emit `ble-device-connected` / `ble-device-disconnected` events
matching the existing event signatures used by the desktop app (defined in
`apps/desktop/src/lib/events.ts`).

### 4.4 Tap data stream and JNI bridge

On each characteristic notification, the Kotlin plugin receives a raw byte array. It passes
this to the Rust `tap_packet_parse` JNI function exported from `mapping-core`:

```kotlin
// Native function declared in Kotlin, implemented in Rust via JNI
private external fun processTapBytes(deviceAddress: String, bytes: ByteArray): String
```

The Rust JNI function:
1. Parses the raw bytes via the same parser used in `tap-ble` (extracted to a `pub fn` in
   `crates/mapping-core/src/tap_parser.rs` or reused from `crates/tap-ble/src/packet_parser.rs`
   — exact location is an implementation detail to decide at task time)
2. Pushes the resulting `RawTapEvent` into the `ComboEngine`
3. Returns a JSON-encoded `Vec<Action>` as a UTF-8 string

The Kotlin plugin deserialises the returned JSON and dispatches each action:
- `KeyPress`, `KeyRelease`, `TypeText` → `AccessibilityPlugin` (Phase 1) or `HidPlugin` (Phase 2)
- `MouseClick`, `MouseScroll` → `AccessibilityPlugin` (Phase 1) or `HidPlugin` (Phase 2)
- `Vibrate` → direct BLE write to the Tap device characteristic
- `LayerSwitch`, `ProfileSwitch` → emitted as Tauri events to the WebView

### 4.5 Reconnection

The Kotlin BLE plugin implements the same reconnection policy as the desktop `tap-ble` crate:

- On unexpected disconnect: wait 1 second, then attempt reconnect up to 5 times with
  exponential backoff (1s, 2s, 4s, 8s, 16s)
- After 5 failed attempts: emit `ble-device-disconnected` with `reason: "reconnect_failed"`
  and stop attempting; the user must manually reconnect via the UI
- On foreground-to-background transition: the `MapxrForegroundService` (§5) keeps the GATT
  connection alive; no special reconnect handling is needed

### 4.6 Known Android BLE quirks

These are implementation notes, not design decisions — the implementer must handle them:

- **Address randomisation:** Devices with randomised MAC addresses may present a different
  address after re-pairing. The device registry must store both address and a stable identifier
  (the Tap device serial extracted from GATT characteristics) to handle this.
- **Connection parameter negotiation:** Some Android OEMs throttle BLE connection intervals;
  request a 7.5ms–15ms connection interval after connection for low-latency tap delivery.
- **GATT cache:** After reconnecting to a previously seen device, call `refreshGattCache()`
  (via reflection on `BluetoothGatt`) before rediscovering services to avoid stale GATT cache
  issues that affect several OEMs.

---

## 5. Android Foreground Service

### 5.1 Purpose

Android kills background processes aggressively. A `Foreground Service` (introduced in
API 26) keeps the process alive by displaying a persistent notification. It is the standard
mechanism for apps that must remain active (music players, navigation apps, etc.).

### 5.2 Service definition

`MapxrForegroundService` extends `android.app.Service`. It is started when:
- The first Tap device connects, or
- The app moves to the background while a device is connected

It is stopped when:
- All Tap devices disconnect **and** the app is in the foreground, or
- The user explicitly quits the app from the notification

### 5.3 Persistent notification

The notification is required by Android and must be visible at all times while the service
runs. Spec:

| Field | Value |
|-------|-------|
| Channel | `mapxr_service` (importance: `LOW` — no sound, no heads-up) |
| Icon | Small monochrome app icon (`ic_notification.png`, 24dp) |
| Title | `"MapXr active"` |
| Content text | `"<N> device(s) connected · <profile name>"` or `"No devices connected"` |
| Actions | **Stop** button — disconnects all devices and stops the service |

The notification content updates when the active profile changes or device connections change.

### 5.4 Service type

```xml
<service
    android:name=".MapxrForegroundService"
    android:foregroundServiceType="connectedDevice"
    android:exported="false" />
```

`connectedDevice` is the correct service type for apps that maintain BLE connections in the
background (Android 14+ requires explicit service types).

### 5.5 Battery optimisation exemption

On first launch, the app requests battery optimisation exemption via
`ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`. This is required on stock Android to prevent
the system from killing the foreground service on low-battery conditions.

This request is shown once, gated behind a "Set up background operation" onboarding step.
The result (granted / denied) is stored in `preferences.json`.

---

## 6. OEM battery setup wizard

### 6.1 Motivation

~50% of Android users are on OEM builds (Samsung, Xiaomi, Oppo, OnePlus, Huawei) that add
their own app-killing layers on top of the Android foreground service system. These cannot be
worked around programmatically — the user must enable specific per-OEM settings.

### 6.2 Wizard trigger

The wizard is shown:
- Once during first-run onboarding (after device pair, before first use)
- On demand from Settings → "Background operation setup"

### 6.3 OEM detection and deep-linking

The wizard detects the device manufacturer via `Build.MANUFACTURER` (lowercased) and shows
OEM-specific instructions and a direct-link button where possible:

| Manufacturer | Setting | Intent / deep link |
|---|---|---|
| `xiaomi` | Settings → Apps → mapxr → Autostart: ON | `miui.intent.action.APP_PERM_EDITOR` |
| `samsung` | Settings → Battery → Background usage limits → Never sleeping | `android.settings.APPLICATION_DETAILS_SETTINGS` |
| `huawei` / `honor` | Settings → Battery → App launch: manage manually | `huawei.intent.action.HSM_SETTINGS` (best-effort) |
| `oppo` / `oneplus` / `realme` | Settings → Battery → App quick freeze: exclude | `android.settings.APPLICATION_DETAILS_SETTINGS` |
| other | Show generic "disable battery optimisation" instructions | `ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS` |

All OEMs: also show the generic battery optimisation request (§5.5) as the baseline.

### 6.4 Wizard UI

The wizard is a Svelte component (`BatterySetupWizard.svelte`) rendered inside the WebView.
It is shown as a full-screen modal. Steps:

1. **Why this is needed** — brief explanation: mapxr must run in the background to keep your
   Tap Strap connected and responsive
2. **Grant battery optimisation exemption** — button triggers `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`;
   shows success/failure feedback
3. **OEM-specific step** (shown only on affected manufacturers) — manufacturer-specific
   instructions with a "Go to settings" button that deep-links to the relevant screen
4. **Done** — the wizard records completion to `preferences.json`

The wizard can be dismissed at any step. Dismissal does not prevent future access from Settings.

---

## 7. Phase 1 — AccessibilityService key injection

### 7.1 Mechanism

Android API 28+ allows an `AccessibilityService` to call `dispatchKeyEvent(KeyEvent)` to
inject a key event into whatever app currently has focus. This is the only non-root mechanism
that supports the full key set (letters, numbers, modifiers, function keys, arrow keys, media
keys) into arbitrary apps.

### 7.2 AccessibilityService definition

`MapxrAccessibilityService` extends `android.accessibilityservice.AccessibilityService`.
It is a **minimal** accessibility service — it declares no event types (no window observation,
no content scanning, no screen reading). Its sole purpose is to receive key injection calls
from the Kotlin plugin via a bound service interface.

```xml
<!-- res/xml/accessibility_service_config.xml -->
<accessibility-service
    android:accessibilityEventTypes="typeNone"
    android:accessibilityFlags="flagDefault"
    android:canPerformGestures="false"
    android:canRetrieveWindowContent="false"
    android:description="@string/accessibility_description"
    android:settingsActivity="com.mapxr.app.MainActivity" />
```

The `android:description` string must clearly state: `"MapXr uses this permission to forward
tap gestures as keystrokes to apps on your phone."` This wording is chosen to be transparent
and honest on any Play Store privacy review.

### 7.3 AccessibilityPlugin Tauri plugin

`AccessibilityPlugin` is a Tauri Kotlin plugin that:

1. Checks whether `MapxrAccessibilityService` is enabled via `AccessibilityManager`
2. Exposes a `checkAccessibilityEnabled(): Boolean` plugin command used by the UI (§9.2)
3. Exposes an `openAccessibilitySettings()` plugin command that launches
   `Settings.ACTION_ACCESSIBILITY_SETTINGS`
4. Receives dispatch requests from the Kotlin BLE plugin's JNI return path and calls
   `MapxrAccessibilityService.dispatchKeyEvent()` via a bound service connection

### 7.4 Key mapping

Actions from `mapping-core` map to Android `KeyEvent` constants as follows:

| mapping-core `Key` variant | Android `KeyEvent.KEYCODE_*` |
|---|---|
| `Key::A` – `Key::Z` | `KEYCODE_A` – `KEYCODE_Z` |
| `Key::Num0` – `Key::Num9` | `KEYCODE_0` – `KEYCODE_9` |
| `Key::Space` | `KEYCODE_SPACE` |
| `Key::Return` | `KEYCODE_ENTER` |
| `Key::Backspace` | `KEYCODE_DEL` |
| `Key::Tab` | `KEYCODE_TAB` |
| `Key::Escape` | `KEYCODE_ESCAPE` |
| `Key::UpArrow` – `Key::RightArrow` | `KEYCODE_DPAD_UP` etc. |
| `Key::F1` – `Key::F24` | `KEYCODE_F1` – `KEYCODE_F12` (F13–F24 unsupported on Android) |
| `Key::MediaPlayPause` | `KEYCODE_MEDIA_PLAY_PAUSE` |
| `Key::MediaNextTrack` | `KEYCODE_MEDIA_NEXT` |
| `Key::MediaPrevTrack` | `KEYCODE_MEDIA_PREVIOUS` |
| `Key::MediaStop` | `KEYCODE_MEDIA_STOP` |
| `Key::VolumeUp` | `KEYCODE_VOLUME_UP` |
| `Key::VolumeDown` | `KEYCODE_VOLUME_DOWN` |
| `Key::VolumeMute` | `KEYCODE_VOLUME_MUTE` |
| `Key::Control` | `KEYCODE_CTRL_LEFT` |
| `Key::Shift` | `KEYCODE_SHIFT_LEFT` |
| `Key::Alt` | `KEYCODE_ALT_LEFT` |
| `Key::Meta` / `Key::Super` | `KEYCODE_META_LEFT` |
| `Key::Insert` | Not supported on Android — no-op, log warning |
| `Key::PrintScreen` | Not supported on Android — no-op, log warning |
| `Key::ScrollLock` | Not supported on Android — no-op, log warning |
| `Key::Pause` | Not supported on Android — no-op, log warning |

For `Action::TypeText { text }`: send a series of `KEYCODE_*` events for each character. For
characters without a direct keycode mapping, use `KeyEvent(KeyEvent.ACTION_MULTIPLE, 0)` with
`characters` set — this allows arbitrary Unicode text injection.

For `Action::MouseClick` and `Action::MouseScroll`: Android's `AccessibilityService`
provides `GestureDescription` for simulated touches. Mouse click maps to a tap gesture at the
centre of the focused view. Mouse scroll maps to a swipe gesture. These are best-effort: some
apps reject gesture injection from accessibility services. Log a warning on failure; do not
crash.

### 7.5 Permission UX

The user must manually enable the accessibility service in Android Settings. This is a
**one-time friction** that cannot be avoided or automated.

The app handles this as follows:

1. On first device connection, if the accessibility service is not enabled, show a
   `AccessibilitySetupPrompt.svelte` modal explaining what the permission does and why it is
   needed, with a "Open Accessibility Settings" button
2. After the user returns to the app, re-check whether it was enabled; show a success
   confirmation or retry prompt accordingly
3. The `preferences.json` records whether the user has completed this step
4. If the user has declined or not set it up, a persistent banner in the UI reads:
   `"Accessibility permission not granted — tap actions will not be dispatched."`

### 7.6 Limitations the user should know about

The settings page includes an informational note:

- **Banking and secure apps:** Key injection is blocked by apps that set
  `FLAG_SECURE` (banking apps, password managers, lock screen). This is intentional security
  behaviour; there is no workaround.
- **~5% OEM interference:** Some OEM builds interfere with Accessibility Services. The OEM
  setup wizard (§6) mitigates the most common cases.
- **API 26–27:** Key injection via `dispatchKeyEvent()` requires API 28. On older Android
  versions, a warning is shown and key injection is disabled.

---

## 8. Platform paths and preferences storage

### 8.1 Android config directory

`apps/desktop/src-tauri/src/platform.rs` currently returns paths for Windows, macOS, and
Linux. Add the Android case:

```rust
#[cfg(target_os = "android")]
pub fn config_dir() -> PathBuf {
    // Tauri on Android provides the app-specific internal storage path
    // via the Tauri path resolver; this is the standard approach
    tauri::api::path::app_data_dir(&tauri::Config::default())
        .expect("Android app data dir unavailable")
}
```

The exact implementation may use `app.path().app_data_dir()` from the Tauri path API —
the implementer should follow whatever Tauri 2 recommends for mobile at task time.

### 8.2 Files stored on Android

All files that exist on desktop also exist on Android, stored in the Android app's internal
storage (`/data/data/com.mapxr.app/files/`):

| File | Contents |
|------|----------|
| `profiles/` | Profile JSON files (identical format) |
| `preferences.json` | App preferences (extended with Android-specific fields — see §8.3) |
| `context-rules.json` | Context switching rules (Android context-switching is not implemented; file may be empty) |
| `device-registry.json` | Known Tap devices (Android uses MAC address as identifier) |

### 8.3 Android-specific preference fields

The existing `Preferences` struct gains Android-only fields, gated with `#[cfg(target_os = "android")]`:

```rust
#[cfg(target_os = "android")]
pub struct AndroidPreferences {
    /// Whether the user has completed the accessibility setup step
    pub accessibility_setup_done: bool,
    /// Whether the user has completed the battery setup wizard
    pub battery_setup_done: bool,
    /// Whether the foreground service should start automatically on app launch
    pub auto_start_service: bool,
}
```

These fields are serialised into `preferences.json` alongside the existing fields when running
on Android.

---

## 9. UI adaptations

### 9.1 Guiding principle

The Svelte UI (`apps/desktop/src/`) is used **unchanged** on Android with the exception of
the Android-specific components listed below. No existing component should be modified for
Android unless strictly necessary. New components are added using `#if` platform detection
where needed.

Platform detection in Svelte uses a Tauri command `get_platform()` that returns `"android"`,
`"windows"`, `"linux"`, or `"macos"`. This command is added to `commands.ts`.

### 9.2 New Android-only components

| Component | Description |
|-----------|-------------|
| `AccessibilitySetupPrompt.svelte` | One-time modal prompting the user to enable the accessibility service (§7.5) |
| `BatterySetupWizard.svelte` | OEM battery setup wizard (§6.4) |
| `AndroidOnboarding.svelte` | Wrapper that sequences AccessibilitySetup → BatterySetup on first launch |
| `HidTargetPairing.svelte` | Phase 2 only: UI for pairing a computer as HID target (§11) |

### 9.3 Navigation changes

The desktop app's title bar and tray-based navigation are replaced on Android by Tauri's
default mobile navigation. No custom navigation component changes are required — Tauri handles
the window chrome.

The desktop Settings page includes sections for tray behaviour, start at login, and other
desktop-only features. These sections are hidden on Android via platform detection. Android
adds its own Settings sections:

- **Accessibility** — shows current enabled/disabled status with a "Set up" button
- **Background operation** — shows battery exemption status with a "Set up" button; links
  to OEM wizard
- **Auto-start service** — toggle: start the foreground service automatically when the app
  opens (default: on)

### 9.4 Device list page

The device list page is unchanged. The "Connect" flow calls the Android `BlePlugin`
commands instead of the desktop Tauri commands, but because both share the same command
names (`start_scan`, `connect_device`, etc.), the Svelte code requires no changes.

---

## 10. Permissions manifest

All permissions are declared in `apps/desktop/src-tauri/gen/android/app/src/main/AndroidManifest.xml`.

### 10.1 Phase 1 permissions

```xml
<!-- BLE — legacy, required pre-API 31 -->
<uses-permission android:name="android.permission.BLUETOOTH"
    android:maxSdkVersion="30" />
<uses-permission android:name="android.permission.BLUETOOTH_ADMIN"
    android:maxSdkVersion="30" />

<!-- BLE — API 31+ -->
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
<uses-permission android:name="android.permission.BLUETOOTH_SCAN"
    android:usesPermissionFlags="neverForLocation" />

<!-- BLE scan on API 23–30 requires location -->
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION"
    android:maxSdkVersion="30" />

<!-- Foreground Service -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CONNECTED_DEVICE" />

<!-- Battery optimisation exemption -->
<uses-permission android:name="android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS" />

<!-- Accessibility Service (declared in service element, not uses-permission) -->
```

```xml
<!-- In <application> block -->
<service
    android:name=".MapxrForegroundService"
    android:foregroundServiceType="connectedDevice"
    android:exported="false" />

<service
    android:name=".MapxrAccessibilityService"
    android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE"
    android:exported="true">
    <intent-filter>
        <action android:name="android.accessibilityservice.AccessibilityService" />
    </intent-filter>
    <meta-data
        android:name="android.accessibilityservice"
        android:resource="@xml/accessibility_service_config" />
</service>
```

### 10.2 Runtime permission requests

Permissions that require a runtime prompt (API 31+) are requested on first use:

- `BLUETOOTH_SCAN` — requested when the user taps "Scan for devices"
- `BLUETOOTH_CONNECT` — requested before the first connection attempt
- `ACCESS_FINE_LOCATION` — requested on API ≤ 30 when scanning

The request flow uses Tauri's `tauri-plugin-android-permissions` (if available) or the
standard ActivityCompat request API from the Kotlin plugin. The result is propagated to the
Svelte UI as a `permissions-result` event.

---

## 11. Phase 2 — BluetoothHidDevice relay

> **Timing constraint:** Phase 2 must not be implemented until Phase 1 is released and
> published. The spec is included here for design completeness and to inform Phase 1
> architectural decisions that must accommodate Phase 2 without rework.

### 11.1 Concept

Android API 28+ implements the `BluetoothHidDevice` profile. The phone registers itself with
a paired computer as a Bluetooth HID keyboard (and optionally mouse). The target computer
sees a standard Bluetooth keyboard — no software installation required. Works with Windows,
macOS, Linux, iPadOS, and other Android devices.

This enables the "phone as adapter" use case: carry a phone and a Tap Strap; type on any
Bluetooth-capable device.

### 11.2 HID descriptor

The phone advertises a standard HID report descriptor covering:

- Boot-compatible keyboard (modifier byte + 6 simultaneous keys)
- Consumer control (media keys, volume)
- Mouse (buttons + X/Y axes + wheel)

The HID descriptor is a static byte array embedded in `HidPlugin.kt`. It follows the standard
USB HID Usage Tables format and is designed to be accepted by all major OS HID drivers without
custom drivers.

### 11.3 HidPlugin Tauri plugin

`HidPlugin` exposes:

| Command | Description |
|---------|-------------|
| `hid_start_host()` | Registers the phone as a HID device; starts advertising |
| `hid_stop_host()` | Unregisters; stops advertising |
| `hid_list_paired_targets()` | Returns list of previously paired computers |
| `hid_connect_target(address: string)` | Connects to a specific paired computer |
| `hid_disconnect_target()` | Disconnects current target |

Events emitted:

| Event | Payload |
|-------|---------|
| `hid-target-connected` | `{ address: string, name: string }` |
| `hid-target-disconnected` | `{ address: string, reason: string }` |
| `hid-pairing-requested` | `{ address: string, name: string }` — shown in UI for confirmation |

### 11.4 Key report encoding

When `mapping-core` resolves a `KeyPress(key, modifiers)` action, `HidPlugin` constructs a
standard HID keyboard report:

```
Byte 0: modifier bitmask (Ctrl=0x01, Shift=0x02, Alt=0x04, Meta=0x08, ...)
Byte 1: 0x00 (reserved)
Bytes 2–7: up to 6 simultaneous keycodes (USB HID Usage IDs, not Android keycodes)
```

For `KeyRelease`: send an all-zeros report.

For `TypeText { text }`: synthesise keydown + keyup pairs for each character.

For `MouseClick` / `MouseScroll`: construct HID mouse reports per the HID mouse descriptor.

Media keys use consumer control reports (separate HID report ID).

### 11.5 Mode selection

When Phase 2 is implemented, the user can choose the output mode in Settings:

| Mode | Description |
|------|-------------|
| **Android direct** (Phase 1) | AccessibilityService — tap into the phone itself |
| **Relay to computer** (Phase 2) | BluetoothHidDevice — tap into a paired computer |

Only one mode is active at a time. The active mode is stored in `preferences.json` as
`android_output_mode: "direct" | "relay"`.

### 11.6 HID target pairing UI

`HidTargetPairing.svelte` (§9.2) presents:

1. A list of previously paired computers with a "Connect" button per entry
2. A "Pair new computer" flow: the user enables Bluetooth pairing on their computer;
   the phone advertises itself as "MapXr Keyboard"; the computer initiates pairing normally
3. Status indicator showing current relay connection state

### 11.7 Phase 2 manifest additions

```xml
<!-- In AndroidManifest.xml -->
<uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" />

<!-- HidDevice profile does not require a separate service declaration;
     it is accessed via BluetoothAdapter.getProfileProxy() -->
```

---

## 12. Tauri command and event surface

### 12.1 Android-only Tauri commands

These commands are added to `apps/desktop/src-tauri/src/commands.rs` under
`#[cfg(target_os = "android")]` and registered in the Android-specific `invoke_handler`.

They are also added to `apps/desktop/src/lib/commands.ts` with platform guards so the Svelte
code can call them without crashing on desktop.

| Command | Signature | Description |
|---------|-----------|-------------|
| `get_platform` | `() -> String` | Returns `"android"`, `"windows"`, `"linux"`, `"macos"` |
| `check_accessibility_enabled` | `() -> bool` | Checks if `MapxrAccessibilityService` is running |
| `open_accessibility_settings` | `() -> ()` | Launches Android Accessibility Settings |
| `check_battery_exemption` | `() -> bool` | Returns true if battery optimisation is exempted |
| `request_battery_exemption` | `() -> ()` | Triggers `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` |
| `get_oem_info` | `() -> OemInfo` | Returns `{ manufacturer: string, needsOemSetup: bool }` |
| `open_oem_settings` | `() -> ()` | Deep-links to OEM-specific battery settings page |
| `start_foreground_service` | `() -> Result<(), String>` | Starts `MapxrForegroundService` |
| `stop_foreground_service` | `() -> Result<(), String>` | Stops `MapxrForegroundService` |

Phase 2 additions (not implemented in Phase 1):

| Command | Signature | Description |
|---------|-----------|-------------|
| `hid_start_host` | `() -> Result<(), String>` | Start HID device advertising |
| `hid_stop_host` | `() -> Result<(), String>` | Stop HID advertising |
| `hid_list_paired_targets` | `() -> Vec<HidTarget>` | List known paired computers |
| `hid_connect_target` | `(address: String) -> Result<(), String>` | Connect to a paired computer |
| `hid_disconnect_target` | `() -> Result<(), String>` | Disconnect current HID target |

### 12.2 Shared commands (same name, Android implementation)

These commands exist on desktop and are re-implemented for Android by the Kotlin plugins.
The Svelte frontend calls them without any platform guard because the names are identical:

| Command | Desktop implementation | Android implementation |
|---------|----------------------|----------------------|
| `start_scan` | `tap-ble` scanner | `BlePlugin.startScan()` |
| `stop_scan` | `tap-ble` scanner | `BlePlugin.stopScan()` |
| `connect_device` | `tap-ble` connection | `BlePlugin.connect()` |
| `disconnect_device` | `tap-ble` disconnection | `BlePlugin.disconnect()` |
| `list_connected_devices` | `AppState` | `BlePlugin` device registry |
| `get_active_profile` | `AppState` | same Rust state |
| `set_active_profile` | `AppState` | same Rust state |
| `list_profiles` | filesystem | same filesystem code |
| `save_profile` | filesystem | same filesystem code |
| `delete_profile` | filesystem | same filesystem code |

### 12.3 Android-specific events

| Event | Payload | Description |
|-------|---------|-------------|
| `android-service-started` | `{}` | Foreground service has started |
| `android-service-stopped` | `{}` | Foreground service has stopped |
| `android-accessibility-changed` | `{ enabled: bool }` | Accessibility service enabled/disabled |
| `android-battery-exemption-changed` | `{ granted: bool }` | Battery exemption grant status changed |

---

## 13. Testing strategy

### 13.1 Unit tests

| Area | Test requirement |
|------|-----------------|
| Key mapping table (§7.4) | Unit test every `Key` variant → `KeyEvent.KEYCODE_*` mapping; verify unsupported keys produce a no-op and a log warning |
| `VibrationPattern` → BLE write encoding | Existing tests in `mapping-core` cover this; verify JNI bridge doesn't corrupt the bytes |
| `preferences.json` Android fields | Serde round-trip test for `AndroidPreferences` |
| HID report encoding (Phase 2) | Unit test key → report byte array for all key categories; test modifier combinations |

### 13.2 Integration tests

Integration tests that require hardware (Android device + Tap Strap) are documented as
manual test plans in `docs/testing/android-manual-tests.md`. They are not automated.

Manual test plan covers:

1. App installs and launches on target device (Pixel, Samsung, Xiaomi)
2. BLE scan discovers Tap Strap
3. BLE connection establishes and survives backgrounding the app
4. Tap events dispatch correctly into a text editor app
5. Foreground service notification appears and "Stop" action works
6. OEM battery wizard shows correct manufacturer-specific instructions
7. Accessibility permission prompt appears and deep-link works
8. Profile changes (switch active profile, layer switch) take effect immediately
9. App survives being killed and relaunched (state restores correctly)
10. (Phase 2) HID pairing pairs with a Windows 11 computer; keystrokes arrive correctly
11. (Phase 2) HID pairing pairs with a macOS 14 computer; keystrokes arrive correctly

### 13.3 Minimum test device matrix

Phase 1 must be manually tested on at least:

| Device | Android version | Why |
|--------|----------------|-----|
| Google Pixel (any model) | Android 14 (API 34) | Stock Android reference |
| Samsung Galaxy (mid-range) | One UI 6 (Android 14) | Most common OEM |
| Xiaomi (any MIUI 14 device) | Android 13+ | Most aggressive OEM background killing |

---

## 14. Distribution

### 14.1 APK signing

The release APK is signed with a keystore. The keystore password and key alias are stored as
GitHub repository secrets:

| Secret | Value |
|--------|-------|
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded `.jks` keystore file |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias within the keystore |
| `ANDROID_KEY_PASSWORD` | Key password |

The `release.yml` workflow gains an `android` job that:
1. Sets up JDK 17 and Android SDK
2. Decodes the keystore from the secret
3. Runs `cargo tauri android build --release`
4. Signs the APK with `apksigner`
5. Uploads the signed APK as a GitHub Release asset

### 14.2 Distribution channels

**Primary: GitHub Releases** — signed APK attached to the same release tag as the Linux/Windows
builds. Users download and sideload.

**Secondary: F-Droid** — F-Droid is an open-source Android app repository. Submission requires:
- A clean open-source license (already satisfied)
- Reproducible builds (achievable with Tauri + NDK)
- No proprietary dependencies in the build (no Play Services, no Firebase — satisfied)

F-Droid submission is a manual process done after Phase 1 is stable. It is not part of the
automated release workflow.

**Not planned: Google Play Store** — Google Play policy may flag an app that uses
AccessibilityService + Bluetooth keyboard simulation as potentially violating developer
policy. F-Droid and GitHub direct distribution avoid this risk entirely.

### 14.3 Version alignment

The Android app version tracks the same semver version as the desktop app. Both are driven
by the same `tauri.conf.json` version field. The `versionCode` for Android is derived from
the semver (e.g. `1.2.3` → `10203`).

---

## 15. Known limitations and non-goals

| Item | Notes |
|------|-------|
| iOS | Out of scope. Specified separately as stretch goal S.2. |
| macOS on Apple Silicon | Not affected by this spec. |
| Context-aware profile switching | `context-rules.json` is not implemented on Android. The UI page is hidden on Android via platform detection. May be revisited if Android exposes a stable API for foreground app detection. |
| Lock screen key injection | Impossible without root. No workaround. |
| Bypassing FLAG_SECURE | Impossible. Banking apps and secure keyboards will not receive injected keystrokes. Document this clearly in the app. |
| Mouse movement (not just click/scroll) | Excluded on all platforms, including Android. Consistent with the desktop spec. |
| Tap firmware upgrades | Not implemented on any platform currently. Android is no different. |
| Multi-device (two Tap Straps on one phone) | The architecture supports it (same as desktop) but is not tested in Phase 1. Treat as best-effort. |
