# hyper-frames reference mirror (read-only)

Code-and-docs snapshot of the owner's `hyper-frames` repo (`~/Projects/WebDev/hyper-frames`), copied 2026-07-02 for the Studio OS design handoff (`docs/superpowers/specs/2026-07-02-studio-os-design.md`). Media, databases, node_modules, and virtualenvs are stripped — this mirror is for **reading patterns, schemas, and prompts**, not for running.

Why it exists: katto's Studio OS design reuses judgment and plumbing that already live in these skills and tools. When planning or implementing a katto module, read the corresponding source here instead of reinventing it.

What matters most for katto:

- `.claude/agents/audio-cut-decider.md` — the cut-planning agent this repo's `agents/cut-decider.md` derives from; canonical cuts.json judgment.
- `.claude/skills/transcribe-and-plan-cuts/` — the transcribe → cut-plan orchestration flow (ElevenLabs Scribe v2 usage, hand-offs).
- `.claude/skills/studio-ideas/` — the idea-curation intelligence for the nightly scheduled job (keep/discard + rationale, never a grade).
- `tools/studio/` — the Hono+React+SQLite production board katto's planner supersedes; schema and promote-flow reference, plus the visual-language spec pointer.
- `tools/cut-video/`, `tools/derive-streams/`, `tools/cut-snap/` — deterministic ffmpeg segment/argv math patterns (pinned flags, no RNG/clock).
- `tools/audio-editor/` — the existing cuts.json refinement UI; UX reference for katto's editor review surface.
- `.claude/skills/thumbnail-and-title-generator/`, `shorts-creator/`, `video-publish-qa/`, `script-writer/` — the downstream skills the Claude Dock will invoke per-project.
- `CLAUDE.md` — the source repo's project notes (pipeline map, hard rules).
- `docs/creator-patterns.md` — the channel's packaging/retention reference the publishing skills are grounded in.

Do not edit files here; edit the originals in the source repo. Refresh by re-running the rsync in the handoff session if the originals change materially.
