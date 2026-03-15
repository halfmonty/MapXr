<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { deviceStore } from "$lib/stores/device.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { debugStore } from "$lib/stores/debug.svelte";
  import { setupEventListeners } from "$lib/events";
  import { setDebugMode } from "$lib/commands";
  import FingerPattern from "$lib/components/FingerPattern.svelte";
  import { tapCodeToPattern } from "$lib/utils/tapCode";

  let { children } = $props();

  onMount(async () => {
    await Promise.all([engineStore.init(), profileStore.init()]);
    const cleanupListeners = await setupEventListeners();

    // Restore persisted debug mode (task 7.6).
    const stored = localStorage.getItem("mapxr.debugMode");
    if (stored !== null) {
      const enabled = stored === "true";
      if (enabled !== engineStore.debugMode) {
        await setDebugMode(enabled);
        engineStore.debugMode = enabled;
      }
    }

    return cleanupListeners;
  });

  // Tick every second to keep "Xs ago" timestamps fresh (task 7.2).
  let tick = $state(0);
  onMount(() => {
    const id = setInterval(() => tick++, 1000);
    return () => clearInterval(id);
  });

  function relativeTime(ms: number): string {
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    void tick; // reactive dependency
    const elapsed = Math.floor((Date.now() - ms) / 1000);
    if (elapsed < 5) return "just now";
    if (elapsed < 60) return `${elapsed}s ago`;
    return `${Math.floor(elapsed / 60)}m ago`;
  }

  function handForRole(role: string): "left" | "right" {
    return role === "left" ? "left" : "right";
  }

  const navItems = [
    { href: "/devices", label: "Devices" },
    { href: "/profiles", label: "Profiles" },
    { href: "/debug", label: "Debug" },
  ];

  let currentPath = $derived($page.url.pathname);

  let hasVariables = $derived(Object.keys(engineStore.variables).length > 0);
</script>

<div class="flex h-screen w-screen overflow-hidden bg-base-200">
  <!-- Sidebar -->
  <aside class="flex w-52 flex-shrink-0 flex-col bg-base-100 shadow-md overflow-y-auto">
    <div class="border-b border-base-300 px-4 py-3">
      <span class="text-lg font-bold tracking-tight">mapxr</span>
    </div>

    <!-- Nav links -->
    <nav class="p-2">
      {#each navItems as item}
        <a
          href={item.href}
          class="mb-1 flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium transition-colors
            {currentPath.startsWith(item.href)
            ? 'bg-primary text-primary-content'
            : 'text-base-content hover:bg-base-200'}"
        >
          {item.label}
        </a>
      {/each}
    </nav>

    <div class="border-t border-base-300 mx-3"></div>

    <!-- State section: layer stack + variables (tasks 7.3, 7.4) -->
    <div class="px-3 py-2 space-y-2">
      <p class="text-[10px] font-semibold uppercase tracking-wider text-base-content/40 px-1">
        State
      </p>

      <!-- Layer stack (task 7.3) -->
      <div class="space-y-0.5">
        <p class="text-xs text-base-content/50 px-1">Layer</p>
        <div class="flex flex-wrap items-center gap-0.5 px-1 text-xs">
          {#if engineStore.layerStack.length === 0}
            <span class="italic text-base-content/40">none</span>
          {:else}
            {#each engineStore.layerStack as id, i}
              {#if i > 0}<span class="text-base-content/30">›</span>{/if}
              <span
                class={i === engineStore.layerStack.length - 1
                  ? "font-semibold text-base-content"
                  : "text-base-content/50"}>{id}</span
              >
            {/each}
          {/if}
        </div>
      </div>

      <!-- Variable values (task 7.4) -->
      {#if hasVariables}
        <div class="space-y-0.5">
          <p class="text-xs text-base-content/50 px-1">Variables</p>
          {#each Object.entries(engineStore.variables) as [name, value]}
            <div class="flex items-center justify-between px-1 gap-2">
              <span class="font-mono text-xs text-base-content/70 truncate">{name}</span>
              {#if typeof value === "boolean"}
                <span
                  class="badge badge-xs {value ? 'badge-success' : 'badge-ghost'}"
                >{value ? "true" : "false"}</span>
              {:else}
                <span class="font-mono text-xs text-base-content/70">{value}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Live section: per-device finger visualiser (tasks 7.1, 7.2) -->
    {#if deviceStore.connected.length > 0}
      <div class="border-t border-base-300 mx-3"></div>
      <div class="px-3 py-2 space-y-3">
        <p class="text-[10px] font-semibold uppercase tracking-wider text-base-content/40 px-1">
          Live
        </p>
        {#each deviceStore.connected as device}
          {@const tapState = debugStore.lastTapByRole[device.role]}
          {@const pattern = tapState
            ? tapCodeToPattern(tapState.tapCode, handForRole(device.role))
            : "ooooo"}
          <div class="space-y-1">
            <p class="text-xs text-base-content/50 px-1 capitalize">{device.role}</p>
            <div class="px-1">
              <FingerPattern
                code={pattern}
                hand={handForRole(device.role)}
                readonly
                flash={tapState?.flash ?? false}
              />
            </div>
            <div class="flex items-center gap-2 px-1">
              <span class="font-mono text-xs text-base-content/40">
                {tapState ? `0x${tapState.tapCode.toString(16).padStart(2, "0").toUpperCase()}` : "—"}
              </span>
              <span class="text-xs text-base-content/40">
                {tapState ? relativeTime(tapState.receivedAtMs) : "—"}
              </span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </aside>

  <!-- Main area -->
  <div class="flex flex-1 flex-col overflow-hidden">
    <!-- Page content -->
    <main class="flex-1 overflow-y-auto p-6">
      {@render children()}
    </main>

    <!-- Status bar: connected devices only (layer moved to sidebar) -->
    <footer class="flex items-center gap-4 border-t border-base-300 bg-base-100 px-4 py-1.5 text-xs text-base-content/70">
      <div class="flex items-center gap-2">
        <span class="font-medium text-base-content/50">Devices:</span>
        {#if deviceStore.connected.length === 0}
          <span class="italic">none</span>
        {:else}
          {#each deviceStore.connected as device}
            <span class="badge badge-success badge-sm">{device.role}</span>
          {/each}
        {/if}
      </div>
    </footer>
  </div>
</div>
