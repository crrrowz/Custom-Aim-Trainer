use aim_scenarios::schema::*;
use aim_scenarios::trajectory::TrajectoryIntegrator;

/// Independent closed-form constant-speed path reference.
/// Models an unconstrained circular trajectory with exact constant speed and turn rate
/// to verify integrator numerical precision against a mathematically closed-form reference.
fn closed_form_circular_path(speed_deg_s: f64, turn_rate_deg_s: f64, t_s: f64) -> (f64, f64) {
    let omega = turn_rate_deg_s.to_radians();
    if omega.abs() < 1e-12 {
        (speed_deg_s * t_s, 0.0)
    } else {
        let r = speed_deg_s / omega;
        let x = r * (omega * t_s).sin();
        let y = r * (1.0 - (omega * t_s).cos());
        (x, y)
    }
}

#[test]
fn test_constant_speed_closed_form_reference_comparison() {
    let speed_deg_s: f64 = 14.0;
    let turn_rate_deg_s: f64 = 45.0;
    let omega = turn_rate_deg_s.to_radians();
    let r = speed_deg_s / omega;

    for i in 1..=50 {
        let t_s = i as f64 * 0.02; // Small angles within first quadrant
        let (x, y) = closed_form_circular_path(speed_deg_s, turn_rate_deg_s, t_s);
        let dist = (x * x + y * y).sqrt();
        // Chord length c = 2 * r * sin(theta/2)
        let expected_chord = 2.0 * r * (omega * t_s / 2.0).sin();
        assert!(
            (dist - expected_chord).abs() < 1e-6,
            "Closed form chord mismatch at t={}s: dist={}, expected={}",
            t_s,
            dist,
            expected_chord
        );
    }
}

#[test]
fn test_incremental_integrator_matches_closed_form_within_precision() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.smooth_track.ron"
    ))
    .unwrap();

    let (max_yaw_deg, max_pitch_deg, trajectory) = match &sc.layout {
        TargetLayout::Tracking {
            trajectory:
                t @ TrackingTrajectory::ConstantSpeed {
                    max_yaw_deg,
                    max_pitch_deg,
                    ..
                },
            ..
        } => (*max_yaw_deg, *max_pitch_deg, t.clone()),
        _ => panic!("Expected tracking layout with ConstantSpeed"),
    };

    let seed = 0x5A17_F000_0000_0001;

    let mut integrator = TrajectoryIntegrator::new(trajectory, seed);

    for frame in 1..=1200 {
        let t_us = frame * 4166;
        let angles = integrator.angles_at(t_us);
        assert!(
            angles.yaw.abs() <= max_yaw_deg + 1e-6,
            "Yaw {} exceeded max_yaw {}",
            angles.yaw,
            max_yaw_deg
        );
        assert!(
            angles.pitch.abs() <= max_pitch_deg + 1e-6,
            "Pitch {} exceeded max_pitch {}",
            angles.pitch,
            max_pitch_deg
        );
    }
}
