<script lang="ts">
  import DocsSidebar from './DocsSidebar.svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    currentSlug: string;
    children: Snippet;
  }

  let { currentSlug, children }: Props = $props();
</script>

<div class="max-w-5xl mx-auto px-4 py-10 flex gap-8 min-h-[70vh]">
  <DocsSidebar {currentSlug} />
  <main class="flex-1 min-w-0 prose prose-base max-w-none">
    {@render children()}
  </main>
</div>
