<script lang="ts">
  import type { Trigger } from "$lib/types";

  interface Props {
    trigger: Trigger;
  }

  let { trigger }: Props = $props();

  let summary = $derived((() => {
    switch (trigger.type) {
      case "tap":        return `Tap ${trigger.code}`;
      case "double_tap": return `Double tap ${trigger.code}`;
      case "triple_tap": return `Triple tap ${trigger.code}`;
      case "sequence":   return `Sequence [${trigger.steps.length} step${trigger.steps.length === 1 ? "" : "s"}]`;
    }
  })());
</script>

<span class="font-mono text-sm">{summary}</span>
