use aim_scenarios::schema::*;

#[test]
fn test_concurrent_cluster_validate_fails_when_bin_cannot_fit() {
    let invalid_sc = Scenario {
        id: "test.invalid_cluster_pitch".to_string(),
        version: 1,
        kind: ScenarioKind::Assessment,
        timeout_s: 45.0,
        spawn_distance_m: 15.0,
        target_shape: TargetShape::Sphere,
        layout: TargetLayout::ConcurrentCluster {
            concurrent_count: 3,
            total_targets: 48,
            target_radius_deg: 0.55,
            distance_bins_deg: vec![10.0, 20.0, 35.0],
            octants: 8,
            min_separation_deg: 8.0,
            max_yaw_deg: 45.0,
            max_pitch_deg: 12.0, // 35° bin cannot fit in 12° pitch (octant 2 has pitch 35°)
            duration_s: 45.0,
        },
        hit_rule: HitRule::SingleShotKill,
        rules: Default::default(),
    };

    let res = invalid_sc.validate();
    assert!(
        res.is_err(),
        "Validation must fail when declared distance bin 35° cannot fit within max_pitch 12°"
    );
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("cannot fit") || err_msg.contains("exceeds bounds"),
        "Error message should mention bounds failure, got: {err_msg}"
    );
}

#[test]
fn test_concurrent_cluster_validate_succeeds_when_all_bins_fit() {
    let valid_sc = Scenario {
        id: "test.valid_cluster".to_string(),
        version: 1,
        kind: ScenarioKind::Assessment,
        timeout_s: 45.0,
        spawn_distance_m: 15.0,
        target_shape: TargetShape::Sphere,
        layout: TargetLayout::ConcurrentCluster {
            concurrent_count: 3,
            total_targets: 48,
            target_radius_deg: 0.55,
            distance_bins_deg: vec![10.0, 20.0, 35.0],
            octants: 8,
            min_separation_deg: 8.0,
            max_yaw_deg: 45.0,
            max_pitch_deg: 35.0,
            duration_s: 45.0,
        },
        hit_rule: HitRule::SingleShotKill,
        rules: Default::default(),
    };

    assert!(valid_sc.validate().is_ok());
}
