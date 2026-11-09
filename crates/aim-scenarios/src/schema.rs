//! Formal RON scenario definitions for AimForge assessments and drills.

use serde::{Deserialize, Serialize};

/// Classification of scenario intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioKind {
    Warmup,
    Assessment,
    Drill,
}

/// Target shape and collision geometry archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetShape {
    Sphere,
    HeadModel,
    WallPeek,
}

/// Trial shooting and hit termination rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitRule {
    /// Exactly one shot permitted: hit concludes trial, miss concludes trial.
    SingleShotKill,
    /// Continuous tracking: target remains alive, recording ray-intersection every tick.
    ContinuousTracking,
}

impl HitRule {
    /// Tracking is scored on staying on the target, so clicking does nothing there.
    pub fn accepts_shots(self) -> bool {
        self == Self::SingleShotKill
    }
}

/// Starting point for angular distance measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceAnchor {
    /// Trial target distance is measured from the central crosshair (home target at yaw=0, pitch=0).
    HomeTargetCenter,
    /// Trial target distance is measured from the previous target's final coordinate.
    PreviousTarget,
}

/// Fitts' Law discrete spatial layout grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FittsGrid {
    /// Radial distances from anchor in angular degrees (e.g., [8.0, 20.0, 40.0]).
    pub distances_deg: Vec<f64>,
    /// Target RADIUS in angular degrees (e.g., [0.4, 0.8]).
    pub radius_sizes_deg: Vec<f64>,
    /// Number of radial direction octants (typically 8, stepped by 45° CCW starting from Right = 0°).
    pub octants: u32,
    /// Repetitions per (distance, size, octant) cell.
    pub repetitions_per_cell: u32,
    /// Distance reference anchor.
    pub anchor: DistanceAnchor,
}

impl FittsGrid {
    pub fn total_trials(&self) -> usize {
        self.distances_deg.len()
            * self.radius_sizes_deg.len()
            * (self.octants as usize)
            * (self.repetitions_per_cell as usize)
    }
}

/// Target motion dynamics for continuous tracking courses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrackingTrajectory {
    /// Constant angular speed path with smooth bounded heading turning.
    ConstantSpeed {
        speed_deg_s: f64,
        max_turn_rate_deg_s: f64,
        max_yaw_deg: f64,
        max_pitch_deg: f64,
    },
    /// Reactive A-D strafe with exponential reversal distribution.
    ReactiveStrafe {
        speed_deg_s: f64,
        mean_reversal_s: f64,
        min_reversal_s: f64,
        max_reversal_s: f64,
        max_yaw_deg: f64,
        fixed_pitch_deg: f64,
    },
}

/// Scenario spatial distribution layout design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TargetLayout {
    /// Discrete Fitts Law grid with per-trial timeout (A0, A1, A2).
    Fitts {
        grid: FittsGrid,
        trial_timeout_s: f64,
    },
    /// Head-level lane with tall occluder walls and vertical Gaussian noise (A3).
    HeadLevelPeeks {
        peeks_count: u32,
        occluder_yaws_deg: Vec<f64>,
        target_radius_deg: f64,
        head_pitch_deg: f64,
        pitch_noise_std_deg: f64,
        pitch_noise_clamp_deg: f64,
        peek_speed_mps: f64,
        trial_timeout_s: f64,
    },
    /// Continuous 1000Hz reconstructible tracking course (A4, A5).
    Tracking {
        target_radius_deg: f64,
        trajectory: TrackingTrajectory,
        duration_s: f64,
    },
    /// Multi-target cluster with concurrent targets (A6).
    ConcurrentCluster {
        concurrent_count: u32,
        total_targets: u32,
        target_radius_deg: f64,
        distance_bins_deg: Vec<f64>,
        octants: u32,
        min_separation_deg: f64,
        max_yaw_deg: f64,
        max_pitch_deg: f64,
        duration_s: f64,
    },
}

