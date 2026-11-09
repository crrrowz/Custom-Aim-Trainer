//! Skill estimates from a set of assessment sessions (03_DIAGNOSIS_ENGINE.md sections 3-4).
//!
use crate::engagement::{build_trace, compute_engagement_metrics_with, median, EngagementMetrics};
use crate::segment::AnalysisThresholds;
use crate::{active_duration_between, extract_pause_intervals};
use aim_scenarios::schema::{TrackingTrajectory, ACCURATE_SPEED_MPS};
use aim_scenarios::trajectory::target_angles_at;
use aim_telemetry::format::AftSession;
use serde::{Deserialize, Serialize};

const BOOTSTRAP_RUNS: usize = 1000;
const REVERSAL_WINDOW_MS: usize = 500;
const FITTS_MIN_PER_CONDITION: usize = 2;
pub const NORMS_VERSION: &str = "norms.v0-provisional";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillId {
    S01Reaction,
    S02FlickSpeed,
    S03FlickPrecision,
    S04GainControl,
    S05MicroCorrection,
    S06TrackSmooth,
    S07TrackReactive,
    S08TargetSwitch,
    S09ClickTiming,
    S10HeadLevel,
    S11DirectionalBalance,
    S13Consistency,
    S12CounterStrafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reliability {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillEstimate {
    pub skill: SkillId,
    pub raw: f64,
    pub score: f64,
    pub ci95: (f64, f64),
    pub n_evidence: u32,
    pub reliability: Reliability,
}

/// Secondary numbers the diagnosis rules and the coach cite as evidence.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub gain_median: Option<f64>,
    pub gain_iqr: Option<f64>,
    pub gain_vs_distance_slope: Option<f64>,
    pub gain_vs_distance_p: Option<f64>,
    pub mean_submovements: Option<f64>,
    pub anticipation_rate: f64,
    pub premature_rate: Option<f64>,
    pub median_dwell_ms: Option<f64>,
    pub hit_rate: Option<f64>,
    pub head_level_bias_deg: Option<f64>,
    /// Weakest direction with enough evidence: (octant, z-score).
    pub weakest_octant: Option<(u8, f64)>,
    pub fatigue_per_trial: Option<f64>,
    pub moving_shot_rate: Option<f64>,
    pub median_stop_to_shot_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillProfile {
    pub norms_version: String,
    pub skills: Vec<SkillEstimate>,
    pub evidence: Evidence,
}

impl SkillProfile {
    pub fn skill(&self, id: SkillId) -> Option<&SkillEstimate> {
        self.skills.iter().find(|s| s.skill == id)
    }
}

/// Provisional norm: raw value at score 0 and at score 100.
#[derive(Clone, Copy)]
struct Anchor {
    worst: f64,
    best: f64,
}

fn score_of(a: Anchor, raw: f64) -> f64 {
    (100.0 * (raw - a.worst) / (a.best - a.worst)).clamp(0.0, 100.0)
}

fn anchor(skill: SkillId) -> Anchor {
    let (worst, best) = match skill {
        SkillId::S01Reaction => (350.0, 150.0),
        SkillId::S02FlickSpeed => (2.0, 8.0),
        SkillId::S03FlickPrecision => (4.0, 0.4),
        // S04 is scored on the absolute distance from the ideal gain of 0.96.
        SkillId::S04GainControl => (0.25, 0.0),
        SkillId::S05MicroCorrection => (600.0, 80.0),
        SkillId::S06TrackSmooth => (0.2, 0.9),
        SkillId::S07TrackReactive => (450.0, 120.0),
        SkillId::S08TargetSwitch => (1300.0, 350.0),
        SkillId::S09ClickTiming => (0.4, 0.0),
        SkillId::S10HeadLevel => (3.0, 0.2),
        SkillId::S11DirectionalBalance => (4.0, 0.0),
        SkillId::S13Consistency => (0.7, 0.15),
        SkillId::S12CounterStrafe => (0.4, 0.0),
    };
    Anchor { worst, best }
}

const IDEAL_GAIN: f64 = 0.96;

fn raw_for_score(skill: SkillId, raw: f64) -> f64 {
    if skill == SkillId::S04GainControl {
        (raw - IDEAL_GAIN).abs()
    } else {
        raw
    }
}

/// Small deterministic generator: the same sessions must always give the same intervals.
struct XorShift(u64);

impl XorShift {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

fn estimate(
    skill: SkillId,
    n: usize,
    statistic: &dyn Fn(&[usize]) -> Option<f64>,
) -> Option<SkillEstimate> {
    if n == 0 {
        return None;
    }
    let all: Vec<usize> = (0..n).collect();
    let raw = statistic(&all)?;
    let a = anchor(skill);
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15 ^ skill as u64);
    let mut scores = Vec::with_capacity(BOOTSTRAP_RUNS);
    let mut sample = vec![0; n];
    for _ in 0..BOOTSTRAP_RUNS {
        for s in sample.iter_mut() {
            *s = rng.below(n);
        }
        if let Some(v) = statistic(&sample) {
            scores.push(score_of(a, raw_for_score(skill, v)));
        }
    }
    if scores.is_empty() {
        return None;
    }
    scores.sort_by(f64::total_cmp);
    let low = scores[(scores.len() as f64 * 0.025) as usize];
    let high = scores[((scores.len() as f64 * 0.975) as usize).min(scores.len() - 1)];
    let width = high - low;
    Some(SkillEstimate {
        skill,
        raw,
        score: score_of(a, raw_for_score(skill, raw)),
        ci95: (low, high),
        n_evidence: n as u32,
        reliability: if width < 10.0 {
            Reliability::High
        } else if width < 20.0 {
            Reliability::Medium
        } else {
            Reliability::Low
        },
    })
}

fn pick(values: &[f64], idx: &[usize]) -> Vec<f64> {
    idx.iter().map(|&i| values[i]).collect()
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

fn quantile(values: &[f64], q: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let pos = q * (v.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    v[lo] + (v[hi] - v[lo]) * (pos - lo as f64)
}

/// Least-squares slope of y on x with a two-sided normal-approximation p-value.
fn slope_with_p(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    let n = x.len();
    if n < 4 {
        return None;
    }
    let (mx, my) = (mean(x), mean(y));
    let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let slope = x
        .iter()
        .zip(y)
        .map(|(a, b)| (a - mx) * (b - my))
        .sum::<f64>()
        / sxx;
    let intercept = my - slope * mx;
    let rss: f64 = x
        .iter()
        .zip(y)
        .map(|(a, b)| (b - intercept - slope * a).powi(2))
        .sum();
    let se = (rss / (n - 2) as f64 / sxx).sqrt();
    if se <= 0.0 {
        return Some((slope, 0.0));
    }
    Some((slope, 2.0 * (1.0 - normal_cdf((slope / se).abs()))))
}

/// Abramowitz & Stegun 26.2.17.
fn normal_cdf(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.231_641_9 * z);
    let poly = t
        * (0.319_381_530
            + t * (-0.356_563_782
                + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    1.0 - (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt() * poly
}

/// Condition key (distance bucket, radius) with its (distance, endpoint error, movement time) rows.
type FittsGroup = ((i64, i64), Vec<(f64, f64, f64)>);

/// ISO 9241-9 throughput, averaged over (distance, size) conditions.
fn fitts_throughput(rows: &[&EngagementMetrics], idx: &[usize]) -> Option<f64> {
    let mut groups: Vec<FittsGroup> = Vec::new();
    for &i in idx {
        let e = rows[i];
        let (Some(d), Some(err), Some(mt)) =
            (e.distance_deg, e.endpoint_error_deg, e.movement_time_us)
        else {
            continue;
        };
        // Starting points vary a little, so conditions are grouped on a 5 degree distance bucket.
        let key = (
            (d / 5.0).round() as i64,
            (e.radius_deg * 100.0).round() as i64,
        );
        let row = (d, err, mt as f64 / 1_000_000.0);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, g)) => g.push(row),
            None => groups.push((key, vec![row])),
        }
    }
    let mut tps = Vec::new();
    for (_, g) in &groups {
        if g.len() < FITTS_MIN_PER_CONDITION {
            continue;
        }
        let errs: Vec<f64> = g.iter().map(|r| r.1).collect();
        let we = (4.133 * sd(&errs)).max(1e-3);
        let d = mean(&g.iter().map(|r| r.0).collect::<Vec<_>>());
        let mt = mean(&g.iter().map(|r| r.2).collect::<Vec<_>>());
        if mt > 0.0 {
            tps.push((d / we + 1.0).log2() / mt);
        }
    }
    (!tps.is_empty()).then(|| mean(&tps))
}

fn is_scenario(session: &AftSession, name: &str) -> bool {
    session.header.scenario_id.contains(name)
}

#[derive(Default)]
struct TrackingResult {
    on_target: Vec<f64>,
    reversal_ms: Vec<f64>,
}

fn tracking_metrics(session: &AftSession) -> TrackingResult {
    let mut result = TrackingResult::default();
    let trace = build_trace(session);
    if trace.is_empty() {
        return result;
    }
    let pauses = extract_pause_intervals(session);
    for target in &session.streams.targets.records {
        let Some(spline) = &target.params.path_spline else {
            continue;
        };
        let Ok((trajectory, seed)) = ron::from_str::<(TrackingTrajectory, u64)>(spline) else {
            continue;
        };
        let from = trace.index_at(target.spawn_t_us);
        let to = trace.index_at(target.despawn_t_us);
        let mut prev_yaw: Option<f64> = None;
        let mut prev_sign = 0.0_f64;
        for i in from..=to {
            let t = trace.time_at(i);
            if pauses.iter().any(|&(a, b)| t >= a && t < b) {
                continue;
            }
            let rel = active_duration_between(&pauses, target.spawn_t_us, t);
            let angles = target_angles_at(&trajectory, seed, rel);
            let e = trace.distance_to(i, angles.yaw, angles.pitch);
            result.on_target.push(if e <= target.params.radius_deg {
                1.0
            } else {
                0.0
            });

            if let Some(p) = prev_yaw {
                let dy = (angles.yaw - p + 180.0).rem_euclid(360.0) - 180.0;
                if dy != 0.0 {
                    let sign = dy.signum();
                    if prev_sign != 0.0 && sign != prev_sign {
                        let last = (i + REVERSAL_WINDOW_MS).min(trace.len() - 1);
                        if let Some(j) = (i..=last).find(|&j| trace.v_yaw[j] * sign > 5.0) {
                            result.reversal_ms.push((j - i) as f64);
                        }
                    }
                    prev_sign = sign;
                }
            }
            prev_yaw = Some(angles.yaw);
        }
    }
    result
}

/// Builds the skill profile from one battery (or any subset of its scenarios).
pub fn analyze_sessions(sessions: &[AftSession], th: &AnalysisThresholds) -> SkillProfile {
    let per_session: Vec<(&AftSession, Vec<EngagementMetrics>)> = sessions
        .iter()
        .filter(|s| !is_scenario(s, "warmup"))
        .map(|s| (s, compute_engagement_metrics_with(s, th)))
        .collect();

    let collect = |names: &[&str]| -> Vec<&EngagementMetrics> {
        per_session
            .iter()
            .filter(|(s, _)| names.iter().any(|n| is_scenario(s, n)))
            .flat_map(|(_, e)| e.iter())
            .filter(|e| !e.is_home)
            .collect()
    };
    let flicks = collect(&["flick_static", "micro_precision"]);
    let peeks = collect(&["head_level"]);
    let all = collect(&[
        "flick_static",
        "micro_precision",
        "head_level",
        "multi_switch",
    ]);

    let mut skills = Vec::new();
    let mut evidence = Evidence::default();

    // S01
    let rt_rows: Vec<&&EngagementMetrics> = flicks
        .iter()
        .chain(peeks.iter())
        .filter(|e| e.reaction_time_us.is_some())
        .collect();
    let rts: Vec<f64> = rt_rows
        .iter()
        .filter(|e| !e.anticipated)
        .map(|e| e.reaction_time_us.unwrap_or(0) as f64 / 1000.0)
        .collect();
    if !rt_rows.is_empty() {
        evidence.anticipation_rate =
            rt_rows.iter().filter(|e| e.anticipated).count() as f64 / rt_rows.len() as f64;
    }
    skills.extend(estimate(SkillId::S01Reaction, rts.len(), &|i| {
        median(&pick(&rts, i))
    }));

    // S02 and S03
    let static_flicks = collect(&["flick_static"]);
    skills.extend(estimate(
        SkillId::S02FlickSpeed,
        static_flicks.len(),
        &|i| fitts_throughput(&static_flicks, i),
    ));
    let errors: Vec<f64> = static_flicks
        .iter()
        .filter_map(|e| e.endpoint_error_deg)
        .collect();
    skills.extend(estimate(SkillId::S03FlickPrecision, errors.len(), &|i| {
        (i.len() >= 2).then(|| 4.133 * sd(&pick(&errors, i)))
    }));

    // S04
    let gain_rows: Vec<(f64, f64)> = static_flicks
        .iter()
        .filter_map(|e| Some((e.distance_deg?, e.gain_ratio?)))
        .collect();
    let gains: Vec<f64> = gain_rows.iter().map(|r| r.1).collect();
    skills.extend(estimate(SkillId::S04GainControl, gains.len(), &|i| {
        median(&pick(&gains, i))
    }));
    if !gains.is_empty() {
        evidence.gain_median = median(&gains);
        evidence.gain_iqr = Some(quantile(&gains, 0.75) - quantile(&gains, 0.25));
        let distances: Vec<f64> = gain_rows.iter().map(|r| r.0).collect();
        if let Some((slope, p)) = slope_with_p(&distances, &gains) {
            evidence.gain_vs_distance_slope = Some(slope);
            evidence.gain_vs_distance_p = Some(p);
        }
    }

    // S05
    let corrections: Vec<f64> = flicks.iter().filter_map(|e| e.correction_time_ms).collect();
    skills.extend(estimate(
        SkillId::S05MicroCorrection,
        corrections.len(),
        &|i| median(&pick(&corrections, i)),
    ));
    if !flicks.is_empty() {
        evidence.mean_submovements = Some(
            flicks
                .iter()
                .map(|e| e.correction_count as f64)
                .sum::<f64>()
                / flicks.len() as f64,
        );
    }

    // S06 and S07
    for (session, _) in per_session
        .iter()
        .filter(|(s, _)| is_scenario(s, "smooth_track"))
    {
        let tr = tracking_metrics(session);
        skills.extend(estimate(
            SkillId::S06TrackSmooth,
            tr.on_target.len(),
            &|i| Some(mean(&pick(&tr.on_target, i))),
        ));
    }
    for (session, _) in per_session
        .iter()
        .filter(|(s, _)| is_scenario(s, "strafe_track"))
    {
        let tr = tracking_metrics(session);
        skills.extend(estimate(
            SkillId::S07TrackReactive,
            tr.reversal_ms.len(),
            &|i| median(&pick(&tr.reversal_ms, i)),
        ));
    }

    // S08: time between consecutive kills in the multi-target test.
    let mut between_kills = Vec::new();
    for (session, _) in per_session
        .iter()
        .filter(|(s, _)| is_scenario(s, "multi_switch"))
    {
        let hits: Vec<u64> = session
            .streams
            .shots
            .records
            .iter()
            .filter(|s| s.hit_target_id.is_some())
            .map(|s| s.t_us)
            .collect();
        let pauses = extract_pause_intervals(session);
        between_kills.extend(
            hits.windows(2)
                .map(|w| active_duration_between(&pauses, w[0], w[1]) as f64 / 1000.0),
        );
    }
    skills.extend(estimate(
        SkillId::S08TargetSwitch,
        between_kills.len(),
        &|i| median(&pick(&between_kills, i)),
    ));

    // S09
    let clicks: Vec<f64> = flicks
        .iter()
        .chain(peeks.iter())
        .filter(|e| e.endpoint_error_deg.is_some())
        .map(|e| if e.premature_click { 1.0 } else { 0.0 })
        .collect();
    skills.extend(estimate(SkillId::S09ClickTiming, clicks.len(), &|i| {
        Some(mean(&pick(&clicks, i)))
    }));
    if !clicks.is_empty() {
        evidence.premature_rate = Some(mean(&clicks));
    }
    let dwell: Vec<f64> = flicks.iter().filter_map(|e| e.dwell_ms).collect();
    evidence.median_dwell_ms = median(&dwell);
    if !all.is_empty() {
        evidence.hit_rate =
            Some(all.iter().filter(|e| e.shot_hit).count() as f64 / all.len() as f64);
    }

    // S10: habit relative to the nominal head line. The mean target pitch approximates the nominal
    // value because the per-trial noise is zero-mean.
    if !peeks.is_empty() {
        let nominal = mean(&peeks.iter().map(|e| e.target_pitch_deg).collect::<Vec<_>>());
        let pitch_errors: Vec<f64> = peeks
            .iter()
            .map(|e| e.pitch_at_spawn_deg - nominal)
            .collect();
        skills.extend(estimate(SkillId::S10HeadLevel, pitch_errors.len(), &|i| {
            let v = pick(&pitch_errors, i);
            Some(sd(&v) + mean(&v).abs())
        }));
        evidence.head_level_bias_deg = Some(mean(&pitch_errors));
    }

    // S11: only directions with n >= 12 are compared, and only |z| >= 2 is reported as a weakness.
    let timed: Vec<(u8, f64)> = static_flicks
        .iter()
        .filter_map(|e| Some((e.octant?, e.movement_time_us? as f64 / 1000.0)))
        .collect();
    if !timed.is_empty() {
        let overall = mean(&timed.iter().map(|t| t.1).collect::<Vec<_>>());
        let mut worst: Option<(u8, f64)> = None;
        for octant in 0..8_u8 {
            let times: Vec<f64> = timed
                .iter()
                .filter(|t| t.0 == octant)
                .map(|t| t.1)
                .collect();
            if times.len() < 12 {
                continue;
            }
            let se = sd(&times) / (times.len() as f64).sqrt();
            if se <= 0.0 {
                continue;
            }
            // Slower than average is a deficit, so the sign is flipped.
            let z = -(mean(&times) - overall) / se;
            if worst.map_or(true, |w| z < w.1) {
                worst = Some((octant, z));
            }
        }
        if let Some((octant, z)) = worst {
            let n = timed.len();
            evidence.weakest_octant = (z <= -2.0).then_some((octant, z));
            skills.extend(estimate(SkillId::S11DirectionalBalance, n, &|_| {
                Some(z.min(0.0).abs())
            }));
        }
    }

    // S12
    let mut moving = Vec::new();
    let mut stop_to_shot = Vec::new();
    for (session, engagements) in per_session
        .iter()
        .filter(|(s, _)| is_scenario(s, "counter_strafe"))
    {
        let player = &session.streams.player;
        for shot in engagements
            .iter()
            .filter(|e| !e.is_home)
            .filter_map(|e| e.shot_t_us)
        {
            let i = player.t_us.partition_point(|&t| t <= shot);
            if i == 0 {
                continue;
            }
            moving.push(f64::from(u8::from(
                player.vel_x[i - 1].abs() > ACCURATE_SPEED_MPS,
            )));
            if let Some(j) = (0..i)
                .rev()
                .find(|&j| player.vel_x[j].abs() > ACCURATE_SPEED_MPS)
            {
                stop_to_shot.push(shot.saturating_sub(player.t_us[j]) as f64 / 1000.0);
            }
        }
    }
    skills.extend(estimate(SkillId::S12CounterStrafe, moving.len(), &|i| {
        Some(mean(&pick(&moving, i)))
    }));
    if !moving.is_empty() {
        evidence.moving_shot_rate = Some(mean(&moving));
        evidence.median_stop_to_shot_ms = median(&stop_to_shot);
    }

    // S13
    let ttk: Vec<f64> = all
        .iter()
        .filter_map(|e| e.time_to_hit_us.map(|t| t as f64 / 1000.0))
        .collect();
    skills.extend(estimate(SkillId::S13Consistency, ttk.len(), &|i| {
        (i.len() >= 2).then(|| {
            let v = pick(&ttk, i);
            sd(&v) / mean(&v)
        })
    }));
    if ttk.len() >= 4 {
        let x: Vec<f64> = (0..ttk.len()).map(|i| i as f64).collect();
        if let Some((slope, _)) = slope_with_p(&x, &ttk) {
            evidence.fatigue_per_trial = Some(slope / mean(&ttk));
        }
    }

    SkillProfile {
        norms_version: NORMS_VERSION.to_string(),
        skills,
        evidence,
    }
}

/// Share of tracking time spent on target, or `None` when the session has no moving target.
pub fn time_on_target(session: &AftSession) -> Option<f64> {
    let samples = tracking_metrics(session).on_target;
    (!samples.is_empty()).then(|| mean(&samples))
}
