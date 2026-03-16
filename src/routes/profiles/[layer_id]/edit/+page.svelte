<script lang="ts">
  import { page } from "$app/stores";
  import { goto, beforeNavigate } from "$app/navigation";
  import { onMount } from "svelte";
  import { loadProfile, saveProfile } from "$lib/commands";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import ActionEditor from "$lib/components/ActionEditor.svelte";
  import FingerPattern from "$lib/components/FingerPattern.svelte";
  import TriggerSummary from "$lib/components/TriggerSummary.svelte";
  import ActionSummary from "$lib/components/ActionSummary.svelte";
  import { logger } from "$lib/logger";
  import type {
    Profile,
    Mapping,
    Trigger,
    Action,
    VariableValue,
    OverloadStrategy,
  } from "$lib/types";

  // ── Load ─────────────────────────────────────────────────────────────────

  let layerId = $derived($page.params.layer_id ?? "");
  let profile = $state<Profile | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      profile = await loadProfile(layerId);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  // ── Dirty tracking & save ─────────────────────────────────────────────────

  let savedJson = $state("");
  let saveError = $state<string | null>(null);
  let saving = $state(false);
  let saveSuccess = $state(false);

  // Capture the initial JSON once loaded so we can detect changes.
  $effect(() => {
    if (profile && !savedJson) {
      savedJson = JSON.stringify(profile);
    }
  });

  let isDirty = $derived(profile ? JSON.stringify(profile) !== savedJson : false);

  async function handleSave() {
    if (!profile) return;
    saving = true;
    saveError = null;
    saveSuccess = false;
    try {
      await saveProfile(profile);
      savedJson = JSON.stringify(profile);
      await profileStore.reload();
      saveSuccess = true;
      setTimeout(() => (saveSuccess = false), 3000);
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
      logger.error("save_profile failed", e);
    } finally {
      saving = false;
    }
  }

  // Unsaved-changes guard (task 5.30)
  beforeNavigate(({ cancel }) => {
    if (isDirty) {
      if (!confirm("You have unsaved changes. Leave without saving?")) {
        cancel();
      }
    }
  });

  // ── Tab navigation ────────────────────────────────────────────────────────

  type Tab = "mappings" | "settings" | "aliases" | "variables" | "lifecycle";
  let activeTab = $state<Tab>("mappings");

  // ── Mapping list (5.18–5.22) ──────────────────────────────────────────────

  let selectedMappingIdx = $state<number | null>(null);
  /** Index pending soft-delete with undo. */
  let pendingDeleteIdx = $state<number | null>(null);
  let pendingDeleteTimer = $state<ReturnType<typeof setTimeout> | null>(null);
  /** Soft-deleted mapping kept for undo. */
  let deletedMapping = $state<{ mapping: Mapping; index: number } | null>(null);

  function selectMapping(i: number) {
    selectedMappingIdx = i === selectedMappingIdx ? null : i;
  }

  function defaultCode(): string {
    return profile?.kind === "dual" ? "xoooo ooooo" : "xoooo";
  }

  function addMapping() {
    if (!profile) return;
    const newMapping: Mapping = {
      label: "New mapping",
      trigger: { type: "tap", code: defaultCode() },
      action: { type: "block" },
    };
    profile.mappings = [...profile.mappings, newMapping];
    selectedMappingIdx = profile.mappings.length - 1;
  }

  function updateMapping(i: number, mapping: Mapping) {
    if (!profile) return;
    profile.mappings = profile.mappings.map((m, idx) => (idx === i ? mapping : m));
  }

  function deleteMapping(i: number) {
    if (!profile) return;
    deletedMapping = { mapping: profile.mappings[i], index: i };
    profile.mappings = profile.mappings.filter((_, idx) => idx !== i);
    if (selectedMappingIdx === i) selectedMappingIdx = null;
    else if (selectedMappingIdx !== null && selectedMappingIdx > i) {
      selectedMappingIdx--;
    }
    if (pendingDeleteTimer) clearTimeout(pendingDeleteTimer);
    pendingDeleteTimer = setTimeout(() => {
      deletedMapping = null;
      pendingDeleteTimer = null;
    }, 5000);
  }

  function undoDelete() {
    if (!profile || !deletedMapping) return;
    const { mapping, index } = deletedMapping;
    const mappings = [...profile.mappings];
    mappings.splice(index, 0, mapping);
    profile.mappings = mappings;
    deletedMapping = null;
    if (pendingDeleteTimer) {
      clearTimeout(pendingDeleteTimer);
      pendingDeleteTimer = null;
    }
  }

  // Drag-and-drop reorder (task 5.19)
  let dragIndex = $state<number | null>(null);

  function onDragStart(e: DragEvent, i: number) {
    dragIndex = i;
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function onDragOver(e: DragEvent, i: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === i || !profile) return;
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  }

  function onDrop(e: DragEvent, i: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === i || !profile) return;
    const mappings = [...profile.mappings];
    const [moved] = mappings.splice(dragIndex, 1);
    mappings.splice(i, 0, moved);
    profile.mappings = mappings;
    if (selectedMappingIdx === dragIndex) selectedMappingIdx = i;
    dragIndex = null;
  }

  function onDragEnd() {
    dragIndex = null;
  }

  // ── Trigger editor (5.23) ─────────────────────────────────────────────────

  function updateTrigger(i: number, trigger: Trigger) {
    if (!profile) return;
    updateMapping(i, { ...profile.mappings[i], trigger });
  }

  function changeTriggerType(i: number, newType: Trigger["type"]) {
    if (!profile) return;
    const code = defaultCode();
    const defaults: Record<Trigger["type"], Trigger> = {
      tap: { type: "tap", code },
      double_tap: { type: "double_tap", code },
      triple_tap: { type: "triple_tap", code },
      sequence: { type: "sequence", steps: [code] },
    };
    updateTrigger(i, defaults[newType]);
  }

  // ── Settings panel (5.25) ─────────────────────────────────────────────────

  function updateSettings(field: string, value: number | string | null) {
    if (!profile) return;
    profile.settings = { ...profile.settings, [field]: value === "" ? undefined : value };
  }

  // ── Alias manager (5.26) ──────────────────────────────────────────────────

  let newAliasName = $state("");
  let newAliasAction = $state<Action>({ type: "block" });
  let editingAlias = $state<string | null>(null);

  function addAlias() {
    if (!profile || !newAliasName.trim()) return;
    profile.aliases = { ...profile.aliases, [newAliasName.trim()]: newAliasAction };
    newAliasName = "";
    newAliasAction = { type: "block" };
  }

  function deleteAlias(name: string) {
    if (!profile) return;
    const { [name]: _, ...rest } = profile.aliases;
    profile.aliases = rest;
    if (editingAlias === name) editingAlias = null;
  }

  function updateAliasAction(name: string, action: Action) {
    if (!profile) return;
    profile.aliases = { ...profile.aliases, [name]: action };
  }

  // ── Variable manager (5.27) ───────────────────────────────────────────────

  let newVarName = $state("");
  let newVarType = $state<"bool" | "int">("bool");
  let newVarBool = $state(false);
  let newVarInt = $state(0);

  function addVariable() {
    if (!profile || !newVarName.trim()) return;
    const value: VariableValue = newVarType === "bool" ? newVarBool : newVarInt;
    profile.variables = { ...profile.variables, [newVarName.trim()]: value };
    newVarName = "";
    newVarBool = false;
    newVarInt = 0;
  }

  function deleteVariable(name: string) {
    if (!profile) return;
    const { [name]: _, ...rest } = profile.variables;
    profile.variables = rest;
  }

  /** Variable names referenced in mappings but not defined in variables. */
  let undefinedVariables = $derived(
    (() => {
      if (!profile) return [] as string[];
      const defined = new Set(Object.keys(profile.variables));
      const referenced = new Set<string>();
      function scan(action: Action) {
        if (action.type === "toggle_variable") referenced.add(action.variable);
        if (action.type === "set_variable") referenced.add(action.variable);
      }
      profile.mappings.forEach((m) => scan(m.action));
      if (profile.on_enter) scan(profile.on_enter);
      if (profile.on_exit) scan(profile.on_exit);
      return [...referenced].filter((v) => !defined.has(v));
    })()
  );
