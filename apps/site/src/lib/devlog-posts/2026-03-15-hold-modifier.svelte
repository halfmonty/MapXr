<h1>Hold modifier action (sticky keys)</h1>

<p>
  This feature came out of a specific use case: one-handed typing with the TAP Strap. When only
  one hand is available, pressing Shift + a letter requires two separate taps: one to activate
  Shift, one for the letter. The <code>hold_modifier</code> action makes that possible.
</p>

<h2>What it does</h2>

<p>
  A <code>hold_modifier</code> action keeps a modifier key (Shift, Ctrl, Alt, etc.) held down at
  the OS level until a release condition is met. The next key event the OS sees will have that
  modifier applied, exactly as if you were physically holding the key.
</p>

<h2>Three release modes</h2>

<p>
  Different workflows need different release behaviour, so three modes were spec'd and implemented:
</p>

<dl>
  <dt><strong>Toggle</strong></dt>
  <dd>
    The first tap holds the modifier; any subsequent tap from the user releases it. Good for
    capitalising a single character.
  </dd>

  <dt><strong>Count (n)</strong></dt>
  <dd>
    The modifier stays held for exactly <em>n</em> key events, then releases automatically. Useful
    for capitalising a fixed-length word.
  </dd>

  <dt><strong>Timeout (ms)</strong></dt>
  <dd>
    The modifier releases after a specified number of milliseconds regardless of what the user
    does. Good for press-and-hold style shortcuts.
  </dd>
</dl>

<h2>Validation rules</h2>

<p>
  Several validation rules were added to catch common mistakes at profile load time rather than
  at runtime:
</p>

<ul>
  <li>A <code>hold_modifier</code> cannot target the same key as a trigger in the same layer (would create an unresolvable loop)</li>
  <li>Count must be ≥ 1</li>
  <li>Timeout must be &gt; 0 ms</li>
  <li>The modifier key must be a recognised modifier (<code>shift</code>, <code>ctrl</code>, <code>alt</code>, <code>meta</code>)</li>
</ul>

<h2>Test coverage</h2>

<p>
  15 new unit tests cover the happy paths for all three modes plus all validation failure cases.
  The spec was written and confirmed before any implementation code was written, per project rules.
</p>
