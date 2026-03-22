package com.mapxr.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.view.KeyEvent
import android.view.accessibility.AccessibilityManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray

private const val TAG = "MapxrAccessibilityPlugin"

/**
 * Tauri Kotlin plugin for Android AccessibilityService integration.
 *
 * Provides:
 * - [checkAccessibilityEnabled] — report whether [MapxrAccessibilityService] is enabled
 * - [openAccessibilitySettings] — open the Android Accessibility Settings screen
 * - [dispatchActions] — inject key events / text from combo engine output
 *
 * [dispatchActions] is called by the WebView JS bridge (`android-bridge.ts`)
 * whenever the Rust `android_pump` emits a `tap-actions-fired` event.
 *
 * Mouse click/scroll dispatch and the full key mapping table are implemented
 * in task 15.10.
 */
@TauriPlugin
class AccessibilityPlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * Return whether [MapxrAccessibilityService] is currently enabled in system settings.
     *
     * Resolves `{ "enabled": boolean }`.
     */
    @Command
    fun checkAccessibilityEnabled(invoke: Invoke) {
        invoke.resolve(JSObject().apply {
            put("enabled", isServiceEnabled())
        })
    }

    /**
     * Launch the system Accessibility Settings screen.
     *
     * The user must manually locate and enable [MapxrAccessibilityService] there.
     * After returning to the app, re-check with [checkAccessibilityEnabled].
     */
    @Command
    fun openAccessibilitySettings(invoke: Invoke) {
        try {
            activity.startActivity(
                Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
            )
        } catch (e: Exception) {
            Log.w(TAG, "openAccessibilitySettings failed: ${e.message}")
        }
        invoke.resolve()
    }

    /**
     * Dispatch a list of resolved actions from the combo engine.
     *
     * Called by the WebView JS bridge with the payload from `tap-actions-fired`.
     *
     * Each element has a `"type"` discriminant (snake_case, matching Rust serde output):
     *   - `"key"`           → single key with optional modifiers
     *   - `"key_chord"`     → all keys pressed simultaneously
     *   - `"type_string"`   → raw Unicode text injection
     *   - `"vibrate"`       → forwarded to [BlePlugin] (no-op if plugin unavailable)
     *   - `"mouse_click"`, `"mouse_double_click"`, `"mouse_scroll"` → TODO: task 15.10
     *   - others            → logged and skipped
     *
     * Resolves immediately. Injection errors are logged as warnings, never rejected.
     */
    @Command
    fun dispatchActions(invoke: Invoke) {
        val data = invoke.getArgs()
        if (data == null) {
            Log.w(TAG, "dispatchActions: null payload")
            invoke.resolve()
            return
        }

        val actionsArray: JSONArray = try {
            data.getJSONArray("actions")
        } catch (e: Exception) {
            Log.w(TAG, "dispatchActions: missing or invalid 'actions' field")
            invoke.resolve()
            return
        }

        val service = MapxrAccessibilityService.instance
        if (service == null) {
            Log.w(TAG, "dispatchActions: service not bound — ${actionsArray.length()} action(s) dropped")
        }

        for (i in 0 until actionsArray.length()) {
            val action = try {
                actionsArray.getJSONObject(i)
            } catch (_: Exception) {
                continue
            }

            when (val type = action.optString("type")) {
                "key" -> {
                    if (service != null) {
                        val keyName = action.optString("key")
                        val modifiersArray = action.optJSONArray("modifiers")
                        val keyCode = keyNameToCode(keyName)
                        if (keyCode != null) {
                            val metaState = modifiersToMetaState(modifiersArray)
                            service.injectKey(keyCode, metaState)
                        }
                    }
                }

                "key_chord" -> {
                    if (service != null) {
                        // Collect all keycodes; inject them with combined meta state.
                        val keysArray = action.optJSONArray("keys")
                        if (keysArray != null) {
                            var meta = 0
                            val codes = mutableListOf<Int>()
                            for (j in 0 until keysArray.length()) {
                                val k = keysArray.getString(j).lowercase()
                                when (k) {
                                    "ctrl", "control" -> meta = meta or KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
                                    "shift" -> meta = meta or KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON
                                    "alt" -> meta = meta or KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON
                                    "meta", "super" -> meta = meta or KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON
                                    else -> {
                                        val code = keyNameToCode(k)
                                        if (code != null) codes.add(code)
                                    }
                                }
                            }
                            for (code in codes) {
                                service.injectKey(code, meta)
                            }
                        }
                    }
                }

                "type_string" -> {
                    if (service != null) {
                        val text = action.optString("text")
                        if (text.isNotEmpty()) {
                            service.injectText(text)
                        }
                    }
                }

                "vibrate" -> {
                    // Forward vibrate actions to BlePlugin if available.
                    // BlePlugin vibrate command is implemented in task 15.10.
                    Log.d(TAG, "dispatchActions: vibrate action — deferred to task 15.10")
                }

                "mouse_click" -> {
                    if (service != null) {
                        val button = action.optString("button")
                        if (button == "middle") {
                            Log.d(TAG, "dispatchActions: mouse middle click has no Android equivalent; skipping")
                        } else {
                            val (cx, cy) = service.displayCenter()
                            service.injectTap(cx, cy)
                        }
                    }
                }

                "mouse_double_click" -> {
                    if (service != null) {
                        val button = action.optString("button")
                        if (button == "middle") {
                            Log.d(TAG, "dispatchActions: mouse middle double-click has no Android equivalent; skipping")
                        } else {
                            val (cx, cy) = service.displayCenter()
                            service.injectDoubleTap(cx, cy)
                        }
                    }
                }

                "mouse_scroll" -> {
                    if (service != null) {
                        val direction = action.optString("direction")
                        val (cx, cy) = service.displayCenter()
                        service.injectSwipe(cx, cy, direction)
                    }
                }

                "macro" -> {
                    // Macro steps contain nested actions. The JS bridge flattens these
                    // before dispatch — this case should not occur in normal operation.
                    Log.w(TAG, "dispatchActions: unexpected macro action (should be flattened by bridge)")
                }

                else -> Log.w(TAG, "dispatchActions: unhandled action type '$type'")
            }
        }

        invoke.resolve()
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /**
     * Return true if [MapxrAccessibilityService] appears in the list of
     * services currently enabled via Settings.Secure.
     */
    private fun isServiceEnabled(): Boolean {
        val am = activity.getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager
        if (am?.isEnabled != true) return false
        val componentFlat = "${activity.packageName}/.MapxrAccessibilityService"
        return try {
            val enabled = Settings.Secure.getString(
                activity.contentResolver,
                Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES,
            )
            enabled?.split(":")?.any { it.equals(componentFlat, ignoreCase = true) } == true
        } catch (e: Exception) {
            Log.w(TAG, "isServiceEnabled check failed: ${e.message}")
            false
        }
    }

    /**
     * Map a `mapping-core` key name string to an Android `KeyEvent.KEYCODE_*` constant.
     *
     * Full table per spec §7.4. Unsupported keys (Insert, PrintScreen, etc.)
     * return `null` and are logged as warnings.
     */
    @Suppress("ComplexMethod")
    private fun keyNameToCode(key: String): Int? = when (key.lowercase()) {
        // Letters
        "a" -> KeyEvent.KEYCODE_A;  "b" -> KeyEvent.KEYCODE_B
        "c" -> KeyEvent.KEYCODE_C;  "d" -> KeyEvent.KEYCODE_D
        "e" -> KeyEvent.KEYCODE_E;  "f" -> KeyEvent.KEYCODE_F
        "g" -> KeyEvent.KEYCODE_G;  "h" -> KeyEvent.KEYCODE_H
        "i" -> KeyEvent.KEYCODE_I;  "j" -> KeyEvent.KEYCODE_J
        "k" -> KeyEvent.KEYCODE_K;  "l" -> KeyEvent.KEYCODE_L
        "m" -> KeyEvent.KEYCODE_M;  "n" -> KeyEvent.KEYCODE_N
        "o" -> KeyEvent.KEYCODE_O;  "p" -> KeyEvent.KEYCODE_P
        "q" -> KeyEvent.KEYCODE_Q;  "r" -> KeyEvent.KEYCODE_R
        "s" -> KeyEvent.KEYCODE_S;  "t" -> KeyEvent.KEYCODE_T
        "u" -> KeyEvent.KEYCODE_U;  "v" -> KeyEvent.KEYCODE_V
        "w" -> KeyEvent.KEYCODE_W;  "x" -> KeyEvent.KEYCODE_X
        "y" -> KeyEvent.KEYCODE_Y;  "z" -> KeyEvent.KEYCODE_Z
        // Digits
        "0", "num0" -> KeyEvent.KEYCODE_0;  "1", "num1" -> KeyEvent.KEYCODE_1
        "2", "num2" -> KeyEvent.KEYCODE_2;  "3", "num3" -> KeyEvent.KEYCODE_3
        "4", "num4" -> KeyEvent.KEYCODE_4;  "5", "num5" -> KeyEvent.KEYCODE_5
        "6", "num6" -> KeyEvent.KEYCODE_6;  "7", "num7" -> KeyEvent.KEYCODE_7
        "8", "num8" -> KeyEvent.KEYCODE_8;  "9", "num9" -> KeyEvent.KEYCODE_9
        // Common editing
        "space"              -> KeyEvent.KEYCODE_SPACE
        "return", "enter"    -> KeyEvent.KEYCODE_ENTER
        "backspace"          -> KeyEvent.KEYCODE_DEL
        "tab"                -> KeyEvent.KEYCODE_TAB
        "escape"             -> KeyEvent.KEYCODE_ESCAPE
        "delete"             -> KeyEvent.KEYCODE_FORWARD_DEL
        // Navigation
        "up_arrow"           -> KeyEvent.KEYCODE_DPAD_UP
        "down_arrow"         -> KeyEvent.KEYCODE_DPAD_DOWN
        "left_arrow"         -> KeyEvent.KEYCODE_DPAD_LEFT
        "right_arrow"        -> KeyEvent.KEYCODE_DPAD_RIGHT
        "home"               -> KeyEvent.KEYCODE_MOVE_HOME
        "end"                -> KeyEvent.KEYCODE_MOVE_END
        "page_up"            -> KeyEvent.KEYCODE_PAGE_UP
        "page_down"          -> KeyEvent.KEYCODE_PAGE_DOWN
        // Modifiers
        "control"            -> KeyEvent.KEYCODE_CTRL_LEFT
        "shift"              -> KeyEvent.KEYCODE_SHIFT_LEFT
        "alt"                -> KeyEvent.KEYCODE_ALT_LEFT
        "meta", "super"      -> KeyEvent.KEYCODE_META_LEFT
        // Function keys (F13–F24 are not available on Android)
        "f1"  -> KeyEvent.KEYCODE_F1;  "f2"  -> KeyEvent.KEYCODE_F2
        "f3"  -> KeyEvent.KEYCODE_F3;  "f4"  -> KeyEvent.KEYCODE_F4
        "f5"  -> KeyEvent.KEYCODE_F5;  "f6"  -> KeyEvent.KEYCODE_F6
        "f7"  -> KeyEvent.KEYCODE_F7;  "f8"  -> KeyEvent.KEYCODE_F8
        "f9"  -> KeyEvent.KEYCODE_F9;  "f10" -> KeyEvent.KEYCODE_F10
        "f11" -> KeyEvent.KEYCODE_F11; "f12" -> KeyEvent.KEYCODE_F12
        // F13–F24: not available on Android — no-op
        "f13", "f14", "f15", "f16", "f17", "f18", "f19", "f20",
        "f21", "f22", "f23", "f24" -> {
            Log.w(TAG, "keyNameToCode: '$key' is not supported on Android (F13–F24)")
            null
        }
        // Media
        "media_play_pause"   -> KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE
        "media_next_track"   -> KeyEvent.KEYCODE_MEDIA_NEXT
        "media_prev_track"   -> KeyEvent.KEYCODE_MEDIA_PREVIOUS
        "media_stop"         -> KeyEvent.KEYCODE_MEDIA_STOP
        "volume_up"          -> KeyEvent.KEYCODE_VOLUME_UP
        "volume_down"        -> KeyEvent.KEYCODE_VOLUME_DOWN
        "volume_mute"        -> KeyEvent.KEYCODE_VOLUME_MUTE
        // Brightness (mapped to media brightness keys available on Android)
        "brightness_down"    -> KeyEvent.KEYCODE_BRIGHTNESS_DOWN
        "brightness_up"      -> KeyEvent.KEYCODE_BRIGHTNESS_UP
        // Unsupported on Android — no-op per spec §7.4
        "insert", "print_screen", "scroll_lock", "pause",
        "eject", "mic_mute" -> {
            Log.w(TAG, "keyNameToCode: '$key' is not supported on Android")
            null
        }
        else -> {
            Log.w(TAG, "keyNameToCode: unknown key '$key'")
            null
        }
    }

    /**
     * Convert a JSON array of modifier name strings to an Android `META_*` flags integer.
     *
     * Recognised modifier strings: `"control"`, `"ctrl"`, `"shift"`, `"alt"`,
     * `"meta"`, `"super"`.
     */
    private fun modifiersToMetaState(modifiers: JSONArray?): Int {
        if (modifiers == null) return 0
        var meta = 0
        for (i in 0 until modifiers.length()) {
            when (modifiers.optString(i).lowercase()) {
                "control", "ctrl" -> meta = meta or KeyEvent.META_CTRL_ON or KeyEvent.META_CTRL_LEFT_ON
                "shift"           -> meta = meta or KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON
                "alt"             -> meta = meta or KeyEvent.META_ALT_ON or KeyEvent.META_ALT_LEFT_ON
                "meta", "super"   -> meta = meta or KeyEvent.META_META_ON or KeyEvent.META_META_LEFT_ON
            }
        }
        return meta
    }
}
