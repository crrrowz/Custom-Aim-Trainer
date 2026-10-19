//! Formal RON scenario definitions for AimForge assessments and drills.

use serde::{Deserialize, Serialize};

/// Classification of scenario intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioKind {
    Assessment,
    Drill,
}

/// Target shape and collision geometry archetype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetShape {
    Sphere,
    HeadModel,
}

/// Trial shooting and hit termination rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitRule {
    /// Exactly one shot permitted: hit concludes trial as success, miss concludes trial as failure.
    SingleShotKill,
    /// Continuous tracking or multi-bullet engagement until health reaches 0.
    MultiShotKill(u32),
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
    /// Computes the total required trial count across all design cells and repetitions.
    pub fn total_trials(&self) -> usize {
        self.distances_deg.len()
            * self.radius_sizes_deg.len()
            * (self.octants as usize)
            * (self.repetitions_per_cell as usize)
    }
}

/// Complete scenario specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub version: u32,
    pub kind: ScenarioKind,
    /// Maximum allowable duration in seconds for a single trial before counting as a timeout miss.
    pub trial_timeout_s: f64,
    /// Overall scenario safety timeout ceiling (seconds).
    pub timeout_s: f64,
    /// Physical spawn distance along the line-of-sight in meters.
    pub spawn_distance_m: f64,
    pub target_shape: TargetShape,
    pub grid: FittsGrid,
    pub hit_rule: HitRule,
}
