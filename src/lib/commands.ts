/**
 * Typed wrappers around all Tauri `invoke` calls.
 *
 * Every function maps 1-to-1 to a `#[tauri::command]` in `src-tauri/src/commands.rs`.
 * Errors from the Rust side arrive as thrown strings (Tauri serialises `Err(String)`
 * as a rejected promise).
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  TapDeviceInfo,
  ProfileSummary,
  Profile,
  PushLayerMode,
  EngineStateSnapshot,
} from "./types";

// ── Device commands ───────────────────────────────────────────────────────────

/**
 * Scan for nearby Tap BLE devices for ~5 seconds.
 *
 * @throws If no Bluetooth adapter is available or the scan fails.
 */
export async function scanDevices(): Promise<TapDeviceInfo[]> {
  return invoke("scan_devices");
}

/**
 * Connect to the Tap device at `address` and assign it `role`.
 *
 * @param address - BLE address in "AA:BB:CC:DD:EE:FF" format.
 * @param role    - One of "solo", "left", or "right".
 * @throws If the address is malformed, the role is invalid, or the connection fails.
 */
export async function connectDevice(
  address: string,
  role: string,
): Promise<void> {
  return invoke("connect_device", { address, role });
}

/**
 * Disconnect the device assigned to `role`.
 *
 * Returns successfully even if no device is connected under that role.
 *
 * @throws If BLE is unavailable or the disconnect call fails.
 */
export async function disconnectDevice(role: string): Promise<void> {
  return invoke("disconnect_device", { role });
}

// ── Profile commands ──────────────────────────────────────────────────────────

/**
 * List all profiles in the profiles directory.
 *
 * Triggers a registry reload so newly added files are included. Any file that
 * fails to load emits a `profile-error` Tauri event rather than throwing here.
 */
export async function listProfiles(): Promise<ProfileSummary[]> {
  return invoke("list_profiles");
}

/**
 * Load the full profile for `layerId`.
 *
 * @throws If no profile with that layer_id exists.
 */
export async function loadProfile(layerId: string): Promise<Profile> {
  const profile = await invoke<Profile>("load_profile", { layerId });
  profile.aliases ??= {};
  profile.variables ??= {};
  return profile;
}

/**
 * Write `profile` to disk and reload the registry.
 *
 * @throws If the profile fails validation or the file cannot be written.
 */
export async function saveProfile(profile: Profile): Promise<void> {
  return invoke("save_profile", { profile });
}

/**
 * Delete the profile file for `layerId`.
 *
 * @throws If the file does not exist or cannot be removed.
 */
export async function deleteProfile(layerId: string): Promise<void> {
  return invoke("delete_profile", { layerId });
}

// ── Engine commands ───────────────────────────────────────────────────────────

/**
 * Replace the engine's base layer with the profile identified by `layerId`.
 *
 * Clears all pending state and emits a `layer-changed` event.
 *
 * @throws If no profile with that layer_id exists.
 */
export async function activateProfile(layerId: string): Promise<void> {
  return invoke("activate_profile", { layerId });
}

/**
 * Reset the engine to the built-in default, leaving no user profile active.
 *
 * After this call `getEngineState` returns `activeLayerId === "default"`,
 * which matches no profile in the registry so the UI shows none as active.
 */
export async function deactivateProfile(): Promise<void> {
  return invoke("deactivate_profile");
}

/**
 * Push the profile `layerId` onto the engine's layer stack.
 *
 * @param mode - How long the pushed layer stays on the stack.
 * @throws If no profile with that layer_id exists.
 */
export async function pushLayer(
  layerId: string,
  mode: PushLayerMode,
): Promise<void> {
  return invoke("push_layer", { layerId, mode });
}

/**
 * Pop the top layer off the engine's layer stack.
 *
 * @throws If the stack is already at the base layer.
 */
export async function popLayer(): Promise<void> {
  return invoke("pop_layer");
}

/**
 * Enable or disable debug event emission from the engine.
 */
export async function setDebugMode(enabled: boolean): Promise<void> {
  return invoke("set_debug_mode", { enabled });
}

/**
 * Return a snapshot of the current engine state.
 *
 * Includes the layer stack, active layer, variable values, connected device
 * roles, and whether debug mode is on.
 */
export async function getEngineState(): Promise<EngineStateSnapshot> {
  return invoke("get_engine_state");
}
