# CLAUDE.md — mapxr project instructions

This file governs how Claude Code operates on this project. Read it in full at the start of every
session before touching any code. These instructions take precedence over general defaults.

---

## Core reference documents

Always read these documents before starting any work. They are the ground truth for this project.

| Document                           | Purpose                                                                        |
| ---------------------------------- | ------------------------------------------------------------------------------ |
| `docs/spec/mapping-core-spec.md`   | Full data model, JSON schema, engine behaviour rules                           |
| `docs/plan/implementation-plan.md` | Agile task list — the authoritative source of what to work on next             |
| `docs/log/`                        | Session progress logs — read the most recent entry to understand current state |

If any of these files are missing, stop and ask the user where to find them before proceeding.

---

## Workflow rules — read these carefully

### 1. Always check the implementation plan first

Before writing a single line of code, open `docs/plan/implementation-plan.md` and identify:

- The lowest-numbered incomplete epic
- The lowest-numbered incomplete task within that epic
- Whether that task has a corresponding spec document (see rule 2)

Work tasks in order. Do not skip ahead to a later task because it seems more interesting or
because an earlier task is blocked. If a task is blocked, say so and ask for guidance.

### 2. Spec-first — no code without an approved spec

Before beginning any new epic or major feature area, a spec document must exist and be approved.

- Spec documents live in `docs/spec/`
- The master spec `mapping-core-spec.md` covers Epics 1 and 2
- For each subsequent epic (BLE layer, Tauri commands, Svelte UI, CLI tool, Android port),
  a dedicated spec document must be written and confirmed by the user before code is written
- When you reach the first task of a new epic that lacks a spec, stop and write a draft spec
  document in `docs/spec/`. Present it to the user and wait for explicit approval (a message
  saying "approved", "looks good", "proceed", or similar) before writing any implementation code
- Do not interpret silence or a non-committal response as approval

### 3. One task at a time

Complete one numbered task fully before starting the next. "Complete" means:

- Code written and compiling without errors or warnings
- Tests written and passing (where applicable — see testing rules)
- Implementation plan checkbox updated from `- [ ]` to `- [x]`
- Progress log entry written (see rule 6)

Do not batch multiple tasks into one commit unless the tasks are explicitly stated as a group
(e.g. "3.13–3.15 are a single atomic unit"). When in doubt, one task = one commit.

### 4. Ask before making architectural decisions

If a task requires a decision that is not covered by the spec or that could have significant
downstream consequences (naming conventions, crate structure changes, new dependencies, API
surface changes), stop and present the options to the user. Do not make the decision unilaterally.

Examples of decisions that require user input:

- Adding a new third-party crate dependency
- Changing a public API signature in `mapping-core`
- Choosing between two valid implementation approaches with different tradeoffs
- Deviating from a pattern established in the spec

Examples of decisions you can make without asking:

- Internal variable names within a function
- Order of match arms
- Whether to use `?` or explicit `match` for error propagation within a single function
- Test helper organisation

### 5. Never modify the spec unilaterally

If you discover during implementation that the spec is incomplete, ambiguous, or incorrect:

- Stop
- Write a clear description of the issue and your proposed resolution
- Present it to the user and wait for approval before continuing
- Once approved, update the spec document to reflect the resolution, then proceed

### 6. Write a progress log entry after every session

After completing any meaningful work, append an entry to `docs/log/progress.md`. If the file
does not exist, create it.

Log entry format:

```markdown
## YYYY-MM-DD — <one-line summary of what was done>

**Tasks completed:** <list of task IDs e.g. 1.1, 1.2, 1.3>
**Tasks in progress:** <list of task IDs if any were started but not finished>
**Files changed:**

- `path/to/file.rs` — description of change
- `path/to/other.rs` — description of change

**Notes:**
Any decisions made, blockers encountered, deviations from spec (with reason), or
things the next session should be aware of.

**Next:** <The next task ID and a one-line description of what it involves>
```

Be specific in the notes. A future Claude Code session reading this log should be able to
understand exactly where work left off and why without re-reading all the code.

---

