package com.mapxr.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

private const val TAG = "MapxrBatteryPlugin"

// OEM manufacturer string prefixes (lowercased Build.MANUFACTURER values).
private const val OEM_XIAOMI = "xiaomi"
private const val OEM_REDMI = "redmi"
private const val OEM_SAMSUNG = "samsung"
private const val OEM_HUAWEI = "huawei"
private const val OEM_HONOR = "honor"
private const val OEM_OPPO = "oppo"
private const val OEM_ONEPLUS = "oneplus"
private const val OEM_REALME = "realme"
private const val OEM_VIVO = "vivo"

/**
 * Tauri Kotlin plugin for Android battery optimisation setup.
 *
 * Provides:
 * - [getOemInfo] — detect manufacturer and return OEM-specific instructions
 * - [checkBatteryExemptionGranted] — check if the app is already exempt
 * - [requestBatteryExemption] — request `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`
 * - [openOemBatterySettings] — deep-link to the OEM-specific battery settings screen
 *
 * These are consumed by `BatterySetupWizard.svelte` in the WebView.
 */
@TauriPlugin
class BatteryPlugin(private val activity: Activity) : Plugin(activity) {

    /**
     * Return manufacturer information and OEM-specific battery setup instructions.
     *
     * Response shape:
     * ```json
     * {
     *   "manufacturer": "xiaomi",
     *   "displayName": "Xiaomi / Redmi / POCO",
     *   "hasOemStep": true,
     *   "oemInstructions": "...",
     *   "exemptionGranted": false
     * }
     * ```
     */
    @Command
    fun getOemInfo(invoke: Invoke) {
        val manufacturer = Build.MANUFACTURER.lowercase().trim()
        val (displayName, hasOemStep, oemInstructions) = oemDetails(manufacturer)
        val exemptionGranted = isBatteryExemptionGranted()

        invoke.resolve(JSObject().apply {
            put("manufacturer", manufacturer)
            put("displayName", displayName)
            put("hasOemStep", hasOemStep)
            put("oemInstructions", oemInstructions)
            put("exemptionGranted", exemptionGranted)
        })
    }

    /**
     * Return whether the app currently holds a battery optimisation exemption.
     *
     * Resolves `{ "granted": boolean }`.
     */
    @Command
    fun checkBatteryExemptionGranted(invoke: Invoke) {
        invoke.resolve(JSObject().apply {
            put("granted", isBatteryExemptionGranted())
        })
    }

