<h1>Porting MapXr to Android: three dead ends and one great open-source library</h1>

<p>
  The Android port has been the most technically demanding part of this project, and not just
  because cross-platform mobile development is inherently fiddly. The real challenge was finding a
  way to inject keystrokes into arbitrary apps without requiring root access. Android does not make
  this easy by design. The OS has progressively locked down the input injection APIs over
  successive releases. Getting here required three distinct implementation attempts before landing on
  something that actually works.
</p>

<h2>Attempt 1: AccessibilityService</h2>

<p>
  The first approach was <code>AccessibilityService</code>. It's the API Android officially exposes
  for keyboard-adjacent automation: screen readers, switch access, assistive tools. It can dispatch
  key events via <code>performGlobalAction</code> and send typed text via
  <code>performAction(ACTION_SET_TEXT)</code>. It's well-documented, requires no root, and every
  Android device supports it. The obvious choice.
</p>

<p>
  The problem is what it can't do. <code>ACTION_SET_TEXT</code> replaces the content of the focused
  text field. It doesn't simulate a keypress arriving at the app. There is no modifier key support.
  Hotkeys like Ctrl+Z or Ctrl+Tab are impossible because there's no mechanism to hold a modifier
  while firing another key. Navigation keys like arrow keys and Page Down work in text fields but
  not in games, editors, or anything that handles raw key events. The deeper problem is that
  <code>AccessibilityService</code> is built for screen reader use cases, not general-purpose
  keyboard emulation. MapXr needs to send arbitrary key chords to any app, and AccessibilityService
  can't do that.
</p>

<h2>Attempt 2: Custom input method (IME)</h2>

<p>
  The second attempt was building a custom <code>InputMethodService</code>. Essentially a software
  keyboard that MapXr controls behind the scenes. An IME has access to
  <code>InputConnection</code>, which lets it inject text and some key events into the currently
  focused text field with decent fidelity.
</p>

<p>
  This hits the same ceiling from a different direction. <code>InputConnection</code> only reaches
  the currently focused editable view. A browser, a game, a terminal app. None of these route key
  events through the IME at all. The IME approach works well if your goal is a smarter text input
  experience, but MapXr needs to be a keyboard replacement for <em>everything</em>. An IME that
  can't send Escape to a terminal or arrow keys to a game is not a keyboard replacement.
</p>

<h2>Attempt 3: ADB wireless debugging shell</h2>

<p>
  The third attempt was more ambitious. Android's developer options expose a wireless debugging
  mode, and <code>adb shell input keyevent</code> can inject arbitrary key events as the shell user,
  bypassing all the usual input API restrictions. If MapXr could establish its own ADB connection
  over the local Wi-Fi interface, it could fire shell commands directly without any external tool.
</p>

<p>
  This turned into a substantial engineering effort. The ADB wireless debugging pairing protocol
  involves SPAKE2 password-authenticated key exchange, followed by TLS negotiation using the
  exchanged keys. The ADB transport protocol for sending shell commands sits on top of that. None of
  this is formally documented. The spec is the AOSP source code.
</p>

<p>
  Pairing worked. The SPAKE2 exchange completed, the device showed "mapxr@mapxr" in the Paired
  Devices list. The TLS connection attempt then failed with <code>CERTIFICATE_VERIFY_FAILED</code>.
  Logcat on the device showed <em>"Invalid base64 key"</em> for every entry in the ADB keystore
  loaded from our pairing. The key was structurally correct. The base64 decoded to 524 bytes,
  the RSA header fields matched the format exactly, and the same key format is accepted without
  complaint on desktop Linux. Something in the Samsung Android 16 (API 36) <code>adbd</code>
  implementation was rejecting it for a reason that can't be diagnosed without root access to the
  device. After exhausting every diagnostic angle available from the unprivileged side, the approach
  was abandoned.
</p>

<h2>Shizuku</h2>

<p>
  <a href="https://shizuku.rikka.app/" target="_blank" rel="noopener noreferrer">Shizuku</a> is an
  open-source project by <a href="https://rikka.app/" target="_blank" rel="noopener noreferrer">Rikka</a>
  that solves exactly this class of problem in a way the previous approaches couldn't.
  It uses Android's wireless debugging infrastructure (the same pairing mechanism that already
  works) to launch a persistent background service as the shell user. Apps that have been granted
  permission via Shizuku can then bind to that service over Binder IPC and make calls as if they
  were running as shell uid 2000, without requiring root and without needing to implement any ADB
  protocol themselves.
