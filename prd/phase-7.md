# Phase 7 — Project Browser, Thumbnails, Resolve, Import & Ship

## Goal

Close the workflow loop (assets file themselves; thumbnails scaffold themselves; Resolve gets
a first-class import; the old planner's ideas migrate in) and ship: design-polish pass and an
installable `.dmg`.

## Why this order

Everything here decorates a working spine. Last also because the browser rides Tauri's
`unstable` multi-webview feature — the highest-risk binding goes in when everything else
already works.

## User stories

- I browse Envato inside katto (still logged in from last week), download a texture pack, and
  it lands unzipped in the active project's `assets/envato/` with a license sidecar — I never
  see `~/Downloads`.
- "New thumbnail" drops a guide-layered PSD into `thumbnails/` and opens Photoshop; when I
  export a PNG, the project card shows it.
- "Open in Resolve" builds the project in Resolve Studio with media in the pool.
- Settings → "Import from studio.db" pulls my old ideas in once, statuses mapped, promoted
  slugs intact.
- I install katto from a `.dmg` on a clean profile and onboarding just works.

## Scope with acceptance criteria

| Feature | Acceptance criteria |
|---|---|
| Browser shell | In-app surface: tab strip, address bar, back/forward, new/close tab; multi-webview (`unstable` feature): one `Window`, app UI webview + one child `WebviewBuilder` per tab, **only the active tab's webview visible** (sidesteps open macOS multi-visible bug tauri#11376); implemented behind a `BrowserTabHost` trait — fallback impl = single reused webview with per-tab serialized URL/history; persistent session partition (Envato login survives restarts); no preloaded home site — an empty pane and every new tab land on a **start page** (search field → Google, plus fixed tiles: Envato Elements, Dribbble, Pinterest, TestMyThumbnails, YouTube Studio, Unsplash, Freesound, Coolors), so a tab may hold no URL at all; the address bar also searches (non-URL input → Google); `target="_blank"` and `window.open` open a new **foreground tab in the strip**, never a macOS window (`WebviewBuilder::on_new_window` → `NewWindowResponse::Deny`, the tab created off the delegate thread because `add_child` blocks on the main thread); wry reports no navigation type, so a script-fired popup is indistinguishable from a click and both open — non-http(s) popups (`about:`/`blob:`/`data:`, which only exist to be scripted through the now-null opener handle) are refused with a `browser_popup_refused` events row; explicitly no extensions/profiles (D16) |
| Download interception | `WebviewBuilder::on_download`: `Requested` rewrites `destination` into `<active project>/assets/` (`assets/envato/` when the source URL is an Envato domain); `Finished{success}` → archive? unzip into a folder named for the item, remove the archive; sidecar `<name>.license.json = {item_url, page_url, downloaded_at, project}`; events row; **no active project** → sheet asks which project before accepting; interception failure/blob edge cases → file goes to `~/Downloads` + notice banner (test blob: downloads against Envato specifically) |
| Thumbnails | "New thumbnail" on project detail: template picker (1280×720 / 1080×1920 vertical) → copy bundled PSD (Tauri resource with guide layers + safe zones) to `thumbnails/<slug>-thumb-<letter>.psd` (letter increments) → `open -a "Adobe Photoshop"`; `notify` watch on `thumbnails/` shows the newest exported PNG on the project card (D17 — no design intelligence in katto) |
| Resolve import | "Open in Resolve": drives Resolve Studio's scripting API (spawn its Python with `DaVinciResolveScript`) to create a project, import the exported FCPXML, and populate the media pool; requires Resolve **Studio** running (free Resolve explicitly unsupported — D12); clear typed errors for not-running/not-Studio/scripting-disabled |
| studio.db import | Settings action, one-time + idempotent (re-run = upsert by `id`): reads `ideas` from the configured `studio.db` path (default `~/Projects/WebDev/hyper-frames/tools/studio/studio.db`); `PRAGMA table_info` guard for post-ship columns (`kind_source`, `kind_why`); status map `new\|keep→backlog`, `rejected→discarded`, `promoted→promoted`; `promoted_slug` preserved **verbatim**; report `{imported, updated, skipped}`; source DB opened read-only, never modified |
| Design-polish pass | Whole-app pass against the Raycast/Linear bar (run the frontend-design skill): token audit, motion (promote animation, dock pulse), empty states, keyboard affordances; no functional changes |
| Packaging | `.dmg` via Tauri bundler; launch-at-login verified **from the bundle**; onboarding re-tested on a clean macOS profile; bundle identifier + notification entitlements correct; signing/notarization deferred (D20) but the config stubs documented |

## Backend (Rust)

New modules: `browser.rs` + `browser/` (`tabs.rs` — `BrowserTabHost` trait + multiwebview and
single-webview impls, `downloads.rs` — interception + filing + unzip + sidecar),
`thumbnails.rs` (scaffold + watch), `resolve.rs` (scripting bridge), `import_studio.rs`,
`commands/{browser,thumbnails,resolve,import}.rs`.

Crates added: `zip` (unzip), tauri `unstable` cargo feature (pin the tauri minor version —
unstable API may shift). Bundled resources: `resources/thumbnail-templates/*.psd`
(`tauri.conf.json` `bundle.resources`; no PSD crate — copy only).

## Frontend (React)

`src/features/browser/` (tab strip, address bar, nav buttons, start page — a loaded page
renders in the child webview, not React; React draws only chrome and reserves the content
rect, except while the active tab has no URL, when the child webview is hidden and React
renders the start page into that rect), download notices; `src/features/thumbnails/` section
on project detail (template picker, PNG preview grid); import wizard in settings (path picker
→ dry-run report → confirm); polish-pass changes across `styles/main.css` tokens +
`components/ui/`.

## Wiring / IPC

| Command | Notes |
|---|---|
| `browser_open_tab(url?) -> tab_id` / `browser_close_tab` / `browser_select_tab` / `browser_navigate(tab_id, url)` / `browser_go(tab_id, delta)` | chrome ↔ host |
| `browser_state() -> {tabs: [{id, title, url?, can_go_back, can_go_forward}], active}` | + `browser-state-changed` broadcast; `url` is null on a start-page tab (titled "New tab") |
| `set_active_asset_project(slug?)` | download filing target (defaults to last touched) |
| `create_thumbnail(slug, format) -> psd_path` | scaffold + open Photoshop |
| `open_in_resolve(slug, timeline_version?) -> ()` | scripting bridge |
| `import_studio_db(path, dry_run: bool) -> ImportReport` | `{imported, updated, skipped, warnings}` |

## Data-model deltas

None (imported ideas use the existing `ideas` table; browser session state is webview-managed).

## Error handling

- Multi-webview instability on the installed tauri version → `BrowserTabHost` swaps to the
  single-webview fallback via a settings flag; the feature degrades, never crashes the shell.
- Download with no writable project target → `~/Downloads` fallback + persistent notice
  (D16); unzip failure keeps the archive in place with the sidecar noting the failure.
- Resolve not running / free edition / scripting off → typed errors with the exact remedy
  string; never launches Resolve itself.
- studio.db missing/locked/schema-alien → dry-run reports it; import never partially applies
  (single transaction).
- Photoshop missing → PSD still scaffolds; open action falls back to reveal-in-Finder.

## Testing

- Pure: unzip+filing layout decisions, sidecar content, Envato-domain routing; import mapping
  (status map, column-guard behavior, idempotent re-run) against a fixture `studio.db` built
  from the hyper-frames DDL with both pre- and post-`ensureColumns` shapes.
- Integration: `BrowserTabHost` contract tests over the fallback impl (tab lifecycle,
  history); thumbnail scaffold + watch (tempdir).
- Manual checkpoints: real Envato download intercepted (incl. a blob-triggered one); Resolve
  Studio import of a real timeline; `.dmg` install on a clean profile → onboarding →
  end-to-end: idea → schedule → ingest → cut → export → assets → thumbnail without leaving
  the app except FCP/Photoshop.

## Out of scope

Extensions/profiles/devtools in the browser; PSD content generation or layer editing;
Premiere-dialect XML; notarization/distribution; multi-user anything.

## Exit criteria

The full loop runs without leaving the app except FCP/Photoshop; installable `.dmg` verified
on a clean profile; `just check` green. **Follow-up ledger for post-ship:** retire
`tools/studio` after the planner proves out (D8); revisit signing/notarization when
distribution matters.
