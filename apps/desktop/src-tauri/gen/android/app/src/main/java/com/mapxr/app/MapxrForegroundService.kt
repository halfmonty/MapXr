package com.mapxr.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder
import android.util.Log

private const val TAG = "MapxrForegroundService"

// Notification channel ID (must be stable across restarts).
private const val CHANNEL_ID = "mapxr_service"
// Notification ID (must be non-zero to identify the foreground notification).
private const val NOTIFICATION_ID = 1001
// Intent actions.
const val ACTION_STOP_SERVICE = "com.mapxr.app.action.STOP_SERVICE"
const val EXTRA_DEVICE_COUNT = "device_count"
const val EXTRA_PROFILE_NAME = "profile_name"

/**
 * Persistent foreground service that keeps the app process alive while Tap
 * Strap devices are connected and the app is in the background.
 *
 * Android kills background processes aggressively; a foreground service with
 * a visible notification is the standard mechanism for apps that must stay
 * responsive (BLE, music playback, navigation, etc.).
 *
 * ## Lifecycle
 *
 * - **Started** by [BlePlugin] when the first Tap device connects, or when the
 *   app transitions to the background with a device already connected.
 * - **Stopped** when the user taps the "Stop" action in the notification, or
 *   when [BlePlugin] calls [stopSelf] after the last device disconnects.
 *
 * ## Notification content
 *
 * The notification is updated via [startService] with updated extras — Android
 * re-delivers the intent to [onStartCommand] which refreshes the notification.
 * [BlePlugin] calls [updateNotification] whenever the device count or active
 * profile changes.
 *
 * ## Service type
 *
 * Declared as `foregroundServiceType="connectedDevice"` in `AndroidManifest.xml`
 * (required for Android 14+ to maintain BLE GATT connections in the background).
 */
class MapxrForegroundService : Service() {

    private lateinit var notificationManager: NotificationManager

    override fun onCreate() {
        super.onCreate()
        notificationManager = getSystemService(Context.NOTIFICATION_SERVICE)
            as NotificationManager
        createNotificationChannel()
        Log.d(TAG, "service created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP_SERVICE) {
            Log.i(TAG, "stop requested by user via notification action")
            // Notify BlePlugin so it can disconnect devices.
            BlePlugin.onUserStopRequested()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return START_NOT_STICKY
        }

        val deviceCount = intent?.getIntExtra(EXTRA_DEVICE_COUNT, 0) ?: 0
        val profileName = intent?.getStringExtra(EXTRA_PROFILE_NAME)
        val notification = buildNotification(deviceCount, profileName)

        startForeground(NOTIFICATION_ID, notification)
        Log.d(TAG, "service running — $deviceCount device(s), profile=$profileName")
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        Log.d(TAG, "service destroyed")
    }

    // ── Notification builder ──────────────────────────────────────────────────

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            "MapXr background service",
            // LOW importance: no sound, no heads-up; just a persistent icon.
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Shows while a Tap Strap device is connected"
            setShowBadge(false)
        }
        notificationManager.createNotificationChannel(channel)
    }

    private fun buildNotification(deviceCount: Int, profileName: String?): Notification {
        // "Stop" action PendingIntent.
        val stopIntent = Intent(this, MapxrForegroundService::class.java).apply {
            action = ACTION_STOP_SERVICE
        }
        val stopPendingIntent = PendingIntent.getService(
            this,
            0,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        // Tap the notification to bring the app to the foreground.
        val openIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val openPendingIntent = PendingIntent.getActivity(
            this,
            0,
            openIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

        val contentText = when {
            deviceCount == 0 -> "No devices connected"
            profileName.isNullOrBlank() -> {
                val deviceWord = if (deviceCount == 1) "device" else "devices"
                "$deviceCount $deviceWord connected"
            }
            else -> {
                val deviceWord = if (deviceCount == 1) "device" else "devices"
                "$deviceCount $deviceWord connected · $profileName"
            }
        }

        val keyboardStatus = when (ShizukuDispatcher.state.value) {
            is ShizukuState.Active -> "Keyboard: active"
            is ShizukuState.Binding,
            is ShizukuState.Reconnecting -> "Keyboard: reconnecting…"
            else -> "Keyboard: setup needed"
        }

        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("MapXr active")
            .setContentText(contentText)
            .setStyle(Notification.BigTextStyle().bigText("$contentText\n$keyboardStatus"))
            .setContentIntent(openPendingIntent)
            .setOngoing(true)
            .setShowWhen(false)
            .addAction(
                Notification.Action.Builder(
                    null,
                    "Stop",
                    stopPendingIntent,
                ).build(),
            )
            .build()
    }

    // ── Companion helpers (called from BlePlugin) ─────────────────────────────

    companion object {
        /**
         * Start or refresh the foreground service with the current device count
         * and active profile name.
         *
         * Safe to call repeatedly — Android re-delivers the intent to
         * [onStartCommand] which updates the notification in place.
         */
        fun start(context: Context, deviceCount: Int, profileName: String?) {
            val intent = Intent(context, MapxrForegroundService::class.java).apply {
                putExtra(EXTRA_DEVICE_COUNT, deviceCount)
                if (profileName != null) putExtra(EXTRA_PROFILE_NAME, profileName)
            }
            context.startForegroundService(intent)
        }

        /**
         * Stop the foreground service (called when the last device disconnects).
         */
        fun stop(context: Context) {
            context.stopService(Intent(context, MapxrForegroundService::class.java))
        }
    }
}
