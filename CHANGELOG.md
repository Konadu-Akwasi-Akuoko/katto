# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] — 2026-05-02

### Changed
- Pin Rust toolchain to stable via `rust-toolchain.toml` (with `rustfmt` and `clippy`).
- Sync `Cargo.lock` to workspace v0.2.0.
- Mark completed workspace-conversion items in `TODO.md`.

## [0.2.0] — 2026-05-02

### Added
- Convert repository into a Cargo workspace with `katto-engine`, `katto-cli`, and `src-tauri` members.
- Initial repository scaffold: Tauri 2 + React + TypeScript + Vite.
- Project documentation: README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY.
- GitHub issue and pull request templates.
- Continuous integration workflow.
- Design specification at `docs/superpowers/specs/app_design_rough_cut.md`.

[Unreleased]: https://github.com/Konadu-Akwasi-Akuoko/katto/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/Konadu-Akwasi-Akuoko/katto/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/Konadu-Akwasi-Akuoko/katto/releases/tag/v0.2.0
