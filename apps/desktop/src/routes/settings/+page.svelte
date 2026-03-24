<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    getPreferences,
    savePreferences,
    checkForUpdate,
    getPlatform,
    checkBatteryExemptionGranted,
    getAndroidPreferences,
    saveAndroidPreferences,
    getShizukuState,
  } from "$lib/commands";
  import type { TrayPreferences, AndroidPreferences } from "$lib/types";
  import type { ShizukuState } from "$lib/commands";
  import { updateStore } from "$lib/stores/updates.svelte";
  import BatterySetupWizard from "$lib/components/BatterySetupWizard.svelte";
  import ShizukuSetup from "$lib/components/ShizukuSetup.svelte";

  let prefs = $state<TrayPreferences>({
    close_to_tray: true,
    start_minimised: false,
    start_at_login: false,
    notify_device_connected: true,
    notify_device_disconnected: true,
    notify_layer_switch: false,
    notify_profile_switch: true,
    haptics_enabled: true,
    haptic_on_tap: false,
    haptic_on_layer_switch: true,
    haptic_on_profile_switch: true,
  });

  let loading = $state(true);
  let saved = $state(false);
  let error = $state<string | null>(null);
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  let checkingForUpdate = $state(false);
  let updateCheckResult = $state<"up-to-date" | "found" | null>(null);

  // Android-specific state.
  let isAndroid = $state(false);
  let batteryExemptionGranted = $state(false);
  let androidPrefs = $state<AndroidPreferences>({
    notify_device_connected: true,
    notify_device_disconnected: true,
    notify_layer_switch: false,
    notify_profile_switch: true,
    haptics_enabled: true,
    haptic_on_tap: false,
    haptic_on_layer_switch: true,
    haptic_on_profile_switch: true,
    accessibility_setup_done: false,
    battery_setup_done: false,
    auto_start_service: false,
  });
  let batteryWizardOpen = $state(false);
  let shizukuState = $state<ShizukuState>("NotRunning");
  let shizukuSetupOpen = $state(false);
  let shizukuStateTimer: ReturnType<typeof setInterval> | undefined;

  onMount(async () => {
    try {
      const platform = await getPlatform();
      if (platform === "android") {
        isAndroid = true;
        await refreshAndroidStatus();
        // Poll Shizuku state so the button badge reflects changes (startup race condition).
        shizukuStateTimer = setInterval(async () => {
          try {
            const r = await getShizukuState();
            shizukuState = r.state;
          } catch {
            // Non-fatal.
          }
        }, 2000);
      } else {
        try {
          prefs = await getPreferences();
        } catch (e) {
          error = String(e);
        }
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  onDestroy(() => {
    clearInterval(shizukuStateTimer);
  });

  async function refreshAndroidStatus() {
    try {
      const [batteryResult, ap] = await Promise.all([
        checkBatteryExemptionGranted(),
        getAndroidPreferences(),
      ]);
      batteryExemptionGranted = batteryResult.granted;
      androidPrefs = ap;
    } catch {
      // Non-fatal.
    }
    try {
      const r = await getShizukuState();
      shizukuState = r.state;
    } catch {
      // Non-fatal — shizukuState retains its current value.
    }
  }

  async function toggleAndroid(field: keyof AndroidPreferences) {
    error = null;
    const prev = androidPrefs[field] as boolean;
    androidPrefs = { ...androidPrefs, [field]: !prev };
    try {
      await saveAndroidPreferences(androidPrefs);
      flashSaved();
    } catch (e) {
      androidPrefs = { ...androidPrefs, [field]: prev };
      error = String(e);
    }
  }

  async function toggle(field: keyof TrayPreferences) {
    error = null;
    prefs[field] = !prefs[field];
    try {
      await savePreferences({ ...prefs });
      flashSaved();
    } catch (e) {
      // Revert on failure.
      prefs[field] = !prefs[field];
      error = String(e);
    }
  }

  function flashSaved() {
    saved = true;
    clearTimeout(savedTimer);
    savedTimer = setTimeout(() => (saved = false), 1500);
  }

  async function manualCheckForUpdate() {
    checkingForUpdate = true;
    updateCheckResult = null;
    try {
      const info = await checkForUpdate();
      if (info) {
        updateStore.setAvailable(info);
        updateCheckResult = "found";
      } else {
        updateCheckResult = "up-to-date";
      }
    } catch {
      updateCheckResult = null;
    } finally {
      checkingForUpdate = false;
    }
  }
</script>

<div class="max-w-2xl m-auto space-y-8">
  <div class="flex items-center gap-3">
    <h1 class="text-2xl font-bold">Settings</h1>
    {#if saved}
      <span class="badge badge-success badge-sm">Saved</span>
    {/if}
  </div>

  {#if error}
    <div class="alert alert-error">
      <span>{error}</span>
    </div>
  {/if}

  {#if loading}
    <div class="loading loading-spinner"></div>
  {:else}
    <!-- ── Desktop-only sections ─────────────────────────────────────────────── -->
    {#if !isAndroid}
      <!-- Window behaviour -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
          Window behaviour
        </h2>

        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <!-- close_to_tray -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Minimise to tray when closed</p>
                <p class="text-sm text-base-content/60">
                  Closing the window keeps tap-mapper running in the background. Use the tray icon
                  to show it again.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.close_to_tray}
                onchange={() => toggle("close_to_tray")}
              />
            </label>

            <div class="divider my-0"></div>

            <!-- start_minimised -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Start minimised</p>
                <p class="text-sm text-base-content/60">
                  Launch directly to the tray without showing the main window.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.start_minimised}
                onchange={() => toggle("start_minimised")}
              />
            </label>
          </div>
        </div>
      </section>

      <!-- Notifications -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
          Notifications
        </h2>

        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <!-- notify_device_connected -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Device connected</p>
                <p class="text-sm text-base-content/60">Notify when a Tap device connects.</p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.notify_device_connected}
                onchange={() => toggle("notify_device_connected")}
              />
            </label>

            <div class="divider my-0"></div>

            <!-- notify_device_disconnected -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Device disconnected</p>
                <p class="text-sm text-base-content/60">Notify when a Tap device disconnects.</p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.notify_device_disconnected}
                onchange={() => toggle("notify_device_disconnected")}
              />
            </label>

            <div class="divider my-0"></div>

            <!-- notify_layer_switch -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Layer switched</p>
                <p class="text-sm text-base-content/60">
                  Notify when the active layer changes. Off by default to avoid noise with frequent
                  layer switches.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.notify_layer_switch}
                onchange={() => toggle("notify_layer_switch")}
              />
            </label>

            <div class="divider my-0"></div>

            <!-- notify_profile_switch -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Profile switched</p>
                <p class="text-sm text-base-content/60">Notify when the active profile changes.</p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.notify_profile_switch}
                onchange={() => toggle("notify_profile_switch")}
              />
            </label>
          </div>
        </div>
      </section>

      <!-- Haptics -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">Haptics</h2>

        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <!-- haptics_enabled -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Enable haptic feedback</p>
                <p class="text-sm text-base-content/60">
                  Allow tap-mapper to send vibration patterns to connected Tap devices. Disabling
                  this overrides all per-event toggles below.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.haptics_enabled}
                onchange={() => toggle("haptics_enabled")}
              />
            </label>

            <div class="divider my-0"></div>

            <!-- per-event toggles (greyed when master is off) -->
            <div
              class:opacity-40={!prefs.haptics_enabled}
              class:pointer-events-none={!prefs.haptics_enabled}
              class="space-y-4"
            >
              <!-- haptic_on_tap -->
              <label class="flex cursor-pointer items-start justify-between gap-4">
                <div>
                  <p class="font-medium">Vibrate on tap</p>
                  <p class="text-sm text-base-content/60">
                    Short pulse on every resolved tap gesture — confirms the device registered it.
                    Off by default to avoid buzz on every keystroke.
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary mt-0.5 flex-shrink-0"
                  checked={prefs.haptic_on_tap}
                  onchange={() => toggle("haptic_on_tap")}
                />
              </label>

              <div class="divider my-0"></div>

              <!-- haptic_on_layer_switch -->
              <label class="flex cursor-pointer items-start justify-between gap-4">
                <div>
                  <p class="font-medium">Vibrate on layer switch</p>
                  <p class="text-sm text-base-content/60">
                    Double pulse when the active layer changes.
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary mt-0.5 flex-shrink-0"
                  checked={prefs.haptic_on_layer_switch}
                  onchange={() => toggle("haptic_on_layer_switch")}
                />
              </label>

              <div class="divider my-0"></div>

              <!-- haptic_on_profile_switch -->
              <label class="flex cursor-pointer items-start justify-between gap-4">
                <div>
                  <p class="font-medium">Vibrate on profile switch</p>
                  <p class="text-sm text-base-content/60">
                    Triple pulse when the active profile changes.
                  </p>
                </div>
                <input
                  type="checkbox"
                  class="toggle toggle-primary mt-0.5 flex-shrink-0"
                  checked={prefs.haptic_on_profile_switch}
                  onchange={() => toggle("haptic_on_profile_switch")}
                />
              </label>
            </div>
          </div>
        </div>
      </section>

      <!-- System -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">System</h2>

        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <!-- start_at_login -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Start at system login</p>
                <p class="text-sm text-base-content/60">
                  Automatically launch tap-mapper when you log in to your computer.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={prefs.start_at_login}
                onchange={() => toggle("start_at_login")}
              />
            </label>
          </div>
        </div>
      </section>
    {/if}
    <!-- ── End desktop-only sections ────────────────────────────────────────── -->

    <!-- ── Android-only sections ──────────────────────────────────────────── -->
    {#if isAndroid}
      <!-- Keyboard Mode -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
          Keyboard Mode
        </h2>
        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <div class="flex items-start justify-between gap-4">
              <div>
                <p class="font-medium">Full keyboard emulation</p>
                <p class="text-sm text-base-content/60">
                  Injects keystrokes via Shizuku — works in games, browsers, and all apps.
                  Requires Android 11 and a one-time Shizuku setup.
                </p>
                <p class="mt-1 text-sm">
                  {#if shizukuState === "Active"}
                    <span class="text-success font-medium">✓ Active</span>
                  {:else if shizukuState === "Binding" || shizukuState === "Reconnecting"}
                    <span class="text-info font-medium">Connecting…</span>
                  {:else if shizukuState === "Unsupported"}
                    <span class="text-error font-medium">Requires Android 11+</span>
                  {:else if shizukuState === "NotInstalled"}
                    <span class="text-warning font-medium">Not installed</span>
                  {:else if shizukuState === "NotRunning"}
                    <span class="text-warning font-medium">Not running</span>
                  {:else}
                    <span class="text-warning font-medium">Permission required</span>
                  {/if}
                </p>
              </div>
              <button
                class="btn btn-outline btn-sm flex-shrink-0"
                onclick={() => (shizukuSetupOpen = true)}
                disabled={shizukuState === "Unsupported"}
                title={shizukuState === "Unsupported" ? "Requires Android 11+" : undefined}
              >
                {shizukuState === "Active" ? "View" : "Set up"}
              </button>
            </div>

            {#if shizukuState !== "Active" && shizukuState !== "Unsupported"}
              <div class="rounded bg-base-200 p-3 text-xs text-base-content/60">
                <p>
                  <strong>After a device restart:</strong> Shizuku auto-starts via Wireless Debugging.
                  MapXr reconnects automatically when you open the app.
                </p>
              </div>
            {/if}
          </div>
        </div>
      </section>

      <!-- Background operation -->
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
          Background operation
        </h2>
        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-4 p-4">
            <div class="flex items-start justify-between gap-4">
              <div>
                <p class="font-medium">Battery optimisation</p>
                <p class="text-sm text-base-content/60">
                  Disabling battery optimisation prevents Android from killing MapXr when it runs in
                  the background.
                </p>
                <p class="mt-1 text-sm">
                  {#if batteryExemptionGranted}
                    <span class="text-success font-medium">✓ Optimisation disabled</span>
                  {:else}
                    <span class="text-warning font-medium">Optimisation still active</span>
                  {/if}
                </p>
              </div>
              <button
                class="btn btn-outline btn-sm flex-shrink-0"
                onclick={() => (batteryWizardOpen = true)}
              >
                {androidPrefs.battery_setup_done ? "Re-run wizard" : "Set up"}
              </button>
            </div>

            <div class="divider my-0"></div>

            <!-- Auto-start service toggle -->
            <label class="flex cursor-pointer items-start justify-between gap-4">
              <div>
                <p class="font-medium">Auto-start background service</p>
                <p class="text-sm text-base-content/60">
                  Start the foreground service automatically when MapXr opens.
                </p>
              </div>
              <input
                type="checkbox"
                class="toggle toggle-primary mt-0.5 flex-shrink-0"
                checked={androidPrefs.auto_start_service}
                onchange={() => toggleAndroid("auto_start_service")}
              />
            </label>
          </div>
        </div>
      </section>
    {/if}
    <!-- ── End Android sections ──────────────────────────────────────────────── -->

    <!-- Updates (desktop only — check_for_update command not available on Android) -->
    {#if !isAndroid}
      <section class="space-y-4">
        <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">Updates</h2>

        <div class="card bg-base-100 shadow-sm">
          <div class="card-body gap-3 p-4">
            <div class="flex items-center justify-between gap-4">
              <div>
                <p class="font-medium">Check for updates</p>
                <p class="text-sm text-base-content/60">
                  MapXr checks automatically on launch and every 24 hours.
                </p>
              </div>
              <button
                class="btn btn-outline btn-sm flex-shrink-0"
                onclick={manualCheckForUpdate}
                disabled={checkingForUpdate}
              >
                {#if checkingForUpdate}
                  <span class="loading loading-spinner loading-xs"></span>
                  Checking…
                {:else}
                  Check now
                {/if}
              </button>
            </div>

            {#if updateCheckResult === "up-to-date"}
              <p class="text-sm text-success">MapXr is up to date.</p>
            {:else if updateCheckResult === "found" && updateStore.available}
              <p class="text-sm text-info">
                Version {updateStore.available.version} is available — see the banner above.
              </p>
            {/if}
          </div>
        </div>
      </section>
    {/if}
  {/if}
</div>

<!-- Android-only modals -->
<ShizukuSetup
  open={shizukuSetupOpen}
  onClose={() => (shizukuSetupOpen = false)}
  onDone={async () => {
    shizukuSetupOpen = false;
    await refreshAndroidStatus();
  }}
/>
<BatterySetupWizard
  open={batteryWizardOpen}
  onClose={() => (batteryWizardOpen = false)}
  onDone={async () => {
    batteryWizardOpen = false;
    await refreshAndroidStatus();
  }}
/>
