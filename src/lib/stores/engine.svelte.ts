import { getEngineState } from "../commands";
import { deviceStore } from "./device.svelte";
import type { EngineStateSnapshot, LayerChangedPayload, VariableValue } from "../types";

class EngineStore {
  /** Layer IDs from bottom (base) to top (active). */
  layerStack = $state<string[]>([]);
  /** layer_id of the currently active (top) layer. */
  activeLayerId = $state<string>("");
  /** Current variable values on the top layer. */
  variables = $state<Record<string, VariableValue>>({});
  /** Whether debug event emission is currently enabled. */
  debugMode = $state(false);

  /** Fetch current state from the backend and apply it. Call once on app mount. */
  async init(): Promise<void> {
    const snap = await getEngineState();
    this.applySnapshot(snap);
  }

  applySnapshot(snap: EngineStateSnapshot): void {
    this.layerStack = snap.layer_stack;
    this.activeLayerId = snap.active_layer_id;
    this.variables = snap.variables;
    this.debugMode = snap.debug_mode;
    // Seed deviceStore with any devices that connected before the frontend
    // registered its event listeners (e.g. auto-reconnect on startup).
    for (const device of snap.connected_devices) {
      deviceStore.onConnected(device);
    }
  }

  /** Called when a `layer-changed` event is received. */
  applyLayerChanged(payload: LayerChangedPayload): void {
    this.layerStack = payload.stack;
    this.activeLayerId = payload.active;
  }
}

export const engineStore = new EngineStore();
