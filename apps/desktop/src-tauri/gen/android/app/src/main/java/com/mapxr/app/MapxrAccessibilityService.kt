package com.mapxr.app

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Path
import android.graphics.Point
import android.media.AudioManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.WindowManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

private const val TAG = "MapxrAccessibilityService"

/**
 * Minimal AccessibilityService that injects key events into the foreground app
 * on behalf of the Tap wearable.
 *
 * Configuration (`res/xml/accessibility_service_config.xml`):
 *   - `accessibilityEventTypes="typeWindowStateChanged"` — minimal subscription; events are ignored
 *   - `canPerformGestures="true"` — required for mouse click/scroll simulation
 *   - `flagRequestFilterKeyEvents` — retained for future key-filtering capability
 *
 * Sole purpose: dispatch key and gesture events forwarded by [AccessibilityPlugin].
 *
 * Key injection strategy (no `dispatchKeyEvent` on [AccessibilityService]):
 *   - Navigation keys → [performGlobalAction]
 *   - Media/volume keys → [android.media.AudioManager.dispatchMediaKeyEvent]
 *   - Printable characters → [KeyCharacterMap] → [injectText]
 *   - DEL/ENTER in focused input → [AccessibilityNodeInfo.performAction]
 */
class MapxrAccessibilityService : AccessibilityService() {

    override fun onServiceConnected() {
        instance = this
        Log.i(TAG, "MapxrAccessibilityService connected")
    }

    override fun onUnbind(intent: Intent?): Boolean {
        instance = null
        Log.i(TAG, "MapxrAccessibilityService unbound")
        return super.onUnbind(intent)
    }

