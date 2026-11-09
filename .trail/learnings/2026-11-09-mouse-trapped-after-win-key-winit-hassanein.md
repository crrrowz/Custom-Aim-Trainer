---
title: "Mouse trapped after Win key: winit leaves the clip when unfocused"
author: Hassanein
date: 2026-11-09
branch: integration/m1
touches:
  - apps/trainer/src/main.rs
  - apps/trainer/src/raw_input.rs
  - crates/aim-scenarios/src/schema.rs
  - crates/aim-scenarios/tests/hit_rules.rs
---

## Decided
On focus loss the trainer calls ClipCursor(NULL) itself (raw_input::release_cursor_clip).

## Tried and ruled out
Setting CursorOptions.grab_mode = None is not enough: winit 0.30 only touches the clip while its window is focused, so after focus is gone the 1x1 clip stays.

## Watch out
Check with GetClipCursor after a real Win key press. mouse_event injection did not move the cursor in the measuring script, so a cursor delta of 0 proves nothing.
