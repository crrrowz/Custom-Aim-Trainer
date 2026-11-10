//! Turns a drill name and difficulty, or the player's own target settings, into a playable scenario.
//!
//! Drills reuse the engine's four layouts; `DrillRules` adds what makes each one train its own habit.

use aim_scenarios::schema::{
    DistanceAnchor, DrillRules, FittsGrid, HitRule, Scenario, ScenarioKind, TargetLayout,
    TargetShape, TrackingTrajectory,
};
use serde::{Deserialize, Serialize};

const SPAWN_DISTANCE_M: f64 = 15.0;
const OCTANTS: u32 = 8;
/// Half the visible screen, in degrees from the centre, with a small margin: the game uses a 103 degree
/// horizontal field of view on a 16:9 screen, which shows about 51 degrees left and right and 35 up and down.
pub const ON_SCREEN_H_DEG: f64 = 48.0;
pub const ON_SCREEN_V_DEG: f64 = 33.0;
const SECONDS_PER_TRIAL: f64 = 1.5;
const PEEK_SPEED_MPS: f64 = 5.73;

pub const DRILL_IDS: [&str; 16] = [
    "warmup",
    "deceleration_ladder",
    "commit_flick",
    "consistency_grid",
    "micro_hold",
    "confirm_click",
    "timed_kill",
    "head_level_hold",
    "peek_line",
    "directional_flick",
    "strafe_track_reactive",
    "smooth_track",
    "reflex_dot",
    "precision_first",
    "speed_push",
    "cs_drill",
];

fn lerp(easy: f64, hard: f64, d: f64) -> f64 {
    easy + (hard - easy) * d.clamp(0.0, 1.0)
}

fn scenario(id: String, layout: TargetLayout, hit_rule: HitRule, timeout_s: f64) -> Scenario {
    Scenario {
        id,
        version: 1,
        kind: ScenarioKind::Drill,
        timeout_s,
        spawn_distance_m: SPAWN_DISTANCE_M,
        target_shape: if matches!(layout, TargetLayout::HeadLevelPeeks { .. }) {
            TargetShape::WallPeek
        } else {
            TargetShape::Sphere
        },
        layout,
        hit_rule,
        rules: DrillRules::default(),
    }
}

fn flick(
    id: String,
    seconds: f64,
    distances: Vec<f64>,
    radius: f64,
    trial_timeout_s: f64,
) -> Scenario {
    let cells = distances.len() as u32 * OCTANTS;
    let wanted = (seconds / SECONDS_PER_TRIAL).ceil() as u32;
    let reps = wanted.div_ceil(cells).max(1);
    scenario(
        id,
        TargetLayout::Fitts {
            grid: FittsGrid {
                distances_deg: distances,
                radius_sizes_deg: vec![radius],
                octants: OCTANTS,
                repetitions_per_cell: reps,
                anchor: DistanceAnchor::HomeTargetCenter,
            },
            trial_timeout_s,
        },
        HitRule::SingleShotKill,
        seconds,
    )
}

fn peeks(id: String, seconds: f64, radius: f64, noise: f64, trial_timeout_s: f64) -> Scenario {
    scenario(
        id,
        TargetLayout::HeadLevelPeeks {
            peeks_count: (seconds / trial_timeout_s).ceil().max(1.0) as u32,
            occluder_yaws_deg: vec![-25.0, 25.0],
            target_radius_deg: radius,
            head_pitch_deg: 1.25,
            pitch_noise_std_deg: noise,
            pitch_noise_clamp_deg: noise * 2.0,
            peek_speed_mps: PEEK_SPEED_MPS,
            trial_timeout_s,
        },
        HitRule::SingleShotKill,
        seconds,
    )
}

fn tracking(id: String, seconds: f64, radius: f64, trajectory: TrackingTrajectory) -> Scenario {
    scenario(
        id,
        TargetLayout::Tracking {
            target_radius_deg: radius,
            trajectory,
            duration_s: seconds,
        },
        HitRule::ContinuousTracking,
        seconds,
    )
}

fn smooth(speed: f64) -> TrackingTrajectory {
    TrackingTrajectory::ConstantSpeed {
        speed_deg_s: speed,
        max_turn_rate_deg_s: 60.0,
        max_yaw_deg: 35.0,
        max_pitch_deg: 12.0,
    }
}

fn strafe(speed: f64, mean_reversal_s: f64) -> TrackingTrajectory {
    TrackingTrajectory::ReactiveStrafe {
        speed_deg_s: speed,
        mean_reversal_s,
        min_reversal_s: mean_reversal_s * 0.6,
        max_reversal_s: mean_reversal_s * 1.4,
        max_yaw_deg: 35.0,
        fixed_pitch_deg: 0.0,
    }
}

