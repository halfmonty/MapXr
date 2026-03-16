<script lang="ts">
  // Task 6.1–6.9: Visual finger pattern input / display component.
  // Replaces FingerPatternPlaceholder from Epic 5.

  const RIGHT_LABELS = ["T", "I", "M", "R", "P"] as const;
  const LEFT_LABELS = ["P", "R", "M", "I", "T"] as const;
  type Label = (typeof RIGHT_LABELS)[number];

  interface Group {
    chars: string[];
    labels: readonly Label[];
    ariaLabel: string;
  }

  interface Props {
    /** Finger-pattern string: "xoooo" (5-char) or "oooox xoooo" (11-char dual). */
    code: string;
    /**
     * Hand orientation for single-hand patterns.
     * "right" (default): position 0 = thumb. "left": position 0 = pinky.
     * Ignored when `code` contains a space (dual pattern).
     */
    hand?: "left" | "right";
    /** When true, circles are not interactive. Default: false. */
    readonly?: boolean;
    /**
     * When true, renders a pulsing "waiting for tap" indicator.
     * The parent owns this state; it subscribes to `tap-event` and calls
     * `onchange` with the decoded pattern, then sets `recording` back to false.
     */
    recording?: boolean;
    /**
     * Show T/I/M/R/P labels below each circle.
     * Defaults to true in interactive/record mode, false in read-only mode.
     */
    showLabels?: boolean;
    /** Called with the new pattern string when a circle is toggled. */
    onchange?: (code: string) => void;
    /**
     * When true, tapped circles play a brief scale animation.
     * Set by the parent when a tap-event is received; cleared after ~500ms.
     */
    flash?: boolean;
    /** Called when `recording` transitions from true to false after a pattern change. */
    onrecorded?: () => void;
  }

  let {
    code,
    hand = "right",
    readonly = false,
    recording = false,
    showLabels,
    flash = false,
    onchange,
    onrecorded,
  }: Props = $props();

  // Track the previous recording state so we can fire onrecorded on transition.
  let wasRecording = $state(false);
  $effect(() => {
    if (wasRecording && !recording) {
      onrecorded?.();
    }
    wasRecording = recording;
  });

  let effectiveShowLabels = $derived(showLabels ?? !readonly);

  let isDual = $derived(code.includes(" "));

  let groups: Group[] = $derived(
    (() => {
      if (isDual) {
        const parts = code.split(" ");
        const l = (parts[0] ?? "ooooo").slice(0, 5).padEnd(5, "o");
        const r = (parts[1] ?? "ooooo").slice(0, 5).padEnd(5, "o");
        return [
          { chars: [...l], labels: LEFT_LABELS, ariaLabel: "Left hand finger pattern" },
          { chars: [...r], labels: RIGHT_LABELS, ariaLabel: "Right hand finger pattern" },
        ];
      }
      const s = code.slice(0, 5).padEnd(5, "o");
      return [
        {
          chars: [...s],
          labels: hand === "left" ? LEFT_LABELS : RIGHT_LABELS,
          ariaLabel: "Finger pattern",
        },
      ];
    })()
  );

  let validationError: string | null = $derived(
    (() => {
      const single = /^[ox]{5}$/i;
      const dual = /^[ox]{5} [ox]{5}$/i;
      if (!single.test(code) && !dual.test(code)) {
        return "Must be 5 characters (e.g. xoooo) or 11 chars for dual (e.g. oooox xoooo).";
      }
      const lower = code.toLowerCase();
      if (lower === "ooooo" || lower === "ooooo ooooo") {
        return "All-open pattern is not a valid trigger.";
      }
      return null;
    })()
  );

  // Flat array of button refs for keyboard navigation (task 6.6).
  // Index = groupIdx * 5 + fingerIdx (max 10 for dual).
  let buttonRefs = $state<(HTMLButtonElement | undefined)[]>(Array(10).fill(undefined));

  function buildPattern(newGroups: string[][]): string {
    if (isDual) return newGroups[0].join("") + " " + newGroups[1].join("");
    return newGroups[0].join("");
  }

  function toggle(groupIdx: number, fingerIdx: number) {
    if (readonly) return;
    const current = groups.map((g) => [...g.chars]);
    current[groupIdx][fingerIdx] = current[groupIdx][fingerIdx] === "x" ? "o" : "x";
    // Prevent reaching an all-open state.
    if (!current.some((g) => g.some((c) => c === "x"))) return;
    onchange?.(buildPattern(current));
  }

  function setFinger(groupIdx: number, fingerIdx: number, value: "x" | "o") {
    if (readonly) return;
    const current = groups.map((g) => [...g.chars]);
    if (current[groupIdx][fingerIdx] === value) return;
    current[groupIdx][fingerIdx] = value;
    if (!current.some((g) => g.some((c) => c === "x"))) return;
    onchange?.(buildPattern(current));
  }

  function focusButton(flatIdx: number) {
    buttonRefs[flatIdx]?.focus();
  }

  function handleKeydown(e: KeyboardEvent, groupIdx: number, fingerIdx: number) {
    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      toggle(groupIdx, fingerIdx);
    } else if (e.key === "x" || e.key === "X") {
      setFinger(groupIdx, fingerIdx, "x");
    } else if (e.key === "o" || e.key === "O") {
      setFinger(groupIdx, fingerIdx, "o");
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      const flat = groupIdx * 5 + fingerIdx;
      const max = groups.length * 5 - 1;
      if (flat < max) focusButton(flat + 1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      const flat = groupIdx * 5 + fingerIdx;
      if (flat > 0) focusButton(flat - 1);
    }
  }