    // We subscribe to typeWindowStateChanged but ignore all events — injection only.
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}
    override fun onInterrupt() {}

    // ── Key injection ──────────────────────────────────────────────────────────

    /**
     * Inject a key event for [keyCode] with [metaState].
     *
     * Dispatch strategy (in priority order):
     * 1. Global navigation keys (Back, Home, Recents) → [performGlobalAction]
     * 2. Media / volume keys → [AudioManager.dispatchMediaKeyEvent]
     * 3. Printable characters → [KeyCharacterMap] lookup → [injectText]
     * 4. DEL / ENTER on a focused input node → [AccessibilityNodeInfo.performAction]
     * 5. Other keys → warning log (no public Android API for arbitrary key injection
     *    without system / root permission)
     *
     * @param keyCode    Android `KeyEvent.KEYCODE_*` constant.
     * @param metaState  Modifier flags (e.g. `KeyEvent.META_CTRL_ON`).
     */
    fun injectKey(keyCode: Int, metaState: Int = 0) {
        // 1. System navigation keys
        val globalAction = when (keyCode) {
            KeyEvent.KEYCODE_BACK       -> GLOBAL_ACTION_BACK
            KeyEvent.KEYCODE_HOME       -> GLOBAL_ACTION_HOME
            KeyEvent.KEYCODE_APP_SWITCH -> GLOBAL_ACTION_RECENTS
            else                        -> -1
        }
        if (globalAction != -1) {
            performGlobalAction(globalAction)
            return
        }

        // 2. Media / volume keys
        if (keyCode in MEDIA_KEY_CODES) {
            val audio = getSystemService(AUDIO_SERVICE) as? AudioManager ?: return
            val now = android.os.SystemClock.uptimeMillis()
            audio.dispatchMediaKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0))
            audio.dispatchMediaKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_UP, keyCode, 0))
            return
        }

        // 3. Printable characters — let the system key character map do the conversion
        val kcm = try {
            KeyCharacterMap.load(KeyCharacterMap.VIRTUAL_KEYBOARD)
        } catch (_: Exception) {
            null
        }
        val ch = kcm?.get(keyCode, metaState) ?: 0
        if (ch != 0) {
            injectText(ch.toChar().toString())
            return
        }

        // 4. Editing keys on the focused input node
        val focused = rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        if (focused != null) {
            when (keyCode) {
                KeyEvent.KEYCODE_DEL -> {
                    val text = focused.text?.toString() ?: ""
                    val sel = focused.textSelectionStart.coerceAtLeast(0)
                    if (sel > 0) {
                        focused.performAction(
                            AccessibilityNodeInfo.ACTION_SET_TEXT,
                            Bundle().apply {
                                putCharSequence(
                                    AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                                    text.removeRange(sel - 1, sel),
                                )
                            },
                        )
                        focused.performAction(
                            AccessibilityNodeInfo.ACTION_SET_SELECTION,
                            Bundle().apply {
                                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, sel - 1)
                                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, sel - 1)
                            },
                        )
                    }
                }
                KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                    // AccessibilityAction.ACTION_IME_ENTER added in API 30.
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                        focused.performAction(
                            AccessibilityNodeInfo.AccessibilityAction.ACTION_IME_ENTER.id,
                        )
                    } else {
                        Log.d(TAG, "injectKey: IME enter action requires API 30; skipping")
                    }
                }
                else ->
                    Log.w(TAG, "injectKey: keyCode $keyCode has no Android equivalent without system permission")
            }
        } else {
            Log.w(TAG, "injectKey: keyCode $keyCode not injectable (no focused input field)")
        }
    }

    /**
     * Inject raw Unicode text into the focused input field.
     *
     * If an input-focused [AccessibilityNodeInfo] is available, uses
     * [AccessibilityNodeInfo.ACTION_SET_TEXT] to insert [text] at the current
     * cursor position.  Falls back to clipboard-paste for non-input contexts.
     */
    fun injectText(text: String) {
        if (text.isEmpty()) return
        val root = rootInActiveWindow ?: run {
            Log.w(TAG, "injectText: no active window")
            return
        }
        val focused = root.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        if (focused != null) {
            val existing = focused.text?.toString() ?: ""
            val selStart = focused.textSelectionStart.coerceAtLeast(0)
            val selEnd = focused.textSelectionEnd.coerceAtLeast(selStart)
            val newText = existing.substring(0, selStart) + text + existing.substring(selEnd)
            focused.performAction(
                AccessibilityNodeInfo.ACTION_SET_TEXT,
                Bundle().apply {
                    putCharSequence(
                        AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                        newText,
                    )
                },
            )
            val cursorPos = selStart + text.length
            focused.performAction(
                AccessibilityNodeInfo.ACTION_SET_SELECTION,
                Bundle().apply {
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, cursorPos)
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, cursorPos)
                },
            )
        } else {
            // Clipboard fallback — pastes into whatever has accessibility focus.
            val cm = getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            cm?.setPrimaryClip(ClipData.newPlainText("mapxr", text))
            root.findFocus(AccessibilityNodeInfo.FOCUS_ACCESSIBILITY)
                ?.performAction(AccessibilityNodeInfo.ACTION_PASTE)
            Log.d(TAG, "injectText: using clipboard fallback for text injection")
        }
    }

    // ── Gesture injection ─────────────────────────────────────────────────────

    /**
     * Inject a single tap at ([cx], [cy]).
     *
     * Requires API 24+. Because our `minSdk` is 26, this is always available.
     * Best-effort: logs a warning if `dispatchGesture` returns false (e.g., the
     * target app has `FLAG_SECURE` or gesture injection is disabled).
     */
    fun injectTap(cx: Float, cy: Float) {
        val path = Path().apply { moveTo(cx, cy) }
        val stroke = GestureDescription.StrokeDescription(path, /* startTime= */ 0L, /* duration= */ 100L)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        if (!dispatchGesture(gesture, null, null)) {
            Log.w(TAG, "injectTap($cx, $cy): dispatchGesture returned false")
        }
    }

    /**
     * Inject two quick taps at ([cx], [cy]) for a double-click.
     */
    fun injectDoubleTap(cx: Float, cy: Float) {
        // First tap
        val path1 = Path().apply { moveTo(cx, cy) }
        val stroke1 = GestureDescription.StrokeDescription(path1, 0L, 100L)
        // Second tap — starts after first finishes, offset slightly for the OS
        val path2 = Path().apply { moveTo(cx, cy) }
        val stroke2 = GestureDescription.StrokeDescription(path2, 200L, 100L)
        val gesture = GestureDescription.Builder()
            .addStroke(stroke1)
            .addStroke(stroke2)
            .build()
        if (!dispatchGesture(gesture, null, null)) {
            Log.w(TAG, "injectDoubleTap($cx, $cy): dispatchGesture returned false")
        }
    }

    /**
     * Inject a swipe gesture starting at screen centre in [direction].
     *
     * [direction] must be one of `"up"`, `"down"`, `"left"`, `"right"`.
     * [distance] is the swipe length in pixels (default 300 dp-equivalent).
     */
    fun injectSwipe(cx: Float, cy: Float, direction: String, distance: Float = 300f) {
        val (dx, dy) = when (direction) {
            "up"    -> Pair(0f, -distance)
            "down"  -> Pair(0f, distance)
            "left"  -> Pair(-distance, 0f)
            "right" -> Pair(distance, 0f)
            else    -> {
                Log.w(TAG, "injectSwipe: unknown direction '$direction'")
                return
            }
        }
        val path = Path().apply {
            moveTo(cx, cy)
            lineTo(cx + dx, cy + dy)
        }
        val stroke = GestureDescription.StrokeDescription(path, 0L, 300L)
        val gesture = GestureDescription.Builder().addStroke(stroke).build()
        if (!dispatchGesture(gesture, null, null)) {
            Log.w(TAG, "injectSwipe direction=$direction: dispatchGesture returned false")
        }
    }

    /**
     * Return the centre of the physical display in pixels.
     *
     * Used as the target point for mouse-emulation gestures when no focused
     * view location is available.
     */
    fun displayCenter(): Pair<Float, Float> {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val wm = getSystemService(WindowManager::class.java)
            val bounds = wm.currentWindowMetrics.bounds
            Pair(bounds.width() / 2f, bounds.height() / 2f)
        } else {
            val wm = getSystemService(WINDOW_SERVICE) as WindowManager
            val point = Point()
            @Suppress("DEPRECATION")
            wm.defaultDisplay.getSize(point)
            Pair(point.x / 2f, point.y / 2f)
        }
    }

    // ── Companion ─────────────────────────────────────────────────────────────

    companion object {
        /** Current bound service instance, or `null` if not yet connected. */
        @Volatile
        var instance: MapxrAccessibilityService? = null

        /** Key codes routed to [AudioManager.dispatchMediaKeyEvent]. */
        private val MEDIA_KEY_CODES = intArrayOf(
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
            KeyEvent.KEYCODE_MEDIA_PLAY,
            KeyEvent.KEYCODE_MEDIA_PAUSE,
            KeyEvent.KEYCODE_MEDIA_STOP,
            KeyEvent.KEYCODE_MEDIA_NEXT,
            KeyEvent.KEYCODE_MEDIA_PREVIOUS,
            KeyEvent.KEYCODE_MEDIA_FAST_FORWARD,
            KeyEvent.KEYCODE_MEDIA_REWIND,
            KeyEvent.KEYCODE_VOLUME_UP,
            KeyEvent.KEYCODE_VOLUME_DOWN,
            KeyEvent.KEYCODE_VOLUME_MUTE,
            KeyEvent.KEYCODE_BRIGHTNESS_UP,
            KeyEvent.KEYCODE_BRIGHTNESS_DOWN,
        )
    }
}
