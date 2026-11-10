use aim_core::camera::angular_distance;
use aim_scenarios::schema::*;
use trainer::layout_engine::LayoutEngineState;

#[test]
fn test_multi_switch_engine_concurrency_and_octant_distance_balance() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.multi_switch.ron"
    ))
    .unwrap();

    let seed = 0x5A17_F000_0000_0001;
    let mut engine = LayoutEngineState::new(&sc, seed, 0);

    let mut next_tid = 1u32;
    let mut spawned = Vec::new();
    let mut active_ids = Vec::new();

    // Initial fill: must spawn exactly 3 concurrent targets
    for _ in 0..3 {
        let action = engine
            .handle_cluster_spawn(&mut next_tid, 15.0)
            .expect("Must spawn concurrent target");
        active_ids.push(action.target_id);
        spawned.push(action);
    }

    assert_eq!(active_ids.len(), 3);
    assert!(
        engine.handle_cluster_spawn(&mut next_tid, 15.0).is_none(),
        "Must not spawn beyond concurrent_count = 3"
    );

    // Simulate hitting targets one by one until all 48 are completed
    while let Some(hit_id) = active_ids.pop() {
        let outcome = engine
            .handle_cluster_hit(hit_id)
            .expect("Target must be recognized as hit");

        // Replenish up to concurrent_count (3)
        while let Some(replenish_action) = engine.handle_cluster_spawn(&mut next_tid, 15.0) {
            active_ids.push(replenish_action.target_id);
            spawned.push(replenish_action);
        }

        if outcome.completed {
            break;
        }
    }

    assert_eq!(spawned.len(), 48, "Must spawn exactly 48 targets");
    assert!(engine.is_completed(), "Engine must be completed");

    // Real spawned angles verification: distance from center = bin within 1e-6, and inside bounds
    let center = aim_core::camera::ViewAngles::new(0.0, 0.0);
    for a in &spawned {
        let actual_dist = angular_distance(center, a.angles);
        assert!(
            (actual_dist - a.distance_bin_deg).abs() < 1e-6,
            "Target #{} distance from center ({actual_dist}) must equal distance bin ({}) within 1e-6",
            a.target_id, a.distance_bin_deg
        );
        let signed_yaw = if a.angles.yaw > 180.0 {
            a.angles.yaw - 360.0
        } else {
            a.angles.yaw
        };
        assert!(
            signed_yaw.abs() <= 45.0,
            "Target #{} signed yaw {} exceeds max_yaw 45.0°",
            a.target_id,
            signed_yaw
        );
        assert!(
            a.angles.pitch.abs() <= 32.0,
            "Target #{} pitch {} exceeds max_pitch 32.0°",
            a.target_id,
            a.angles.pitch
        );
    }

    // Verify distance bins distribution through the real engine execution (16 each)
    let c10 = spawned
        .iter()
        .filter(|a| (a.distance_bin_deg - 10.0).abs() < 1e-5)
        .count();
    let c20 = spawned
        .iter()
        .filter(|a| (a.distance_bin_deg - 20.0).abs() < 1e-5)
        .count();
    let c30 = spawned
        .iter()
        .filter(|a| (a.distance_bin_deg - 30.0).abs() < 1e-5)
        .count();

    assert_eq!(c10, 16, "Distance bin 10° must have 16 targets");
    assert_eq!(c20, 16, "Distance bin 20° must have 16 targets");
    assert_eq!(c30, 16, "Distance bin 30° must have 16 targets");

    // Verify octant distribution through the real engine execution (6 each)
    for oct in 0..8 {
        let count = spawned.iter().filter(|a| a.octant == oct).count();
        assert_eq!(count, 6, "Octant {oct} must have 6 targets");
    }

    // Verify minimum separation between concurrently active targets
    // Let's re-simulate initial 3 to verify separation constraint
    let mut check_engine = LayoutEngineState::new(&sc, seed, 0);
    let mut check_tid = 1;
    let t1 = check_engine
        .handle_cluster_spawn(&mut check_tid, 15.0)
        .unwrap();
    let t2 = check_engine
        .handle_cluster_spawn(&mut check_tid, 15.0)
        .unwrap();
    let t3 = check_engine
        .handle_cluster_spawn(&mut check_tid, 15.0)
        .unwrap();

    let d12 = angular_distance(t1.angles, t2.angles);
    let d13 = angular_distance(t1.angles, t3.angles);
    let d23 = angular_distance(t2.angles, t3.angles);

    assert!(
        d12 >= 8.0,
        "t1 and t2 separation must be >= 8°, got {}",
        d12
    );
    assert!(
        d13 >= 8.0,
        "t1 and t3 separation must be >= 8°, got {}",
        d13
    );
    assert!(
        d23 >= 8.0,
        "t2 and t3 separation must be >= 8°, got {}",
        d23
    );
}
