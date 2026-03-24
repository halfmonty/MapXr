package com.mapxr.app

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.Build
import android.os.IBinder
import android.os.SystemClock
import android.util.Log
import android.view.InputDevice
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.WindowManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku

private const val TAG = "ShizukuDispatcher"
private const val SHIZUKU_REQUEST_CODE = 1001
private const val MAX_RECONNECT_ATTEMPTS = 3
private const val RECONNECT_DELAY_MS = 1000L
private const val SHIZUKU_PACKAGE = "moe.shizuku.privileged.api"

/** Represents the current state of the Shizuku integration lifecycle. */
sealed class ShizukuState {
    /** Android < 11 (API 30) — Shizuku requires API 11+, but our pump requires API 30. */
    object Unsupported : ShizukuState()
    /** Shizuku app is not installed on this device. */
    object NotInstalled : ShizukuState()
    /** Shizuku is installed but not running (user must start it via Wireless Debugging). */
    object NotRunning : ShizukuState()
    /** Shizuku is running but mapxr has not been granted permission yet. */
    object PermissionRequired : ShizukuState()
    /** Permission granted; binding [InputUserService] via Shizuku. */
    object Binding : ShizukuState()
    /** [InputUserService] is bound and ready to inject input events. */
    object Active : ShizukuState()
    /** [InputUserService] disconnected unexpectedly; auto-rebind in progress. */
    object Reconnecting : ShizukuState()
}

/**
 * Singleton that owns the Shizuku [InputUserService] connection and converts
 * mapping-core action JSON into [IInputService.injectKey] / [IInputService.injectMotion]
 * calls running as shell uid.
 *
 * ## Lifecycle
 *
 * Call [init] once from [MainActivity.onCreate] after `super.onCreate()`. The dispatcher
 * then manages the Shizuku binder and UserService connection autonomously, updating
 * [state] as conditions change.
 *
 * ## Dispatch
 *
 * [dispatch] is called from JNI by the Rust pump. If [state] is not [ShizukuState.Active],
 * actions are dropped with a warning (graceful degradation per spec §11).
 */
object ShizukuDispatcher {

    private val _state = MutableStateFlow<ShizukuState>(ShizukuState.NotInstalled)

    /** Observable Shizuku integration state; drive the setup wizard UI from this. */
    val state: StateFlow<ShizukuState> = _state.asStateFlow()

    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    private var reconnectJob: Job? = null
    private var reconnectAttempts = 0

    private var inputService: IInputService? = null
    private lateinit var appContext: Context
    private lateinit var userServiceArgs: Shizuku.UserServiceArgs

    // ── Shizuku event listeners ───────────────────────────────────────────────

    private val binderReceivedListener = Shizuku.OnBinderReceivedListener {
        Log.i(TAG, "Shizuku binder received")
        updateState()
    }

    private val binderDeadListener = Shizuku.OnBinderDeadListener {
        Log.w(TAG, "Shizuku binder died")
        inputService = null
        _state.value = ShizukuState.NotRunning
        startStatePoller()
    }

    private val permissionResultListener =
        Shizuku.OnRequestPermissionResultListener { requestCode, grantResult ->
            if (requestCode == SHIZUKU_REQUEST_CODE) {
                if (grantResult == PackageManager.PERMISSION_GRANTED) {
                    Log.i(TAG, "Shizuku permission granted")
                    bind()
                } else {
                    Log.w(TAG, "Shizuku permission denied")
                    _state.value = ShizukuState.PermissionRequired
                }
            }
        }

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            Log.i(TAG, "InputUserService connected")
            inputService = IInputService.Stub.asInterface(binder)
            _state.value = ShizukuState.Active
            reconnectAttempts = 0
            reconnectJob?.cancel()
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            Log.w(TAG, "InputUserService disconnected unexpectedly")
            inputService = null
            _state.value = ShizukuState.Reconnecting
            scheduleReconnect()
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Initialise the dispatcher. Must be called once from [MainActivity.onCreate] **after**
     * `super.onCreate()`. Subsequent calls are silently ignored.
     */
    fun init(ctx: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            _state.value = ShizukuState.Unsupported
            return
        }
        if (::appContext.isInitialized) return // idempotent

