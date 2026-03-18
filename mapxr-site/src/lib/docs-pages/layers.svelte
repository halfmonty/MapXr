<script lang="ts">
  import { base } from '$app/paths';
</script>

<h1>Layers</h1>

<p>
  <strong>Layers</strong> let you define multiple sets of mappings within a single profile and
  switch between them at runtime. Think of layers like the Fn key on a laptop keyboard —
  the same physical key produces different output depending on the active layer.
</p>

<h2>Defining layers</h2>

<p>Each layer is a named object inside the <code>layers</code> map in your profile:</p>

<pre><code>{`{
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [ ... ]
    },
    "nav": {
      "mappings": [ ... ]
    },
    "symbols": {
      "mappings": [ ... ]
    }
  }
}`}</code></pre>

<h2>The layer stack</h2>

<p>
  mapxr maintains a <strong>layer stack</strong>. At startup the stack contains only the
  <code>default_layer</code>. You can push additional layers on top or pop back to previous layers.
</p>

<p>
  When resolving a trigger, mapxr searches from the top of the stack downward.
  The first matching mapping wins.
</p>

<pre><code>{`Stack (top to bottom):
  nav      ← searched first
  base     ← searched second (fallback)`}</code></pre>

<h2>Switching layers</h2>

<p>Use layer actions in your mappings to navigate the stack:</p>

<pre><code>{`// Enter nav layer (stacks on top of base)
{ "type": "push_layer", "layer": "nav" }

// Return to base
{ "type": "pop_layer" }

// Jump directly to symbols, clearing the stack
{ "type": "activate_layer", "layer": "symbols" }`}</code></pre>

<h2>Example: nav layer</h2>

<pre><code>{`{
  "name": "Productivity",
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [1,1,0,0,0] },
          "action":  { "type": "push_layer", "layer": "nav" }
        }
      ]
    },
    "nav": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [0,0,0,0,1] },
          "action":  { "type": "pop_layer" }
        },
        {
          "trigger": { "type": "single", "device": "right", "fingers": [1,0,0,0,0] },
          "action":  { "type": "key", "key": "up" }
        }
      ]
    }
  }
}`}</code></pre>

<div class="alert alert-info mt-6">
  <span>
    Layer actions are documented in full on the
    <a href="{base}/docs/actions" class="link">Actions</a> page.
  </span>
</div>
