---
name: emitter-snapshot-change
description: Use when changing any snapshot-frozen emitter in katto-engine — FCPXML, SRT/VTT, or ffmpeg filtergraph text — or when an insta snapshot test fails after an engine change.
---

# Change a snapshot-frozen emitter

Emitter output is contract, not implementation detail: FCP rejects out-of-order FCPXML elements with cryptic errors, and filtergraph text must stay byte-identical for determinism. Snapshots are how the contract is frozen.

## Checklist (todo per item)

1. **Decide: bug or contract change?** If the snapshot diff is *unexpected*, the code change is wrong — fix the code, don't accept the snapshot. Only intentional output changes proceed.
2. **Failing test first.** For new emitter behavior (e.g. a new element, the rescue track), add the fixture + named snapshot assertion before implementing: `assert_snapshot!("<scenario_name>", output)`.
3. **Run** `cargo insta test` — inspect every pending snapshot.
4. **Review** with `cargo insta review` — accept only diffs you can explain line-by-line. An unexplained attribute reorder is a regression, not noise.
5. **Invariants intact.** FCPXML: every `offset`/`duration`/`start`/`frameDuration` is rational `<num>/<den>s` (never a decimal), rescue-track fixture still emits removed segments on the disabled/muted track, element order unchanged unless that was the point. Filtergraphs: floats at 6 decimals, `trim/atrim + concat` shape.
6. **Commit `.snap` files in the same commit as the code change**, reason in the body. Never commit `.snap.new` (gitignored and hook-blocked anyway).
7. **Real-import check.** If the FCPXML structure changed (not just values), note in the PR that the manual FCP/Resolve import checkpoint must re-run before release.
8. **Gate.** `just check`.

## Red flags

- Accepting a snapshot to "make CI green" without explaining the diff.
- Editing a `.snap` file by hand.
- Converting a rational time to a decimal to simplify a diff.
