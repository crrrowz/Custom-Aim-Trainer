use aim_scenarios::generator::generate_fitts_trials;
use aim_scenarios::schema::{
    DistanceAnchor, FittsGrid, HitRule, Scenario, ScenarioKind, TargetLayout, TargetShape,
};
use trainer::layout_engine::{FittsHitOutcome, FittsSpawnAction, LayoutEngineState};

#[test]
fn test_fitts_engine_sequence_and_state_machine() {
    let scenario = Scenario {
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
        rules: Default::default(),
    };

    let seed = 0x1337_CAFE_u64;
    let expected_trials = match &scenario.layout {
        TargetLayout::Fitts {
            grid,
            trial_timeout_s,
        } => generate_fitts_trials(grid, *trial_timeout_s, seed),
        _ => unreachable!(),
    };

    let mut engine = LayoutEngineState::new(&scenario, seed, 0);
    assert_eq!(engine.total_trials_or_targets(), 96);
    assert!(!engine.is_completed());

    let mut next_tid = 1u32;
    let mut spawned_trials = Vec::new();

    for (expected_idx, expected_trial) in expected_trials.iter().enumerate() {
        // Step 1: Must spawn Home target
        let home_action = engine.handle_fitts_spawn(&mut next_tid, 1000, 15.0);
        match home_action {
            Some(FittsSpawnAction::SpawnHome {
                target_id,
                distance_m,
            }) => {
                assert_eq!(target_id, (expected_idx * 2 + 1) as u32);
                assert!((distance_m - 15.0).abs() < 1e-5);
            }
            _ => panic!("Expected SpawnHome at step {}", expected_idx),
        }

        // Home target hit
        let home_outcome = engine.handle_fitts_hit(true, 500_000);
        assert_eq!(home_outcome, Some(FittsHitOutcome::HomeHit));

        // Step 2: Must spawn Trial target matching expected sequence
        let trial_action = engine.handle_fitts_spawn(&mut next_tid, 500_100, 15.0);
        match trial_action {
            Some(FittsSpawnAction::SpawnTrial {
                target_id,
                trial,
                distance_m,
            }) => {
                assert_eq!(target_id, (expected_idx * 2 + 2) as u32);
                assert!((distance_m - 15.0).abs() < 1e-5);
                assert_eq!(trial.index, expected_idx);
                assert_eq!(trial.distance_deg, expected_trial.distance_deg);
                assert_eq!(trial.radius_deg, expected_trial.radius_deg);
                assert_eq!(trial.octant, expected_trial.octant);
                assert_eq!(trial.yaw_deg, expected_trial.yaw_deg);
                assert_eq!(trial.pitch_deg, expected_trial.pitch_deg);
                spawned_trials.push(trial);
            }
            _ => panic!("Expected SpawnTrial at step {}", expected_idx),
        }

        // Trial target hit
        let trial_outcome = engine.handle_fitts_hit(false, 900_000);
        let is_last = expected_idx == expected_trials.len() - 1;
        match trial_outcome {
            Some(FittsHitOutcome::TrialHit {
                trial_idx,
                trial_elapsed_s,
                completed,
            }) => {
                assert_eq!(trial_idx, expected_idx);
                assert!((trial_elapsed_s - 0.3999).abs() < 1e-3);
                assert_eq!(completed, is_last);
            }
            _ => panic!("Expected TrialHit at step {}", expected_idx),
        }
    }

    assert_eq!(spawned_trials.len(), expected_trials.len());
    assert!(engine.is_completed());
}

#[test]
fn after_a_miss_the_cycle_goes_home_then_to_the_next_target() {
    use trainer::layout_engine::{FittsSpawnAction, LayoutEngineState};
    let scenario = aim_scenarios::schema::Scenario::from_ron_str(
        &std::fs::read_to_string("../../content/scenarios/assess.v1.warmup.ron").unwrap(),
    )
    .unwrap();
    let mut engine = LayoutEngineState::new(&scenario, 7, 0);
    let mut id = 1;
    assert!(engine.awaiting_home());
    assert!(matches!(
        engine.handle_fitts_spawn(&mut id, 0, 15.0),
        Some(FittsSpawnAction::SpawnHome { .. })
    ));
    engine.handle_fitts_hit(true, 100);
    assert!(!engine.awaiting_home());
    assert!(matches!(
        engine.handle_fitts_spawn(&mut id, 200, 15.0),
        Some(FittsSpawnAction::SpawnTrial { .. })
    ));
    engine.handle_fitts_miss();
    assert!(
        engine.awaiting_home(),
        "a miss sends the player back to the centre"
    );
    engine.handle_fitts_hit(true, 900);
    assert!(matches!(
        engine.handle_fitts_spawn(&mut id, 1000, 15.0),
        Some(FittsSpawnAction::SpawnTrial { .. })
    ));
}

#[test]
fn a_slow_return_to_the_centre_does_not_skip_the_next_target() {
    use trainer::layout_engine::{FittsSpawnAction, FittsTimeoutOutcome, LayoutEngineState};
    let scenario = aim_scenarios::schema::Scenario::from_ron_str(
        &std::fs::read_to_string("../../content/scenarios/assess.v1.flick_static.ron").unwrap(),
    )
    .unwrap();
    let mut engine = LayoutEngineState::new(&scenario, 7, 0);
    let mut id = 1;
    engine.handle_fitts_spawn(&mut id, 0, 15.0);
    engine.handle_fitts_hit(true, 100_000);
    engine.handle_fitts_spawn(&mut id, 100_000, 15.0);
    engine.handle_fitts_miss();
    // The player takes 10 s to click the centre dot; the trial timeout is 3 s.
    engine.handle_fitts_hit(true, 10_100_000);
    assert_eq!(
        engine.handle_fitts_timeout(10_101_000),
        FittsTimeoutOutcome::NoTimeout
    );
    assert!(matches!(
        engine.handle_fitts_spawn(&mut id, 10_101_000, 15.0),
        Some(FittsSpawnAction::SpawnTrial { .. })
    ));
}
