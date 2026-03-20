/**
 * TypeScript mirrors of all Rust types that cross the Tauri command boundary.
 *
 * Serde conventions used on the Rust side:
 *   - Action, Trigger: internally tagged with `"type"` key, snake_case variants
 *   - PushLayerMode: internally tagged with `"mode"` key, flattened into PushLayer
 *   - VariableValue: untagged (serialises as plain `boolean`; integer variant exists in engine but is not exposed in UI)
 *   - ProfileKind, Hand, Modifier: plain lowercase strings
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
  /**
   * `true` if the device was actively advertising during the scan window, or
   * is currently connected to this host.
   * `false` means the entry is from the OS Bluetooth cache — the device may
   * be off or out of range and should not be shown as connectable.
   */
  seen_in_scan: boolean;
  /**
   * `true` if the device currently has an active BLE connection to this host (the OS).
   * Its connection slot is occupied; our app cannot connect until the OS releases it.
   * The UI should show a distinct "Connected to OS" state and disable the connect action.
   */
  is_connected_to_os: boolean;
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
export type MouseButton = "left" | "right" | "middle";
export type ScrollDirection = "up" | "down" | "left" | "right";

// ── Variable values ──────────────────────────────────────────────────────────

/**
 * VariableValue serialises as an untagged JSON value:
 *   Bool(true)  → true
 *   Bool(false) → false
 *
 * Integer variables are supported by the engine but not exposed in the UI.
 */
export type VariableValue = boolean;

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
/**
 * Alternating on/off durations in milliseconds for haptic vibration.
 *
 * Mirrors `VibrationPattern(pub Vec<u16>)` in mapping-core, which serialises
 * as a plain JSON array: `[200, 100, 200]` = 200 ms on, 100 ms off, 200 ms on.
 *
 * Constraints (enforced at BLE send time, not in the UI):
 *   - Each value: 10–2550 ms, 10 ms resolution
 *   - Maximum 18 elements (longer sequences are truncated before sending)
 */
export type VibrationPattern = number[];

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
  | {
      type: "conditional";
      variable: string;
      on_true: Action;
      on_false: Action;
    }
  | { type: "block" }
  | { type: "alias"; name: string }
  | ({ type: "hold_modifier"; modifiers: Modifier[] } & HoldModifierMode)
  | { type: "mouse_click"; button: MouseButton }
  | { type: "mouse_double_click"; button: MouseButton }
  | { type: "mouse_scroll"; direction: ScrollDirection }
  | { type: "vibrate"; pattern: VibrationPattern };

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

/** Guards a mapping so it only fires when a variable matches a given value. */
export interface MappingCondition {
  variable: string;
  value: boolean;
}

/**
 * A single mapping entry in a profile.
 *
 * `enabled` is omitted from JSON when `true` (the default). When `false` it
 * is present. Treat `undefined` as `true` when reading incoming JSON.
 * `condition` is omitted when absent.
 */
