<h1>Profile validation and layer stack</h1>

<p>
  A MapXr profile is a JSON file that users write by hand. That means the engine needs to catch
  mistakes clearly and early — at load time, not at runtime when a tap fires the wrong action.
  This session focused on building out that validation layer and the LayerStack that manages
  runtime layer switching.
</p>

<h2>Profile validation</h2>

<p>
  Validation runs immediately after deserialization, before any profile is made active.
  The rules enforced include:
</p>

<ul>
  <li><strong>Duplicate triggers</strong> — the same finger combination bound to two actions in the same layer is always a mistake</li>
  <li><strong>Missing layer references</strong> — a <code>push_layer</code> action that names a layer not defined in the profile</li>
  <li><strong>Empty layers</strong> — a layer with no mappings is valid but suspicious; flagged as a warning</li>
  <li><strong>Hold modifier constraints</strong> — the modifier key in a <code>hold_modifier</code> action can't also be a trigger key in the same layer</li>
  <li><strong>Default layer must exist</strong> — the <code>default_layer</code> field must name a layer actually defined in the profile</li>
</ul>

<p>
  Each rule has at least one passing test and one failing test in the unit suite, as required by
  the project's testing rules.
</p>

<h2>LayerStack</h2>

<p>
  The LayerStack maintains the ordered list of active layers at runtime. Trigger resolution walks
  the stack from top to bottom — the first layer with a matching mapping wins, lower layers act
  as fallbacks.
</p>

<p>
  Three operations are supported:
</p>

<dl>
  <dt><strong>push_layer(name)</strong></dt>
  <dd>Adds a layer on top of the current stack. The previous layers remain as fallbacks.</dd>

  <dt><strong>pop_layer()</strong></dt>
  <dd>Removes the topmost layer. Has no effect if only the default layer remains.</dd>

  <dt><strong>activate_layer(name)</strong></dt>
  <dd>Replaces the entire stack with a single layer. Used for switching contexts completely.</dd>
</dl>
