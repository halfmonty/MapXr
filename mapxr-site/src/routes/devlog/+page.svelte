<svelte:head>
  <title>Devlog — MapXr</title>
  <meta name="description" content="MapXr development log: progress notes, milestone tracking, and session summaries from the MapXr project." />
  <meta property="og:title" content="Devlog — MapXr" />
  <meta property="og:description" content="Progress notes and milestone tracking from the MapXr development sessions." />
</svelte:head>

<script lang="ts">
  import { DEVLOG } from '$lib/data/devlog.js';
  import DevlogEntry from '$lib/components/devlog/DevlogEntry.svelte';
  import MilestoneList from '$lib/components/devlog/MilestoneList.svelte';
  import ProgressBar from '$lib/components/devlog/ProgressBar.svelte';

  const latest = DEVLOG[0];
</script>

<div class="max-w-4xl mx-auto px-4 py-10">
  <div class="mb-10">
    <h1 class="text-3xl font-bold mb-2">Devlog</h1>
    <p class="text-base-content/60 mb-6">
      Progress notes from the MapXr development sessions.
    </p>
    <ProgressBar value={latest.epicsDone} max={latest.totalEpics} label="Overall completion" />
  </div>

  <div class="grid md:grid-cols-3 gap-6">
    <div class="md:col-span-2">
      <h2 class="text-xl font-semibold mb-4">Session logs</h2>
      {#each DEVLOG as entry}
        <DevlogEntry {entry} />
      {/each}
    </div>
    <div>
      <h2 class="text-xl font-semibold mb-4">Milestones</h2>
      <MilestoneList />
    </div>
  </div>
</div>
