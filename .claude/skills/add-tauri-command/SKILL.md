---
name: add-tauri-command
description: Use when adding, renaming, or changing the signature of a Tauri command in src-tauri — any new IPC surface between the Rust core and the React UI.
---

# Add a Tauri command

End-to-end checklist. A command is not "done" until both sides are typed and tested.

## Checklist (todo per item)

1. **Domain module.** Add the command to `src-tauri/src/commands/<domain>.rs` (create the domain file if new — never a catch-all module). Name: snake_case verb phrase, one command per user-facing action.
2. **Signature.** `async fn` if it does I/O; owned params; `State<'_, AppState>` for state; `Channel<T>` param if it streams. Returns `Result<T, Error>` (the app error enum — add a variant if no existing one fits; wrap engine errors via `#[from]`).
3. **Thin shell.** Body = unwrap args → call engine / `db/` repository / jobs layer → map errors. If you're writing SQL or business logic inside the command, stop and move it down a layer.
4. **Register.** Add to the single `generate_handler![...]` list in `lib.rs` (and to `tauri_specta::Builder` command list). There is exactly one registration site.
5. **Regenerate bindings.** Run the specta export (see `justfile`: `just bindings`) so `src/lib/ipc/bindings.gen.ts` updates. Never hand-edit that file.
6. **Typed wrapper.** Expose it from `src/lib/ipc/<domain>.ts` re-exporting the generated function with any ergonomics (query-key helpers for TanStack Query). Feature code imports the wrapper, never the bindings file or raw `invoke`.
7. **Tests, both sides.** Rust: unit test for the layer the command delegates to (the command itself stays thin enough not to need one); if the command has branching, test it with a test `AppState`. TS: if a hook/component consumes it, mock via `mockIPC` in the colocated test.
8. **Capabilities.** If the command needs a plugin permission, add it to the relevant file in `src-tauri/capabilities/` (per-concern files, minimum scope).
9. **Gate.** `just check`.

## Common mistakes

- Registering in a second `invoke_handler` call — Tauri silently keeps only the last one.
- Returning `String` errors — the frontend loses the `kind` discriminant.
- Streaming via events instead of `Channel<T>` — events are broadcast and unordered per docs; channels are the streaming path.
- Forgetting `clearMocks()` in the TS test — IPC mocks leak across tests.
