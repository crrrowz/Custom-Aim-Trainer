# Contributing to AimForge

Thanks for helping. AimForge measures people, so the most important thing a change can do is keep
measurements trustworthy and comparable over time. The rules below exist for that reason.

## Before you start

- Read [README.md](README.md) for the layout, and [AGENTS.md](AGENTS.md) for the project guardrails
  (it is written for AI coding agents, but the rules apply to everyone).
- For anything bigger than a small fix, open an issue first and describe the problem you want to
  solve. It saves work on both sides.
- Windows is required to run the trainer. Analysis and coaching crates build and test anywhere.

## From idea to merged change

1. **Fork** the repository on GitHub (outside contributors) or work in a branch (maintainers).
   Nobody pushes to `main` directly; every change arrives through a pull request.
2. **Branch from the latest `main`**, named after what it does:
   `fix/centre-dot-timeout`, `feat/drill-library-preview`, `docs/worktrees`.

   ```powershell
   git fetch origin
   git switch -c fix/centre-dot-timeout origin/main
   ```

3. **Make the change** following the steps under [Working on a change](#working-on-a-change).
4. **Push and open a pull request into `main`.** The template asks what changed, why, and how you
   checked it. Link the issue (`Closes #123`).
5. **CI runs** (see [Checks before merging](#checks-before-merging)). Fix anything it reports by
   pushing more commits to the same branch; the run starts again by itself.
6. **Review.** A maintainer reviews, asks for changes if needed, and merges once CI is green.
   Pull requests are squash-merged or rebased so `main` stays a readable history.
7. **Clean up:** delete the branch after the merge (GitHub offers a button), and remove its worktree
   if you used one.

## Parallel work with worktrees

A git worktree is a second folder checked out from the same repository on its own branch. It lets
you keep one change building or under review while you work on another, without stashing or
switching branches, and without two changes touching the same files by accident.

```powershell
# A new folder next to the repository, on a new branch from main.
git worktree add ..\aimforge-drill-preview -b feat/drill-library-preview origin/main

# See every worktree and its branch.
git worktree list

# When the pull request is merged: remove the folder, then the branch.
git worktree remove ..\aimforge-drill-preview
git branch -d feat/drill-library-preview
```

Rules that keep this safe:

- One branch per worktree, one task per branch. Two worktrees should not edit the same files.
- Builds share nothing by default, so each worktree compiles from scratch. Point them at one build
  folder to save time and disk: `$env:CARGO_TARGET_DIR = "D:\cargo-target\aimforge"`.
- `git worktree remove` refuses to delete a folder with uncommitted changes. Commit or save them first.
- Run `git worktree prune` if you deleted a worktree folder by hand.

## Working on a change

1. Write the test first when you change behaviour, and see it fail before the fix.
2. Keep one change per commit, with a message that says what changed and why
   (`fix(trainer): ...`, `feat(ui): ...`, `docs: ...`).
3. Run all checks before every commit:

   ```powershell
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   .\scripts\check-boundaries.ps1
   ```

   If you touched the desktop app, also run `cargo clippy --all-targets -- -D warnings` and
   `cargo test --lib` in `apps/desktop/src-tauri`, and `npm run build` in `apps/desktop/ui`.
4. Check UI changes in a running app or in the browser preview (`npm --prefix apps/desktop/ui run dev`),
   in both Arabic and English, and describe what you checked in the pull request.

## Checks before merging

Every pull request into `main` runs [`.github/workflows/ci.yml`](.github/workflows/ci.yml) on
Windows. It has three jobs, and all three must pass before the pull request can be merged:

| Job | What it checks |
| --- | --- |
| Architecture boundaries | `aim-analysis` and `aim-coach` do not depend on Bevy, Tauri, winit, reqwest or rusqlite |
| Rust workspace | `cargo fmt`, `cargo clippy -D warnings` and `cargo test` for every crate and the trainer |
| Desktop app | the UI type-checks and builds, then `cargo fmt`, `clippy` and the backend tests in `apps/desktop/src-tauri` |

When a check fails, open it from the pull request page and read the first error. Most failures are
one of these:

- **Formatting:** run `cargo fmt --all` (and `cargo fmt` inside `apps/desktop/src-tauri`) and push.
- **Clippy:** fix the warning it names; warnings fail the build on purpose.
- **A test:** run the same test locally (`cargo test -p <crate> <test name>`), fix the cause, push.
- **Boundaries:** move the code that needs the outside dependency out of the domain crate.

A newer push cancels the older run, so you only wait for the latest one.

## Releasing

Maintainers only.

1. Raise `version` in `apps/desktop/src-tauri/tauri.conf.json` (and the matching `Cargo.toml` and
   `package.json`), update `docs/release-notes.md`, and merge that through a PR.
2. Tag the merge commit on `main` and push the tag: `git tag v0.2.0` then `git push origin v0.2.0`.
3. The **Release** workflow builds the installer, the portable zip and `SHA256SUMS.txt` and attaches
   them to a draft release. It stops if the tag and the app version differ.
4. Install the draft's files on a clean Windows user, check they start, then publish the draft.

To build the same files locally, run `scripts\package-release.ps1`; they land in `dist\`.

## Rules that protect the data

- **The `.aft` v1 format is frozen.** Never reorder, rename or remove fields or enum variants in
  `crates/aim-telemetry/src/format.rs`. Old recordings must stay readable; a golden fixture test
  enforces this. New data needs a new format version.
- **Keep the domain crates pure.** `aim-analysis` and `aim-coach` must not depend on Bevy, Tauri,
  winit, reqwest or rusqlite. `scripts/check-boundaries.ps1` checks this in CI.
- **Scenarios are data and must stay deterministic.** The same seed must produce the same targets.
  Changing an assessment scenario in `content/` changes what the scores mean; discuss it in an issue
  first and version it.
- **No 3D world in the trainer.** Plain patterned background, targets and a crosshair. The camera stays
  a perspective camera matched to VALORANT and driven by view angles.
- **Measure in degrees, not pixels.** Angles are what match the game across screens and resolutions.

## Interface text

The app is bilingual. Every user-facing string needs both Arabic (the Iraqi dialect used across the
app) and English. Write short, plain sentences that tell the player what to do. Every screen should
handle its empty, loading and error states.

## Reporting bugs

Include your Windows version, DPI and in-game sensitivity, the steps to reproduce, and if possible the
`.aft` recording of the session (`tools/aft-dump` prints a readable summary of one). Recordings contain
only mouse movement, target and timing data from the trainer.

For security issues, see [SECURITY.md](SECURITY.md) instead.

## License

By contributing, you agree that your contributions are licensed under the same terms as the project:
MIT or Apache-2.0, at the user's option.
