<script lang="ts">
  import { goto } from "$app/navigation";
  import { activateProfile, deactivateProfile, deleteProfile, saveProfile, readFileText } from "$lib/commands";
  import { profileStore } from "$lib/stores/profile.svelte";
  import { engineStore } from "$lib/stores/engine.svelte";
  import { deviceStore } from "$lib/stores/device.svelte";
  import { logger } from "$lib/logger";
  import type { Profile, ProfileKind, Hand } from "$lib/types";

  // ── Activate ────────────────────────────────────────────────────────────────

  let activatingId = $state<string | null>(null);
  let deactivating = $state(false);
  let actionError = $state<string | null>(null);

  async function handleActivate(layerId: string) {
    activatingId = layerId;
    actionError = null;
    try {
      await activateProfile(layerId);
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
      logger.error("activate_profile failed", e);
    } finally {
      activatingId = null;
    }
  }

  async function handleDeactivate() {
    deactivating = true;
    actionError = null;
    try {
      await deactivateProfile();
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
      logger.error("deactivate_profile failed", e);
    } finally {
      deactivating = false;
    }
  }

  // ── Delete ──────────────────────────────────────────────────────────────────

  let deleteConfirmId = $state<string | null>(null);
  let deletingId = $state<string | null>(null);

  async function handleDelete(layerId: string) {
    deletingId = layerId;
    deleteConfirmId = null;
    actionError = null;
    try {
      await deleteProfile(layerId);
      await profileStore.reload();
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
      logger.error("delete_profile failed", e);
    } finally {
      deletingId = null;
    }
  }

  // ── New profile wizard ──────────────────────────────────────────────────────

  let showNewWizard = $state(false);
  let wizardName = $state("");
  let wizardKind = $state<ProfileKind>("single");
  let wizardHand = $state<Hand>("right");
  let wizardDescription = $state("");
  let wizardSaving = $state(false);
  let wizardError = $state<string | null>(null);

  let wizardLayerId = $derived(
    wizardName
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "",
  );

  function openNewWizard() {
    wizardName = "";
    wizardKind = "single";
    wizardHand = "right";
    wizardDescription = "";
    wizardError = null;
    showNewWizard = true;
  }

  async function handleCreateProfile() {
    if (!wizardName.trim() || !wizardLayerId) {
      wizardError = "Name is required.";
      return;
    }
    wizardSaving = true;
    wizardError = null;
    const newProfile: Profile = {
      version: 1,
      kind: wizardKind,
      name: wizardName.trim(),
      description: wizardDescription.trim() || undefined,
      layer_id: wizardLayerId,
      hand: wizardKind === "single" ? wizardHand : undefined,
      settings: {},
      aliases: {},
      variables: {},
      mappings: [],
    };
    try {
      await saveProfile(newProfile);
      await profileStore.reload();
      showNewWizard = false;
    } catch (e) {
      wizardError = e instanceof Error ? e.message : String(e);
      logger.error("save_profile (new) failed", e);
    } finally {
      wizardSaving = false;
    }
  }

  // ── Import ──────────────────────────────────────────────────────────────────

  let importError = $state<string | null>(null);
  let isDragOver = $state(false);

  async function importFile(file: File) {
    importError = null;
    const text = await file.text();
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      importError = `${file.name} is not valid JSON.`;
      return;
    }
    try {
      await saveProfile(parsed as Profile);
      await profileStore.reload();
    } catch (e) {
      importError = e instanceof Error ? e.message : String(e);
      logger.error("save_profile (import) failed", e);
    }
  }

  async function handleImport(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    await importFile(file);
    input.value = "";
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragOver = true;
  }

  function handleDragLeave(e: DragEvent) {
    // Only clear when leaving the container itself, not a child element.
    if (!e.currentTarget || !(e.currentTarget as Element).contains(e.relatedTarget as Node)) {
      isDragOver = false;
    }
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;

    // Standard path: Chromium-style WebViews populate dataTransfer.files.
    const file = e.dataTransfer?.files[0];
    if (file) {
      await importFile(file);
      return;
    }

    // Fallback for Linux/WebKitGTK: dataTransfer.files is empty and getData for
    // text/uri-list returns "" despite being listed as a type (WebKitGTK bug).
    // The file URI ends up in text/html as the href or text content of an anchor.
    let uri: string | undefined;

    // Try text/uri-list first (works on some platforms).
    const uriList = e.dataTransfer?.getData("text/uri-list") ?? "";
    uri = uriList.split(/\r?\n/).find((line) => line.startsWith("file://"));

    // Fall back to parsing the file URI out of the text/html anchor.
    if (!uri) {
      const html = e.dataTransfer?.getData("text/html") ?? "";
      if (html) {
        const doc = new DOMParser().parseFromString(html, "text/html");
        const anchor = doc.querySelector("a");
        const candidate = anchor?.href || anchor?.textContent?.trim() || "";
        if (candidate.startsWith("file://")) uri = candidate;
      }
    }

    if (!uri) {
      importError = "Could not read dropped item — no file URI found.";
      return;
    }
    // Decode percent-encoded characters (e.g. %20 → space) and strip the scheme.
    const path = decodeURIComponent(new URL(uri).pathname);
    const fileName = path.split("/").pop() ?? path;
    importError = null;
    let text: string;
    try {
      text = await readFileText(path);
    } catch (e) {
      importError = e instanceof Error ? e.message : String(e);
      return;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      importError = `${fileName} is not valid JSON.`;
      return;
    }
    try {
      await saveProfile(parsed as Profile);
      await profileStore.reload();
    } catch (e) {
      importError = e instanceof Error ? e.message : String(e);
      logger.error("save_profile (import via URI) failed", e);
    }
  }

  // ── Missing-roles warning ───────────────────────────────────────────────────

  let activeProfile = $derived(
    profileStore.profiles.find((p) => p.layer_id === engineStore.activeLayerId),
  );
  let showDualWarning = $derived(
    activeProfile?.kind === "dual" && deviceStore.connected.length < 2,
  );
