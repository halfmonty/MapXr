# profiles/

This directory contains example and default mapping profiles shipped with mapxr.

## File format

Each profile is a `.json` file conforming to the mapxr profile schema (see
`docs/spec/mapping-core-spec.md`). Profiles are loaded by `mapping-core` via
`Profile::load(path)`.

## Naming convention

```
<descriptive-name>.json       # e.g. default-single.json, coding-dual.json
```

Use lowercase kebab-case. The filename (without `.json`) is not used as the
layer ID — the `layer_id` field inside the JSON is the canonical identifier.

## Runtime profile storage

Profiles the user creates or imports at runtime are stored in a per-OS config
directory, **not** here:

| OS      | Path                                              |
| ------- | ------------------------------------------------- |
| Linux   | `~/.config/mapxr/profiles/`                      |
| macOS   | `~/Library/Application Support/mapxr/profiles/`  |
| Windows | `%APPDATA%\mapxr\profiles\`                      |

Files in this directory are bundled with the application and serve as
read-only starter profiles. They are copied to the runtime directory on first
launch if no profiles are present.
