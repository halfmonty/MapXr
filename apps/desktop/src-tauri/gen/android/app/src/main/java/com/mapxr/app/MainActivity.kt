package com.mapxr.app

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        // Register Kotlin-only plugins before super.onCreate() so the PluginManager
        // has them available when Rust initialises the plugin bridge via JNI.
        pluginManager.load(null, "ble", BlePlugin(this), "{}")
        pluginManager.load(null, "battery", BatteryPlugin(this), "{}")
        pluginManager.load(null, "shizuku", ShizukuPlugin(this), "{}")
        super.onCreate(savedInstanceState)
        // Initialise Shizuku lifecycle — registers binder listeners and checks initial state.
        // Must run after super.onCreate() so the application context is fully ready.
        ShizukuDispatcher.init(this)
        // Store the JavaVM and ShizukuDispatcher class reference in the Rust JNI layer so
        // the pump can call ShizukuDispatcher.dispatch() from any thread without WebView.
        // Must run after super.onCreate() — Rust init() is called inside Tauri's setup.
        NativeBridge.registerShizukuDispatcher()
    }
}
