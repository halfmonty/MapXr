<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    getShizukuState,
    requestShizukuPermission,
    openShizukuApp,
  } from "$lib/commands";
  import type { ShizukuState } from "$lib/commands";

  interface Props {
    open: boolean;
    onClose: () => void;
    onDone: () => void;
  }

  let { open, onClose, onDone }: Props = $props();

  let state = $state<ShizukuState>("NotRunning");
  let polling = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    if (open) {
      startPolling();
    } else {
      stopPolling();
    }
  });

  onDestroy(() => stopPolling());

  function startPolling() {
    if (polling) return;
    polling = true;
    void fetchState();
    pollTimer = setInterval(() => void fetchState(), 1000);
  }

  function stopPolling() {
    polling = false;
    clearInterval(pollTimer);
    pollTimer = undefined;
  }

  async function fetchState() {
    try {
      const r = await getShizukuState();
      state = r.state;
      if (state === "Active") {
        stopPolling();
      }
    } catch {
      // Non-fatal.
    }
  }

  async function handleGrantPermission() {
    try {
      await requestShizukuPermission();
    } catch {
      // Result arrives via state poll.
    }
  }

  async function handleOpenShizuku() {
    try {
      await openShizukuApp();
    } catch {
      // Non-fatal.
    }
  }

  function handleDone() {
    stopPolling();
    onDone();
  }

  function handleClose() {
    stopPolling();
    onClose();
  }

  /** True while Shizuku is in a transitional state. */
  const isSpinning = $derived(state === "Binding" || state === "Reconnecting");
</script>

{#if open}
  <dialog class="modal modal-open">
    <div class="modal-box max-w-sm">
      <h3 class="font-bold text-lg mb-4">Set up Shizuku</h3>

      {#if state === "Active"}
        <!-- ── Active ─────────────────────────────────────────────────────── -->
        <div class="flex flex-col items-center gap-4 py-4">
          <div class="text-success text-4xl">✓</div>
          <p class="font-medium text-success">Shizuku is active</p>
          <p class="text-sm text-base-content/60 text-center">
            MapXr can now inject keystrokes and mouse events into any app.
          </p>
        </div>
        <div class="modal-action">
          <button class="btn btn-primary" onclick={handleDone}>Done</button>
        </div>

      {:else if state === "NotInstalled"}
        <!-- ── Step 1: Install ────────────────────────────────────────────── -->
        <div class="steps steps-vertical mb-6">
          <div class="step step-primary">Install Shizuku</div>
          <div class="step">Start Shizuku</div>
          <div class="step">Grant permission</div>
        </div>
        <p class="text-sm text-base-content/60 mb-4">
          Shizuku is a free app (~6 MB) that gives MapXr the permissions needed to inject
          keyboard and mouse input into other apps.
        </p>
        <div class="flex gap-2">
          <button class="btn btn-primary flex-1" onclick={handleOpenShizuku}>
            Open Play Store
          </button>
          <button class="btn btn-outline flex-1" onclick={() => void fetchState()}>
            Already installed
          </button>
        </div>
        <div class="modal-action">
          <button class="btn btn-ghost btn-sm" onclick={handleClose}>Cancel</button>
        </div>

      {:else if state === "NotRunning"}
        <!-- ── Step 2: Start ──────────────────────────────────────────────── -->
        <div class="steps steps-vertical mb-6">
          <div class="step step-primary">Install Shizuku</div>
          <div class="step step-primary">Start Shizuku</div>
          <div class="step">Grant permission</div>
        </div>
        <p class="text-sm text-base-content/60 mb-2">
          Open the Shizuku app and tap <strong>Start via Wireless Debugging</strong>.
        </p>
        <p class="text-sm text-base-content/60 mb-4">
          After the first start, Shizuku auto-starts on every reboot — no repeated setup needed.
        </p>
        <button class="btn btn-outline w-full mb-2" onclick={handleOpenShizuku}>
          Open Shizuku
        </button>
        <p class="text-xs text-base-content/40 text-center">Waiting for Shizuku to start…</p>
        <div class="modal-action">
          <button class="btn btn-ghost btn-sm" onclick={handleClose}>Cancel</button>
        </div>

      {:else if state === "PermissionRequired"}
        <!-- ── Step 3: Permit ─────────────────────────────────────────────── -->
        <div class="steps steps-vertical mb-6">
          <div class="step step-primary">Install Shizuku</div>
          <div class="step step-primary">Start Shizuku</div>
          <div class="step step-primary">Grant permission</div>
        </div>
        <p class="text-sm text-base-content/60 mb-4">
          Tap the button below to grant MapXr permission in the Shizuku app.
        </p>
        <button class="btn btn-primary w-full" onclick={handleGrantPermission}>
          Grant permission
        </button>
        <div class="modal-action">
          <button class="btn btn-ghost btn-sm" onclick={handleClose}>Cancel</button>
        </div>

      {:else if isSpinning}
        <!-- ── Binding / Reconnecting ─────────────────────────────────────── -->
        <div class="flex flex-col items-center gap-4 py-6">
          <span class="loading loading-spinner loading-lg"></span>
          <p class="text-sm text-base-content/60">
            {state === "Reconnecting" ? "Reconnecting…" : "Binding service…"}
          </p>
        </div>
        <div class="modal-action">
          <button class="btn btn-ghost btn-sm" onclick={handleClose}>Cancel</button>
        </div>

      {:else}
        <!-- Fallback (Unsupported or unexpected state) -->
        <p class="text-sm text-base-content/60">
          Shizuku is not supported on this device (requires Android 11+).
        </p>
        <div class="modal-action">
          <button class="btn" onclick={handleClose}>Close</button>
        </div>
      {/if}
    </div>

    <!-- Backdrop close -->
    <button class="modal-backdrop" onclick={handleClose} aria-label="Close">
    </button>
  </dialog>
{/if}
