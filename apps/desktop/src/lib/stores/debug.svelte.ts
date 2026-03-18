import type { TapEventPayload, ActionFiredPayload, DebugEvent } from "../types";

/** Maximum number of debug events kept in the rolling buffer. */
const MAX_DEBUG_EVENTS = 200;

/** Live tap state tracked per device role. */
export interface DeviceTapState {
  tapCode: number;
  receivedAtMs: number;
  /** True for ~500ms after the tap arrives; cleared by a setTimeout. */
  flash: boolean;
}

class DebugStore {
  /** Whether the debug panel is enabled in the UI. */
  enabled = $state(false);
  /** Rolling buffer of debug events, newest first. */
  debugEvents = $state<DebugEvent[]>([]);
  /** Most recently received tap event (for the live visualiser). */
  lastTap = $state<TapEventPayload | null>(null);
  /** Most recently fired action. */
  lastAction = $state<ActionFiredPayload | null>(null);
  /** Per-device last tap state, keyed by device role ("solo", "left", "right"). */
  lastTapByRole = $state<Record<string, DeviceTapState>>({});

  /** Called when a `tap-event` event is received. */
  recordTap(payload: TapEventPayload): void {
    this.lastTap = payload;
    this.lastTapByRole = {
      ...this.lastTapByRole,
      [payload.device_id]: {
        tapCode: payload.tap_code,
        receivedAtMs: payload.received_at_ms,
        flash: true,
      },
    };
    setTimeout(() => {
      const current = this.lastTapByRole[payload.device_id];
      if (current) {
        this.lastTapByRole = {
          ...this.lastTapByRole,
          [payload.device_id]: { ...current, flash: false },
        };
      }
    }, 500);
  }

  /** Called when an `action-fired` event is received. */
  recordAction(payload: ActionFiredPayload): void {
    this.lastAction = payload;
  }

  /** Called when a `debug-event` event is received. Prepends to the buffer. */
  appendDebugEvent(event: DebugEvent): void {
    this.debugEvents = [event, ...this.debugEvents].slice(0, MAX_DEBUG_EVENTS);
  }

  /** Clear the debug event buffer. */
  clear(): void {
    this.debugEvents = [];
  }
}

export const debugStore = new DebugStore();
