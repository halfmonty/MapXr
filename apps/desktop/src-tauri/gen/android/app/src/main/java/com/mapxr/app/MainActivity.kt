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
        pluginManager.load(null, "accessibility", AccessibilityPlugin(this), "{}")
        super.onCreate(savedInstanceState)
    }
}
