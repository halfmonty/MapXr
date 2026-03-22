# Android Manual Test Plan

Manual integration tests for the MapXr Android port (Epic 15).
These tests require a physical Android device + Tap Strap; they are not automated.

---

## Device matrix

| Device | Android / UI version | Why |
|--------|---------------------|-----|
| Google Pixel (any model) | Android 14, API 34 | Stock Android reference — no OEM customisation |
| Samsung Galaxy (mid-range) | One UI 6, Android 14 | Most common OEM; aggressive battery management |
| Xiaomi (MIUI 14 device) | Android 13+, MIUI 14 | Most aggressive OEM background killing |

---

## Pre-requisites

- Tap Strap charged and in BLE pairing mode
- MapXr APK built and sideloaded (`cargo tauri android build`)
- A plain text editor app installed (e.g. stock Notes, Keep, or Markor)

---

## Test cases

### T1 — App install and launch

**Steps:**
1. Sideload the MapXr APK
2. Tap the icon to launch

**Expected:** App opens to the Devices page. No crash. Status bar shows no errors.

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T2 — Onboarding wizard (first launch)

**Steps:**
1. First launch after clean install
2. Observe onboarding prompt

**Expected:**
- `AndroidOnboarding` shows `AccessibilitySetupPrompt` if accessibility not yet enabled
- After completing or skipping accessibility, `BatterySetupWizard` appears
- Completing both marks `accessibility_setup_done` and `battery_setup_done` in preferences

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T3 — Accessibility service setup

**Steps:**
1. Tap "Open Accessibility Settings" in the prompt
2. Find "MapXr" in the list
3. Enable it
4. Return to MapXr

**Expected:**
- Settings screen opens to the Accessibility page
- After returning, prompt shows "✓ Accessibility permission is enabled"

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T4 — OEM battery wizard

**Steps:**
1. Open the battery setup wizard (from onboarding or Settings → Background operation → Set up)
2. Tap "Disable optimisation"
3. In the system dialog, allow MapXr to ignore battery optimisations
4. Return to MapXr
5. On Xiaomi/Samsung/Huawei: follow the OEM-specific step

**Expected:**
- Step 1 shows device-specific instructions when `hasOemStep = true`
- Step 2 shows the system battery exemption dialog
- After returning, status shows "✓ Battery optimisation is disabled for MapXr"
- On Xiaomi: OEM step shows MIUI Autostart instructions
- On Samsung: OEM step shows "Never sleeping" instructions

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T5 — BLE scan

**Steps:**
1. Navigate to Devices page
2. Tap "Scan for devices"
3. Put Tap Strap in pairing mode

**Expected:**
- Runtime BLE permission prompt appears (first time on API 31+)
- Tap Strap appears in the scan results within 30 seconds
- Scan stops automatically after 30 seconds

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T6 — BLE connection

**Steps:**
1. Select Tap Strap from scan results
2. Tap "Connect"

**Expected:**
- Connection established (device appears as connected)
- Foreground service notification appears: "MapXr active · 1 device(s) connected"
- Tap events are received (visible in Debug panel)

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T7 — Tap event dispatch into text editor

**Steps:**
1. Activate a profile that maps taps to letter keys
2. Open a text editor app
3. Focus on a text input field
4. Perform several Tap Strap gestures

**Expected:**
- Keystrokes appear in the text field matching the active profile mapping
- No missed or doubled characters

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T8 — Background survival

**Steps:**
1. Connect Tap Strap
2. Background the MapXr app (press Home)
3. Wait 5 minutes
4. Foreground MapXr

**Expected:**
- Tap Strap remains connected throughout
- Foreground service notification persists
- Tap events resume immediately after foregrounding

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung (with "Never sleeping" set) | | |
| Xiaomi (with MIUI Autostart on) | | |

---

### T9 — Foreground service "Stop" action

**Steps:**
1. With Tap Strap connected, expand the notification
2. Tap the "Stop" action in the notification

**Expected:**
- Foreground service stops
- Tap Strap disconnects
- Notification disappears

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T10 — Reconnection after background kill

**Steps:**
1. Connect Tap Strap
2. Force-stop MapXr from Settings → Apps
3. Relaunch MapXr

**Expected:**
- App starts cleanly
- Previously connected device is shown in Devices (persistent registry)
- Manual reconnect works

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T11 — Profile switch

**Steps:**
1. Load two profiles
2. With Tap Strap connected, activate profile A
3. Switch to profile B via the UI

**Expected:**
- Active layer changes in sidebar
- Tap events resolve using profile B's mappings immediately

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

### T12 — Layer push / pop

**Steps:**
1. Activate a profile that includes a PushLayer action
2. Trigger the PushLayer gesture
3. Trigger the PopLayer gesture

**Expected:**
- Layer stack shows correct pushed and popped states in the sidebar
- Tap events resolve from the pushed layer while active

| Device | Pass / Fail | Notes |
|--------|-------------|-------|
| Pixel | | |
| Samsung | | |
| Xiaomi | | |

---

## Results summary

| Test | Pixel | Samsung | Xiaomi |
|------|-------|---------|--------|
| T1 Install and launch | | | |
| T2 Onboarding wizard | | | |
| T3 Accessibility setup | | | |
| T4 Battery wizard | | | |
| T5 BLE scan | | | |
| T6 BLE connection | | | |
| T7 Tap dispatch | | | |
| T8 Background survival | | | |
| T9 Service stop action | | | |
| T10 Reconnection | | | |
| T11 Profile switch | | | |
| T12 Layer push/pop | | | |

---

## Known limitations

- **FLAG_SECURE apps:** Key injection is blocked by apps that set `FLAG_SECURE` (banking,
  password managers). This is by design and cannot be worked around.
- **API 26–27:** `dispatchKeyEvent` requires API 28. On API 26–27 devices, key events will
  not be dispatched. A warning is logged.
- **Macro actions:** Macro dispatch (nested actions) is not yet implemented; logged as a
  warning and skipped.
- **Context rules:** The context-switching (auto-switch profile on app focus) feature is
  desktop-only and is not available on Android.