## Coding standards

### Rust

- Edition: 2024 (library crates); `src-tauri` uses 2021 as generated by `create-tauri-app`
- MSRV: document via `rust-version` in `Cargo.toml` `[package]` section; do not use features from a newer version
- All public items must have doc comments (`///`)
- All `pub` functions in `mapping-core` must have at least one unit test in the same file
- Use `thiserror` for error types; do not use `anyhow` in library crates (`mapping-core`, `tap-ble`)
- `anyhow` is acceptable in binary crates (`mapxr` / `src-tauri`, `tap-cli`) and test code
- Never use `unwrap()` or `expect()` in library code outside of tests; use `?` or explicit error handling
- `unwrap()` in tests is acceptable but prefer `expect("description")` for better test failure messages
- Run `cargo clippy -- -D warnings` before considering a task complete; fix all warnings
- Run `cargo fmt` before committing
- Keep `unsafe` code to zero unless absolutely unavoidable; document any exceptions in a `// SAFETY:` comment

### Rust module structure

- Each module should have a `#[cfg(test)]` block at the bottom of the file containing its unit tests
- Integration tests go in `tests/` at the crate root
- Test fixtures (sample JSON profile files etc.) go in `tests/fixtures/`
- Use `rstest` for parameterised tests when testing the same function against many inputs

### TypeScript / Svelte

- Strict TypeScript: `"strict": true` in `tsconfig.json`; no `any` types without a `// TODO:` comment explaining why
- All Tauri command wrappers in `src/lib/commands.ts` must have JSDoc comments
- Svelte components: props typed with explicit interfaces, not inlined object types
- Use Svelte stores for all shared state; do not pass deeply nested props
- No `console.log` in committed code; use a structured logger wrapper

### General

- No commented-out code in commits
- No TODO comments without a corresponding task ID in the implementation plan (e.g. `// TODO: task 5.23`)
- Keep functions short: if a function exceeds ~40 lines, consider splitting it
- Prefer explicit over clever: readable code over terse code

---

## Testing rules

### When to write tests

Write tests for every Rust module where it makes sense to do so. Specifically:

| Code                                              | Test requirement                                                              |
| ------------------------------------------------- | ----------------------------------------------------------------------------- |
| `mapping-core` data types (serialise/deserialise) | Required — unit tests in the same file                                        |
| `mapping-core` engine logic                       | Required — unit tests for every public method                                 |
| `mapping-core` validation logic                   | Required — at least one passing and one failing case per rule                 |
| `tap-ble` packet parser                           | Required — unit tests for all valid and edge-case inputs                      |
| `tap-ble` BLE connection logic                    | Integration tests only (require hardware); mock where possible                |
| Tauri commands                                    | Integration tests using Tauri's test harness where feasible                   |
| Svelte components                                 | Not required for initial implementation; add if a component has complex logic |
| CLI tool                                          | Integration tests using fixture files                                         |

### Test naming convention

```rust
#[test]
fn <unit_under_test>_<scenario>_<expected_outcome>() { ... }

// Examples:
fn tap_code_from_str_valid_single_hand_parses_correctly() { ... }
fn tap_code_from_str_all_open_returns_error() { ... }
fn combo_engine_cross_device_within_window_fires_combo_action() { ... }
fn combo_engine_cross_device_outside_window_resolves_as_two_singles() { ... }
```

### Test data

- Do not hardcode magic numbers in tests; use named constants or builder functions
- For profile JSON fixtures, store them in `tests/fixtures/` as real `.json` files and load them
  with `include_str!()` — this tests the full deserialisation path
- For timing tests, use `tokio::time::pause()` and `tokio::time::advance()` to control time
  deterministically rather than `sleep()`

---

## Dependency policy

Before adding any new crate or npm package, check:

1. Is there an existing dependency in the workspace that covers this need?
2. Is the crate well-maintained (recent commits, responsive to issues)?
3. Does it compile for all target platforms (Windows, macOS, Linux, and eventually Android)?

Approved dependencies (already decided; add these freely):

**Rust:**

