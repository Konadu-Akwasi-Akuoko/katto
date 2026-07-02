---
paths:
  - "crates/**/*.rs"
  - "src-tauri/**/*.rs"
---

# Rust rules

## Naming and modules

- RFC-430 naming: `snake_case` files/modules/fns, `UpperCamelCase` types/traits, `SCREAMING_SNAKE_CASE` consts.
- 2018 module style: parent module is `foo.rs` with children in `foo/bar.rs`. Never create a new `mod.rs` (exception: `tests/common/mod.rs`, where Cargo requires it).
- Split a module when it grows a second concern or a second consumer, not at a line count. One primary type or domain per file; a file over ~500 lines needs a stated reason.

## Errors

- One `thiserror` error enum per crate in `error.rs`. The app crate's `Error` wraps `katto_engine::Error` via `#[from]`.
- `anyhow` only in binary `main` functions, never in library APIs.
- No `unwrap()` / `expect()` outside `#[cfg(test)]` code. Propagate with `?`; if a value is truly infallible, make the type prove it.

## Crate boundaries

- Dependency direction is one-way: `src-tauri` → `katto-engine`, `katto-cli` → `katto-engine`. `katto-engine` never depends on `tauri` or any UI concern.
- Engine `lib.rs` re-exports the intended pub surface; internals are `pub(crate)`. If both the CLI and the app need it, it's `pub`; otherwise it isn't. No `pub use module::*` globs.
- `#![warn(missing_docs)]` on `katto-engine`: every pub item gets a one-sentence `///` summary; pub `Result`-returning fns get an `# Errors` section. Do not doc-comment private items or restate signatures — comments explain why, invariants, and units.

## Time

- All timestamps/durations in the engine are `Rational` (`num/den` in the media's timebase). Convert to `f64` seconds only at UI and model (LLM/transcript JSON) boundaries, never mid-pipeline.

## Tests

- Unit tests in-module under `#[cfg(test)] mod tests`; integration tests in `tests/` exercising only the pub API; fixtures in `tests/fixtures/` loaded relative to `CARGO_MANIFEST_DIR`.
- Test names are `<scenario>_<expected>` (module path already names the unit). No `test_` prefix.
- Table-driven cases use `rstest` `#[case]`; round-trip/parser invariants use `proptest`. Plain loops only for trivially small tables.
- Slow or hardware-bound tests: `#[ignore = "reason"]`. Network tests stay behind `--features expensive-tests`.
- Snapshot tests use `insta` with explicit snapshot names. Workflow: change → `cargo insta test` → `cargo insta review` → commit the `.snap` files in the same commit as the code, with the reason in the body. Never commit `.snap.new`.
