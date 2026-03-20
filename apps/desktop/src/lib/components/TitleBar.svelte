<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { browser } from "$app/environment";

  interface Props {
    title: string;
  }

  let { title }: Props = $props();

  const appWindow = getCurrentWindow();

  const LIGHT = "corporate";
  const DARK = "business";

  function systemPrefers(): string {
    if (!browser) return LIGHT;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? DARK : LIGHT;
  }

  let theme = $state(browser ? (localStorage.getItem("theme") ?? systemPrefers()) : LIGHT);

  $effect(() => {
    if (!browser) return;
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem("theme", theme);
  });

  function toggleTheme() {
    theme = theme === LIGHT ? DARK : LIGHT;
  }
</script>

<!--
  data-tauri-drag-region on the outer div makes the whole bar draggable.
  Individual buttons use onclick with stopPropagation so drags don't
  accidentally fire button actions.
-->
<header
  data-tauri-drag-region
  class="flex h-9 w-full flex-shrink-0 select-none items-center justify-between
         border-b border-base-300 bg-base-100"
>
  <!-- Title -->
  <span
    data-tauri-drag-region
    class="pointer-events-none truncate pl-4 text-sm font-medium text-base-content/60"
  >
    {title}
  </span>

  <!-- Window controls -->
  <div class="flex h-full flex-shrink-0">
    <!-- Theme toggle -->
    <button
      class="flex h-full w-11 items-center justify-center text-base-content/50
             transition-colors hover:bg-base-200 hover:text-base-content"
      aria-label="Toggle theme"
      onclick={(e) => { e.stopPropagation(); toggleTheme(); }}
    >
      {#if theme === DARK}
        <!-- Sun icon (click to go light) -->
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 6.5A5.5 5.5 0 1 0 17.5 12 5.51 5.51 0 0 0 12 6.5Zm0 9A3.5 3.5 0 1 1 15.5 12 3.5 3.5 0 0 1 12 15.5Zm-7-3.5H4a1 1 0 0 0 0 2h1a1 1 0 0 0 0-2Zm16 0h-1a1 1 0 0 0 0 2h1a1 1 0 0 0 0-2ZM12 3a1 1 0 0 0 1-1V1a1 1 0 0 0-2 0v1a1 1 0 0 0 1 1Zm0 18a1 1 0 0 0-1 1v1a1 1 0 0 0 2 0v-1a1 1 0 0 0-1-1ZM5.64 7.05a1 1 0 0 0 .7-.29 1 1 0 0 0 0-1.41l-.71-.71a1 1 0 1 0-1.41 1.41l.71.71a1 1 0 0 0 .71.29Zm12.02 9.9a1 1 0 0 0-1.41 1.41l.71.71a1 1 0 0 0 1.41-1.41Zm.7-9.9a1 1 0 0 0 .71-.29l.71-.71a1 1 0 1 0-1.41-1.41l-.71.71a1 1 0 0 0 0 1.41 1 1 0 0 0 .7.29ZM5.64 17a1 1 0 0 0-.71.29l-.71.71a1 1 0 0 0 1.41 1.41l.71-.71A1 1 0 0 0 5.64 17Z"/>
        </svg>
      {:else}
        <!-- Moon icon (click to go dark) -->
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <path d="M21.64 13a1 1 0 0 0-1.05-.14 8.05 8.05 0 0 1-3.37.73A8.15 8.15 0 0 1 9.08 5.49a8.59 8.59 0 0 1 .25-2A1 1 0 0 0 8 2.36 10.14 10.14 0 1 0 22 14.05 1 1 0 0 0 21.64 13Zm-9.5 6.69A8.14 8.14 0 0 1 7.08 5.22v.27A10.15 10.15 0 0 0 17.22 15.63a9.79 9.79 0 0 0 2.1-.22A8.11 8.11 0 0 1 12.14 19.73Z"/>
        </svg>
      {/if}
    </button>

    <!-- Minimize -->
    <button
      class="flex h-full w-11 items-center justify-center text-base-content/50
             transition-colors hover:bg-base-200 hover:text-base-content"
      aria-label="Minimize"
      onclick={(e) => { e.stopPropagation(); appWindow.minimize(); }}
    >
      <svg width="11" height="2" viewBox="0 0 11 2" fill="none">
        <line x1="0.5" y1="1" x2="10.5" y2="1" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </button>

    <!-- Maximize / restore -->
    <button
      class="flex h-full w-11 items-center justify-center text-base-content/50
             transition-colors hover:bg-base-200 hover:text-base-content"
      aria-label="Maximize"
      onclick={(e) => { e.stopPropagation(); appWindow.toggleMaximize(); }}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" fill="none">
        <rect x="0.75" y="0.75" width="9.5" height="9.5" rx="1" stroke="currentColor" stroke-width="1.5"/>
      </svg>
    </button>

    <!-- Close -->
    <button
      class="flex h-full w-11 items-center justify-center text-base-content/50
             transition-colors hover:bg-error hover:text-error-content"
      aria-label="Close"
      onclick={(e) => { e.stopPropagation(); appWindow.close(); }}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" fill="none">
        <line x1="1" y1="1" x2="10" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        <line x1="10" y1="1" x2="1" y2="10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
    </button>
  </div>
</header>
