<h1>Profile persistence and startup profile selection</h1>

<p>
  Two related quality-of-life improvements landed today around how the app decides which profile
  to activate at startup, and how it remembers that decision across sessions.
</p>

<h2>The problem: alphabetical roulette</h2>

<p>
  Before this change, the startup profile was determined by a single line:
</p>

<pre><code>layer_registry.profiles().min_by_key(|p| &amp;p.name)</code></pre>

<p>
  Whichever profile name sorted first alphabetically became the active profile on every launch.
  No "last used" memory, no user intent. Just alphabetical order. If you had spent the session
  using a specific profile, closing and reopening the app silently reset to whatever name happened
  to sort first.
</p>

<h2>The fix: preferences.json</h2>

<p>
  A new <code>preferences.json</code> file lives alongside <code>devices.json</code> in the app
  config directory. It is written every time the user explicitly activates or deactivates a profile.
</p>

<pre><code>{JSON.stringify({
  "version": 1,
  "profile_active": true,
  "last_active_profile_id": "starter-right"
  }, null, 2)}</code></pre>

<p>
  On startup, the backend reads this file before initialising the engine. The selection logic is:
</p>

<ol>
  <li>If <code>profile_active</code> is <code>false</code>: start with the built-in empty profile (no mappings). The user explicitly asked for no profile.</li>
  <li>If <code>profile_active</code> is <code>true</code> and <code>last_active_profile_id</code> names a profile that still exists: activate it.</li>
  <li>If the named profile no longer exists (deleted between sessions): fall back to alphabetically-first.</li>
  <li>If there are no profiles at all: use the built-in empty profile.</li>
</ol>

<p>
  Persistence lives on the Rust side so the engine is initialised with the correct profile before
  the frontend even loads. Doing this in <code>localStorage</code> and calling
  <code>activate_profile</code> from the frontend would have caused a brief flicker. The wrong
  profile would be active during the window between page load and the first invoke.
</p>

<h2>Bug: deactivate didn't stick across restarts</h2>

<p>
  The initial implementation had a subtle flaw. When <code>deactivate_profile</code> was called,
  it saved <code>last_active_profile_id: null</code> to disk. On the next launch, this was
  indistinguishable from a first-launch state where <code>preferences.json</code> doesn't exist
  yet. In both cases the field is absent or null, and both fell through to the alphabetical
  fallback. So the app always activated a profile on restart regardless of whether the user had
  explicitly deactivated.
</p>

<p>
  The fix is the <code>profile_active</code> boolean. It defaults to <code>true</code> (so
  first-launch still picks the seeded starter profile), and is only set to <code>false</code> by
  an explicit <code>deactivate_profile</code> call. The startup logic now checks this flag first
  and short-circuits to the empty built-in if it is <code>false</code>, skipping all fallbacks.
</p>

<p>
  The field uses <code>#[serde(default = "default_true")]</code> so any
  <code>preferences.json</code> written before this field existed (or on a fresh install before
  the first deactivate) reads as <code>true</code> and behaves identically to before.
</p>

<h2>Device-aware profile suggestions</h2>

<p>
  A related UX gap: the app already warned when a dual profile was active with only one device
  connected, but said nothing about the reverse. Two devices connected with only a single-hand
  profile active. A dismissible suggestion banner now appears on the Devices page in that case:
</p>

<p>
  <em>"You have two devices connected. Consider switching to a dual profile."</em>
</p>

<p>
  The banner includes a direct link to the Profiles page and a Dismiss button. The dismiss is
  session-only and resets on the next launch so the user is reminded if the condition still holds.
  Auto-switching was deliberately not implemented: changing the active profile clears the layer
  stack and fires <code>on_exit</code> actions, which would be disruptive mid-session without
  explicit user intent.
</p>
