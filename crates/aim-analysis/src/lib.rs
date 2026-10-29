//! Phase 2 diagnostic and engagement analysis engine.
//!
//! Provides pure analytical routines over decoded .aft telemetry sessions:
//! - Pause extraction and duration math
//! - Quality flags decoding and formatting
//! - High-level session summaries
//! - Fine-grained engagement metrics (reaction time, time to hit, gain ratio, correction counts)

use aim_core::camera::{angular_distance, ViewAngles};
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_telemetry::format::{quality_flags, AftSession, KeyCode};
use serde::{Deserialize, Serialize};

/// Formats the session quality flags bitmask into human-readable pipe-separated names.
pub fn format_quality_flags(flags: u32) -> String {
    if flags == 0 {
        return "CLEAN".to_string();
    }
    let mut names = Vec::new();
    if (flags & quality_flags::FOCUS_LOST) != 0 {
        names.push("FOCUS_LOST");
    }
    if (flags & quality_flags::SETTINGS_CHANGED) != 0 {
        names.push("SETTINGS_CHANGED");
    }
    if (flags & quality_flags::FRAME_DROPS) != 0 {
        names.push("FRAME_DROPS");
    }
    if (flags & quality_flags::ABORTED) != 0 {
        names.push("ABORTED");
    }
    if (flags & quality_flags::SECONDARY_DEVICE_MOTION) != 0 {
        names.push("SECONDARY_DEVICE_MOTION");
    }
    if (flags & quality_flags::POLLING_RATE_ANOMALY) != 0 {
        names.push("POLLING_RATE_ANOMALY");
    }
    if (flags & quality_flags::PAUSED) != 0 {
        names.push("PAUSED");
    }
    if names.is_empty() {
        format!("UNKNOWN({:#x})", flags)
    } else {
        names.join(" | ")
    }
}

/// Extracts paired pause intervals `(start_t_us, end_t_us)` from the session's key stream.
///
/// Pauses are signaled by Escape key (`KeyCode::Unknown(0x1B)`):
/// - `down: true` marks the beginning of a pause interval.
/// - `down: false` marks the end (resumption) of the pause interval.
pub fn extract_pause_intervals(session: &AftSession) -> Vec<(u64, u64)> {
    let mut intervals = Vec::new();
    let mut current_start: Option<u64> = None;

    for record in &session.streams.keys.records {
        if record.key == KeyCode::Unknown(0x1B) {
            if record.down {
                if current_start.is_none() {
                    current_start = Some(record.t_us);
                }
            } else if let Some(start) = current_start.take() {
                if record.t_us >= start {
                    intervals.push((start, record.t_us));
                }
            }
        }
    }

    // If session ended while still paused, cap interval at the latest stream timestamp
    if let Some(start) = current_start {
        let max_t = session
            .streams
            .input
            .t_us
            .last()
            .copied()
            .or_else(|| session.streams.shots.records.last().map(|s| s.t_us))
            .unwrap_or(start);
        intervals.push((start, max_t.max(start)));
    }

    intervals
}

/// Calculates the total pause duration in microseconds overlapping the interval `[t_start, t_end]`.
pub fn pause_duration_between(pauses: &[(u64, u64)], t_start: u64, t_end: u64) -> u64 {
    if t_start >= t_end {
        return 0;
    }
    let mut total_paused: u64 = 0;
    for &(p_start, p_end) in pauses {
        let overlap_start = t_start.max(p_start);
        let overlap_end = t_end.min(p_end);
        if overlap_end > overlap_start {
            total_paused = total_paused.saturating_add(overlap_end - overlap_start);
        }
    }
    total_paused
}

/// Computes active duration in microseconds for a time span `[t_start, t_end]`,
/// strictly subtracting all overlapping pause intervals.
pub fn active_duration_between(pauses: &[(u64, u64)], t_start: u64, t_end: u64) -> u64 {
    let wall = t_end.saturating_sub(t_start);
    let paused = pause_duration_between(pauses, t_start, t_end);
    wall.saturating_sub(paused)
}

