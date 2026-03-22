package com.mapxr.app

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.le.BluetoothLeScanner
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelUuid
import android.util.Log
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import app.tauri.PermissionState
import java.util.UUID

// ── Tap Strap GATT UUIDs ─────────────────────────────────────────────────────

private val TAP_SERVICE_UUID: UUID =
    UUID.fromString("C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370")
private val TAP_DATA_CHAR_UUID: UUID =
    UUID.fromString("C3FF0005-1D8B-40FD-A56F-C7BD5D0F3370")

// Nordic UART Service — used to send controller mode commands to the Tap.
private val NUS_SERVICE_UUID: UUID =
    UUID.fromString("6E400001-B5A3-F393-E0A9-E50E24DCCA9E")
private val NUS_RX_UUID: UUID =
    UUID.fromString("6E400002-B5A3-F393-E0A9-E50E24DCCA9E")

// Standard CCCD descriptor UUID — needed to enable notifications on BLE characteristics.
private val CCCD_UUID: UUID =
    UUID.fromString("00002902-0000-1000-8000-00805F9B34FB")

// Controller mode entry command (matches desktop tap-ble crate: ENTER_CONTROLLER_MODE).
private val CONTROLLER_MODE_CMD: ByteArray = byteArrayOf(0x03, 0x0C, 0x00, 0x01)

// Reconnection policy.
private const val MAX_RECONNECT_ATTEMPTS = 5
private val RECONNECT_DELAYS_MS = longArrayOf(1_000, 2_000, 4_000, 8_000, 16_000)

// Re-send the controller mode command 2 s before the 10 s device timeout.
private const val KEEPALIVE_INTERVAL_MS = 8_000L

private const val TAG = "MapxrBlePlugin"
private const val SCAN_TIMEOUT_MS = 30_000L

// ── Invoke arguments ──────────────────────────────────────────────────────────

@InvokeArg
class ConnectArgs {
    lateinit var address: String
}

@InvokeArg
class DisconnectArgs {
    lateinit var address: String
}

@InvokeArg
class UpdateNotificationArgs {
    var deviceCount: Int = 0
    var profileName: String? = null
}

// ── Plugin ────────────────────────────────────────────────────────────────────

@TauriPlugin(
    permissions = [
        // API 31+ permissions (BLUETOOTH_SCAN / BLUETOOTH_CONNECT are runtime permissions)
        Permission(strings = [Manifest.permission.BLUETOOTH_SCAN], alias = "bluetoothScan"),
        Permission(strings = [Manifest.permission.BLUETOOTH_CONNECT], alias = "bluetoothConnect"),
        // Pre-API 31: BLE scanning required ACCESS_FINE_LOCATION
        Permission(strings = [Manifest.permission.ACCESS_FINE_LOCATION], alias = "location"),
    ]
)
class BlePlugin(private val activity: Activity) : Plugin(activity) {

    private val mainHandler = Handler(Looper.getMainLooper())
    private val bluetoothAdapter: BluetoothAdapter? by lazy {
        val manager = activity.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
        manager?.adapter
    }

    // Active GATT connections keyed by device address.
    private val connections = mutableMapOf<String, BluetoothGatt>()
    // Reconnection attempt counters.
    private val reconnectAttempts = mutableMapOf<String, Int>()
    // Addresses that are explicitly disconnected by the user (no reconnect).
    private val userDisconnected = mutableSetOf<String>()
    // Pending keepalive runnables keyed by device address.
    private val keepaliveRunnables = mutableMapOf<String, Runnable>()

    // Last-known profile name for notification updates.
    private var currentProfileName: String? = null

    private var scanner: BluetoothLeScanner? = null
    private var scanCallback: ScanCallback? = null
    private val scanStopRunnable = Runnable { stopScanInternal() }

    init {
        // Register this instance so MapxrForegroundService can notify us
        // when the user taps "Stop" in the notification.
        instance = this
    }

    // ── Debug logging ─────────────────────────────────────────────────────────

    /**
     * Emit a log message to both Android logcat and the WebView UI.
     *
     * The `ble-log` plugin event is received by the devices page debug panel via
     * addPluginListener so the user can see GATT progress without adb logcat.
     */
    private fun bleLog(msg: String) {
        Log.d(TAG, msg)
        val payload = JSObject().apply { put("msg", msg) }
        trigger("ble-log", payload)
    }

    // ── Permission request (task 15.3) ────────────────────────────────────────