/// Speed at or below which a shot is accurate (m/s). Provisional, calibrated against VALORANT later.
pub const ACCURATE_SPEED_MPS: f64 = 0.7;

/// Extra rules that make a drill train one specific habit. All off by default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DrillRules {
    /// A hit does not count if the flick went past this share of the distance.
    #[serde(default)]
    pub overshoot_fail_ratio: Option<f64>,
    /// A hit counts only if the crosshair was at rest for this long before the click.
    #[serde(default)]
    pub settle_ms: Option<f64>,
    /// Most targets appear in this direction (0..8, counter-clockwise from right).
    #[serde(default)]
    pub weak_octant: Option<u32>,
    /// Show how far the first movement went after every flick.
    #[serde(default)]
    pub show_gain: bool,
    /// A/D movement is active and a hit does not count while moving.
    #[serde(default)]
    pub counter_strafe: bool,
    /// Directions outside the allowed area are skipped instead of rejecting the scenario.
    #[serde(default)]
    pub clip_to_area: bool,
}

/// Complete scenario specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub version: u32,
    pub kind: ScenarioKind,
    pub timeout_s: f64,
    pub spawn_distance_m: f64,
    pub target_shape: TargetShape,
    pub layout: TargetLayout,
    pub hit_rule: HitRule,
    #[serde(default)]
    pub rules: DrillRules,
}

impl Scenario {
    /// Validates that layout and hit_rule are mutually compatible and layout geometry is valid.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.layout, self.hit_rule) {
            (TargetLayout::Tracking { .. }, HitRule::SingleShotKill) => {
                return Err(format!(
                    "Invalid scenario '{}': Tracking layout requires ContinuousTracking, cannot use SingleShotKill",
                    self.id
                ));
            }
            (TargetLayout::Fitts { .. }, HitRule::ContinuousTracking) => {
                return Err(format!(
                    "Invalid scenario '{}': Fitts layout requires SingleShotKill, cannot use ContinuousTracking",
                    self.id
                ));
            }
            (TargetLayout::HeadLevelPeeks { .. }, HitRule::ContinuousTracking) => {
                return Err(format!(
                    "Invalid scenario '{}': HeadLevelPeeks layout requires SingleShotKill, cannot use ContinuousTracking",
                    self.id
                ));
            }
            _ => {}
        }

        if let (
            false,
            TargetLayout::ConcurrentCluster {
                distance_bins_deg,
                octants,
                max_yaw_deg,
                max_pitch_deg,
                ..
            },
        ) = (self.rules.clip_to_area, &self.layout)
        {
            for &dist in distance_bins_deg {
                for oct in 0..*octants {
                    let bearing_deg = oct as f64 * (360.0 / *octants as f64);
                    let (yaw, pitch) =
                        crate::generator::target_angles_from_bearing(dist, bearing_deg);
                    let signed_yaw = if yaw > 180.0 { yaw - 360.0 } else { yaw };
                    if signed_yaw.abs() > *max_yaw_deg + 1e-6 || pitch.abs() > *max_pitch_deg + 1e-6
                    {
                        return Err(format!(
                            "Invalid scenario '{}': declared distance bin {}° at octant {} (yaw {:.2}°, pitch {:.2}°) cannot fit within max_yaw {}° / max_pitch {}°",
                            self.id, dist, oct, signed_yaw, pitch, max_yaw_deg, max_pitch_deg
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Parses and validates a scenario from a RON string.
    ///
    /// Fails immediately if the RON syntax is invalid or if scenario validation rules fail.
    pub fn from_ron_str(s: &str) -> Result<Self, String> {
        let sc: Self = ron::from_str(s).map_err(|e| format!("Failed parsing scenario RON: {e}"))?;
        sc.validate()?;
        Ok(sc)
    }
}

/// Convenience loader function for aim-scenarios that parses and validates a scenario.
pub fn load_scenario(s: &str) -> Result<Scenario, String> {
    Scenario::from_ron_str(s)
}

impl std::str::FromStr for Scenario {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_ron_str(s)
    }
}
