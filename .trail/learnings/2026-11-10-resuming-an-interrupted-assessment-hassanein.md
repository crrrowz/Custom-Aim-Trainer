---
title: Resuming an interrupted assessment
author: Hassanein
date: 2026-11-10
branch: integration/m1
---

## Decided
- The trainer takes `--battery-start N` and names files by battery stage (`N_scenario.aft`), so a resumed run fills the same folder.
- The desktop resumes the latest incomplete AssessmentRecord within 2 days (RESUME_DAYS), reusing its folder and form. Finished stages = leading stage indices with a non-ABORTED file.
- A resumed run that adds no file and no finished stage counts as "stopped", not a new incomplete record.
- An incomplete record with skills is shown as `view.estimate` (Home labels it as an estimate) until a complete one exists.

## Watch out
- Screen limits live in aim-scenarios (ON_SCREEN_H_DEG 48, ON_SCREEN_V_DEG 33) and aim-coach re-exports them. Custom setups, drills and every assessment scenario stay inside them; tests/on_screen.rs checks all assessment targets.
- flick_static (max 32) and multi_switch (bins 10/20/30, pitch 32) are scenario version 2. Recordings of version 1 used 40 and 35 degrees, so a retest against a version 1 baseline is not a like-for-like comparison.
- The trainer HUD and feedback line sit in the top-left corner because targets never reach the corners.
