<script lang="ts">
  import { debugStore } from "$lib/stores/debug.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { setDebugMode } from "$lib/commands";
  import { logger } from "$lib/logger";
  import FingerPattern from "$lib/components/FingerPattern.svelte";
  import ActionSummary from "$lib/components/ActionSummary.svelte";
  import type { DebugEvent } from "$lib/types";

  // ── Debug mode toggle (task 7.6) ──────────────────────────────────────────

  async function toggleDebugMode() {
    const next = !engineStore.debugMode;
    try {
      await setDebugMode(next);
      engineStore.debugMode = next;
      localStorage.setItem("mapxr.debugMode", String(next));
    } catch (e) {
      logger.error("setDebugMode failed", e);
    }
  }

  // ── Event type filter (task 7.11) ─────────────────────────────────────────

  let enabledKinds = $state(new Set<string>(["resolved", "unmatched", "combo_timeout"]));

  function toggleKind(kind: string) {
    const next = new Set(enabledKinds);
    if (next.has(kind)) {
      next.delete(kind);
    } else {
      next.add(kind);
    }
    enabledKinds = next;
  }

  // ── Pause / resume (task 7.12) ────────────────────────────────────────────

  let paused = $state(false);
  let pausedSnapshot = $state<DebugEvent[]>([]);
  let bufferedSincePause = $state(0);

  function togglePause() {
    if (paused) {
      bufferedSincePause = 0;
      pausedSnapshot = [];
      paused = false;
    } else {
      pausedSnapshot = [...debugStore.debugEvents];
      bufferedSincePause = 0;
      paused = true;
    }
  }

  // Track how many new events arrive while paused.
  let prevEventCount = $state(debugStore.debugEvents.length);
  $effect(() => {
    const current = debugStore.debugEvents.length;
    if (paused && current !== prevEventCount) {
      bufferedSincePause += Math.abs(current - prevEventCount);
    }
    prevEventCount = current;
  });

  // ── Rendered list (filtered + possibly frozen) ────────────────────────────

  let displayEvents = $derived(
    (paused ? pausedSnapshot : debugStore.debugEvents).filter((e) =>
      enabledKinds.has(e.kind),
    ),
  );

  // ── Clear (task 7.13) ─────────────────────────────────────────────────────

  function handleClear() {
    debugStore.clear();
    pausedSnapshot = [];
    bufferedSincePause = 0;
  }

  // ── Export as .jsonl (task 7.14) ──────────────────────────────────────────

  function exportEvents() {
    const lines = debugStore.debugEvents.map((e) => JSON.stringify(e)).join("\n");
    const blob = new Blob([lines], { type: "application/jsonl" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `mapxr-debug-${new Date().toISOString().slice(0, 19).replace(/:/g, "-")}.jsonl`;
    a.click();
    URL.revokeObjectURL(url);
  }

  // ── Timing bar helper (task 7.8) ──────────────────────────────────────────

  function timingBarPct(waitedMs: number, windowMs: number): number {
    if (windowMs === 0) return 0;
    return Math.min((waitedMs / windowMs) * 100, 100);
  }
</script>

<div class="flex flex-col h-full gap-4">
  <!-- Toolbar -->
  <div class="flex items-center gap-3 flex-wrap">
    <h1 class="text-lg font-bold mr-2">Debug</h1>

    <!-- Debug mode toggle (task 7.6) -->
    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        class="toggle toggle-sm toggle-primary"
        checked={engineStore.debugMode}
        onchange={toggleDebugMode}
      />
      <span class="text-sm">Debug mode</span>
    </label>

    <div class="flex-1"></div>

    <!-- Filter buttons (task 7.11) -->
    <div class="join">
      {#each ([
        { kind: "resolved",      label: "Resolved",      cls: "btn-success" },
        { kind: "unmatched",     label: "Unmatched",     cls: "btn-warning" },
        { kind: "combo_timeout", label: "Combo timeout", cls: "btn-error"   },
      ] as const) as f}
        <button
          class="join-item btn btn-xs {enabledKinds.has(f.kind) ? f.cls : 'btn-ghost opacity-50'}"
          onclick={() => toggleKind(f.kind)}
        >{f.label}</button>
      {/each}
    </div>

    <!-- Pause/resume (task 7.12) -->
    <button class="btn btn-sm {paused ? 'btn-warning' : 'btn-ghost'}" onclick={togglePause}>
      {paused ? "Resume" : "Pause"}
    </button>

    <!-- Clear (task 7.13) -->
    <button
      class="btn btn-sm btn-ghost"
      onclick={handleClear}
      disabled={debugStore.debugEvents.length === 0}
    >Clear</button>

    <!-- Export (task 7.14) -->
    <button
      class="btn btn-sm btn-ghost"
      onclick={exportEvents}
      disabled={debugStore.debugEvents.length === 0}
    >Export</button>
  </div>

  <!-- Paused indicator -->
  {#if paused && bufferedSincePause > 0}
    <div class="alert alert-warning py-1.5 text-sm">
      <span>Paused — {bufferedSincePause} event{bufferedSincePause === 1 ? "" : "s"} buffered</span>
      <button class="btn btn-xs btn-ghost" onclick={togglePause}>Resume</button>
    </div>
  {/if}

  <!-- No debug mode hint -->
  {#if !engineStore.debugMode}
    <div class="alert alert-info text-sm">
      <span>Enable debug mode to start recording engine events.</span>
    </div>
  {/if}

  <!-- Event stream (tasks 7.7–7.10) -->
  <div class="flex-1 min-h-0 overflow-y-auto space-y-2 pb-4">
    {#if displayEvents.length === 0}
      <p class="text-sm text-base-content/40 text-center py-8">
        {engineStore.debugMode ? "No events yet." : "Debug mode is off."}
      </p>
    {:else}
      {#each displayEvents as event}

        <!-- Resolved event (task 7.8) -->
        {#if event.kind === "resolved"}
          <div class="card bg-base-100 shadow-sm border-l-4 border-success">
            <div class="card-body p-3 space-y-2">
              <div class="flex items-start gap-3 flex-wrap">
                <FingerPattern code={event.pattern} readonly showLabels={false} />
                <div class="flex-1 min-w-0 space-y-1">
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="badge badge-success badge-sm">resolved</span>
                    <span class="badge badge-ghost badge-sm">{event.device}</span>
                    <span class="text-xs text-base-content/50">
                      {event.layer_stack.join(" › ")}
                    </span>
                  </div>
                  <p class="text-sm">
                    Matched <span class="font-semibold">"{event.matched_mapping}"</span>
                    in <span class="font-mono text-xs">{event.matched_layer}</span>
                  </p>
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-base-content/60">Action:</span>
                    <ActionSummary action={event.action_fired} />
                  </div>
                </div>
              </div>
              <!-- Timing bar -->
              {#if event.window_ms > 0}
                <div class="space-y-0.5">
                  <div class="flex justify-between text-xs text-base-content/50">
                    <span>waited</span>
                    <span>{event.waited_ms}ms / {event.window_ms}ms</span>
                  </div>
                  <div class="w-full h-1.5 bg-base-200 rounded-full overflow-hidden">
                    <div
                      class="h-full bg-success rounded-full"
                      style="width: {timingBarPct(event.waited_ms, event.window_ms)}%"
                    ></div>
                  </div>
                </div>
              {:else}
                <p class="text-xs text-base-content/40">
                  Immediate resolution (waited: {event.waited_ms}ms)
                </p>
              {/if}
            </div>
          </div>

        <!-- Unmatched event (task 7.9) -->
        {:else if event.kind === "unmatched"}
          <div class="card bg-base-100 shadow-sm border-l-4 border-warning">
            <div class="card-body p-3">
              <div class="flex items-start gap-3 flex-wrap">
                <FingerPattern code={event.pattern} readonly showLabels={false} />
                <div class="flex-1 min-w-0 space-y-1">
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="badge badge-warning badge-sm">no match</span>
                    <span class="badge badge-ghost badge-sm">{event.device}</span>
                  </div>
                  <p class="text-xs text-base-content/60">
                    Checked: {event.passthrough_layers_checked.join(", ") || "—"}
                  </p>
                </div>
              </div>
            </div>
          </div>

        <!-- Combo timeout event (task 7.10) -->
        {:else if event.kind === "combo_timeout"}
          <div class="card bg-base-100 shadow-sm border-l-4 border-error">
            <div class="card-body p-3 space-y-2">
              <div class="flex items-center gap-2">
                <span class="badge badge-error badge-sm">combo timeout</span>
              </div>
              <div class="flex items-center gap-4 flex-wrap">
                <div class="space-y-1">
                  <span class="badge badge-ghost badge-xs">{event.first_device}</span>
                  <FingerPattern code={event.first_pattern} readonly showLabels={false} />
                </div>
                <span class="text-base-content/30 text-lg">+</span>
                <div class="space-y-1">
                  <span class="badge badge-ghost badge-xs">{event.second_device}</span>
                  <FingerPattern code={event.second_pattern} readonly showLabels={false} />
                </div>
              </div>
              <!-- Gap vs window bar -->
              {#if true}
                {@const windowPct = Math.min(
                  (event.combo_window_ms / event.actual_gap_ms) * 100,
                  100,
                )}
                <div class="space-y-0.5">
                  <div class="flex justify-between text-xs">
                    <span class="text-base-content/50">gap</span>
                    <span class="text-error font-medium">{event.actual_gap_ms}ms</span>
                    <span class="text-base-content/50">window: {event.combo_window_ms}ms</span>
                  </div>
                  <div class="w-full h-1.5 bg-error rounded-full overflow-hidden">
                    <div class="h-full bg-success" style="width: {windowPct}%"></div>
                  </div>
                </div>
              {/if}
            </div>
          </div>
        {/if}

      {/each}
    {/if}
  </div>
</div>
