//! Phase 2 diagnostic and engagement analysis engine.
//!
//! Provides pure analytical routines over decoded .aft telemetry sessions:
//! - Pause extraction and duration math
//! - Quality flags decoding and formatting
//! - High-level session summaries
//! - Fine-grained engagement metrics (reaction time, time to hit, gain ratio, correction counts)

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

pub mod detail;
pub mod engagement;
pub mod segment;
pub mod skills;

pub use detail::{session_detail, SessionDetail};
pub use engagement::{
    compute_engagement_metrics, compute_engagement_metrics_with, compute_flick_metrics,
    EngagementMetrics, FlickMetrics,
};
pub use segment::AnalysisThresholds;
pub use skills::{
    analyze_sessions, time_on_target, Evidence, Reliability, SkillEstimate, SkillId, SkillProfile,
};
