---
title: NVIDIA Driver VSync Cap and WGPU Present Mode Fallback
author: Hassanein
date: 2026-10-19
branch: main
cost: { minutes: 28, tokens: 326331 }
touches:
  - apps/trainer/src/main.rs
  - Cargo.lock
  - apps/trainer/Cargo.toml
  - apps/trainer/src/camera.rs
  - apps/trainer/src/target.rs
  - apps/trainer/src/lib.rs
  - apps/trainer/src/replay.rs
  - apps/trainer/tests/chronological_shots.rs
  - apps/trainer/src/raw_input.rs
  - crates/aim-telemetry/src/format.rs
---

## Decided
