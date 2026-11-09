//! Rule-based diagnosis (03_DIAGNOSIS_ENGINE.md section 6). D11 and D12 need A7 and multi-day data.

use aim_analysis::{Reliability, SkillId, SkillProfile};
use serde::{Deserialize, Serialize};

const MAX_SHOWN: usize = 3;
const GOOD_SCORE: f64 = 60.0;
const WEAK_SCORE: f64 = 40.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub skill: SkillId,
    pub severity: f64,
    pub priority: f64,
    pub evidence: Vec<(String, f64)>,
    pub drills: Vec<String>,
}

fn reliability_factor(r: Reliability) -> f64 {
    match r {
        Reliability::High => 1.0,
        Reliability::Medium => 0.7,
        Reliability::Low => 0.0,
    }
}

/// (importance for VALORANT, how quickly the skill improves): provisional, calibrated after the Beta.
fn weights(skill: SkillId) -> (f64, f64) {
    match skill {
        SkillId::S01Reaction => (0.8, 0.3),
        SkillId::S02FlickSpeed => (0.8, 0.6),
        SkillId::S03FlickPrecision => (1.0, 0.6),
        SkillId::S04GainControl => (0.8, 0.8),
        SkillId::S05MicroCorrection => (1.0, 0.6),
        SkillId::S06TrackSmooth => (0.5, 0.6),
        SkillId::S07TrackReactive => (0.8, 0.6),
        SkillId::S08TargetSwitch => (0.5, 0.6),
        SkillId::S09ClickTiming => (1.0, 1.0),
        SkillId::S10HeadLevel => (1.0, 1.0),
        SkillId::S11DirectionalBalance => (0.5, 0.7),
        SkillId::S13Consistency => (0.5, 0.4),
    }
}

/// (rule id, skill, severity, evidence, drills) before priorities are computed.
type Candidate<'a> = (&'a str, SkillId, f64, Vec<(String, f64)>, Vec<String>);