    /**
     * Check whether all BLE permissions required on this API level are granted.
     *
     * Returns `{ "granted": true }` immediately if all permissions are in place,
     * or `{ "granted": false }` if any are missing.
     */
    @Command
    fun checkBlePermissions(invoke: Invoke) {
        val granted = areBlePermissionsGranted()
        invoke.resolve(JSObject().apply { put("granted", granted) })
    }

    /**
     * Request the BLE permissions required on this API level.
     *
     * On API 31+ this requests BLUETOOTH_SCAN and BLUETOOTH_CONNECT.
     * On API ≤ 30 this requests ACCESS_FINE_LOCATION (BLUETOOTH / BLUETOOTH_ADMIN
     * are install-time permissions and require no runtime request).
     *
     * Resolves with `{ "granted": true | false }` after the system dialog completes.
     */
    @Command
    fun requestBlePermissions(invoke: Invoke) {
        if (areBlePermissionsGranted()) {
            invoke.resolve(JSObject().apply { put("granted", true) })
            return
        }
        val aliases = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            arrayOf("bluetoothScan", "bluetoothConnect")
        } else {
            arrayOf("location")
        }
        requestPermissionForAliases(aliases, invoke, "onBlePermissionsResult")
    }

    @PermissionCallback
    private fun onBlePermissionsResult(invoke: Invoke) {
        val granted = areBlePermissionsGranted()
        invoke.resolve(JSObject().apply { put("granted", granted) })
    }

    /**
     * Return all Bluetooth-bonded (paired) devices visible to the OS.
     *
     * Requires BLUETOOTH_CONNECT on API 31+; no runtime permission below that.
     * Resolves `{ "devices": [{ "address": string, "name": string|null }] }`.
     *
     * The caller should connect to a returned device via [connect] and let GATT
     * service discovery confirm it is a Tap device before assigning a role.
     */
    @SuppressLint("MissingPermission")
    @Command
    fun listBondedDevices(invoke: Invoke) {
        val adapter = bluetoothAdapter
        if (adapter == null) {
            invoke.reject("Bluetooth is not available")
            return
        }
        val bonded = try {
            adapter.bondedDevices ?: emptySet()
        } catch (e: SecurityException) {
            invoke.reject("Bluetooth Connect permission is required")
            return
        }
        val devices = JSArray()
        for (device in bonded) {
            // Include BLE-capable and dual-mode devices; skip BR/EDR-only (classic audio etc.)
            val type = try { device.type } catch (_: SecurityException) { BluetoothDevice.DEVICE_TYPE_UNKNOWN }
            if (type == BluetoothDevice.DEVICE_TYPE_CLASSIC) continue
            val name = try { device.name } catch (_: SecurityException) { null }
            devices.put(JSObject().apply {
                put("address", device.address)
                put("name", name)
            })
        }
        invoke.resolve(JSObject().apply { put("devices", devices) })
    }

    private fun areBlePermissionsGranted(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            getPermissionState("bluetoothScan") == PermissionState.GRANTED &&
                getPermissionState("bluetoothConnect") == PermissionState.GRANTED
        } else {
            getPermissionState("location") == PermissionState.GRANTED
        }
    }

    // ── Foreground service commands (task 15.7) ───────────────────────────────

    /**
     * Start the MapXr foreground service.
     *
     * Called by the WebView UI (e.g. Settings "Auto-start service" toggle) or
     * internally by [BlePlugin] when the first device connects.
     */
    @Command
    fun startForegroundService(invoke: Invoke) {
        MapxrForegroundService.start(activity, connections.size, currentProfileName)
        invoke.resolve()
    }

    /**
     * Stop the MapXr foreground service.
     *
     * Disconnects all connected devices before stopping so the GATT connections
     * are cleaned up properly.
     */
    @SuppressLint("MissingPermission")
    @Command
    fun stopForegroundService(invoke: Invoke) {
        disconnectAll()
        MapxrForegroundService.stop(activity)
        invoke.resolve()
    }

    /**
     * Update the foreground service notification with the current device count
     * and active profile name.
     *
     * Called by the WebView JS when it receives a `layer-changed` event so the
     * notification stays in sync with the active profile.
     */
    @Command
    fun updateServiceNotification(invoke: Invoke) {
        val args = invoke.parseArgs(UpdateNotificationArgs::class.java)
        currentProfileName = args.profileName
        if (connections.isNotEmpty()) {
            MapxrForegroundService.start(activity, args.deviceCount, args.profileName)
        }
        invoke.resolve()
    }

    // ── BLE scanning (task 15.4) ──────────────────────────────────────────────

    /**
     * Start scanning for Tap Strap devices.
     *
     * Emits `ble-device-found` events for each discovered device. Scanning stops
     * automatically after 30 seconds or when [stopScan] is called.
     */
    @SuppressLint("MissingPermission")
    @Command
    fun startScan(invoke: Invoke) {
        if (!areBlePermissionsGranted()) {
            invoke.reject("BLE permissions not granted")
            return
        }
        val adapter = bluetoothAdapter
        if (adapter == null || !adapter.isEnabled) {
            invoke.reject("Bluetooth is not enabled")
            return
        }

        stopScanInternal() // stop any in-progress scan

        val bleScanner = adapter.bluetoothLeScanner
        if (bleScanner == null) {
            invoke.reject("BluetoothLeScanner unavailable")
            return
        }
        scanner = bleScanner

        val cb = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                val device = result.device
                val name = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    // API 31+: BLUETOOTH_CONNECT is required to read device name
                    if (getPermissionState("bluetoothConnect") == PermissionState.GRANTED)
                        device.name else null
                } else {
                    device.name
                }
                val payload = JSObject().apply {
                    put("address", device.address)
                    put("name", name)
                    put("rssi", result.rssi)
                }
                trigger("ble-device-found", payload)
            }

            override fun onScanFailed(errorCode: Int) {
                Log.e(TAG, "BLE scan failed with error $errorCode")
            }
        }
        scanCallback = cb

        val filter = ScanFilter.Builder()
            .setServiceUuid(ParcelUuid(TAP_SERVICE_UUID))
            .build()
        val settings = ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build()

        bleScanner.startScan(listOf(filter), settings, cb)
        mainHandler.postDelayed(scanStopRunnable, SCAN_TIMEOUT_MS)

        invoke.resolve()
    }

    /** Stop an in-progress BLE scan. */
    @Command
    fun stopScan(invoke: Invoke) {
        stopScanInternal()
        invoke.resolve()
    }

    @SuppressLint("MissingPermission")
    private fun stopScanInternal() {
        mainHandler.removeCallbacks(scanStopRunnable)
        scanCallback?.let { scanner?.stopScan(it) }
        scanCallback = null
        scanner = null
    }

    // ── Connection and GATT setup (task 15.4) ─────────────────────────────────

    /**
     * Connect to a Tap Strap device by MAC address.
     *
     * Performs the full GATT setup sequence:
     * 1. Discover services
     * 2. Find TAP_DATA_CHAR_UUID
     * 3. Enable notifications (CCCD write)
     * 4. Enter controller mode (write 0x0C 0x00)
     * 5. Emit `ble-device-connected`
     */
    @SuppressLint("MissingPermission")
    @Command
    fun connect(invoke: Invoke) {
        val args = invoke.parseArgs(ConnectArgs::class.java)
        val address = args.address

        if (!areBlePermissionsGranted()) {
            invoke.reject("BLE permissions not granted")
            return
        }
        if (connections.containsKey(address)) {
            invoke.resolve() // already connected
            return
        }

        val adapter = bluetoothAdapter
        if (adapter == null || !adapter.isEnabled) {
            invoke.reject("Bluetooth is not enabled")
            return
        }

        val device: BluetoothDevice = try {
            adapter.getRemoteDevice(address)
        } catch (e: IllegalArgumentException) {
            invoke.reject("Invalid device address: $address")
            return
        }

        userDisconnected.remove(address)
        reconnectAttempts[address] = 0

        bleLog("[$address] calling connectGatt (TRANSPORT_LE)")
        device.connectGatt(
            activity,
            /* autoConnect= */ false,
            buildGattCallback(address),
            BluetoothDevice.TRANSPORT_LE,
        )

        invoke.resolve()
    }

    /**
     * Disconnect from a Tap Strap device by MAC address.
     *
     * Clears the reconnection state so no automatic reconnect attempt is made.
     */
    @SuppressLint("MissingPermission")
    @Command
    fun disconnect(invoke: Invoke) {
        val args = invoke.parseArgs(DisconnectArgs::class.java)
        val address = args.address

        userDisconnected.add(address)
        reconnectAttempts.remove(address)
        cancelKeepalive(address)

        connections[address]?.let { gatt ->
            gatt.disconnect()
            gatt.close()
        }
        connections.remove(address)

        val payload = JSObject().apply {
            put("address", address)
            put("reason", "user_request")
        }
        trigger("ble-device-disconnected", payload)

        invoke.resolve()
    }

    // ── GATT callback ─────────────────────────────────────────────────────────

    @SuppressLint("MissingPermission")
    private fun buildGattCallback(address: String): BluetoothGattCallback {
        return object : BluetoothGattCallback() {

            override fun onConnectionStateChange(
                gatt: BluetoothGatt,
                status: Int,
                newState: Int,
            ) {
                when (newState) {
                    BluetoothGatt.STATE_CONNECTED -> {
                        bleLog("[$address] GATT connected (status=$status), refreshing cache…")
                        refreshGattCache(gatt)
                        gatt.discoverServices()
                        bleLog("[$address] discoverServices() called")
                    }
                    BluetoothGatt.STATE_DISCONNECTED -> {
                        bleLog("[$address] GATT disconnected (status=$status)")
                        cancelKeepalive(address)
                        connections.remove(address)
                        gatt.close()

                        if (address in userDisconnected) {
                            // disconnect() already fired the event; just
                            // stop the service if nothing else is connected.
                            if (connections.isEmpty()) {
                                MapxrForegroundService.stop(activity)
                            } else {
                                MapxrForegroundService.start(
                                    activity, connections.size, currentProfileName
                                )
                            }
                            return
                        }

                        // Unexpected disconnect — attempt reconnect with backoff.
                        val attempts = reconnectAttempts.getOrDefault(address, 0)
                        if (attempts < MAX_RECONNECT_ATTEMPTS) {
                            val delayMs = RECONNECT_DELAYS_MS[attempts]
                            reconnectAttempts[address] = attempts + 1
                            Log.i(TAG, "[$address] reconnect attempt ${attempts + 1} in ${delayMs}ms")
                            mainHandler.postDelayed({
                                if (address !in userDisconnected) {
                                    scheduleReconnect(address)
                                }
                            }, delayMs)
                        } else {
                            Log.w(TAG, "[$address] max reconnect attempts reached")
                            reconnectAttempts.remove(address)
                            val payload = JSObject().apply {
                                put("address", address)
                                put("reason", "reconnect_failed")
                            }
                            trigger("ble-device-disconnected", payload)
                            if (connections.isEmpty()) {
                                MapxrForegroundService.stop(activity)
                            } else {
                                MapxrForegroundService.start(
                                    activity, connections.size, currentProfileName
                                )
                            }
                        }
                    }
                }
            }

            override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    bleLog("[$address] service discovery FAILED status=$status")
                    gatt.disconnect()
                    return
                }

                val allServices = gatt.services.map { it.uuid.toString() }
                bleLog("[$address] discovered ${allServices.size} services: $allServices")

                val service = gatt.getService(TAP_SERVICE_UUID)
                if (service == null) {
                    bleLog("[$address] TAP service NOT found — disconnecting")
                    gatt.disconnect()
                    return
                }
                bleLog("[$address] TAP service found")

                val dataChar = service.getCharacteristic(TAP_DATA_CHAR_UUID)
                if (dataChar == null) {
                    bleLog("[$address] TAP_DATA char NOT found — disconnecting")
                    gatt.disconnect()
                    return
                }
                bleLog("[$address] TAP_DATA char found, enabling notifications")

                // Enable notifications on the data characteristic.
                gatt.setCharacteristicNotification(dataChar, true)
                val descriptor = dataChar.getDescriptor(CCCD_UUID)
                if (descriptor != null) {
                    bleLog("[$address] writing CCCD to enable notifications")
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        gatt.writeDescriptor(
                            descriptor,
                            BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE,
                        )
                    } else {
                        @Suppress("DEPRECATION")
                        descriptor.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                        @Suppress("DEPRECATION")
                        gatt.writeDescriptor(descriptor)
                    }
                } else {
                    bleLog("[$address] no CCCD, going straight to controller mode")
                    enterControllerMode(gatt, address)
                }
            }

            override fun onDescriptorWrite(
                gatt: BluetoothGatt,
                descriptor: BluetoothGattDescriptor,
                status: Int,
            ) {
                bleLog("[$address] onDescriptorWrite uuid=${descriptor.uuid} status=$status")
                if (descriptor.uuid == CCCD_UUID) {
                    if (status == BluetoothGatt.GATT_SUCCESS) {
                        bleLog("[$address] CCCD write OK, entering controller mode")
                        enterControllerMode(gatt, address)
                    } else {
                        bleLog("[$address] CCCD write FAILED status=$status — disconnecting")
                        gatt.disconnect()
                    }
                }
            }

            @Deprecated("Used on API < 33")
            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
            ) {
                if (characteristic.uuid == TAP_DATA_CHAR_UUID) {
                    @Suppress("DEPRECATION")
                    onTapBytes(address, characteristic.value ?: ByteArray(0))
                }
            }

            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                value: ByteArray,
            ) {
                if (characteristic.uuid == TAP_DATA_CHAR_UUID) {
                    onTapBytes(address, value)
                }
            }

        }
    }

    // ── GATT helpers ──────────────────────────────────────────────────────────

    @SuppressLint("MissingPermission")
    private fun enterControllerMode(gatt: BluetoothGatt, address: String) {
        bleLog("[$address] enterControllerMode: looking for NUS service $NUS_SERVICE_UUID")
        val nusService = gatt.getService(NUS_SERVICE_UUID)
        if (nusService == null) {
            val available = gatt.services.map { it.uuid.toString() }
            bleLog("[$address] NUS service NOT found. Available: $available — disconnecting")
            gatt.disconnect()
            return
        }
        bleLog("[$address] NUS service found, looking for RX char $NUS_RX_UUID")
        val nusRx = nusService.getCharacteristic(NUS_RX_UUID)
        if (nusRx == null) {
            val chars = nusService.characteristics.map { it.uuid.toString() }
            bleLog("[$address] NUS_RX NOT found. NUS chars: $chars — disconnecting")
            gatt.disconnect()
            return
        }
        bleLog("[$address] NUS_RX found, writing controller mode cmd with WRITE_TYPE_NO_RESPONSE")

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            gatt.writeCharacteristic(
                nusRx,
                CONTROLLER_MODE_CMD,
                BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE,
            )
        } else {
            @Suppress("DEPRECATION")
            nusRx.value = CONTROLLER_MODE_CMD
            @Suppress("DEPRECATION")
            nusRx.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
            @Suppress("DEPRECATION")
            gatt.writeCharacteristic(nusRx)
        }

        // NUS_RX uses write-without-response — no onCharacteristicWrite callback fires.
        // The command is sent; register the device as connected immediately.
        connections[address] = gatt
        reconnectAttempts[address] = 0

        requestConnectionPriority(gatt)

        val deviceName = try {
            gatt.device.name ?: address
        } catch (_: SecurityException) {
            address
        }
        val payload = JSObject().apply {
            put("address", address)
            put("name", deviceName)
        }
        bleLog("[$address] triggering ble-device-connected, hasListener=${hasListener("ble-device-connected")}")
        trigger("ble-device-connected", payload)
        bleLog("[$address] Tap device ready — controller mode entered")

        MapxrForegroundService.start(activity, connections.size, currentProfileName)

        // The Tap device exits controller mode if it doesn't receive the keepalive
        // command within 10 seconds. Schedule a repeating native timer to re-send it
        // every 8 s — runs entirely on the Android main thread, independent of WebView.
        scheduleKeepalive(address)
    }

    /**
     * Schedule a repeating keepalive that re-sends the controller mode command every
     * [KEEPALIVE_INTERVAL_MS] milliseconds.
     *
     * Runs on the Android main thread via [mainHandler] — independent of WebView/JS
     * performance, so it fires reliably in both dev and production builds.
     */
    @SuppressLint("MissingPermission")
    private fun scheduleKeepalive(address: String) {
        cancelKeepalive(address) // cancel any existing runnable for this address
        val runnable = object : Runnable {
            override fun run() {
                val gatt = connections[address] ?: return // device disconnected
                val nusRx = gatt.getService(NUS_SERVICE_UUID)
                    ?.getCharacteristic(NUS_RX_UUID)
                    ?: return // service gone
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    gatt.writeCharacteristic(nusRx, CONTROLLER_MODE_CMD,
                        BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE)
                } else {
                    @Suppress("DEPRECATION")
                    nusRx.value = CONTROLLER_MODE_CMD
                    @Suppress("DEPRECATION")
                    nusRx.writeType = BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
                    @Suppress("DEPRECATION")
                    gatt.writeCharacteristic(nusRx)
                }
                mainHandler.postDelayed(this, KEEPALIVE_INTERVAL_MS)
            }
        }
        keepaliveRunnables[address] = runnable
        mainHandler.postDelayed(runnable, KEEPALIVE_INTERVAL_MS)
    }

    /** Cancel any pending keepalive runnable for [address]. */
    private fun cancelKeepalive(address: String) {
        keepaliveRunnables.remove(address)?.let { mainHandler.removeCallbacks(it) }
    }

    /** Request CONNECTION_PRIORITY_HIGH to reduce BLE connection interval to ~7.5–15 ms. */
    @SuppressLint("MissingPermission")
    private fun requestConnectionPriority(gatt: BluetoothGatt) {
        gatt.requestConnectionPriority(BluetoothGatt.CONNECTION_PRIORITY_HIGH)
    }

    /**
     * Refresh the GATT service cache for a device via reflection.
     *
     * This avoids stale service discovery results on OEM builds (Samsung, Xiaomi, etc.)
     * that cache GATT attributes aggressively.  The `refresh()` method is hidden but
     * present on all Android builds since API 21.
     */
    private fun refreshGattCache(gatt: BluetoothGatt) {
        try {
            val refresh = gatt.javaClass.getMethod("refresh")
            val result = refresh.invoke(gatt) as? Boolean
            Log.d(TAG, "GATT cache refresh: $result")
        } catch (e: Exception) {
            Log.w(TAG, "GATT cache refresh failed (non-fatal): ${e.message}")
        }
    }

    /** Schedule a reconnect attempt for a device address. */
    @SuppressLint("MissingPermission")
    private fun scheduleReconnect(address: String) {
        val adapter = bluetoothAdapter ?: return
        val device = try {
            adapter.getRemoteDevice(address)
        } catch (_: IllegalArgumentException) {
            return
        }
        device.connectGatt(
            activity,
            /* autoConnect= */ false,
            buildGattCallback(address),
            BluetoothDevice.TRANSPORT_LE,
        )
    }

    // ── Tap data dispatch (task 15.5) ─────────────────────────────────────────

    /**
     * Forward a raw tap byte array from the Tap Strap to the Rust engine via the
     * `process_tap_event` Tauri command.
     *
     * The packet format matches `crates/tap-ble/src/packet_parser.rs`:
     *   bytes[0] = tap_code bitmask  (u8)
     *   bytes[1..2] = little-endian interval_ms (u16)
     *
     * The Rust side pushes the event through `ComboEngine` and emits resolved
     * `tap-actions-fired` events back to the WebView / Kotlin plugins.
     */
    private fun onTapBytes(address: String, bytes: ByteArray) {
        if (bytes.isEmpty()) return

        // Convert raw bytes to a JS-safe integer array so they survive the invoke bridge.
        val jsArray = app.tauri.plugin.JSArray()
        for (b in bytes) {
            jsArray.put(b.toInt() and 0xFF)
        }

        val payload = JSObject().apply {
            put("address", address)
            put("bytes", jsArray)
        }
        trigger("tap-bytes-received", payload)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /**
     * Disconnect all currently connected devices.
     *
     * Used by [stopForegroundService] and by [onUserStopRequested] (notification
     * "Stop" action) to cleanly tear down GATT connections before stopping the
     * foreground service.
     */
    @SuppressLint("MissingPermission")
    private fun disconnectAll() {
        val addresses = connections.keys.toList()
        for (address in addresses) {
            userDisconnected.add(address)
            reconnectAttempts.remove(address)
            cancelKeepalive(address)
            connections[address]?.let { gatt ->
                gatt.disconnect()
                gatt.close()
            }
            connections.remove(address)
            val payload = JSObject().apply {
                put("address", address)
                put("reason", "user_request")
            }
            trigger("ble-device-disconnected", payload)
        }
    }

    // ── Companion object ──────────────────────────────────────────────────────

    companion object {
        // Weak reference to the active plugin instance so the foreground service
        // can notify BlePlugin when the user taps "Stop" in the notification.
        @Volatile
        private var instance: BlePlugin? = null

        /**
         * Called by [MapxrForegroundService] when the user taps the "Stop"
         * notification action.
         *
         * Disconnects all devices and stops the service.
         */
        @SuppressLint("MissingPermission")
        fun onUserStopRequested() {
            val plugin = instance ?: return
            plugin.disconnectAll()
            MapxrForegroundService.stop(plugin.activity)
        }
    }
}
