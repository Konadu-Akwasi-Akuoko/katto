# katto Design System — v1

Status: approved (owner design session, 2026-07-03). Source of truth for katto's visual
language. The distilled, enforced version lives in `.claude/rules/design-system.md`; the
runnable tokens live in `src/styles/main.css` (created during the Phase 1 frontend scaffold).

macOS-only, single user, menu-bar-resident. Renders in the system WebView (WebKit) and in
React 19 + Tailwind v4 + shadcn (copy-in Radix primitives). Everything below is framework-
agnostic at the token level.

---

## 1. Identity

katto is a **characterful workshop**, not a dashboard and not a toy. A menu-bar production OS
for one working video creator: dense, keyboard-first, running long AI jobs in plain view.

The character comes from **restraint, warmth, and material honesty** — never decoration. The
discipline is Raycast / Zed / Linear (the references the owner ranked highest); the warmth is
the Workshop Ember accent and a faint grain. If a choice reads as "themed," "decorated," or
"templated," it is wrong.

Reference study and rationale: the owner ranked Raycast > Zed > Linear > Arc — all quiet,
dense, keyboard-first tools. Loud references (Teenage Engineering, Gumroad, Descript) were
admired but not chosen as the spine. Takeaway: **discipline of a pro tool + warmth from
palette and texture, not ornament.**

---

## 2. Colour

