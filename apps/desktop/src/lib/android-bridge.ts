/**
 * Android bridge — wires Kotlin plugin events to the Rust backend and debug store.
 *
 * Architecture (after Epic 21):
 *
 *   Kotlin BlePlugin.onTapBytes()
 *     └─ NativeBridge.processTapBytes() → Rust pump → ShizukuDispatcher  [native, always]
 *     └─ trigger("tap-bytes-received")  → WebView JS (UI only, best-effort)
 *   WebView JS (this file)
 *     └─ addPluginListener("ble", "tap-bytes-received") → debug store (finger visualiser)
 *   Rust android_pump
 *     └─ app.emit("tap-actions-fired", { actions }) → WebView JS (debug panel only)
 *   WebView JS (this file)
 *     └─ listen("tap-actions-fired") → debug store (event log)
 *
 * The WebView path does not invoke `process_tap_event` or `dispatch`.
 * Both roles are handled by the JNI native path, which runs even when the WebView
 * is backgrounded and suspended.
 *
 * Call `startAndroidBridge()` once from the app root on Android.
 */

import { listen } from "@tauri-apps/api/event";
import { invoke, addPluginListener } from "@tauri-apps/api/core";
import type { Action, BleDeviceConnectedPayload, BleDeviceDisconnectedPayload } from "./types";
import { logger } from "./logger";
import { debugStore } from "./stores/debug.svelte";

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

  // tap-bytes-received: UI update only. The native JNI path (NativeBridge.processTapBytes)
  // has already fed these bytes into the Rust engine — do NOT invoke process_tap_event here
  // as that would double-feed the engine.
  const bytesListener = await addPluginListener<TapBytesPayload>(
    "ble",
    "tap-bytes-received",
    ({ address, bytes }) => {
      logger.info(`android-bridge: tap-bytes-received address=${address} len=${bytes.length}`);
    },
  );

  // tap-actions-fired: debug panel only. The native JNI path has already dispatched the
  // actions to ShizukuDispatcher — do NOT invoke dispatch here as that would
  // double-dispatch when the app is foregrounded.
  const unlistenActions = await listen<TapActionsFiredPayload>(
    "tap-actions-fired",
    (event) => {
      const { actions } = event.payload;
      for (const action of actions) {
        debugStore.recordAction({ action_kind: action.type, label: null });
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
