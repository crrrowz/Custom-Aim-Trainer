## What and why

<!-- What does this change, and what problem does it solve? Link the issue: Closes #123 -->

## How I checked it

<!-- Tests added or run, and what you saw in the app or the trainer. Screenshots help for UI changes. -->

## Checklist

- [ ] One focused change; commits say what changed and why.
- [ ] Tests added or updated for changed behaviour, and they failed before the fix.
- [ ] `cargo fmt`, `cargo clippy -D warnings`, `cargo test` and `scripts/check-boundaries.ps1` pass locally
      (CI runs the same checks and blocks the merge if one fails).
- [ ] UI text has both Arabic and English.
- [ ] No change to the `.aft` v1 format, and assessment scenarios are versioned if their design changed.
