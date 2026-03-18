<script lang="ts">
  import { base } from '$app/paths';
</script>

<div class="hero min-h-[70vh] bg-base-100">
  <div class="hero-content text-center max-w-3xl">
    <div>
      <div class="badge badge-primary badge-outline mb-4">Open Source · MIT License</div>
      <h1 class="text-5xl md:text-6xl font-bold mb-6 leading-tight">
        Your TAP.<br />
        <span class="text-primary">Your rules.</span>
      </h1>
      <p class="text-xl text-base-content/70 mb-8 max-w-2xl mx-auto leading-relaxed">
        MapXr lets you remap every finger tap on your TAP Strap wearable keyboard into keyboard
        shortcuts, mouse clicks, app-specific macros, and complex multi-key combos — without writing
        a single line of code.
      </p>
      <div class="flex flex-wrap gap-3 justify-center">
        <a
          href="https://github.com/mapxr/releases"
          target="_blank"
          rel="noreferrer"
          class="btn btn-primary btn-lg"
        >
          Download for Free
        </a>
        <a href="{base}/docs" class="btn btn-outline btn-lg"> Read the Docs </a>
      </div>
    </div>
  </div>
</div>
