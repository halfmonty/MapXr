<h1>Actions</h1>

<p>
  An <strong>action</strong> is what happens when a trigger fires. mapxr supports keyboard output,
  mouse control, layer switching, and sticky modifier keys.
</p>

<h2>Key action</h2>

<p>Send a key combination to the active window:</p>

<pre><code>{`{ "type": "key", "key": "ctrl+shift+p" }`}</code></pre>

<p>
  Key strings follow the format <code>modifier+key</code>. Multiple modifiers can be chained:
  <code>ctrl+alt+delete</code>. Key names match the
  <a href="https://docs.rs/enigo/latest/enigo/enum.Key.html" target="_blank" rel="noreferrer">enigo key list</a>.
</p>

<h2>Type action</h2>

<p>Type a string of text character by character:</p>

<pre><code>{`{ "type": "type", "text": "Hello, world!" }`}</code></pre>

<h2>Mouse action</h2>

<pre><code>{`{ "type": "mouse_click", "button": "left" }
{ "type": "mouse_click", "button": "right" }
{ "type": "mouse_scroll", "direction": "up", "amount": 3 }`}</code></pre>

<h2>Layer action</h2>

<p>Push a new layer onto the layer stack, or pop back to the previous one:</p>

<pre><code>{`{ "type": "push_layer",    "layer": "nav" }
{ "type": "pop_layer" }
{ "type": "activate_layer", "layer": "symbols" }`}</code></pre>

<p>
  <code>push_layer</code> adds a layer on top — pop it later with <code>pop_layer</code>.<br />
  <code>activate_layer</code> replaces the entire stack with a single layer.
</p>

<h2>Hold modifier action</h2>

<p>Simulate a sticky modifier key that stays held until released by a second tap:</p>

<pre><code>{`{
  "type": "hold_modifier",
  "key": "shift",
  "mode": { "type": "toggle" }
}`}</code></pre>

<div class="overflow-x-auto">
  <table class="table">
    <thead>
      <tr><th>Mode</th><th>Behaviour</th></tr>
    </thead>
    <tbody>
      <tr>
        <td><code>toggle</code></td>
        <td>First tap holds the key; second tap releases it</td>
      </tr>
      <tr>
        <td><code>count</code></td>
        <td>Holds for N taps then auto-releases</td>
      </tr>
      <tr>
        <td><code>timeout</code></td>
        <td>Auto-releases after N milliseconds</td>
      </tr>
    </tbody>
  </table>
</div>

<h2>No-op action</h2>

<pre><code>{`{ "type": "noop" }`}</code></pre>

<p>Explicitly consume a trigger without doing anything. Useful for disabling inherited mappings in a child layer.</p>