</script>

<!-- Read-only with invalid code: graceful degradation -->
{#if readonly && validationError}
  <span class="inline-flex items-center gap-1 text-error text-xs font-mono">
    <span aria-hidden="true">⚠</span>
    <span>{code}</span>
  </span>
{:else}
  <!-- Outer container; pulsing ring in record mode -->
  <div
    class="inline-flex items-start gap-2 {recording
      ? 'outline outline-2 outline-primary rounded-lg p-1 animate-pulse'
      : ''}"
    role="group"
    aria-label="Finger pattern"
  >
    {#each groups as group, gi}
      <!-- Visual gap between dual groups -->
      {#if gi > 0}
        <div class="w-3 self-stretch border-l border-base-content/20 mx-1"></div>
      {/if}

      <div role="group" aria-label={group.ariaLabel} class="flex items-end gap-1">
        {#each group.chars as char, fi}
          <div class="flex flex-col items-center gap-0.5">
            {#if readonly}
              <!-- Non-interactive circle (task 6.5) -->
              <span
                class="inline-flex items-center justify-center w-8 h-8 rounded-full border-2 select-none
                  {char === 'x'
                  ? 'bg-primary border-primary text-primary-content'
                  : 'border-base-content/30 bg-base-100'}
                  {flash && char === 'x' ? '[animation:tap-flash_0.45s_ease-out]' : ''}"
                role="img"
                aria-label="{group.labels[fi]} {char === 'x' ? 'tapped' : 'not tapped'}"
              ></span>
            {:else}
              <!-- Interactive circle (tasks 6.2, 6.3, 6.6) -->
              <button
                type="button"
                class="btn btn-circle btn-xs {char === 'x' ? 'btn-primary' : 'btn-outline'}
                  {flash && char === 'x' ? '[animation:tap-flash_0.45s_ease-out]' : ''}"
                aria-pressed={char === "x"}
                aria-label="{group.labels[fi]} {char === 'x' ? 'tapped' : 'not tapped'}"
                bind:this={buttonRefs[gi * 5 + fi]}
                onclick={() => toggle(gi, fi)}
                onkeydown={(e) => handleKeydown(e, gi, fi)}
              ></button>
            {/if}

            <!-- Finger labels (tasks 6.3, 6.4) -->
            {#if effectiveShowLabels}
              <span class="font-mono text-xs text-base-content/60 leading-none select-none">
                {group.labels[fi]}
              </span>
            {/if}
          </div>
        {/each}
      </div>
    {/each}

    <!-- Screen-reader announcement for record mode (task 6.7) -->
    {#if recording}
      <span class="sr-only" aria-live="polite">Waiting for tap…</span>
    {/if}
  </div>

  <!-- Inline validation error in interactive mode (task 6.8) -->
  {#if validationError && !readonly}
    <p class="mt-1 text-xs text-error">{validationError}</p>
  {/if}
{/if}
