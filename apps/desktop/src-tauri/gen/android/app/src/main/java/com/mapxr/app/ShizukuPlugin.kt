package com.mapxr.app

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val SHIZUKU_PACKAGE = "moe.shizuku.privileged.api"
private const val PLAY_STORE_URL = "https://play.google.com/store/apps/details?id=$SHIZUKU_PACKAGE"

/**
 * Tauri plugin exposing Shizuku integration state and control to the Svelte UI.
 *
 * Registered in [MainActivity] as plugin name `"shizuku"`.
 *
 * Commands:
 * - [getShizukuState] — current [ShizukuState] as string + device API level
 * - [requestShizukuPermission] — trigger the Shizuku permission dialog
 * - [openShizukuApp] — launch Shizuku, or open Play Store if not installed
 */
@TauriPlugin
class ShizukuPlugin(private val activity: android.app.Activity) : Plugin(activity) {

    /**
     * Returns the current Shizuku integration state and device API level.
     *
     * Response shape:
     * ```json
     * { "state": "Active", "apiLevel": 36 }
     * ```
     * Possible `state` values: `"Unsupported"`, `"NotInstalled"`, `"NotRunning"`,
     * `"PermissionRequired"`, `"Binding"`, `"Active"`, `"Reconnecting"`.
     */
    @Command
    fun getShizukuState(invoke: Invoke) {
        val stateStr = when (ShizukuDispatcher.state.value) {
            is ShizukuState.Unsupported -> "Unsupported"
            is ShizukuState.NotInstalled -> "NotInstalled"
            is ShizukuState.NotRunning -> "NotRunning"
            is ShizukuState.PermissionRequired -> "PermissionRequired"
            is ShizukuState.Binding -> "Binding"
            is ShizukuState.Active -> "Active"
            is ShizukuState.Reconnecting -> "Reconnecting"
        }
        val result = JSObject().apply {
            put("state", stateStr)
            put("apiLevel", Build.VERSION.SDK_INT)
        }
        invoke.resolve(result)
    }

    /**
     * Trigger the Shizuku in-app permission request dialog.
     *
     * Resolves immediately; the state update arrives via polling ([getShizukuState]).
     */
    @Command
    fun requestShizukuPermission(invoke: Invoke) {
        ShizukuDispatcher.requestPermission()
        invoke.resolve()
    }

    /**
     * Launch the Shizuku app if installed, otherwise open its Play Store listing.
     *
     * Always resolves.
     */
    @Command
    fun openShizukuApp(invoke: Invoke) {
        try {
            val launchIntent = activity.packageManager.getLaunchIntentForPackage(SHIZUKU_PACKAGE)
            if (launchIntent != null) {
                launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                activity.startActivity(launchIntent)
            } else {
                val storeIntent = Intent(Intent.ACTION_VIEW, Uri.parse(PLAY_STORE_URL)).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                activity.startActivity(storeIntent)
            }
        } catch (e: Exception) {
            Log.e("ShizukuPlugin", "openShizukuApp failed: $e")
        }
        invoke.resolve()
    }
}