/// Comprehensive summary metrics of a recorded telemetry session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub scenario_id: String,
    pub form: Option<u32>,
    pub seed: u64,
    pub dpi: u32,
    pub sens: f64,
    pub est_polling_hz: u32,
    pub quality_flags: u32,
    pub quality_flags_decoded: String,
    pub wall_duration_us: u64,
    pub active_duration_us: u64,
    pub pauses: Vec<(u64, u64)>,
    pub total_shots: usize,
    pub total_hits: usize,
    pub total_misses: usize,
    pub accuracy_pct: f64,
    pub trial_reaction_times_us: Vec<Option<u64>>,
}

/// Computes high-level summary metrics from an `AftSession`.
pub fn compute_session_summary(session: &AftSession) -> SessionSummary {
    let pauses = extract_pause_intervals(session);

    let t_first = session
        .streams
        .input
        .t_us
        .first()
        .copied()
        .or_else(|| {
            session
                .streams
                .targets
                .records
                .first()
                .map(|t| t.spawn_t_us)
        })
        .unwrap_or(0);

    let t_last = session
        .streams
        .input
        .t_us
        .last()
        .copied()
        .or_else(|| session.streams.shots.records.last().map(|s| s.t_us))
        .unwrap_or(t_first);

    let wall_duration_us = t_last.saturating_sub(t_first);
    let active_duration_us = active_duration_between(&pauses, t_first, t_last);

    let total_shots = session.streams.shots.records.len();
    let total_hits = session
        .streams
        .shots
        .records
        .iter()
        .filter(|s| s.hit_target_id.is_some())
        .count();
    let total_misses = total_shots.saturating_sub(total_hits);
    let accuracy_pct = if total_shots > 0 {
        (total_hits as f64 / total_shots as f64) * 100.0
    } else {
        0.0
    };

    // Compute reaction time for each trial target with pause time subtracted
    let mut trial_reaction_times_us = Vec::new();
    let input_t = &session.streams.input.t_us;
    let input_dx = &session.streams.input.dx;
    let input_dy = &session.streams.input.dy;

    for target in &session.streams.targets.records {
        let spawn_t = target.spawn_t_us;
        let despawn_t = target.despawn_t_us;

        // Find first motion sample where dx != 0 or dy != 0 after spawn_t
        let mut first_motion_t: Option<u64> = None;
        for i in 0..input_t.len() {
            let t = input_t[i];
            if t >= spawn_t && t <= despawn_t && (input_dx[i] != 0 || input_dy[i] != 0) {
                first_motion_t = Some(t);
                break;
            }
        }

        if let Some(motion_t) = first_motion_t {
            let rt = active_duration_between(&pauses, spawn_t, motion_t);
            trial_reaction_times_us.push(Some(rt));
        } else {
            trial_reaction_times_us.push(None);
        }
    }

    SessionSummary {
        session_id: session.header.session_id.clone(),
        scenario_id: session.header.scenario_id.clone(),
        form: session.header.form,
        seed: session.header.seed,
        dpi: session.header.mouse.dpi,
        sens: session.header.mouse.sens,
        est_polling_hz: session.header.input.est_polling_hz,
        quality_flags: session.header.quality_flags,
        quality_flags_decoded: format_quality_flags(session.header.quality_flags),
        wall_duration_us,
        active_duration_us,
        pauses,
        total_shots,
        total_hits,
        total_misses,
        accuracy_pct,
        trial_reaction_times_us,
    }
}

/// Detailed per-engagement analytical metrics for a single target presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementMetrics {
    pub target_id: u32,
    pub spawn_t_us: u64,
    pub reaction_time_us: Option<u64>,
    pub time_to_hit_us: Option<u64>,
    pub primary_end_t_us: Option<u64>,
    pub gain_ratio: Option<f64>,
    pub correction_count: usize,
    pub shot_hit: bool,
}

