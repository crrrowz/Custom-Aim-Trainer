//! Layout execution engines for the trainer.
//!
//! Decomposes scenario execution into clean, dedicated handlers:
//! - Fitts (A0, A1, A2)
//! - HeadLevelPeeks (A3)
//! - Tracking (A4, A5)
//! - ConcurrentCluster (A6)

use aim_core::geometry::SphericalTarget;
use aim_scenarios::generator::PeekTrial;
use aim_scenarios::schema::TargetLayout;
use aim_scenarios::{
    generate_cluster_candidates, generate_fitts_trials, generate_peek_trials, Scenario, Trial,
};
use bevy::prelude::*;

#[derive(Component)]
pub struct TrackingTargetMarker {
    pub target_id: u32,
    pub spherical: SphericalTarget,
}

#[derive(Component)]
pub struct PeekTargetMarker {
    pub trial_idx: usize,
    pub target_id: u32,
    pub spherical: SphericalTarget,
    pub occluder_yaw: f64,
    pub target_yaw: f64,
    pub pitch: f64,
    pub peek_speed_mps: f64,
    pub spawn_distance_m: f32,
    pub spawn_t_us: u64,
}

#[derive(Component)]
pub struct NeutralSlitMarker;

#[derive(Component)]
pub struct ClusterTargetMarker {
    pub target_id: u32,
    pub spherical: SphericalTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FittsPhase {
    AwaitingHomeTarget,
    AwaitingTrialTarget,
}

pub enum LayoutEngineState {
    Fitts {
        trials: Vec<Trial>,
        current_idx: usize,
        phase: FittsPhase,
        trial_start_t_us: u64,
    },
    Peeks {
        trials: Vec<PeekTrial>,
        current_idx: usize,
        awaiting_neutral: bool,
        trial_start_t_us: u64,
        occluders_spawned: bool,
    },
    Tracking {
        target_id: u32,
        duration_s: f64,
        spawned: bool,
    },
    Cluster {
        candidates: Vec<(f64, u32)>,
        next_cand_idx: usize,
        total_targets: u32,
        spawned_count: u32,
        concurrent_count: u32,
        min_separation_deg: f64,
        max_yaw_deg: f64,
        max_pitch_deg: f64,
    },
}

impl LayoutEngineState {
    pub fn new(scenario: &Scenario, seed: u64, initial_t_us: u64) -> Self {
        match &scenario.layout {
            TargetLayout::Fitts {
                grid,
                trial_timeout_s,
            } => {
                let trials = generate_fitts_trials(grid, *trial_timeout_s, seed);
                Self::Fitts {
                    trials,
                    current_idx: 0,
                    phase: FittsPhase::AwaitingHomeTarget,
                    trial_start_t_us: initial_t_us,
                }
            }
            TargetLayout::HeadLevelPeeks { .. } => {
                let trials = generate_peek_trials(scenario, seed);
                Self::Peeks {
                    trials,
                    current_idx: 0,
                    awaiting_neutral: true,
                    trial_start_t_us: initial_t_us,
                    occluders_spawned: false,
                }
            }
            TargetLayout::Tracking { duration_s, .. } => Self::Tracking {
                target_id: 1,
                duration_s: *duration_s,
                spawned: false,
            },
            TargetLayout::ConcurrentCluster {
                concurrent_count,
                total_targets,
                min_separation_deg,
                max_yaw_deg,
                max_pitch_deg,
                ..
            } => {
                let candidates = generate_cluster_candidates(scenario, seed);
                Self::Cluster {
                    candidates,
                    next_cand_idx: 0,
                    total_targets: *total_targets,
                    spawned_count: 0,
                    concurrent_count: *concurrent_count,
                    min_separation_deg: *min_separation_deg,
                    max_yaw_deg: *max_yaw_deg,
                    max_pitch_deg: *max_pitch_deg,
                }
            }
        }
    }

    pub fn is_completed(&self) -> bool {
        match self {
            Self::Fitts {
                trials,
                current_idx,
                ..
            } => *current_idx >= trials.len(),
            Self::Peeks {
                trials,
                current_idx,
                ..
            } => *current_idx >= trials.len(),
            Self::Tracking { .. } => false,
            Self::Cluster {
                spawned_count,
                total_targets,
                ..
            } => *spawned_count >= *total_targets,
        }
    }
}