/// Builds a drill at difficulty `d`. From day 3 `varied` spreads the distances around the same skill.
pub fn build_drill(name: &str, d: f64, varied: bool, seconds: f64) -> Option<Scenario> {
    let id = format!("drill.{name}");
    // Flicks come in every direction, so the farthest one must still be on screen straight up.
    let far = lerp(20.0, 40.0, d).min(ON_SCREEN_V_DEG - 1.5);
    let spread = if varied {
        vec![far * 0.6, far * 0.8, far]
    } else {
        vec![far]
    };
    let mut built = match name {
        "warmup" => flick(id, seconds, vec![10.0, 20.0], 1.0, 3.5),
        "cs_drill" => flick(
            id,
            seconds,
            vec![8.0, 20.0],
            lerp(1.2, 0.6, d),
            lerp(3.0, 1.5, d),
        ),
        "deceleration_ladder" => flick(id, seconds, spread, lerp(1.2, 0.45, d), lerp(1.8, 0.7, d)),
        "commit_flick" => flick(id, seconds, spread, lerp(1.5, 0.8, d), lerp(1.2, 0.5, d)),
        "consistency_grid" => {
            let distances = if varied {
                vec![15.0, 20.0, 25.0]
            } else {
                vec![20.0]
            };
            flick(id, seconds, distances, lerp(1.0, 0.5, d), 2.0)
        }
        "micro_hold" | "precision_first" => {
            flick(id, seconds, vec![3.0, 6.0], lerp(0.6, 0.3, d), 3.0)
        }
        "confirm_click" => flick(id, seconds, spread, lerp(1.0, 0.5, d), 2.5),
        "timed_kill" | "speed_push" => {
            flick(id, seconds, spread, lerp(1.0, 0.6, d), lerp(1.0, 0.45, d))
        }
        "directional_flick" => flick(id, seconds, spread, lerp(1.0, 0.5, d), 2.0),
        "reflex_dot" => flick(id, seconds, vec![4.0], 0.8, lerp(0.9, 0.4, d)),
        "head_level_hold" | "peek_line" => peeks(
            id,
            seconds,
            lerp(0.7, 0.4, d),
            lerp(0.2, 0.6, d),
            lerp(2.5, 1.2, d),
        ),
        "strafe_track_reactive" => tracking(
            id,
            seconds,
            lerp(0.9, 0.5, d),
            strafe(lerp(14.0, 28.0, d), lerp(0.8, 0.45, d)),
        ),
        "smooth_track" => tracking(id, seconds, lerp(0.9, 0.5, d), smooth(lerp(10.0, 26.0, d))),
        _ => return None,
    };
    built.rules = rules_for(name, d);
    Some(built)
}

fn rules_for(name: &str, d: f64) -> DrillRules {
    let mut rules = DrillRules::default();
    match name {
        "deceleration_ladder" => {
            rules.overshoot_fail_ratio = Some(lerp(1.25, 1.08, d));
            rules.show_gain = true;
        }
        "commit_flick" => rules.show_gain = true,
        "confirm_click" => rules.settle_ms = Some(lerp(20.0, 60.0, d)),
        "micro_hold" => rules.settle_ms = Some(lerp(150.0, 300.0, d)),
        "cs_drill" => rules.counter_strafe = true,
        _ => {}
    }
    rules
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomMode {
    Flick,
    Track,
    Strafe,
    Peek,
    Multi,
}

/// The target options the player sets in the practice screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomTargets {
    pub mode: CustomMode,
    pub radius_deg: f64,
    pub min_distance_deg: f64,
    pub max_distance_deg: f64,
    pub speed_deg_s: f64,
    pub time_limit_s: f64,
    pub concurrent: u32,
    pub duration_s: f64,
    /// How far targets may go left/right and up/down from the centre.
    #[serde(default = "default_area_h")]
    pub area_h_deg: f64,
    #[serde(default = "default_area_v")]
    pub area_v_deg: f64,
    /// Average seconds between direction changes, for strafe tracking.
    #[serde(default = "default_reversal")]
    pub reversal_s: f64,
}

fn default_area_h() -> f64 {
    35.0
}

fn default_area_v() -> f64 {
    12.0
}

fn default_reversal() -> f64 {
    0.55
}

