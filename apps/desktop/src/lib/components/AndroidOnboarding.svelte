<script lang="ts">
  import { onMount } from "svelte";
  import { getAndroidPreferences } from "$lib/commands";
  import AccessibilitySetupPrompt from "./AccessibilitySetupPrompt.svelte";
  import BatterySetupWizard from "./BatterySetupWizard.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  type Phase = "accessibility" | "battery";
  let phase = $state<Phase | null>(null);
  let initialized = $state(false);

  onMount(async () => {
    if (!open) return;
    await init();
  });

  async function init() {
    try {
      const prefs = await getAndroidPreferences();
      if (!prefs.accessibility_setup_done) {
        phase = "accessibility";
      } else if (!prefs.battery_setup_done) {
        phase = "battery";
      } else {
        // Both steps done — nothing to show.
        onClose();
        return;
      }
    } catch {
      // If we can't read prefs, skip onboarding gracefully.
      onClose();
      return;
    }
    initialized = true;
  }

  /** Called when the user completes or dismisses the accessibility step. */
  function onAccessibilityDone() {
    // Advance to battery setup regardless of whether accessibility was granted.
    phase = "battery";
  }

  /** Called when the user completes or dismisses the battery setup step. */
  function onBatteryDone() {
    onClose();
  }
</script>

{#if initialized}
  {#if phase === "accessibility"}
    <AccessibilitySetupPrompt
      open={open}
      onClose={onClose}
      onDone={onAccessibilityDone}
    />
  {:else if phase === "battery"}
    <BatterySetupWizard
      open={open}
      onClose={onClose}
      onDone={onBatteryDone}
    />
  {/if}
{/if}
