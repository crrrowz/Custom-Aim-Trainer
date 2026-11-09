# 07 UI/UX

The product's job on screen: show the player what their hand actually did, why it matters, and what to do next.
Every number comes from the analysis engine. A screen never shows an id, a file path, or an internal metric code.

## Visual system

| Token | Value | Use |
|---|---|---|
| Background / surface / raised | `#0a0c0f` / `#12161b` / `#1a2027` | Page, cards, controls |
| Line | `#242c36` | Card borders, grid lines |
| Text / muted | `#e8edf3` / `#8a97a8` | Values and labels |
| Accent | `#3ddc97` | Primary action, hit, selected state. Used sparingly |
| Warn / bad | `#e5ad45` / `#ff6b6b` | Slowest direction, miss, regression |
| Fonts | Inter, IBM Plex Sans Arabic | Both bundled; numbers are tabular |

In-game overlays use the same palette from `apps/trainer/src/ui_tokens.rs`.

## Screens

| Screen | Answers | Contents |
|---|---|---|
| Dashboard | What should I do now? | Next action, overall score and trend, skill radar, top problems, recent sessions |
| Analysis | What did my hand do? | Session picker, summary stats, four views below |
| Progress | Am I improving? | Skill curves with confidence bands, all-skills list |
| Plan | What do I train today? | 6-day strip, today's drills with purpose and level, why this plan, block verdict |
| Free practice | How do I train my way? | Build a session, drill library |
| Coach | Why, in plain words? | Conversation grounded in the player's numbers |
| Settings | Is it set up right? | Mouse, language, game window, AI source |

### Analysis views

| View | Shows |
|---|---|
| Aim replay | The real crosshair path of each flick, target at the centre, markers for movement start, first stop and click. Plays at 0.1×, 0.25× or 1×. The speed graph below is linked and can be scrubbed. Filter by misses, slowest, biggest overshoot |
| Over the session | Time for every target, rolling median trend (fatigue shows as a rising line) |
| Shot map | Where each shot landed in target radii, and the centre of all shots (bias) |
| Directions | Median speed in each of 8 directions, slowest highlighted |
| Tracking (track scenarios) | Distance from the target over time against the on-target band |

### Free practice

Build a session: five target types, a live preview drawn with the game's own projection so size and distance are true,
difficulty tiers that set all sliders at once, then fine control. Each type keeps its best score and history, and any
past session opens in Analysis. The drill library groups ready-made drills by what they train and shows the player's level.

## In-game flow

Instructions → 3-2-1 → Running → Results. Pause card appears on Esc or focus loss. Results show accuracy, hits and
reaction as three figures. Names come from `scenario_title`; ids and paths never reach the screen.

## States every screen handles

Empty (what will appear here and how to get it), loading, error (the reason in plain language), game not found, game running.

## Not built yet

Share cards.
