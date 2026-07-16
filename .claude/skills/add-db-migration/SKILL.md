---
name: add-db-migration
description: Use when changing the SQLite schema — adding a table, column, or index, or altering how existing data is stored in katto's app database.
---

# Add a DB migration

Migrations are numbered, embedded, and forward-only. Shipped migrations are immutable.

## Checklist (todo per item)

1. **New migration, never an edit.** Append the next `M::up(...)` to the migrations list in `src-tauri/src/db/migrations.rs`. Never modify an existing entry — even a typo ships as a new migration.
2. **Column additions are a plain `ALTER`.** `ALTER TABLE <t> ADD COLUMN <c> …;` in the `.sql` file — no presence guard. `rusqlite_migration` tracks `user_version` and applies each entry exactly once, so a guard protects against nothing. It is also not expressible: `PRAGMA table_info` returns rows and plain SQLite cannot branch on them, so a guard would force `M::up_with_hook` and Rust-side logic for no gain. Follow `002_last_touched.sql` / `003_priority.sql`. (hyper-frames' `ensureColumns` is the right pattern *there* — it has no migration ladder, so nothing else guarantees once-only. Do not port it here.)
3. **Validate test.** The mandatory migrations test (`migrations_apply_on_fresh_memory_db`) must still pass — it runs the full ladder on `Connection::open_in_memory()`.
4. **Repository layer.** Add/adjust the query functions in `src-tauri/src/db/<aggregate>.rs` (functions take `&Connection`, return typed rows). No SQL anywhere else.
5. **Tests.** New/changed repository functions get in-memory DB tests via the shared `test_db()` helper (all migrations applied, production pragmas minus WAL).
6. **Bindings ripple.** If row types cross IPC, follow the add-tauri-command skill from step 5 (regenerate specta bindings, update wrappers).
7. **Gate.** `just check`.

## Hard rules

- `ideas` and `raw_signal` keep column parity with `hyper-frames/tools/studio/server/lib/db.ts` — external tools (`studio-discover`, the curation pattern) write to them directly. Renaming their columns is a breaking change to tools we don't own here.
- No score/rank/grade columns on `ideas` — AI suggests, the human decides. If a change adds a numeric judgment column, it's wrong by design.
- `events` is append-only: no UPDATE or DELETE paths on it (pruning by age is the one exception and lives in a single housekeeping function).
