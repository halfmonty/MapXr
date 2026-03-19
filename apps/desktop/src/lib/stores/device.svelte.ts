import type { DeviceStatusPayload } from "../types";

const NAMES_STORAGE_KEY = "mapxr:device-names";

interface ConnectedDevice {
  role: string;
  address: string;
  name: string | null;
}

class DeviceStore {
  connected = $state<ConnectedDevice[]>([]);

  /** address → name, persisted to localStorage so names survive app restarts
   *  and are available when the reconnect loop fires device-connected events
   *  without any UI interaction. */
  private _names: Map<string, string | null> = this._loadNames();

  private _loadNames(): Map<string, string | null> {
    try {
      const raw = localStorage.getItem(NAMES_STORAGE_KEY);
      if (raw) return new Map(JSON.parse(raw));
    } catch {
      // Corrupt or unavailable storage — start fresh.
    }
    return new Map();
  }

  private _saveNames(): void {
    try {
      localStorage.setItem(NAMES_STORAGE_KEY, JSON.stringify([...this._names]));
    } catch {
      // Storage unavailable — ignore.
    }
  }

  /** Called when a `device-connected` event is received. */
  onConnected(payload: DeviceStatusPayload): void {
    const name = this._names.get(payload.address) ?? null;
    // Replace any existing entry for this role so reconnects update cleanly.
    this.connected = [
      ...this.connected.filter((d) => d.role !== payload.role),
      { role: payload.role, address: payload.address, name },
    ];
  }

  /** Called when a `device-disconnected` event is received. */
  onDisconnected(payload: DeviceStatusPayload): void {
    this.connected = this.connected.filter((d) => d.role !== payload.role);
    // Intentionally keep _names so reconnects still display the name.
  }

  /** Store the human-readable name for `address`.
   *
   *  Call this from the connect flow (where the scan result is available)
   *  before or after `onConnected` fires — both orderings are handled.
   *  The name is persisted to localStorage for use across app restarts. */
  setName(address: string, name: string | null): void {
    this._names.set(address, name);
    this._saveNames();
    // Patch any already-present entry so the UI updates immediately.
    const idx = this.connected.findIndex((d) => d.address === address);
    if (idx !== -1) {
      this.connected = this.connected.map((d) =>
        d.address === address ? { ...d, name } : d,
      );
    }
  }

  /** Returns true if a device is currently connected under `role`. */
  isConnected(role: string): boolean {
    return this.connected.some((d) => d.role === role);
  }
}

export const deviceStore = new DeviceStore();
