# Security Policy

## Supported versions

`katto` is in early development and has not yet had a stable release. Security fixes are applied to the `main` branch only. Once a `1.0` ships, this policy will be updated with a supported-version table.

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use one of the following private channels:

1. **GitHub Security Advisories** (preferred) — open a private advisory at
   https://github.com/Konadu-Akwasi-Akuoko/katto/security/advisories/new
2. **Email** — send a report to **akwasikonadueverlasting+katto@gmail.com** with the subject line `katto security report`.

Please include:

- A description of the vulnerability and its potential impact.
- Steps to reproduce, ideally with a minimal proof of concept.
- The version, commit SHA, or branch you tested against.
- Your environment (OS, Rust version, Bun version) if relevant.

## What to expect

- **Acknowledgement** within 72 hours.
- **Initial assessment** within 7 days, including whether the issue is accepted as a vulnerability and a rough timeline for a fix.
- **Coordinated disclosure** — we will work with you on a disclosure window, typically 30–90 days depending on severity and complexity, before any public details are released.
- **Credit** — reporters are credited in the release notes and the advisory unless they prefer to remain anonymous.

## Scope

In scope:

- The `katto-engine` library and CLI.
- The `katto` desktop application (Tauri shell, Rust backend, React frontend).
- Build, packaging, and release artifacts published from this repository.

Out of scope:

- Vulnerabilities in third-party dependencies (please report those upstream; we will pick up fixes via dependency updates).
- Issues that require a compromised host machine or attacker-controlled local privileges to exploit.
- Social engineering of maintainers or users.

Thank you for helping keep `katto` and its users safe.
