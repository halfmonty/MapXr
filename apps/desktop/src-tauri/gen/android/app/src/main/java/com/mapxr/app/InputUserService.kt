package com.mapxr.app

import android.util.Log
import android.view.InputEvent
import android.view.KeyEvent
import android.view.MotionEvent
import java.lang.reflect.Method

private const val TAG = "InputUserService"

/**
 * Shizuku UserService running as shell uid (2000).
 *
 * Started by Shizuku via [ShizukuDispatcher]. Exposes [IInputService] over Binder.
 * Shell uid holds `android.permission.INJECT_EVENTS`, which enables
 * [android.hardware.input.InputManager.injectInputEvent] via reflection since the method
 * is `@hide` but accessible from shell uid.
 *
 * Shizuku expects either a no-arg constructor or one that takes an [android.os.IBinder].
 * We use the no-arg form; Shizuku calls [asBinder] to obtain the IBinder to return to the client.
 */
class InputUserService : IInputService.Stub() {

    /** Lazy-initialised reflection handle for `InputManager(Global).injectInputEvent`. */
    private val injectMethod: Method by lazy { resolveInjectMethod() }

    /** The `InputManager` (or `InputManagerGlobal`) singleton instance. */
    private var managerInstance: Any? = null

    private fun resolveInjectMethod(): Method {
        // Android 13+ exposes InputManagerGlobal; older APIs use InputManager directly.
        val cls = try {
            Class.forName("android.hardware.input.InputManagerGlobal")
        } catch (_: ClassNotFoundException) {
            Class.forName("android.hardware.input.InputManager")
        }
        val getInstance = cls.getDeclaredMethod("getInstance").also { it.isAccessible = true }
        managerInstance = getInstance.invoke(null)
        return cls.getDeclaredMethod("injectInputEvent", InputEvent::class.java, Int::class.java)
            .also { it.isAccessible = true }
    }

    private fun inject(event: InputEvent) {
        try {
            // INJECT_INPUT_EVENT_MODE_ASYNC = 0
            injectMethod.invoke(managerInstance, event, 0)
        } catch (e: Exception) {
            Log.e(TAG, "injectInputEvent failed: $e")
            // Last-resort fallback for key events: shell command (no timing guarantees).
            if (event is KeyEvent && event.action == KeyEvent.ACTION_DOWN) {
                Log.w(TAG, "falling back to 'input keyevent' for keyCode=${event.keyCode}")
                try {
                    Runtime.getRuntime().exec(arrayOf("input", "keyevent", "${event.keyCode}"))
                } catch (ex: Exception) {
                    Log.e(TAG, "fallback also failed: $ex")
                }
            }
        }
    }

    override fun injectKey(event: KeyEvent) = inject(event)

    override fun injectMotion(event: MotionEvent) = inject(event)

    override fun destroy() = System.exit(0)
}
