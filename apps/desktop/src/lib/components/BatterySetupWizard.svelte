<script lang="ts">
  import { onMount } from "svelte";
  import {
    getOemInfo,
    checkBatteryExemptionGranted,
    requestBatteryExemption,
    openOemBatterySettings,
    getAndroidPreferences,
    saveAndroidPreferences,
  } from "$lib/commands";
  import type { OemInfo } from "$lib/types";

  interface Props {
    open: boolean;
    onClose: () => void;
    /** Called when the user completes or dismisses the wizard. */
    onDone?: () => void;
  }

  let { open, onClose, onDone }: Props = $props();

  // Wizard state.
  type Step = "why" | "exemption" | "oem" | "done";
  let step = $state<Step>("why");
  let oemInfo = $state<OemInfo | null>(null);
  let exemptionGranted = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    await loadOemInfo();
  });

  async function loadOemInfo() {
    try {
      oemInfo = await getOemInfo();
      exemptionGranted = oemInfo.exemptionGranted;
    } catch (e) {
      error = `Could not load device info: ${e}`;
    }
  }

  async function handleRequestExemption() {
    loading = true;
    error = null;
    try {
      await requestBatteryExemption();
      // The intent was launched. The result is not known until the user returns.
      // A short delay allows the user to action the dialog before we re-check.
      await new Promise((r) => setTimeout(r, 1500));
      const result = await checkBatteryExemptionGranted();
      exemptionGranted = result.granted;
    } catch (e) {
      error = `Request failed: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function handleOpenOemSettings() {
    loading = true;
    error = null;
    try {
      await openOemBatterySettings();
    } catch (e) {
      error = `Could not open settings: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function handleDone() {
    try {
      // Record wizard completion in preferences.json.
      const prefs = await getAndroidPreferences();
      await saveAndroidPreferences({ ...prefs, battery_setup_done: true });
    } catch (_) {
      // Non-fatal — completion is advisory.
    }
    onDone?.();
    onClose();
  }

  function next() {
    if (step === "why") {
      step = "exemption";
    } else if (step === "exemption") {
      step = oemInfo?.hasOemStep ? "oem" : "done";
    } else if (step === "oem") {
      step = "done";
    }
  }

  function stepLabel(s: Step): string {
    const steps: Step[] = oemInfo?.hasOemStep
      ? ["why", "exemption", "oem", "done"]
      : ["why", "exemption", "done"];
    const idx = steps.indexOf(s);
    return idx >= 0 ? `${idx + 1} / ${steps.length}` : "";
  }
</script>

<dialog class="modal" class:modal-open={open}>
  <div class="modal-box w-[480px] max-w-full">

    <!-- Header -->
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-bold">Background operation setup</h2>
      {#if oemInfo}
        <span class="text-xs text-base-content/50">{stepLabel(step)}</span>
      {/if}
    </div>

    {#if error}
      <div class="alert alert-error mt-3 text-sm">
        <span>{error}</span>
      </div>
    {/if}

    <!-- ── Step 1: Why this is needed ──────────────────────────────────────── -->
    {#if step === "why"}
      <div class="mt-4 space-y-3 text-sm text-base-content/80">
        <p>
          MapXr must run in the background to keep your Tap Strap connected and responsive.
          Without the right settings, Android may kill the app after a few minutes.
        </p>
        <p>
          This wizard takes about 2 minutes and guides you through two steps:
        </p>
        <ol class="ml-4 list-decimal space-y-1">
          <li>Disable battery optimisation for MapXr (built-in Android setting).</li>
          {#if oemInfo?.hasOemStep}
            <li>Enable an additional setting specific to your <strong>{oemInfo.displayName}</strong> device.</li>
          {/if}
        </ol>
      </div>
      <div class="modal-action">
        <button class="btn btn-ghost btn-sm" onclick={onClose}>Skip for now</button>
        <button class="btn btn-primary btn-sm" onclick={next}>Get started</button>
      </div>

    <!-- ── Step 2: Battery exemption ───────────────────────────────────────── -->
    {:else if step === "exemption"}
      <div class="mt-4 space-y-3 text-sm text-base-content/80">
        <p>
          Tap <strong>Disable optimisation</strong> in the system dialog to let MapXr run
          without Android restricting its battery usage.
        </p>
        {#if exemptionGranted}
          <div class="alert alert-success text-sm">
            <span>✓ Battery optimisation is disabled for MapXr.</span>
          </div>
        {:else}
          <div class="alert alert-warning text-sm">
            <span>Battery optimisation is currently <strong>enabled</strong> for MapXr.</span>
          </div>
        {/if}
      </div>
      <div class="modal-action flex-wrap gap-2">
        <button class="btn btn-ghost btn-sm" onclick={onClose}>Skip</button>
        {#if !exemptionGranted}
          <button
            class="btn btn-secondary btn-sm"
            onclick={handleRequestExemption}
            disabled={loading}
          >
            {#if loading}
              <span class="loading loading-spinner loading-xs"></span>
            {/if}
            Disable optimisation
          </button>
        {/if}
        <button class="btn btn-primary btn-sm" onclick={next}>
          {exemptionGranted ? "Continue" : "Skip this step"}
        </button>
      </div>

    <!-- ── Step 3: OEM-specific ─────────────────────────────────────────────── -->
    {:else if step === "oem" && oemInfo?.hasOemStep}
      <div class="mt-4 space-y-3 text-sm text-base-content/80">
        <p>
          Your <strong>{oemInfo.displayName}</strong> device has additional battery restrictions
          that require a manual setting:
        </p>
        <div class="rounded bg-base-200 p-3 text-sm">
          {oemInfo.oemInstructions}
        </div>
        <p class="text-xs text-base-content/60">
          Tap <em>Go to settings</em> to open the relevant screen. Return to MapXr when done.
        </p>
      </div>
      <div class="modal-action flex-wrap gap-2">
        <button class="btn btn-ghost btn-sm" onclick={onClose}>Skip</button>
        <button
          class="btn btn-secondary btn-sm"
          onclick={handleOpenOemSettings}
          disabled={loading}
        >
          {#if loading}
            <span class="loading loading-spinner loading-xs"></span>
          {/if}
          Go to settings
        </button>
        <button class="btn btn-primary btn-sm" onclick={next}>Done</button>
      </div>

    <!-- ── Step 4: Done ──────────────────────────────────────────────────────── -->
    {:else if step === "done"}
      <div class="mt-4 space-y-3 text-sm text-base-content/80">
        <p class="text-base font-semibold">Setup complete ✓</p>
        <p>
          MapXr is now configured to run in the background. Your Tap Strap will stay connected
          even when you switch to other apps.
        </p>
        {#if !exemptionGranted}
          <div class="alert alert-warning text-sm">
            <span>
              Battery optimisation was not disabled. MapXr may disconnect after a few minutes
              in the background on some devices. You can re-run this wizard from Settings.
            </span>
          </div>
        {/if}
      </div>
      <div class="modal-action">
        <button class="btn btn-primary btn-sm" onclick={handleDone}>
          Finish
        </button>
      </div>
    {/if}

  </div>

  <!-- Backdrop click dismisses (except on the "done" step). -->
  {#if step !== "done"}
    <form method="dialog" class="modal-backdrop">
      <button onclick={onClose}>close</button>
    </form>
  {/if}
</dialog>
