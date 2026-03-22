<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { addPluginListener } from "@tauri-apps/api/core";
  import {
    scanDevices, connectDevice, disconnectDevice, reassignDeviceRole, renameDevice,
    getPlatform,
    checkBlePermissions, requestBlePermissions, listBondedDevices,
    bleConnect, bleDisconnect,
    assignAndroidDevice, reassignAndroidDeviceRole,
  } from "$lib/commands";
  import { deviceStore } from "$lib/stores/device.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { logger } from "$lib/logger";
  import type { TapDeviceInfo, BleDevicePendingPayload, DeviceRole } from "$lib/types";

  // ── Helpers ─────────────────────────────────────────────────────────────────

  /** Extract a human-readable string from any thrown value.
   *
   * Tauri plugin invoke rejections on mobile arrive as plain objects
   * (e.g. `{message: "..."}`) rather than Error instances. */
  function errMsg(e: unknown): string {
    if (e instanceof Error) return e.message;
    if (typeof e === "string") return e;
    if (typeof e === "object" && e !== null && "message" in e)
      return String((e as Record<string, unknown>).message);
    return JSON.stringify(e);
  }

  // ── Platform ────────────────────────────────────────────────────────────────

  let isAndroid = $state(false);
  onMount(async () => {
    isAndroid = (await getPlatform()) === "android";
  });

  // ── Desktop scan state ──────────────────────────────────────────────────────

  let scanning = $state(false);
  let discovered = $state<TapDeviceInfo[]>([]);
  let scanError = $state<string | null>(null);

  /** Addresses of devices that are currently connected — excluded from the scan list. */
  let connectedAddresses = $derived(new Set(deviceStore.connected.map((d) => d.address)));
  /** Scan results with already-connected devices removed. Updates live as devices connect/disconnect. */
  let availableDevices = $derived(discovered.filter((d) => !connectedAddresses.has(d.address)));

  async function handleScan() {
    scanning = true;
    scanError = null;
    discovered = [];
    try {
      discovered = await scanDevices();
    } catch (e) {
      const msg = errMsg(e);
      if (msg.toLowerCase().includes("bluetooth") || msg.toLowerCase().includes("adapter")) {
        scanError = "Bluetooth is not available on this device.";
      } else {
        scanError = msg;
      }
      logger.error("scan_devices failed", e);
    } finally {
      scanning = false;
    }
  }

  // ── Android bonded-device state ─────────────────────────────────────────────

  let bleLoading = $state(false);
  let bleDiscovered = $state<{ address: string; name: string | null }[]>([]);
  let bleScanError = $state<string | null>(null);

  /** Devices BLE-connected but not yet assigned a role. */
  let pendingDevices = $state<BleDevicePendingPayload[]>([]);

  let androidConnectingAddress = $state<string | null>(null);
  let androidConnectError = $state<string | null>(null);

  /** address → selected role for pending devices */
  let pendingRoles = $state<Record<string, string>>({});

  let blePendingUnlisten: (() => void) | null = null;

  onMount(async () => {
    blePendingUnlisten = await listen<BleDevicePendingPayload>(
      "ble-device-pending",
      ({ payload }) => {
        if (!pendingDevices.some((d) => d.address === payload.address)) {
          pendingDevices = [...pendingDevices, payload];
        }
        bleDiscovered = bleDiscovered.filter((d) => d.address !== payload.address);
      },
    );
    return () => { blePendingUnlisten?.(); };
  });

  // Remove devices from pending once they appear in deviceStore.connected.
  $effect(() => {
    const connected = new Set(deviceStore.connected.map((d) => d.address));
    pendingDevices = pendingDevices.filter((d) => !connected.has(d.address));
  });

  async function handleAndroidScan() {
    bleScanError = null;
    bleDiscovered = [];

    // BLUETOOTH_CONNECT permission is required to read bonded device names on API 31+.
    try {
      const check = await checkBlePermissions();
      if (!check.granted) {
        const req = await requestBlePermissions();
        if (!req.granted) {
          bleScanError = "Bluetooth permission is required.";
          return;
        }
      }
    } catch (e) {
      bleScanError = errMsg(e);
      return;
    }

    bleLoading = true;
    try {
      const result = await listBondedDevices();
      bleDiscovered = result.devices;
      if (result.devices.length === 0) {
        bleScanError = "No paired Bluetooth devices found. Pair your Tap in Android Bluetooth Settings first.";
      }
    } catch (e) {
      bleScanError = errMsg(e);
    } finally {
      bleLoading = false;
    }
  }

  async function handleAndroidConnect(address: string, name: string | null) {
    androidConnectingAddress = address;
    androidConnectError = null;
    try {
      await bleConnect(address);
      deviceStore.setName(address, name);
      // Device will move to pendingDevices via `ble-device-pending` event
      // (or straight to deviceStore if a role is already persisted).
    } catch (e) {
      androidConnectError = errMsg(e);
    } finally {
      androidConnectingAddress = null;
    }
  }

  function selectPendingRole(address: string, role: string) {
    pendingRoles = { ...pendingRoles, [address]: role };
  }

  async function handleAssignRole(address: string, name: string | null) {
    const role = pendingRoles[address];
    if (!role) return;
    try {
      await assignAndroidDevice(address, role, name);
      const updated = { ...pendingRoles };
      delete updated[address];
      pendingRoles = updated;
    } catch (e) {
      logger.error("assignAndroidDevice failed", e);
    }
  }

  function isRoleTaken(role: string): boolean {
    return deviceStore.connected.some((d) => d.role === role);
  }

  async function handleAndroidDisconnect(address: string) {
    androidDisconnectConfirmAddress = null;
    try {
      await bleDisconnect(address);
    } catch (e) {
      logger.error("bleDisconnect failed", e);
    }
  }

  async function handleAndroidReassign(address: string, newRole: string) {
    try {
      await reassignAndroidDeviceRole(address, newRole);
    } catch (e) {
      logger.error("reassignAndroidDeviceRole failed", e);
    }
  }

  let androidDisconnectConfirmAddress = $state<string | null>(null);

  // ── BLE debug log (temporary diagnostic panel) ──────────────────────────────

  let bleDebugLog = $state<string[]>([]);

  function bleDebugAppend(msg: string) {
    const ts = new Date().toLocaleTimeString();
    bleDebugLog = [`[${ts}] ${msg}`, ...bleDebugLog].slice(0, 30);
  }

  let _bleLogUnregister: (() => void) | null = null;
  $effect(() => {
    if (!isAndroid) return;
    addPluginListener<{ msg: string }>("ble", "ble-log", ({ msg }) => bleDebugAppend(msg))
      .then((l) => { _bleLogUnregister = () => void l.unregister(); })
      .catch(() => {});
    return () => { _bleLogUnregister?.(); };
  });

  // ── Desktop connect state ───────────────────────────────────────────────────

  let connectingAddress = $state<string | null>(null);
  let connectError = $state<string | null>(null);
  /** address → role being selected */
  let pendingRole = $state<Record<string, string>>({});

  function selectRole(address: string, role: string) {
    pendingRole = { ...pendingRole, [address]: role };
  }

  async function handleConnect(address: string) {
    const role = pendingRole[address];
    if (!role) return;
    // Capture the name before awaiting — availableDevices reactively drops this
    // device once device-connected fires, so the lookup would return undefined after the await.
    const name = discovered.find((d) => d.address === address)?.name ?? null;
    connectingAddress = address;
    connectError = null;
    try {
      await connectDevice(address, role);
      deviceStore.setName(address, name);
      const updatedRole = { ...pendingRole };
      delete updatedRole[address];
      pendingRole = updatedRole;
    } catch (e) {
      connectError = e instanceof Error ? e.message : String(e);
      logger.error("connect_device failed", e);
    } finally {
      connectingAddress = null;
    }
  }

  // ── Desktop reassign state ──────────────────────────────────────────────────

  let reassigningAddress = $state<string | null>(null);
  let reassignError = $state<string | null>(null);

  /** Set of roles currently occupied by any connected device. */
  let connectedRoles = $derived(new Set(deviceStore.connected.map((d) => d.role)));

  /** A role button is enabled only when the role is unoccupied and differs from the device's current role. */
  function canReassignTo(deviceRole: string, candidate: string): boolean {
    return candidate !== deviceRole && !connectedRoles.has(candidate);
  }

  async function handleReassign(address: string, newRole: string) {
    reassigningAddress = address;
    reassignError = null;
    try {
      await reassignDeviceRole(address, newRole);
    } catch (e) {
      reassignError = e instanceof Error ? e.message : String(e);
      logger.error("reassign_device_role failed", e);
    } finally {
      reassigningAddress = null;
    }
  }

  // ── Desktop rename state ────────────────────────────────────────────────────

  let renamingAddress = $state<string | null>(null);
  let renameInput = $state("");
  let renameError = $state<string | null>(null);
  let renameNotice = $state<string | null>(null);
  let renamingInProgress = $state(false);

  function startRename(address: string, currentName: string | null) {
    renamingAddress = address;
    renameInput = currentName ?? "";
    renameError = null;
    renameNotice = null;
  }

  function cancelRename() {
    renamingAddress = null;
    renameError = null;
  }

  async function handleRename(address: string) {
    renamingInProgress = true;
    renameError = null;
    try {
      await renameDevice(address, renameInput);
      deviceStore.setName(address, renameInput.trim());
      renamingAddress = null;
      renameNotice = "Name saved — reconnect the device to see the new name in scan results.";
    } catch (e) {
      renameError = e instanceof Error ? e.message : String(e);
      logger.error("rename_device failed", e);
    } finally {
      renamingInProgress = false;
    }
  }

  // ── Desktop disconnect state ────────────────────────────────────────────────

  let disconnectConfirmRole = $state<string | null>(null);
  let disconnectingRole = $state<string | null>(null);

  async function handleDisconnect(role: string) {
    disconnectingRole = role;
    disconnectConfirmRole = null;
    try {
      await disconnectDevice(role);
    } catch (e) {
      logger.error("disconnect_device failed", e);
    } finally {
      disconnectingRole = null;
    }
  }

  // ── Profile / device compatibility banners ─────────────────────────────────

  let activeProfile = $derived(
    profileStore.profiles.find((p) => p.layer_id === engineStore.activeLayerId),
  );
  /** Dual profile active but fewer than two devices connected. */
  let showDualWarning = $derived(
    activeProfile?.kind === "dual" && deviceStore.connected.length < 2,
  );
  /** Two devices connected but only a single-hand profile is active. */
  let showSingleSuggestion = $derived(
    deviceStore.connected.length === 2 && activeProfile?.kind === "single",
  );
  let dismissedSingleSuggestion = $state(false);

  // ── Signal strength helpers ─────────────────────────────────────────────────

  function rssiLabel(rssi: number | null): string {
    if (rssi === null) return "—";
    if (rssi >= -60) return "Strong";
    if (rssi >= -75) return "Good";
    if (rssi >= -85) return "Fair";
    return "Weak";
  }

  function rssiClass(rssi: number | null): string {
    if (rssi === null) return "badge-ghost";
    if (rssi >= -60) return "badge-success";
    if (rssi >= -75) return "badge-info";
    if (rssi >= -85) return "badge-warning";
    return "badge-error";
  }

  function signalBadgeLabel(device: TapDeviceInfo): string {
    if (device.is_connected_to_os) return "Paired";
    if (device.seen_in_scan) return rssiLabel(device.rssi);
    return "Cached";
  }

  function signalBadgeClass(device: TapDeviceInfo): string {
    if (device.is_connected_to_os) return "badge-secondary";
    if (device.seen_in_scan) return rssiClass(device.rssi);
    return "badge-ghost";
  }

  function canConnect(device: TapDeviceInfo): boolean {
    return device.seen_in_scan;
  }

  const ROLES: DeviceRole[] = ["solo", "left", "right"];
