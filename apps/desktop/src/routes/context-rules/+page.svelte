<script lang="ts">
  import { onMount } from "svelte";
  import { contextRulesStore } from "$lib/stores/contextRules.svelte";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { logger } from "$lib/logger";
  import type { ContextRule } from "$lib/types";

  onMount(() => {
    contextRulesStore.init();
  });

  // ── Add / edit modal ─────────────────────────────────────────────────────────

  interface EditState {
    index: number | null; // null = new rule
    name: string;
    layerId: string;
    matchApp: string;
    matchTitle: string;
  }

  let editState = $state<EditState | null>(null);
  let editError = $state<string | null>(null);
  let saveError = $state<string | null>(null);

  function openAdd() {
    editState = { index: null, name: "", layerId: "", matchApp: "", matchTitle: "" };
    editError = null;
  }

  function openEdit(index: number, rule: ContextRule) {
    editState = {
      index,
      name: rule.name,
      layerId: rule.layer_id,
      matchApp: rule.match_app ?? "",
      matchTitle: rule.match_title ?? "",
    };
    editError = null;
  }

  function closeEdit() {
    editState = null;
    editError = null;
  }

  async function handleSaveEdit() {
    if (!editState) return;

    const trimName = editState.name.trim();
    const trimApp = editState.matchApp.trim();
    const trimTitle = editState.matchTitle.trim();

    if (!trimName) {
      editError = "Name is required.";
      return;
    }
    if (!editState.layerId) {
      editError = "Profile is required.";
      return;
    }
    if (!trimApp && !trimTitle) {
      editError = "At least one of App pattern or Title pattern is required.";
      return;
    }

    const rule: ContextRule = {
      name: trimName,
      layer_id: editState.layerId,
      match_app: trimApp || null,
      match_title: trimTitle || null,
    };

    const updated = [...contextRulesStore.rules];
    if (editState.index === null) {
      updated.push(rule);
    } else {
      updated[editState.index] = rule;
    }

    try {
      await contextRulesStore.save(updated);
      closeEdit();
    } catch (e) {
      editError = e instanceof Error ? e.message : String(e);
      logger.error("save_context_rules failed", e);
    }
  }

  // ── Delete ───────────────────────────────────────────────────────────────────

  let deleteConfirmIndex = $state<number | null>(null);

  async function handleDelete(index: number) {
    deleteConfirmIndex = null;
    saveError = null;
    const updated = contextRulesStore.rules.filter((_, i) => i !== index);
    try {
      await contextRulesStore.save(updated);
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
      logger.error("save_context_rules (delete) failed", e);
    }
  }

  // ── Drag-to-reorder ──────────────────────────────────────────────────────────

  let dragIndex = $state<number | null>(null);
  let dropIndex = $state<number | null>(null);

  function onDragStart(index: number) {
    dragIndex = index;
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex !== null && dragIndex !== index) {
      dropIndex = index;
    }
  }

  function onDragLeave() {
    dropIndex = null;
  }

  async function onDrop(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === index) {
      dragIndex = null;
      dropIndex = null;
      return;
    }
    const reordered = [...contextRulesStore.rules];
    const [moved] = reordered.splice(dragIndex, 1);
    reordered.splice(index, 0, moved);
    dragIndex = null;
    dropIndex = null;
    saveError = null;
    try {
      await contextRulesStore.save(reordered);
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
      logger.error("save_context_rules (reorder) failed", e);
    }
  }

  function onDragEnd() {
    dragIndex = null;
    dropIndex = null;
  }
</script>

