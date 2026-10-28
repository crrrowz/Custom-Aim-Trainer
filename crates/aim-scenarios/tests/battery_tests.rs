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
            for t_ms in (200..44_000).step_by(500) {
                let t_us = t_ms * 1000;
                let dt_us = 20_000; // 20ms

                let mut integrator = TrajectoryIntegrator::new(trajectory.clone(), seed);
                let p1 = integrator.angles_at(t_us);
                let reflections_before = integrator.cs_reflection_count;
                let p2 = integrator.angles_at(t_us + dt_us);
                let reflections_after = integrator.cs_reflection_count;

                let dist = angular_distance(p1, p2);
                let speed = dist / 0.020; // deg/s

                let in_reflection_window = reflections_after > reflections_before;

                if in_reflection_window {
                    // During chord spanning reflection corner, chord velocity is slightly lower than arc velocity
                    assert!(
                        (10.0..=15.0).contains(&speed),
                        "Reflection chord speed out of bounds: expected [10.0, 15.0], got {speed} at t={t_ms}ms (form {form})"
                    );
                } else {
                    // Outside wall reflections, speed strictly satisfies the ±5% band (13.3 - 14.7 °/s)
                    assert!(
                        (13.3..=14.7).contains(&speed),
                        "Speed deviation outside reflection: expected [13.3, 14.7] (14.0 ±5%), got {speed} at t={t_ms}ms (form {form})"
                    );
                }
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
        assert_eq!(candidates.len(), 48);

        // Verify distance bins distribution (48 / 3 = 16 each)
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

        assert_eq!(c10, 16);
        assert_eq!(c20, 16);
        assert_eq!(c35, 16);

        // Verify octants distribution (48 / 8 = 6 each)
        for octant in 0..8 {
            let count = candidates.iter().filter(|(_, o)| *o == octant).count();
            assert_eq!(count, 6, "Octant {octant} imbalance on form {form}");
        }
    }
}
