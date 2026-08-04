# Browser start page + toolbar occlusion fix

Design spec. Owner-approved direction, two sub-decisions taken by the implementer under stated
assumptions (see "Assumptions taken autonomously").

## Problem

Three defects/gaps in the Browser surface, found while reviewing the running app:

1. **The toolbar is invisible.** The 40px `Toolbar` row (back/forward, address bar, downloads
   popover trigger) is painted over by the native child webview. Only a 2–3px sliver of the
   address input's ember focus ring shows above the page. Because the downloads popover trigger
   lives in that row, the download-target project selector — which exists and works — is
   unreachable.
2. **The pane auto-opens Envato.** `browser-surface.tsx:96-113` opens the default tab on first
   mount with no tabs. The owner wants to land on a start page instead.
3. **There is no start page.** The empty state (`browser-surface.tsx:165`) is unreachable on
   entry, gated on `openFailed || sawTabs.current`, and offers only an "Open Envato Elements"
   button.

## Non-goals

Session restore across app restarts; user-editable tiles; a search-engine setting; automatic
download routing into `assets/music` or `assets/sfx`. Each is a separate decision.

## Part 1 — Toolbar occlusion

**RESOLVED 2026-07-28 by measurement against the running app.** A temporary probe rendered above
the webview's rect reported:

```
host.y=129.0 host.h=708.0 | innerH=837 screenY=900 dpr=2
| tauri innerH=869.0 outerH=869.0 innerPos.y=31.0 scale=2
```

The window's content area is 869 logical px tall; the document viewport is 837. **A 32px inset.**
Tauri gives every macOS window `FullSizeContentView`, so the content view spans the titlebar strip
while WKWebView lays the document out below it. wry positions child webviews against that content
view, so a DOM-relative `y` lands one inset too high — burying the 40px toolbar.

Fix: `use-browser-bounds.ts` derives the inset as `window inner height (logical) − innerHeight` and
adds it before reporting. Derived, never hardcoded; collapses to 0 where no inset exists
(fullscreen, other platforms), so it is a no-op rather than a new bug.

Note `window.screenY` reports **900** — the screen height, not the viewport origin. An earlier
attempt calibrated off `screenY` and was correctly removed as broken; the wrong conclusion drawn
from that removal was that no inset existed at all.

Verified in the running app: toolbar visible with a page loaded, downloads popover and its "Files
to" project selector reachable.

### Original hypotheses (both wrong, kept for the record)

- **Coordinate-space mismatch.** `apply_bounds` (`host.rs:411`) positions child webviews with
  `LogicalPosition` relative to the window; `getBoundingClientRect()` (`browser-surface.tsx:120`)
  measures relative to the main webview's viewport. The comment at `host.rs:19-20` asserts these
  "map 1:1". On macOS with a custom titlebar that assertion may be false by the titlebar inset.
  Symptom fits: the webview sits ~one toolbar-height too high.
- **Stale bounds.** `ResizeObserver` fires on size change only. A content host that *moves*
  without resizing (tab strip appearing/disappearing, toolbar mounting after first paint) leaves
  the last-reported rect stale.

Research must settle which, from tauri/wry source and the window construction in
`src-tauri/src/window.rs`. The fix must **derive** the correct position, never hardcode a pixel
inset. If research cannot settle it, implement the stale-bounds correction (which is sound
regardless) and record the coordinate-space question as unresolved rather than guessing.

## Part 2 — Start page

### Tab model

A tab may have no current URL. `TabModel` (`tabs.rs`) gains an optional current URL; such a tab
titles as "New tab". Consequences:

- `ensure_webview` does not create a webview for a URL-less tab.
- `sync_visibility` hides the webview whenever the active tab is URL-less, so the DOM start page
  is visible.
- Navigating a URL-less tab (search submit, tile click, address bar) sets its URL and creates the
  webview lazily — the existing path.
- `TabSnapshot.url` becomes nullable; the frontend and `bindings.gen.ts` follow.

Both `MultiWebviewHost` and `SingleWebviewHost` implement this.

### Behavior

- Opening the Browser pane with no tabs shows a start-page tab. The auto-open of Envato is
  removed.
- "+" opens a new tab on the start page rather than Envato.
- The unreachable-empty-state gate (`openFailed || sawTabs.current`) is removed; the start page
  is a normal tab state, not an error state.

### Search

`model/address.ts` gains a pure function turning free text into
`https://www.google.com/search?q=<encodeURIComponent(query)>`. Anything parsing as a URL still
navigates directly. TDD-first per `.claude/rules/testing.md`.

This function serves **both** the start-page search box and the address bar. The address bar's
`hint` state and "Enter a full address" copy are deleted, along with the "katto is not a search
engine" claim in the `Toolbar` doc comment — that decision is explicitly overturned by the owner.

### Tiles

Fixed constant, 4x2:

| Tile | URL |
|---|---|
| Envato Elements | https://elements.envato.com/ |
| Dribbble | https://dribbble.com/ |
| Pinterest | https://www.pinterest.com/ |
| TestMyThumbnails | https://www.testmythumbnails.com/ |
| YouTube Studio | https://studio.youtube.com/ |
| Unsplash | https://unsplash.com/ |
| Freesound | https://freesound.org/ |
| Coolors | https://coolors.co/ |

Freesound and Envato earn placement now that `assets/music` and `assets/sfx` exist.

### Visual treatment

**Not** the centered Chrome new-tab clone. `.claude/rules/design-system.md` bans "everything
centered" and templated hero treatments as AI tells. Instead: left-aligned and dense, search
field matching the address bar's width and height, tiles as compact rows or small marked entries.

**No remote favicons.** Fetching eight third-party icons on every new tab is a privacy and
loading cost, and circular favicon tiles are the exact Chrome-clone look being avoided. Use
Phosphor marks or letterforms.

## Part 3 — Docs

`prd/phase-7.md:31` states "Envato Elements preloaded as the default tab". That is now false.
Update it and any D16 twin in `prd/README.md` to describe the start page. Do not touch the
download-routing rows — routing is unchanged.

## Testing

- Pure: the search/URL normalizer (TDD-first), tab-model URL-less transitions.
- Rust: `TabModel` open/navigate/close with a URL-less tab; both host impls' visibility logic.
- Frontend: start page renders at zero tabs; a tile click navigates; search submit issues the
  encoded Google URL; the address bar searches on non-URL input.
- Gate: `just check`.

## Assumptions taken autonomously

The owner was away; these were decided by the implementer rather than blocking:

1. **katto-native visual treatment** over the centered Google layout the owner's reference image
   showed — justified by the committed design rules and the owner's recorded anti-AI-tell
   preference. Reversible: it is one component's layout.
2. **Fixed tile list, not user-editable** — YAGNI; editable tiles need a settings row and an
   editor UI.
