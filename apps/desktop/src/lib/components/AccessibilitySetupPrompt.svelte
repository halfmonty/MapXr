<script lang="ts">
  import { onMount } from "svelte";
  import {
    checkAccessibilityEnabled,
    openAccessibilitySettings,
    getAndroidPreferences,
    saveAndroidPreferences,
  } from "$lib/commands";

  interface Props {
    open: boolean;
    onClose: () => void;
    /** Called when the user completes or explicitly dismisses the prompt. */
    onDone?: () => void;
  }

  let { open, onClose, onDone }: Props = $props();

  let enabled = $state(false);
  let loading = $state(false);
  let checked = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    await checkStatus();
  });

  async function checkStatus() {
    try {
      const result = await checkAccessibilityEnabled();
      enabled = result.enabled;
      checked = true;
    } catch (e) {
      error = `Could not check accessibility status: ${e}`;
    }
  }

  async function handleOpenSettings() {
    loading = true;
    error = null;
    try {
      await openAccessibilitySettings();
      // Give the user a moment to return from the settings screen.
      await new Promise((r) => setTimeout(r, 2000));
      await checkStatus();
    } catch (e) {
      error = `Could not open settings: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function handleDone() {
    try {
      const prefs = await getAndroidPreferences();
      await saveAndroidPreferences({ ...prefs, accessibility_setup_done: true });
    } catch (_) {
      // Non-fatal — completion is advisory.
    }
    onDone?.();
    onClose();
  }
</script>

<dialog class="modal" class:modal-open={open}>
  <div class="modal-box w-[480px] max-w-full">

    <!-- Header -->
    <h2 class="text-lg font-bold">Accessibility permission</h2>

    {#if error}
      <div class="alert alert-error mt-3 text-sm">
        <span>{error}</span>
      </div>
    {/if}

    <div class="mt-4 space-y-3 text-sm text-base-content/80">
      <p>
        MapXr needs the Accessibility permission to forward your Tap Strap gestures as
        keystrokes and actions in other apps on your phone.
      </p>
      <p>
        Tap <strong>Open Accessibility Settings</strong>, find
        <strong>MapXr</strong> in the list, and switch it on.
      </p>

      {#if checked}
        {#if enabled}
          <div class="alert alert-success text-sm">
            <span>✓ Accessibility permission is enabled.</span>
          </div>
        {:else}
          <div class="alert alert-warning text-sm">
            <span>Accessibility permission is not yet enabled.</span>
          </div>
        {/if}
      {/if}

      <div class="rounded bg-base-200 p-3 text-xs text-base-content/60">
        <strong>Privacy note:</strong> MapXr's accessibility service is configured with
        <code>typeNone</code> — it does not read screen content, observe window titles, or
        monitor any app activity. It only receives the key injection calls you trigger with
        your Tap Strap.
      </div>
    </div>

    <div class="modal-action flex-wrap gap-2">
      <button class="btn btn-ghost btn-sm" onclick={onClose}>Skip for now</button>
      {#if !enabled}
        <button
          class="btn btn-secondary btn-sm"
          onclick={handleOpenSettings}
          disabled={loading}
        >
          {#if loading}
            <span class="loading loading-spinner loading-xs"></span>
          {/if}
          Open Accessibility Settings
        </button>
      {/if}
      <button class="btn btn-primary btn-sm" onclick={handleDone}>
        {enabled ? "Done" : "Skip this step"}
      </button>
    </div>

  </div>

  <!-- Backdrop click dismisses. -->
  <form method="dialog" class="modal-backdrop">
    <button onclick={onClose}>close</button>
  </form>
</dialog>
