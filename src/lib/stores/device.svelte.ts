import type { DeviceStatusPayload } from "../types";

interface ConnectedDevice {
  role: string;
  address: string;
}

class DeviceStore {
  connected = $state<ConnectedDevice[]>([]);

  /** Called when a `device-connected` event is received. */
  onConnected(payload: DeviceStatusPayload): void {
    // Replace any existing entry for this role so reconnects update cleanly.
    this.connected = [
      ...this.connected.filter((d) => d.role !== payload.role),
      { role: payload.role, address: payload.address },
    ];
  }

  /** Called when a `device-disconnected` event is received. */
  onDisconnected(payload: DeviceStatusPayload): void {
    this.connected = this.connected.filter((d) => d.role !== payload.role);
  }

  /** Returns true if a device is currently connected under `role`. */
  isConnected(role: string): boolean {
    return this.connected.some((d) => d.role === role);
  }
}

export const deviceStore = new DeviceStore();
