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
    <CardTitle>Rough Cut · draft-v3</CardTitle>
    <CardDescription>AI-assembled from 4 SD cards · 18 clips kept</CardDescription>
  </CardHeader>
  <CardContent className="text-muted-foreground text-sm">
    Duration 12:41 · 1080p ProRes
  </CardContent>
  <CardFooter style={{ gap: 8 }}>
    <Button>Open in NLE</Button>
    <Button variant="outline">Re-cut</Button>
  </CardFooter>
</Card>
```

Note the katto flavor: serif `CardTitle`, ember-primary `Button`, muted metadata, warm card
surface — a real component for the control, tokens/utilities for your own layout glue.