impl CustomTargets {
    /// The same settings, limited so every target appears on screen and the nearest distance never
    /// exceeds the farthest. Flick targets come in every direction, so straight up must fit as well.
    pub fn on_screen(&self) -> Self {
        let mut c = self.clone();
        let edge = |limit: f64| (limit - c.radius_deg).max(1.0);
        let reach = match c.mode {
            CustomMode::Flick => edge(ON_SCREEN_V_DEG),
            _ => edge(ON_SCREEN_H_DEG),
        };
        let (lo, hi) = (
            c.min_distance_deg.min(c.max_distance_deg),
            c.min_distance_deg.max(c.max_distance_deg),
        );
        c.max_distance_deg = hi.min(reach);
        c.min_distance_deg = lo.min(c.max_distance_deg);
        c.area_h_deg = c.area_h_deg.min(edge(ON_SCREEN_H_DEG));
        c.area_v_deg = c.area_v_deg.min(edge(ON_SCREEN_V_DEG));
        c
    }
}

fn distance_steps(c: &CustomTargets) -> Vec<f64> {
    let (lo, hi) = (
        c.min_distance_deg.min(c.max_distance_deg),
        c.min_distance_deg.max(c.max_distance_deg),
    );
    if hi - lo < 1.0 {
        vec![hi]
    } else {
        vec![lo, (lo + hi) / 2.0, hi]
    }
}

