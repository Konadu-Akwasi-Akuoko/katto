# Contributing to katto

Thanks for your interest in contributing. This document covers how to get set up, the conventions we follow, and the process for proposing changes.

By participating in this project you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- **Report bugs** — open an issue using the bug report template.
- **Propose features** — open an issue using the feature request template. Big changes are easier to land if discussed in an issue first.
- **Improve docs** — typos, clearer explanations, or missing setup steps are always welcome.
- **Submit code** — pick up an open issue, especially ones tagged `good first issue`.

## Development setup

### Prerequisites

- Rust (stable, latest)
- [Bun](https://bun.sh/)
- `ffmpeg` and `ffprobe` on `PATH`
- Platform Tauri prerequisites: see https://v2.tauri.app/start/prerequisites/

### Install

```bash
git clone https://github.com/Konadu-Akwasi-Akuoko/katto.git
cd katto
bun install
```

### Run the app

```bash
bun run tauri dev
```

### Run checks

```bash
# Rust
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Frontend
cd ..
bun run tsc --noEmit
bun run build
```

All of the above run in CI on every PR. Run them locally before pushing.

## Code style

- **Rust:** `rustfmt` defaults, `clippy` clean. Public APIs get explicit types and doc comments. Internal locals match surrounding style.
- **TypeScript:** strict mode, no `any` (prefer `unknown`), no `var`. Functional React components. Match existing file style.
- **Comments:** for the *why*, not the *what*. No emojis in code or commit messages.
- **Imports:** group by stdlib / third-party / local; let the formatter handle ordering.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(engine): add ffmpeg audio extraction
fix(app): correct frame-accurate seek on cut boundary
docs(readme): clarify Claude Code detection
```

Common types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `build`, `ci`.

Keep commits atomic — one logical change per commit.

## Pull request process

1. Fork the repo and create a topic branch off `main`: `git checkout -b feat/short-description`.
2. Make your changes with tests.
3. Run all checks locally (see above).
4. Push and open a PR against `main` using the PR template.
5. CI must be green. A maintainer will review; address feedback by pushing more commits (don't force-push during review).
6. Once approved, a maintainer will merge — usually as a squash merge.

## Reporting security issues

Please do **not** open public issues for security vulnerabilities. See [SECURITY.md](SECURITY.md) for the private reporting process.

## Questions

Open a [Discussion](https://github.com/Konadu-Akwasi-Akuoko/katto/discussions) or a low-priority issue. We don't have a chat channel yet.
