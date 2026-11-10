---
title: "Trainer timing: input clock vs session clock, despawn is 0"
author: Hassanein
date: 2026-11-09
branch: integration/m1
touches:
  - apps/desktop/ui/src/App.tsx
  - apps/trainer/src/clock.rs
  - apps/trainer/src/main.rs
  - crates/aim-analysis/src/detail.rs
  - crates/aim-analysis/src/engagement.rs
  - crates/aim-analysis/tests/session_detail.rs
  - apps/trainer/tests/clock_alignment.rs
---

## Decided
Input events carry a timestamp from the raw-input thread start; the session clock is reset when Running begins. Events are rebased with SessionClock::from_input_us.
Trial targets are recorded with despawn_t_us = 0, so analysis uses aim_analysis::engagement::target_end.

## Watch out
Headless tests and synthetic sessions missed both. Decode a real .aft and compare shots against spawns before trusting timing.
