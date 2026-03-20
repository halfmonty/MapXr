<script>
    import { base } from "$app/paths";
</script>

<h1>Actions</h1>

<p>
    An <strong>action</strong> is what happens when a map tap binding fires. MapXr
    supports keyboard output, mouse buttons, layer switching, and sticky modifier
    keys, macros, key chords, altering variables and more.
</p>

<h2>Key action</h2>

<p>
    Send a key combination to the active window, optionally with the modifiers
    selected also pressed.
</p>

<h2>Key Chord</h2>

<p>Send multiple keys at once.</p>

<h2>Type String</h2>

<p>
    Allows you to define a string of letters to type. Perhaps if you type
    something frequently enough this could be useful.
</p>

<h2>Macro</h2>

<p>
    A macro is a sequence of actions. If you want a tap to perform multiple
    actions, you can define a macro for it and even specify a delay between each
    action.
</p>

<h2>Push Layer</h2>

<p>
    Pushed the specified profile onto the layer stack, for more info see the <a
        class="link link-primary"
        href="{base}/docs/layers">layers page</a
    >.
</p>

<h2>Pop Layer</h2>

<p>
    Removes the current layer from the stack, for more info see the <a
        class="link link-primary"
        href="{base}/docs/layers">layers page</a
    >.
</p>

<h2>Switch Layer</h2>

<p>
    Ignoring the layer stack, this action just switches the current active layer
    to the specified profile. For more info see the <a
        class="link link-primary"
        href="{base}/docs/layers">layers page</a
    >.
</p>

<h2>Toggle Variable</h2>

<p>
    Toggles a variable from true to false and from false to true. For more info
    see the variables page.
</p>

<h2>Set Variable</h2>

<p>
    Explicitly sets a variable to a specific value. For more info see the
    variables page.
</p>

<h2>Conditional</h2>

<p>
    Branching action logic based on the state of a specified variable. For more
    info see the variables page.
</p>

<h2>Block</h2>

<p>
    An explicitly empty action that will do nothing even if the current layer
    has passthrough enabled. For more info see the <a
        class="link link-primary"
        href="{base}/docs/layers">layers page</a
    >.
</p>

<h2>Alias</h2>

<p>
    This will call a pre-defined action by name. For more info see the <a
        class="link link-primary"
        href="{base}/docs/aliases">aliases page</a
    >.
</p>

<h2>Hold Modifier</h2>

<p>
    This will hold the specified modifier keys for the next registered tap
    actions. It has the modes toggle, count and timeout.
    <br />
    Toggle just turns the modifiers on and will turn them off when repeated.
    <br />
    Count turns the modifiers on for a specified number of next taps and then turns
    them off.
    <br />
    Timeout turns the modifiers on for a specified duration and then turns them off.
</p>

<h2>Mouse Click</h2>

<p>Fires a selected mouse click at the current cursor position.</p>

<h2>Mouse Double Click</h2>

<p>Fires a selected mouse double click at the current cursor position.</p>

<h2>Mouse Scroll</h2>

<p>Performs a mouse scroll action in the specified direction.</p>

<h2>Vibrate</h2>

<p>
    You can specify a vibration pattern to send to the device. Not useful on
    it's own but can be paired with a macro.
</p>
