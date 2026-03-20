<h1>Extended key support, OS notifications, and six silent bugs fixed</h1>

<p>
  Today had two distinct acts: shipping OS desktop notifications (Epic 16) and overhauling the
  keyboard key dispatch layer (Epic 17). The second one started as "add F13–F24 and media keys" and
  quickly turned into an audit that uncovered six categories of silent failure — keys that profiles
  happily accepted but that never actually fired.
</p>

<h2>OS desktop notifications (Epic 16)</h2>

<p>
  mapxr now sends native OS notifications for device connect/disconnect, layer switches, and profile
  switches. Each event type is independently toggleable in Settings, so users who switch layers
  constantly via combos don't have to live with a notification storm.
</p>

<p>
  A few design decisions worth noting. Notifications are best-effort — OS errors are logged at
  <code>warn!</code> and never propagated to the UI. The layer-switch toggle defaults to
  <em>off</em> for this reason. Profile switches default to on since they're infrequent and
  represent a meaningful mode change the user almost certainly wants to know about.
</p>

<p>
  Device notifications now include the human-readable device name and role in the body, e.g.
  <em>"TapXR_A0363 (Right) connected"</em>. Getting the name into the notification required
  fetching it from <code>TapDevice</code> before the map entry is removed at disconnect time — a
  subtle ordering issue that had caused the name to show as empty in an earlier attempt.
</p>

<p>
  On the same day: <code>save_profile</code> gained a hot-reload path. If the profile being saved
  is currently active in the engine's layer stack, the engine's <code>set_profile</code> is called
  immediately and a <code>layer-changed</code> event fires — no need to deactivate and reactivate
  to pick up edits.
</p>

<h2>The key audit (Epic 17)</h2>

<p>
  The work started with an enigo 0.2 source audit: what named keys does it actually expose, on which
  platforms, and how do those map to mapxr's key name strings? The answer was uncomfortable.
</p>

<p>
  The key dispatch path has two moving parts that need to stay in sync: <code>VALID_KEYS</code> in
  <code>mapping-core</code> (the list profiles are validated against at load time) and
  <code>name_to_key</code> in <code>pump.rs</code> (the function that converts a name string to an
  <code>enigo::Key</code> at fire time). They had drifted badly.
</p>

<h3>Bug 1 — All arrow keys silently broken</h3>

<p>
  <code>VALID_KEYS</code> uses <code>"left_arrow"</code>, <code>"right_arrow"</code>,
  <code>"up_arrow"</code>, <code>"down_arrow"</code>. <code>name_to_key</code> was matching
  <code>"left"</code>, <code>"right"</code>, <code>"up"</code>, <code>"down"</code>. Every arrow
  key in every profile was firing nothing and logging a warning. Since most profiles use
  <code>ctrl+c</code>-style shortcuts rather than arrows this went unnoticed.
</p>

<h3>Bug 2 — All function keys silently broken</h3>

<p>
  <code>VALID_KEYS</code> stores lowercase <code>"f1"</code>–<code>"f12"</code>.
  <code>name_to_key</code> was matching uppercase <code>"F1"</code>–<code>"F12"</code>. Zero
  function keys worked. The spec examples use <code>"f13"</code> and <code>"f15"</code> extensively
  for virtual-button tricks — all of those were silently no-ops.
</p>

<h3>Bugs 3–6 — F13–F24, punctuation, system keys, and media keys</h3>

<p>
  Even if the case had been right, F13–F24 had no arms at all. The same was true for every
  punctuation key (<code>grave</code>, <code>minus</code>, <code>left_bracket</code>, etc.),
  all locking/system keys (<code>caps_lock</code>, <code>insert</code>, <code>num_lock</code>,
  <code>print_screen</code>, <code>scroll_lock</code>), and all media/volume keys
  (<code>media_play</code>, <code>volume_up</code>, etc.). All were in <code>VALID_KEYS</code>,
  all silently no-oped.
</p>

<h2>The fix and new additions</h2>

<p>
  <code>name_to_key</code> was rewritten from scratch. The new version is around 130 lines of
  straightforward match arms organised by group. Platform-specific keys use
  <code>#[cfg(...)]</code> directly on match arms — on unsupported platforms those arms simply
  aren't compiled, so the name falls through to the catch-all <code>other</code> arm which logs a
  warning and returns <code>None</code>. No runtime platform detection needed.
</p>