/// Aggregate flick summary metrics across all engagements in a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlickMetrics {
    pub median_reaction_time_ms: Option<f64>,
    pub median_time_to_hit_ms: Option<f64>,
    pub median_gain_ratio: Option<f64>,
    pub mean_corrections: f64,
    pub engagements: Vec<EngagementMetrics>,
}

/// Computes detailed per-target engagement metrics across the session:
/// - Reaction time: from spawn_t to first movement onset, excluding pauses
/// - Time to hit: from spawn_t to successful hit shot, excluding pauses
/// - Flick gain ratio: A_primary / D (projected on target vector)
/// - Correction count: submovement direction reversals after primary flick
pub fn compute_engagement_metrics(session: &AftSession) -> Vec<EngagementMetrics> {
    let pauses = extract_pause_intervals(session);
    let deg_per_count = if session.header.mouse.deg_per_count > 0.0 {
        session.header.mouse.deg_per_count
    } else {
        VALORANT_YAW_DEG_PER_COUNT * session.header.mouse.sens
    };

    let input_t = &session.streams.input.t_us;
    let input_dx = &session.streams.input.dx;
    let input_dy = &session.streams.input.dy;
    let n_input = input_t.len();

    let mut results = Vec::new();

    for target in &session.streams.targets.records {
        let spawn_t = target.spawn_t_us;
        let despawn_t = target.despawn_t_us;

        // 1. Identify successful shot for this target
        let hit_shot = session
            .streams
            .shots
            .records
            .iter()
            .find(|s| s.hit_target_id == Some(target.id) && s.t_us >= spawn_t);

        let time_to_hit_us = hit_shot.map(|s| active_duration_between(&pauses, spawn_t, s.t_us));

        // 2. Locate first movement after spawn
        let mut first_move_idx: Option<usize> = None;
        for i in 0..n_input {
            let t = input_t[i];
            if t >= spawn_t && t <= despawn_t && (input_dx[i] != 0 || input_dy[i] != 0) {
                first_move_idx = Some(i);
                break;
            }
        }

        let reaction_time_us =
            first_move_idx.map(|idx| active_duration_between(&pauses, spawn_t, input_t[idx]));

        // 3. Trajectory analysis: compute crosshair position relative to target
        let target_yaw = target.params.initial_pos[0];
        let target_pitch = target.params.initial_pos[1];
        let target_angles = ViewAngles::new(target_yaw, target_pitch);

        // Find initial crosshair position at spawn_t
        let mut cur_angles = ViewAngles::new(0.0, 0.0);
        let mut last_processed_idx = 0;

        for i in 0..n_input {
            if input_t[i] <= spawn_t {
                cur_angles.apply_counts(input_dx[i], input_dy[i], deg_per_count);
                last_processed_idx = i + 1;
            } else {
                break;
            }
        }
        let start_angles = cur_angles;

        let total_distance_d = angular_distance(start_angles, target_angles);

        // Analyze movement up to shot or despawn
        let end_t = hit_shot.map(|s| s.t_us).unwrap_or(despawn_t);

        // Vector from start to target (dyaw, dpitch)
        let delta_yaw = target_angles.yaw - start_angles.yaw;
        let delta_pitch = target_angles.pitch - start_angles.pitch;
        let target_mag = (delta_yaw * delta_yaw + delta_pitch * delta_pitch).sqrt();

        let mut primary_end_t_us: Option<u64> = None;
        let mut gain_ratio: Option<f64> = None;
        let mut correction_count = 0;

        if let Some(move_idx) = first_move_idx {
            let mut projected_progress = Vec::new();
            let mut sample_times = Vec::new();

            for i in last_processed_idx..n_input {
                let t = input_t[i];
                if t < input_t[move_idx] {
                    cur_angles.apply_counts(input_dx[i], input_dy[i], deg_per_count);
                    continue;
                }
                if t > end_t {
                    break;
                }
                cur_angles.apply_counts(input_dx[i], input_dy[i], deg_per_count);

                let moved_yaw = cur_angles.yaw - start_angles.yaw;
                let moved_pitch = cur_angles.pitch - start_angles.pitch;

                // Projection along target axis
                let proj = if target_mag > 1e-6 {
                    (moved_yaw * delta_yaw + moved_pitch * delta_pitch) / target_mag
                } else {
                    moved_yaw
                };

                projected_progress.push(proj);
                sample_times.push(t);
            }

            // Find primary flick endpoint (first significant peak / reversal along movement axis)
            if !projected_progress.is_empty() && total_distance_d > 0.01 {
                let mut peak_val = projected_progress[0];
                let mut peak_idx = 0;
                let mut reversed = false;

                for (idx, &val) in projected_progress.iter().enumerate() {
                    if val > peak_val {
                        peak_val = val;
                        peak_idx = idx;
                    } else if (peak_val - val) > 0.05 * total_distance_d {
                        // Reversal of at least 5% target distance
                        reversed = true;
                        break;
                    }
                }

                let primary_proj = if reversed {
                    primary_end_t_us = Some(sample_times[peak_idx]);
                    correction_count += 1;
                    peak_val
                } else {
                    // Ballistic movement ended at the end of the motion or plateau
                    primary_end_t_us = sample_times.last().copied();
                    *projected_progress.last().unwrap_or(&peak_val)
                };

                let gain = primary_proj / total_distance_d;
                gain_ratio = Some(gain);

                // Detect any additional direction reversals after peak_idx
                if reversed && peak_idx + 1 < projected_progress.len() {
                    let mut prev_diff = 0.0;
                    for w in projected_progress[peak_idx..].windows(2) {
                        let diff = w[1] - w[0];
                        if diff.abs() > 0.05 * total_distance_d {
                            if prev_diff != 0.0 && (diff > 0.0) != (prev_diff > 0.0) {
                                correction_count += 1;
                            }
                            prev_diff = diff;
                        }
                    }
                }
            }
        }

        results.push(EngagementMetrics {
            target_id: target.id,
            spawn_t_us: target.spawn_t_us,
            reaction_time_us,
            time_to_hit_us,
            primary_end_t_us,
            gain_ratio,
            correction_count,
            shot_hit: hit_shot.is_some(),
        });
    }

    results
}

