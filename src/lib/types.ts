/**
 * TypeScript mirrors of all Rust types that cross the Tauri command boundary.
 *
 * Serde conventions used on the Rust side:
 *   - Action, Trigger: internally tagged with `"type"` key, snake_case variants
 *   - PushLayerMode: internally tagged with `"mode"` key, flattened into PushLayer
 *   - VariableValue: untagged (serialises as plain `boolean` or `number`)
 *   - ProfileKind, Hand, Modifier, OverloadStrategy: plain lowercase strings
 *   - TapStep: serialises as a plain finger-pattern string, not an object
 *   - KeyDef: transparent newtype over string
 *   - ProfileSettings fields: all optional (omitted from JSON when null/default)
 *   - Mapping.enabled: omitted when true (default); present as false when disabled
 *   - Profile.passthrough: omitted when false (default); present as true when set
 */

// ── Primitives ───────────────────────────────────────────────────────────────

/** BLE hardware address in "AA:BB:CC:DD:EE:FF" format. */
export type BleAddress = string;

/** Finger-pattern string: "xoooo" (single) or "oooox xoooo" (dual). */
export type FingerPattern = string;

/** A validated key name string, e.g. "a", "ctrl", "F1", "space". */
export type KeyName = string;

// ── Device ───────────────────────────────────────────────────────────────────

/** A Tap device discovered during a BLE scan. */
export interface TapDeviceInfo {
  name: string | null;
  /** "AA:BB:CC:DD:EE:FF" */
  address: BleAddress;
  rssi: number | null;
}

/** Payload for the `device-connected` / `device-disconnected` Tauri events. */
export interface DeviceStatusPayload {
  /** "solo" | "left" | "right" */
  role: string;
  address: BleAddress;
}

export type DeviceRole = "solo" | "left" | "right";

// ── Profile summary ──────────────────────────────────────────────────────────

/** Lightweight summary returned by `list_profiles`. */
export interface ProfileSummary {
  layer_id: string;
  name: string;
  kind: ProfileKind;
  description: string | null;
}

// ── Enums ────────────────────────────────────────────────────────────────────

export type ProfileKind = "single" | "dual";
export type Hand = "left" | "right";
export type Modifier = "ctrl" | "shift" | "alt" | "meta";
export type OverloadStrategy = "patient" | "eager";

// ── Variable values ──────────────────────────────────────────────────────────

/**
 * VariableValue serialises as an untagged JSON value:
 *   Bool(true)  → true
 *   Bool(false) → false
 *   Int(42)     → 42
 */
export type VariableValue = boolean | number;

// ── PushLayerMode ─────────────────────────────────────────────────────────────

/**
 * When passed as a standalone parameter to the `push_layer` command, PushLayerMode
 * serialises with `"mode"` as the discriminant key:
 *   { "mode": "permanent" }
 *   { "mode": "count", "count": 3 }
 *   { "mode": "timeout", "timeout_ms": 2000 }
 */
export type PushLayerMode =
  | { mode: "permanent" }
  | { mode: "count"; count: number }
  | { mode: "timeout"; timeout_ms: number };

// ── HoldModifierMode ──────────────────────────────────────────────────────────

/**
 * Controls how long a sticky modifier stays active. Serialised with `"mode"` as the
 * discriminant key, flattened into the parent `hold_modifier` action object:
 *   { ..., "mode": "toggle" }
 *   { ..., "mode": "count",   "count": 1 }
 *   { ..., "mode": "timeout", "timeout_ms": 2000 }
 */
export type HoldModifierMode =
  | { mode: "toggle" }
  | { mode: "count"; count: number }
  | { mode: "timeout"; timeout_ms: number };

// ── Action ───────────────────────────────────────────────────────────────────