</script>

{#if loading}
  <div class="flex items-center justify-center h-40">
    <span class="loading loading-spinner loading-lg"></span>
  </div>
{:else if loadError}
  <div class="alert alert-error">
    <span>{loadError}</span>
    <button class="btn btn-sm btn-ghost" onclick={() => goto("/profiles")}>← Back</button>
  </div>
{:else if profile}
  <div class="mx-auto max-w-3xl space-y-4">
    <!-- Header -->
    <div class="flex items-center gap-3">
      <button class="btn btn-ghost btn-sm" onclick={() => goto("/profiles")}>←</button>
      <div class="flex-1 min-w-0">
        <input
          type="text"
          class="input input-ghost text-xl font-bold w-full px-1"
          bind:value={profile.name}
          placeholder="Profile name"
        />
      </div>
      <div class="flex items-center gap-2 flex-shrink-0">
        {#if saveSuccess}
          <span class="text-sm text-success">Saved ✓</span>
        {/if}
        {#if saveError}
          <span class="text-sm text-error truncate max-w-xs" title={saveError}>{saveError}</span>
        {/if}
        <button class="btn btn-primary btn-sm" onclick={handleSave} disabled={saving || !isDirty}>
          {#if saving}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            Save
          {/if}
        </button>
      </div>
    </div>

    {#if isDirty}
      <div class="alert alert-warning py-2 text-sm">
        <span>Unsaved changes</span>
        <button class="btn btn-sm btn-ghost" onclick={handleSave} disabled={saving}>
          Save now
        </button>
      </div>
    {/if}

    <!-- Undo delete toast -->
    {#if deletedMapping}
      <div class="alert alert-info py-2 text-sm">
        <span>Mapping deleted.</span>
        <button class="btn btn-sm btn-ghost" onclick={undoDelete}>Undo</button>
      </div>
    {/if}

    <!-- Tabs -->
    <div class="tabs tabs-bordered">
      {#each ["mappings", "settings", "aliases", "variables", "lifecycle"] as const as tab}
        <button
          class="tab {activeTab === tab ? 'tab-active' : ''}"
          onclick={() => (activeTab = tab)}
        >
          {tab.charAt(0).toUpperCase() + tab.slice(1)}
        </button>
      {/each}
    </div>

    <!-- ── Mappings tab ───────────────────────────────────────────────────── -->
    {#if activeTab === "mappings"}
      <div class="space-y-2">
        {#if profile.mappings.length === 0}
          <p class="text-sm text-base-content/50 py-4 text-center">
            No mappings yet. Add one below.
          </p>
        {:else}
          <div class="card bg-base-100 shadow overflow-hidden">
            <table class="table table-sm">
              <thead>
                <tr>
                  <th class="w-6"></th>
                  <th>Label</th>
                  <th>Trigger</th>
                  <th>Action</th>
                  <th class="w-20">On</th>
                  <th class="w-10"></th>
                </tr>
              </thead>
              <tbody>
                {#each profile.mappings as mapping, i}
                  <tr
                    class="cursor-pointer hover:bg-base-200
                      {selectedMappingIdx === i ? 'bg-base-200' : ''}
                      {dragIndex === i ? 'opacity-50' : ''}"
                    onclick={() => selectMapping(i)}
                    draggable={true}
                    ondragstart={(e) => onDragStart(e, i)}
                    ondragover={(e) => onDragOver(e, i)}
                    ondrop={(e) => onDrop(e, i)}
                    ondragend={onDragEnd}
                  >
                    <td class="cursor-grab text-base-content/30 text-center select-none">⠿</td>
                    <td class="max-w-32 truncate">{mapping.label}</td>
                    <td><TriggerSummary trigger={mapping.trigger} /></td>
                    <td><ActionSummary action={mapping.action} /></td>
                    <td onclick={(e) => e.stopPropagation()}>
                      <input
                        type="checkbox"
                        class="toggle toggle-sm toggle-success"
                        checked={mapping.enabled !== false}
                        onchange={(e) =>
                          updateMapping(i, {
                            ...mapping,
                            enabled: (e.target as HTMLInputElement).checked,
                          })}
                      />
                    </td>
                    <td onclick={(e) => e.stopPropagation()}>
                      <button
                        class="btn btn-ghost btn-xs text-error"
                        onclick={() => deleteMapping(i)}
                        aria-label="Delete mapping">✕</button
                      >
                    </td>
                  </tr>

                  <!-- Inline editor row -->
                  {#if selectedMappingIdx === i}
                    <tr>
                      <td colspan={6} class="bg-base-200 p-4">
                        <div class="grid grid-cols-2 gap-6">
                          <!-- Label -->
                          <div class="col-span-2">
                            <label class="form-control w-full">
                              <div class="label py-0">
                                <span class="label-text text-xs">Label</span>
                              </div>
                              <input
                                type="text"
                                class="input input-bordered input-sm w-full"
                                value={mapping.label}
                                oninput={(e) =>
                                  updateMapping(i, {
                                    ...mapping,
                                    label: (e.target as HTMLInputElement).value,
                                  })}
                              />
                            </label>
                          </div>

                          <!-- Trigger editor (5.23) -->
                          <div class="space-y-3">
                            <h4 class="font-semibold text-sm">Trigger</h4>

                            <!-- Type selector (5.23a) -->
                            <label class="form-control">
                              <div class="label py-0">
                                <span class="label-text text-xs">Type</span>
                              </div>
                              <select
                                class="select select-bordered select-sm w-full"
                                value={mapping.trigger.type}
                                onchange={(e) =>
                                  changeTriggerType(
                                    i,
                                    (e.target as HTMLSelectElement).value as Trigger["type"]
                                  )}
                              >
                                <option value="tap">Tap</option>
                                <option value="double_tap">Double tap</option>
                                <option value="triple_tap">Triple tap</option>
                                <option value="sequence">Sequence</option>
                              </select>
                            </label>

                            <!-- Finger pattern (5.23b) -->
                            {#if mapping.trigger.type !== "sequence"}
                              <label class="form-control">
                                <div class="label py-0">
                                  <span class="label-text text-xs">Pattern</span>
                                </div>
                                <div class="flex">
                                  <FingerPattern
                                    code={mapping.trigger.code}
                                    hand={profile.hand ?? "right"}
                                    onchange={(c) =>
                                      updateTrigger(i, { ...mapping.trigger, code: c } as Trigger)}
                                  />
                                </div>
                              </label>
                            {/if}

                            <!-- Sequence steps (5.23c) -->
                            {#if mapping.trigger.type === "sequence"}
                              <div class="space-y-2">
                                <p class="label-text text-xs">Steps</p>
                                {#each mapping.trigger.steps as step, si}
                                  <div class="flex items-center gap-2">
                                    <span class="text-xs text-base-content/50 w-4">{si + 1}.</span>
                                    <FingerPattern
                                      code={step}
                                      hand={profile.hand ?? "right"}
                                      onchange={(c) => {
                                        if (mapping.trigger.type !== "sequence") return;
                                        const steps = mapping.trigger.steps.map((s, idx) =>
                                          idx === si ? c : s
                                        );
                                        updateTrigger(i, { ...mapping.trigger, steps });
                                      }}
                                    />
                                    <button
                                      class="btn btn-ghost btn-xs text-error"
                                      onclick={() => {
                                        if (mapping.trigger.type !== "sequence") return;
                                        const steps = mapping.trigger.steps.filter(
                                          (_, idx) => idx !== si
                                        );
                                        updateTrigger(i, { ...mapping.trigger, steps });
                                      }}
                                      aria-label="Remove step">✕</button
                                    >
                                  </div>
                                {/each}
                                <button
                                  class="btn btn-xs btn-ghost btn-outline"
                                  onclick={() => {
                                    if (mapping.trigger.type !== "sequence") return;
                                    updateTrigger(i, {
                                      ...mapping.trigger,
                                      steps: [...mapping.trigger.steps, defaultCode()],
                                    });
                                  }}>+ Add step</button
                                >
                              </div>
                            {/if}

                            <!-- Per-trigger window_ms override (5.23d) -->
                            {#if mapping.trigger.type === "sequence"}
                              <label class="form-control">
                                <div class="label py-0">
                                  <span class="label-text text-xs">Window override (ms)</span>
                                  <span class="label-text-alt text-xs opacity-50">optional</span>
                                </div>
                                <input
                                  type="number"
                                  class="input input-bordered input-sm w-36"
                                  min={1}
                                  placeholder="profile default"
                                  value={mapping.trigger.window_ms ?? ""}
                                  oninput={(e) => {
                                    if (mapping.trigger.type !== "sequence") return;
                                    const v = (e.target as HTMLInputElement).value;
                                    updateTrigger(i, {
                                      ...mapping.trigger,
                                      window_ms: v === "" ? undefined : Number(v),
                                    });
                                  }}
                                />
                              </label>
                            {/if}
                          </div>

                          <!-- Action editor (5.24) -->
                          <div class="space-y-3">
                            <h4 class="font-semibold text-sm">Action</h4>
                            <ActionEditor
                              action={mapping.action}
                              {profile}
                              onchange={(a: Action) => updateMapping(i, { ...mapping, action: a })}
                            />
                          </div>
                        </div>
                      </td>
                    </tr>
                  {/if}
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        <button class="btn btn-ghost btn-sm btn-outline w-full" onclick={addMapping}>
          + Add mapping
        </button>
      </div>

      <!-- ── Settings tab (5.25) ───────────────────────────────────────────── -->
    {:else if activeTab === "settings"}
      <div class="card bg-base-100 shadow">
        <div class="card-body space-y-4">
          {#each [{ field: "combo_window_ms", label: "Combo window (ms)", hint: "Cross-device chord detection window. Dual profiles only." }, { field: "double_tap_window_ms", label: "Double-tap window (ms)", hint: "Max time between first and second tap." }, { field: "triple_tap_window_ms", label: "Triple-tap window (ms)", hint: "Max time from first to third tap." }, { field: "sequence_window_ms", label: "Sequence step timeout (ms)", hint: "Max gap between consecutive sequence steps." }] as const as row}
            <label class="form-control">
              <div class="label">
                <span class="label-text">{row.label}</span>
                <span class="label-text-alt opacity-60 text-xs">{row.hint}</span>
              </div>
              <input
                type="number"
                class="input input-bordered w-40"
                min={1}
                placeholder="engine default"
                value={profile.settings[row.field] ?? ""}
                oninput={(e) => {
                  const raw = (e.target as HTMLInputElement).value;
                  updateSettings(row.field, raw === "" ? "" : Number(raw));
                }}
              />
            </label>
          {/each}

          <!-- Overload strategy -->
          <label class="form-control">
            <div class="label">
              <span class="label-text">Overload strategy</span>
              <span class="label-text-alt opacity-60 text-xs"
                >Required when a code is used for both tap and double/triple-tap.</span
              >
            </div>
            <select
              class="select select-bordered w-40"
              value={profile.settings.overload_strategy ?? ""}
              onchange={(e) => {
                const v = (e.target as HTMLSelectElement).value;
                updateSettings("overload_strategy", v === "" ? null : (v as OverloadStrategy));
              }}
            >
              <option value="">None</option>
              <option value="patient">Patient</option>
              <option value="eager">Eager</option>
            </select>
          </label>

          <!-- Passthrough -->
          <label class="form-control">
            <div class="label cursor-pointer">
              <span class="label-text">Passthrough</span>
              <span class="label-text-alt opacity-60 text-xs"
                >Pass unmatched codes to lower layers.</span
              >
            </div>
            <input
              type="checkbox"
              class="toggle toggle-primary"
              checked={profile.passthrough === true}
              onchange={(e) => {
                if (profile) profile.passthrough = (e.target as HTMLInputElement).checked;
              }}
            />
          </label>
        </div>
      </div>

      <!-- ── Aliases tab (5.26) ────────────────────────────────────────────── -->
    {:else if activeTab === "aliases"}
      <div class="space-y-3">
        {#if Object.keys(profile.aliases).length === 0 && !editingAlias}
          <p class="text-sm text-base-content/50 py-2">No aliases defined.</p>
        {:else}
          {#each Object.entries(profile.aliases) as [name, action]}
            <div class="card bg-base-100 shadow">
              <div class="card-body py-3 space-y-2">
                <div class="flex items-center justify-between">
                  <span class="font-mono font-semibold text-sm">{name}</span>
                  <div class="flex gap-1">
                    <button
                      class="btn btn-ghost btn-xs"
                      onclick={() => (editingAlias = editingAlias === name ? null : name)}
                      >{editingAlias === name ? "Done" : "Edit"}</button
                    >
                    <button
                      class="btn btn-ghost btn-xs text-error"
                      onclick={() => deleteAlias(name)}
                      aria-label="Delete alias {name}">✕</button
                    >
                  </div>
                </div>
                {#if editingAlias === name}
                  <ActionEditor
                    {action}
                    {profile}
                    onchange={(a: Action) => updateAliasAction(name, a)}
                  />
                {:else}
                  <ActionSummary {action} />
                {/if}
              </div>
            </div>
          {/each}
        {/if}

        <!-- Add alias -->
        <div class="card bg-base-100 shadow">
          <div class="card-body py-3 space-y-3">
            <h4 class="font-semibold text-sm">Add alias</h4>
            <label class="form-control">
              <div class="label py-0"><span class="label-text text-xs">Name</span></div>
              <input
                type="text"
                class="input input-bordered input-sm w-full font-mono"
                bind:value={newAliasName}
                placeholder="save"
              />
            </label>
            <ActionEditor
              action={newAliasAction}
              {profile}
              onchange={(a: Action) => (newAliasAction = a)}
            />
            <button
              class="btn btn-sm btn-primary"
              onclick={addAlias}
              disabled={!newAliasName.trim()}>Add</button
            >
          </div>
        </div>
      </div>

      <!-- ── Variables tab (5.27) ──────────────────────────────────────────── -->
    {:else if activeTab === "variables"}
      <div class="space-y-3">
        {#if undefinedVariables.length > 0}
          <div class="alert alert-warning text-sm">
            <span>
              Referenced but not defined:
              {undefinedVariables.map((v) => `"${v}"`).join(", ")}
            </span>
          </div>
        {/if}

        {#if Object.keys(profile.variables).length === 0}
          <p class="text-sm text-base-content/50 py-2">No variables defined.</p>
        {:else}
          <div class="card bg-base-100 shadow overflow-hidden">
            <table class="table table-sm">
              <thead><tr><th>Name</th><th>Type</th><th>Initial value</th><th></th></tr></thead>
              <tbody>
                {#each Object.entries(profile.variables) as [name, value]}
                  <tr>
                    <td class="font-mono">{name}</td>
                    <td
                      ><span class="badge badge-ghost badge-sm"
                        >{typeof value === "boolean" ? "bool" : "int"}</span
                      ></td
                    >
                    <td class="font-mono">{JSON.stringify(value)}</td>
                    <td>
                      <button
                        class="btn btn-ghost btn-xs text-error"
                        onclick={() => deleteVariable(name)}
                        aria-label="Delete variable {name}">✕</button
                      >
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        <!-- Add variable -->
        <div class="card bg-base-100 shadow">
          <div class="card-body py-3 space-y-3">
            <h4 class="font-semibold text-sm">Add variable</h4>
            <div class="flex gap-3 flex-wrap items-end">
              <label class="form-control">
                <div class="label py-0"><span class="label-text text-xs">Name</span></div>
                <input
                  type="text"
                  class="input input-bordered input-sm font-mono w-36"
                  bind:value={newVarName}
                  placeholder="my_var"
                />
              </label>
              <label class="form-control">
                <div class="label py-0"><span class="label-text text-xs">Type</span></div>
                <div class="join">
                  {#each ["bool", "int"] as t}
                    <button
                      class="btn join-item btn-xs {newVarType === t ? 'btn-primary' : 'btn-ghost'}"
                      onclick={() => (newVarType = t as "bool" | "int")}>{t}</button
                    >
                  {/each}
                </div>
              </label>
              <label class="form-control">
                <div class="label py-0"><span class="label-text text-xs">Initial value</span></div>
                {#if newVarType === "bool"}
                  <select class="select select-bordered select-sm w-24" bind:value={newVarBool}>
                    <option value={false}>false</option>
                    <option value={true}>true</option>
                  </select>
                {:else}
                  <input
                    type="number"
                    class="input input-bordered input-sm w-24"
                    bind:value={newVarInt}
                  />
                {/if}
              </label>
              <button
                class="btn btn-sm btn-primary"
                onclick={addVariable}
                disabled={!newVarName.trim()}>Add</button
              >
            </div>
          </div>
        </div>
      </div>

      <!-- ── Lifecycle tab (5.28) ──────────────────────────────────────────── -->
    {:else if activeTab === "lifecycle"}
      <div class="space-y-4">
        <!-- on_enter -->
        <div class="card bg-base-100 shadow">
          <div class="card-body space-y-3">
            <div class="flex items-center justify-between">
              <h4 class="font-semibold">On enter</h4>
              {#if profile.on_enter}
                <button
                  class="btn btn-ghost btn-xs text-error"
                  onclick={() => {
                    if (profile) profile.on_enter = undefined;
                  }}>Clear</button
                >
              {:else}
                <button
                  class="btn btn-ghost btn-xs"
                  onclick={() => {
                    if (profile) profile.on_enter = { type: "block" };
                  }}>+ Set action</button
                >
              {/if}
            </div>
            {#if profile.on_enter}
              <ActionEditor
                action={profile.on_enter}
                {profile}
                onchange={(a: Action) => {
                  if (profile) profile.on_enter = a;
                }}
              />
            {:else}
              <p class="text-sm text-base-content/50">No action on layer enter.</p>
            {/if}
          </div>
        </div>

        <!-- on_exit -->
        <div class="card bg-base-100 shadow">
          <div class="card-body space-y-3">
            <div class="flex items-center justify-between">
              <h4 class="font-semibold">On exit</h4>
              {#if profile.on_exit}
                <button
                  class="btn btn-ghost btn-xs text-error"
                  onclick={() => {
                    if (profile) profile.on_exit = undefined;
                  }}>Clear</button
                >
              {:else}
                <button
                  class="btn btn-ghost btn-xs"
                  onclick={() => {
                    if (profile) profile.on_exit = { type: "block" };
                  }}>+ Set action</button
                >
              {/if}
            </div>
            {#if profile.on_exit}
              <ActionEditor
                action={profile.on_exit}
                {profile}
                onchange={(a: Action) => {
                  if (profile) profile.on_exit = a;
                }}
              />
            {:else}
              <p class="text-sm text-base-content/50">No action on layer exit.</p>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
