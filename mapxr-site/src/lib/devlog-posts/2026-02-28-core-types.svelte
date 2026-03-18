<h1>Core types and JSON schema</h1>

<p>
  The first real session on MapXr after the initial project setup. The goal was to define the
  complete data model that everything else would build on — get this right and the rest follows
  naturally; get it wrong and every subsequent layer pays the price.
</p>

<h2>The type hierarchy</h2>

<p>
  The core model is intentionally flat and serialization-first. Everything is designed to round-trip
  cleanly through <code>serde_json</code>:
</p>

<ul>
  <li><strong>TapCode</strong> — a 5-bit finger mask (one bit per finger, thumb to pinky) plus a device identifier. This is the raw event the BLE layer emits.</li>
  <li><strong>Trigger</strong> — a single, double, or cross-device combo specification. Wraps one or two TapCodes with timing semantics.</li>
  <li><strong>Action</strong> — what happens when a trigger fires. An enum covering key output, text typing, mouse, layer switching, and hold_modifier.</li>
  <li><strong>Mapping</strong> — a (Trigger, Action) pair.</li>
  <li><strong>Layer</strong> — a named list of Mappings.</li>
  <li><strong>Profile</strong> — the top-level document: metadata, default layer name, and a map of Layer names to Layer definitions.</li>
</ul>

<h2>Design decisions</h2>

<p>
  <strong>Why a 5-bit mask rather than named fingers?</strong> The TAP Strap hardware reports
  finger state as a bitmask. Keeping that representation in the type system means zero conversion
  overhead in the hot path (BLE event → engine lookup) and makes test fixtures trivial to write.
</p>

<p>
  <strong>Why serde's adjacently-tagged enum representation?</strong> The JSON
  <code>&#123;"type": "key", "key": "ctrl+c"&#125;</code> shape is readable by humans editing profiles by
  hand, and unambiguous for the parser. Internal enum tags would leak Rust naming conventions into
  the user-facing format.
</p>

<h2>Test fixtures</h2>

<p>
  Sample profile JSON files live in <code>crates/mapping-core/tests/fixtures/</code> and are
  loaded via <code>include_str!()</code> in tests. This tests the full deserialization path with
  real files rather than inline strings — catching any mismatch between the schema spec and the
  actual Rust types.
</p>
