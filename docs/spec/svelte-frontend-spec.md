---
covers: Epics 5, 6, and 7 (Svelte core UI, finger pattern widget, live visualiser and debug panel)
status: Approved and fully implemented
last-updated: 2026-03-19
---

# svelte-frontend — Epic 5 specification

## Table of contents

1. [Overview](#overview)
2. [Technology choices and new dependencies](#technology-choices-and-new-dependencies)
3. [App-level structure and routing](#app-level-structure-and-routing)
4. [TypeScript types (`types.ts`)](#typescript-types-typests)
5. [Tauri command wrappers (`commands.ts`)](#tauri-command-wrappers-commandsts)
6. [Tauri event listeners (`events.ts`)](#tauri-event-listeners-eventsts)
7. [Svelte stores](#svelte-stores)
8. [Persistent layout](#persistent-layout)
9. [Device management page (`/devices`)](#device-management-page-devices)
10. [Profile list page (`/profiles`)](#profile-list-page-profiles)
11. [Profile editor page (`/profiles/[layer_id]/edit`)](#profile-editor-page-profileslayer_idedit)
12. [Component inventory](#component-inventory)
13. [Logging](#logging)
14. [Testing strategy](#testing-strategy)
15. [Out of scope for Epic 5](#out-of-scope-for-epic-5)

---

## Overview

Epic 5 replaces the generated Tauri scaffold with a functional UI covering the three primary
workflows:

1. **Device management** — scan, connect, and monitor Tap BLE devices.
2. **Profile management** — browse, activate, import, and delete profiles.
3. **Profile editing** — build and modify the mapping table, settings, aliases, and variables for
   a profile.

The UI need not be polished at this stage — the goal is correct behaviour with all data wired to
the Rust backend. Styling and visual polish are follow-up work; structure and wiring are the
deliverables.

**Tech stack:**

- Svelte 5 (runes API — `$state`, `$derived`, `$effect`, `$props`)
- SvelteKit 2.x in static/SPA mode (SSR disabled, `adapter-static`)
- TypeScript strict mode throughout
- `@tauri-apps/api` v2 for `invoke` and `listen`
- No additional UI framework beyond what is already in `package.json` (see §Technology choices)

---

## Technology choices and new dependencies

### Styling

**Approved:** Tailwind CSS v4 + daisyUI v5 (component library built on Tailwind).

- Use daisyUI components as the primary building blocks: `btn`, `card`, `modal`, `toast`,
  `badge`, `tabs`, `toggle`, `select`, `input`, `table`, `drawer`, `alert`, etc.
- Customise with Tailwind utility classes where daisyUI components are insufficient.
- Do not write scoped component `<style>` blocks; all styling goes through Tailwind/daisyUI
  classes on elements directly.
- The `src/lib/styles/variables.css` file is not needed — theming is handled by daisyUI's
  theme system.

### New npm dependencies

The following packages must be added to `package.json` before Epic 5 implementation begins:

| Package | Role |
| ------- | ---- |
| `tailwindcss` | Utility-first CSS framework |
| `@tailwindcss/vite` | Vite integration for Tailwind v4 |
| `daisyui` | Component library built on Tailwind |

Install as dev dependencies:

```sh
npm install -D tailwindcss @tailwindcss/vite daisyui
```

Tailwind v4 config is CSS-first (no `tailwind.config.js`). Add to `src/app.css` (create if
absent):

```css
@import "tailwindcss";
@plugin "daisyui";
```

Import `app.css` in `+layout.svelte` so it applies globally.

---

## App-level structure and routing

### Directory layout

```
src/
├── app.html                         ← existing, update <title> to "mapxr"
├── routes/
│   ├── +layout.svelte               ← new: persistent sidebar + status bar
│   ├── +layout.ts                   ← existing (ssr = false)
│   ├── +page.svelte                 ← new: redirect to /profiles
│   ├── devices/
│   │   └── +page.svelte             ← device management
│   ├── profiles/
│   │   ├── +page.svelte             ← profile list
│   │   └── [layer_id]/
│   │       └── edit/
│   │           └── +page.svelte     ← profile editor
│   └── debug/
│       └── +page.svelte             ← debug panel (task 7.6–7.14, stub only in Epic 5)
└── lib/
    ├── types.ts                     ← TypeScript mirrors of Rust structs
    ├── commands.ts                  ← invoke wrappers
    ├── events.ts                    ← listen wrappers
    ├── logger.ts                    ← structured logger (replaces console.log)
    ├── stores/
    │   ├── device.svelte.ts         ← deviceStore
    │   ├── engine.svelte.ts         ← engineStore
    │   ├── profile.svelte.ts        ← profileStore
    │   └── debug.svelte.ts          ← debugStore
    └── components/
        ├── Sidebar.svelte
        ├── StatusBar.svelte
        ├── Toast.svelte
        ├── ConfirmDialog.svelte
        ├── DeviceCard.svelte
        ├── ProfileCard.svelte
        ├── MappingRow.svelte
        ├── TriggerSummary.svelte
        ├── ActionSummary.svelte
        ├── TriggerEditor.svelte
        ├── ActionEditor.svelte
        └── FingerPatternPlaceholder.svelte  ← stub; replaced by Epic 6
```

### Routing

| URL | Component | Purpose |
| --- | --------- | ------- |
| `/` | `+page.svelte` | Redirect to `/profiles` |
| `/devices` | `devices/+page.svelte` | Device management |
| `/profiles` | `profiles/+page.svelte` | Profile list |
| `/profiles/[layer_id]/edit` | `profiles/[layer_id]/edit/+page.svelte` | Profile editor |
| `/debug` | `debug/+page.svelte` | Debug panel (stub in Epic 5; Epic 7 fleshes it out) |

The `/` redirect is implemented with:

```typescript
// src/routes/+page.svelte
<script lang="ts">
  import { goto } from '$app/navigation';
  goto('/profiles');
</script>
```

### Layout initialisation

`+layout.svelte` initialises the stores once on mount:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { engineStore } from '$lib/stores/engine.svelte';
  import { deviceStore } from '$lib/stores/device.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { setupEventListeners } from '$lib/events';

  onMount(async () => {
    await engineStore.init();
    await profileStore.init();
    setupEventListeners();
  });
</script>
```

Event listeners are set up once globally and never torn down (the app's lifetime equals the
listener lifetime).

---

## TypeScript types (`types.ts`)

All types mirror the Rust structs from `commands.rs`, `events.rs`, and `mapping-core`. Keep them
in a single file. Do not use `any` without a `// TODO:` comment.

```typescript
// src/lib/types.ts

// ── Device ──────────────────────────────────────────────────────────────────

export interface TapDeviceInfo {
  name: string | null;
  address: string;  // "AA:BB:CC:DD:EE:FF"
  rssi: number | null;
}

export interface DeviceStatusPayload {
  role: string;     // "solo" | "left" | "right"
  address: string;
}

export type DeviceRole = 'solo' | 'left' | 'right';

// ── Profile summary ─────────────────────────────────────────────────────────

export interface ProfileSummary {
  layer_id: string;
  name: string;
  kind: 'single' | 'dual';
  description: string | null;
}

// ── Full profile ─────────────────────────────────────────────────────────────
// Mirrors mapping_core::types::Profile and all referenced types.

export type ProfileKind = 'single' | 'dual';
export type Hand = 'left' | 'right';
export type PushLayerMode =
  | { type: 'permanent' }
  | { type: 'count'; count: number }
  | { type: 'timeout'; timeout_ms: number };

export type VariableValue =
  | { Bool: boolean }
  | { Int: number };

export interface ProfileSettings {
  combo_window_ms: number;
  double_tap_window_ms: number;
  triple_tap_window_ms: number;
  sequence_step_timeout_ms: number;
}

export type Modifier = 'ctrl' | 'shift' | 'alt' | 'meta';

export type Action =
  | { type: 'key'; key: string; modifiers: Modifier[] }
  | { type: 'key_chord'; keys: string[] }
  | { type: 'type_string'; text: string }
  | { type: 'macro'; steps: MacroStep[] }
  | { type: 'push_layer'; layer: string; mode: PushLayerMode }
  | { type: 'pop_layer' }
  | { type: 'switch_layer'; layer: string }
  | { type: 'toggle_variable'; variable: string; on_true: Action; on_false: Action }
  | { type: 'set_variable'; variable: string; value: VariableValue }
  | { type: 'block' }
  | { type: 'alias'; name: string };

export interface MacroStep {
  action: Action;
  delay_ms: number;
}

export type TriggerKind = 'tap' | 'double_tap' | 'triple_tap' | 'sequence';

export interface TapStep {
  code: string;          // finger pattern string e.g. "xoooo"
  window_ms: number | null;
}

export type Trigger =
  | { type: 'tap'; code: string }
  | { type: 'double_tap'; code: string }
  | { type: 'triple_tap'; code: string }
  | { type: 'sequence'; steps: TapStep[]; window_ms: number | null };

export interface Mapping {
  label: string;
  trigger: Trigger;
  action: Action;
  enabled: boolean;
}

export interface Profile {
  version: number;
  kind: ProfileKind;
  name: string;
  description: string | null;
  layer_id: string;
  hand: Hand | null;
  passthrough: boolean;
  settings: ProfileSettings;
  aliases: Record<string, Action>;
  variables: Record<string, VariableValue>;
  on_enter: Action | null;
  on_exit: Action | null;
  mappings: Mapping[];
}

// ── Engine state ─────────────────────────────────────────────────────────────

export interface EngineStateSnapshot {
  layer_stack: string[];        // bottom to top
  active_layer_id: string;
  variables: Record<string, unknown>;  // JSON-serialised VariableValue
  connected_device_roles: string[];
  debug_mode: boolean;
}

// ── Events ───────────────────────────────────────────────────────────────────

export interface TapEventPayload {
  device_id: string;
  tap_code: number;
  received_at_ms: number;
}

export interface ActionFiredPayload {
  action_kind: string;
  label: string | null;
}

export interface LayerChangedPayload {
  stack: string[];   // bottom to top
  active: string;
}

export interface ProfileErrorPayload {
  file_name: string;
  message: string;
}

export interface DebugEvent {
  // Mirrors mapping_core::engine::DebugEvent — keep loosely typed as the
  // structure may evolve. The debug panel (Epic 7) will tighten these types.
  // TODO: task 7.8–7.10 — tighten DebugEvent types when the debug panel is built
  kind: 'resolved' | 'unmatched' | 'combo_timeout';
  [key: string]: unknown;
}
```

> **Note on serde tag conventions:** The Rust `Action` enum uses `serde(tag = "type")` (or
> equivalent). Confirm the actual tag key used by `serde` for each enum before finalising these
> TypeScript types. If the Rust side uses `serde(rename_all = "snake_case")` + external/internal
> tagging, update the TypeScript union discriminants to match exactly. This must be verified before
> task 5.1 is marked complete.

---

## Tauri command wrappers (`commands.ts`)

All `invoke` calls are centralised here. Every function has a JSDoc comment.

```typescript
// src/lib/commands.ts
import { invoke } from '@tauri-apps/api/core';
import type {
  TapDeviceInfo, ProfileSummary, Profile, PushLayerMode, EngineStateSnapshot
} from './types';

/** Scan for nearby Tap BLE devices for ~5 seconds. */
export async function scanDevices(): Promise<TapDeviceInfo[]> {
  return invoke('scan_devices');
}

/** Connect to a Tap device at `address` and assign it `role`. */
export async function connectDevice(address: string, role: string): Promise<void> {
  return invoke('connect_device', { address, role });
}

/** Disconnect the device assigned to `role`. */
export async function disconnectDevice(role: string): Promise<void> {
  return invoke('disconnect_device', { role });
}

/** List all profiles in the profiles directory (triggers a reload). */
export async function listProfiles(): Promise<ProfileSummary[]> {
  return invoke('list_profiles');
}

/** Load the full profile for `layerId`. */
export async function loadProfile(layerId: string): Promise<Profile> {
  return invoke('load_profile', { layer_id: layerId });
}

/** Write a profile to disk and reload the registry. */
export async function saveProfile(profile: Profile): Promise<void> {
  return invoke('save_profile', { profile });
}

/** Delete the profile file for `layerId`. */
export async function deleteProfile(layerId: string): Promise<void> {
  return invoke('delete_profile', { layer_id: layerId });
}

/** Replace the engine's base layer with `layerId`. */
export async function activateProfile(layerId: string): Promise<void> {
  return invoke('activate_profile', { layer_id: layerId });
}

/** Push the profile `layerId` onto the engine's layer stack. */
export async function pushLayer(layerId: string, mode: PushLayerMode): Promise<void> {
  return invoke('push_layer', { layer_id: layerId, mode });
}

/** Pop the top layer off the engine's layer stack. */
export async function popLayer(): Promise<void> {
  return invoke('pop_layer');
}

/** Enable or disable debug event emission. */
export async function setDebugMode(enabled: boolean): Promise<void> {
  return invoke('set_debug_mode', { enabled });
}

/** Return a snapshot of the current engine state. */
export async function getEngineState(): Promise<EngineStateSnapshot> {
  return invoke('get_engine_state');
}
```

---

## Tauri event listeners (`events.ts`)

All `listen` calls are centralised here. The setup function is called once from the layout.

```typescript
// src/lib/events.ts
import { listen } from '@tauri-apps/api/event';
import { deviceStore } from './stores/device.svelte';
import { engineStore } from './stores/engine.svelte';
import { profileStore } from './stores/profile.svelte';
import { debugStore } from './stores/debug.svelte';
import { logger } from './logger';
import type {
  TapEventPayload, ActionFiredPayload, LayerChangedPayload,
  DeviceStatusPayload, ProfileErrorPayload, DebugEvent
} from './types';

/** Register all Tauri event listeners. Call once from the root layout. */
export async function setupEventListeners(): Promise<void> {
  await listen<TapEventPayload>('tap-event', ({ payload }) => {
    debugStore.recordTap(payload);
  });

  await listen<ActionFiredPayload>('action-fired', ({ payload }) => {
    debugStore.recordAction(payload);
  });

  await listen<LayerChangedPayload>('layer-changed', ({ payload }) => {
    engineStore.applyLayerChanged(payload);
  });

  await listen<DeviceStatusPayload>('device-connected', ({ payload }) => {
    deviceStore.onConnected(payload);
  });

  await listen<DeviceStatusPayload>('device-disconnected', ({ payload }) => {
    deviceStore.onDisconnected(payload);
  });

  await listen<DebugEvent>('debug-event', ({ payload }) => {
    debugStore.appendDebugEvent(payload);
  });

  await listen<ProfileErrorPayload>('profile-error', ({ payload }) => {
    logger.warn(`Profile load error: ${payload.file_name} — ${payload.message}`);
    profileStore.appendError(payload);
  });
}
```

---

## Svelte stores

Stores use Svelte 5 runes (`.svelte.ts` files). State is reactive via `$state`; derived values
via `$derived`. Stores export a singleton instance.

### `deviceStore` (task 5.4)

```typescript
// src/lib/stores/device.svelte.ts
import type { DeviceStatusPayload } from '../types';

interface ConnectedDevice {
  role: string;
  address: string;
}

class DeviceStore {
  connected = $state<ConnectedDevice[]>([]);

  onConnected(payload: DeviceStatusPayload): void {
    // Replace any existing entry for this role.
    this.connected = [
      ...this.connected.filter(d => d.role !== payload.role),
      { role: payload.role, address: payload.address },
    ];
  }

  onDisconnected(payload: DeviceStatusPayload): void {
    this.connected = this.connected.filter(d => d.role !== payload.role);
  }

  isConnected(role: string): boolean {
    return this.connected.some(d => d.role === role);
  }
}

export const deviceStore = new DeviceStore();
```

### `engineStore` (task 5.5)

```typescript
// src/lib/stores/engine.svelte.ts
import { getEngineState } from '../commands';
import type { EngineStateSnapshot, LayerChangedPayload } from '../types';

class EngineStore {
  layerStack = $state<string[]>([]);
  activeLayerId = $state<string>('');
  variables = $state<Record<string, unknown>>({});
  debugMode = $state(false);

  async init(): Promise<void> {
    const snap = await getEngineState();
    this.applySnapshot(snap);
  }

  applySnapshot(snap: EngineStateSnapshot): void {
    this.layerStack = snap.layer_stack;
    this.activeLayerId = snap.active_layer_id;
    this.variables = snap.variables;
    this.debugMode = snap.debug_mode;
  }

  applyLayerChanged(payload: LayerChangedPayload): void {
    this.layerStack = payload.stack;
    this.activeLayerId = payload.active;
  }
}

export const engineStore = new EngineStore();
```

### `profileStore` (task 5.6)

```typescript
// src/lib/stores/profile.svelte.ts
import { listProfiles } from '../commands';
import type { ProfileSummary, ProfileErrorPayload } from '../types';

class ProfileStore {
  profiles = $state<ProfileSummary[]>([]);
  loadErrors = $state<ProfileErrorPayload[]>([]);

  async init(): Promise<void> {
    await this.reload();
  }

  async reload(): Promise<void> {
    this.loadErrors = [];
    this.profiles = await listProfiles();
  }

  appendError(err: ProfileErrorPayload): void {
    this.loadErrors = [...this.loadErrors, err];
  }
}

export const profileStore = new ProfileStore();
```

### `debugStore` (task 5.7)

```typescript
// src/lib/stores/debug.svelte.ts
import type { TapEventPayload, ActionFiredPayload, DebugEvent } from '../types';

const MAX_DEBUG_EVENTS = 200;

class DebugStore {
  enabled = $state(false);
  debugEvents = $state<DebugEvent[]>([]);
  lastTap = $state<TapEventPayload | null>(null);
  lastAction = $state<ActionFiredPayload | null>(null);

  recordTap(payload: TapEventPayload): void {
    this.lastTap = payload;
  }

  recordAction(payload: ActionFiredPayload): void {
    this.lastAction = payload;
  }

  appendDebugEvent(event: DebugEvent): void {
    this.debugEvents = [event, ...this.debugEvents].slice(0, MAX_DEBUG_EVENTS);
  }

  clear(): void {
    this.debugEvents = [];
  }
}

export const debugStore = new DebugStore();
```

---

## Persistent layout

`+layout.svelte` renders a two-panel shell:

```
┌─────────────────────────────────────────────────────────┐
│  Sidebar       │  Main content                           │
│                │                                         │
│  • Devices     │  <slot />                               │
│  • Profiles    │                                         │
│  • Debug       │                                         │
│                │                                         │
├────────────────┴─────────────────────────────────────────┤
│  StatusBar: layer stack breadcrumb | connected devices   │
└─────────────────────────────────────────────────────────┘
```

### `Sidebar.svelte`

Props: none. Uses `$page.url.pathname` from SvelteKit to highlight the active route.

Navigation items:
- **Devices** → `/devices`
- **Profiles** → `/profiles`
- **Debug** → `/debug`

### `StatusBar.svelte`

Reads from `engineStore` and `deviceStore`. Shows:
- Layer stack breadcrumb (bottom → active): e.g. `base > symbols`
- Connected device chips: one chip per connected device, labelled by role

No interaction — purely informational.

---

## Device management page (`/devices`)

### Layout

```
┌──────────────────────────────────────────┐
│  Scan  [Scan for devices]                │  ← scan button
│                                          │
│  Discovered devices                      │
│  ┌────────────────────────────────────┐  │
│  │ Tap Strap 2  AA:BB:CC:DD:EE:FF  -60 dBm  [Connect] │
│  │ ...                                                  │
│  └────────────────────────────────────┘  │
│                                          │
│  Connected devices                       │
│  ┌────────────────────────────────────┐  │
│  │ solo  AA:BB:CC:DD:EE:FF  [Disconnect] │
│  │ ...                                   │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

### Task 5.8 — Scan UI

- A **Scan** button calls `scanDevices()` and populates a list of `TapDeviceInfo`.
- While scanning, the button shows a spinner and is disabled.
- If BLE is unavailable (the invoke rejects with the "No Bluetooth adapter" error), show an
  inline error banner: "Bluetooth is not available on this device."
- Each discovered device shows: name (or address if no name), address, signal strength badge.

### Task 5.9 — Role assignment

- Each discovered device has a **Connect** button.
- Clicking opens an inline role selector: three buttons labelled Solo / Left / Right.
- Selecting a role calls `connectDevice(address, role)`.
- On success: the device appears in the "Connected devices" section and the button changes to
  a spinner then disappears from discovered list.
- On failure: show a toast error.

### Task 5.10 — Connected device status panel

- Lists all devices from `deviceStore.connected`.
- Each entry shows: role badge, address, signal strength if available.
- No battery level in Epic 5 (the backend does not yet expose it).

### Task 5.11 — Disconnect button

- Each connected device has a **Disconnect** button.
- Clicking shows a `ConfirmDialog`: "Disconnect [role]? The device will exit controller mode."
- On confirm: calls `disconnectDevice(role)`.
- The store updates automatically via the `device-disconnected` event.

### Task 5.12 — Missing roles warning

- When the profile editor or profile list page detects that the active profile is `"dual"` but
  fewer than 2 devices are connected, show an inline warning banner on those pages:
  "This profile requires two connected devices."
- Computed from `engineStore.activeLayerId`, `profileStore.profiles`, and
  `deviceStore.connected`.

---

## Profile list page (`/profiles`)

### Layout

```
┌────────────────────────────────────────────────┐
│  Profiles           [+ New]  [Import]           │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ ● Base       [single]  "Default layout"  │  │  ← active indicator
│  │   [Activate]  [Edit]   [Delete]          │  │
│  │──────────────────────────────────────────│  │
│  │   Gaming     [single]  "FPS bindings"    │  │
│  │   [Activate]  [Edit]   [Delete]          │  │
│  └──────────────────────────────────────────┘  │
└────────────────────────────────────────────────┘
```

### Task 5.13 — Profile list

- Calls `listProfiles()` on mount (via `profileStore.init()`).
- Displays each profile as a card: name, kind badge (`single` / `dual`), description (if set).
- The active profile (matching `engineStore.activeLayerId`) shows a filled dot indicator.
- Sort order: alphabetical by name (already returned sorted by the backend).

### Task 5.14 — Activate

- **Activate** button calls `activateProfile(layerId)`.
- On success: `engineStore` updates via the `layer-changed` event; the active indicator moves.
- On failure: show a toast error.

### Task 5.15 — Delete

- **Delete** button shows a `ConfirmDialog`: "Delete [name]? This cannot be undone."
- On confirm: calls `deleteProfile(layerId)`, then calls `profileStore.reload()`.
- If deleting the active profile, also re-init the engine by calling `getEngineState()` and
  updating `engineStore`.

### Task 5.16 — New profile wizard

- **+ New** button opens a modal `NewProfileWizard` component.
- Fields:
  - Name (text, required)
  - Kind: Single / Dual (radio)
  - Hand: Left / Right (only shown for Single)
  - Description (text, optional)
- `layer_id` is derived from `name` by lowercasing and replacing non-alphanumeric characters
  with underscores. Show the derived `layer_id` as a read-only hint below the name field.
- On submit: constructs a minimal `Profile` object (empty mappings, default settings) and calls
  `saveProfile(profile)`, then `profileStore.reload()`.
- The new profile is not automatically activated.

### Task 5.17 — Profile import

- **Import** button opens an `<input type="file" accept=".json">` file picker.
- On file selection: reads the file content as text, parses as JSON (show error if not valid
  JSON), then calls `saveProfile(parsed)`.
- If `saveProfile` rejects (validation failure on the Rust side), show the error message in a
  toast.
- On success: `profileStore.reload()`.

---

## Profile editor page (`/profiles/[layer_id]/edit`)

The editor is accessed by clicking **Edit** on a profile card. It loads the full profile via
`loadProfile(layerId)` and holds an editable in-memory copy as local `$state`.

### Unsaved-changes guard (task 5.30)

Track whether the in-memory profile has diverged from the loaded profile. When the user attempts
to navigate away with unsaved changes, show a `ConfirmDialog`: "You have unsaved changes. Leave
without saving?"

Detect navigation attempts via SvelteKit's `beforeNavigate` hook.

### Save button (task 5.29)

- A persistent **Save** button (or a "Save changes" banner when dirty) calls `saveProfile(profile)`.
- On success: show a transient success toast; clear the dirty flag.
- On failure: show an error toast with the Rust error message.

### Sub-sections

The editor is divided into tabs or collapsible sections. For Epic 5, use a simple tab layout:

| Tab | Content |
| --- | ------- |
| Mappings | Mapping list (tasks 5.18–5.22) + trigger/action editors (5.23–5.24) |
| Settings | Profile settings fields (task 5.25) |
| Aliases | Alias manager (task 5.26) |
| Variables | Variable manager (task 5.27) |
| Lifecycle | `on_enter` / `on_exit` action editors (task 5.28) |

### Task 5.18 — Mapping list

- Shows all `profile.mappings` as rows in a table/list.
- Each row: label, trigger summary (via `TriggerSummary` component), action summary (via
  `ActionSummary` component), enabled toggle, reorder handles, delete button.
- Clicking a row (or a dedicated Edit button) opens the inline trigger/action editor panel below
  the list.

### Task 5.19 — Mapping reorder

- Drag handles on each row. Implement drag-and-drop reorder using native HTML5 drag-and-drop
  (`draggable` attribute, `dragover`/`drop` events).
- Reorder is local state only; saved when the user clicks Save.
- No library dependency required.

### Task 5.20 — Enable/disable toggle

- A checkbox or toggle switch on each row sets `mapping.enabled = false/true`.
- Does not delete the mapping from the profile JSON.

### Task 5.21 — Add mapping

- **+ Add mapping** button at the bottom of the list.
- Appends a new `Mapping` with an empty label, a default `Tap` trigger (`code: "xoooo"`), and
  a default `Block` action.
- The new row is immediately selected for editing.

### Task 5.22 — Delete mapping

- **Delete** icon button on each row.
- Shows an undo toast for 5 seconds: "Mapping deleted — Undo". If the user does not click Undo
  within 5 seconds, the mapping is removed from the in-memory profile. If Undo is clicked, the
  mapping is restored at its original position.
- Undo is in-memory only (no backend call until Save).

### Task 5.23 — Trigger editor panel

Opens inline below the selected mapping row (or in a side panel). Contains:

#### 5.23a — Trigger type selector

A segmented control or `<select>` with options: Tap / Double Tap / Triple Tap / Sequence.
Switching type replaces the trigger object with a default for the new type.

#### 5.23b — Finger pattern input

Uses `FingerPatternPlaceholder.svelte` in Epic 5 (a simple text input accepting the `"xoooo"`
string format with inline validation). Epic 6 replaces this with the visual component.

The placeholder component:
- Accepts a `code: string` prop and emits a `change(code: string)` event.
- Validates: must be exactly 5 chars (single profile) or `"5 5"` format (dual), using only
  `o`/`x`. Shows an inline error if invalid.

#### 5.23c — Sequence step list

Only shown when trigger type is `Sequence`. Shows an ordered list of `TapStep`s. Each step has:
- A `FingerPatternPlaceholder` for the code.
- An optional `window_ms` override field (number input, blank = use profile default).
- A delete button.
- **+ Add step** button appends a new step.

#### 5.23d — Per-trigger `window_ms` override

Shown for all trigger types. A number input labelled "Window override (ms)". Blank = use the
profile setting.

### Task 5.24 — Action editor panel

Below the trigger editor. Contains:

#### 5.24a — Action type selector

A `<select>` with all action types: Key / Key Chord / Type String / Macro / Push Layer /
Pop Layer / Switch Layer / Toggle Variable / Set Variable / Block / Alias.

Switching type replaces the action object with a default for the new type.

#### 5.24b — `key` action

- Key name: text input with a datalist offering known key names (sourced from a static list in
  `types.ts`).
- Modifiers: four checkboxes — Ctrl, Shift, Alt, Meta.

#### 5.24c — `key_chord` action

- A multi-value key list: shows existing keys as chips, with a text input to add more.
- Each chip has a remove (×) button.

#### 5.24d — `type_string` action

- A single `<textarea>` labelled "Text to type".

#### 5.24e — `macro` action

- A list of `MacroStep`s. Each step has:
  - A nested `ActionEditor` (recursive; limited to non-macro action types to prevent nesting).
  - A `delay_ms` number input (0 = no delay).
  - Up/down arrows or drag handles for reordering.
  - A delete button.
- **+ Add step** button appends a new step.

#### 5.24f — `push_layer` action

- Layer selector: `<select>` populated from `profileStore.profiles`.
- Mode selector: radio buttons — Permanent / Count / Timeout.
- If Count selected: show a count number input.
- If Timeout selected: show a `timeout_ms` number input.

#### 5.24g — `pop_layer` / `block` actions

No additional fields. Shows a descriptive label: "Pops the current layer." / "Blocks
passthrough to lower layers."

#### 5.24h — `toggle_variable` action

- Variable name: `<select>` populated from the profile's `variables` map.
- On-true action editor: a nested `ActionEditor` (non-macro, non-toggle).
- On-false action editor: a nested `ActionEditor` (non-macro, non-toggle).

#### 5.24i — `alias` action

- Alias name: `<select>` populated from the profile's `aliases` map.

### Task 5.25 — Settings panel

A form showing all `ProfileSettings` fields with labels and range hints:

| Field | Input type | Default | Hint |
| ----- | ---------- | ------- | ---- |
| `combo_window_ms` | number | 200 | "Dual-tap combo detection window (ms)" |
| `double_tap_window_ms` | number | 300 | "Double-tap detection window (ms)" |
| `triple_tap_window_ms` | number | 400 | "Triple-tap detection window (ms)" |
| `sequence_step_timeout_ms` | number | 600 | "Max gap between sequence steps (ms)" |

All values are numbers in milliseconds. Validate that all ms values are positive integers.

### Task 5.26 — Alias manager

A table listing all `profile.aliases` entries (name → action).

- **+ Add** button opens an inline form: alias name text input + `ActionEditor`.
- Each row has an **Edit** button (opens inline editor) and a **Delete** button.
- Deletion is immediate (local state) — no undo.

### Task 5.27 — Variable manager

A table listing all `profile.variables` entries (name → initial value).

- **+ Add** button opens an inline form: name input + type selector (Bool / Int) + value input.
- For Bool: a checkbox or toggle for the initial value.
- For Int: a number input for the initial value.
- Each row shows the name, type, and current initial value. Delete button removes the variable.
- Warn inline if a variable name is referenced in a `toggle_variable` or `set_variable` action
  in the mappings but does not exist in the variables map.

### Task 5.28 — `on_enter` / `on_exit` editors

Two `ActionEditor` panels on the Lifecycle tab, labelled "On enter" and "On exit".
Each has a "None" option (sets the field to `null`) in addition to all action types.

---

## Component inventory

### `Toast.svelte`

Props:
```typescript
interface Props {
  message: string;
  kind: 'info' | 'success' | 'error' | 'warning';
  duration?: number;  // ms; default 4000
  action?: { label: string; onClick: () => void };
}
```

- Auto-dismisses after `duration` ms.
- If `action` is set, shows an action button (used for undo in task 5.22).
- Multiple toasts stack vertically (fixed-position at bottom-right).
- Manage a toast queue with a module-level `$state<ToastEntry[]>`.

### `ConfirmDialog.svelte`

Props:
```typescript
interface Props {
  title: string;
  body: string;
  confirmLabel?: string;   // default "Confirm"
  cancelLabel?: string;    // default "Cancel"
  onConfirm: () => void;
  onCancel: () => void;
}
```

- Renders as a modal overlay.
- Focus trap: Tab/Shift-Tab cycle between Cancel and Confirm; Escape → Cancel.

### `TriggerSummary.svelte`

Props:
```typescript
interface Props {
  trigger: Trigger;
}
```

- Read-only one-line summary: e.g. "Tap xoooo", "Double Tap xoooo", "Sequence [3 steps]".

### `ActionSummary.svelte`

Props:
```typescript
interface Props {
  action: Action;
}
```

- Read-only one-line summary: e.g. "Key ctrl+c", "Type: Hello world", "Push layer: gaming".

### `FingerPatternPlaceholder.svelte`

Props:
```typescript
interface Props {
  code: string;
  readonly?: boolean;
  onchange?: (code: string) => void;
}
```

- Renders a plain text input (single-hand) or two text inputs (dual, split on space).
- Validates on input; shows inline error for invalid patterns.
- In `readonly` mode, renders as plain text.
- Replaced by the real `FingerPattern` component in Epic 6.

### `ActionEditor.svelte`

Props:
```typescript
interface Props {
  action: Action;
  profile: Profile;        // for populating alias/variable/layer selectors
  disallow?: ActionType[]; // e.g. ['macro'] to prevent nesting
  onchange: (action: Action) => void;
}
```

Renders the appropriate sub-editor based on `action.type`. Handles the type selector and
conditional field rendering as described in task 5.24.

---

## Logging

No `console.log` in committed code. Use a structured logger wrapper:

```typescript
// src/lib/logger.ts
const isDev = import.meta.env.DEV;

export const logger = {
  info:  (msg: string, ...args: unknown[]) => { if (isDev) console.info(`[mapxr] ${msg}`, ...args); },
  warn:  (msg: string, ...args: unknown[]) => { if (isDev) console.warn(`[mapxr] ${msg}`, ...args); },
  error: (msg: string, ...args: unknown[]) => { console.error(`[mapxr] ${msg}`, ...args); },
};
```

`logger.error` always emits (not gated on `isDev`) so errors are visible in production builds.

---

## Testing strategy

Svelte components do not require tests for the initial implementation per `CLAUDE.md`. However,
the following pure TypeScript modules should have unit tests:

| Module | Test |
| ------ | ---- |
| `types.ts` — serde tag compatibility | No automated test; verified manually against a sample `invoke` response in dev. Note in progress log if any type mismatch is found. |
| Store logic | Light unit tests for `deviceStore.onConnected/onDisconnected`, `engineStore.applyLayerChanged`, `profileStore.appendError` — pure functions, no DOM needed. |
| `FingerPatternPlaceholder` validation | Unit test the validation logic (extracted to a helper function) against valid and invalid strings. |

For end-to-end validation, run the app in dev mode (`npm run tauri dev`) against the sample
profiles in `profiles/`.

---

## Out of scope for Epic 5

The following are explicitly deferred to later epics:

| Feature | Epic |
| ------- | ---- |
| Visual finger pattern widget | Epic 6 |
| Live tap visualiser in status bar | Epic 7 |
| Debug panel implementation | Epic 7 |
| Battery level display | Not yet exposed by backend |
| Profile `required_roles` enforcement beyond warning | Stretch goal |
| Dark mode | Follow-up styling pass |
| Drag-and-drop polish (animations, scroll-into-view) | Follow-up styling pass |
| Keyboard shortcuts for common actions | Follow-up |
