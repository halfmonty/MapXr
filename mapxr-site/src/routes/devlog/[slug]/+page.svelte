<script lang="ts">
  import type { Component } from 'svelte';
  import { base } from '$app/paths';

  let { data } = $props();

  const posts = import.meta.glob<{ default: Component }>(
    '/src/lib/devlog-posts/*.svelte',
    { eager: true }
  );

  let PostComponent = $derived(
    posts[`/src/lib/devlog-posts/${data.entry.slug}.svelte`]?.default ?? null
  );
</script>

<svelte:head>
  <title>{data.entry.title} — MapXr Devlog</title>
  <meta name="description" content={data.entry.body} />
  <meta property="og:title" content="{data.entry.title} — MapXr Devlog" />
  <meta property="og:description" content={data.entry.body} />
</svelte:head>

<div class="max-w-3xl mx-auto px-4 py-10">
  <a href="{base}/devlog" class="btn btn-ghost btn-sm mb-8 -ml-2">
    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
    </svg>
    Back to logs
  </a>

  <div class="flex flex-wrap items-center gap-2 mb-6">
    <span class="font-mono text-sm text-base-content/50">{data.entry.date}</span>
    {#if data.entry.tags}
      {#each data.entry.tags as tag}
        <span class="badge badge-outline badge-sm">{tag}</span>
      {/each}
    {/if}
  </div>

  {#if PostComponent}
    <article class="prose prose-base max-w-none">
      <PostComponent />
    </article>
  {/if}
</div>
