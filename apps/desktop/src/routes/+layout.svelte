<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { deviceStore } from "$lib/stores/device.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { debugStore } from "$lib/stores/debug.svelte";
  import { contextRulesStore } from "$lib/stores/contextRules.svelte";
  import { setupEventListeners } from "$lib/events";
  import { setDebugMode, getPlatform } from "$lib/commands";
  import { startAndroidBridge } from "$lib/android-bridge";
  import { getAndroidPreferences } from "$lib/commands";
  import AndroidOnboarding from "$lib/components/AndroidOnboarding.svelte";
  import FingerPattern from "$lib/components/FingerPattern.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";
  import { tapCodeToPattern } from "$lib/utils/tapCode";

  let updateDialogOpen = $state(false);
  let onboardingOpen = $state(false);
  let drawerOpen = $state(false);

  // Synchronous UA check so the correct top bar is rendered on the first frame
  // (avoids a flash from TitleBar → mobile bar on Android).
  const isAndroid = typeof navigator !== "undefined" && /android/i.test(navigator.userAgent);

  let { children } = $props();

  onMount(() => {
    let cleanupListeners: (() => void) | null = null;

    (async () => {
      // Detect platform first: contextRulesStore.init() calls list_context_rules which is
      // a desktop-only command (#[cfg(not(mobile))]) and must be skipped on Android.
      const platform = await getPlatform();
      const inits: Promise<void>[] = [engineStore.init(), profileStore.init()];
      if (platform !== "android") {
        inits.push(contextRulesStore.init());
      }
      await Promise.all(inits);
      cleanupListeners = await setupEventListeners();

      // Android: wire BLE tap bytes from Kotlin BlePlugin to the Rust engine;
      // show onboarding if setup is not yet complete.
      if (platform === "android") {
        const stopBridge = await startAndroidBridge();
        const _prevCleanup = cleanupListeners;
        cleanupListeners = () => { _prevCleanup(); stopBridge(); };

        // Check whether any onboarding step needs to be shown.
        try {
          const androidPrefs = await getAndroidPreferences();
          if (!androidPrefs.battery_setup_done) {
            onboardingOpen = true;
          }
        } catch {
          // Non-fatal — proceed without onboarding check.
        }
      }

      // Restore persisted debug mode (task 7.6).
      const stored = localStorage.getItem("mapxr.debugMode");
      if (stored !== null) {
        const enabled = stored === "true";
        if (enabled !== engineStore.debugMode) {
          await setDebugMode(enabled);
          engineStore.debugMode = enabled;
        }
      }
    })();

    return () => cleanupListeners?.();
  });

  // Tick every second to keep "Xs ago" timestamps fresh (task 7.2).
  let tick = $state(0);
  onMount(() => {
    const id = setInterval(() => tick++, 1000);
    return () => clearInterval(id);
  });

  function relativeTime(ms: number): string {
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
    { href: "/context-rules", label: "Auto-switch" },
    { href: "/debug", label: "Debug" },
    { href: "/settings", label: "Settings" },
  ];

  let currentPath = $derived($page.url.pathname);

  // Close the drawer whenever the route changes (navigation on mobile).
  $effect(() => {
    void currentPath;
    drawerOpen = false;
  });

  let hasVariables = $derived(Object.keys(engineStore.variables).length > 0);

  let activeProfileName = $derived(
    profileStore.profiles.find((p) => p.layer_id === engineStore.activeLayerId)?.name,
  );
  let windowTitle = $derived(
    activeProfileName ? `MapXr - ${activeProfileName}` : "MapXr",
  );
</script>

<!-- drawer-open on lg+ keeps sidebar permanently visible; on smaller screens it
     slides in when drawerOpen is true. z-[1] keeps it below Tauri's title bar. -->
<div class="drawer md:drawer-open h-screen w-screen bg-base-200">
  <input id="nav-drawer" type="checkbox" class="drawer-toggle" bind:checked={drawerOpen} />

  <!-- ── Main content area ─────────────────────────────────────────────────── -->
  <div class="drawer-content flex flex-col overflow-hidden">
    {#if isAndroid}
      <!--
        Mobile top bar: no window controls, title + hamburger only.
        padding-top: env(safe-area-inset-top) pushes content below the Android
        status bar. Height expands to absorb the inset so the visible bar stays
        the same size regardless of status bar height.
      -->
      <header
        class="flex flex-shrink-0 items-center border-b border-base-300 bg-base-100 px-2"
        style="padding-top: env(safe-area-inset-top); height: calc(2.25rem + env(safe-area-inset-top))"
      >
        <label
          for="nav-drawer"
          class="btn btn-ghost btn-sm btn-square md:hidden"
          aria-label="Open navigation"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="3" y1="6" x2="21" y2="6"/>
            <line x1="3" y1="12" x2="21" y2="12"/>
            <line x1="3" y1="18" x2="21" y2="18"/>
          </svg>
        </label>
        <span class="flex-1 truncate pl-2 text-sm font-medium text-base-content/60">
          {windowTitle}
        </span>
      </header>
    {:else}
      <TitleBar title={windowTitle}>
        {#snippet leading()}
          <!-- Hamburger: only visible below md breakpoint on desktop -->
          <label
            for="nav-drawer"
            class="btn btn-ghost btn-sm btn-square ml-1 md:hidden"
            aria-label="Open navigation"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <line x1="3" y1="6" x2="21" y2="6"/>
              <line x1="3" y1="12" x2="21" y2="12"/>
              <line x1="3" y1="18" x2="21" y2="18"/>
            </svg>
          </label>
        {/snippet}
      </TitleBar>
    {/if}

    <UpdateBanner onViewUpdate={() => (updateDialogOpen = true)} />

    <!-- Page content -->
    <main class="flex-1 overflow-y-auto p-6">
      {@render children()}
    </main>

    <!-- Status bar -->
    <footer class="flex items-center gap-4 border-t border-base-300 bg-base-100 px-4 py-1.5 text-xs text-base-content/70">
      <div class="flex items-center gap-2">
        <span class="font-medium text-base-content/50">Devices:</span>
        {#if deviceStore.connected.length === 0}
          <span class="italic">none</span>
        {:else}
          {#each deviceStore.connected as device (device.address)}
            <span class="badge badge-success badge-sm">{device.role}</span>
          {/each}
        {/if}
      </div>
    </footer>
  </div>

  <!-- ── Sidebar ────────────────────────────────────────────────────────────── -->
  <div class="drawer-side z-50">
    <!-- Overlay: tapping it closes the drawer on mobile -->
    <label for="nav-drawer" aria-label="Close navigation" class="drawer-overlay"></label>

    <aside class="flex h-full w-52 flex-shrink-0 flex-col bg-base-100 shadow-md overflow-y-auto">
      <div class="border-b border-base-300 px-4 py-3">
        <span class="text-lg font-bold tracking-tight">mapxr</span>
      </div>

      <!-- Nav links -->
      <nav class="p-2">
        {#each navItems as item (item.href)}
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
              {#each engineStore.layerStack as id, i (id)}
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
            {#each Object.entries(engineStore.variables) as [name, value] (name)}
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
          {#each deviceStore.connected as device (device.address)}
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
  </div>
</div>

<UpdateDialog open={updateDialogOpen} onClose={() => (updateDialogOpen = false)} />
<AndroidOnboarding open={onboardingOpen} onClose={() => (onboardingOpen = false)} />
