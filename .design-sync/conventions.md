# katto — Workshop Ember

katto is a personal macOS "Studio OS" for a YouTube production workflow. Its UI is a
shadcn-style React library (radix-ui primitives, `class-variance-authority` variants)
themed in **Workshop Ember**: a warm, paper-light palette with an ember-orange accent, a
serif for titles, and hairline borders. Import every component from `window.Katto`.

## Setup — components are self-styling; theme comes from tokens

There is **no global provider to wrap the app in**. The bundle's `styles.css` defines all
theme tokens on `:root`, and each component carries its own styles. Two things to know:

- **Dark mode**: put `class="dark"` on any ancestor. Only the katto tokens flip; every
  component follows automatically (the shadcn color contract aliases onto katto tokens).
- **Tooltip is the one component that needs a wrapper**: nest it in `TooltipProvider`
  (`<TooltipProvider><Tooltip>…</Tooltip></TooltipProvider>`) — without it the tooltip
  throws. Everything else renders standalone.

## Styling idiom — Tailwind utilities + katto tokens

Style with Tailwind utility classes. Reach for katto's own token utilities so designs stay
on-brand; the shadcn contract classes are aliases onto the same tokens (either works).

| Purpose | katto utilities | shadcn alias |
|---|---|---|
| Surfaces | `bg-surface`, `bg-surface-2` | `bg-card`, `bg-secondary`, `bg-accent` |
| Text | `text-fg`, `text-fg-muted`, `text-fg-faint` | `text-muted-foreground` |
| Accent (ember) | `bg-ember`, `text-ember` | `bg-primary`, `text-primary` |
| Status | `bg-done`, `bg-failed`, `bg-queued`, `bg-warn` (+ `text-*`) | `bg-destructive` (= failed) |
| Type | `font-serif` (titles), `font-mono` (data/timecode) | — |
| Borders | `border-input` | `border` |

**For anything not in that table, use the CSS variables directly** — they are always defined
in `styles.css`, purge-proof: `style={{ background: "var(--surface)", color: "var(--fg)",
borderColor: "var(--hairline)" }}`. Full token set (each has a `--color-*` twin for Tailwind):
`--bg --surface --surface-2 --fg --fg-muted --fg-faint --ember --ember-hover --on-ember
--hairline --done --failed --queued --warn`; fonts `--serif --sans --mono`. Prefer these
var(--*) tokens over inventing new utility names — the shipped stylesheet is Tailwind-purged
to what katto already uses, so novel utilities may not resolve. For rounding use the
components' own corners or `rounded-md` / `rounded-lg`: radii are modest (controls ~6px,
cards ~10px) and **there are no full pills**.

## Voice and anti-tells — this DS must NOT look AI-generated

katto is a **characterful workshop**, not a dashboard and not a landing page: dense,
keyboard-first, warm. Character comes from restraint, warmth, and grain — never decoration.
If a layout reads as themed, decorated, or templated, it is wrong. The reference discipline is
Raycast / Zed / Linear (quiet, dense, pro tools); the warmth is the ember accent and a faint
grain, not ornament.

**Never produce these — they read as AI-generated:**

- Mono-uppercase-letterspaced **eyebrows / kickers / preheads**. Put the context in the heading
  or a plain inline label, or omit it.
- **An accent rail / coloured stripe down the side of a card.** Encode state ONCE — a chip or a
  dot — never also a stripe. (A card with a "failed" chip must not also get a red left border.)
- **Monospace as decorative texture.** `font-mono` is machine data ONLY (timecodes, file paths,
  ids, rational time), always with `tabular-nums` — never for labels, headings, or chrome.
- Interpunct `·` as separator filler; fake part-numbers or faux-metadata as decoration.
- Bento grids; `01 / 02 / 03` numbered markers unless it's a real ordered sequence; emoji as
  section markers or bullets.
- Gradient heroes (especially purple→blue); glassmorphism / frosted panels.
- Everything centered; everything `rounded-2xl`; full-pill buttons; flawless symmetry with no
  texture.
- Downloaded webfonts (Inter, Space Grotesk, thin-line Lucide, etc.). katto uses only native
  Apple faces — serif New York, sans SF Pro, mono SF Mono — served by the OS.

**Type discipline**: serif (`font-serif`) for display, titles, and the wordmark — that's the
warmth; sans (SF Pro, the default) for all UI and body; mono (`font-mono`) for machine data
only. Mono is for machines, never texture.

**Native-app signals** (this is a macOS app, not a website): arrow cursor on controls (no hand
pointer); `tabular-nums` wherever digits align; focus ring `2px solid var(--ember)` at 2px
offset; disabled at ~45% opacity; modest radii (6px controls, 10px cards), no pills. A faint
grain already lives in the surfaces (`Card` carries it) — never add a live `filter:` noise
layer or a full-viewport overlay.

**Copy voice**: write from the user's side of the screen, active voice. A control says exactly
what it does ("Export to Final Cut"), then a result. Errors say what broke and how to fix it.
Banned copy tells: "not just X, it's Y" negative parallelism, rule-of-three everywhere,
"delve," "seamless / effortless / elevate / unlock," "in today's fast-paced world." Specific
beats clever.

## Where the truth lives

- `styles.css` (and its `@import`s, incl. `_ds_bundle.css`) — the single stylesheet; the
  token definitions and every component's compiled CSS. Read it before styling.
- Per component: `<Name>.d.ts` (the props contract) and `<Name>.prompt.md` (usage). Read the
  component's own files before composing it.

## One idiomatic composition

```jsx
const { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter, Button } = window.Katto;

<Card>
  <CardHeader>
    <CardTitle>Rough Cut — draft-v3</CardTitle>
    <CardDescription>AI-assembled from 4 SD cards, 18 clips kept</CardDescription>
  </CardHeader>
  <CardContent className="text-muted-foreground text-sm">
    12:41, 1080p ProRes
  </CardContent>
  <CardFooter style={{ gap: 8 }}>
    <Button>Open in NLE</Button>
    <Button variant="outline">Re-cut</Button>
  </CardFooter>
</Card>
```

Note the katto flavor: serif `CardTitle`, ember-primary `Button`, muted metadata, warm card
surface — a real component for the control, tokens/utilities for your own layout glue.
