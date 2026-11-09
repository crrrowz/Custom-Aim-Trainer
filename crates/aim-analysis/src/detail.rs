//! Everything the analysis screen draws for one recording: each flick's real crosshair path,
//! speed over time, where shots landed, and tracking error over the session.

use crate::engagement::{build_trace, compute_engagement_metrics, median};
use crate::{active_duration_between, extract_pause_intervals};
use aim_scenarios::schema::TrackingTrajectory;
use aim_scenarios::trajectory::target_angles_at;
use aim_telemetry::format::AftSession;
use serde::{Deserialize, Serialize};

const PATH_STEP_MS: usize = 4;
const PATH_TAIL_MS: usize = 40;
const TRACK_BIN_MS: usize = 250;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathPoint {
    pub t_ms: f32,
    /// Crosshair position in degrees, with the target at the origin. +x right, +y up.
    pub x: f32,
    pub y: f32,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlickDetail {
    pub id: u32,
    /// Seconds of active play when the target appeared.
    pub t_s: f64,
    pub radius_deg: f64,
    pub distance_deg: Option<f64>,
    pub octant: Option<u8>,
    pub reaction_ms: Option<f64>,
    pub anticipated: bool,
    pub time_to_hit_ms: Option<f64>,
    pub gain: Option<f64>,
    pub corrections: usize,
    pub hit: bool,
    pub premature: bool,
    pub path: Vec<PathPoint>,
    pub onset_ms: Option<f64>,
    pub primary_end_ms: Option<f64>,
    pub stable_ms: Option<f64>,
    pub shot_ms: Option<f64>,
    /// Where the shot landed, target at the origin.
    pub endpoint: Option<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackBin {
    pub t_s: f64,
    pub error_deg: f64,
    pub on_target: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDetail {
    pub scenario_id: String,
    pub duration_s: f64,
    pub targets: usize,
    pub hits: usize,
    pub accuracy: Option<f64>,
    pub median_reaction_ms: Option<f64>,
    pub median_time_to_hit_ms: Option<f64>,
    pub median_gain: Option<f64>,
    pub paused_s: f64,
    pub flicks: Vec<FlickDetail>,
    pub tracking: Vec<TrackBin>,
    pub time_on_target: Option<f64>,
    pub tracking_radius_deg: Option<f64>,
}

fn tracking_bins(session: &AftSession) -> Vec<TrackBin> {
    let trace = build_trace(session);
    let pauses = extract_pause_intervals(session);
    let mut bins = Vec::new();
    if trace.is_empty() {
        return bins;
    }
    for target in &session.streams.targets.records {
        let Some(spline) = &target.params.path_spline else {
            continue;
        };
        let Ok((trajectory, seed)) = ron::from_str::<(TrackingTrajectory, u64)>(spline) else {
            continue;
        };
        let (from, to) = (
            trace.index_at(target.spawn_t_us),
            trace.index_at(target.despawn_t_us),
        );
        let (mut sum, mut on, mut n) = (0.0, 0.0, 0.0);
        for i in from..=to {
            let t = trace.time_at(i);
            if pauses.iter().any(|&(a, b)| t >= a && t < b) {
                continue;
            }
            let rel = active_duration_between(&pauses, target.spawn_t_us, t);
            let angles = target_angles_at(&trajectory, seed, rel);
            let e = trace.distance_to(i, angles.yaw, angles.pitch);
            sum += e;
            on += f64::from(u8::from(e <= target.params.radius_deg));
            n += 1.0;
            if n >= TRACK_BIN_MS as f64 {
                bins.push(TrackBin {
                    t_s: rel as f64 / 1_000_000.0,
                    error_deg: sum / n,
                    on_target: on / n,
                });
                (sum, on, n) = (0.0, 0.0, 0.0);
            }
        }
    }
    bins
}

pub fn session_detail(session: &AftSession) -> SessionDetail {
    let pauses = extract_pause_intervals(session);
    let trace = build_trace(session);
    let session_start = session
        .streams
        .targets
        .records
        .iter()
        .map(|t| t.spawn_t_us)
        .min()
        .unwrap_or(0);
    let engagements = compute_engagement_metrics(session);
    let targets = &session.streams.targets.records;

    let mut flicks = Vec::new();
    for e in engagements.iter().filter(|e| !e.is_home) {
        let Some(target) = targets.iter().find(|t| t.id == e.target_id) else {
            continue;
        };
        let aim = (target.params.initial_pos[0], target.params.initial_pos[1]);
        let end_t = e.shot_t_us.unwrap_or(target.despawn_t_us);
        let from = trace.index_at(e.spawn_t_us);
        let to = (trace.index_at(end_t) + PATH_TAIL_MS).min(trace.len().saturating_sub(1));
        let path = (from..=to)
            .step_by(PATH_STEP_MS)
            .map(|i| {
                let (ey, ep) = trace.error_to(i, aim.0, aim.1);
                PathPoint {
                    t_ms: (i - from) as f32,
                    x: -ey as f32,
                    y: -ep as f32,
                    speed: trace.speed[i] as f32,
                }
            })
            .collect();
        let ms_after_spawn =
            |t: u64| active_duration_between(&pauses, e.spawn_t_us, t) as f64 / 1000.0;
        let endpoint = e.shot_t_us.map(|t| {
            let (ey, ep) = trace.error_to(trace.index_at(t), aim.0, aim.1);
            [-ey, -ep]
        });
        flicks.push(FlickDetail {
            id: e.target_id,
            t_s: active_duration_between(&pauses, session_start, e.spawn_t_us) as f64 / 1e6,
            radius_deg: e.radius_deg,
            distance_deg: e.distance_deg,
            octant: e.octant,
            reaction_ms: e.reaction_time_us.map(|v| v as f64 / 1000.0),
            anticipated: e.anticipated,
            time_to_hit_ms: e.time_to_hit_us.map(|v| v as f64 / 1000.0),
            gain: e.gain_ratio,
            corrections: e.correction_count,
            hit: e.shot_hit,
            premature: e.premature_click,
            path,
            onset_ms: e.onset_t_us.map(ms_after_spawn),
            primary_end_ms: e.primary_end_t_us.map(ms_after_spawn),
            stable_ms: e.stable_t_us.map(ms_after_spawn),
            shot_ms: e.shot_t_us.map(ms_after_spawn),
            endpoint,
        });
    }

    let values = |f: &dyn Fn(&FlickDetail) -> Option<f64>| -> Vec<f64> {
        flicks.iter().filter_map(f).collect()
    };
    let hits = flicks.iter().filter(|f| f.hit).count();
    let tracking = tracking_bins(session);
    let time_on_target = (!tracking.is_empty())
        .then(|| tracking.iter().map(|b| b.on_target).sum::<f64>() / tracking.len() as f64);
    let last = session
        .streams
        .targets
        .records
        .iter()
        .map(|t| t.despawn_t_us)
        .max()
        .unwrap_or(session_start);
    let paused: u64 = pauses.iter().map(|&(a, b)| b.saturating_sub(a)).sum();

    SessionDetail {
        scenario_id: session.header.scenario_id.clone(),
        duration_s: active_duration_between(&pauses, session_start, last) as f64 / 1e6,
        targets: flicks.len(),
        hits,
        accuracy: (!flicks.is_empty()).then(|| hits as f64 / flicks.len() as f64),
        median_reaction_ms: median(&values(&|f| f.reaction_ms.filter(|_| !f.anticipated))),
        median_time_to_hit_ms: median(&values(&|f| f.time_to_hit_ms)),
        median_gain: median(&values(&|f| f.gain)),
        paused_s: paused as f64 / 1e6,
        flicks,
        tracking,
        time_on_target,
        tracking_radius_deg: session
            .streams
            .targets
            .records
            .iter()
            .find(|t| t.params.path_spline.is_some())
            .map(|t| t.params.radius_deg),
    }
}
