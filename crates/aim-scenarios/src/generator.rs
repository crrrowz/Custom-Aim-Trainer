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

/// Generates the complete, deterministically shuffled trial sequence for a scenario and seed.
///
/// Octant convention:
/// - Octants are numbered 0..octants (e.g. 0..8).
/// - Octant 0 is 0.0° (Directly Right along horizontal yaw).
/// - Steps increment Counter-Clockwise (CCW) by `360.0 / octants` degrees:
///   - Octant 0 = 0.0° (Right)
///   - Octant 1 = 45.0° (Up-Right)
///   - Octant 2 = 90.0° (Up / Positive Pitch)
///   - Octant 3 = 135.0° (Up-Left)
///   - Octant 4 = 180.0° (Left / Negative Yaw)
///   - Octant 5 = 225.0° (Down-Left)
///   - Octant 6 = 270.0° (Down / Negative Pitch)
///   - Octant 7 = 315.0° (Down-Right)
///
/// Angular coordinates:
/// - `yaw_deg` = distance_deg * cos(direction)
/// - `pitch_deg` = distance_deg * sin(direction)
pub fn generate_trials(scenario: &Scenario, seed: u64) -> Vec<Trial> {
    let grid = &scenario.grid;
    let mut trials = Vec::with_capacity(grid.total_trials());

    let step_deg = 360.0 / (grid.octants as f64);

    // 1. Build canonical list of all cell combinations x repetitions
    for &dist in &grid.distances_deg {
        for &radius in &grid.radius_sizes_deg {
            for octant in 0..grid.octants {
                let direction_deg = (octant as f64) * step_deg;
                let dir_rad = direction_deg.to_radians();

                let yaw_deg = dist * dir_rad.cos();
                let pitch_deg = dist * dir_rad.sin();

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

    fn sample_flick_scenario() -> Scenario {
        Scenario {
            id: "assess.v1.flick_static".to_string(),
            version: 1,
            kind: ScenarioKind::Assessment,
            trial_timeout_s: 3.0,
            timeout_s: 288.0, // 96 trials * 3.0s trial_timeout
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

        let t1 = &trials[1];
        assert_eq!(t1.index, 1);
        assert_eq!(t1.distance_deg, 20.0);
        assert_eq!(t1.radius_deg, 0.8);
        assert_eq!(t1.octant, 6);
        assert!((t1.direction_deg - 270.0).abs() < 1e-9);

        let t2 = &trials[2];
        assert_eq!(t2.index, 2);
        assert_eq!(t2.distance_deg, 8.0);
        assert_eq!(t2.radius_deg, 0.4);
        assert_eq!(t2.octant, 4);
        assert!((t2.direction_deg - 180.0).abs() < 1e-9);

        let t3 = &trials[3];
        assert_eq!(t3.index, 3);
        assert_eq!(t3.distance_deg, 8.0);
        assert_eq!(t3.radius_deg, 0.4);
        assert_eq!(t3.octant, 7);
        assert!((t3.direction_deg - 315.0).abs() < 1e-9);

        let t4 = &trials[4];
        assert_eq!(t4.index, 4);
        assert_eq!(t4.distance_deg, 20.0);
        assert_eq!(t4.radius_deg, 0.4);
        assert_eq!(t4.octant, 1);
        assert!((t4.direction_deg - 45.0).abs() < 1e-9);
    }
}
