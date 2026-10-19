# AGENTS.md

Rules for every AI coding agent working in this repository.

## 1. Code navigation: Trail/Graft first, always

This repo is indexed by **Trail-code** (the Graft code graph): every file, symbol and
call edge, in `.trail/cache/` (git-ignored, regenerable). Using it is mandatory for
every task. The full command reference is the "Trail — repo context graph" section
at the end of this file. In short:

1. **Start of every task:** `trail map` (orientation), then read `.trail/learnings/`
   for anything about your task.
2. **Before grepping or opening source:** `trail ask "<task>" --source`, or
   `trail skeleton <file>` for a file's API. Open source only at the exact
   file:line spans it returns.
3. **Before changing a function's signature or behavior:**
   `trail callers <symbol> --depth 2` to see the blast radius.
4. **After changing code:** `trail build` (a few seconds, no API key).
5. **Before every commit:** `trail check`. When a task took real digging, or the user
   corrected how something is done here, save it with `trail note` and commit it
   with the change.

Antigravity also has these as MCP tools (`trail` server); prefer the tools when available.

### Known limits (Rust): don't trust Graft blindly
Rust is parsed at Graft's "broad" tier: symbols are complete, but call edges are
**name-resolved**, not type-resolved. Trait method calls, Bevy system registration
(`add_systems(...)`), generics, and macros can be missing from `callers`.
- An empty `callers` result for Rust does **not** prove nothing calls it. Confirm with a
  targeted `grep` for the symbol name before deleting or changing a public API.
- `grep` is fine for things Graft does not index: docs (`docs/`), RON scenarios
  (`content/`), Cargo features, string literals, comments.

## 2. Project guardrails

- Domain crates (`aim-analysis`, `aim-coach`) must never depend on bevy, tauri, winit,
  reqwest, or rusqlite. Check with `.\scripts\check-boundaries.ps1`.
- `.aft` v1 is postcard-encoded: never reorder, rename, or remove struct fields or enum
  variants in `crates/aim-telemetry/src/format.rs`. The golden v1 fixture test enforces this.
- No 3D world in the trainer: plain patterned background + targets + crosshair only.
  The camera stays a VALORANT-matched perspective camera driven by `ViewAngles`.
- Before every commit: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `.\scripts\check-boundaries.ps1`.

<!-- trail:start -->
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
