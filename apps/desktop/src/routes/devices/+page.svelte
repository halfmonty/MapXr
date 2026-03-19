<script lang="ts">
  import { scanDevices, connectDevice, disconnectDevice, reassignDeviceRole } from "$lib/commands";
  import { deviceStore } from "$lib/stores/device.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { logger } from "$lib/logger";
  import type { TapDeviceInfo } from "$lib/types";

  // ── Scan state ──────────────────────────────────────────────────────────────

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
      const msg = e instanceof Error ? e.message : String(e);
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

  // ── Connect state ───────────────────────────────────────────────────────────

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
      const { [address]: _, ...rest } = pendingRole;
      pendingRole = rest;
    } catch (e) {
      connectError = e instanceof Error ? e.message : String(e);
      logger.error("connect_device failed", e);
    } finally {
      connectingAddress = null;
    }
  }

  // ── Reassign state ──────────────────────────────────────────────────────────

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

  // ── Disconnect state ────────────────────────────────────────────────────────

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

  // ── Signal strength label ───────────────────────────────────────────────────

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

  <!-- Scan section -->
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
              {#each availableDevices as device}
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
                      {#each ["solo", "left", "right"] as role}
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

  <!-- Connected devices section -->
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
                <th>Address</th>
                <th>Role</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {#each deviceStore.connected as device}
                <tr>
                  <td class="font-medium">{device.name ?? "—"}</td>
                  <td class="font-mono text-xs">{device.address}</td>
                  <td>
                    <div class="join">
                      {#each ["solo", "left", "right"] as role}
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
</div>

<!-- Disconnect confirm modal (task 5.11) -->
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
