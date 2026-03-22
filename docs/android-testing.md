# Android local development guide

How to build, run, and debug the MapXr Android app on a physical device.

---

## Prerequisites

### Android Studio and SDK

Download and install [Android Studio](https://developer.android.com/studio), then open
**SDK Manager** and install:

- Android SDK Platform-Tools
- Android SDK Build-Tools (latest)
- NDK (Side by side) — latest version
- Android SDK Command-line Tools

### Commandline Tools Only
sudo dnf install java-17-openjdk-devel
export JAVA_HOME="/usr/lib/jvm/java-17-openjdk"

Download the command-line tools (no Android Studio) from:
https://developer.android.com/studio#command-line-tools-only

Then install the required SDK components:
mkdir -p ~/Android/Sdk/cmdline-tools
unzip commandlinetools-linux-*.zip -d ~/Android/Sdk/cmdline-tools
mv ~/Android/Sdk/cmdline-tools/cmdline-tools ~/Android/Sdk/cmdline-tools/latest

export ANDROID_HOME="$HOME/Android/Sdk"
export
PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

sdkmanager "platform-tools"
sdkmanager "build-tools;34.0.0"
sdkmanager "platforms;android-34"
sdkmanager "ndk;27.0.12077973"

export NDK_HOME="$ANDROID_HOME/ndk/27.0.12077973"

### Environment variables

Add to `~/.bashrc` (or `~/.zshrc`):

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk | tail -1)"
export JAVA_HOME="/opt/android-studio/jbr"   # adjust if installed elsewhere
```

Reload: `source ~/.bashrc`

### Rust Android targets (one-time)

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

---

## Connect your device

On the Android device:

1. **Settings → About Phone** — tap **Build Number** 7 times to enable Developer Options
2. **Settings → System → Developer Options** — enable **USB Debugging**
3. Plug in via USB and tap **Allow** on the device prompt

Verify the connection:

```bash
adb devices   # should list your device with status "device"
```

If the device shows `offline`:

```bash
adb kill-server && adb start-server
# unplug and replug the USB cable
adb devices
```

---

## Build and run (debug)

```bash
cd apps/desktop
cargo tauri android dev
```

The first build takes 5–10 minutes (Rust cross-compilation + Gradle). Subsequent builds are
much faster. The CLI stays running — Svelte/TypeScript changes hot-reload automatically.
Rust or Kotlin changes require a full rebuild (the CLI picks them up automatically when you
save).

---

## View logs

Open a second terminal while the app is running.

```bash
# All app-related output
adb logcat | grep -iE "mapxr"

# Kotlin plugin logs by tag
adb logcat | grep -E "(MapxrBle|MapxrBattery|MapxrAccessibility|MapxrForeground)"

# Rust log output
adb logcat | grep RustBackend

# Clear stale logs before a new session
adb logcat -c && adb logcat | grep -iE "mapxr"
```

---

## Frontend debugging (Chrome DevTools)

With the app running on the device:

1. Open Chrome on your desktop and navigate to `chrome://inspect`
2. Tick **Discover USB devices**
3. Your device's WebView will appear in the list
4. Click **Inspect** — full DevTools opens (console, network, element inspector)

---

## Release build

```bash
cd apps/desktop
cargo tauri android build --apk
```

The unsigned APK is written to:

```
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

Install directly to the connected device:

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

The unsigned APK is fine for local testing. Signed APKs for distribution are produced by the
`build-android` job in `.github/workflows/release.yml` using the keystore secrets documented
in `CHANGELOG.md`.

---

## Quick-start checklist

```bash
source ~/.bashrc                   # load ANDROID_HOME / NDK_HOME / JAVA_HOME
adb devices                        # confirm device is listed as "device"
cd apps/desktop
cargo tauri android dev            # build, deploy, and keep running

# in a second terminal:
adb logcat -c && adb logcat | grep -iE "mapxr"
```
