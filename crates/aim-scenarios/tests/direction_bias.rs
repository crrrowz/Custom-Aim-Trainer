use aim_scenarios::generator::{generate_fitts_trials, weight_toward_octant};
use aim_scenarios::schema::{DistanceAnchor, DrillRules, FittsGrid, Scenario};

fn grid() -> FittsGrid {
    FittsGrid {
        distances_deg: vec![10.0, 20.0],
        radius_sizes_deg: vec![0.8],
        octants: 8,
        repetitions_per_cell: 5,
        anchor: DistanceAnchor::HomeTargetCenter,
    }
}

fn share(trials: &[aim_scenarios::generator::Trial], octant: u32) -> f64 {
    trials.iter().filter(|t| t.octant == octant).count() as f64 / trials.len() as f64
}

#[test]
fn seventy_percent_of_targets_point_in_the_weak_direction() {
    let mut trials = generate_fitts_trials(&grid(), 2.0, 7);
    let before = trials.len();
    weight_toward_octant(&mut trials, 3, 0.7);
    assert_eq!(trials.len(), before);
    assert!(
        (share(&trials, 3) - 0.7).abs() < 0.02,
        "{}",
        share(&trials, 3)
    );
}

#[test]
fn trial_numbers_stay_in_place() {
    let mut trials = generate_fitts_trials(&grid(), 2.0, 7);
    let indices: Vec<usize> = trials.iter().map(|t| t.index).collect();
    weight_toward_octant(&mut trials, 3, 0.7);
    assert_eq!(trials.iter().map(|t| t.index).collect::<Vec<_>>(), indices);
}

#[test]
fn other_directions_still_appear() {
    let mut trials = generate_fitts_trials(&grid(), 2.0, 7);
    weight_toward_octant(&mut trials, 3, 0.7);
    assert!(trials.iter().any(|t| t.octant != 3));
}

#[test]
fn unknown_direction_changes_nothing() {
    let mut trials = generate_fitts_trials(&grid(), 2.0, 7);
    let original = trials.clone();
    weight_toward_octant(&mut trials, 99, 0.7);
    assert_eq!(trials, original);
}

#[test]
fn scenarios_without_rules_still_load() {
    let text =
        std::fs::read_to_string("../../content/scenarios/assess.v1.flick_static.ron").unwrap();
    let scenario: Scenario = ron::from_str(&text).unwrap();
    assert_eq!(scenario.rules, DrillRules::default());
}

#[test]
fn the_counter_strafe_test_turns_on_its_rule() {
    let text =
        std::fs::read_to_string("../../content/scenarios/assess.v1.counter_strafe.ron").unwrap();
    let scenario = Scenario::from_ron_str(&text).unwrap();
    assert!(scenario.rules.counter_strafe);
}
