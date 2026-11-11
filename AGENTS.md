# AGENTS.md

Rules for every AI coding agent working in this repository (Claude Code, Antigravity/Gemini, Codex, ...).
`CLAUDE.md` and `GEMINI.md` only import this file. Read it in full before any task.

## 0. How work is organised here

- **Two roles.** A *reviewer* (usually Claude) audits, plans and writes task prompts; an *executor*
  (another AI) implements one prompt at a time. The reviewer re-runs the checks and reads the diff before
  anything is merged. Never trust a report, including your own: show results from a fresh run.
- **Backlog:** [`docs/BACKLOG.md`](docs/BACKLOG.md) is the single list of known problems and next tasks,
  each with an ID (`B3`, `M1`, ...). A task prompt names its ID; the PR that closes it updates the list.
- **Prompt shape:** [`docs/agents/PROMPT_TEMPLATE.md`](docs/agents/PROMPT_TEMPLATE.md). A prompt that does
  not name its allowed files, its tests and its acceptance checks is not ready to run.
- **Language:** reply to the user in Iraqi Arabic; code, comments, commits, PRs and docs in English.

## 1. Project map

| Path | What it is | Notes |
| --- | --- | --- |
| `apps/trainer` | Bevy game window: targets, crosshair, raw input, writes `.aft` | Windows only (Win32 raw input) |
| `apps/desktop/src-tauri` | Tauri backend, `Engine` in `src/core.rs` | **Separate Cargo workspace**: run its commands inside that folder |
| `apps/desktop/ui` | React + TypeScript UI | Outside Tauri it runs on `src/mock.ts` sample data |
| `crates/aim-core` | units, camera, sensitivity | shared types: reviewer-owned |
| `crates/aim-telemetry` | `.aft` format | **frozen v1**, see guardrails |
| `crates/aim-scenarios` | scenario schema and seeded generators | |
| `crates/aim-analysis`, `crates/aim-coach` | pure domain logic | no I/O, see guardrails |
| `crates/aim-store`, `crates/aim-llm` | SQLite, optional AI reports | |
| `content/` | scenarios, batteries, drills, thresholds (RON) | changing a test means bumping its `version` |
| `docs/` | user guide, architecture, backlog, `platform/` (deep design, Arabic) | map in `docs/README.md` |
| `scripts/` | `run-desktop.ps1`, `package-release.ps1`, `check-boundaries.ps1`, `screenshots.mjs` | |

Architecture and data flow: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## 2. Workflow (every task)

1. **Start:** `git status --short`, `git branch --show-current`, `git worktree list`. Never work on `main`
   and never in a checkout with changes you did not make.
2. **Branch:** one task, one branch from fresh `main`: `fix/<id>-<slug>`, `feat/<id>-<slug>`,
   `docs/<slug>`, `chore/<slug>`. Parallel tasks use worktrees:
   `git worktree add ../Custom-Aim-Trainer-wt/<slug> -b <branch> main`. Two agents never edit the same
   files at the same time.
3. **Locate with Trail/Graft** (section 4) before opening source. Read only the spans you need.
4. **Test first.** Write the test, run it, and keep the failure lines: it must fail on the current code
   for the reason it names. Then the smallest change that makes it pass. No unrelated refactors.
