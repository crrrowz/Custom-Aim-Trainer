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
}

impl Scenario {
    /// Validates that layout and hit_rule are mutually compatible.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.layout, self.hit_rule) {
            (TargetLayout::Tracking { .. }, HitRule::SingleShotKill) => {
                Err(format!(
                    "Invalid scenario '{}': Tracking layout requires ContinuousTracking, cannot use SingleShotKill",
                    self.id
                ))
            }
            (TargetLayout::Fitts { .. }, HitRule::ContinuousTracking) => {
                Err(format!(
                    "Invalid scenario '{}': Fitts layout requires SingleShotKill, cannot use ContinuousTracking",
                    self.id
                ))
            }
            (TargetLayout::HeadLevelPeeks { .. }, HitRule::ContinuousTracking) => {
                Err(format!(
                    "Invalid scenario '{}': HeadLevelPeeks layout requires SingleShotKill, cannot use ContinuousTracking",
                    self.id
                ))
            }
            _ => Ok(()),
        }
    }
}
