---
paths:
  - "src/**/*.tsx"
  - "src/**/*.css"
---

# Design-system rules (katto visual language)

Full spec: `docs/superpowers/specs/2026-07-03-katto-design-system.md` (local). Tokens live in
`src/styles/main.css`. These rules are enforced on every UI change — follow them, don't restate
the spec.

katto is a **characterful workshop**: dense, keyboard-first, warm. Character comes from
restraint, warmth, and grain — never decoration. If a choice reads as themed, decorated, or
templated, it is wrong.

## Tokens, never literals

- Never hardcode a colour, radius, spacing, shadow, or duration. Use the CSS variables /
  Tailwind tokens: `--bg --surface --surface-2 --border --hairline --fg --fg-muted --fg-faint
  --ember --ember-hover --on-ember` and semantic `--done --failed --queued --warn`.
- Colour is OKLCH. Both themes ship: light (`:root`) and dark (`.dark`, katto's primary). Style
  through tokens so both themes work; never write colours only inside a dark selector.
- Warmth is in the accent, not the ground: dark neutrals are near-grey (chroma ~0.005), never a
  reddish/brown tint.

## Type

- Display/headings/wordmark → `--serif` (New York). UI + body → `--sans` (SF Pro). Data only →
  `--mono` (SF Mono) with `tabular-nums`: timecodes, paths, ids, rational time.
- **Mono is for machines, never texture.** No mono labels, eyebrows, or chrome. No downloaded
  fonts (native Apple faces only).

## Icons

- Phosphor (`@phosphor-icons/react`), Regular weight, Bold for emphasis only. 16px inline, 20px
  standalone. Never introduce a second icon set.

## Layout primitives

- Spacing: 4px grid (`--space-*`), via flex/grid `gap` — not per-element margins.
- Radii: `--r` (6px) for controls/chips/inputs/buttons; `--r-lg` (10px) for cards/panels/tray/
  palette; `50%` only for dots/avatars. Buttons are never full pills.
- Elevation: border + surface step first (e0, ~all UI). `--shadow` only for floating layers
  (tray, palette, dropdown, toast) and modals. In dark, elevation is a lighter surface.
- Motion: `--ease` + `--dur-fast|--dur|--dur-slow`. No bounce/spring. Gate on
  `prefers-reduced-motion`.
- Density: fixed heights (row/tray 30–32, button/input 32, chip 20). Dashboards dense;
  onboarding breathes.

## Native-app signals

- `cursor: default` on buttons/controls (macOS apps show no hand cursor); `cursor: text` only in
  fields.
- `tabular-nums` wherever digits align. Focus ring: `2px solid var(--ember)`, 2px offset.
  Disabled: 45% opacity.

## Grain

- The fine grain is baked into surfaces via the `--grain` `feTurbulence` tile +
  `background-blend-mode: overlay` (see main.css). `Card` carries the `grain` utility by
  default. Do not add live `filter:` noise, a full-viewport overlay layer, or
  `background-attachment: fixed`. Keep it subtle.
- Grain goes on OPAQUE surfaces only. On a translucent fill (e.g. a `bg-warn/10` alert)
  `background-blend-mode` amplifies the noise — opt that element out with
  `style={{ backgroundImage: "none" }}`.

## Banned — reads as AI-generated (never do these unless the owner asks)

- Mono-uppercase-letterspaced **eyebrows / kickers**. Put context in the heading or omit it.
- **Accent rail down the side of a card.** Encode state **once** — a chip or dot — never also a
  coloured stripe.
- Mono as decorative texture; interpunct `·` filler; fake part-numbers / faux-metadata; bento
  grids; `01/02/03` markers unless it's a real sequence; emoji section markers; gradient heroes;
  glassmorphism/frosted panels; everything centered; everything `rounded-2xl`.
- Copy tells: "not just X, it's Y," rule-of-three, "delve," "seamless/effortless/elevate/
  unlock," "in today's fast-paced world." Write from the user's side; a control says what it
  does; errors say what broke and how to fix it.

## Components

- Buttons: primary (ember/on-ember), secondary (surface+border), ghost. Chips: `● label`, state
  colour + dot, state shown once. Cards/panels: e0, `--r-lg`, grain, no rail. Progress: 4px
  track, ember fill (failed → `--failed`).
