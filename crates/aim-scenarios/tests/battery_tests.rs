use aim_core::camera::angular_distance;
use aim_scenarios::*;

#[test]
fn test_all_battery_scenarios_parse_and_validate() {
    let files = [
        include_str!("../../../content/scenarios/assess.v1.warmup.ron"),
        include_str!("../../../content/scenarios/assess.v1.flick_static.ron"),
        include_str!("../../../content/scenarios/assess.v1.micro_precision.ron"),
        include_str!("../../../content/scenarios/assess.v1.head_level_peeks.ron"),
        include_str!("../../../content/scenarios/assess.v1.smooth_track.ron"),
        include_str!("../../../content/scenarios/assess.v1.strafe_track.ron"),
        include_str!("../../../content/scenarios/assess.v1.multi_switch.ron"),
    ];

    for content in files {
        let sc: Scenario = ron::from_str(content).expect("Failed parsing scenario");
        assert!(sc.validate().is_ok(), "Validation failed for {}", sc.id);
    }
}

#[test]
fn test_battery_manifest_parse() {
    let manifest: BatteryManifest =
        ron::from_str(include_str!("../../../content/batteries/battery.v1.ron"))
            .expect("Failed parsing battery.v1.ron");
    assert_eq!(manifest.stages.len(), 7);
}

#[test]
fn test_a4_constant_speed_bounds_and_speed_stability() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.smooth_track.ron"
    ))
    .unwrap();
    if let TargetLayout::Tracking { trajectory, .. } = sc.layout {
        for form in 0..5 {
            let seed = derive_scenario_seed(form, &sc.id);
            for t_ms in (0..45_000).step_by(100) {
                let t_us = t_ms * 1000;
                let dt_us = 10_000; // 10ms
                let p1 = target_angles_at(&trajectory, seed, t_us);
                let p2 = target_angles_at(&trajectory, seed, t_us + dt_us);

                let dist = angular_distance(p1, p2);
                let speed = dist / 0.010; // deg/s

                // Verify angular speed remains within ±15% of 14°/s across reflections
                assert!(
                    (speed - 14.0).abs() < 2.5,
                    "Speed deviation: expected ~14.0, got {speed} at t={t_ms}ms"
                );
            }
        }
    } else {
        panic!("Wrong layout");
    }
}

#[test]
fn test_a5_strafe_reversals_count_and_reliability() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.strafe_track.ron"
    ))
    .unwrap();
    if let TargetLayout::Tracking { trajectory, .. } = sc.layout {
        for form in 0..5 {
            let seed = derive_scenario_seed(form, &sc.id);
            let reversals = generate_strafe_reversals(&trajectory, seed, 45.0);
            assert!(
                reversals.len() >= 60,
                "Form {form} had only {} reversals, expected >= 60 for S07 reliability",
                reversals.len()
            );
        }
    } else {
        panic!("Wrong layout");
    }
}

#[test]
fn test_a6_cluster_distribution_balance() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.multi_switch.ron"
    ))
    .unwrap();
    for form in 0..5 {
        let seed = derive_scenario_seed(form, &sc.id);
        let candidates = generate_cluster_candidates(&sc, seed);
        assert_eq!(candidates.len(), 45);

        // Verify distance bins distribution
        let c10 = candidates
            .iter()
            .filter(|(d, _)| (*d - 10.0).abs() < 1e-5)
            .count();
        let c20 = candidates
            .iter()
            .filter(|(d, _)| (*d - 20.0).abs() < 1e-5)
            .count();
        let c35 = candidates
            .iter()
            .filter(|(d, _)| (*d - 35.0).abs() < 1e-5)
            .count();

        assert_eq!(c10, 15);
        assert_eq!(c20, 15);
        assert_eq!(c35, 15);
    }
}
