<script lang="ts">
  import { onMount } from "svelte";
  import { getAndroidPreferences } from "$lib/commands";
  import BatterySetupWizard from "./BatterySetupWizard.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  let initialized = $state(false);

  onMount(async () => {
    if (!open) return;
    await init();
  });

  async function init() {
    try {
      const prefs = await getAndroidPreferences();
      if (!prefs.battery_setup_done) {
        initialized = true;
      } else {
        // Battery setup already done — nothing to show.
        onClose();
      }
    } catch {
      // If we can't read prefs, skip onboarding gracefully.
      onClose();
    }
  }
</script>

{#if initialized}
  <BatterySetupWizard
    open={open}
    onClose={onClose}
    onDone={onClose}
  />
{/if}
