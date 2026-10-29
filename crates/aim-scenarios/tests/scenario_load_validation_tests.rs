use aim_scenarios::schema::Scenario;

#[test]
fn test_load_scenario_invalid_tracking_single_shot_fails() {
    let broken_ron = r#"
Scenario(
    id: "broken.tracking.singleshot",
    version: 1,
    kind: Assessment,
    timeout_s: 45.0,
    spawn_distance_m: 15.0,
    target_shape: Sphere,
    layout: Tracking(
        target_radius_deg: 0.6,
        trajectory: ConstantSpeed(
            speed_deg_s: 14.0,
            max_turn_rate_deg_s: 24.0,
            max_yaw_deg: 35.0,
            max_pitch_deg: 6.0,
        ),
        duration_s: 45.0,
    ),
    hit_rule: SingleShotKill,
)
"#;
    let res = Scenario::from_ron_str(broken_ron);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("Tracking layout requires ContinuousTracking"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_load_scenario_invalid_fitts_continuous_tracking_fails() {
    let broken_ron = r#"
Scenario(
    id: "broken.fitts.continuous",
    version: 1,
    kind: Assessment,
    timeout_s: 45.0,
    spawn_distance_m: 15.0,
    target_shape: Sphere,
    layout: Fitts(
        grid: (
            distances_deg: [8.0, 20.0],
            radius_sizes_deg: [0.4],
            octants: 8,
            repetitions_per_cell: 1,
            anchor: HomeTargetCenter,
        ),
        trial_timeout_s: 3.0,
    ),
    hit_rule: ContinuousTracking,
)
"#;
    let res = Scenario::from_ron_str(broken_ron);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("Fitts layout requires SingleShotKill"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_load_scenario_invalid_peeks_continuous_tracking_fails() {
    let broken_ron = r#"
Scenario(
    id: "broken.peeks.continuous",
    version: 1,
    kind: Assessment,
    timeout_s: 45.0,
    spawn_distance_m: 15.0,
    target_shape: WallPeek,
    layout: HeadLevelPeeks(
        peeks_count: 10,
        occluder_yaws_deg: [-25.0, 25.0],
        target_radius_deg: 0.45,
        head_pitch_deg: 1.25,
        pitch_noise_std_deg: 0.4,
        pitch_noise_clamp_deg: 0.8,
        peek_speed_mps: 5.73,
        trial_timeout_s: 2.5,
    ),
    hit_rule: ContinuousTracking,
)
"#;
    let res = Scenario::from_ron_str(broken_ron);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("HeadLevelPeeks layout requires SingleShotKill"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_load_scenario_invalid_cluster_out_of_bounds_bin_fails() {
    let broken_ron = r#"
Scenario(
    id: "broken.cluster.bin",
    version: 1,
    kind: Assessment,
    timeout_s: 45.0,
    spawn_distance_m: 15.0,
    target_shape: Sphere,
    layout: ConcurrentCluster(
        concurrent_count: 3,
        total_targets: 48,
        target_radius_deg: 0.55,
        distance_bins_deg: [10.0, 20.0, 35.0],
        octants: 8,
        min_separation_deg: 8.0,
        max_yaw_deg: 45.0,
        max_pitch_deg: 10.0,
        duration_s: 45.0,
    ),
    hit_rule: SingleShotKill,
)
"#;
    let res = Scenario::from_ron_str(broken_ron);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.contains("cannot fit within max_yaw"),
        "Unexpected error: {err}"
    );
}

#[test]
fn test_load_scenario_valid_succeeds() {
    let valid_ron = include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
    let res = Scenario::from_ron_str(valid_ron);
    assert!(
        res.is_ok(),
        "Valid scenario flick_static must load successfully"
    );
}