/// Computes aggregate flick metrics across the session.
pub fn compute_flick_metrics(session: &AftSession) -> FlickMetrics {
    let engagements = compute_engagement_metrics(session);

    let mut rts: Vec<f64> = engagements
        .iter()
        .filter_map(|e| e.reaction_time_us.map(|t| t as f64 / 1_000.0))
        .collect();
    rts.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_reaction_time_ms = if !rts.is_empty() {
        let n = rts.len();
        Some(if n % 2 == 1 {
            rts[n / 2]
        } else {
            (rts[n / 2 - 1] + rts[n / 2]) / 2.0
        })
    } else {
        None
    };

    let mut tths: Vec<f64> = engagements
        .iter()
        .filter_map(|e| e.time_to_hit_us.map(|t| t as f64 / 1_000.0))
        .collect();
    tths.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_time_to_hit_ms = if !tths.is_empty() {
        let n = tths.len();
        Some(if n % 2 == 1 {
            tths[n / 2]
        } else {
            (tths[n / 2 - 1] + tths[n / 2]) / 2.0
        })
    } else {
        None
    };

    let mut gains: Vec<f64> = engagements.iter().filter_map(|e| e.gain_ratio).collect();
    gains.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_gain_ratio = if !gains.is_empty() {
        let n = gains.len();
        Some(if n % 2 == 1 {
            gains[n / 2]
        } else {
            (gains[n / 2 - 1] + gains[n / 2]) / 2.0
        })
    } else {
        None
    };

    let total_corrections: usize = engagements.iter().map(|e| e.correction_count).sum();
    let mean_corrections = if !engagements.is_empty() {
        total_corrections as f64 / engagements.len() as f64
    } else {
        0.0
    };

    FlickMetrics {
        median_reaction_time_ms,
        median_time_to_hit_ms,
        median_gain_ratio,
        mean_corrections,
        engagements,
    }
}
