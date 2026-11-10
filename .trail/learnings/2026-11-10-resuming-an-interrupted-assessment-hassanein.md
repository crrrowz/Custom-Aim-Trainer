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
- Custom and drill targets are capped to the visible screen (aim-coach ON_SCREEN_H_DEG 48, ON_SCREEN_V_DEG 33). Assessment scenarios in content/ still use 40/35 degrees and can place targets off screen; changing them changes the norms, so ask first.
