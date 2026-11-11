# Backlog

The single list of known problems and next tasks. Each item has an ID that task prompts and PRs refer to
(`Closes B3`). When a PR fixes an item, it moves the line to **Done** with the PR number.

Status after the v0.1.0 alpha (2026-11-11): the full loop works (assessment, plan, retest, free practice,
analysis, progress, coach reports, installer). CI runs four checks on every PR. 226 Rust tests; the UI has
type-checking only.

Priority: **P1** wrong for users now · **P2** quality, safety or maintainability · **P3** later.
Owner: **E** an executor prompt can do it · **U** needs the user (a decision, hardware or the real game).

## Bugs and user-facing problems

| ID | P | Owner | Problem | Evidence | Done when |
| --- | --- | --- | --- | --- | --- |
| B2 | P1 | E | The trainer run on its own writes `session-<id>.aft` into the current folder; the repo root holds 17 such files. | `apps/trainer/src/main.rs:1267`, `:1304` | Without `--output` the file goes to `recordings/` under the current folder (created if missing, git-ignored) and the path is printed; test covers the default path |
| B3 | P2 | E | `SHA256SUMS.txt` has Windows line endings, so `sha256sum -c` fails on Linux and macOS. | `scripts/package-release.ps1:56` | File written with `\n` endings; checked with `sha256sum -c` in the release workflow |
| B4 | P2 | E | Release drafts are not marked as pre-release, and the version lives in four files that must be bumped by hand. | `.github/workflows/release.yml:54`; `Cargo.toml`, `apps/desktop/src-tauri/Cargo.toml`, `apps/desktop/ui/package.json`, `apps/desktop/src-tauri/tauri.conf.json` | `0.x` and `-alpha/-beta` tags create pre-releases; `scripts/bump-version.ps1 <x.y.z>` updates all four; CI fails if they differ |
| B5 | P2 | E | `run-desktop.ps1` defaults the build folder to `D:\cargo-target\aimforge`, which fails on machines without a `D:` drive. | `scripts/run-desktop.ps1:6` | Default is `<repo>\target`; `CARGO_TARGET_DIR` still wins |
| B6 | P2 | E | The desktop app has no Content Security Policy. | `apps/desktop/src-tauri/tauri.conf.json:14` (`"csp": null`) | A strict CSP (self only, no remote scripts); app and AI reports still work |
| B7 | P2 | E | Trainer panics if its raw-input window thread fails to start, instead of showing an error. | `apps/trainer/src/raw_input.rs:206`, `:228` | Error returned and shown; no `unwrap` on the channel |
| B8 | P3 | E | Two `unwrap()` calls on the battery sequencer in the game loop. | `apps/trainer/src/main.rs:2059-2061` | Handled without panicking; behaviour unchanged |
| B9 | P2 | E | AI provider keys are stored unencrypted in the SQLite database. | `apps/desktop/src-tauri/src/core.rs:1053` (`set_ai_key`) | Keys in Windows Credential Manager; old keys migrated once and removed from the database |

## Maintainability

| ID | P | Owner | Problem | Evidence | Done when |
| --- | --- | --- | --- | --- | --- |
| M1 | P2 | E | Three very large files make every change expensive and conflict-prone. | `apps/desktop/ui/src/App.tsx` 1937 lines, `apps/desktop/src-tauri/src/core.rs` 2278, `apps/trainer/src/main.rs` 2251 | Split by responsibility (UI: one file per page under `src/pages/`; backend: settings, sessions, plan, reports modules), no behaviour change, all tests green; one PR per file |
| M2 | P2 | E | The UI has no tests; only type-checking. | `apps/desktop/ui/package.json` | Vitest set up; tests for pure helpers (date bucketing, label stacking, flick notes) and a render test of every page on `mock.ts`; runs in CI |
| M3 | P3 | E/U | Empty placeholders confuse contributors. | `crates/aim-sync`, `crates/aim-wasm` (1 line each), `services/api`, `services/supabase` | Removed, or kept with a README that says what they are for (user decides) |
| M4 | P3 | U | `valorant-mcp-server/` is a separate Python project inside this repo. | README "Other folders" | Moved to its own repo or kept with a reason (user decides) |
| M5 | P2 | E | No dependency or security scanning. | `.github/workflows/` | Dependabot for cargo, npm and GitHub Actions; `cargo deny check` and `npm audit` in CI |
| M6 | P2 | U | `main` is not protected (the free plan allows it only on public repos). | GitHub settings | Repo public; `main` requires the four CI checks and a PR |
| M7 | P3 | E | No changelog. | repo root | `CHANGELOG.md` (Keep a Changelog), updated by every user-facing PR |

## Documentation

| ID | P | Owner | Problem | Done when |
| --- | --- | --- | --- | --- |
| D1 | P2 | U | `docs/platform/06_ROADMAP.md` is from 2026-10-19: it predates M1 and the open-source decision (it plans paid subscriptions and cloud sync). | The user decides the direction; roadmap rewritten from this backlog |
| D2 | P3 | E | `docs/roadmap/` holds superseded plans. | Moved under `docs/archive/` with a note, or removed (user decides) |
| D3 | P2 | E | The diagnosis engine and training loop are documented only in Arabic. | English summaries `docs/DIAGNOSIS.md` and `docs/TRAINING_LOOP.md`, checked against the code |

## Measurement and product (needs the user or the real game)

| ID | P | Owner | Problem | Done when |
| --- | --- | --- | --- | --- |
| P1 | P1 | U | Skill norms are provisional (`norms.v0-provisional`). | Test-retest study with real players; norms v1 |
| P2 | P1 | U | In-game sensitivity and field of view are not yet verified against VALORANT. | Procedure in `docs/platform/VALORANT_SENSITIVITY_VERIFICATION.md` run and recorded |
| P3 | P2 | U | Occasional camera freeze reported, not yet investigated. | Reproduced with a recording, cause found, regression test |
| P4 | P2 | U | Installer and app are not code-signed, so SmartScreen warns. | Signing certificate chosen; signing step in the release workflow |
| P5 | P3 | E | No auto-update. | Tauri updater with signed update manifests on GitHub releases |

## Done

| ID | What | PR |
| --- | --- | --- |
| B1 | New installs start in the Windows UI language, not always Arabic | #? |
| — | Faster CI (ci profile, Linux for portable crates) | #1 |
| — | Installer, portable zip, release workflow, screenshots, user guide, architecture | #2 |
| — | First-run language choice switches the page right away | #3 |
