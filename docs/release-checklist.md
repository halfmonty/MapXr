# Release Checklist

Run through this list in order before pushing a release tag.

---

## 1. Verify the build is clean

```bash
cargo clippy -- -D warnings     # zero warnings
cargo test --workspace          # all tests pass
cargo fmt --check               # no formatting drift
```

---

## 2. Bump the version

```bash
./scripts/bump-version.sh patch   # or minor / major / x.y.z
```

Confirm all three files updated to the same version:
- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/package.json`

---

## 3. Update CHANGELOG.md

Move relevant items from `[Unreleased]` into a new dated section:

```markdown
## [x.y.z] — YYYY-MM-DD

### Added
- …

### Fixed
- …
```

Leave `[Unreleased]` empty (but present) for the next cycle.

---

## 4. Write a devlog entry (if meaningful user-facing work was done)

Devlog entries are public-facing — skip for pure infrastructure/patch releases if there's nothing interesting to say.

**Step 4a — add the entry to `apps/site/src/lib/data/devlog.ts`:**

```ts
{
  slug: 'YYYY-MM-DD-short-slug',
  date: 'YYYY-MM-DD',
  title: 'One-line summary of what shipped',
  body: 'Two or three sentence plain-English description. What changed, why it matters to users.',
  epicsDone: N,      // cumulative epics completed so far
  totalEpics: N,     // total epics in the implementation plan
  tags: ['feature', 'bug-fix', 'ux', 'ble', 'ui'],
},
```

Insert at the top of the `DEVLOG` array (most recent first).

**Step 4b — create the body file `apps/site/src/lib/devlog-posts/YYYY-MM-DD-short-slug.svelte`:**

```svelte
<p>
  Opening paragraph — what shipped and why.
</p>

<p>
  Technical detail or interesting decision, written for a curious non-expert reader.
</p>
```

The slug in `devlog.ts` must exactly match the filename (without `.svelte`).

---

## 5. Commit everything

```bash
git add apps/desktop/src-tauri/tauri.conf.json \
        apps/desktop/src-tauri/Cargo.toml \
        apps/desktop/package.json \
        Cargo.lock \
        CHANGELOG.md \
        apps/site/src/lib/data/devlog.ts \
        apps/site/src/lib/devlog-posts/YYYY-MM-DD-short-slug.svelte   # if written

git commit -m "chore: release vx.y.z"
```

---

## 6. Tag and push

```bash
git tag vx.y.z
git push && git push --tags
```

Pushing the tag triggers the release workflow. Monitor it at:
`https://github.com/halfmonty/mapxr/actions`

---

## 7. Verify the release

Once the workflow completes (~10–15 min):

- [ ] GitHub release exists with the correct tag name
- [ ] Linux AppImage, DEB, and RPM are attached
- [ ] Windows MSI and NSIS installer are attached
- [ ] `latest.json` is attached (required for auto-updater)
- [ ] Download and run the RPM (or MSI on Windows) and confirm it launches
- [ ] Open Settings → Updates and confirm the in-app updater sees no newer version

---

## Tag naming conventions

| Type | Example | When |
|------|---------|------|
| Stable | `v1.2.3` | Normal release |
| Beta | `v1.2.3-beta.1` | Feature complete, needs testing |
| RC | `v1.2.3-rc.1` | Final check before stable |

Pre-release tags (`-beta`, `-rc`, `-alpha`) are automatically marked as pre-release on GitHub.