- `serde`, `serde_json` — serialisation
- `thiserror` — error types in library crates
- `anyhow` — error handling in binary crates
- `btleplug` — BLE (desktop platforms)
- `tokio` — async runtime
- `tauri` 2.x — desktop shell
- `enigo` — keyboard/mouse simulation
- `rstest` — parameterised tests

**TypeScript/Svelte:**

- Svelte 5 with runes
- TypeScript strict mode
- `@tauri-apps/api` — Tauri bindings
- `@tauri-apps/plugin-*` — Tauri official plugins only

For any dependency not on this list, present the candidate to the user with a brief justification
and wait for approval before adding it to `Cargo.toml` or `package.json`.

---

## File layout reference

Project name: `mapxr` — Tauri app identifier: `com.mapxr.app` — package manager: `npm`

```
mapxr/
├── CLAUDE.md                        ← this file
├── Cargo.toml                       ← workspace root
├── Cargo.lock
├── package.json
├── justfile                         ← dev convenience targets
│
├── src-tauri/                       ← Tauri binary crate (standard Tauri location)
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs
│   │   └── state.rs
│   ├── icons/
│   ├── tauri.conf.json
│   ├── build.rs
│   └── Cargo.toml
│
├── crates/
│   ├── mapping-core/                ← pure library, no BLE, no UI
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types/               ← TapCode, Trigger, Action, Profile etc.
│   │   │   ├── engine/              ← ComboEngine, LayerStack, SequenceEngine
│   │   │   └── error.rs
│   │   ├── tests/
│   │   │   └── fixtures/            ← sample .json profile files
│   │   └── Cargo.toml
│   │
│   ├── tap-ble/                     ← BLE abstraction, desktop only
│   │   ├── src/
│   │   └── Cargo.toml
│   │
│   └── tap-cli/                     ← CLI tool binary
│       ├── src/
│       └── Cargo.toml
│
├── src/                             ← Svelte frontend
│   ├── lib/
│   │   ├── commands.ts              ← typed Tauri invoke wrappers
│   │   ├── events.ts                ← typed Tauri listen wrappers
│   │   ├── types.ts                 ← TypeScript types mirroring Rust structs
│   │   └── stores/
│   ├── routes/
│   └── app.html
│
├── profiles/                        ← example/default profile files
│
└── docs/
    ├── spec/
    │   ├── mapping-core-spec.md     ← approved: covers Epics 1 and 2
    │   └── <epic-name>-spec.md      ← write one before each new epic
    ├── plan/
    │   └── implementation-plan.md  ← the task checklist
    └── log/
        └── progress.md             ← session logs
```

---

## How to start a new session

1. Read this file (`CLAUDE.md`) in full
2. Read `docs/log/progress.md` — specifically the last entry — to understand current state
3. Open `docs/plan/implementation-plan.md` and find the next incomplete task
4. Check whether a spec exists for that task's epic in `docs/spec/`
5. If no spec exists for the epic, write a draft and wait for user approval before coding
6. If a spec exists and is approved, proceed with the task
7. When done, update the implementation plan checkbox and write a log entry

Do not skip step 1 even if you believe you remember the instructions. Re-read this file at the
start of every session.

---

## How to handle uncertainty

If you are uncertain about any of the following, stop and ask rather than guessing:

- Whether a spec has been approved for a given epic
- Whether a proposed implementation matches the spec intent
- Whether a dependency is acceptable
- Whether a task is truly complete (e.g. tests are missing but you're unsure if they're required)
- Whether a deviation from the spec is justified

A short clarifying question takes less time than implementing something wrong and having to undo it.

---

## What not to do

- Do not refactor code outside the scope of the current task
- Do not upgrade dependencies unless a task explicitly requires it
- Do not add features not described in the spec or implementation plan, even if they seem useful
- Do not mark a task complete if tests are failing
- Do not mark a task complete if `cargo clippy` or `cargo fmt --check` produces output
- Do not skip writing the progress log entry
- Do not begin an epic without an approved spec document
- Do not assume the user's silence on a spec draft means approval — wait for explicit confirmation