export interface Mapping {
  label: string;
  trigger: Trigger;
  action: Action;
  /** Omitted from JSON when true. Treat undefined as true. */
  enabled?: boolean;
  /** Optional variable guard. Omitted when absent. */
  condition?: MappingCondition;
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
   * VariableValue::Bool(b) → boolean.
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

// ── Context rules ─────────────────────────────────────────────────────────────

/** A single rule that maps a window focus pattern to a profile layer_id. */
export interface ContextRule {
  /** Human-readable label shown in the UI. */
  name: string;
  /** layer_id of the profile to activate when this rule matches. */
  layer_id: string;
  /** Pattern matched against the application name. null = match any app. */
  match_app: string | null;
  /** Pattern matched against the window title. null = match any title. */
  match_title: string | null;
}

/** Ordered list of context rules, as returned by `list_context_rules`. */
export interface ContextRules {
  version: number;
  rules: ContextRule[];
}

/** Payload for the `context-rule-matched` Tauri event. */
export interface ContextRuleMatchedPayload {
  /** Human-readable label of the matched rule. */
  rule_name: string;
  /** layer_id of the profile that was activated. */
  layer_id: string;
}

// ── Preferences ──────────────────────────────────────────────────────────────

/** User preferences surfaced in the Settings page. */
export interface TrayPreferences {
  /** Hide window on close instead of quitting. */
  close_to_tray: boolean;
  /** Launch directly to tray without showing the main window. */
  start_minimised: boolean;
  /** Register the app to start automatically at OS login. */
  start_at_login: boolean;
  /** Notify when a Tap device connects. */
  notify_device_connected: boolean;
  /** Notify when a Tap device disconnects. */
  notify_device_disconnected: boolean;
  /** Notify when the active layer switches within a profile. */
  notify_layer_switch: boolean;
  /** Notify when the active profile switches. */
  notify_profile_switch: boolean;
  /** Master haptic toggle — gates all vibration. */
  haptics_enabled: boolean;
  /** Vibrate on every resolved tap event. */
  haptic_on_tap: boolean;
  /** Vibrate on layer push/pop/switch. */
  haptic_on_layer_switch: boolean;
  /** Vibrate on profile activate. */
  haptic_on_profile_switch: boolean;
}

// ── Updates ───────────────────────────────────────────────────────────────────

/** Info about an available update, returned by `check_for_update`. */
export interface UpdateInfo {
  /** Semver version string, e.g. `"1.2.0"`. */
  version: string;
  /** Markdown release notes from the update manifest, if present. */
  release_notes: string | null;
}

/** Payload for the `update-download-progress` Tauri event. */
export interface UpdateProgressPayload {
  /** Total bytes downloaded so far. */
  downloaded: number;
  /** Total download size in bytes, if the server sent a Content-Length header. */
  total: number | null;
}

// ── Key name list ─────────────────────────────────────────────────────────────

/**
 * A group of related keys shown together in the key-picker UI.
 * `platformNote` is shown as a tooltip when provided (e.g. "macOS only").
 */
export interface KeyGroup {
  label: string;
  keys: readonly { name: string; platformNote?: string }[];
}

/**
 * All key names recognised by `pump.rs` `name_to_key`, organised into display
 * groups for the action editor key picker. Matches `VALID_KEYS` in `key_def.rs`.
 *
 * Keys with a `platformNote` are valid profile values on all platforms but
 * only dispatch successfully on the noted platform.
 */
export const KEY_GROUPS: readonly KeyGroup[] = [
  {
    label: "Standard",
    keys: [
      { name: "a" }, { name: "b" }, { name: "c" }, { name: "d" }, { name: "e" },
      { name: "f" }, { name: "g" }, { name: "h" }, { name: "i" }, { name: "j" },
      { name: "k" }, { name: "l" }, { name: "m" }, { name: "n" }, { name: "o" },
      { name: "p" }, { name: "q" }, { name: "r" }, { name: "s" }, { name: "t" },
      { name: "u" }, { name: "v" }, { name: "w" }, { name: "x" }, { name: "y" },
      { name: "z" },
      { name: "0" }, { name: "1" }, { name: "2" }, { name: "3" }, { name: "4" },
      { name: "5" }, { name: "6" }, { name: "7" }, { name: "8" }, { name: "9" },
      { name: "grave" }, { name: "minus" }, { name: "equals" },
      { name: "left_bracket" }, { name: "right_bracket" }, { name: "backslash" },
      { name: "semicolon" }, { name: "quote" }, { name: "comma" },
      { name: "period" }, { name: "slash" },
    ],
  },
  {
    label: "Navigation",
    keys: [
      { name: "space" }, { name: "return" }, { name: "tab" },
      { name: "backspace" }, { name: "delete" }, { name: "escape" },
      { name: "left_arrow" }, { name: "right_arrow" },
      { name: "up_arrow" }, { name: "down_arrow" },
      { name: "home" }, { name: "end" },
      { name: "page_up" }, { name: "page_down" },
      { name: "caps_lock" },
      { name: "insert", platformNote: "Windows / Linux" },
      { name: "num_lock", platformNote: "Windows / Linux" },
      { name: "scroll_lock", platformNote: "Linux" },
      { name: "print_screen", platformNote: "Windows / Linux" },
    ],
  },
  {
    label: "Function",
    keys: [
      { name: "f1" }, { name: "f2" }, { name: "f3" }, { name: "f4" },
      { name: "f5" }, { name: "f6" }, { name: "f7" }, { name: "f8" },
      { name: "f9" }, { name: "f10" }, { name: "f11" }, { name: "f12" },
      { name: "f13" }, { name: "f14" }, { name: "f15" }, { name: "f16" },
      { name: "f17" }, { name: "f18" }, { name: "f19" }, { name: "f20" },
      { name: "f21", platformNote: "Windows / Linux" },
      { name: "f22", platformNote: "Windows / Linux" },
      { name: "f23", platformNote: "Windows / Linux" },
      { name: "f24", platformNote: "Windows / Linux" },
    ],
  },
  {
    label: "Media / System",
    keys: [
      { name: "media_play" }, { name: "media_next" }, { name: "media_prev" },
      { name: "media_stop", platformNote: "Windows / Linux" },
      { name: "volume_up" }, { name: "volume_down" }, { name: "volume_mute" },
      { name: "pause", platformNote: "Windows / Linux" },
      { name: "brightness_down", platformNote: "macOS" },
      { name: "brightness_up", platformNote: "macOS" },
      { name: "eject", platformNote: "macOS" },
      { name: "mic_mute", platformNote: "Linux" },
    ],
  },
] as const;

/**
 * Flat list of all valid key names, derived from `KEY_GROUPS`.
 * Used where a flat array is more convenient than the grouped structure.
 */
export const KNOWN_KEY_NAMES: readonly string[] =
  KEY_GROUPS.flatMap((g) => g.keys.map((k) => k.name));
