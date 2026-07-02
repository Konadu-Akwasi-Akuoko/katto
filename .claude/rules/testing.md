# Testing rules (always loaded)

- TDD is the default for logic: engine functions (Rational math, validators, keep-window math, retiming), pure frontend `model/` functions, and `db/` repository modules get a failing test before implementation. Run the single failing test during the loop (`cargo test -p <crate> <name>` / `bunx vitest run <file>`); run the full gate before claiming done.
- The gate is `just check` — it is what CI runs. Never claim work is complete without it passing; paste the tail of its output when reporting.
- DB tests run against `Connection::open_in_memory()` with all migrations applied via a shared `test_db()` helper, same pragmas as production (skip WAL in memory). A mandatory test runs every migration up on a fresh in-memory DB.
- Do not test: pixel styling, Tailwind classes, DOM-tree snapshots, or Rust behavior from JS. Do snapshot-test: emitter output (FCPXML/SRT) via insta golden files.
- Deterministic subprocess layers (ffmpeg filtergraphs, argv builders) are pure functions tested without spawning; the single `spawn` call sites stay thin and untested-by-unit (covered by `#[ignore]`d integration tests and manual hardware checkpoints).
- Fixtures are real files under `tests/fixtures/` (Rust) or `src/test/fixtures/` (TS), named for their scenario (`cuts.overlap.json`), shared across suites rather than re-invented.
