<script lang="ts">
  import type { DevlogEntry } from '../../data/devlog.js';
  import { base } from '$app/paths';

  interface Props {
    entry: DevlogEntry;
  }

  let { entry }: Props = $props();

  let pct = $derived(Math.round((entry.epicsDone / entry.totalEpics) * 100));
</script>

<a href={`${base}/devlog/${entry.slug}`} class="block mb-4 group">
  <div class="card bg-base-200 shadow-sm group-hover:shadow-md transition-shadow cursor-pointer">
    <div class="card-body">
      <div class="flex flex-wrap items-start justify-between gap-2">
        <div>
          <p class="text-sm text-base-content/50 font-mono mb-1">{entry.date}</p>
          <h3 class="card-title text-lg group-hover:text-primary transition-colors">{entry.title}</h3>
        </div>
        <div class="flex flex-col items-end gap-1 shrink-0">
          <span class="text-xs text-base-content/50">{entry.epicsDone}/{entry.totalEpics} epics</span>
          <progress class="progress progress-primary w-32" value={pct} max="100"></progress>
        </div>
      </div>
      <p class="text-base-content/70 text-sm mt-2">{entry.body}</p>
      {#if entry.tags && entry.tags.length > 0}
        <div class="flex flex-wrap gap-1 mt-3">
          {#each entry.tags as tag}
            <span class="badge badge-outline badge-sm">{tag}</span>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</a>