OKLCH throughout, so the ember hue stays identical across themes and only lightness shifts.
Two themes: **warm-light** (default) and **dark** (katto's primary working theme).

Principle: **warmth lives in the accent, not the ground.** Light leans a faint amber; dark is
a near-neutral greyer ground (like Raycast's near-black) with only a whisper of warmth — it
must never read as a reddish/brown tint. The ember carries the heat.

### Light theme (`:root`)

| Token | OKLCH | Role |
|---|---|---|
| `--bg` | `oklch(0.968 0.012 78)` | app ground (warm off-white, not `#fff`) |
| `--surface` | `oklch(0.992 0.006 82)` | cards, panels |
| `--surface-2` | `oklch(0.935 0.014 78)` | raised / hover |
| `--border` | `oklch(0.872 0.016 76)` | structural borders |
| `--hairline` | `oklch(0.872 0.016 76 / 0.6)` | internal dividers |
| `--fg` | `oklch(0.245 0.018 60)` | primary text |
| `--fg-muted` | `oklch(0.500 0.014 64)` | secondary text |
| `--fg-faint` | `oklch(0.620 0.012 68)` | captions, metadata |
| `--ember` | `oklch(0.605 0.205 42)` | Workshop Ember accent |
| `--ember-hover` | `oklch(0.560 0.205 42)` | accent hover |
| `--on-ember` | `oklch(0.985 0.010 80)` | text/icon on ember |

### Dark theme (`.dark`)

| Token | OKLCH | Role |
|---|---|---|
| `--bg` | `oklch(0.148 0.004 74)` | darker, near-neutral ground |
| `--surface` | `oklch(0.188 0.005 74)` | cards, panels |
| `--surface-2` | `oklch(0.232 0.006 74)` | raised / hover |
| `--border` | `oklch(0.288 0.007 74)` | structural borders |
| `--hairline` | `oklch(0.288 0.007 74 / 0.7)` | internal dividers |
| `--fg` | `oklch(0.918 0.006 82)` | primary text |
| `--fg-muted` | `oklch(0.660 0.007 78)` | secondary text |
| `--fg-faint` | `oklch(0.518 0.007 76)` | captions, metadata |
| `--ember` | `oklch(0.705 0.185 46)` | ember, lifted for dark ground |
| `--ember-hover` | `oklch(0.750 0.185 46)` | accent hover |
| `--on-ember` | `oklch(0.185 0.020 50)` | dark text on ember |

Note the chroma discipline in dark: neutrals sit at `0.004–0.007` (near-grey), hue `74–82`
(faint warm), so there is no reddish cast. The accent stays saturated (`0.185`).

### Semantic colours (separate from the accent)

State colour is **not** the accent hue. The accent (ember) may double as "running / work in
progress," but done/failed/queued/warning are their own hues.

| Token | Light | Dark | Meaning |
|---|---|---|---|
| `--done` | `oklch(0.520 0.130 150)` | `oklch(0.760 0.140 152)` | completed |
| `--failed` | `oklch(0.545 0.205 25)` | `oklch(0.680 0.180 26)` | failed / error |
| `--queued` | `oklch(0.600 0.012 68)` | `oklch(0.560 0.010 76)` | queued (neutral) |
| `--warn` | `oklch(0.640 0.130 78)` | `oklch(0.810 0.130 82)` | warning / banner |

---

## 3. Typography

All native Apple faces — zero downloaded fonts (katto is macOS-only, WebKit). This dodges the
Inter / Space Grotesk "AI default" tell entirely.

| Role | Stack | Use |
|---|---|---|
| Display | `ui-serif, "New York", "Iowan Old Style", Georgia, serif` | headings, page/panel titles, wordmark — the characterful warmth |
| UI / body | `-apple-system, "SF Pro Text", "SF Pro Display", system-ui, sans-serif` | all interface and running text |
| Data | `ui-monospace, "SF Mono", "Menlo", monospace` | machine truth **only** |

**Mono is for machines, never texture.** SF Mono appears only where alignment is functional:
timecodes (`00:04:12:18`), file paths, job ids, rational time (`1001/30000 s`). Always with
`font-variant-numeric: tabular-nums`. It never dresses a heading, label, or eyebrow.

### Type scale

| px | Face | Role |
|---|---|---|
| 40 / 44 | New York | display, page titles |
| 26 | New York | section headings |
| 18 / 20 | New York or SF Pro | panel headings, lead text |
| 14 | SF Pro | body and interface — the base size |
| 12 / 13 | SF Pro | secondary, captions, metadata |
| 12 | SF Mono | data, timecodes, paths |

Headings get `text-wrap: balance`; body stays near 65ch; display sizes track tight
(`letter-spacing: -0.02em`).

---

## 4. Icons

**Phosphor** (`@phosphor-icons/react`), Regular weight for everything, **Bold** for emphasis
only. Geometric with warmth — pairs with SF Pro and avoids the thin-line Lucide "AI default."

- Sizes: **16px** inline (dense rows), **20px** standalone. 24px rare.
- SF Symbols would be the native ideal but Apple's licence bars web/WebView use — Phosphor is
  the chosen web-friendly substitute.
- Icon↔text gap is `--space-2` (8px).

---

## 5. Spacing — one 4px grid

Every padding, gap, and margin snaps to a 4px base. No eyeballed values.

`--space-1: 4` · `--space-2: 8` · `--space-3: 12` · `--space-4: 16` · `--space-6: 24` ·
`--space-8: 32` · `--space-12: 48` · `--space-16: 64`

Lay out sibling groups with flex/grid + `gap`, not per-element margins.

---

## 6. Radii — 6 / 10

Fixed two-step. Buttons are **never** fully rounded pills (that's a tell).

- `--r: 6px` — chips, inputs, buttons, small controls.
- `--r-lg: 10px` — cards, panels, tray, command palette, floating surfaces.
- `50%` — only dots, avatars, status indicators.

---

## 7. Elevation — border first, shadow last

Definition comes from a 1px border + a surface lightness step, not drop shadows.

- **e0 — flat**: `1px solid var(--border)` + `var(--surface)`. ~99% of the UI.
- **e1 — floating**: tray, command palette, dropdowns, toasts. One soft, tight shadow.
- **e2 — modal**: the only heavier shadow.

**In dark mode, elevation is a lighter surface, not a shadow** (shadows are invisible on
near-black). Shadow tokens:

- Light `--shadow`: `0 1px 2px oklch(0.245 0.018 60 / 0.08), 0 4px 16px oklch(0.245 0.018 60 / 0.06)`
- Dark `--shadow`: `0 1px 2px oklch(0 0 0 / 0.40), 0 8px 22px oklch(0 0 0 / 0.34)`

No glassmorphism, no frosted panels, no shadow theater.

---

## 8. Motion — one curve

`--ease: cubic-bezier(0.2, 0, 0, 1)` everywhere. Durations: `--dur-fast: 120ms` (hover/state),
`--dur: 160ms` (open/close), `--dur-slow: 240ms` (progress/reveal). No bounce, no spring.
Progress and state are the only things worth animating. All motion is gated behind
`@media (prefers-reduced-motion: no-preference)` / disabled under `reduce`.

---

## 9. Sizing / density

Fixed control heights keep it Raycast-tight, not web-roomy.

- List row `30px` · button `32px` · input `32px` · chip `20px` · tray item `32px`.
- Dashboard/feed/jobs are dense; onboarding and empty states get room to breathe.

---

## 10. Grain — the whisper of texture

A fine procedural grain sits over both themes — finer than Zed's, never consciously seen. It
kills the plastic, too-clean flatness that reads as machine-made. Established, well-regarded
2026 practice; the only failure mode is overuse, so it stays subtle.

**Technique (researched best practice):** a small fixed-size SVG `feTurbulence` tile used as a
tiled `background-image`, blended into each surface with `background-blend-mode: overlay`
(centers on mid-grey → adds grain without dulling colour).

- Tile 160×160, `type="fractalNoise"`, `baseFrequency="0.8"` (fineness knob),
  `numOctaves="2"` (keep ≤ 3), `stitchTiles="stitch"` (no seams when repeated),
  `feColorMatrix saturate 0` (greyscale), `feComponentTransfer` linear
  `slope 0.28 intercept 0.36` compressing contrast into a narrow band (strength knob).
- Why this way: the filter rasterizes **once** on the tiny tile, then repeats as a cheap
  bitmap — no live `filter:` on a large element, no `background-attachment: fixed` (both
  cause per-repaint cost, bad on WebKit).
- Applied to `.app` (ground) and raised surfaces. Slightly more present on dark than light.
- Fallback only if WebKit ever hitches on first paint: pre-bake the tile to a PNG data URI.

The exact `--grain` value is in the `main.css` block below.

---

## 11. Native details (the "not a website" signal)

- **Arrow cursor on controls** — macOS apps do not show the hand/pointer cursor on buttons.
  Keep `cursor: default` on interactive elements; `cursor: text` only in text fields.
- **Tabular numerals** wherever digits align (percentages, counts, timecodes).
- Focus: `2px solid var(--ember)` ring at `2px` offset, visible on keyboard focus.
- Disabled: `45%` opacity. Hover: one surface step. No per-component invention.

---

## 12. Anti-tell rules (non-negotiable)

katto must not read as AI-generated. These patterns are banned unless the owner explicitly
asks for one:

- **Mono-uppercase-letterspaced eyebrows / kickers / preheads.** Put context into the heading
  or a plain inline label, or omit it.
- **Monospace as decorative texture.** Data only (§3).
- **Accent rail down the side of a card.** State is encoded **once** — in a chip or a dot,
  never a coloured stripe. (A card that already has a "failed" chip must not also have a red
  left border.)
- Interpunct `·` used as separator filler.
- Fake technical part-numbers / faux-metadata as decoration.
- Bento grids.
- `01 / 02 / 03` numbered markers unless the content is a real ordered sequence.
- Emoji as section markers or bullets.
- Gradient heroes (especially purple→blue).
- Hyperpolished perfect symmetry with zero texture (the grain and slight asymmetry counter it).
- Glassmorphism / frosted panels.
- Everything centered; everything `rounded-2xl`.
- Copy tells: "not just X, it's Y" negative parallelism, rule-of-three everywhere, "delve,"
  "seamless / effortless / elevate / unlock," "in today's fast-paced world."

Copy voice: write from the user's side of the screen; active voice; a control says exactly
what happens ("Export to Final Cut," then a result). Errors explain what went wrong and how to
fix it. Specific beats clever.

---

## 13. Component conventions

- **Buttons** — primary (ember bg, `--on-ember` text), secondary (surface + border), ghost
  (transparent, muted → fg on hover). Radius 6, height 32, arrow cursor.
- **Status chips** — `● label`, radius 6, height 20, colour + a 6px dot; `running` uses ember,
  `done/failed/queued` use semantic colours. State appears once (see §12).
- **Cards / panels** — e0 (border + surface), radius 10, grain-textured. No accent rail.
- **Progress bars** — 4px track (`--surface-2`), ember fill; failed fill uses `--failed`.
- **Command palette (⌘K)** — e1 surface, Phosphor search icon, grouped commands, mono shortcut
  hints, Phosphor leading icon per command.
- **Tray menu** — quiet chrome, text-forward (native menu idiom), live project + next-shoot
  lines, active job mirrored with ember highlight.
- **Drive banner** — warn-tinted surface + border, warn dot, mono expected path, a Retry ghost
  button. Not an error toast — a persistent state.

---

## 14. Implementation — `src/styles/main.css`

Tailwind v4 CSS-first. `@custom-variant dark` drives the `.dark` class (toggled on `<html>`
from the Zustand UI store; the chosen theme persists in the `settings` table). `@theme inline`
exposes tokens to Tailwind utilities. Ready-to-drop-in skeleton:

```css
@import "tailwindcss";
@custom-variant dark (&:is(.dark *));

:root {
  --bg: oklch(0.968 0.012 78);
  --surface: oklch(0.992 0.006 82);
  --surface-2: oklch(0.935 0.014 78);
  --border: oklch(0.872 0.016 76);
  --hairline: oklch(0.872 0.016 76 / 0.6);
  --fg: oklch(0.245 0.018 60);
  --fg-muted: oklch(0.500 0.014 64);
  --fg-faint: oklch(0.620 0.012 68);
  --ember: oklch(0.605 0.205 42);
  --ember-hover: oklch(0.560 0.205 42);
  --on-ember: oklch(0.985 0.010 80);
  --done: oklch(0.520 0.130 150);
  --failed: oklch(0.545 0.205 25);
  --queued: oklch(0.600 0.012 68);
  --warn: oklch(0.640 0.130 78);
  --shadow: 0 1px 2px oklch(0.245 0.018 60 / 0.08), 0 4px 16px oklch(0.245 0.018 60 / 0.06);

  --serif: ui-serif, "New York", "Iowan Old Style", Georgia, serif;
  --sans: -apple-system, "SF Pro Text", "SF Pro Display", system-ui, sans-serif;
  --mono: ui-monospace, "SF Mono", "Menlo", monospace;

  --space-1: 4px; --space-2: 8px; --space-3: 12px; --space-4: 16px;
  --space-6: 24px; --space-8: 32px; --space-12: 48px; --space-16: 64px;
  --r: 6px; --r-lg: 10px;
  --ease: cubic-bezier(0.2, 0, 0, 1);
  --dur-fast: 120ms; --dur: 160ms; --dur-slow: 240ms;

  --grain: url("data:image/svg+xml,%3Csvg%20xmlns='http://www.w3.org/2000/svg'%20width='160'%20height='160'%3E%3Cfilter%20id='g'%3E%3CfeTurbulence%20type='fractalNoise'%20baseFrequency='0.8'%20numOctaves='2'%20stitchTiles='stitch'/%3E%3CfeColorMatrix%20type='saturate'%20values='0'/%3E%3CfeComponentTransfer%3E%3CfeFuncR%20type='linear'%20slope='0.28'%20intercept='0.36'/%3E%3CfeFuncG%20type='linear'%20slope='0.28'%20intercept='0.36'/%3E%3CfeFuncB%20type='linear'%20slope='0.28'%20intercept='0.36'/%3E%3C/feComponentTransfer%3E%3C/filter%3E%3Crect%20width='100%25'%20height='100%25'%20filter='url(%23g)'/%3E%3C/svg%3E");
}

.dark {
  --bg: oklch(0.148 0.004 74);
  --surface: oklch(0.188 0.005 74);
  --surface-2: oklch(0.232 0.006 74);
  --border: oklch(0.288 0.007 74);
  --hairline: oklch(0.288 0.007 74 / 0.7);
  --fg: oklch(0.918 0.006 82);
  --fg-muted: oklch(0.660 0.007 78);
  --fg-faint: oklch(0.518 0.007 76);
  --ember: oklch(0.705 0.185 46);
  --ember-hover: oklch(0.750 0.185 46);
  --on-ember: oklch(0.185 0.020 50);
  --done: oklch(0.760 0.140 152);
  --failed: oklch(0.680 0.180 26);
  --queued: oklch(0.560 0.010 76);
  --warn: oklch(0.810 0.130 82);
  --shadow: 0 1px 2px oklch(0 0 0 / 0.40), 0 8px 22px oklch(0 0 0 / 0.34);
}

@theme inline {
  --color-bg: var(--bg);
  --color-surface: var(--surface);
  --color-surface-2: var(--surface-2);
  --color-border: var(--border);
  --color-fg: var(--fg);
  --color-fg-muted: var(--fg-muted);
  --color-fg-faint: var(--fg-faint);
  --color-ember: var(--ember);
  --color-on-ember: var(--on-ember);
  --color-done: var(--done);
  --color-failed: var(--failed);
  --color-queued: var(--queued);
  --color-warn: var(--warn);
  --font-serif: var(--serif);
  --font-sans: var(--sans);
  --font-mono: var(--mono);
  --radius: var(--r);
  --radius-lg: var(--r-lg);
}

html, body { background: var(--bg); color: var(--fg); font-family: var(--sans); }

/* grain: baked into surfaces, not a live filter or overlay layer */
.grain, .app, [data-surface] {
  background-image: var(--grain);
  background-blend-mode: overlay;
}
```

Grain is applied by giving the ground (`.app`) and surface components the tile via
`background-blend-mode`; shadcn card/panel primitives get a `data-surface` or a shared class so
they inherit it.

---

## 15. Deferred (define when a screen first needs it)

Sidebar layout specifics, empty-state / illustration voice, data-viz beyond progress bars,
toast styling. Do not over-spec these in the abstract.

---

## 16. Reference

Living visual reference (light + dark, all components):
`https://claude.ai/code/artifact/de88a0cc-5ee3-4770-ada7-963c4f468c1f`
