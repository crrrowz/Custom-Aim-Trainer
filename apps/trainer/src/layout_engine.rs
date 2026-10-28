//! Layout execution engines for the trainer.
//!
//! Decomposes scenario execution into clean, dedicated handlers:
//! - Fitts (A0, A1, A2)
//! - HeadLevelPeeks (A3)
//! - Tracking (A4, A5)
//! - ConcurrentCluster (A6)

use aim_core::camera::angular_distance;
use aim_core::geometry::SphericalTarget;
use aim_core::ViewAngles;
use aim_scenarios::generator::PeekTrial;
use aim_scenarios::schema::{TargetLayout, TrackingTrajectory};
use aim_scenarios::trajectory::TrajectoryIntegrator;
use aim_scenarios::{
    generate_cluster_candidates, generate_fitts_trials, generate_peek_trials,
    target_angles_from_bearing, Scenario, Trial,
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
    pub emergence_t_us: u64,
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
        target_radius_deg: f64,
        trajectory: TrackingTrajectory,
        seed: u64,
        spawn_t_us: u64,
        integrator: Box<TrajectoryIntegrator>,
        spawned: bool,
    },
    Cluster {
        candidates: Vec<(f64, u32)>,
        next_cand_idx: usize,
        total_targets: u32,
        spawned_count: u32,
        concurrent_count: u32,
        target_radius_deg: f64,
        min_separation_deg: f64,
        max_yaw_deg: f64,
        max_pitch_deg: f64,
        active_targets: Vec<(u32, ViewAngles)>,
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
            TargetLayout::Tracking {
                target_radius_deg,
                trajectory,
                duration_s,
            } => {
                let integrator = Box::new(TrajectoryIntegrator::new(trajectory.clone(), seed));
                Self::Tracking {
                    target_id: 1,
                    duration_s: *duration_s,
                    target_radius_deg: *target_radius_deg,
                    trajectory: trajectory.clone(),
                    seed,
                    spawn_t_us: 0,
                    integrator,
                    spawned: false,
                }
            }
            TargetLayout::ConcurrentCluster {
                concurrent_count,
                total_targets,
                target_radius_deg,
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
                    target_radius_deg: *target_radius_deg,
                    min_separation_deg: *min_separation_deg,
                    max_yaw_deg: *max_yaw_deg,
                    max_pitch_deg: *max_pitch_deg,
                    active_targets: Vec::new(),
                }
            }
        }
    }

    pub fn total_trials_or_targets(&self) -> usize {
        match self {
            Self::Fitts { trials, .. } => trials.len(),
            Self::Peeks { trials, .. } => trials.len(),
            Self::Tracking { .. } => 1,
            Self::Cluster { total_targets, .. } => *total_targets as usize,
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
                active_targets,
                ..
            } => *spawned_count >= *total_targets && active_targets.is_empty(),
        }
    }

    // --- Fitts Handlers ---

    pub fn handle_fitts_spawn(
        &mut self,
        next_target_id: &mut u32,
        now_qpc_us: u64,
        spawn_distance_m: f32,
    ) -> Option<FittsSpawnAction> {
        if let Self::Fitts {
            trials,
            current_idx,
            phase,
            trial_start_t_us,
        } = self
        {
            if *current_idx >= trials.len() {
                return None;
            }
            let tid = *next_target_id;
            *next_target_id += 1;
            match phase {
                FittsPhase::AwaitingHomeTarget => Some(FittsSpawnAction::SpawnHome {
                    target_id: tid,
                    distance_m: spawn_distance_m,
                }),
                FittsPhase::AwaitingTrialTarget => {
                    let trial = trials[*current_idx].clone();
                    *trial_start_t_us = now_qpc_us;
                    Some(FittsSpawnAction::SpawnTrial {
                        target_id: tid,
                        trial,
                        distance_m: spawn_distance_m,
                    })
                }
            }
        } else {
            None
        }
    }

    pub fn handle_fitts_hit(&mut self, is_home_target: bool, t_us: u64) -> Option<FittsHitOutcome> {
        if let Self::Fitts {
            trials,
            current_idx,
            phase,
            trial_start_t_us,
        } = self
        {
            if is_home_target {
                *phase = FittsPhase::AwaitingTrialTarget;
                Some(FittsHitOutcome::HomeHit)
            } else {
                let trial_elapsed_s = (t_us.saturating_sub(*trial_start_t_us) as f64) / 1_000_000.0;
                let trial_idx = *current_idx;
                *current_idx += 1;
                let completed = *current_idx >= trials.len();
                if !completed {
                    *phase = FittsPhase::AwaitingHomeTarget;
                }
                Some(FittsHitOutcome::TrialHit {
                    trial_idx,
                    trial_elapsed_s,
                    completed,
                })
            }
        } else {
            None
        }
    }

    pub fn handle_fitts_miss(&mut self) -> FittsMissOutcome {
        if let Self::Fitts {
            trials,
            current_idx,
            phase,
            ..
        } = self
        {
            if *phase == FittsPhase::AwaitingTrialTarget {
                let trial_idx = *current_idx;
                *current_idx += 1;
                let completed = *current_idx >= trials.len();
                if !completed {
                    *phase = FittsPhase::AwaitingHomeTarget;
                }
                FittsMissOutcome::TrialMiss {
                    trial_idx,
                    completed,
                }
            } else {
                FittsMissOutcome::Ignored
            }
        } else {
            FittsMissOutcome::Ignored
        }
    }

    pub fn handle_fitts_timeout(&mut self, now_qpc_us: u64) -> FittsTimeoutOutcome {
        if let Self::Fitts {
            trials,
            current_idx,
            phase,
            trial_start_t_us,
        } = self
        {
            if *phase == FittsPhase::AwaitingTrialTarget && *current_idx < trials.len() {
                let timeout_s = trials[*current_idx].trial_timeout_s;
                let trial_elapsed_s =
                    (now_qpc_us.saturating_sub(*trial_start_t_us) as f64) / 1_000_000.0;
                if trial_elapsed_s >= timeout_s {
                    let trial_idx = *current_idx;
                    *current_idx += 1;
                    let completed = *current_idx >= trials.len();
                    if !completed {
                        *phase = FittsPhase::AwaitingHomeTarget;
                    }
                    return FittsTimeoutOutcome::TimedOut {
                        trial_idx,
                        trial_elapsed_s,
                        completed,
                    };
                }
            }
        }
        FittsTimeoutOutcome::NoTimeout
    }

    // --- Peeks Handlers ---

    pub fn handle_peeks_spawn(
        &mut self,
        next_target_id: &mut u32,
        now_qpc_us: u64,
        spawn_distance_m: f32,
    ) -> Option<PeeksSpawnAction> {
        if let Self::Peeks {
            trials,
            current_idx,
            awaiting_neutral,
            trial_start_t_us,
            ..
        } = self
        {
            if *current_idx >= trials.len() {
                return None;
            }
            let tid = *next_target_id;
            *next_target_id += 1;
            if *awaiting_neutral {
                Some(PeeksSpawnAction::SpawnNeutralTarget {
                    target_id: tid,
                    distance_m: spawn_distance_m,
                })
            } else {
                let trial = trials[*current_idx].clone();
                *trial_start_t_us = now_qpc_us;
                Some(PeeksSpawnAction::SpawnPeekTarget {
                    target_id: tid,
                    trial,
                    distance_m: spawn_distance_m,
                })
            }
        } else {
            None
        }
    }

    pub fn handle_peeks_hit(
        &mut self,
        is_neutral_target: bool,
        t_us: u64,
    ) -> Option<PeeksHitOutcome> {
        if let Self::Peeks {
            trials,
            current_idx,
            awaiting_neutral,
            trial_start_t_us,
            ..
        } = self
        {
            if is_neutral_target {
                *awaiting_neutral = false;
                Some(PeeksHitOutcome::NeutralHit)
            } else {
                let trial_elapsed_s = (t_us.saturating_sub(*trial_start_t_us) as f64) / 1_000_000.0;
                let trial_idx = *current_idx;
                *current_idx += 1;
                let completed = *current_idx >= trials.len();
                if !completed {
                    *awaiting_neutral = true;
                }
                Some(PeeksHitOutcome::PeekHit {
                    trial_idx,
                    trial_elapsed_s,
                    completed,
                })
            }
        } else {
            None
        }
    }

    pub fn handle_peeks_miss(&mut self) -> PeeksMissOutcome {
        if let Self::Peeks {
            trials,
            current_idx,
            awaiting_neutral,
            ..
        } = self
        {
            if !*awaiting_neutral {
                let trial_idx = *current_idx;
                *current_idx += 1;
                let completed = *current_idx >= trials.len();
                if !completed {
                    *awaiting_neutral = true;
                }
                PeeksMissOutcome::PeekMiss {
                    trial_idx,
                    completed,
                }
            } else {
                PeeksMissOutcome::Ignored
            }
        } else {
            PeeksMissOutcome::Ignored
        }
    }

    pub fn handle_peeks_timeout(&mut self, now_qpc_us: u64) -> PeeksTimeoutOutcome {
        if let Self::Peeks {
            trials,
            current_idx,
            awaiting_neutral,
            trial_start_t_us,
            ..
        } = self
        {
            if !*awaiting_neutral && *current_idx < trials.len() {
                let timeout_s = trials[*current_idx].trial_timeout_s;
                let trial_elapsed_s =
                    (now_qpc_us.saturating_sub(*trial_start_t_us) as f64) / 1_000_000.0;
                if trial_elapsed_s >= timeout_s {
                    let trial_idx = *current_idx;
                    *current_idx += 1;
                    let completed = *current_idx >= trials.len();
                    if !completed {
                        *awaiting_neutral = true;
                    }
                    return PeeksTimeoutOutcome::TimedOut {
                        trial_idx,
                        trial_elapsed_s,
                        completed,
                    };
                }
            }
        }
        PeeksTimeoutOutcome::NoTimeout
    }

    pub fn void_in_flight_trial_on_resume(&mut self, resume_active_us: u64) {
        match self {
            Self::Fitts {
                trial_start_t_us, ..
            } => {
                *trial_start_t_us = resume_active_us;
            }
            Self::Peeks {
                trial_start_t_us, ..
            } => {
                *trial_start_t_us = resume_active_us;
            }
            _ => {}
        }
    }

    // --- Tracking Handlers ---

    pub fn handle_tracking_spawn(
        &mut self,
        next_target_id: &mut u32,
        now_qpc_us: u64,
        spawn_distance_m: f32,
    ) -> Option<TrackingSpawnAction> {
        if let Self::Tracking {
            target_id,
            duration_s,
            target_radius_deg,
            trajectory,
            seed,
            spawn_t_us,
            integrator,
            spawned,
            ..
        } = self
        {
            if !*spawned {
                *spawned = true;
                *spawn_t_us = now_qpc_us;
                let tid = *next_target_id;
                *next_target_id += 1;
                *target_id = tid;
                let angles = integrator.angles_at(0);
                let despawn_t_us = now_qpc_us + (*duration_s * 1_000_000.0) as u64;
                let spline_ron = ron::to_string(&(trajectory.clone(), *seed))
                    .expect("Failed serializing tracking spline");
                Some(TrackingSpawnAction {
                    target_id: tid,
                    initial_angles: angles,
                    radius_deg: *target_radius_deg,
                    distance_m: spawn_distance_m,
                    spline_ron,
                    despawn_t_us,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn update_tracking_target(&mut self, now_qpc_us: u64) -> Option<(u32, ViewAngles, f64)> {
        if let Self::Tracking {
            target_id,
            target_radius_deg,
            spawn_t_us,
            integrator,
            spawned,
            ..
        } = self
        {
            if *spawned {
                let rel_t_us = now_qpc_us.saturating_sub(*spawn_t_us);
                let angles = integrator.angles_at(rel_t_us);
                Some((*target_id, angles, *target_radius_deg))
            } else {
                None
            }
        } else {
            None
        }
    }

    // --- Cluster Handlers ---

    pub fn handle_cluster_spawn(
        &mut self,
        next_target_id: &mut u32,
        spawn_distance_m: f32,
    ) -> Option<ClusterSpawnAction> {
        if let Self::Cluster {
            candidates,
            next_cand_idx,
            total_targets,
            spawned_count,
            concurrent_count,
            target_radius_deg,
            min_separation_deg,
            max_yaw_deg,
            max_pitch_deg,
            active_targets,
        } = self
        {
            if *spawned_count >= *total_targets
                || active_targets.len() >= *concurrent_count as usize
                || *next_cand_idx >= candidates.len()
            {
                return None;
            }

            // Find next candidate in order that satisfies min_separation_deg with currently active targets
            let mut chosen_idx = None;
            let mut chosen_angles = ViewAngles::new(0.0, 0.0);

            for (search_idx, &(dist, octant)) in candidates.iter().enumerate().skip(*next_cand_idx)
            {
                let bearing_deg = octant as f64 * 45.0;
                let (yaw, pitch) = target_angles_from_bearing(dist, bearing_deg);
                let candidate_angles = ViewAngles::new(yaw, pitch);

                let signed_yaw = if candidate_angles.yaw > 180.0 {
                    candidate_angles.yaw - 360.0
                } else {
                    candidate_angles.yaw
                };

                // Verify within prescribed angular bounds
                if signed_yaw.abs() > *max_yaw_deg || candidate_angles.pitch.abs() > *max_pitch_deg
                {
                    continue;
                }

                let satisfies_sep = active_targets.iter().all(|(_, active_angles)| {
                    angular_distance(candidate_angles, *active_angles) >= *min_separation_deg
                });

                if satisfies_sep {
                    chosen_idx = Some(search_idx);
                    chosen_angles = candidate_angles;
                    break;
                }
            }

            let search_idx = match chosen_idx {
                Some(idx) => idx,
                None => {
                    // Do not spawn a target that violates min_separation_deg
                    return None;
                }
            };

            // Move chosen candidate to next_cand_idx position so all candidates are spawned without loss
            if search_idx != *next_cand_idx {
                candidates.swap(*next_cand_idx, search_idx);
            }

            let (dist, octant) = candidates[*next_cand_idx];

            let tid = *next_target_id;
            *next_target_id += 1;
            *next_cand_idx += 1;
            *spawned_count += 1;

            active_targets.push((tid, chosen_angles));

            Some(ClusterSpawnAction {
                target_id: tid,
                angles: chosen_angles,
                radius_deg: *target_radius_deg,
                distance_m: spawn_distance_m,
                distance_bin_deg: dist,
                octant,
                spawned_index: (*spawned_count - 1) as usize,
            })
        } else {
            None
        }
    }

    pub fn handle_cluster_hit(&mut self, hit_target_id: u32) -> Option<ClusterHitOutcome> {
        if let Self::Cluster {
            active_targets,
            total_targets,
            spawned_count,
            ..
        } = self
        {
            if let Some(pos) = active_targets
                .iter()
                .position(|(id, _)| *id == hit_target_id)
            {
                active_targets.remove(pos);
                let completed = *spawned_count >= *total_targets && active_targets.is_empty();
                Some(ClusterHitOutcome {
                    target_id: hit_target_id,
                    completed,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FittsSpawnAction {
    SpawnHome {
        target_id: u32,
        distance_m: f32,
    },
    SpawnTrial {
        target_id: u32,
        trial: Trial,
        distance_m: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FittsHitOutcome {
    HomeHit,
    TrialHit {
        trial_idx: usize,
        trial_elapsed_s: f64,
        completed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FittsMissOutcome {
    Ignored,
    TrialMiss { trial_idx: usize, completed: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FittsTimeoutOutcome {
    NoTimeout,
    TimedOut {
        trial_idx: usize,
        trial_elapsed_s: f64,
        completed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeeksSpawnAction {
    SpawnNeutralTarget {
        target_id: u32,
        distance_m: f32,
    },
    SpawnPeekTarget {
        target_id: u32,
        trial: PeekTrial,
        distance_m: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeeksHitOutcome {
    NeutralHit,
    PeekHit {
        trial_idx: usize,
        trial_elapsed_s: f64,
        completed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeeksMissOutcome {
    Ignored,
    PeekMiss { trial_idx: usize, completed: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeeksTimeoutOutcome {
    NoTimeout,
    TimedOut {
        trial_idx: usize,
        trial_elapsed_s: f64,
        completed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackingSpawnAction {
    pub target_id: u32,
    pub initial_angles: ViewAngles,
    pub radius_deg: f64,
    pub distance_m: f32,
    pub spline_ron: String,
    pub despawn_t_us: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterSpawnAction {
    pub target_id: u32,
    pub angles: ViewAngles,
    pub radius_deg: f64,
    pub distance_m: f32,
    pub distance_bin_deg: f64,
    pub octant: u32,
    pub spawned_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterHitOutcome {
    pub target_id: u32,
    pub completed: bool,
}
