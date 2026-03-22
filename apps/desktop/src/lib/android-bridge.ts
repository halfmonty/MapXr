/**
 * Android bridge — proxies events between the Kotlin plugins and the Rust backend.
 *
 * Architecture:
 *
 *   Kotlin BlePlugin
 *     └─ trigger("tap-bytes-received", { address, bytes })
 *   WebView JS (this file)
 *     └─ addPluginListener("ble", "tap-bytes-received") → invoke("process_tap_event", ...)
 *   Rust android_pump
 *     └─ ComboEngine → app.emit("tap-actions-fired", { actions })
 *   WebView JS (this file)
 *     └─ listen("tap-actions-fired") → invoke("plugin:accessibility|dispatchActions", ...)
 *   Kotlin AccessibilityPlugin
 *     └─ dispatchKeyEvent / injectText
 *
 * Call `startAndroidBridge()` once from the app root on Android.
 */

import { listen } from "@tauri-apps/api/event";
import { invoke, addPluginListener } from "@tauri-apps/api/core";
import type { Action, BleDeviceConnectedPayload, BleDeviceDisconnectedPayload } from "./types";
import { logger } from "./logger";

interface TapBytesPayload {
  address: string;
  bytes: number[];
}

interface TapActionsFiredPayload {
  actions: Action[];
}

/** Begin forwarding tap events and actions. Returns a combined cleanup function. */
export async function startAndroidBridge(): Promise<() => void> {
  // Kotlin BlePlugin events use plugin.trigger() — must use addPluginListener(), not listen().
  // Rust events (tap-actions-fired) use app.emit() — must use listen().

  const bytesListener = await addPluginListener<TapBytesPayload>(
    "ble",
    "tap-bytes-received",
    async ({ address, bytes }) => {
      try {
        await invoke("process_tap_event", { address, bytes });
      } catch (err) {
        logger.warn(`android-bridge: process_tap_event failed: ${err}`);
      }
    },
  );

  // tap-actions-fired is emitted by Rust (app.emit()), so listen() is correct here.
  const unlistenActions = await listen<TapActionsFiredPayload>(
    "tap-actions-fired",
    async (event) => {
      const { actions } = event.payload;
      if (actions.length === 0) return;
      try {
        await invoke("plugin:accessibility|dispatchActions", { actions });
      } catch (err) {
        logger.warn(`android-bridge: dispatchActions failed: ${err}`);
      }
    },
  );

  // When BlePlugin reports a GATT connection complete, notify the Rust engine.
  // Rust will emit `device-connected` (if role is persisted) or `ble-device-pending`.
  const connectedListener = await addPluginListener<BleDeviceConnectedPayload>(
    "ble",
    "ble-device-connected",
    async (payload) => {
      try {
        await invoke("notify_android_device_connected", {
          address: payload.address,
          name: payload.name ?? null,
        });
      } catch (err) {
        logger.warn(`android-bridge: notify_android_device_connected failed: ${err}`);
      }
    },
  );

  // When BlePlugin reports a GATT disconnection, notify the Rust engine so it
  // can emit `device-disconnected` and update the deviceStore.
  const disconnectedListener = await addPluginListener<BleDeviceDisconnectedPayload>(
    "ble",
    "ble-device-disconnected",
    async (payload) => {
      try {
        await invoke("notify_android_device_disconnected", { address: payload.address });
      } catch (err) {
        logger.warn(`android-bridge: notify_android_device_disconnected failed: ${err}`);
      }
    },
  );

  return () => {
    void bytesListener.unregister();
    unlistenActions();
    void connectedListener.unregister();
    void disconnectedListener.unregister();
  };
}
