//! Deterministic seeded scenario trial generator.
//!
//! Generates balanced, unbiased trial sequences from Fitts' Law discrete grids,
//! head-level peeking lanes, and multi-target clusters.

use crate::rng::{create_seeded_rng, sample_range_u32, sample_unit_f64};
use crate::schema::{FittsGrid, Scenario, TargetLayout};

/// Individual concrete trial spawned in the trainer.
#[derive(Debug, Clone, PartialEq)]
pub struct Trial {
    pub index: usize,
    pub distance_deg: f64,
    pub radius_deg: f64,
    pub octant: u32,
    pub direction_deg: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub trial_timeout_s: f64,
}

/// Head-level peeking trial specification.
#[derive(Debug, Clone, PartialEq)]
pub struct PeekTrial {
    pub index: usize,
    pub occluder_yaw_deg: f64,
    pub target_radius_deg: f64,
    pub head_pitch_deg: f64,
    pub noisy_pitch_deg: f64,
    pub peek_speed_mps: f64,
    pub trial_timeout_s: f64,
}

/// Computes exact spherical coordinates (yaw, pitch) for a target placed at
/// true angular distance `d_deg` along bearing angle `bearing_deg` from center (0, 0).
pub fn target_angles_from_bearing(distance_deg: f64, bearing_deg: f64) -> (f64, f64) {
    let d_rad = distance_deg.to_radians();
    let theta_rad = bearing_deg.to_radians();

    let sin_d = d_rad.sin();
    let cos_d = d_rad.cos();

    let vx = theta_rad.cos() * sin_d;
    let vy = theta_rad.sin() * sin_d;
    let vz = cos_d;

    let pitch_deg = vy.clamp(-1.0, 1.0).asin().to_degrees();
    let mut yaw_deg = vx.atan2(vz).to_degrees().rem_euclid(360.0);
    if (yaw_deg - 360.0).abs() < 1e-9 {
        yaw_deg = 0.0;
    }

    (yaw_deg, pitch_deg)
}

/// Generates the complete, deterministically shuffled trial sequence for a scenario and seed.
pub fn generate_trials(scenario: &Scenario, seed: u64) -> Vec<Trial> {
    match &scenario.layout {
        TargetLayout::Fitts {
            grid,
            trial_timeout_s,
        } => generate_fitts_trials(grid, *trial_timeout_s, seed),
        _ => Vec::new(),
    }
}

pub fn generate_fitts_trials(grid: &FittsGrid, trial_timeout_s: f64, seed: u64) -> Vec<Trial> {
    let mut trials = Vec::with_capacity(grid.total_trials());
    let step_deg = 360.0 / (grid.octants as f64);

    for &dist in &grid.distances_deg {
        for &radius in &grid.radius_sizes_deg {
            for octant in 0..grid.octants {
                let direction_deg = (octant as f64) * step_deg;
                let (yaw_deg, pitch_deg) = target_angles_from_bearing(dist, direction_deg);

                for _ in 0..grid.repetitions_per_cell {
                    trials.push(Trial {
                        index: 0,
                        distance_deg: dist,
                        radius_deg: radius,
                        octant,
                        direction_deg,
                        yaw_deg,
                        pitch_deg,
                        trial_timeout_s,
                    });
                }
            }
        }
    }

    let mut rng = create_seeded_rng(seed);
    let n = trials.len();
    if n > 1 {
        for i in (1..n).rev() {
            let j = sample_range_u32(&mut rng, 0, (i + 1) as u32) as usize;
            trials.swap(i, j);
        }
    }

    for (idx, trial) in trials.iter_mut().enumerate() {
        trial.index = idx;
    }

    trials
}

/// Generates deterministic peeking trials for A3 (`HeadLevelPeeks`).
pub fn generate_peek_trials(scenario: &Scenario, seed: u64) -> Vec<PeekTrial> {
    if let TargetLayout::HeadLevelPeeks {
        peeks_count,
        occluder_yaws_deg,
        target_radius_deg,
        head_pitch_deg,
        pitch_noise_std_deg,
        pitch_noise_clamp_deg,
        peek_speed_mps,
        trial_timeout_s,
    } = &scenario.layout
    {
        let mut rng = create_seeded_rng(seed);
        let mut trials = Vec::with_capacity(*peeks_count as usize);

        for i in 0..(*peeks_count as usize) {
            // Balance left and right occluder selection
            let occluder_yaw = occluder_yaws_deg[i % occluder_yaws_deg.len()];

            // Box-Muller Gaussian noise for head height variations
            let u1 = sample_unit_f64(&mut rng).clamp(1e-7, 1.0 - 1e-7);
            let u2 = sample_unit_f64(&mut rng);
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            let noise =
                (z * pitch_noise_std_deg).clamp(-*pitch_noise_clamp_deg, *pitch_noise_clamp_deg);

            let noisy_pitch = head_pitch_deg + noise;

            trials.push(PeekTrial {
                index: i,
                occluder_yaw_deg: occluder_yaw,
                target_radius_deg: *target_radius_deg,
                head_pitch_deg: *head_pitch_deg,
                noisy_pitch_deg: noisy_pitch,
                peek_speed_mps: *peek_speed_mps,
                trial_timeout_s: *trial_timeout_s,
            });
        }
        trials
    } else {
        Vec::new()
    }
}

