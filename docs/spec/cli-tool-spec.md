# Epic 8 — Profile normalisation CLI tool (`tap-cli`)

## Overview

`tap-cli` is a standalone binary that manipulates mapxr profile JSON files from the command line,
without requiring the GUI or a connected device. It is a thin wrapper around `mapping-core`.

Crate path: `crates/tap-cli/`
Binary name: `tap-mapper`
Error handling: `anyhow` (binary crate)

---

## Commands

### `tap-mapper validate <file>`

Load and validate a profile file, print all errors to stderr with context.

- Calls `Profile::load(path)`
- On success: print `OK: <file>` to stdout, exit 0
- On failure: print each `ProfileError` variant on its own line to stderr, exit 1
- If the file does not exist: print "Error: file not found: <path>", exit 2

### `tap-mapper normalize <file> [--dry-run] [--output <path>]`

Rewrite all legacy integer `TapCode` values to finger-pattern strings.

- Loads the raw JSON without full validation (just parse)
- Walks all `TapCode` fields and converts integers to the canonical `"ooooo"` string form
- If `--dry-run`: print the normalised JSON to stdout, do not write
- If `--output <path>`: write to `<path>` instead of overwriting `<file>`
- Otherwise: overwrite `<file>` in place (write-then-rename for atomicity, same as `Profile::save`)
- Prints `Normalized: <n> codes updated` (or `0 codes updated` if already clean)
- Exit 0 on success, 1 on parse error

### `tap-mapper migrate <file> [--output <path>]`

Apply pending schema version migrations.

- Currently only version 1 exists; this command is a no-op that validates the version field
  and prints `Already at latest schema version (1)` or `Migrated from version <x> to 1`
- Designed as a forward-compatible hook: future schema versions add migration steps here
- `--output <path>` flag behaves the same as in `normalize`
- Exit 0 on success, 1 on error

### `tap-mapper lint <file>`

Run `validate` plus optional style warnings.

- All errors from `validate` are also lint errors
- Additional warnings (exit 0 even if warnings present, exit 1 on errors):
  - Overloaded codes without an explicit `overload_strategy` set → WARN
  - `combo_window_ms` < 30 → WARN "very short combo window may cause missed combos"
  - `double_tap_window_ms` < 100 → WARN "very short double-tap window may cause missed double-taps"
  - Mapping with no label → WARN "mapping at index <n> has no label"
- Output format: `<file>:<level>: <message>` (e.g. `profile.json:WARN: overloaded codes without overload_strategy`)
- Exit 0 if no errors (warnings are OK), exit 1 if any errors

---

## Argument parsing

Use the `clap` crate (derive API). Approved dependency for `tap-cli` only.

---

## Error output

All errors go to stderr. Stdout is reserved for normalised JSON (`--dry-run`) and status messages.

---

## Integration tests

Tests live in `crates/tap-cli/tests/`. Fixture files are shared with `mapping-core` via a relative
path where possible, and supplemented with CLI-specific fixtures in `crates/tap-cli/tests/fixtures/`.

Each command gets at least:
- One test with a valid input (expected exit 0)
- One test with an invalid input (expected exit 1)
- `normalize`: one test with legacy integer codes, one with clean codes (idempotent)

---

## File layout

```
crates/tap-cli/
├── Cargo.toml
└── src/
    ├── main.rs      ← clap app definition, command dispatch
    ├── validate.rs  ← validate command implementation
    ├── normalize.rs
    ├── migrate.rs
    └── lint.rs
```

---

## Approved dependencies

| Crate   | Version | Reason                        |
| ------- | ------- | ----------------------------- |
| `clap`  | 4.x     | Argument parsing (derive API) |
| `anyhow`| 1.x     | Error handling (binary crate) |
