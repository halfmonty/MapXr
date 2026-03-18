<script lang="ts">
  import type { Component } from 'svelte';
  import DocsLayout from '$lib/components/docs/DocsLayout.svelte';

  let { data } = $props();

  const modules = import.meta.glob<{ default: Component }>(
    '/src/lib/docs-pages/*.svelte',
    { eager: true }
  );

  let PageComponent = $derived(
    modules[`/src/lib/docs-pages/${data.slug}.svelte`]?.default ?? null
  );
</script>

<svelte:head>
  <title>{data.title} — MapXr Docs</title>
  <meta name="description" content="MapXr documentation: {data.title}" />
  <meta property="og:title" content="{data.title} — MapXr Docs" />
</svelte:head>

<DocsLayout currentSlug={data.slug}>
  {#if PageComponent}
    <PageComponent />
  {/if}
</DocsLayout>