fn drills(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

pub fn diagnose(profile: &SkillProfile) -> Vec<Finding> {
    let e = &profile.evidence;
    let score = |id| profile.skill(id).map(|s| s.score);
    let raw = |id| profile.skill(id).map(|s| s.raw);
    let good = |id| score(id).is_some_and(|s| s >= GOOD_SCORE);
    let weak = |id| score(id).is_some_and(|s| s < WEAK_SCORE);

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut add = |id, skill, severity: f64, evidence: Vec<(&str, f64)>, d: &[&str]| {
        candidates.push((
            id,
            skill,
            severity.clamp(0.0, 1.0),
            evidence
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            drills(d),
        ));
    };

    let others_good = [SkillId::S03FlickPrecision, SkillId::S05MicroCorrection]
        .iter()
        .all(|&s| good(s));
    if weak(SkillId::S01Reaction) && others_good {
        let rt = raw(SkillId::S01Reaction).unwrap_or(0.0);
        add(
            "D1",
            SkillId::S01Reaction,
            (rt - 220.0) / 130.0,
            vec![("S01.median_ms", rt)],
            &["reflex_dot", "peek_line"],
        );
    }

    if let (Some(g), Some(iqr)) = (e.gain_median, e.gain_iqr) {
        let growing = e.gain_vs_distance_slope.is_some_and(|s| s > 0.0)
            && e.gain_vs_distance_p.is_some_and(|p| p < 0.05);
        if g > 1.05 && growing && iqr < 0.20 {
            add(
                "D2",
                SkillId::S04GainControl,
                (g - 1.0) / 0.25,
                vec![
                    ("S04.median", g),
                    (
                        "gain_vs_distance_slope",
                        e.gain_vs_distance_slope.unwrap_or(0.0),
                    ),
                    ("S04.iqr", iqr),
                ],
                &["deceleration_ladder"],
            );
        }
        if iqr > 0.30 {
            add(
                "D3",
                SkillId::S04GainControl,
                (iqr - 0.2) / 0.3,
                vec![("S04.iqr", iqr)],
                &["consistency_grid"],
            );
        }
        let subs = e.mean_submovements.unwrap_or(0.0);
        if g < 0.85 && subs > 1.0 {
            add(
                "D4",
                SkillId::S04GainControl,
                (1.0 - g) / 0.3,
                vec![("S04.median", g), ("S05.mean_submovements", subs)],
                &["commit_flick"],
            );
        }
    }

    if weak(SkillId::S05MicroCorrection) && e.mean_submovements.is_some_and(|s| s > 2.0) {
        let t = raw(SkillId::S05MicroCorrection).unwrap_or(0.0);
        add(
            "D5",
            SkillId::S05MicroCorrection,
            (t - 200.0) / 400.0,
            vec![
                ("S05.median_ms", t),
                ("S05.mean_submovements", e.mean_submovements.unwrap_or(0.0)),
            ],
            &["micro_hold"],
        );
    }

    if let Some(rate) = e.premature_rate.filter(|&r| r > 0.25) {
        add(
            "D6",
            SkillId::S09ClickTiming,
            rate / 0.5,
            vec![("S09.premature_rate", rate)],
            &["confirm_click"],
        );
    }
    if let Some(dwell) = e.median_dwell_ms.filter(|&d| d > 150.0) {
        if e.hit_rate.is_some_and(|h| h > 0.8) {
            add(
                "D7",
                SkillId::S09ClickTiming,
                (dwell - 100.0) / 300.0,
                vec![("S09.median_dwell_ms", dwell)],
                &["timed_kill"],
            );
        }
    }
    if let Some(bias) = e.head_level_bias_deg.filter(|b| b.abs() > 1.0) {
        add(
            "D8",
            SkillId::S10HeadLevel,
            bias.abs() / 3.0,
            vec![("S10.bias_deg", bias)],
            &["head_level_hold", "peek_line"],
        );
    }
    if let Some((octant, z)) = e.weakest_octant {
        add(
            "D9",
            SkillId::S11DirectionalBalance,
            z.abs() / 4.0,
            vec![("S11.octant", octant as f64), ("S11.z", z)],
            &["directional_flick"],
        );
    }
    if weak(SkillId::S07TrackReactive) && good(SkillId::S06TrackSmooth) {
        let t = raw(SkillId::S07TrackReactive).unwrap_or(0.0);
        add(
            "D10",
            SkillId::S07TrackReactive,
            (t - 200.0) / 250.0,
            vec![("S07.median_ms", t)],
            &["strafe_track_reactive"],
        );
    }
    if good(SkillId::S02FlickSpeed) && weak(SkillId::S03FlickPrecision) {
        let we = raw(SkillId::S03FlickPrecision).unwrap_or(0.0);
        add(
            "D13",
            SkillId::S03FlickPrecision,
            (we - 1.0) / 3.0,
            vec![("S03.we_deg", we)],
            &["precision_first"],
        );
    }
    if good(SkillId::S03FlickPrecision) && weak(SkillId::S02FlickSpeed) {
        let tp = raw(SkillId::S02FlickSpeed).unwrap_or(0.0);
        add(
            "D14",
            SkillId::S02FlickSpeed,
            (6.0 - tp) / 4.0,
            vec![("S02.tp_bits_s", tp)],
            &["speed_push"],
        );
    }

    let mut findings: Vec<Finding> = candidates
        .into_iter()
        .filter_map(|(id, skill, severity, evidence, drills)| {
            let reliability = reliability_factor(profile.skill(skill)?.reliability);
            let (impact, trainability) = weights(skill);
            let priority = severity * impact * trainability * reliability;
            (priority > 0.0).then(|| Finding {
                id: id.to_string(),
                skill,
                severity,
                priority,
                evidence,
                drills,
            })
        })
        .collect();
    findings.sort_by(|a, b| b.priority.total_cmp(&a.priority));
    findings.truncate(MAX_SHOWN);
    findings
}
