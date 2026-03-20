<script lang="ts">
  import { updateStore } from "$lib/stores/updates.svelte";
  import { downloadAndInstallUpdate } from "$lib/commands";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let error = $state<string | null>(null);

  async function install() {
    error = null;
    updateStore.downloading = true;
    updateStore.progress = null;
    try {
      // Never resolves on success — app restarts.
      await downloadAndInstallUpdate();
    } catch (e) {
      error = String(e);
      updateStore.resetDownload();
    }
  }

  let progressPercent = $derived(
    updateStore.progress && updateStore.progress.total
      ? Math.round((updateStore.progress.downloaded / updateStore.progress.total) * 100)
      : null,
  );

  let downloadedMb = $derived(
    updateStore.progress
      ? (updateStore.progress.downloaded / 1_048_576).toFixed(1)
      : null,
  );

  let totalMb = $derived(
    updateStore.progress?.total
      ? (updateStore.progress.total / 1_048_576).toFixed(1)
      : null,
  );
</script>

{#if updateStore.available}
  <dialog class="modal" class:modal-open={open}>
    <div class="modal-box w-[480px] max-w-full">
      <h2 class="text-lg font-bold">
        MapXr {updateStore.available.version} available
      </h2>

      {#if updateStore.available.release_notes}
        <div class="mt-3 max-h-48 overflow-y-auto rounded bg-base-200 p-3">
          <pre class="whitespace-pre-wrap font-sans text-sm text-base-content/80"
            >{updateStore.available.release_notes}</pre
          >
        </div>
      {/if}

      {#if error}
        <div class="alert alert-error mt-3 text-sm">
          <span>{error}</span>
        </div>
      {/if}

      {#if updateStore.downloading}
        <div class="mt-4 space-y-1">
          <div class="flex justify-between text-xs text-base-content/60">
            <span>Downloading…</span>
            {#if downloadedMb && totalMb}
              <span>{downloadedMb} / {totalMb} MB</span>
            {:else if downloadedMb}
              <span>{downloadedMb} MB</span>
            {/if}
          </div>
          <progress
            class="progress progress-info w-full"
            value={progressPercent}
            max="100"
          ></progress>
        </div>
      {/if}

      <div class="modal-action">
        <button
          class="btn btn-ghost btn-sm"
          onclick={onClose}
          disabled={updateStore.downloading}
        >
          Not now
        </button>
        <button
          class="btn btn-primary btn-sm"
          onclick={install}
          disabled={updateStore.downloading}
        >
          {#if updateStore.downloading}
            <span class="loading loading-spinner loading-xs"></span>
            Installing…
          {:else}
            Install & Restart
          {/if}
        </button>
      </div>
    </div>

    <!-- Backdrop click closes the dialog (unless downloading) -->
    <form method="dialog" class="modal-backdrop">
      <button onclick={onClose} disabled={updateStore.downloading}>close</button>
    </form>
  </dialog>
{/if}
