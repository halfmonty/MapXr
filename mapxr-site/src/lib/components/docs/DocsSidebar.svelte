<script lang="ts">
  import { DOCS, type DocEntry } from '../../data/docs-manifest.js';
  import { base } from '$app/paths';

  interface Props {
    currentSlug: string;
  }

  let { currentSlug }: Props = $props();

  const groups = [...new Set(DOCS.map(d => d.group))];

  function entriesForGroup(group: string): DocEntry[] {
    return DOCS.filter(d => d.group === group);
  }
</script>

<aside class="w-56 shrink-0 hidden md:block">
  <ul class="menu menu-sm bg-base-200 rounded-box p-2 sticky top-20">
    {#each groups as group}
      <li class="menu-title">{group}</li>
      {#each entriesForGroup(group) as entry}
        <li>
          <a
            href={`${base}/docs/${entry.slug}`}
            class:active={currentSlug === entry.slug}
          >
            {entry.title}
          </a>
        </li>
      {/each}
    {/each}
  </ul>
</aside>
