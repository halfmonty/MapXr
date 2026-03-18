<h1>Triggers</h1>

<p>
  A <strong>trigger</strong> describes the tap gesture that activates a mapping.
  Each trigger specifies which device (left/right hand or both), which fingers are down,
  and how many times the gesture must be tapped.
</p>

<h2>Finger notation</h2>

<p>
  Fingers are represented as a 5-element array of <code>0</code> (up) and <code>1</code> (down),
  ordered thumb → pinky:
</p>

<pre><code>{`[thumb, index, middle, ring, pinky]
[1, 0, 0, 0, 0]  // thumb only
[0, 1, 1, 0, 0]  // index + middle
[1, 1, 1, 1, 1]  // all five`}</code></pre>

<h2>Single tap</h2>

<pre><code>{`{
  "type": "single",
  "device": "left",
  "fingers": [1, 0, 0, 0, 0]
}`}</code></pre>

<h2>Double tap</h2>

<pre><code>{`{
  "type": "double",
  "device": "left",
  "fingers": [1, 0, 0, 0, 0]
}`}</code></pre>

<p>
  The same gesture tapped twice within the combo window (default 300 ms) fires this trigger.
</p>

<h2>Cross-device combo</h2>

<pre><code>{`{
  "type": "combo",
  "first":  { "device": "left",  "fingers": [1, 0, 0, 0, 0] },
  "second": { "device": "right", "fingers": [0, 1, 0, 0, 0] }
}`}</code></pre>

<p>
  Two gestures on different hands within the combo window fire as a single combo event.
</p>

<h2>Device values</h2>

<div class="overflow-x-auto">
  <table class="table">
    <thead>
      <tr><th>Value</th><th>Meaning</th></tr>
    </thead>
    <tbody>
      <tr><td><code>"left"</code></td><td>Left-hand TAP Strap only</td></tr>
      <tr><td><code>"right"</code></td><td>Right-hand TAP Strap only</td></tr>
      <tr><td><code>"any"</code></td><td>Either device (first one to fire)</td></tr>
    </tbody>
  </table>
</div>