    /**
     * Open the system dialog to request battery optimisation exemption.
     *
     * On Android 6+ (API 23) this launches `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`
     * for the app package, which shows a one-tap system dialog.
     * On older API levels the generic battery optimisation settings screen is shown instead.
     *
     * The call resolves immediately after launching the intent; the result
     * (granted / denied) is determined when the user returns to the app and
     * calls [checkBatteryExemptionGranted].
     */
    @Command
    fun requestBatteryExemption(invoke: Invoke) {
        try {
            val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                    data = Uri.parse("package:${activity.packageName}")
                }
            } else {
                Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
            }
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            Log.w(TAG, "requestBatteryExemption failed: ${e.message}")
            // Fallback to general battery settings.
            try {
                activity.startActivity(
                    Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
                )
            } catch (_: Exception) {}
            invoke.resolve() // best-effort; never reject
        }
    }

    /**
     * Open the OEM-specific battery settings screen for this manufacturer.
     *
     * Falls back to the generic `ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS`
     * if the OEM-specific intent is not available.
     */
    @Command
    fun openOemBatterySettings(invoke: Invoke) {
        val manufacturer = Build.MANUFACTURER.lowercase().trim()
        val launched = tryLaunchOemIntent(manufacturer)
        if (!launched) {
            try {
                activity.startActivity(
                    Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
                )
            } catch (e: Exception) {
                Log.w(TAG, "openOemBatterySettings fallback failed: ${e.message}")
            }
        }
        invoke.resolve()
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private fun isBatteryExemptionGranted(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val pm = activity.getSystemService(android.content.Context.POWER_SERVICE) as? PowerManager
            pm?.isIgnoringBatteryOptimizations(activity.packageName) == true
        } else {
            true // Pre-Marshmallow: battery optimisation doesn't apply
        }
    }

    /**
     * Attempt to launch the OEM-specific battery settings intent.
     * Returns `true` if the intent was successfully launched.
     */
    private fun tryLaunchOemIntent(manufacturer: String): Boolean {
        val intent = when {
            manufacturer.startsWith(OEM_XIAOMI) || manufacturer.startsWith(OEM_REDMI) -> {
                // Xiaomi / Redmi / POCO: Settings → Apps → mapxr → Autostart
                Intent("miui.intent.action.APP_PERM_EDITOR").apply {
                    setClassName(
                        "com.miui.securitycenter",
                        "com.miui.permcenter.autostart.AutoStartManagementActivity",
                    )
                }
            }
            manufacturer.startsWith(OEM_HUAWEI) || manufacturer.startsWith(OEM_HONOR) -> {
                // Huawei / Honor: App launch management
                Intent().apply {
                    setClassName(
                        "com.huawei.systemmanager",
                        "com.huawei.systemmanager.startupmgr.ui.StartupNormalAppListActivity",
                    )
                }
            }
            manufacturer.startsWith(OEM_OPPO) ||
                    manufacturer.startsWith(OEM_ONEPLUS) ||
                    manufacturer.startsWith(OEM_REALME) -> {
                // OPPO / OnePlus / Realme: App details (battery section)
                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                    data = Uri.parse("package:${activity.packageName}")
                }
            }
            manufacturer.startsWith(OEM_VIVO) -> {
                // Vivo: Background app management
                Intent().apply {
                    setClassName(
                        "com.vivo.permissionmanager",
                        "com.vivo.permissionmanager.activity.BgStartUpManagerActivity",
                    )
                }
            }
            manufacturer.startsWith(OEM_SAMSUNG) -> {
                // Samsung: App details (battery section handles "Never sleeping")
                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                    data = Uri.parse("package:${activity.packageName}")
                }
            }
            else -> return false
        }

        return try {
            activity.startActivity(intent)
            true
        } catch (e: Exception) {
            Log.w(TAG, "OEM intent failed for '$manufacturer': ${e.message}")
            false
        }
    }

    /**
     * Return display name, hasOemStep, and OEM instructions for the manufacturer.
     */
    private fun oemDetails(manufacturer: String): Triple<String, Boolean, String> {
        return when {
            manufacturer.startsWith(OEM_XIAOMI) || manufacturer.startsWith(OEM_REDMI) ->
                Triple(
                    "Xiaomi / Redmi / POCO",
                    true,
                    "Go to Settings → Apps → mapxr → tap \"Autostart\" and switch it ON. " +
                        "This prevents MIUI from killing the app when it is in the background.",
                )
            manufacturer.startsWith(OEM_SAMSUNG) ->
                Triple(
                    "Samsung",
                    true,
                    "Go to Settings → Battery → Background usage limits → Sleeping apps, " +
                        "then find mapxr and set it to \"Never sleeping\".",
                )
            manufacturer.startsWith(OEM_HUAWEI) || manufacturer.startsWith(OEM_HONOR) ->
                Triple(
                    "Huawei / Honor",
                    true,
                    "Go to Settings → Battery → App launch → find mapxr and set it to " +
                        "\"Manage manually\", then enable all three options (Auto-launch, " +
                        "Secondary launch, Run in background).",
                )
            manufacturer.startsWith(OEM_OPPO) ->
                Triple(
                    "OPPO / ColorOS",
                    true,
                    "Go to Settings → Battery → App quick freeze → make sure mapxr is not " +
                        "in the frozen list. Also enable \"Allow auto-startup\" in App details.",
                )
            manufacturer.startsWith(OEM_ONEPLUS) ->
                Triple(
                    "OnePlus",
                    true,
                    "Go to Settings → Battery → Battery optimisation → find mapxr and set " +
                        "it to \"Don't optimise\". Also enable Auto-launch in App details.",
                )
            manufacturer.startsWith(OEM_REALME) ->
                Triple(
                    "Realme / Narzo",
                    true,
                    "Go to Settings → Battery → App quick freeze → exclude mapxr. " +
                        "Also enable Auto-start in App details → Permissions.",
                )
            manufacturer.startsWith(OEM_VIVO) ->
                Triple(
                    "Vivo / iQOO",
                    true,
                    "Go to Settings → Battery → Background power consumption → find mapxr " +
                        "and disable battery restrictions.",
                )
            else ->
                Triple(
                    Build.MANUFACTURER,
                    false,
                    "",
                )
        }
    }
}
