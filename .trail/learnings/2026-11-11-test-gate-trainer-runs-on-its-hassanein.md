---
title: "Test gate: trainer runs on its own; commit messages via file"
author: Hassanein
date: 2026-11-11
branch: docs/agents-test-gate
touches:
  - AGENTS.md
  - CONTRIBUTING.md
  - README.md
  - ".github/workflows/ci.yml"
---

## Decided
- The test gate is `cargo test --workspace --exclude trainer` then `cargo test -p trainer`, exactly as `.github/workflows/ci.yml` does. AGENTS.md, CONTRIBUTING.md and README.md say so.
- Commit messages are written to a UTF-8 file without BOM and committed with `git commit -F`.

## Tried and ruled out
- `cargo test --workspace` (with trainer): fails on Windows with "crate `wgpu` / `bevy_pbr` required to be available in rlib format", even in an empty target folder. Not caused by any PR.

## Watch out
- After a failed `--workspace` build, the same CARGO_TARGET_DIR also breaks `cargo test -p trainer`; use a fresh target dir (CI-like: `--profile ci`).
- PowerShell `-m "..."` eats backtick escapes (`f -> form feed) and Out-File adds a BOM; PR #5's branch commits showed both.
- Correct totals at 0e63ee1: 122 (workspace without trainer) + 77 (trainer) + 31 (src-tauri lib) = 230.