/// Generates deterministic balanced candidates for A6 (`ConcurrentCluster`).
///
/// Preserves balance across distance bins and octants without rejection bias.
pub fn generate_cluster_candidates(scenario: &Scenario, seed: u64) -> Vec<(f64, u32)> {
    if let TargetLayout::ConcurrentCluster {
        total_targets,
        distance_bins_deg,
        octants,
        ..
    } = &scenario.layout
    {
        let mut table = Vec::with_capacity(*total_targets as usize);
        let dist_count = distance_bins_deg.len();

        for i in 0..(*total_targets as usize) {
            let dist = distance_bins_deg[i % dist_count];
            let octant = (i as u32) % octants;
            table.push((dist, octant));
        }

        let mut rng = create_seeded_rng(seed);
        let n = table.len();
        if n > 1 {
            for i in (1..n).rev() {
                let j = sample_range_u32(&mut rng, 0, (i + 1) as u32) as usize;
                table.swap(i, j);
            }
        }
        table
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use aim_core::camera::{angular_distance, ViewAngles};

    fn sample_flick_scenario() -> Scenario {
        Scenario {
            id: "assess.v1.flick_static".to_string(),
            version: 1,
            kind: ScenarioKind::Assessment,
            timeout_s: 288.0,
            spawn_distance_m: 15.0,
            target_shape: TargetShape::Sphere,
            layout: TargetLayout::Fitts {
                grid: FittsGrid {
                    distances_deg: vec![8.0, 20.0, 40.0],
                    radius_sizes_deg: vec![0.4, 0.8],
                    octants: 8,
                    repetitions_per_cell: 2,
                    anchor: DistanceAnchor::HomeTargetCenter,
                },
                trial_timeout_s: 3.0,
            },
            hit_rule: HitRule::SingleShotKill,
        }
    }

    #[test]
    fn test_exact_great_circle_distance_for_all_octants_and_distances() {
        let center = ViewAngles::new(0.0, 0.0);
        let distances = [8.0, 20.0, 40.0, 60.0];

        for &d in &distances {
            for octant in 0..8 {
                let bearing = octant as f64 * 45.0;
                let (yaw, pitch) = target_angles_from_bearing(d, bearing);
                let target_angles = ViewAngles::new(yaw, pitch);

                let actual_dist = angular_distance(center, target_angles);
                assert!(
                    (actual_dist - d).abs() < 1e-9,
                    "Failed for dist {d} at octant {octant} (bearing {bearing}°): expected {d}, got {actual_dist}"
                );
            }
        }
    }

    #[test]
    fn test_golden_trials_order_deadbeef() {
        let scenario = sample_flick_scenario();
        let trials = generate_trials(&scenario, 0xDEADBEEF);

        assert_eq!(trials.len(), 96);

        let t0 = &trials[0];
        assert_eq!(t0.index, 0);
        assert_eq!(t0.distance_deg, 20.0);
        assert_eq!(t0.radius_deg, 0.4);
        assert_eq!(t0.octant, 0);
        assert!((t0.direction_deg - 0.0).abs() < 1e-9);
        assert!((t0.yaw_deg - 20.0).abs() < 1e-9);
        assert!((t0.pitch_deg - 0.0).abs() < 1e-9);

        let t1 = &trials[1];
        assert_eq!(t1.index, 1);
        assert_eq!(t1.distance_deg, 20.0);
        assert_eq!(t1.radius_deg, 0.8);
        assert_eq!(t1.octant, 6);
        assert!((t1.direction_deg - 270.0).abs() < 1e-9);
        assert!((t1.yaw_deg - 0.0).abs() < 1e-9);
        assert!((t1.pitch_deg - (-20.0)).abs() < 1e-9);

        let t2 = &trials[2];
        assert_eq!(t2.index, 2);
        assert_eq!(t2.distance_deg, 8.0);
        assert_eq!(t2.radius_deg, 0.4);
        assert_eq!(t2.octant, 4);
        assert!((t2.direction_deg - 180.0).abs() < 1e-9);
        assert!((t2.yaw_deg - 352.0).abs() < 1e-9);

        let t3 = &trials[3];
        assert_eq!(t3.index, 3);
        assert_eq!(t3.distance_deg, 8.0);
        assert_eq!(t3.radius_deg, 0.4);
        assert_eq!(t3.octant, 7);
        assert!((t3.direction_deg - 315.0).abs() < 1e-9);
        assert!((t3.yaw_deg - 5.67527051328665).abs() < 1e-9);
        assert!((t3.pitch_deg - (-5.647632570320603)).abs() < 1e-9);

        let t4 = &trials[4];
        assert_eq!(t4.index, 4);
        assert_eq!(t4.distance_deg, 20.0);
        assert_eq!(t4.radius_deg, 0.4);
        assert_eq!(t4.octant, 1);
        assert!((t4.direction_deg - 45.0).abs() < 1e-9);
        assert!((t4.yaw_deg - 14.432755043267381).abs() < 1e-9);
        assert!((t4.pitch_deg - 13.995445358891418).abs() < 1e-9);
    }
}
