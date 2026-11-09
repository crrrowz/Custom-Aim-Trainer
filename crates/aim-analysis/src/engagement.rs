//! Per-target engagement metrics built on [`crate::segment`].

use crate::segment::{segment, AnalysisThresholds, Trace};
use crate::{active_duration_between, extract_pause_intervals};
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_telemetry::format::AftSession;
use serde::{Deserialize, Serialize};

/// Clicks further than this many target radii away are ordinary misses, not premature clicks.
const PREMATURE_MAX_RADII: f64 = 1.5;
const PREMATURE_MIN_SPEED_DEG_S: f64 = 60.0;
const HESITANT_DWELL_MS: f64 = 150.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementMetrics {
    pub target_id: u32,
    pub spawn_t_us: u64,
    /// First frame presented after the spawn; reaction time is measured from here.
    pub displayed_t_us: u64,
    pub onset_t_us: Option<u64>,
    pub reaction_time_us: Option<u64>,
    /// Reaction below the anticipation threshold: excluded from S01.
    pub anticipated: bool,
    pub time_to_hit_us: Option<u64>,
    /// Shot minus onset, pause time excluded.
    pub movement_time_us: Option<u64>,
    pub shot_t_us: Option<u64>,
    pub primary_end_t_us: Option<u64>,
    pub stable_t_us: Option<u64>,
    pub is_home: bool,
    pub radius_deg: f64,
    pub distance_deg: Option<f64>,
    /// 0..8, counter-clockwise from "right".
    pub octant: Option<u8>,
    pub gain_ratio: Option<f64>,
    pub correction_count: usize,
    pub correction_time_ms: Option<f64>,
    /// Signed endpoint error along the movement axis at the shot; positive = overshoot.
    pub endpoint_error_deg: Option<f64>,
    pub shot_hit: bool,
    pub premature_click: bool,
    pub hesitant_click: bool,
    pub dwell_ms: Option<f64>,
    /// Crosshair pitch when the target appeared (S10).
    pub pitch_at_spawn_deg: f64,
    pub target_pitch_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlickMetrics {
    pub median_reaction_time_ms: Option<f64>,
    pub anticipation_rate: f64,
    pub median_time_to_hit_ms: Option<f64>,
    pub median_gain_ratio: Option<f64>,
    pub mean_corrections: f64,
    pub engagements: Vec<EngagementMetrics>,
}

pub fn deg_per_count(session: &AftSession) -> f64 {
    if session.header.mouse.deg_per_count > 0.0 {
        session.header.mouse.deg_per_count
    } else {
        VALORANT_YAW_DEG_PER_COUNT * session.header.mouse.sens
    }
}

pub fn build_trace(session: &AftSession) -> Trace {
    let targets = &session.streams.targets.records;
    let start = targets.iter().map(|t| t.spawn_t_us).min().unwrap_or(0);
    let end = targets
        .iter()
        .map(|t| t.despawn_t_us)
        .max()
        .unwrap_or(start);
    let input = &session.streams.input;
    Trace::build(
        &input.t_us,
        &input.dx,
        &input.dy,
        deg_per_count(session),
        start,
        end,
    )
}

fn displayed_at(session: &AftSession, spawn_t_us: u64) -> u64 {
    let present = &session.streams.frames.present_t_us;
    let i = present.partition_point(|&p| p < spawn_t_us);
    present.get(i).copied().unwrap_or(spawn_t_us)
}

pub fn compute_engagement_metrics(session: &AftSession) -> Vec<EngagementMetrics> {
    compute_engagement_metrics_with(session, &AnalysisThresholds::default())
}

/// Metrics for every static target. Moving (tracking) targets are analysed by the tracking skills.
pub fn compute_engagement_metrics_with(
    session: &AftSession,
    th: &AnalysisThresholds,
) -> Vec<EngagementMetrics> {
    let pauses = extract_pause_intervals(session);
    let trace = build_trace(session);
    if trace.is_empty() {
        return Vec::new();
    }
    let shots = &session.streams.shots.records;

    let mut out = Vec::new();
    for target in &session.streams.targets.records {
        if target.params.path_spline.is_some() {
            continue;
        }
        let (spawn, despawn) = (target.spawn_t_us, target.despawn_t_us);
        let aim = (target.params.initial_pos[0], target.params.initial_pos[1]);
        let radius = target.params.radius_deg;
        let displayed = displayed_at(session, spawn);

        let hit = shots
            .iter()
            .find(|s| s.hit_target_id == Some(target.id) && s.t_us >= spawn);
        let shot = hit.or_else(|| {
            shots
                .iter()
                .find(|s| s.t_us >= spawn && s.t_us <= despawn && s.hit_target_id.is_none())
        });
        let end_t = shot.map_or(despawn, |s| s.t_us);

        let from = trace.index_at(spawn);
        let to = trace.index_at(end_t);
        let seg = segment(&trace, th, from, to, aim, radius);

        let mut m = EngagementMetrics {
            target_id: target.id,
            spawn_t_us: spawn,
            displayed_t_us: displayed,
            onset_t_us: None,
            reaction_time_us: None,
            anticipated: false,
            time_to_hit_us: hit.map(|s| active_duration_between(&pauses, spawn, s.t_us)),
            movement_time_us: None,
            shot_t_us: shot.map(|s| s.t_us),
            primary_end_t_us: None,
            stable_t_us: None,
            is_home: aim.0.abs() < 0.01 && aim.1.abs() < 0.01,
            radius_deg: radius,
            distance_deg: None,
            octant: None,
            gain_ratio: None,
            correction_count: 0,
            correction_time_ms: None,
            endpoint_error_deg: None,
            shot_hit: hit.is_some(),
            premature_click: false,
            hesitant_click: false,
            dwell_ms: None,
            pitch_at_spawn_deg: trace.pitch[from],
            target_pitch_deg: aim.1,
        };

        if let Some(seg) = seg {
            let onset_t = trace.time_at(seg.onset);
            let end_primary_t = trace.time_at(seg.primary_end);
            let rt = active_duration_between(&pauses, displayed.min(onset_t), onset_t);
            let (ey, ep) = trace.error_to(seg.onset, aim.0, aim.1);
            let d = trace.distance_to(seg.onset, aim.0, aim.1);
            let norm = ey.hypot(ep).max(1e-9);
            let axis = (ey / norm, ep / norm);

            let moved_yaw = trace.yaw[seg.primary_end] - trace.yaw[seg.onset];
            let moved_pitch = trace.pitch[seg.primary_end] - trace.pitch[seg.onset];
            let primary = moved_yaw * axis.0 + moved_pitch * axis.1;

            m.onset_t_us = Some(onset_t);
            m.reaction_time_us = Some(rt);
            m.anticipated = (rt as f64 / 1000.0) < th.anticipation_threshold_ms;
            m.primary_end_t_us = Some(end_primary_t);
            m.stable_t_us = seg.stable.map(|i| trace.time_at(i));
            m.distance_deg = Some(d);
            m.octant = Some(
                ((ep.atan2(ey).to_degrees().rem_euclid(360.0) + 22.5) / 45.0).floor() as u8 % 8,
            );
            m.gain_ratio = (d > 0.01).then_some(primary / d);
            m.correction_count = seg.submovements as usize;
            m.correction_time_ms = m
                .stable_t_us
                .map(|s| active_duration_between(&pauses, end_primary_t.min(s), s) as f64 / 1000.0);

            if let Some(s) = shot {
                let i = trace.index_at(s.t_us);
                let (ex, epx) = trace.error_to(i, aim.0, aim.1);
                m.movement_time_us = Some(active_duration_between(&pauses, onset_t, s.t_us));
                m.endpoint_error_deg = Some(-(ex * axis.0 + epx * axis.1));
                let e = trace.distance_to(i, aim.0, aim.1);
                let inside = e <= radius;
                m.premature_click = !inside
                    && e < PREMATURE_MAX_RADII * radius
                    && trace.speed[i] > PREMATURE_MIN_SPEED_DEG_S;
                m.dwell_ms = m
                    .stable_t_us
                    .filter(|&st| s.t_us >= st)
                    .map(|st| active_duration_between(&pauses, st, s.t_us) as f64 / 1000.0);
                m.hesitant_click = inside && m.dwell_ms.is_some_and(|d| d > HESITANT_DWELL_MS);
            }
        }
        out.push(m);
    }
    out
}

pub(crate) fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

pub fn compute_flick_metrics(session: &AftSession) -> FlickMetrics {
    let engagements = compute_engagement_metrics(session);
    let with_rt = engagements.iter().filter(|e| e.reaction_time_us.is_some());
    let total_rt = with_rt.clone().count();
    let anticipated = with_rt.clone().filter(|e| e.anticipated).count();
    let rts: Vec<f64> = with_rt
        .filter(|e| !e.anticipated)
        .filter_map(|e| e.reaction_time_us.map(|t| t as f64 / 1000.0))
        .collect();
    let tths: Vec<f64> = engagements
        .iter()
        .filter_map(|e| e.time_to_hit_us.map(|t| t as f64 / 1000.0))
        .collect();
    let gains: Vec<f64> = engagements.iter().filter_map(|e| e.gain_ratio).collect();
    let corrections: usize = engagements.iter().map(|e| e.correction_count).sum();

    FlickMetrics {
        median_reaction_time_ms: median(&rts),
        anticipation_rate: if total_rt == 0 {
            0.0
        } else {
            anticipated as f64 / total_rt as f64
        },
        median_time_to_hit_ms: median(&tths),
        median_gain_ratio: median(&gains),
        mean_corrections: if engagements.is_empty() {
            0.0
        } else {
            corrections as f64 / engagements.len() as f64
        },
        engagements,
    }
}
