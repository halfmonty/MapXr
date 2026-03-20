/**
 * Typed wrappers around all Tauri `listen` subscriptions.
 *
 * Call `setupEventListeners()` once from the root layout on mount. The listeners
 * are never torn down — their lifetime equals the app's lifetime.
 */

import { listen } from "@tauri-apps/api/event";
import { deviceStore } from "./stores/device.svelte";
import { engineStore } from "./stores/engine.svelte";
import { profileStore } from "./stores/profile.svelte";
import { debugStore } from "./stores/debug.svelte";
import { updateStore } from "./stores/updates.svelte";
import { logger } from "./logger";
import type {
  TapEventPayload,
  ActionFiredPayload,
  LayerChangedPayload,
  DeviceStatusPayload,
  ProfileErrorPayload,
  DebugEvent,
  UpdateInfo,
  UpdateProgressPayload,
} from "./types";

/**
 * Register all Tauri event listeners.
 *
 * Returns a cleanup function that removes all listeners. Call once from the
 * root layout and invoke the returned function on unmount so that HMR
 * remounts don't accumulate duplicate listeners.
 */
export async function setupEventListeners(): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<TapEventPayload>("tap-event", ({ payload }) => {
      debugStore.recordTap(payload);
    }),
    listen<ActionFiredPayload>("action-fired", ({ payload }) => {
      debugStore.recordAction(payload);
    }),
    listen<LayerChangedPayload>("layer-changed", ({ payload }) => {
      engineStore.applyLayerChanged(payload);
    }),
    listen<DeviceStatusPayload>("device-connected", ({ payload }) => {
      deviceStore.onConnected(payload);
    }),
    listen<DeviceStatusPayload>("device-disconnected", ({ payload }) => {
      deviceStore.onDisconnected(payload);
    }),
    listen<DebugEvent>("debug-event", ({ payload }) => {
      debugStore.appendDebugEvent(payload);
    }),
    listen<ProfileErrorPayload>("profile-error", ({ payload }) => {
      logger.warn(`Profile load error: ${payload.file_name} — ${payload.message}`);
      profileStore.appendError(payload);
    }),
    listen<UpdateInfo>("update-available", ({ payload }) => {
      updateStore.setAvailable(payload);
    }),
    listen<UpdateProgressPayload>("update-download-progress", ({ payload }) => {
      updateStore.applyProgress(payload);
    }),
  ]);

  return () => unlisteners.forEach((u) => u());
}
