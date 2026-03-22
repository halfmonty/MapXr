# Project structure

Project name: `mapxr` — Tauri app identifier: `com.mapxr.app` — package manager: `npm`

```
mapxr/
├── CLAUDE.md                        ← project instructions for AI agents
├── Cargo.toml                       ← workspace root
├── Cargo.lock
├── .github/
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
│   └── tap-cli/                     ← CLI tool binary (future)
│       ├── src/
│       └── Cargo.toml
│
├── packages/
│   └── design-tokens/               ← shared CSS design tokens; both apps import base.css
│       └── base.css                 ← DaisyUI plugin config (wireframe/business themes)
│
├── apps/
│   ├── desktop/                     ← Tauri app (desktop + Android build target)
│   │   ├── src/                     ← Svelte frontend
│   │   │   ├── lib/
│   │   │   │   ├── commands.ts      ← typed Tauri invoke wrappers
│   │   │   │   ├── events.ts        ← typed Tauri listen wrappers
│   │   │   │   ├── types.ts         ← TypeScript types mirroring Rust structs
│   │   │   │   └── stores/
│   │   │   ├── routes/
│   │   │   │   ├── settings/           ← Settings page (tray / startup prefs)
│   │   │   │   ├── context-rules/      ← Auto-switch rules page
│   │   │   │   ├── devices/
│   │   │   │   ├── profiles/
│   │   │   │   └── debug/
│   │   │   └── app.html
│   │   ├── src-tauri/               ← Rust Tauri backend
│   │   │   ├── src/
│   │   │   │   ├── main.rs
│   │   │   │   ├── lib.rs              ← tray setup, close handler
│   │   │   │   ├── commands.rs
│   │   │   │   ├── state.rs
│   │   │   │   ├── login_item.rs       ← start-at-login per platform
│   │   │   │   ├── context_rules.rs
│   │   │   │   ├── focus_monitor/      ← per-platform window focus monitor
│   │   │   │   └── platform.rs
│   │   │   ├── icons/
│   │   │   ├── profiles/            ← starter .json files (embedded via include_str!, seeded on first launch)
│   │   │   ├── tauri.conf.json
│   │   │   ├── build.rs
│   │   │   └── Cargo.toml
│   │   ├── static/
│   │   ├── package.json
│   │   ├── svelte.config.js
│   │   ├── vite.config.js
│   │   └── tsconfig.json
│   │
│   └── site/                        ← documentation website
│       ├── src/
│       └── package.json
│
└── docs/
    ├── vision.md                    ← project vision, target user, design principles, scope
    ├── decisions.md                 ← architectural decision record
    ├── spec/                        ← one spec per epic; load only the relevant one per session
    │   ├── mapping-core-spec.md     ← approved: covers Epics 1 and 2
    │   └── <epic-name>-spec.md      ← write one before each new epic
    ├── plan/
    │   └── implementation-plan.md  ← the task checklist
    ├── log/
    │   ├── progress.md             ← session logs (recent entries only; older entries archived)
    │   └── archive/                ← archived log entries by month
    ├── testing/                    ← manual test plans
    │   └── android-manual-tests.md ← Phase 1 Android test matrix
    └── reference/                  ← hardware reference docs, project structure
        └── project-structure.md    ← this file
```
