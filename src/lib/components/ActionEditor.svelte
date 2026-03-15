<script lang="ts">
  import ActionEditor from "./ActionEditor.svelte";
  import { KNOWN_KEY_NAMES } from "$lib/types";
  import type { Action, MacroStep, Modifier, Profile } from "$lib/types";
  import { profileStore } from "$lib/stores/profile.svelte";

  interface Props {
    action: Action;
    profile: Profile;
    /** Action types to exclude from the type selector (e.g. prevent macro nesting). */
    disallow?: Action["type"][];
    onchange: (action: Action) => void;
  }

  let { action, profile, disallow = [], onchange }: Props = $props();

  const ALL_TYPES: Action["type"][] = [
    "key", "key_chord", "type_string", "macro",
    "push_layer", "pop_layer", "switch_layer",
    "toggle_variable", "set_variable", "block", "alias",
  ];

  const TYPE_LABELS: Record<Action["type"], string> = {
    key: "Key", key_chord: "Key chord", type_string: "Type string",
    macro: "Macro", push_layer: "Push layer", pop_layer: "Pop layer",
    switch_layer: "Switch layer", toggle_variable: "Toggle variable",
    set_variable: "Set variable", block: "Block", alias: "Alias",
  };

  let availableTypes = $derived(ALL_TYPES.filter((t) => !disallow.includes(t)));

  function defaultAction(type: Action["type"]): Action {
    switch (type) {
      case "key":             return { type: "key", key: "a", modifiers: [] };
      case "key_chord":       return { type: "key_chord", keys: ["ctrl", "c"] };
      case "type_string":     return { type: "type_string", text: "" };
      case "macro":           return { type: "macro", steps: [] };
      case "push_layer":      return { type: "push_layer", layer: "", mode: "permanent" };
      case "pop_layer":       return { type: "pop_layer" };
      case "switch_layer":    return { type: "switch_layer", layer: "" };
      case "toggle_variable": return { type: "toggle_variable", variable: "", on_true: { type: "block" }, on_false: { type: "block" } };
      case "set_variable":    return { type: "set_variable", variable: "", value: false };
      case "block":           return { type: "block" };
      case "alias":           return { type: "alias", name: "" };
    }
  }

  function changeType(newType: Action["type"]) {
    if (newType !== action.type) onchange(defaultAction(newType));
  }

  // ── Key action helpers ──────────────────────────────────────────────────────

  function toggleModifier(mod: Modifier) {
    if (action.type !== "key") return;
    const mods = action.modifiers ?? [];
    const next = mods.includes(mod) ? mods.filter((m) => m !== mod) : [...mods, mod];
    onchange({ ...action, modifiers: next });
  }

  // ── Key chord helpers ───────────────────────────────────────────────────────

  let chordInput = $state("");

  function addChordKey() {
    if (action.type !== "key_chord" || !chordInput.trim()) return;
    onchange({ ...action, keys: [...action.keys, chordInput.trim()] });
    chordInput = "";
  }

  function removeChordKey(i: number) {
    if (action.type !== "key_chord") return;
    onchange({ ...action, keys: action.keys.filter((_, idx) => idx !== i) });
  }

  // ── Macro helpers ───────────────────────────────────────────────────────────

  function addMacroStep() {
    if (action.type !== "macro") return;
    const step: MacroStep = { action: { type: "block" }, delay_ms: 0 };
    onchange({ ...action, steps: [...action.steps, step] });
  }

  function updateMacroStep(i: number, step: MacroStep) {
    if (action.type !== "macro") return;
    const steps = action.steps.map((s, idx) => (idx === i ? step : s));
    onchange({ ...action, steps });
  }

  function removeMacroStep(i: number) {
    if (action.type !== "macro") return;
    onchange({ ...action, steps: action.steps.filter((_, idx) => idx !== i) });
  }

  function moveMacroStep(i: number, dir: -1 | 1) {
    if (action.type !== "macro") return;
    const steps = [...action.steps];
    const j = i + dir;
    if (j < 0 || j >= steps.length) return;
    [steps[i], steps[j]] = [steps[j], steps[i]];
    onchange({ ...action, steps });
  }

  // ── PushLayer helpers ───────────────────────────────────────────────────────

  type PushMode = "permanent" | "count" | "timeout";

  let pushMode = $derived(
    action.type === "push_layer" ? (action.mode as PushMode) : "permanent",
  );

  function setPushLayer(field: "layer" | "mode" | "count" | "timeout_ms", value: string | number) {
    if (action.type !== "push_layer") return;
    if (field === "layer") {
      onchange({ ...action, layer: value as string });
    } else if (field === "mode") {
      const m = value as PushMode;
      if (m === "permanent") onchange({ ...action, mode: "permanent" });
      else if (m === "count") onchange({ ...action, mode: "count", count: 1 } as Action);
      else onchange({ ...action, mode: "timeout", timeout_ms: 1000 } as Action);
    } else if (field === "count" && "count" in action) {
      onchange({ ...action, count: Number(value) } as Action);
    } else if (field === "timeout_ms" && "timeout_ms" in action) {
      onchange({ ...action, timeout_ms: Number(value) } as Action);
    }
  }

  // ── Variable helpers ────────────────────────────────────────────────────────

  let variableNames = $derived(Object.keys(profile.variables));

  function setVariableField(field: string, value: unknown) {
    if (action.type !== "toggle_variable" && action.type !== "set_variable") return;
    onchange({ ...action, [field]: value } as Action);
  }

  function setVariableValue(raw: string) {
    if (action.type !== "set_variable") return;
    const current = action.value;
    if (typeof current === "boolean") {
      onchange({ ...action, value: raw === "true" });
    } else {
      onchange({ ...action, value: Number(raw) || 0 });
    }
  }

  function setVariableValueType(t: "bool" | "int") {
    if (action.type !== "set_variable") return;
    onchange({ ...action, value: t === "bool" ? false : 0 });
  }
