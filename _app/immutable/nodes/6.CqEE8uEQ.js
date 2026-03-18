import{$ as e,C as t,D as n,I as r,J as i,L as a,N as o,Q as s,R as c,S as l,U as u,Y as d,et as f,h as p,j as m,m as h,u as g,x as _,y as v,z as y}from"../chunks/CGwnShg4.js";import{s as b}from"../chunks/CTb4NadF.js";import"../chunks/CFKVnMbq.js";import"../chunks/D40A1Rfp.js";import{t as x}from"../chunks/Cx85uYP6.js";import{i as S,n as C,r as w}from"../chunks/ULfJJV0e.js";var T=f({entries:()=>E,load:()=>D});function E(){return S.map(e=>({slug:e.slug}))}function D({params:e}){let t=S.find(t=>t.slug===e.slug);return t||x(404,`Page not found`),{slug:e.slug,title:t.title}}var O=f({default:()=>A}),k=t(`<h1>Actions</h1> <p>An <strong>action</strong> is what happens when a trigger fires. mapxr supports keyboard output,
  mouse control, layer switching, and sticky modifier keys.</p> <h2>Key action</h2> <p>Send a key combination to the active window:</p> <pre><code></code></pre> <p>Key strings follow the format <code>modifier+key</code>. Multiple modifiers can be chained: <code>ctrl+alt+delete</code>. Key names match the <a href="https://docs.rs/enigo/latest/enigo/enum.Key.html" target="_blank" rel="noreferrer">enigo key list</a>.</p> <h2>Type action</h2> <p>Type a string of text character by character:</p> <pre><code></code></pre> <h2>Mouse action</h2> <pre><code></code></pre> <h2>Layer action</h2> <p>Push a new layer onto the layer stack, or pop back to the previous one:</p> <pre><code></code></pre> <p><code>push_layer</code> adds a layer on top — pop it later with <code>pop_layer</code>.<br/> <code>activate_layer</code> replaces the entire stack with a single layer.</p> <h2>Hold modifier action</h2> <p>Simulate a sticky modifier key that stays held until released by a second tap:</p> <pre><code></code></pre> <div class="overflow-x-auto"><table class="table"><thead><tr><th>Mode</th><th>Behaviour</th></tr></thead><tbody><tr><td><code>toggle</code></td><td>First tap holds the key; second tap releases it</td></tr><tr><td><code>count</code></td><td>Holds for N taps then auto-releases</td></tr><tr><td><code>timeout</code></td><td>Auto-releases after N milliseconds</td></tr></tbody></table></div> <h2>No-op action</h2> <pre><code></code></pre> <p>Explicitly consume a trigger without doing anything. Useful for disabling inherited mappings in a child layer.</p>`,1);function A(t){var n=k(),r=y(c(n),8),i=a(r);i.textContent=`{ "type": "key", "key": "ctrl+shift+p" }`,e(r);var o=y(r,8),l=a(o);l.textContent=`{ "type": "type", "text": "Hello, world!" }`,e(o);var u=y(o,4),d=a(u);d.textContent=`{ "type": "mouse_click", "button": "left" }
{ "type": "mouse_click", "button": "right" }
{ "type": "mouse_scroll", "direction": "up", "amount": 3 }`,e(u);var f=y(u,6),p=a(f);p.textContent=`{ "type": "push_layer",    "layer": "nav" }
{ "type": "pop_layer" }
{ "type": "activate_layer", "layer": "symbols" }`,e(f);var m=y(f,8),h=a(m);h.textContent=`{
  "type": "hold_modifier",
  "key": "shift",
  "mode": { "type": "toggle" }
}`,e(m);var g=y(m,6),v=a(g);v.textContent=`{ "type": "noop" }`,e(g),s(2),_(t,n)}var j=f({default:()=>N}),M=t(`<h1>Layers</h1> <p><strong>Layers</strong> let you define multiple sets of mappings within a single profile and
  switch between them at runtime. Think of layers like the Fn key on a laptop keyboard —
  the same physical key produces different output depending on the active layer.</p> <h2>Defining layers</h2> <p>Each layer is a named object inside the <code>layers</code> map in your profile:</p> <pre><code></code></pre> <h2>The layer stack</h2> <p>mapxr maintains a <strong>layer stack</strong>. At startup the stack contains only the <code>default_layer</code>. You can push additional layers on top or pop back to previous layers.</p> <p>When resolving a trigger, mapxr searches from the top of the stack downward.
  The first matching mapping wins.</p> <pre><code></code></pre> <h2>Switching layers</h2> <p>Use layer actions in your mappings to navigate the stack:</p> <pre><code></code></pre> <h2>Example: nav layer</h2> <pre><code></code></pre> <div class="alert alert-info mt-6"><span>Layer actions are documented in full on the <a class="link">Actions</a> page.</span></div>`,1);function N(t){var n=M(),r=y(c(n),8),i=a(r);i.textContent=`{
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [ ... ]
    },
    "nav": {
      "mappings": [ ... ]
    },
    "symbols": {
      "mappings": [ ... ]
    }
  }
}`,e(r);var l=y(r,8),u=a(l);u.textContent=`Stack (top to bottom):
  nav      ← searched first
  base     ← searched second (fallback)`,e(l);var d=y(l,6),f=a(d);f.textContent=`// Enter nav layer (stacks on top of base)
{ "type": "push_layer", "layer": "nav" }

// Return to base
{ "type": "pop_layer" }

// Jump directly to symbols, clearing the stack
{ "type": "activate_layer", "layer": "symbols" }`,e(d);var p=y(d,4),m=a(p);m.textContent=`{
  "name": "Productivity",
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [1,1,0,0,0] },
          "action":  { "type": "push_layer", "layer": "nav" }
        }
      ]
    },
    "nav": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [0,0,0,0,1] },
          "action":  { "type": "pop_layer" }
        },
        {
          "trigger": { "type": "single", "device": "right", "fingers": [1,0,0,0,0] },
          "action":  { "type": "key", "key": "up" }
        }
      ]
    }
  }
}`,e(p);var h=y(p,2),v=a(h),x=y(a(v));s(),e(v),e(h),o(()=>g(x,`href`,`${b??``}/docs/actions`)),_(t,n)}var P=f({default:()=>I}),F=t(`<h1>Profiles</h1> <p>A <strong>profile</strong> is a JSON file that tells mapxr how to translate TAP Strap finger
  combinations into actions. You can have multiple profiles — one per application, workflow,
  or context — and switch between them at any time.</p> <h2>Profile structure</h2> <pre><code></code></pre> <h2>Top-level fields</h2> <div class="overflow-x-auto"><table class="table table-zebra"><thead><tr><th>Field</th><th>Type</th><th>Required</th><th>Description</th></tr></thead><tbody><tr><td><code>name</code></td><td>string</td><td>Yes</td><td>Human-readable profile name</td></tr><tr><td><code>description</code></td><td>string</td><td>No</td><td>Optional description</td></tr><tr><td><code>default_layer</code></td><td>string</td><td>Yes</td><td>Name of the layer that is active on startup</td></tr><tr><td><code>layers</code></td><td>object</td><td>Yes</td><td>Map of layer name → layer definition</td></tr></tbody></table></div> <h2>App-specific profiles</h2> <p>To activate a profile automatically when a specific application is in focus, add an <code>app_match</code> field:</p> <pre><code></code></pre> <p>mapxr checks the active window every 500 ms and switches profiles automatically.
  If no app-specific profile matches, the global default profile is used.</p> <div class="alert alert-info mt-6"><span>See <a class="link">Triggers</a> and <a class="link">Actions</a> for the full list of mapping options.</span></div>`,1);function I(t){var n=F(),r=y(c(n),6),i=a(r);i.textContent=`{
  "name": "My Profile",
  "description": "Optional description",
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [1, 0, 0, 0, 0] },
          "action":  { "type": "key", "key": "ctrl+c" }
        }
      ]
    }
  }
}`,e(r);var l=y(r,10),u=a(l);u.textContent=`{
  "name": "VS Code",
  "app_match": { "process_name": "code" },
  ...
}`,e(l);var d=y(l,4),f=a(d),p=y(a(f)),m=y(p,2);s(),e(f),e(d),o(()=>{g(p,`href`,`${b??``}/docs/triggers`),g(m,`href`,`${b??``}/docs/actions`)}),_(t,n)}var L=f({default:()=>z}),R=t(`<h1>Triggers</h1> <p>A <strong>trigger</strong> describes the tap gesture that activates a mapping.
  Each trigger specifies which device (left/right hand or both), which fingers are down,
  and how many times the gesture must be tapped.</p> <h2>Finger notation</h2> <p>Fingers are represented as a 5-element array of <code>0</code> (up) and <code>1</code> (down),
  ordered thumb → pinky:</p> <pre><code></code></pre> <h2>Single tap</h2> <pre><code></code></pre> <h2>Double tap</h2> <pre><code></code></pre> <p>The same gesture tapped twice within the combo window (default 300 ms) fires this trigger.</p> <h2>Cross-device combo</h2> <pre><code></code></pre> <p>Two gestures on different hands within the combo window fire as a single combo event.</p> <h2>Device values</h2> <div class="overflow-x-auto"><table class="table"><thead><tr><th>Value</th><th>Meaning</th></tr></thead><tbody><tr><td><code>"left"</code></td><td>Left-hand TAP Strap only</td></tr><tr><td><code>"right"</code></td><td>Right-hand TAP Strap only</td></tr><tr><td><code>"any"</code></td><td>Either device (first one to fire)</td></tr></tbody></table></div>`,1);function z(t){var n=R(),r=y(c(n),8),i=a(r);i.textContent=`[thumb, index, middle, ring, pinky]
[1, 0, 0, 0, 0]  // thumb only
[0, 1, 1, 0, 0]  // index + middle
[1, 1, 1, 1, 1]  // all five`,e(r);var o=y(r,4),l=a(o);l.textContent=`{
  "type": "single",
  "device": "left",
  "fingers": [1, 0, 0, 0, 0]
}`,e(o);var u=y(o,4),d=a(u);d.textContent=`{
  "type": "double",
  "device": "left",
  "fingers": [1, 0, 0, 0, 0]
}`,e(u);var f=y(u,6),p=a(f);p.textContent=`{
  "type": "combo",
  "first":  { "device": "left",  "fingers": [1, 0, 0, 0, 0] },
  "second": { "device": "right", "fingers": [0, 1, 0, 0, 0] }
}`,e(f),s(6),_(t,n)}var B=t(`<meta name="description"/> <meta property="og:title"/>`,1);function V(e,t){d(t,!0);let a=Object.assign({"/src/lib/docs-pages/actions.svelte":O,"/src/lib/docs-pages/getting-started.svelte":C,"/src/lib/docs-pages/layers.svelte":j,"/src/lib/docs-pages/profiles.svelte":P,"/src/lib/docs-pages/triggers.svelte":L}),s=u(()=>a[`/src/lib/docs-pages/${t.data.slug}.svelte`]?.default??null);h(`11o795e`,e=>{var n=B(),i=c(n),a=y(i,2);o(()=>{g(i,`content`,`MapXr documentation: ${t.data.title??``}`),g(a,`content`,`${t.data.title??``} — MapXr Docs`)}),m(()=>{r.title=`${t.data.title??``} — MapXr Docs`}),_(e,n)}),w(e,{get currentSlug(){return t.data.slug},children:(e,t)=>{var r=l(),i=c(r),a=e=>{var t=l();p(c(t),()=>n(s),(e,t)=>{t(e,{})}),_(e,t)};v(i,e=>{n(s)&&e(a)}),_(e,r)},$$slots:{default:!0}}),i()}export{V as component,T as universal};