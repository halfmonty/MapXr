<script lang="ts">
  interface Props {
    icon: string;
    title: string;
    description: string;
  }

  let { icon, title, description }: Props = $props();
</script>

<div class="card bg-base-200 shadow-sm hover:shadow-md transition-shadow">
  <div class="card-body">
    <div class="text-3xl mb-2">{icon}</div>
    <h2 class="card-title text-lg">{title}</h2>
    <p class="text-base-content/70 text-sm leading-relaxed">{description}</p>
  </div>
</div>
