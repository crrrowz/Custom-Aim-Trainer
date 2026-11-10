use aim_scenarios::schema::Scenario;
use trainer::{load_scenario_by_id, resolve_scenario_duration, VALID_ASSESS_SCENARIO_IDS};

#[test]
fn test_all_assess_v1_scenarios_resolve_expected_duration() {
    let test_cases = [
        (
            include_str!("../../../content/scenarios/assess.v1.warmup.ron"),
            16.0 * 3.5 + 10.0,
        ), // 66.0s
        (
            include_str!("../../../content/scenarios/assess.v1.flick_static.ron"),
            96.0 * 3.0 + 10.0,
        ), // 298.0s
        (
            include_str!("../../../content/scenarios/assess.v1.micro_precision.ron"),
            48.0 * 2.5 + 10.0,
        ), // 130.0s
        (
            include_str!("../../../content/scenarios/assess.v1.head_level_peeks.ron"),
            36.0 * 2.5 + 10.0,
        ), // 100.0s
        (
            include_str!("../../../content/scenarios/assess.v1.smooth_track.ron"),
            45.0,
        ),
        (
            include_str!("../../../content/scenarios/assess.v1.strafe_track.ron"),
            45.0,
        ),
        (
            include_str!("../../../content/scenarios/assess.v1.multi_switch.ron"),
            45.0,
        ),
    ];

    for (ron_str, expected_duration) in test_cases {
        let sc: Scenario =
            ron::from_str(ron_str).unwrap_or_else(|e| panic!("Failed parsing scenario ron: {}", e));

        let duration = resolve_scenario_duration(&sc, None);
        assert!(
            (duration - expected_duration).abs() < 1e-6,
            "Scenario {} expected duration {}s, got {}s",
            sc.id,
            expected_duration,
            duration
        );

        // Verify override wins
        let override_dur = 12.34;
        assert_eq!(
            resolve_scenario_duration(&sc, Some(override_dur)),
            override_dur
        );
    }
}

#[test]
fn test_unknown_scenario_returns_error_listing_valid_ids() {
    let err = load_scenario_by_id("non_existent_scenario_12345").unwrap_err();
    assert!(
        err.contains("Unknown scenario 'non_existent_scenario_12345'"),
        "Error did not name unknown scenario: {}",
        err
    );
    for valid_id in VALID_ASSESS_SCENARIO_IDS {
        assert!(
            err.contains(valid_id),
            "Error did not list valid scenario id {}: {}",
            valid_id,
            err
        );
    }
}
