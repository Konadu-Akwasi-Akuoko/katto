# Packaging katto

How the installable build is produced, what its signing state actually is, and how to
verify a fresh install. Complements PRD decisions D20 (signing deferred, stubs
documented) and D21 (Regular activation policy — Dock icon and tray both present).

## Building

```sh
just dmg
```

Runs `bun run tauri build --bundles app,dmg` (release profile + Vite production build;
expect several minutes cold). Artifacts:

- dmg: `src-tauri/target/release/bundle/dmg/katto_<version>_aarch64.dmg`
- app: `src-tauri/target/release/bundle/macos/katto.app`

`<version>` comes from `src-tauri/tauri.conf.json` (`version`). The bundle carries
`resources/thumbnail-templates/` (the guide-lined PSD scaffolds) via
`bundle.resources`; `bundle.macOS.minimumSystemVersion` is `13.0` — the
objc2/user-notifications/multi-webview stack sits comfortably above the 10.13 tauri
default, and the target machine runs current macOS.

For a quick `.app`-only build (no dmg), `just bundle` remains; it also picks up a
stable dev signing identity from the keychain when one exists (see
`scripts/macos-dev-sign.sh` for the keychain-ACL rationale).

## Signing reality (unsigned, deliberately)

No `signingIdentity` is configured, so the bundle carries only the linker's **ad-hoc
signature** (`codesign -dv …/katto.app` reports `Signature=adhoc`).

- A **locally built** dmg installs and runs normally — nothing downloaded, so no
  quarantine attribute, so Gatekeeper never engages.
- A **downloaded** copy of the same dmg would be quarantined and hit Gatekeeper
  ("unidentified developer"). Escape hatches: right-click → Open on the app, or
  `xattr -dr com.apple.quarantine katto.app`.

This is fine for a personal, one-Mac app. It becomes wrong the moment the dmg is
distributed.

### Real-signing stubs (paste when a Developer ID exists — D20)

`src-tauri/tauri.conf.json`:

```json
"bundle": {
  "macOS": {
    "minimumSystemVersion": "13.0",
    "signingIdentity": "Developer ID Application: <name> (<team>)",
    "hardenedRuntime": true
  }
}
```

Notarization is driven by environment variables consumed by `tauri build`:

```sh
export APPLE_ID="<apple id email>"
export APPLE_PASSWORD="<app-specific password>"
export APPLE_TEAM_ID="<team id>"
just dmg
```

`APPLE_SIGNING_IDENTITY="-"` is the explicit ad-hoc escape hatch (forces ad-hoc even
when identities exist in the keychain).

## Launch-at-login

Registered through `tauri-plugin-autostart` in AppleScript login-item mode (see the
plugin registration in `src-tauri/src/lib.rs`). In dev the login item points at the
**dev binary**, so the only meaningful test is from the installed bundle: enable it in
katto's settings, then confirm System Settings → General → Login Items shows "katto".
It should survive a reboot.

## Clean-profile test procedure

1. Create a new macOS user account and log into it.
2. Copy the dmg over (local copy — no quarantine) and install `katto.app` to
   `/Applications`.
3. Launch. Onboarding must walk: Studio root picker → ElevenLabs key → claude binary
   detection.
4. The menu-bar tray icon appears **and** the Dock icon is present (D21 — packaging
   must not change the activation policy; no `LSUIElement`).
5. Quit and relaunch: settings and login state persist; enabling launch-at-login puts
   "katto" in Login Items.
