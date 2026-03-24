<h1>ComboEngine and cross-device combo resolution</h1>

<p>
  The ComboEngine is the most complex piece of MapXr's core logic. It has to correctly
  distinguish between a single tap, a double tap, and a two-handed combo. All from a stream
  of raw tap events arriving over Bluetooth with no inherent timing guarantees.
</p>

<h2>The resolution problem</h2>

<p>
  When a tap event arrives, the engine can't immediately know whether it's:
</p>

<ul>
  <li>A standalone single tap</li>
  <li>The first tap of a double-tap</li>
  <li>The first half of a cross-device combo</li>
</ul>

<p>
  It has to wait for a short window to see if more events arrive before resolving. This window
  (300 ms by default) is long enough to catch intentional combos but short enough that single
  taps feel instantaneous.
</p>

<h2>State machine</h2>

<p>
  The engine tracks a <code>TapPending</code> state per profile type:
</p>

<ul>
  <li><strong>None:</strong> no pending events</li>
  <li><strong>One(event, deadline):</strong> one event received, waiting for a possible second</li>
  <li><strong>Two(event1, event2):</strong> two events received, resolving as double-tap or combo</li>
</ul>

<p>
  When the deadline passes without a second event, the pending single tap resolves. When a second
  event arrives before the deadline, the engine checks whether they form a valid cross-device
  combo (different devices, matching a combo trigger) or a double-tap (same device, same fingers).
</p>

<h2>Deterministic tests with tokio::time</h2>

<p>
  Timing-sensitive tests use <code>tokio::time::pause()</code> and
  <code>tokio::time::advance(Duration)</code> to control the clock without actually sleeping.
  This makes the test suite fast and deterministic. The 300 ms combo window can be "waited out"
  in zero real time.
</p>

<p>
  The test suite covers single, double, cross-device combos, timeout resolution, rapid alternating
  events, and the dual-profile stacking behaviour where same-device events intentionally accumulate.
</p>
