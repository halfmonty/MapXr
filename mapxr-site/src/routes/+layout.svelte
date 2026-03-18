<script lang="ts">
  import '../app.css';
  import Nav from '$lib/components/Nav.svelte';
  import Footer from '$lib/components/Footer.svelte';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="min-h-screen flex flex-col bg-base-100">
  <Nav />
  <div class="flex-1">
    {@render children()}
  </div>
  <Footer />
</div>
