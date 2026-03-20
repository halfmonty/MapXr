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
  ContextRules,
  TrayPreferences,
  UpdateInfo,
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
 * Reassign the connected device at `address` to `newRole` without disconnecting.
 *
 * The BLE connection is preserved; only the logical role changes. The frontend
 * will receive a `device-disconnected` event for the old role followed by a
 * `device-connected` event for the new role.
 *
 * @param address - BLE address in "AA:BB:CC:DD:EE:FF" format.
 * @param newRole - One of "solo", "left", or "right".
 * @throws If BLE is unavailable, the address is not currently connected, or the role is invalid.
 */
export async function reassignDeviceRole(
  address: string,
  newRole: string,
): Promise<void> {
  return invoke("reassign_device_role", { address, newRole });
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

/**
 * Rename a connected Tap device.
 *
 * The name is written directly to the device over BLE and persists across
 * power cycles. The change takes effect after the device reconnects and
 * re-advertises — inform the user of this after a successful call.
 *
 * @param address - BLE address in "AA:BB:CC:DD:EE:FF" format.
 * @param name    - New friendly name (1–20 printable ASCII chars; leading/trailing
 *                  whitespace is trimmed automatically by the backend).
 * @throws If the device is not connected, the name fails validation, or the BLE write fails.
 */
export async function renameDevice(address: string, name: string): Promise<void> {
  return invoke("rename_device", { address, name });
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

// ── Context rules commands ────────────────────────────────────────────────────

/**
 * Return the current context-switching rules.
 *
 * Rules are evaluated in list order; the first match activates the associated
 * profile. Matching is case-insensitive substring search on app name and/or
 * window title.
 */
export async function listContextRules(): Promise<ContextRules> {
  return invoke("list_context_rules");
}

/**
 * Validate, persist, and replace the context-switching rules.
 *
 * The backend validates all rules before writing. On success the focus monitor
 * immediately starts using the new rules.
 *
 * @param rules - The full replacement rule list (version + rules array).
 * @throws If any rule fails validation or the file cannot be written.
 */
export async function saveContextRules(rules: ContextRules): Promise<void> {
  return invoke("save_context_rules", { rules });
}

// ── Preferences commands ──────────────────────────────────────────────────────

/**
 * Return the current tray-related preferences.
 */
export async function getPreferences(): Promise<TrayPreferences> {
  return invoke("get_preferences");
}

/**
 * Persist updated tray preferences and apply live effects.
 *
 * `start_at_login` takes effect immediately (registers/deregisters the OS login
 * item). Other settings are applied the next time they become relevant.
 *
 * @param prefs - The full replacement preferences object.
 * @throws If the OS login item cannot be registered/deregistered.
 */
export async function savePreferences(prefs: TrayPreferences): Promise<void> {
  return invoke("save_preferences", { prefsUpdate: prefs });
}

// ── Update commands ───────────────────────────────────────────────────────────

/**
 * Query the update endpoint and return info about the available update, or
 * `null` if the app is already on the latest version.
 *
 * Does not download anything. Call `downloadAndInstallUpdate` to apply the update.
 *
 * @throws If the update endpoint is unreachable or the check fails.
 */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  return invoke("check_for_update");
}

/**
 * Download and install the latest available update, then restart the app.
 *
 * Emits `update-download-progress` events during the download. This call never
 * resolves on success because the app restarts; it rejects with an error string
 * if the download or installation fails.
 *
 * @throws If no update is available or the download fails.
 */
export async function downloadAndInstallUpdate(): Promise<void> {
  return invoke("download_and_install_update");
}

// ── Filesystem helpers ────────────────────────────────────────────────────────

/**
 * Read a file at an absolute path and return its text content.
 *
 * Used for drag-and-drop imports on Linux/WebKitGTK where the WebView receives
 * a `file://` URI instead of a `File` object in `dataTransfer.files`.
 *
 * @throws If the file cannot be read or is not valid UTF-8.
 */
export async function readFileText(path: string): Promise<string> {
  return invoke("read_file_text", { path });
}