pub fn build_custom(c: &CustomTargets) -> Result<Scenario, String> {
    let c = &c.on_screen();
    let id = "custom.practice".to_string();
    let scenario = match c.mode {
        CustomMode::Flick => flick(
            id,
            c.duration_s,
            distance_steps(c),
            c.radius_deg,
            c.time_limit_s,
        ),
        CustomMode::Track => tracking(
            id,
            c.duration_s,
            c.radius_deg,
            TrackingTrajectory::ConstantSpeed {
                speed_deg_s: c.speed_deg_s,
                max_turn_rate_deg_s: 60.0,
                max_yaw_deg: c.area_h_deg,
                max_pitch_deg: c.area_v_deg,
            },
        ),
        CustomMode::Strafe => tracking(
            id,
            c.duration_s,
            c.radius_deg,
            TrackingTrajectory::ReactiveStrafe {
                speed_deg_s: c.speed_deg_s,
                mean_reversal_s: c.reversal_s,
                min_reversal_s: c.reversal_s * 0.6,
                max_reversal_s: c.reversal_s * 1.4,
                max_yaw_deg: c.area_h_deg,
                fixed_pitch_deg: 0.0,
            },
        ),
        CustomMode::Peek => peeks(id, c.duration_s, c.radius_deg, 0.3, c.time_limit_s),
        CustomMode::Multi => {
            let bins = distance_steps(c);
            let fits = (0..OCTANTS)
                .flat_map(|o| bins.iter().map(move |&d| (d, o)))
                .filter(|&(d, o)| {
                    let (yaw, pitch) = aim_scenarios::generator::target_angles_from_bearing(
                        d,
                        f64::from(o) * 360.0 / f64::from(OCTANTS),
                    );
                    let signed = if yaw > 180.0 { yaw - 360.0 } else { yaw };
                    signed.abs() <= c.area_h_deg && pitch.abs() <= c.area_v_deg
                })
                .count() as u32;
            if fits == 0 {
                return Err(
                    "No target fits inside that area. Widen it or bring the targets closer."
                        .to_string(),
                );
            }
            let wanted = (c.duration_s / SECONDS_PER_TRIAL).ceil() as u32;
            let cells = bins.len() as u32 * OCTANTS;
            let mut built = scenario(
                id,
                TargetLayout::ConcurrentCluster {
                    concurrent_count: c.concurrent.max(1),
                    total_targets: wanted.div_ceil(cells).max(1) * cells,
                    target_radius_deg: c.radius_deg,
                    distance_bins_deg: bins,
                    octants: OCTANTS,
                    min_separation_deg: (c.radius_deg * 4.0).max(3.0),
                    max_yaw_deg: c.area_h_deg,
                    max_pitch_deg: c.area_v_deg,
                    duration_s: c.duration_s,
                },
                HitRule::SingleShotKill,
                c.duration_s,
            );
            built.rules.clip_to_area = true;
            built
        }
    };
    scenario.validate()?;
    Ok(scenario)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn radius(s: &Scenario) -> f64 {
        match &s.layout {
            TargetLayout::Fitts { grid, .. } => grid.radius_sizes_deg[0],
            _ => panic!("expected a flick drill"),
        }
    }

    #[test]
    fn every_drill_builds_a_valid_scenario_at_both_ends() {
        for name in DRILL_IDS {
            for d in [0.0, 1.0] {
                let s = build_drill(name, d, true, 120.0).unwrap();
                s.validate().unwrap();
            }
        }
    }

    #[test]
    fn higher_difficulty_means_smaller_targets() {
        let easy = build_drill("deceleration_ladder", 0.0, false, 120.0).unwrap();
        let hard = build_drill("deceleration_ladder", 1.0, false, 120.0).unwrap();
        assert!(radius(&hard) < radius(&easy));
    }

    fn multi(area_h_deg: f64, area_v_deg: f64) -> CustomTargets {
        CustomTargets {
            mode: CustomMode::Multi,
            radius_deg: 0.7,
            min_distance_deg: 10.0,
            max_distance_deg: 30.0,
            speed_deg_s: 20.0,
            time_limit_s: 2.0,
            concurrent: 3,
            duration_s: 60.0,
            area_h_deg,
            area_v_deg,
            reversal_s: 0.55,
        }
    }

    #[test]
    fn a_narrow_area_keeps_only_the_targets_that_fit() {
        let s = build_custom(&multi(40.0, 12.0)).unwrap();
        let table = aim_scenarios::generator::generate_cluster_candidates(&s, 7);
        assert!(!table.is_empty());
        assert!(s.rules.clip_to_area);
    }

    #[test]
    fn the_hardest_drills_stay_on_screen() {
        for name in [
            "deceleration_ladder",
            "commit_flick",
            "confirm_click",
            "speed_push",
        ] {
            let TargetLayout::Fitts { grid, .. } =
                build_drill(name, 1.0, true, 60.0).unwrap().layout
            else {
                panic!("{name} is a flick drill");
            };
            assert!(
                grid.distances_deg
                    .iter()
                    .all(|d| *d + 1.0 <= ON_SCREEN_V_DEG),
                "{name}"
            );
        }
    }

    #[test]
    fn custom_targets_stay_on_screen() {
        let mut c = multi(90.0, 80.0);
        c.mode = CustomMode::Flick;
        c.min_distance_deg = 70.0;
        c.max_distance_deg = 60.0;
        let fixed = c.on_screen();
        assert!(fixed.min_distance_deg <= fixed.max_distance_deg);
        assert!(fixed.max_distance_deg + fixed.radius_deg <= ON_SCREEN_V_DEG);
        let TargetLayout::Fitts { grid, .. } = build_custom(&c).unwrap().layout else {
            panic!("flick builds a Fitts layout");
        };
        assert!(grid
            .distances_deg
            .iter()
            .all(|d| d + c.radius_deg <= ON_SCREEN_V_DEG));

        c.mode = CustomMode::Track;
        let fixed = c.on_screen();
        assert!(fixed.area_h_deg + fixed.radius_deg <= ON_SCREEN_H_DEG);
        assert!(fixed.area_v_deg + fixed.radius_deg <= ON_SCREEN_V_DEG);
    }

    #[test]
    fn an_impossible_area_is_explained() {
        assert!(build_custom(&multi(2.0, 2.0)).is_err());
    }

    #[test]
    fn drills_carry_their_own_rules() {
        let ladder = build_drill("deceleration_ladder", 1.0, false, 60.0).unwrap();
        assert!((ladder.rules.overshoot_fail_ratio.unwrap() - 1.08).abs() < 1e-9);
        assert!(ladder.rules.show_gain);
        let confirm = build_drill("confirm_click", 0.0, false, 60.0).unwrap();
        assert!((confirm.rules.settle_ms.unwrap() - 20.0).abs() < 1e-9);
        assert!(
            build_drill("cs_drill", 0.5, false, 60.0)
                .unwrap()
                .rules
                .counter_strafe
        );
        assert_eq!(
            build_drill("warmup", 0.5, false, 60.0).unwrap().rules,
            DrillRules::default()
        );
    }

    #[test]
    fn rules_survive_the_trip_through_a_scenario_file() {
        let ladder = build_drill("deceleration_ladder", 0.5, false, 60.0).unwrap();
        let text = ron::to_string(&ladder).unwrap();
        let back: Scenario = ron::from_str(&text).unwrap();
        assert_eq!(back.rules, ladder.rules);
    }

    #[test]
    fn all_prescribed_drills_exist() {
        assert!(build_drill("does_not_exist", 0.5, false, 60.0).is_none());
    }

    #[test]
    fn custom_settings_reach_the_scenario() {
        let mut c = CustomTargets {
            mode: CustomMode::Flick,
            radius_deg: 0.7,
            min_distance_deg: 10.0,
            max_distance_deg: 30.0,
            speed_deg_s: 20.0,
            time_limit_s: 2.0,
            concurrent: 3,
            duration_s: 60.0,
            area_h_deg: 35.0,
            area_v_deg: 12.0,
            reversal_s: 0.55,
        };
        assert!((radius(&build_custom(&c).unwrap()) - 0.7).abs() < 1e-9);
        for mode in [
            CustomMode::Track,
            CustomMode::Strafe,
            CustomMode::Peek,
            CustomMode::Multi,
        ] {
            c.mode = mode;
            build_custom(&c).unwrap();
        }
    }
}