        appContext = ctx.applicationContext
        userServiceArgs = Shizuku.UserServiceArgs(
            ComponentName(ctx.packageName, InputUserService::class.java.name),
        )
            .daemon(false)
            .processNameSuffix("input_service")
            .debuggable(BuildConfig.DEBUG)
            .version(BuildConfig.VERSION_CODE)

        // addBinderReceivedListenerSticky fires immediately on the calling thread if
        // Shizuku's binder is already alive, so updateState() may run synchronously.
        Shizuku.addBinderReceivedListenerSticky(binderReceivedListener)
        Shizuku.addBinderDeadListener(binderDeadListener)
        Shizuku.addRequestPermissionResultListener(permissionResultListener)

        // Active-poll pingBinder() so we recover from any timing gap between app start
        // and Shizuku's binder becoming available (the sticky listener may have fired
        // before the ShizukuProvider was fully ready on some devices).
        startStatePoller()
    }

    /** Trigger the Shizuku in-app permission request dialog. */
    fun requestPermission() {
        Shizuku.requestPermission(SHIZUKU_REQUEST_CODE)
    }

    /** Bind to [InputUserService] via Shizuku. Updates [state] to [ShizukuState.Binding]. */
    fun bind() {
        _state.value = ShizukuState.Binding
        Shizuku.bindUserService(userServiceArgs, serviceConnection)
    }

    /** Unbind from [InputUserService] and release the Binder. */
    fun unbind() {
        Shizuku.unbindUserService(userServiceArgs, serviceConnection, true)
        inputService = null
    }

    /**
     * Dispatch a JSON array of resolved mapping-core actions via [InputUserService].
     *
     * Called from JNI by the Rust pump on the pump thread. If [inputService] is null
     * (any non-[ShizukuState.Active] state), all actions are dropped with a warning.
     * Actions are processed synchronously; macro delays use [Thread.sleep] on the
     * calling (pump) thread.
     *
     * @param actionsJson JSON array string, e.g.
     *   `[{"type":"key","key":"a","modifiers":["ctrl"]}]`
     */
    @JvmStatic
    fun dispatch(actionsJson: String) {
        val service = inputService
        if (service == null) {
            Log.w(TAG, "Shizuku not active — dropping actions (state=${_state.value})")
            return
        }
        try {
            val array = JSONArray(actionsJson)
            for (i in 0 until array.length()) {
                dispatchAction(service, array.getJSONObject(i))
            }
        } catch (e: Exception) {
            Log.e(TAG, "dispatch error: $e")
        }
    }

    // ── State management ──────────────────────────────────────────────────────

    /**
     * Poll [Shizuku.pingBinder] every 2 s while state is [ShizukuState.NotRunning] or
     * [ShizukuState.NotInstalled]. Stops as soon as state advances (permission required,
     * binding, or active). Called from [init] and [binderDeadListener].
     */
    private fun startStatePoller() {
        scope.launch {
            Log.d(TAG, "startStatePoller: starting")
            while (true) {
                delay(2000L)
                when (_state.value) {
                    is ShizukuState.NotRunning, is ShizukuState.NotInstalled -> updateState()
                    else -> {
                        Log.d(TAG, "startStatePoller: stopping (state=${_state.value})")
                        break
                    }
                }
            }
        }
    }

    private fun updateState() {
        val ping = Shizuku.pingBinder()
        Log.d(TAG, "updateState: pingBinder=$ping state=${_state.value}")
        if (!ping) {
            val installed = isShizukuInstalled()
            Log.d(TAG, "updateState: Shizuku not running; installed=$installed")
            _state.value = if (installed) ShizukuState.NotRunning else ShizukuState.NotInstalled
            return
        }
        val perm = Shizuku.checkSelfPermission()
        Log.d(TAG, "updateState: permission=$perm (GRANTED=${PackageManager.PERMISSION_GRANTED})")
        if (perm != PackageManager.PERMISSION_GRANTED) {
            _state.value = ShizukuState.PermissionRequired
            return
        }
        val current = _state.value
        if (current !is ShizukuState.Active && current !is ShizukuState.Binding) {
            bind()
        }
    }

    private fun isShizukuInstalled(): Boolean {
        return try {
            appContext.packageManager.getPackageInfo(SHIZUKU_PACKAGE, 0)
            true
        } catch (_: PackageManager.NameNotFoundException) {
            Log.d(TAG, "isShizukuInstalled: NameNotFoundException — package not visible (missing <queries>?)")
            false
        }
    }

    private fun scheduleReconnect() {
        reconnectJob?.cancel()
        reconnectJob = scope.launch {
            if (reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
                Log.w(TAG, "max reconnect attempts reached — user must restart Shizuku")
                _state.value = ShizukuState.NotRunning
                reconnectAttempts = 0
                return@launch
            }
            reconnectAttempts++
            Log.i(TAG, "reconnect attempt $reconnectAttempts/$MAX_RECONNECT_ATTEMPTS")
            delay(RECONNECT_DELAY_MS)
            if (Shizuku.pingBinder() &&
                Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
            ) {
                bind()
            } else {
                _state.value = ShizukuState.NotRunning
            }
        }
    }

    // ── Action dispatch ───────────────────────────────────────────────────────

    private fun dispatchAction(service: IInputService, action: JSONObject) {
        when (val type = action.getString("type")) {
            "key" -> {
                val keyName = action.getString("key")
                val keyCode = keyNameToCode(keyName) ?: run {
                    Log.w(TAG, "unknown key name: '$keyName'")
                    return
                }
                val modifiers = action.optJSONArray("modifiers")
                val modCodes = jsonArrayToModifierCodes(modifiers)
                val metaState = jsonArrayToMetaState(modifiers)
                injectKeyWithModifiers(service, keyCode, metaState, modCodes)
            }

            "key_chord" -> {
                val keys = action.getJSONArray("keys")
                val keyList = (0 until keys.length()).map { keys.getString(it) }
                val modNames = keyList.filter { isModifierName(it) }
                val primaryNames = keyList.filter { !isModifierName(it) }
                val metaState = modNames.fold(0) { acc, m -> acc or modifierNameToMetaState(m) }
                val modCodes = modNames.mapNotNull { modifierNameToKeyCode(it) }
                // Modifier DOWN (build metaState incrementally)
                var activeMeta = 0
                for (code in modCodes) {
                    activeMeta = activeMeta or modifierCodeToMetaMask(code)
                    service.injectKey(buildKeyEvent(KeyEvent.ACTION_DOWN, code, activeMeta))
                }
                // Primary keys DOWN + UP
                for (name in primaryNames) {
                    val code = keyNameToCode(name)
                    if (code == null) {
                        Log.w(TAG, "unknown key in chord: '$name'")
                        continue
                    }
                    service.injectKey(buildKeyEvent(KeyEvent.ACTION_DOWN, code, metaState))
                    service.injectKey(buildKeyEvent(KeyEvent.ACTION_UP, code, metaState))
                }
                // Modifier UP in reverse
                activeMeta = metaState
                for (code in modCodes.reversed()) {
                    service.injectKey(buildKeyEvent(KeyEvent.ACTION_UP, code, activeMeta))
                    activeMeta = activeMeta and modifierCodeToMetaMask(code).inv()
                }
            }

            "type_string" -> {
                injectTypeString(service, action.getString("text"))
            }

            "mouse_click" -> {
                injectMouseClick(service, doubleClick = false)
            }

            "mouse_double_click" -> {
                injectMouseClick(service, doubleClick = true)
            }

            "mouse_scroll" -> {
                injectMouseScroll(service, action.getString("direction"))
            }

            "macro" -> {
                val steps = action.getJSONArray("steps")
                for (i in 0 until steps.length()) {
                    val step = steps.getJSONObject(i)
                    dispatchAction(service, step.getJSONObject("action"))
                    val delayMs = step.optLong("delay_ms", 0L)
                    if (delayMs > 0) {
                        // Blocks the pump thread for the duration of the macro step delay.
                        Thread.sleep(delayMs)
                    }
                }
            }

            "vibrate" -> {
                // Vibration is forwarded to BlePlugin via the tap-vibrate WebView event;
                // ShizukuDispatcher does not need to handle it.
            }

            else -> Log.w(TAG, "unhandled action type: '$type'")
        }
    }

    // ── Input event helpers ───────────────────────────────────────────────────

    /**
     * Send modifier DOWN events, then the primary key DOWN+UP, then modifier UP in reverse.
     * This is the correct sequence for OS-level key injection with held modifiers.
     */
    private fun injectKeyWithModifiers(
        service: IInputService,
        keyCode: Int,
        metaState: Int,
        modCodes: List<Int>,
    ) {
        var activeMeta = 0
        for (code in modCodes) {
            activeMeta = activeMeta or modifierCodeToMetaMask(code)
            service.injectKey(buildKeyEvent(KeyEvent.ACTION_DOWN, code, activeMeta))
        }
        service.injectKey(buildKeyEvent(KeyEvent.ACTION_DOWN, keyCode, metaState))
        service.injectKey(buildKeyEvent(KeyEvent.ACTION_UP, keyCode, metaState))
        activeMeta = metaState
        for (code in modCodes.reversed()) {
            service.injectKey(buildKeyEvent(KeyEvent.ACTION_UP, code, activeMeta))
            activeMeta = activeMeta and modifierCodeToMetaMask(code).inv()
        }
    }

    private fun buildKeyEvent(action: Int, keyCode: Int, metaState: Int): KeyEvent {
        val now = SystemClock.uptimeMillis()
        return KeyEvent(
            now, now, action, keyCode, 0, metaState,
            KeyCharacterMap.VIRTUAL_KEYBOARD, 0,
            KeyEvent.FLAG_FROM_SYSTEM or KeyEvent.FLAG_VIRTUAL_HARD_KEY,
            InputDevice.SOURCE_KEYBOARD,
        )
    }

    private fun injectTypeString(service: IInputService, text: String) {
        if (text.isEmpty()) return
        val now = SystemClock.uptimeMillis()
        // ACTION_MULTIPLE with KEYCODE_UNKNOWN carries the full string in one event.
        val event = KeyEvent(now, text, KeyCharacterMap.VIRTUAL_KEYBOARD, 0)
        try {
            service.injectKey(event)
        } catch (e: Exception) {
            // Some OEMs drop ACTION_MULTIPLE; fall back to per-character injection.
            Log.w(TAG, "ACTION_MULTIPLE failed ($e); falling back to per-char injection")
            val kcm = KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD)
            for (ch in text) {
                val events = kcm.getEvents(charArrayOf(ch))
                if (events != null) {
                    for (ev in events) service.injectKey(ev)
                } else {
                    Log.w(TAG, "no KeyEvents for character '${ch}' (U+${ch.code.toString(16)})")
                }
            }
        }
    }

    /**
     * Inject a touchscreen tap at the screen centre. For double-click, two tap sequences
     * are injected without delay (the OS deduces double-tap from event timing).
     */
    private fun injectMouseClick(service: IInputService, doubleClick: Boolean) {
        val (x, y) = screenCenter()
        val taps = if (doubleClick) 2 else 1
        repeat(taps) {
            val downTime = SystemClock.uptimeMillis()
            val down = MotionEvent.obtain(
                downTime, downTime, MotionEvent.ACTION_DOWN, x, y, 0,
            ).also { it.source = InputDevice.SOURCE_TOUCHSCREEN }
            val up = MotionEvent.obtain(
                downTime, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP, x, y, 0,
            ).also { it.source = InputDevice.SOURCE_TOUCHSCREEN }
            service.injectMotion(down)
            service.injectMotion(up)
            down.recycle()
            up.recycle()
        }
    }

    private fun injectMouseScroll(service: IInputService, direction: String) {
        val (x, y) = screenCenter()
        val vScroll = when (direction) {
            "up" -> 1.0f
            "down" -> -1.0f
            else -> 0.0f
        }
        val hScroll = when (direction) {
            "right" -> 1.0f
            "left" -> -1.0f
            else -> 0.0f
        }
        val now = SystemClock.uptimeMillis()
        val props = arrayOf(MotionEvent.PointerProperties().also { it.id = 0 })
        val coords = arrayOf(MotionEvent.PointerCoords().also { c ->
            c.x = x
            c.y = y
            c.setAxisValue(MotionEvent.AXIS_VSCROLL, vScroll)
            c.setAxisValue(MotionEvent.AXIS_HSCROLL, hScroll)
        })
        val scroll = MotionEvent.obtain(
            now, now, MotionEvent.ACTION_SCROLL,
            1, props, coords, 0, 0, 1.0f, 1.0f, 0, 0,
            InputDevice.SOURCE_MOUSE, 0,
        )
        service.injectMotion(scroll)
        scroll.recycle()
    }

    /** Returns the centre of the current display in pixels. */
    private fun screenCenter(): Pair<Float, Float> {
        val wm = appContext.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val bounds = wm.currentWindowMetrics.bounds
        return Pair(bounds.centerX().toFloat(), bounds.centerY().toFloat())
    }

    // ── Key name → KEYCODE mapping ────────────────────────────────────────────

    /**
     * Map from mapping-core key name strings to Android [KeyEvent] KEYCODE constants.
     *
     * Key name strings are defined in `crates/mapping-core/src/types/key_def.rs` (VALID_KEYS).
     * KEYCODE values are from [android.view.KeyEvent] constants; integer literals are used
     * for keys whose named constant requires a higher API level than minSdk (e.g. F13–F24
     * require API 29; brightness keys require API 33).
     */
    private val KEY_MAP: Map<String, Int> by lazy {
        buildMap {
            // Letters a–z (KEYCODE_A=29 … KEYCODE_Z=54)
            for (c in 'a'..'z') put(c.toString(), KeyEvent.KEYCODE_A + (c - 'a'))
            // Digits 0–9 (KEYCODE_0=7, KEYCODE_1=8 … KEYCODE_9=16)
            put("0", KeyEvent.KEYCODE_0)
            for (d in '1'..'9') put(d.toString(), KeyEvent.KEYCODE_1 + (d - '1'))
            // Function keys f1–f12 (KEYCODE_F1=131 … KEYCODE_F12=142)
            for (n in 1..12) put("f$n", KeyEvent.KEYCODE_F1 + (n - 1))
            // Function keys f13–f24 (KEYCODE_F13=183 … KEYCODE_F24=194; API 29+)
            for (n in 13..24) put("f$n", 170 + n) // 170+13=183, 170+24=194
            // Navigation
            put("backspace", KeyEvent.KEYCODE_DEL)
            put("delete", KeyEvent.KEYCODE_FORWARD_DEL)
            put("insert", KeyEvent.KEYCODE_INSERT)
            put("home", KeyEvent.KEYCODE_MOVE_HOME)
            put("end", KeyEvent.KEYCODE_MOVE_END)
            put("page_up", KeyEvent.KEYCODE_PAGE_UP)
            put("page_down", KeyEvent.KEYCODE_PAGE_DOWN)
            put("left_arrow", KeyEvent.KEYCODE_DPAD_LEFT)
            put("right_arrow", KeyEvent.KEYCODE_DPAD_RIGHT)
            put("up_arrow", KeyEvent.KEYCODE_DPAD_UP)
            put("down_arrow", KeyEvent.KEYCODE_DPAD_DOWN)
            put("return", KeyEvent.KEYCODE_ENTER)
            put("escape", KeyEvent.KEYCODE_ESCAPE)
            put("tab", KeyEvent.KEYCODE_TAB)
            put("space", KeyEvent.KEYCODE_SPACE)
            put("caps_lock", KeyEvent.KEYCODE_CAPS_LOCK)
            put("num_lock", KeyEvent.KEYCODE_NUM_LOCK)
            put("scroll_lock", KeyEvent.KEYCODE_SCROLL_LOCK)
            put("print_screen", KeyEvent.KEYCODE_SYSRQ)
            put("pause", KeyEvent.KEYCODE_BREAK)
            // Punctuation
            put("grave", KeyEvent.KEYCODE_GRAVE)
            put("minus", KeyEvent.KEYCODE_MINUS)
            put("equals", KeyEvent.KEYCODE_EQUALS)
            put("left_bracket", KeyEvent.KEYCODE_LEFT_BRACKET)
            put("right_bracket", KeyEvent.KEYCODE_RIGHT_BRACKET)
            put("backslash", KeyEvent.KEYCODE_BACKSLASH)
            put("semicolon", KeyEvent.KEYCODE_SEMICOLON)
            put("quote", KeyEvent.KEYCODE_APOSTROPHE)
            put("comma", KeyEvent.KEYCODE_COMMA)
            put("period", KeyEvent.KEYCODE_PERIOD)
            put("slash", KeyEvent.KEYCODE_SLASH)
            // Media
            put("media_play", KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE)
            put("media_next", KeyEvent.KEYCODE_MEDIA_NEXT)
            put("media_prev", KeyEvent.KEYCODE_MEDIA_PREVIOUS)
            put("media_stop", KeyEvent.KEYCODE_MEDIA_STOP)
            // Volume
            put("volume_up", KeyEvent.KEYCODE_VOLUME_UP)
            put("volume_down", KeyEvent.KEYCODE_VOLUME_DOWN)
            put("volume_mute", KeyEvent.KEYCODE_VOLUME_MUTE)
            // System (brightness keys: KEYCODE_BRIGHTNESS_DOWN=220, UP=221; API 33)
            put("brightness_down", 220)
            put("brightness_up", 221)
            put("eject", KeyEvent.KEYCODE_MEDIA_EJECT)
            put("mic_mute", KeyEvent.KEYCODE_MUTE)
            // Android navigation globals
            put("back", KeyEvent.KEYCODE_BACK)
        }
    }

    private fun keyNameToCode(name: String): Int? = KEY_MAP[name]

    private fun isModifierName(name: String): Boolean =
        name == "ctrl" || name == "shift" || name == "alt" || name == "meta"

    private fun modifierNameToMetaState(name: String): Int = when (name) {
        "ctrl" -> KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
        "shift" -> KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON
        "alt" -> KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON
        "meta" -> KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON
        else -> 0
    }

    private fun modifierNameToKeyCode(name: String): Int? = when (name) {
        "ctrl" -> KeyEvent.KEYCODE_CTRL_LEFT
        "shift" -> KeyEvent.KEYCODE_SHIFT_LEFT
        "alt" -> KeyEvent.KEYCODE_ALT_LEFT
        "meta" -> KeyEvent.KEYCODE_META_LEFT
        else -> null
    }

    private fun modifierCodeToMetaMask(keyCode: Int): Int = when (keyCode) {
        KeyEvent.KEYCODE_CTRL_LEFT, KeyEvent.KEYCODE_CTRL_RIGHT ->
            KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
        KeyEvent.KEYCODE_SHIFT_LEFT, KeyEvent.KEYCODE_SHIFT_RIGHT ->
            KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON
        KeyEvent.KEYCODE_ALT_LEFT, KeyEvent.KEYCODE_ALT_RIGHT ->
            KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON
        KeyEvent.KEYCODE_META_LEFT, KeyEvent.KEYCODE_META_RIGHT ->
            KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON
        else -> 0
    }

    private fun jsonArrayToMetaState(arr: JSONArray?): Int {
        if (arr == null) return 0
        return (0 until arr.length()).fold(0) { acc, i ->
            acc or modifierNameToMetaState(arr.getString(i))
        }
    }

    private fun jsonArrayToModifierCodes(arr: JSONArray?): List<Int> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).mapNotNull { i ->
            modifierNameToKeyCode(arr.getString(i))
        }
    }
}