</script>

<div class="mx-auto max-w-2xl space-y-6">
  <h1 class="text-2xl font-bold">Devices</h1>

  <!-- Dual profile needs a second device -->
  {#if showDualWarning}
    <div class="alert alert-warning">
      <span>The active profile requires two connected devices.</span>
    </div>
  {/if}

  <!-- Two devices connected but only a single-hand profile is active -->
  {#if showSingleSuggestion && !dismissedSingleSuggestion}
    <div class="alert alert-info">
      <span>You have two devices connected. Consider switching to a dual profile.</span>
      <div class="flex gap-2">
        <a href="/profiles" class="btn btn-sm btn-ghost">Go to Profiles</a>
        <button
          class="btn btn-sm btn-ghost"
          onclick={() => (dismissedSingleSuggestion = true)}
        >Dismiss</button>
      </div>
    </div>
  {/if}

  {#if isAndroid}
    <!-- ── Android paired-device section ────────────────────────────────── -->
    <section class="card bg-base-100 shadow">
      <div class="card-body gap-4">
        <div class="flex items-center justify-between">
          <h2 class="card-title text-base">Paired devices</h2>
          <button class="btn btn-primary btn-sm" onclick={handleAndroidScan} disabled={bleLoading}>
            {#if bleLoading}
              <span class="loading loading-spinner loading-xs"></span>
            {/if}
            Refresh
          </button>
        </div>

        {#if bleScanError}
          <div class="alert alert-warning text-sm">
            <span>{bleScanError}</span>
          </div>
        {/if}

        {#if androidConnectError}
          <div class="alert alert-error text-sm">
            <span>{androidConnectError}</span>
          </div>
        {/if}

        {#if bleDiscovered.length > 0}
          <div class="overflow-x-auto">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th>Device</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#each bleDiscovered.filter((d) => !connectedAddresses.has(d.address) && !pendingDevices.some((p) => p.address === d.address)) as device (device.address)}
                  <tr>
                    <td>
                      <div class="font-medium">{device.name ?? "Unknown"}</div>
                      <div class="font-mono text-xs text-base-content/50">{device.address}</div>
                    </td>
                    <td>
                      <button
                        class="btn btn-sm btn-success"
                        onclick={() => handleAndroidConnect(device.address, device.name)}
                        disabled={androidConnectingAddress === device.address}
                      >
                        {#if androidConnectingAddress === device.address}
                          <span class="loading loading-spinner loading-xs"></span>
                        {:else}
                          Connect
                        {/if}
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else if !bleLoading && !bleScanError}
          <p class="text-sm text-base-content/50">
            Tap "Refresh" to load paired devices. If your Tap doesn't appear,
            pair it via Android Bluetooth Settings first.
          </p>
        {/if}
      </div>
    </section>

    <!-- ── Android pending role assignment ──────────────────────────────── -->
    {#if pendingDevices.length > 0}
      <section class="card bg-base-100 shadow">
        <div class="card-body gap-4">
          <h2 class="card-title text-base">Assign role</h2>
          <p class="text-sm text-base-content/60">
            Choose a role for each connected device.
          </p>
          {#each pendingDevices as device (device.address)}
            <div class="flex flex-col gap-2 rounded-lg border border-base-300 p-3">
              <div>
                <span class="font-medium">{device.name ?? "Unknown"}</span>
                <span class="ml-2 font-mono text-xs text-base-content/50">{device.address}</span>
              </div>
              <div class="flex items-center gap-2">
                <div class="join">
                  {#each ROLES as role (role)}
                    <button
                      class="btn join-item btn-sm {pendingRoles[device.address] === role ? 'btn-primary' : 'btn-ghost'}"
                      onclick={() => selectPendingRole(device.address, role)}
                      disabled={isRoleTaken(role)}
                    >{role}</button>
                  {/each}
                </div>
                <button
                  class="btn btn-sm btn-success"
                  onclick={() => handleAssignRole(device.address, device.name)}
                  disabled={!pendingRoles[device.address]}
                >Assign</button>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <!-- ── Android connected devices ────────────────────────────────────── -->
    <section class="card bg-base-100 shadow">
      <div class="card-body gap-4">
        <h2 class="card-title text-base">Connected devices</h2>
        {#if deviceStore.connected.length === 0}
          <p class="text-sm text-base-content/50">No devices connected.</p>
        {:else}
          <div class="overflow-x-auto">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th>Device</th>
                  <th>Role</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#each deviceStore.connected as device (device.address)}
                  <tr>
                    <td>
                      <div class="font-medium">{device.name ?? "—"}</div>
                      <div class="font-mono text-xs text-base-content/50">{device.address}</div>
                    </td>
                    <td>
                      <div class="join">
                        {#each ROLES as role (role)}
                          <button
                            class="btn join-item btn-xs {device.role === role ? 'btn-primary' : 'btn-ghost'}"
                            onclick={() => handleAndroidReassign(device.address, role)}
                            disabled={device.role === role || isRoleTaken(role)}
                          >{role}</button>
                        {/each}
                      </div>
                    </td>
                    <td>
                      <button
                        class="btn btn-sm btn-error btn-outline"
                        onclick={() => (androidDisconnectConfirmAddress = device.address)}
                      >Disconnect</button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
      </div>
    </section>

    <!-- ── BLE debug log ─────────────────────────────────────────────────── -->
    {#if bleDebugLog.length > 0}
      <section class="card bg-base-100 shadow">
        <div class="card-body gap-2">
          <div class="flex items-center justify-between">
            <h2 class="card-title text-base">BLE debug log</h2>
            <button class="btn btn-xs btn-ghost" onclick={() => (bleDebugLog = [])}>Clear</button>
          </div>
          <div class="max-h-48 overflow-y-auto rounded bg-base-200 p-2">
            {#each bleDebugLog as line, i (i)}
              <p class="font-mono text-xs leading-5">{line}</p>
            {/each}
          </div>
        </div>
      </section>
    {/if}

  {:else}
    <!-- ── Desktop scan section ──────────────────────────────────────────── -->
    <section class="card bg-base-100 shadow">
      <div class="card-body gap-4">
        <div class="flex items-center justify-between">
          <h2 class="card-title text-base">Scan for devices</h2>
          <button
            class="btn btn-primary btn-sm"
            onclick={handleScan}
            disabled={scanning}
          >
            {#if scanning}
              <span class="loading loading-spinner loading-xs"></span>
              Scanning…
            {:else}
              Scan
            {/if}
          </button>
        </div>

        {#if scanError}
          <div class="alert alert-error text-sm">
            <span>{scanError}</span>
          </div>
        {/if}

        {#if availableDevices.length > 0}
          <div class="overflow-x-auto">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th>Device</th>
                  <th>Address</th>
                  <th>Signal</th>
                  <th>Role</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#each availableDevices as device (device.address)}
                  <tr>
                    <td class="font-medium">{device.name ?? "Unknown"}</td>
                    <td class="font-mono text-xs">{device.address}</td>
                    <td>
                      <span class="badge badge-sm {signalBadgeClass(device)}">
                        {signalBadgeLabel(device)}
                      </span>
                    </td>
                    <td>
                      <div class="join">
                        {#each ROLES as role (role)}
                          <button
                            class="btn join-item btn-xs
                              {pendingRole[device.address] === role
                              ? 'btn-primary'
                              : 'btn-ghost'}"
                            onclick={() => selectRole(device.address, role)}
                            disabled={!canConnect(device)}
                          >
                            {role}
                          </button>
                        {/each}
                      </div>
                    </td>
                    <td>
                      <button
                        class="btn btn-sm btn-success"
                        onclick={() => handleConnect(device.address)}
                        disabled={!pendingRole[device.address] ||
                          connectingAddress === device.address ||
                          !canConnect(device)}
                      >
                        {#if connectingAddress === device.address}
                          <span class="loading loading-spinner loading-xs"></span>
                        {:else}
                          Connect
                        {/if}
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {:else if !scanning && !scanError}
          <p class="text-sm text-base-content/50">
            Press Scan to search for nearby Tap devices.
          </p>
        {/if}

        {#if connectError}
          <div class="alert alert-error text-sm">
            <span>{connectError}</span>
          </div>
        {/if}
      </div>
    </section>

    <!-- ── Desktop connected devices ────────────────────────────────────── -->
    <section class="card bg-base-100 shadow">
      <div class="card-body gap-4">
        <h2 class="card-title text-base">Connected devices</h2>

        {#if renameNotice}
          <div class="alert alert-info text-sm">
            <span>{renameNotice}</span>
            <button class="btn btn-xs btn-ghost" onclick={() => (renameNotice = null)}>✕</button>
          </div>
        {/if}

        {#if deviceStore.connected.length === 0}
          <p class="text-sm text-base-content/50">No devices connected.</p>
        {:else}
          <div class="overflow-x-auto">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th>Device</th>
                  <th>Address</th>
                  <th>Role</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {#each deviceStore.connected as device (device.address)}
                  <tr>
                    <td class="font-medium">
                      {#if renamingAddress === device.address}
                        <div class="flex flex-col gap-1">
                          <div class="flex items-center gap-1">
                            <input
                              class="input input-xs input-bordered w-36"
                              type="text"
                              bind:value={renameInput}
                              maxlength={20}
                              onkeydown={(e) => {
                                if (e.key === "Enter") handleRename(device.address);
                                if (e.key === "Escape") cancelRename();
                              }}
                            />
                            <button
                              class="btn btn-xs btn-success"
                              onclick={() => handleRename(device.address)}
                              disabled={renamingInProgress}
                              title="Confirm"
                            >✓</button>
                            <button
                              class="btn btn-xs btn-ghost"
                              onclick={cancelRename}
                              disabled={renamingInProgress}
                              title="Cancel"
                            >✕</button>
                          </div>
                          {#if renameError}
                            <span class="text-xs text-error">{renameError}</span>
                          {/if}
                        </div>
                      {:else}
                        <span>{device.name ?? "—"}</span>
                        <button
                          class="btn btn-xs btn-ghost ml-1"
                          onclick={() => startRename(device.address, device.name)}
                          title="Rename device"
                        >✎</button>
                      {/if}
                    </td>
                    <td class="font-mono text-xs">{device.address}</td>
                    <td>
                      <div class="join">
                        {#each ROLES as role (role)}
                          <button
                            class="btn join-item btn-xs
                              {device.role === role ? 'btn-primary' : 'btn-ghost'}"
                            onclick={() => handleReassign(device.address, role)}
                            disabled={!canReassignTo(device.role, role) ||
                              reassigningAddress === device.address}
                          >
                            {role}
                          </button>
                        {/each}
                      </div>
                    </td>
                    <td>
                      <button
                        class="btn btn-sm btn-error btn-outline"
                        onclick={() => (disconnectConfirmRole = device.role)}
                        disabled={disconnectingRole === device.role ||
                          reassigningAddress === device.address}
                      >
                        {#if disconnectingRole === device.role}
                          <span class="loading loading-spinner loading-xs"></span>
                        {:else}
                          Disconnect
                        {/if}
                      </button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        {#if reassignError}
          <div class="alert alert-error text-sm">
            <span>{reassignError}</span>
          </div>
        {/if}
      </div>
    </section>
  {/if}
</div>

<!-- Desktop disconnect confirm modal -->
{#if disconnectConfirmRole}
  <dialog class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold">Disconnect {deviceStore.connected.find((d) => d.role === disconnectConfirmRole)?.name ?? disconnectConfirmRole}?</h3>
      <p class="py-4 text-sm">
        The device will exit controller mode and return to text-input mode.
      </p>
      <div class="modal-action">
        <button
          class="btn btn-ghost"
          onclick={() => (disconnectConfirmRole = null)}
        >
          Cancel
        </button>
        <button
          class="btn btn-error"
          onclick={() => {
            const role = disconnectConfirmRole!;
            disconnectConfirmRole = null;
            handleDisconnect(role);
          }}
        >
          Disconnect
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={() => (disconnectConfirmRole = null)}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}

<!-- Android disconnect confirm modal -->
{#if androidDisconnectConfirmAddress}
  {@const device = deviceStore.connected.find((d) => d.address === androidDisconnectConfirmAddress)}
  <dialog class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold">Disconnect {device?.name ?? androidDisconnectConfirmAddress}?</h3>
      <p class="py-4 text-sm">
        The device will exit controller mode and return to text-input mode.
      </p>
      <div class="modal-action">
        <button
          class="btn btn-ghost"
          onclick={() => (androidDisconnectConfirmAddress = null)}
        >
          Cancel
        </button>
        <button
          class="btn btn-error"
          onclick={() => {
            const addr = androidDisconnectConfirmAddress!;
            handleAndroidDisconnect(addr);
          }}
        >
          Disconnect
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={() => (androidDisconnectConfirmAddress = null)}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}
