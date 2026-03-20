<script lang="ts">
    import { base } from "$app/paths";
</script>

<h1>Layers</h1>

<p>
    <strong>Layers</strong> let you define multiple profiles of mappings and switch
    between them at runtime. Think of layers like the Fn key on a laptop keyboard
    — the same physical key produces different output depending on the active layer.
</p>

<h2>Defining layers</h2>

<p>
    A layer is just a profile that is loaded into the layer stack when
    activated. So defining a profile and then activating it with a layer action
    is all you need to do to use a profile as a layer.
</p>

<h2>The layer stack</h2>

<p>
    mapxr maintains a <strong>layer stack</strong>. At startup the stack
    contains only the
    <code>default_layer</code>. You can push additional layers on top or pop
    back to previous layers.
</p>

<p>
    When resolving a trigger, mapxr searches from the top of the stack downward.
    The first matching mapping wins.
</p>

<pre><code
        >{`Stack (top to bottom):
  nav      ← searched first
  base     ← searched second (fallback)`}</code
    ></pre>

<h2>Switching layers</h2>

<p>Use layer actions in your mappings to navigate the stack:</p>

<p>
    A push layer action pushes the specified profile onto the stack above the
    current layer.
</p>

<p>
    A pop layer action removes the top layer from the stack, returning to the
    previous layer.
</p>

<p>
    An activate layer action clears the stack and pushes the specified profile
    onto it, making it the new top layer.
</p>

<h2>Layer Passthrough</h2>
<p>
    <strong>Layer passthrough</strong> allows you to define a layer that passes through
    to the base layer when no matching mapping is found. So if you have layer 'A'
    above a layer 'B' with passthrough enabled, and no mapping is found for a trigger
    on 'A', mapxr will search layer 'B'.
</p>

<p>
    To use layer passthrough, set the <code>passthrough</code> property to
    <code>true</code> in your profile settings tab
</p>

<p>
    To prevent a trigger from being passed through to the base layer, you can
    set that trigger's action to <code>block</code>
</p>