5. **Gates** (section 3) green before every commit. **One change per commit**; the body names the test
   and pastes the failure excerpt. Write the message to a file and commit with `git commit -F <file>`,
   saved as UTF-8 **without** a BOM (PowerShell: `[IO.File]::WriteAllText($path, $msg)`); never pass it
   through PowerShell `-m "..."`, where a backtick is an escape (`` `f `` becomes a form feed).
6. **Finish:** push the branch and open a PR to `main` only when the prompt says so. Never push to
   `main`, never `--force`, `rebase`, `amend`, delete branches or tag unless the user asked.
7. **Report** in the format of section 7.

## 3. Gates (the same checks CI runs)

From the repo root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude trainer
cargo test -p trainer
.\scripts\check-boundaries.ps1
```

Inside `apps/desktop/src-tauri`: `cargo clippy --all-targets -- -D warnings` and `cargo test --lib`.
Inside `apps/desktop/ui`: `npm ci` once, then `npm run build` (type-check and build).

- Test `trainer` on its own, as CI does: building it together with the other crates
  (`cargo test --workspace`) fails with "crate `wgpu` required to be available in rlib format", and the
  build folder it leaves behind also breaks a later `cargo test -p trainer`. If that happens, use a fresh
  `CARGO_TARGET_DIR`.
- Report test counts by summing the `test result:` lines of this run (`passed` per suite), not from memory.
- While iterating run the narrowest test (`cargo test -p aim-coach <name>`); run everything before a commit.
- Timing-sensitive tests (threads, input, frame timing): run 10 times and report `0/10 failures`.
- CI (`.github/workflows/ci.yml`) uses `--profile ci` and runs the portable crates on Linux; same checks,
  only faster. A PR is mergeable only with all four checks green.
- Set `CARGO_TARGET_DIR` to a shared folder if you build in several worktrees; never commit `target/`.

## 4. Code navigation: Trail/Graft first, always

The repo is indexed by **Trail-code** (the Graft code graph) in `.trail/cache/` (git-ignored); `trail` and `graft` are the same tool. If `graft`
is not on PATH on this machine, add `D:\nodejs\npm-global`.

1. **Start of every task:** `trail map`, then read `.trail/learnings/` for anything about your task.
2. **Before grepping or opening source:** `trail ask "<task>" --source`, or `trail skeleton <file>`.
3. **Before changing a function's signature or behaviour:** `trail callers <symbol> --depth 2`.
4. **After changing code:** `trail build`. **Before every commit:** `trail check`.
5. When a task took real digging, or the user corrected how something is done here, save a learning
   with `trail note` and commit it with the change.

### Known limits (Rust): don't trust Graft blindly
Rust is parsed at Graft's "broad" tier: symbols are complete, but call edges are
**name-resolved**, not type-resolved. Trait method calls, Bevy system registration
(`add_systems(...)`), generics, and macros can be missing from `callers`.
- An empty `callers` result for Rust does **not** prove nothing calls it. Confirm with a
  targeted `grep` for the symbol name before deleting or changing a public API.
- `grep` is fine for things Graft does not index: docs (`docs/`), RON scenarios
  (`content/`), Cargo features, string literals, comments.

## 5. Guardrails (stop and ask instead of crossing these)

- **Pure domain crates.** `aim-analysis` and `aim-coach` never depend on bevy, tauri, winit, reqwest or
  rusqlite. `.\scripts\check-boundaries.ps1` enforces it.
- **Frozen `.aft` v1.** Never reorder, rename or remove fields or variants in
  `crates/aim-telemetry/src/format.rs`; the golden v1 fixture test enforces it. New data needs a new
  version and a decision from the user.
- **Comparable measurements.** Changing a scenario's targets, timing or scoring bumps its `version` in
  `content/`. Analysis thresholds change only with a test that shows why.
- **No 3D world** in the trainer: plain patterned background, targets and crosshair only. The camera stays
  a VALORANT-matched perspective camera driven by `ViewAngles`.
- **Shared types** (`aim-core`, the `View` and `Settings` structs in `apps/desktop/src-tauri/src/core.rs`
  and their TypeScript copies in `apps/desktop/ui/src/api.ts`) change only when the prompt says so, and
  both sides change in the same commit.
- **No new dependencies** unless the prompt names them.
- **Data:** tests use temporary folders and databases; nothing reads or writes the real app data
  (`%APPDATA%\com.aimforge.desktop`). Recordings (`*.aft`), databases, keys and `.env` are never committed.
- **Removing or renaming** a feature, setting, command or public API needs the user's yes.

## 6. UI rules

- Every visible string is bilingual: `T("Arabic", "English")` in components, `[ar, en]` pairs in
  `src/i18n.ts`. Arabic is short, plain and Iraqi-friendly; English is plain and direct. Both directions
  must work (RTL and LTR); charts are drawn with `direction="ltr"`.
- The UI talks to the backend only through `call()` in `src/api.ts`. Every new command also gets a branch
  in `src/mock.ts`, so the screen works in a browser on sample data.
- Every screen handles its states: empty, busy, error and normal.
- Charts draw at their measured width (`useWidth` in `src/charts.tsx`) so text stays 12px; never scale a
  fixed viewBox down.
- UI work gets its own commit and is checked visually: `npm --prefix apps/desktop/ui run dev`, open the
  page, and attach screenshots to the report (`scripts/screenshots.mjs` captures every page).
- Trainer overlays (HUD, pause menu) are 2D and use `apps/trainer/src/ui_tokens.rs`.

## 7. Report format (end of every task)

1. Branch and commits: `git log --oneline main..HEAD`.
2. Per change: test name, failure excerpt before the fix, one-line fix, files (`path:line`).
3. Gate results from this run (pass or fail per gate, test counts); repeat counts for timing tests.
4. Screenshots for UI changes.
5. Deviations from the prompt and anything you stopped on, with the reason.
6. What was **not** verified (the real game, other hardware, a real AI provider, ...).

## Trail — repo context graph

This repo is indexed in `.trail/cache/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `trail map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `trail ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `trail grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `trail skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `trail callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `.trail/cache/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `trail ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `trail build` (deterministic,
no API key, $0).
<!-- trail:end -->
