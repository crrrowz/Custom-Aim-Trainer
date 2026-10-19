//! Deterministic seeded scenario trial generator.
//!
//! Generates balanced, unbiased trial sequences from Fitts' Law discrete grids
//! using Fisher-Yates shuffle driven by ChaCha8 RNG.

use crate::rng::{create_seeded_rng, sample_range_u32};
use crate::schema::Scenario;

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
}

/// Computes exact spherical coordinates (yaw, pitch) for a target placed at
/// true angular distance `d_deg` along bearing angle `bearing_deg` from center (0, 0).
///
/// Bearing convention:
/// - 0.0° = Directly Right along positive yaw
/// - Advances counter-clockwise (CCW):
///   - 0° = Right (+yaw, 0 pitch)
///   - 45° = Up-Right
///   - 90° = Up (0 yaw, +pitch)
///   - 180° = Left (-yaw / 180° yaw, 0 pitch)
///   - 270° = Down (0 yaw, -pitch)
///
/// Exact 3D spherical rotation math:
/// - Start with forward unit vector v0 = [0, 0, 1].
/// - Target displaced by angle D along bearing θ in tangential plane:
///   - Unit tangent vector T = [cos(θ), sin(θ), 0].
///   - Target direction vector V = v0 * cos(D) + T * sin(D)
///     V = [cos(θ)*sin(D), sin(θ)*sin(D), cos(D)].
/// - Note that ||V||^2 = cos^2(θ)*sin^2(D) + sin^2(θ)*sin^2(D) + cos^2(D) = sin^2(D) + cos^2(D) = 1.
/// - The true angular distance from forward [0, 0, 1] is acos(V.z) = acos(cos(D)) = D (exact!).
/// - Pitch: elevation angle from XZ plane = asin(V.y) = asin(sin(θ)*sin(D)).
/// - Yaw: azimuth angle in XZ plane = atan2(V.x, V.z) = atan2(cos(θ)*sin(D), cos(D)).
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
    let grid = &scenario.grid;
    let mut trials = Vec::with_capacity(grid.total_trials());

    let step_deg = 360.0 / (grid.octants as f64);

    // 1. Build canonical list of all cell combinations x repetitions
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
                    });
                }
            }
        }
    }

    // 2. Fisher-Yates shuffle using deterministic sample_range_u32
    let mut rng = create_seeded_rng(seed);
    let n = trials.len();
    if n > 1 {
        for i in (1..n).rev() {
            let j = sample_range_u32(&mut rng, 0, (i + 1) as u32) as usize;
            trials.swap(i, j);
        }
    }

    // 3. Assign sequential 0-based trial indices
    for (idx, trial) in trials.iter_mut().enumerate() {
        trial.index = idx;
    }

    trials
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
            trial_timeout_s: 3.0,
            timeout_s: 288.0,
            spawn_distance_m: 15.0,
            target_shape: TargetShape::Sphere,
            grid: FittsGrid {
                distances_deg: vec![8.0, 20.0, 40.0],
                radius_sizes_deg: vec![0.4, 0.8],
                octants: 8,
                repetitions_per_cell: 2,
                anchor: DistanceAnchor::HomeTargetCenter,
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
    fn test_ron_file_deserialization() {
        let ron_str = include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
        let scenario: Scenario =
            ron::from_str(ron_str).expect("Failed deserializing assess.v1.flick_static.ron");
        assert_eq!(scenario.id, "assess.v1.flick_static");
        assert_eq!(scenario.grid.total_trials(), 96);
    }

    #[test]
    fn test_trial_count_and_full_cell_coverage() {
        let scenario = sample_flick_scenario();
        let trials = generate_trials(&scenario, 42);

        assert_eq!(trials.len(), 96);

        // Verify all 48 unique cells appear exactly 2 times
        for &dist in &scenario.grid.distances_deg {
            for &radius in &scenario.grid.radius_sizes_deg {
                for octant in 0..8 {
                    let matching = trials
                        .iter()
                        .filter(|t| {
                            (t.distance_deg - dist).abs() < 1e-9
                                && (t.radius_deg - radius).abs() < 1e-9
                                && t.octant == octant
                        })
                        .count();
                    assert_eq!(
                        matching, 2,
                        "Cell (dist={dist}, radius={radius}, octant={octant}) occurred {matching} times, expected 2"
                    );
                }
            }
        }
    }

    #[test]
    fn test_golden_trials_order_deadbeef() {
        let scenario = sample_flick_scenario();
        let trials = generate_trials(&scenario, 0xDEADBEEF);

        assert_eq!(trials.len(), 96);

        // Lock exact first 5 trials generated for seed 0xDEADBEEF
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
