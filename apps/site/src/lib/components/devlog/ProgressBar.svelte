<script lang="ts">
  interface Props {
    value: number;
    max: number;
    label?: string;
  }

  let { value, max, label }: Props = $props();

  let pct = $derived(Math.round((value / max) * 100));
</script>

<div class="w-full">
  {#if label}
    <div class="flex justify-between text-sm mb-1">
      <span class="text-base-content/70">{label}</span>
      <span class="font-mono font-semibold text-primary">{pct}%</span>
    </div>
  {/if}
  <progress class="progress progress-primary w-full" {value} {max}></progress>
</div>
