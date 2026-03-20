<script lang="ts">
  import { updateStore } from "$lib/stores/updates.svelte";

  interface Props {
    onViewUpdate: () => void;
  }

  let { onViewUpdate }: Props = $props();
</script>

{#if updateStore.shouldShow && updateStore.available}
  <div
    class="flex items-center justify-between gap-4 border-b border-info/30 bg-info/10 px-4 py-2 text-sm"
  >
    <span class="text-info-content">
      MapXr <strong>{updateStore.available.version}</strong> is available.
    </span>
    <div class="flex items-center gap-2 flex-shrink-0">
      <button class="btn btn-info btn-xs" onclick={onViewUpdate}>View update</button>
      <button
        class="btn btn-ghost btn-xs"
        onclick={() => updateStore.dismiss()}
        aria-label="Dismiss update notification"
      >✕</button>
    </div>
  </div>
{/if}
