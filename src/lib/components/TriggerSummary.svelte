<script lang="ts">
  import type { Trigger } from "$lib/types";

  interface Props {
    trigger: Trigger;
  }

  let { trigger }: Props = $props();

  function triggerCodeToSymbols(trigger: string): string {
    return trigger.replace(/x/g, "●").replace(/o/g, "○");
  }

  let summary = $derived(
    (() => {
      switch (trigger.type) {
        case "tap":
          return `Tap ${triggerCodeToSymbols(trigger.code)}`;
        case "double_tap":
          return `Double tap ${trigger.code}`;
        case "triple_tap":
          return `Triple tap ${trigger.code}`;
        case "sequence":
          return `Sequence [${trigger.steps.length} step${trigger.steps.length === 1 ? "" : "s"}]`;
      }
    })()
  );
</script>

<span class="font-mono text-sm">{summary}</span>
