# MapXr design system

This document describes the visual conventions shared between `apps/desktop` and `apps/site`.
Both apps import the same base configuration from `packages/design-tokens/base.css` and follow
the rules below so that colours, typography, and components look consistent across surfaces.

---

## Theme configuration

| Aspect | Value |
|---|---|
| CSS framework | Tailwind v4 (`@import "tailwindcss"`) |
| Component library | DaisyUI v5 (`@plugin "daisyui"`) |
| Light theme | `corporate` |
| Dark theme | `business` |
| Default | System preference (`--prefersdark` flag + flash-prevention inline script in `app.html`) |
| Theme persistence | `localStorage` key `"theme"` |
| Theme switching | TitleBar toggle (desktop) · Nav toggle (site) |

### Shared base file

`packages/design-tokens/base.css` is imported by both apps after their own `@plugin "daisyui"`
declaration. Currently it is a placeholder for future shared `@theme` variable overrides.
The `@plugin "daisyui"` directive must remain in each app's own `app.css` because Tailwind v4
resolves plugin modules relative to the declaring file's directory.

---

## Colour usage rules

**Rule: never use raw Tailwind palette classes for colour.** All colour references must go
through DaisyUI semantic tokens so they adapt automatically when the theme changes.

### Semantic tokens in use

| Token | Meaning | Common usage |
|---|---|---|
| `bg-base-100` | Card / panel surface | Sidebar, cards, modals, table rows |
| `bg-base-200` | Page background / inset | Main `<body>` wrapper, section backgrounds |
| `bg-base-300` | Subtle dividers | Drag handles, disabled states |
| `border-base-300` | Standard border | Cards, inputs, dividers, table borders |
| `text-base-content` | Primary body text | Headings, labels, values |
| `text-base-content/70` | Secondary text | Descriptions, subtitles |
| `text-base-content/50` | Tertiary / muted | Timestamps, helper text |
| `text-base-content/40` | Very muted | Section labels, empty states |
| `text-base-content/30` | Near-invisible | Legal text, ultra-muted captions |
| `text-primary` | Brand accent | Logo mark, active nav items, links |
| `bg-primary` / `text-primary-content` | Primary interactive | Active nav background, CTA buttons |
| `text-error` / `bg-error` | Destructive | Delete actions, close-button hover, validation errors |
| `text-success` / `badge-success` | Positive state | Connected device badges, success indicators |
| `text-warning` / `badge-warning` | Caution state | Partial-connection states |
| `badge-info` | Informational | Role labels |

### Prohibited colour classes

Do **not** use raw Tailwind palette classes such as:

```
text-gray-*   text-zinc-*   text-slate-*   text-neutral-*
bg-gray-*     bg-zinc-*     bg-slate-*
border-gray-* border-zinc-*
```

These do not respond to theme changes and will look wrong in dark mode.

---

## Typography

Both apps inherit DaisyUI's type scale without override. Conventions in use:

| Usage | Classes |
|---|---|
| Page heading | `text-2xl font-bold` or `text-3xl font-bold` |
| Section heading | `text-xl font-semibold` or `text-lg font-semibold` |
| Body / paragraph | default (inherits DaisyUI base size) |
| Small label / meta | `text-sm` |
| Tiny label (e.g. sidebar section headers) | `text-[10px] font-semibold uppercase tracking-wider` |
| Monospace (tap codes, IDs) | `font-mono text-xs` |
| Logo wordmark | `text-primary` on "Map", unstyled on "Xr" |

---

## Spacing

Both apps use Tailwind's default spacing scale. Common patterns:

| Pattern | Classes |
|---|---|
| Page content padding | `p-6` |
| Card / panel padding | `p-4` |
| Section gap (flex/grid) | `gap-4` or `gap-6` |
| Tight list gap | `gap-2` or `space-y-2` |
| Sidebar internal padding | `px-3 py-2` |
| Inline badge / pill gap | `gap-1` or `gap-2` |

---

## Component conventions

### Buttons

Use DaisyUI's `btn` classes. Never style `<button>` elements with raw Tailwind only.

```html
<button class="btn btn-primary">Primary action</button>
<button class="btn btn-ghost btn-sm">Secondary / icon action</button>
<button class="btn btn-error btn-sm">Destructive action</button>
```

### Cards / panels

```html
<div class="bg-base-100 rounded-box p-4 shadow-sm">...</div>
```

Or use DaisyUI's `card` component for richer layouts.

### Badges and status indicators

```html
<span class="badge badge-success badge-sm">connected</span>
<span class="badge badge-warning badge-sm">partial</span>
<span class="badge badge-ghost badge-sm">inactive</span>
```

### Form inputs

```html
<input class="input input-bordered w-full" ... />
<select class="select select-bordered" ...>...</select>
<input type="checkbox" class="toggle toggle-primary" />
```

### Modals

Use DaisyUI's `dialog` / `modal` pattern. Open state driven by `open` attribute on `<dialog>`.

### Empty states

```html
<p class="text-base-content/40 italic text-sm">No items yet.</p>
```

### Interactive list items (nav, rule lists)

Active state: `bg-primary text-primary-content`
Hover state: `hover:bg-base-200`
Base: `text-base-content`

---

## Layout conventions

### Desktop app

```
┌─ TitleBar (h-9, bg-base-100, border-b border-base-300) ──────────┐
│  <title>        [theme toggle] [minimize] [maximize] [close]      │
├─ Sidebar (w-52, bg-base-100, shadow-md) ──┬─ Main ───────────────┤
│  Logo + nav links                         │  <main p-6>          │
│  State panel (layer stack, variables)     │                      │
│  Live panel (finger visualiser)           ├─ Footer (bg-base-100)│
└───────────────────────────────────────────┴──────────────────────┘
```

- Outer wrapper: `flex h-screen w-screen flex-col overflow-hidden bg-base-200`
- Sidebar: `flex w-52 flex-shrink-0 flex-col bg-base-100 shadow-md overflow-y-auto`
- Main: `flex-1 overflow-y-auto p-6`
- Footer: `border-t border-base-300 bg-base-100 px-4 py-1.5 text-xs`

### Site (marketing)

```
┌─ Nav (sticky, bg-base-100, border-b border-base-200, z-50) ──────┐
├─ Page content (flex-1) ──────────────────────────────────────────┤
└─ Footer (bg-base-200, border-t border-base-300) ─────────────────┘
```

- Outer wrapper: `min-h-screen flex flex-col bg-base-100`

---

## Animation

One custom keyframe is defined in `apps/desktop/src/app.css`:

```css
@keyframes tap-flash {
  0%   { transform: scale(1.25); }
  100% { transform: scale(1); }
}
```

Used in `FingerPattern.svelte` via `[animation:tap-flash_0.45s_ease-out]` to pulse a finger
segment when a tap is received. No other custom animations are in use.

---

## Adding new components

1. Use DaisyUI semantic colour tokens — never raw palette classes.
2. Match the padding / gap scale of neighbouring components.
3. Test visually in both `corporate` (light) and `business` (dark) themes before committing.
4. If a new colour need arises that DaisyUI tokens cannot cover, add a `@theme` override to
   `packages/design-tokens/base.css` so both apps inherit it automatically.