/**
 * Action enum, internally tagged with `"type"` key.
 *
 * PushLayer note: PushLayerMode is `#[serde(flatten)]`ed into PushLayer, so the
 * `mode` discriminant and any extra fields appear directly on the action object:
 *   { "type": "push_layer", "layer": "nav", "mode": "permanent" }
 *   { "type": "push_layer", "layer": "nav", "mode": "count", "count": 3 }
 *   { "type": "push_layer", "layer": "nav", "mode": "timeout", "timeout_ms": 2000 }
 *
 * Key note: `modifiers` is omitted from JSON when empty; use `modifiers?: Modifier[]`.
 */
export type Action =
  | { type: "key"; key: KeyName; modifiers?: Modifier[] }
  | { type: "key_chord"; keys: string[] }
  | { type: "type_string"; text: string }
  | { type: "macro"; steps: MacroStep[] }
  | { type: "push_layer"; layer: string; mode: "permanent" }
  | { type: "push_layer"; layer: string; mode: "count"; count: number }
  | { type: "push_layer"; layer: string; mode: "timeout"; timeout_ms: number }
  | { type: "pop_layer" }
  | { type: "switch_layer"; layer: string }
  | {
      type: "toggle_variable";
      variable: string;
      on_true: Action;
      on_false: Action;
    }
  | { type: "set_variable"; variable: string; value: VariableValue }
  | { type: "block" }
  | { type: "alias"; name: string }
  | ({ type: "hold_modifier"; modifiers: Modifier[] } & HoldModifierMode);

/** A single step inside a `macro` action. */
export interface MacroStep {
  action: Action;
  /** Milliseconds to wait after this action fires before the next step. */
  delay_ms: number;
}

// ── Trigger ──────────────────────────────────────────────────────────────────

/**
 * Trigger enum, internally tagged with `"type"` key.
 *
 * TapStep note: sequence steps serialise as plain finger-pattern strings, not
 * objects. So `steps` is `FingerPattern[]`, not `TapStep[]`.
 *   { "type": "sequence", "steps": ["xoooo", "ooxoo"], "window_ms": 400 }
 */
export type Trigger =
  | { type: "tap"; code: FingerPattern }
  | { type: "double_tap"; code: FingerPattern }
  | { type: "triple_tap"; code: FingerPattern }
  | {
      type: "sequence";
      steps: FingerPattern[];
      /** Per-trigger window override (ms). Omitted when null. */
      window_ms?: number;
    };

// ── Mapping ──────────────────────────────────────────────────────────────────

/**
 * A single mapping entry in a profile.
 *
 * `enabled` is omitted from JSON when `true` (the default). When `false` it
 * is present. Treat `undefined` as `true` when reading incoming JSON.
 */
export interface Mapping {
  label: string;
  trigger: Trigger;
  action: Action;
  /** Omitted from JSON when true. Treat undefined as true. */
  enabled?: boolean;
}

// ── Profile settings ─────────────────────────────────────────────────────────

/**
 * All fields are optional — omitted from JSON when null/default.
 * The engine applies its own defaults for any absent field.
 */
export interface ProfileSettings {
  /** Cross-device chord detection window (ms). Dual profiles only. */
  combo_window_ms?: number;
  /** Max gap between sequence steps (ms). Per-trigger `window_ms` overrides this. */
  sequence_window_ms?: number;
  /** Max time between first and second tap of a double_tap (ms). */
  double_tap_window_ms?: number;
  /** Max time from first to third tap of a triple_tap (ms). */
  triple_tap_window_ms?: number;
  /** Required when any code is bound to both tap and double_tap/triple_tap. */
  overload_strategy?: OverloadStrategy;
  /**
   * Actions used to undo an eagerly-fired single-tap before firing the
   * double-tap action. Only relevant when `overload_strategy` is `"eager"`.
   */
  eager_undo_sequence?: Action[];
}

// ── Profile ──────────────────────────────────────────────────────────────────

/**
 * A full profile document, as returned by `load_profile` and accepted by
 * `save_profile`.
 *
 * Optional fields are omitted from JSON by the Rust side when at their defaults:
 *   - `description`: omitted when null
 *   - `hand`: omitted when null
 *   - `passthrough`: omitted when false (default)
 *   - `aliases`, `variables`: omitted when empty
 *   - `on_enter`, `on_exit`: omitted when null
 */
