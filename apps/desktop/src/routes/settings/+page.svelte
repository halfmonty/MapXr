<script lang="ts">
  import { onMount } from "svelte";
  import { getPreferences, savePreferences, checkForUpdate } from "$lib/commands";
  import type { TrayPreferences } from "$lib/types";
  import { updateStore } from "$lib/stores/updates.svelte";

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

  onMount(async () => {
    try {
      prefs = await getPreferences();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

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

<div class="max-w-lg space-y-8">
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
                Closing the window keeps tap-mapper running in the background.
                Use the tray icon to show it again.
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
                Notify when the active layer changes. Off by default to avoid noise with
                frequent layer switches.
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
      <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
        Haptics
      </h2>

      <div class="card bg-base-100 shadow-sm">
        <div class="card-body gap-4 p-4">
          <!-- haptics_enabled -->
          <label class="flex cursor-pointer items-start justify-between gap-4">
            <div>
              <p class="font-medium">Enable haptic feedback</p>
              <p class="text-sm text-base-content/60">
                Allow tap-mapper to send vibration patterns to connected Tap devices.
                Disabling this overrides all per-event toggles below.
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
          <div class:opacity-40={!prefs.haptics_enabled} class:pointer-events-none={!prefs.haptics_enabled} class="space-y-4">

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
                <p class="text-sm text-base-content/60">Double pulse when the active layer changes.</p>
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
                <p class="text-sm text-base-content/60">Triple pulse when the active profile changes.</p>
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
      <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
        System
      </h2>

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
    <!-- Updates -->
    <section class="space-y-4">
      <h2 class="text-sm font-semibold uppercase tracking-wider text-base-content/50">
        Updates
      </h2>

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
</div>