<p>
  Five new keys were also added to the canonical list: <code>media_stop</code>,
  <code>pause</code>, <code>brightness_down</code> and <code>brightness_up</code> (macOS only),
  <code>eject</code> (macOS), and <code>mic_mute</code> (Linux). Platform availability is
  documented in a new <code>docs/spec/extended-keys-spec.md</code>.
</p>

<p>
  The platform matrix in brief:
</p>

<ul>
  <li><strong>Cross-platform:</strong> a–z, 0–9, F1–F20, all punctuation, all navigation (arrows, home/end, page up/down, backspace, delete, etc.), caps lock, media play/next/prev, volume up/down/mute</li>
  <li><strong>Windows + Linux, not macOS:</strong> insert, num lock, print screen, pause, media stop, F21–F24, scroll lock (Linux only)</li>
  <li><strong>macOS only:</strong> brightness down/up, eject</li>
  <li><strong>Linux only:</strong> mic mute, scroll lock</li>
</ul>

<h2>Key picker UI</h2>

<p>
  The key picker in the action editor was a plain text input with a <code>&lt;datalist&gt;</code>
  autocomplete — fine for a small list, but impractical with 100+ keys and no way to convey which
  keys are platform-limited. It's now a <code>&lt;select&gt;</code> with four
  <code>&lt;optgroup&gt;</code> sections: Standard, Navigation, Function, and Media / System.
  Platform-limited keys show a note in parentheses (e.g. <em>f24 (Windows / Linux)</em>) so users
  know before they bind.
</p>

<p>
  The key name data lives in a <code>KEY_GROUPS</code> constant in <code>types.ts</code> — the
  flat <code>KNOWN_KEY_NAMES</code> array that other parts of the codebase use is now derived from
  it via <code>flatMap</code>, so there's a single source of truth.
</p>

<h2>Haptic feedback (Epic 18)</h2>

<p>
  mapxr can now send vibration patterns to connected Tap devices. There are two surfaces: a
  <code>vibrate</code> action type in the profile editor (bind any tap code to a custom on/off
  sequence), and automatic event-driven haptics for tap confirmation, layer switches, and profile
  switches — each independently toggleable in Settings.
</p>

<p>
  The BLE protocol is straightforward. The haptic characteristic (<code>C3FF0009</code>) accepts a
  payload of alternating on/off durations encoded as <code>duration_ms / 10</code> per byte.
  Durations are clamped to [10, 2550] ms with 10 ms resolution, and sequences longer than 18
  elements are truncated before sending.
</p>

<p>
  Shipping it surfaced two bugs, one predictable and one not.
</p>

<h3>Bug 1 — Context monitor firing haptics on every browser tab switch</h3>

<p>
  After connecting a device and triggering a layer shift, the device would vibrate again and again
  without any user input. The culprit was the Wayland focus monitor: it calls
  <code>publish_focused()</code> on every <code>Done</code> event from any toplevel, which includes
  window <em>title</em> changes of the already-focused window — browser tabs, document titles, anything.
  The context monitor had no guard against re-applying a profile that was already active, so every
  title change on a matching window re-fired the profile-switch haptic. The fix was a one-line
  idempotency check: skip the activation if <code>last_active_profile_id</code> already matches the
  rule's target.
</p>

<h3>Bug 2 — Every vibrate action produced a shower of phantom buzzes</h3>

<p>
  Even with all event-driven haptics disabled, firing a <code>vibrate</code> action with pattern
  <code>[1000, 100, 200]</code> would produce the correct long-then-short buzz followed by three to
  five additional random-length vibrations. The first suspects — duplicate BLE notifications,
  double-tap buffering, the context monitor — were all ruled out by adding diagnostic logging that
  confirmed a single BLE notification and a single software dispatch per physical tap.
</p>

<p>
  The real cause was in <code>VibrationPattern::encode()</code>. Our implementation sent only as
  many bytes as the pattern contained — five bytes for a three-element pattern. The Tap device
  firmware, it turns out, requires exactly 20 bytes (2-byte header + 18 duration slots). When the
  payload is short, the firmware reads the remainder from uninitialised RAM and plays whatever
  garbage values it finds as additional durations. The fix came from comparing against the C# SDK,
  which always zero-initialises a 20-byte buffer before filling in the pattern. We now do the same:
  <code>encode()</code> always returns a fixed 20-byte payload with unused slots explicitly zeroed.
</p>
