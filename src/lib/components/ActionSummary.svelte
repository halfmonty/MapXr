<script lang="ts">
  import type { Action } from "$lib/types";

  interface Props {
    action: Action;
  }

  let { action }: Props = $props();

  let summary = $derived((() => {
    switch (action.type) {
      case "key": {
        const mods = action.modifiers?.length ? action.modifiers.join("+") + "+" : "";
        return `Key ${mods}${action.key}`;
      }
      case "key_chord":    return `Chord ${action.keys.join("+")}`;
      case "type_string":  return `Type: ${action.text.slice(0, 30)}${action.text.length > 30 ? "…" : ""}`;
      case "macro":        return `Macro [${action.steps.length} step${action.steps.length === 1 ? "" : "s"}]`;
      case "push_layer":   return `Push layer: ${action.layer}`;
      case "pop_layer":    return "Pop layer";
      case "switch_layer": return `Switch layer: ${action.layer}`;
      case "toggle_variable": return `Toggle: ${action.variable}`;
      case "set_variable": return `Set ${action.variable} = ${JSON.stringify(action.value)}`;
      case "block":        return "Block";
      case "alias":        return `Alias: ${action.name}`;
    }
  })());
</script>

<span class="text-sm">{summary}</span>
