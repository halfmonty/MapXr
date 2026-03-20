<h1>First public release: three bugs, two platforms, one coffee button</h1>

<p>
  v0.1.2 is the first release where the full pipeline works end-to-end: push a tag, GitHub Actions
  builds Linux and Windows installers in parallel, signs them, publishes a GitHub Release, uploads
  <code>latest.json</code>, and in-app update checking picks it up automatically. Getting there
  required fixing three separate bugs — each one silent enough that it had slipped through
  earlier testing.
</p>

<h2>Bug 1 — The auto-updater manifest was never generated</h2>

<p>
  The first release had all the right pieces: a signing keypair, the <code>TAURI_SIGNING_PRIVATE_KEY</code>
  GitHub secret, a <code>tauri-action</code> step configured to publish a release. But
  <code>latest.json</code> never appeared in the release assets.
</p>

<p>
  The cause was a single missing line in <code>tauri.conf.json</code>. Tauri v2 requires
  <code>"createUpdaterArtifacts": true</code> in the <code>bundle</code> section before it will
  generate <code>.sig</code> signature files during the build. Without those signature files,
  <code>tauri-action</code> has nothing to sign and produces no manifest. The config option isn't
  mentioned prominently anywhere in the getting-started path — it lives in a note partway down the
  plugin documentation.
</p>

<p>
  Once the config was added, the manual third job I had written to stitch together
  <code>latest.json</code> from GitHub Actions artifacts became unnecessary.
  <code>tauri-action</code> handles it automatically. That job was deleted.
</p>

<h2>Bug 2 — Windows CI broke after a crate update</h2>

<p>
  The Windows build was failing with a type mismatch in the focus monitor. The <code>windows</code>
  crate 0.58 changed the inner type of <code>HWND</code> from <code>isize</code> to
  <code>*mut c_void</code>, which broke a null-check written against the old type. The same update
  changed <code>GetWindowTextW</code> to take <code>&amp;mut [u16]</code> directly instead of a
  raw pointer plus length. Both call sites needed updating.
</p>

<h2>Bug 3 — The RPM crashed before the first window opened</h2>

<p>
  Installing and running the RPM on Fedora/Nobara produced an immediate panic:
  <em>tray icon error: Zero width not allowed</em>. The app never reached the main window.
</p>

<p>
  The tray icon setup tried two file paths at runtime: the Tauri resource directory and, as a
  fallback, the source tree path baked in at compile time via <code>env!("CARGO_MANIFEST_DIR")</code>.
  Both failed silently under the RPM install layout, and the final fallback was
  <code>Image::new(&amp;[], 0, 0)</code> — a valid call that creates a zero-width image.
  <code>TrayIconBuilder</code> then rejected it with a hard panic.
</p>

<p>
  The fix was to stop doing runtime path lookups for this at all. The icon is now embedded directly
  in the binary via <code>include_bytes!</code> at compile time. It's always available regardless of
  where the binary is installed, and the fallback chain is gone.
</p>

<p>
  The AppImage on NVIDIA + Wayland systems hits a separate crash in the NVIDIA EGL Wayland driver
  during WebKit initialization — a known incompatibility between the AppImage's bundled WebKitGTK
  and certain NVIDIA driver versions. The workaround is to run with
  <code>WEBKIT_DISABLE_DMABUF_RENDERER=1</code>, or to use the RPM/DEB package instead, which uses
  the system's WebKitGTK and doesn't have this problem.
</p>

<h2>Release automation</h2>

<p>
  Getting the first release out surfaced how many manual steps the process had. A
  <code>scripts/bump-version.sh</code> script now handles the version increment across all three
  files that need it (<code>tauri.conf.json</code>, <code>Cargo.toml</code>,
  <code>package.json</code>) in one command. A <code>docs/release-checklist.md</code> documents the
  full sequence from "tests pass" to "verify the release artifacts" so nothing gets missed.
</p>
