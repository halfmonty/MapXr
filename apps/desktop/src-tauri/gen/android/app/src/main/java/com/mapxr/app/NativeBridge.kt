package com.mapxr.app

/**
 * JNI bridge to the Rust engine inside `libmapxr_lib.so`.
 *
 * The library is already loaded by the Tauri-generated [WryActivity] glue code.
 * Loading it again here is safe — Android's class loader deduplicates loads.
 *
 * ## Lifecycle
 *
 * - [registerShizukuDispatcher] must be called from [MainActivity.onCreate] **after**
 *   `super.onCreate()` so that the Rust JNI statics are fully initialised before any
 *   tap event arrives.
 * - [processTapBytes] is called by [BlePlugin] on every BLE characteristic notification.
 * - Resolved actions are dispatched to [ShizukuDispatcher] via a JNI callback stored in
 *   the Rust `android_jni` module by [registerShizukuDispatcher].
 */
object NativeBridge {

    init {
        System.loadLibrary("mapxr_lib")
    }

    /**
     * Feed raw Tap Strap BLE bytes into the Rust combo engine.
     *
     * Sends the bytes through the Android pump mpsc channel (bypassing the WebView).
     * Resolved actions are dispatched to [ShizukuDispatcher] via [registerShizukuDispatcher].
     *
     * @param address MAC address of the source device (e.g. `"AA:BB:CC:DD:EE:FF"`).
     * @param bytes   Raw GATT characteristic value — `bytes[0]` is the tap code.
     * @return `"ok"` on success, `"err"` if the channel is closed or state is not
     *         yet initialised. The return value is informational only; errors are
     *         also logged to logcat via stderr.
     */
    external fun processTapBytes(address: String, bytes: ByteArray): String

    /**
     * Store the current [JavaVM] and a global reference to [ShizukuDispatcher] in
     * the Rust JNI layer so the pump can call [ShizukuDispatcher.dispatch] from any thread.
     *
     * Must be called once from [MainActivity.onCreate] **after** `super.onCreate()`.
     */
    external fun registerShizukuDispatcher()
}
