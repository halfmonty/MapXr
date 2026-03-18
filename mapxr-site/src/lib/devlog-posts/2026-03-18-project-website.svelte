<h1>Project website launched</h1>

<p>
  With the core application logic well underway, it felt like the right time to give MapXr a proper
  home on the web. This entry covers the decisions behind the site's technical stack and structure.
</p>

<h2>Stack choices</h2>

<p>
  The site lives inside the existing monorepo at <code>mapxr-site/</code>. Since the app itself
  already uses Svelte 5 and Vite, reusing those tools was a natural fit — no context switching,
  and the same Tailwind + DaisyUI pair used in the main UI carries over cleanly.
</p>

<ul>
  <li><strong>Svelte 5</strong> with runes (<code>$state</code>, <code>$derived</code>, <code>$effect</code>)</li>
  <li><strong>Tailwind CSS v4</strong> via its native Vite plugin — no <code>postcss.config</code> needed</li>
  <li><strong>DaisyUI v5</strong> for component classes (<code>navbar</code>, <code>hero</code>, <code>card</code>, <code>steps</code>, etc.)</li>
  <li><strong>Hash-based routing</strong> — no router library, just <code>window.location.hash</code> and a <code>$state</code> rune</li>
</ul>

<h2>Why hash routing?</h2>

<p>
  GitHub Pages serves static files from a single directory. Any URL that doesn't correspond to an
  actual file returns a 404. Hash routing sidesteps this entirely — the browser never sends the
  fragment to the server, so <code>#/docs/triggers</code> and <code>#/devlog</code> both load
  <code>index.html</code> without any redirect tricks.
</p>

<h2>Lazy-loaded doc pages</h2>

<p>
  Each documentation topic is a plain <code>.svelte</code> file under <code>src/docs-pages/</code>.
  Vite's <code>import.meta.glob</code> discovers them at build time and splits each into its own
  JS chunk. Navigating to a doc page fetches only that page's chunk — the rest stay unloaded until
  needed. The same pattern is used for devlog articles.
</p>

<h2>Theme</h2>

<p>
  DaisyUI's <code>corporate</code> theme is the light default; <code>business</code> is the dark
  alternative. The toggle reads <code>prefers-color-scheme</code> on first visit so the site
  matches the user's OS preference without any stored setting.
</p>

<h2>Deployment</h2>

<p>
  A GitHub Actions workflow at <code>.github/workflows/deploy-site.yml</code> triggers on any push
  to <code>main</code> that touches <code>mapxr-site/**</code>. It runs <code>npm ci && npm run build</code>
  then pushes <code>mapxr-site/dist</code> to the <code>gh-pages</code> branch via
  <code>peaceiris/actions-gh-pages</code>.
</p>
