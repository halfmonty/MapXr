<h1>Hardware debounce fix + Windows CI</h1>

<p>
  Two separate but both overdue pieces of work landed in this session: a fix for an infuriating
  double-tap inversion bug, and the first working Windows build pipeline.
</p>

<h2>The double-tap inversion bug</h2>

<p>
  Users reported that intentional double-taps were sometimes firing the single-tap action instead,
  and single taps were occasionally triggering double-tap actions. After adding some diagnostic
  logging, the root cause became clear.
</p>

<p>
  The TAP Strap hardware emits spurious duplicate BLE notifications within roughly 10–30 ms of a
  genuine tap event. This is normal for the hardware — it's a byproduct of how the firmware
  debounces the capacitive sensors internally — but it was invisible to MapXr's engine.
</p>

<p>
  With a double-tap binding present, the sequence looked like this to the engine:
</p>

<ol>
  <li>Genuine tap event → engine starts waiting for a possible second tap</li>
  <li>Hardware bounce (8 ms later) → engine sees a "second tap", advances to <code>TapPending::Two</code>, fires double-tap action</li>
  <li>User's actual intended second tap (150 ms later) → engine has already resolved, starts a new single-tap sequence</li>
</ol>

<p>
  The fix is a 50 ms debounce window per device: if the same tap code arrives from the same device
  within 50 ms of the previous event, it's silently discarded. Real double-taps arrive 100–300 ms
  apart, well outside the window. Dual-device profiles (where two TAP Straps legitimately stack
  same-device events for cross-device combo detection) are exempt from debouncing.
</p>

<h2>Windows CI via GitHub Actions</h2>

<p>
  The workflow uses <code>tauri-apps/tauri-action@v0</code> on a <code>windows-latest</code> runner.
  It produces both an <code>.msi</code> installer and an NSIS <code>.exe</code> as build artifacts,
  uploaded to the GitHub Actions run. The workflow triggers on <code>workflow_dispatch</code> or a
  <code>v*</code> tag push, so it doesn't burn CI minutes on every commit.
</p>

<p>
  <code>GITHUB_TOKEN</code> is a GitHub built-in secret — no manual secret creation needed. The
  repo just needs "Read and write permissions" enabled under Settings → Actions → General for the
  release upload step to work.
</p>
