# Contributing to AimForge

Thanks for helping. AimForge measures people, so the most important thing a change can do is keep
measurements trustworthy and comparable over time. The rules below exist for that reason.

## Before you start

- Read [README.md](README.md) for the layout, and [AGENTS.md](AGENTS.md) for the project guardrails
  (it is written for AI coding agents, but the rules apply to everyone).
- For anything bigger than a small fix, open an issue first and describe the problem you want to
  solve. It saves work on both sides.
- Windows is required to run the trainer. Analysis and coaching crates build and test anywhere.

## Workflow

1. Branch from `main`. Do not commit to `main` directly.
2. Write the test first when you change behaviour, and see it fail before the fix.
3. Keep one change per commit, with a message that says what changed and why
   (`fix(trainer): ...`, `feat(ui): ...`, `docs: ...`).
4. Run all checks before every commit:

   ```powershell
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   .\scripts\check-boundaries.ps1
   ```

   If you touched the desktop app, also run `cargo clippy --all-targets -- -D warnings` and
   `cargo test --lib` in `apps/desktop/src-tauri`, and `npm run build` in `apps/desktop/ui`.
5. Check UI changes in a running app or in the browser preview (`npm --prefix apps/desktop/ui run dev`),
   in both Arabic and English, and describe what you checked in the pull request.

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