<div class="mx-auto max-w-2xl space-y-4">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold">Auto-switch</h1>
      <p class="mt-0.5 text-sm text-base-content/60">
        Activate a profile automatically when a matching window is focused.
        Rules are evaluated top-to-bottom; the first match wins.
        Matching is case-insensitive substring search.
      </p>
    </div>
    <button class="btn btn-primary btn-sm flex-shrink-0" onclick={openAdd}>
      + Add rule
    </button>
  </div>

  <!-- Save error -->
  {#if saveError || contextRulesStore.error}
    <div class="alert alert-error text-sm">
      <span>{saveError ?? contextRulesStore.error}</span>
    </div>
  {/if}

  <!-- Rule list -->
  {#if contextRulesStore.rules.length === 0}
    <div class="card bg-base-100 shadow">
      <div class="card-body">
        <p class="text-sm text-base-content/50">
          No rules yet. Add a rule to start switching profiles automatically.
        </p>
      </div>
    </div>
  {:else}
    <div class="space-y-1.5">
      {#each contextRulesStore.rules as rule, i}
        <div
          role="listitem"
          draggable="true"
          ondragstart={() => onDragStart(i)}
          ondragover={(e) => onDragOver(e, i)}
          ondragleave={onDragLeave}
          ondrop={(e) => onDrop(e, i)}
          ondragend={onDragEnd}
          class="card bg-base-100 shadow transition-all
            {dragIndex === i ? 'opacity-40' : ''}
            {dropIndex === i ? 'ring-2 ring-primary' : ''}"
        >
          <div class="card-body flex-row items-center gap-3 py-2.5 px-3">
            <!-- Drag handle -->
            <span
              class="cursor-grab text-base-content/30 select-none text-lg leading-none flex-shrink-0"
              title="Drag to reorder"
            >⠿</span>

            <!-- Priority badge -->
            <span class="badge badge-ghost badge-sm flex-shrink-0 font-mono">{i + 1}</span>

            <!-- Rule info -->
            <div class="flex-1 min-w-0 space-y-0.5">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="font-semibold text-sm truncate">{rule.name}</span>
                <span class="badge badge-primary badge-sm truncate max-w-[12rem]">{rule.layer_id}</span>
              </div>
              <div class="flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-base-content/50">
                {#if rule.match_app}
                  <span>app: <span class="font-mono text-base-content/70">{rule.match_app}</span></span>
                {/if}
                {#if rule.match_title}
                  <span>title: <span class="font-mono text-base-content/70">{rule.match_title}</span></span>
                {/if}
              </div>
            </div>

            <!-- Actions -->
            <div class="flex gap-1.5 flex-shrink-0">
              <button
                class="btn btn-xs btn-ghost"
                onclick={() => openEdit(i, rule)}
              >Edit</button>
              <button
                class="btn btn-xs btn-ghost btn-error"
                onclick={() => (deleteConfirmIndex = i)}
              >Delete</button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Delete confirm modal -->
{#if deleteConfirmIndex !== null}
  {@const rule = contextRulesStore.rules[deleteConfirmIndex]}
  <dialog class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold">Delete "{rule?.name}"?</h3>
      <p class="py-4 text-sm">This rule will no longer trigger automatic profile switching.</p>
      <div class="modal-action">
        <button class="btn btn-ghost" onclick={() => (deleteConfirmIndex = null)}>
          Cancel
        </button>
        <button
          class="btn btn-error"
          onclick={() => {
            const idx = deleteConfirmIndex!;
            deleteConfirmIndex = null;
            handleDelete(idx);
          }}
        >
          Delete
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={() => (deleteConfirmIndex = null)}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}

<!-- Add / edit modal -->
{#if editState !== null}
  <dialog class="modal modal-open">
    <div class="modal-box space-y-4">
      <h3 class="text-lg font-bold">
        {editState.index === null ? "Add rule" : "Edit rule"}
      </h3>

      <!-- Name -->
      <label class="form-control w-full">
        <div class="label"><span class="label-text">Rule name</span></div>
        <input
          type="text"
          placeholder="e.g. VS Code"
          class="input input-bordered w-full"
          bind:value={editState.name}
        />
      </label>

      <!-- Profile selector -->
      <label class="form-control w-full">
        <div class="label">
          <span class="label-text">Activate profile</span>
        </div>
        <select class="select select-bordered w-full" bind:value={editState.layerId}>
          <option value="" disabled>— select a profile —</option>
          {#each profileStore.profiles as p}
            <option value={p.layer_id}>{p.name}</option>
          {/each}
        </select>
      </label>

      <!-- Match app -->
      <label class="form-control w-full">
        <div class="label">
          <span class="label-text">App pattern</span>
          <span class="label-text-alt">optional</span>
        </div>
        <input
          type="text"
          placeholder="e.g. code, firefox"
          class="input input-bordered w-full"
          bind:value={editState.matchApp}
        />
        <div class="label">
          <span class="label-text-alt text-base-content/40">
            Matches if the focused app name contains this substring (case-insensitive).
          </span>
        </div>
      </label>

      <!-- Match title -->
      <label class="form-control w-full">
        <div class="label">
          <span class="label-text">Title pattern</span>
          <span class="label-text-alt">optional</span>
        </div>
        <input
          type="text"
          placeholder="e.g. vim, — mapxr"
          class="input input-bordered w-full"
          bind:value={editState.matchTitle}
        />
        <div class="label">
          <span class="label-text-alt text-base-content/40">
            Matches if the window title contains this substring (case-insensitive).
          </span>
        </div>
      </label>

      {#if editError}
        <div class="alert alert-error text-sm">
          <span>{editError}</span>
        </div>
      {/if}

      <div class="modal-action">
        <button class="btn btn-ghost" onclick={closeEdit} disabled={contextRulesStore.saving}>
          Cancel
        </button>
        <button
          class="btn btn-primary"
          onclick={handleSaveEdit}
          disabled={contextRulesStore.saving}
        >
          {#if contextRulesStore.saving}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            {editState.index === null ? "Add" : "Save"}
          {/if}
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={closeEdit}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}