</script>

<div class="space-y-3">
  <!-- Type selector -->
  <label class="form-control w-full">
    <div class="label py-0"><span class="label-text text-xs">Action type</span></div>
    <select
      class="select select-bordered select-sm w-full"
      value={action.type}
      onchange={(e) => changeType((e.target as HTMLSelectElement).value as Action["type"])}
    >
      {#each availableTypes as t}
        <option value={t}>{TYPE_LABELS[t]}</option>
      {/each}
    </select>
  </label>

  <!-- ── Key ──────────────────────────────────────────────────────────────── -->
  {#if action.type === "key"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Key</span></div>
      <input
        type="text"
        list="known-keys"
        class="input input-bordered input-sm w-full font-mono"
        value={action.key}
        oninput={(e) => onchange({ ...action, key: (e.target as HTMLInputElement).value })}
        placeholder="a"
      />
      <datalist id="known-keys">
        {#each KNOWN_KEY_NAMES as k}<option value={k}></option>{/each}
      </datalist>
    </label>
    <div>
      <p class="label-text mb-1 text-xs">Modifiers</p>
      <div class="flex gap-2">
        {#each ["ctrl", "shift", "alt", "meta"] as m}
          <label class="flex cursor-pointer items-center gap-1 text-sm">
            <input
              type="checkbox"
              class="checkbox checkbox-sm"
              checked={action.modifiers?.includes(m as Modifier) ?? false}
              onchange={() => toggleModifier(m as Modifier)}
            />
            {m}
          </label>
        {/each}
      </div>
    </div>

  <!-- ── Key chord ─────────────────────────────────────────────────────────── -->
  {:else if action.type === "key_chord"}
    <div>
      <p class="label-text mb-1 text-xs">Keys</p>
      <div class="flex flex-wrap gap-1 mb-2">
        {#each action.keys as k, i}
          <span class="badge badge-neutral gap-1 font-mono">
            {k}
            <button
              class="text-xs opacity-60 hover:opacity-100"
              onclick={() => removeChordKey(i)}
              aria-label="Remove {k}"
            >✕</button>
          </span>
        {/each}
      </div>
      <div class="flex gap-2">
        <input
          type="text"
          list="known-keys"
          class="input input-bordered input-sm flex-1 font-mono"
          placeholder="Add key…"
          bind:value={chordInput}
          onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addChordKey(); } }}
        />
        <button class="btn btn-sm btn-ghost" onclick={addChordKey}>Add</button>
      </div>
      <datalist id="known-keys">
        {#each KNOWN_KEY_NAMES as k}<option value={k}></option>{/each}
      </datalist>
    </div>

  <!-- ── Type string ───────────────────────────────────────────────────────── -->
  {:else if action.type === "type_string"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Text to type</span></div>
      <textarea
        class="textarea textarea-bordered textarea-sm w-full font-mono"
        rows={3}
        value={action.text}
        oninput={(e) => onchange({ ...action, text: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
    </label>

  <!-- ── Macro ─────────────────────────────────────────────────────────────── -->
  {:else if action.type === "macro"}
    <div class="space-y-2">
      {#each action.steps as step, i}
        <div class="rounded-lg border border-base-300 p-3 space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium text-base-content/60">Step {i + 1}</span>
            <div class="flex gap-1">
              <button class="btn btn-xs btn-ghost" onclick={() => moveMacroStep(i, -1)} disabled={i === 0} aria-label="Move up">↑</button>
              <button class="btn btn-xs btn-ghost" onclick={() => moveMacroStep(i, 1)} disabled={i === action.steps.length - 1} aria-label="Move down">↓</button>
              <button class="btn btn-xs btn-ghost text-error" onclick={() => removeMacroStep(i)} aria-label="Remove step">✕</button>
            </div>
          </div>
          <ActionEditor
            action={step.action}
            {profile}
            disallow={["macro"]}
            onchange={(a: Action) => updateMacroStep(i, { ...step, action: a })}
          />
          <label class="form-control">
            <div class="label py-0"><span class="label-text text-xs">Delay after step (ms)</span></div>
            <input
              type="number"
              class="input input-bordered input-sm w-32"
              min={0}
              value={step.delay_ms}
              oninput={(e) => updateMacroStep(i, { ...step, delay_ms: Number((e.target as HTMLInputElement).value) })}
            />
          </label>
        </div>
      {/each}
      <button class="btn btn-sm btn-ghost btn-outline w-full" onclick={addMacroStep}>
        + Add step
      </button>
    </div>

  <!-- ── Push layer ────────────────────────────────────────────────────────── -->
  {:else if action.type === "push_layer"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Layer</span></div>
      <select
        class="select select-bordered select-sm w-full"
        value={action.layer}
        onchange={(e) => setPushLayer("layer", (e.target as HTMLSelectElement).value)}
      >
        <option value="">— select profile —</option>
        {#each profileStore.profiles as p}
          <option value={p.layer_id}>{p.name} ({p.layer_id})</option>
        {/each}
      </select>
    </label>
    <div>
      <p class="label-text mb-1 text-xs">Mode</p>
      <div class="join">
        {#each ["permanent", "count", "timeout"] as m}
          <button
            class="btn join-item btn-xs {pushMode === m ? 'btn-primary' : 'btn-ghost'}"
            onclick={() => setPushLayer("mode", m)}
          >{m}</button>
        {/each}
      </div>
    </div>
    {#if action.mode === "count"}
      <label class="form-control">
        <div class="label py-0"><span class="label-text text-xs">Count</span></div>
        <input type="number" class="input input-bordered input-sm w-32" min={1}
          value={action.count}
          oninput={(e) => setPushLayer("count", (e.target as HTMLInputElement).value)} />
      </label>
    {:else if action.mode === "timeout"}
      <label class="form-control">
        <div class="label py-0"><span class="label-text text-xs">Timeout (ms)</span></div>
        <input type="number" class="input input-bordered input-sm w-36" min={1}
          value={action.timeout_ms}
          oninput={(e) => setPushLayer("timeout_ms", (e.target as HTMLInputElement).value)} />
      </label>
    {/if}

  <!-- ── Pop layer ─────────────────────────────────────────────────────────── -->
  {:else if action.type === "pop_layer"}
    <p class="text-sm text-base-content/60">Pops the current layer off the stack.</p>

  <!-- ── Switch layer ──────────────────────────────────────────────────────── -->
  {:else if action.type === "switch_layer"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Layer</span></div>
      <select
        class="select select-bordered select-sm w-full"
        value={action.layer}
        onchange={(e) => onchange({ ...action, layer: (e.target as HTMLSelectElement).value })}
      >
        <option value="">— select profile —</option>
        {#each profileStore.profiles as p}
          <option value={p.layer_id}>{p.name} ({p.layer_id})</option>
        {/each}
      </select>
    </label>

  <!-- ── Toggle variable ───────────────────────────────────────────────────── -->
  {:else if action.type === "toggle_variable"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Variable</span></div>
      <select
        class="select select-bordered select-sm w-full"
        value={action.variable}
        onchange={(e) => setVariableField("variable", (e.target as HTMLSelectElement).value)}
      >
        <option value="">— select variable —</option>
        {#each variableNames as v}<option value={v}>{v}</option>{/each}
      </select>
    </label>
    <div class="rounded-lg border border-base-300 p-3 space-y-2">
      <p class="text-xs font-medium text-base-content/60">When true → fire:</p>
      <ActionEditor
        action={action.on_true}
        {profile}
        disallow={["macro", "toggle_variable"]}
        onchange={(a: Action) => setVariableField("on_true", a)}
      />
    </div>
    <div class="rounded-lg border border-base-300 p-3 space-y-2">
      <p class="text-xs font-medium text-base-content/60">When false → fire:</p>
      <ActionEditor
        action={action.on_false}
        {profile}
        disallow={["macro", "toggle_variable"]}
        onchange={(a: Action) => setVariableField("on_false", a)}
      />
    </div>

  <!-- ── Set variable ──────────────────────────────────────────────────────── -->
  {:else if action.type === "set_variable"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Variable</span></div>
      <select
        class="select select-bordered select-sm w-full"
        value={action.variable}
        onchange={(e) => setVariableField("variable", (e.target as HTMLSelectElement).value)}
      >
        <option value="">— select variable —</option>
        {#each variableNames as v}<option value={v}>{v}</option>{/each}
      </select>
    </label>
    <div>
      <p class="label-text mb-1 text-xs">Value type</p>
      <div class="join mb-2">
        {#each ["bool", "int"] as t}
          <button
            class="btn join-item btn-xs {(typeof action.value === 'boolean' ? 'bool' : 'int') === t ? 'btn-primary' : 'btn-ghost'}"
            onclick={() => setVariableValueType(t as "bool" | "int")}
          >{t}</button>
        {/each}
      </div>
      {#if typeof action.value === "boolean"}
        <select
          class="select select-bordered select-sm w-32"
          value={String(action.value)}
          onchange={(e) => setVariableValue((e.target as HTMLSelectElement).value)}
        >
          <option value="true">true</option>
          <option value="false">false</option>
        </select>
      {:else}
        <input
          type="number"
          class="input input-bordered input-sm w-32"
          value={action.value}
          oninput={(e) => setVariableValue((e.target as HTMLInputElement).value)}
        />
      {/if}
    </div>

  <!-- ── Block ─────────────────────────────────────────────────────────────── -->
  {:else if action.type === "block"}
    <p class="text-sm text-base-content/60">Blocks passthrough to lower layers.</p>

  <!-- ── Alias ─────────────────────────────────────────────────────────────── -->
  {:else if action.type === "alias"}
    <label class="form-control w-full">
      <div class="label py-0"><span class="label-text text-xs">Alias name</span></div>
      <select
        class="select select-bordered select-sm w-full"
        value={action.name}
        onchange={(e) => onchange({ ...action, name: (e.target as HTMLSelectElement).value })}
      >
        <option value="">— select alias —</option>
        {#each Object.keys(profile.aliases) as a}<option value={a}>{a}</option>{/each}
      </select>
    </label>
  {/if}
</div>
