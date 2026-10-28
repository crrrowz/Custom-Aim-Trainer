use aim_scenarios::schema::*;
use aim_scenarios::trajectory::{target_angles_at, TrajectoryIntegrator};

#[test]
fn test_incremental_integrator_matches_reference_within_1e_9() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.smooth_track.ron"
    ))
    .unwrap();

    let trajectory = match &sc.layout {
        TargetLayout::Tracking { trajectory, .. } => trajectory.clone(),
        _ => panic!("Expected tracking layout"),
    };

    let seed = 0x5A17_F000_0000_0001;

    let mut integrator = TrajectoryIntegrator::new(trajectory.clone(), seed);

    // Advance frame-by-frame like a 240Hz simulation (~4166us per frame)
    for frame in 1..=1200 {
        let t_us = frame * 4166;
        let incremental_angles = integrator.angles_at(t_us);
        let reference_angles = target_angles_at(&trajectory, seed, t_us);

        let yaw_diff = (incremental_angles.yaw - reference_angles.yaw).abs();
        let pitch_diff = (incremental_angles.pitch - reference_angles.pitch).abs();

        assert!(
            yaw_diff <= 1e-9,
            "Yaw difference at t={}us (frame {}): incremental={}, reference={}, diff={}",
            t_us,
            frame,
            incremental_angles.yaw,
            reference_angles.yaw,
            yaw_diff
        );
        assert!(
            pitch_diff <= 1e-9,
            "Pitch difference at t={}us (frame {}): incremental={}, reference={}, diff={}",
            t_us,
            frame,
            incremental_angles.pitch,
            reference_angles.pitch,
            pitch_diff
        );
    }
}