</p>

<p>
  The key Android API this unlocks is <code>InputManager.injectInputEvent()</code>. This is the same
  method that <code>adb shell input</code> uses internally. With shell uid access, MapXr can inject
  any <code>KeyEvent</code> or <code>MotionEvent</code> into the global input pipeline. Modifier
  keys, function keys, key chords, everything.
</p>

<p>
  I want to specifically thank the Shizuku team for what they've built. This is a genuinely
  difficult problem to solve. Navigating Android's increasingly restrictive permission model while
  keeping things user-friendly and not requiring root is no small feat. The library is well-designed,
  the documentation is clear, and the project has been actively maintained across multiple Android
  major versions. MapXr would not have an Android port without their work.
</p>

<h2>How the Shizuku integration works</h2>

<p>
  The integration has three layers: a Shizuku UserService running as shell uid, a dispatcher
  singleton on the app side, and a JNI bridge to the Rust engine.
</p>

<h3>InputUserService</h3>

<p>
  <code>InputUserService</code> is a bound service that Shizuku launches as the shell user.
  It implements an AIDL interface (<code>IInputService</code>) with two methods:
  <code>injectKey(KeyEvent)</code> and <code>injectMotion(MotionEvent)</code>. The implementation
  uses reflection to call <code>InputManagerGlobal.injectInputEvent()</code> directly, with a
  fallback to the public <code>InputManager</code> API. Running as shell uid 2000, these calls
  succeed where they would be silently dropped from a normal app process.
</p>

<h3>ShizukuDispatcher</h3>

<p>
  <code>ShizukuDispatcher</code> is a Kotlin singleton that owns the Shizuku connection lifecycle
  and translates mapping-core action JSON into <code>KeyEvent</code> and <code>MotionEvent</code>
  calls. It exposes a <code>StateFlow&lt;ShizukuState&gt;</code> that tracks whether Shizuku is
  installed, running, permission-granted, binding, active, or reconnecting. The UI wizard drives
  from that state. It polls every second and auto-advances through the setup steps as each
  condition is met.
</p>

<p>
  The dispatcher handles the full action vocabulary: <code>key</code> (with complete modifier key
  sequencing: down all modifiers, down+up the primary key, up modifiers in reverse), <code>key_chord</code>,
  <code>type_string</code> (via <code>ACTION_MULTIPLE</code> with a per-character fallback for
  OEMs that drop it), <code>mouse_click</code>, <code>mouse_double_click</code>,
  <code>mouse_scroll</code>, and <code>macro</code> with per-step delays.
</p>

<h3>JNI bridge and background dispatch</h3>

<p>
  The reason background key injection works (even when the MapXr WebView is suspended) is that
  the dispatch path never goes through JavaScript at all. When a BLE characteristic notification
  arrives from the Tap Strap, <code>BlePlugin.onTapBytes()</code> calls
  <code>NativeBridge.processTapBytes()</code>, which crosses the JNI boundary into Rust. The Rust
  pump resolves the tap bytes into actions, then calls back into Kotlin via JNI to invoke
  <code>ShizukuDispatcher.dispatch(actionsJson)</code> on the <code>InputUserService</code> binder.
  The WebView is not involved at any point in this path, so the pipeline keeps working when the app
  is in the background.
</p>

<h3>Setup wizard</h3>

<p>
  The <code>ShizukuSetup</code> wizard in Settings walks through three steps: install Shizuku,
  start it via Wireless Debugging, and grant MapXr permission. Each step polls the dispatcher state
  every second and advances automatically. The user doesn't need to tap "Next". Once the
  <code>InputUserService</code> binds successfully, the wizard shows a confirmation screen and
  that's the full setup. After the first start, Shizuku auto-starts on every reboot via the
  wireless debugging daemon, so the one-time setup is genuinely one-time.
</p>

<h2>What's next</h2>

<p>
  The Android implementation is complete. The APK build is working and will be included in the next
  release alongside the existing Linux and Windows installers. If you've been waiting to try MapXr
  on Android, it's almost there.
</p>