</script>

<div
  class="mx-auto max-w-2xl space-y-4 rounded-lg transition-colors
    {isDragOver ? 'outline outline-2 outline-primary bg-primary/5' : ''}"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  role="region"
  aria-label="Profile list — drop a JSON file to import"
>
  <!-- Header -->
  <div class="flex items-center justify-between">
    <h1 class="text-2xl font-bold">Profiles</h1>
    <div class="flex gap-2">
      <label class="btn btn-ghost btn-sm">
        Import
        <input
          type="file"
          accept=".json"
          class="hidden"
          onchange={handleImport}
        />
      </label>
      <button class="btn btn-primary btn-sm" onclick={openNewWizard}>
        + New
      </button>
    </div>
  </div>

  <!-- Dual-device warning -->
  {#if showDualWarning}
    <div class="alert alert-warning text-sm">
      <span>Active profile requires two connected devices.</span>
    </div>
  {/if}

  <!-- Action error -->
  {#if actionError}
    <div class="alert alert-error text-sm">
      <span>{actionError}</span>
    </div>
  {/if}

  <!-- Import error -->
  {#if importError}
    <div class="alert alert-error text-sm">
      <span>{importError}</span>
    </div>
  {/if}

  <!-- Load errors from registry reload -->
  {#each profileStore.loadErrors as err}
    {@const errLayerId = err.file_name.replace(/\.json$/, "")}
    <div class="alert alert-warning text-sm flex items-start justify-between gap-2">
      <span><strong>{err.file_name}</strong>: {err.message}</span>
      <button
        class="btn btn-xs btn-ghost btn-error flex-shrink-0"
        onclick={() => (deleteConfirmId = errLayerId)}
      >Delete</button>
    </div>
  {/each}

  <!-- Profile list -->
  {#if profileStore.profiles.length === 0}
    <div class="card bg-base-100 shadow">
      <div class="card-body">
        <p class="text-sm text-base-content/50">
          No profiles found. Create a new one or import a JSON file.
        </p>
      </div>
    </div>
  {:else}
    <div class="space-y-2">
      {#each profileStore.profiles as profile}
        {@const isActive = profile.layer_id === engineStore.activeLayerId}
        <div
          class="card bg-base-100 shadow transition-shadow
            {isActive ? 'ring-2 ring-primary' : ''}"
        >
          <div class="card-body flex-row items-center gap-4 py-3">
            <!-- Active indicator -->
            <div
              class="h-2.5 w-2.5 flex-shrink-0 rounded-full
                {isActive ? 'bg-primary' : 'bg-base-300'}"
              title={isActive ? "Active" : ""}
            ></div>

            <!-- Profile info -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2">
                <span class="font-semibold truncate">{profile.name}</span>
                <span
                  class="badge badge-sm
                    {profile.kind === 'dual' ? 'badge-secondary' : 'badge-ghost'}"
                >
                  {profile.kind}
                </span>
              </div>
              {#if profile.description}
                <p class="text-xs text-base-content/60 truncate mt-0.5">
                  {profile.description}
                </p>
              {/if}
            </div>

            <!-- Actions -->
            <div class="flex gap-2 flex-shrink-0">
              {#if isActive}
                <button
                  class="btn btn-sm btn-ghost btn-outline"
                  onclick={handleDeactivate}
                  disabled={deactivating}
                >
                  {#if deactivating}
                    <span class="loading loading-spinner loading-xs"></span>
                  {:else}
                    Deactivate
                  {/if}
                </button>
              {:else}
                <button
                  class="btn btn-sm btn-primary btn-outline"
                  onclick={() => handleActivate(profile.layer_id)}
                  disabled={activatingId === profile.layer_id}
                >
                  {#if activatingId === profile.layer_id}
                    <span class="loading loading-spinner loading-xs"></span>
                  {:else}
                    Activate
                  {/if}
                </button>
              {/if}
              <button
                class="btn btn-sm btn-ghost"
                onclick={() => goto(`/profiles/${profile.layer_id}/edit`)}
              >
                Edit
              </button>
              <button
                class="btn btn-sm btn-ghost btn-error"
                onclick={() => (deleteConfirmId = profile.layer_id)}
                disabled={deletingId === profile.layer_id}
              >
                {#if deletingId === profile.layer_id}
                  <span class="loading loading-spinner loading-xs"></span>
                {:else}
                  Delete
                {/if}
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Delete confirm modal -->
{#if deleteConfirmId}
  {@const name =
    profileStore.profiles.find((p) => p.layer_id === deleteConfirmId)?.name ??
    deleteConfirmId}
  <dialog class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold">Delete "{name}"?</h3>
      <p class="py-4 text-sm">This cannot be undone.</p>
      <div class="modal-action">
        <button class="btn btn-ghost" onclick={() => (deleteConfirmId = null)}>
          Cancel
        </button>
        <button
          class="btn btn-error"
          onclick={() => {
            const id = deleteConfirmId!;
            deleteConfirmId = null;
            handleDelete(id);
          }}
        >
          Delete
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={() => (deleteConfirmId = null)}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}

<!-- New profile wizard modal -->
{#if showNewWizard}
  <dialog class="modal modal-open">
    <div class="modal-box">
      <h3 class="mb-4 text-lg font-bold">New profile</h3>

      <div class="space-y-4">
        <!-- Name -->
        <label class="form-control w-full">
          <div class="label"><span class="label-text">Name</span></div>
          <input
            type="text"
            placeholder="My Profile"
            class="input input-bordered w-full"
            bind:value={wizardName}
          />
          {#if wizardLayerId}
            <div class="label">
              <span class="label-text-alt text-base-content/50">
                ID: {wizardLayerId}
              </span>
            </div>
          {/if}
        </label>

        <!-- Kind -->
        <div class="form-control">
          <div class="label"><span class="label-text">Kind</span></div>
          <div class="join">
            {#each ["single", "dual"] as k}
              <button
                class="btn join-item btn-sm
                  {wizardKind === k ? 'btn-primary' : 'btn-ghost'}"
                onclick={() => (wizardKind = k as ProfileKind)}
              >
                {k}
              </button>
            {/each}
          </div>
        </div>

        <!-- Hand (single only) -->
        {#if wizardKind === "single"}
          <div class="form-control">
            <div class="label"><span class="label-text">Hand</span></div>
            <div class="join">
              {#each ["right", "left"] as h}
                <button
                  class="btn join-item btn-sm
                    {wizardHand === h ? 'btn-primary' : 'btn-ghost'}"
                  onclick={() => (wizardHand = h as Hand)}
                >
                  {h}
                </button>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Description -->
        <label class="form-control w-full">
          <div class="label">
            <span class="label-text">Description</span>
            <span class="label-text-alt">optional</span>
          </div>
          <input
            type="text"
            placeholder="What is this profile for?"
            class="input input-bordered w-full"
            bind:value={wizardDescription}
          />
        </label>

        {#if wizardError}
          <div class="alert alert-error text-sm">
            <span>{wizardError}</span>
          </div>
        {/if}
      </div>

      <div class="modal-action">
        <button
          class="btn btn-ghost"
          onclick={() => (showNewWizard = false)}
          disabled={wizardSaving}
        >
          Cancel
        </button>
        <button
          class="btn btn-primary"
          onclick={handleCreateProfile}
          disabled={wizardSaving || !wizardName.trim()}
        >
          {#if wizardSaving}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            Create
          {/if}
        </button>
      </div>
    </div>
    <button
      class="modal-backdrop"
      onclick={() => (showNewWizard = false)}
      aria-label="Close dialog"
    ></button>
  </dialog>
{/if}
