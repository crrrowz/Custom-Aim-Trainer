# 08 - AimForge Session Execution & Integrity Rules

## 1. Assessment vs. Drill & Free Play

AimForge distinguishes between two primary session archetypes:

### 1.1 Fixed-Time Assessments
- **Strict Protocol**: Assessments (such as the standard `assess.v1` battery A0–A6) measure baseline mechanical competencies (flick accuracy, micro-precision, reaction time, smooth and reactive tracking, target switching).
- **Fixed Duration & Determinism**: Each assessment runs either for an exact calibrated duration (e.g., 45.0s for tracking and cluster scenarios) or until a fixed sequence of pseudo-random pseudo-fitts trials completes with an explicit safety timeout.
- **Data Integrity**: Telemetry is recorded in high-frequency `.aft` format (Raw Input QPC timestamps, mouse counts, frame intervals). The timeline must remain strictly uncorrupted.

### 1.2 Flexible Drills and Free Play
- **Targeted Practice**: Drills isolate specific motor bottlenecks diagnosed in prior assessments (e.g., deceleration overshoot, vertical micro-corrections).
- **Flexible Timing**: Drills can be configured for flexible durations, repetition counts, and difficulty levels ($d \in [0, 1]$).
- **Exploratory Environment**: Free play permits unbound practice without impacting baseline ranking or diagnostic validity.

---

## 2. Pause Policy & Validity Rules

### 2.1 Triggering Pauses
- **Manual Pause**: Pressing `Esc` during an active running session pauses the simulation immediately, unlocks the mouse cursor, and presents the centered Pause overlay card.
- **Automatic Pause on Focus Loss**: When the game window loses OS focus (`WindowFocused(false)`), the session automatically pauses if in the `Running` state. This prevents unrecorded or skewed telemetry while the player is alt-tabbed.

### 2.2 Pause Time Accounting
- **Exclusion from Performance**: All pause intervals are recorded with microsecond QPC precision (`PauseRecord`). The active session clock subtracts paused intervals:
  $$\text{Active Time} = \text{Total Elapsed QPC Time} - \sum \text{Pause Durations}$$
- **Reaction Time (RT) Integrity**: Reaction times for trials active during a pause strictly exclude the pause duration.
- **Validity Thresholds**:
  - An assessment session remains valid if total pause time does not exceed 60 seconds and total pause count does not exceed 3.
  - Exceeding pause thresholds flags the assessment session as compromised in analysis, though the raw `.aft` remains valid and parseable.

---

## 3. Session Flow State Machine

Each scenario execution follows a strict 4-stage lifecycle:

```
[Instructions] ──(Space / Click)──▶ [Countdown 3-2-1] ──(3.0s elapsed)──▶ [Running] ──(Completed / Duration)──▶ [Results]
      │                                    │                                  │
   (Esc)                                (Esc)                              (Esc / Focus Lost)
      ▼                                    ▼                                  ▼
[Exit (No Save)]                     [Exit (No Save)]                      [Pause Menu]
```

### 3.1 Instructions Stage
- **Presentation**: Centered card showing scenario title, 1–2 lines describing the objective, and `"Click or press Space to start"`.
- **Pre-session Boundary**:
  - Session clock, recording streams, and `.aft` timeline **do not start**.
  - Pressing `Esc` exits immediately without saving any telemetry file.
  - Window focus loss during Instructions is ignored (no pause overlay, no error).

### 3.2 Countdown 3-2-1 Stage
- **Visuals**: Large centered digits counting down: `3`, `2`, `1` (1.0 second per tick, 3.0s total).
- **Motor Readying**: Allows the player to center their focus and grip before input recording engages.
- **Pre-session Boundary**:
  - Session clock and recording streams remain dormant.
  - Pressing `Esc` cancels the session immediately without saving.
  - Focus loss is ignored.

### 3.3 Running Stage
- **Active Gameplay**: Target spawning, raw input capture, hit testing, and QPC timeline recording are live.
- **Lifecycle Events**:
  - `Esc` or focus loss transitions to `Paused`.
  - Reaching target/trial completion or scenario `duration_s` transitions to `Results`.

### 3.4 Results Stage
- **Performance Summary**: Displays scenario name, hits, misses, accuracy percentage, median reaction time (excluding pause intervals), active session duration, and the saved `.aft` path.
- **Action**: Pressing `Enter` or `Esc` closes the app (or in battery mode advances to the next scenario).

---

## 4. Familiarization Runs

- **First-Time Assessment Rule**: The very first time a user runs an assessment battery, it is treated as a **Familiarization Run**.
- **Rationale**: Novelty effects, UI adaptation, and initial sensitivity adjustments introduce significant noise into motor baseline statistics.
- **Implementation**: The first completed assessment battery is retained for habituation analysis but baseline diagnostic classification (e.g. skill profiles, weakness detection) is flagged with `familiarization: true` until a confirmed retest is recorded. No `.aft` v1 format changes are required.