export interface Profile {
  version: number;
  kind: ProfileKind;
  name: string;
  description?: string;
  layer_id: string;
  hand?: Hand;
  /** Omitted from JSON when false. Treat undefined as false. */
  passthrough?: boolean;
  settings: ProfileSettings;
  aliases: Record<string, Action>;
  variables: Record<string, VariableValue>;
  on_enter?: Action;
  on_exit?: Action;
  mappings: Mapping[];
}

// ── Engine state ─────────────────────────────────────────────────────────────

/** Snapshot returned by the `get_engine_state` command. */
export interface EngineStateSnapshot {
  /** Layer IDs from bottom (base) to top (active). */
  layer_stack: string[];
  /** layer_id of the currently active (top) layer. */
  active_layer_id: string;
  /**
   * Current variable values on the top layer, serialised as JSON values.
   * VariableValue::Bool(b) → boolean, VariableValue::Int(n) → number.
   */
  variables: Record<string, VariableValue>;
  /** Currently connected BLE devices with their roles and addresses. */
  connected_devices: DeviceStatusPayload[];
  /** Whether debug mode is currently enabled. */
  debug_mode: boolean;
}

// ── Tauri event payloads ──────────────────────────────────────────────────────

/** Payload for the `tap-event` Tauri event. */
export interface TapEventPayload {
  device_id: string;
  tap_code: number;
  /** Milliseconds since Unix epoch (for `new Date(received_at_ms)`). */
  received_at_ms: number;
}

/** Payload for the `action-fired` Tauri event. */
export interface ActionFiredPayload {
  /** Variant name of the Action enum, e.g. "key", "push_layer". */
  action_kind: string;
  /** Mapping label if available. Currently always null. */
  label: string | null;
}

/** Payload for the `layer-changed` Tauri event. */
export interface LayerChangedPayload {
  /** Layer IDs from bottom (base) to top (active). */
  stack: string[];
  /** layer_id of the currently active (top) layer. */
  active: string;
}

/** Payload for the `profile-error` Tauri event. */
export interface ProfileErrorPayload {
  file_name: string;
  message: string;
}

/**
 * Payload for the `debug-event` Tauri event.
 *
 * Discriminated union matching `DebugEvent` in Rust (internally tagged with
 * `"kind"`, `rename_all = "snake_case"`).
 */
export type DebugEvent =
  | {
      kind: "resolved";
      pattern: string;
      device: string;
      /** Layer IDs at resolution time, top first. */
      layer_stack: string[];
      matched_layer: string;
      matched_mapping: string;
      action_fired: Action;
      /** Milliseconds the engine held the event before resolving it. */
      waited_ms: number;
      /** The timing window (ms) that governed this resolution. */
      window_ms: number;
    }
  | {
      kind: "unmatched";
      pattern: string;
      device: string;
      /** Layer IDs checked via the passthrough walk, in order. */
      passthrough_layers_checked: string[];
    }
  | {
      kind: "combo_timeout";
      first_pattern: string;
      first_device: string;
      second_pattern: string;
      second_device: string;
      combo_window_ms: number;
      actual_gap_ms: number;
    };

// ── Key name list ─────────────────────────────────────────────────────────────

/**
 * All key names recognised by the Rust `name_to_key` function in `pump.rs`.
 * Used to populate autocomplete in the action editor.
 */
export const KNOWN_KEY_NAMES: readonly KeyName[] = [
  // Modifiers
  "ctrl", "shift", "alt", "meta",
  // Alphabet
  "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
  "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
  // Digits
  "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
  // Special keys
  "space", "return", "enter", "backspace", "tab", "escape", "delete",
  "home", "end", "page_up", "page_down",
  "left", "right", "up", "down",
  // Function keys
  "F1", "F2", "F3", "F4", "F5", "F6",
  "F7", "F8", "F9", "F10", "F11", "F12",
] as const;
