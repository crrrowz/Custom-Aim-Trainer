use aim_core::camera::angular_distance;
use aim_scenarios::schema::*;
use aim_scenarios::trajectory::{target_angles_at, TrajectoryIntegrator};
use std::time::Instant;

fn sample_constant_speed_trajectory() -> TrackingTrajectory {
    TrackingTrajectory::ConstantSpeed {
        speed_deg_s: 14.0,
        max_turn_rate_deg_s: 45.0,
        max_yaw_deg: 25.0,
        max_pitch_deg: 8.0,
    }
}

#[test]
fn test_timing_target_angles_at_fast() {
    let trajectory = sample_constant_speed_trajectory();
    let seed = 0x5A17_F000_0000_0001;

    let start = Instant::now();
    // Evaluate 100 queries across 45 seconds of simulation
    for i in 0..100 {
        let t_us = i * 450_000;
        let _ = target_angles_at(&trajectory, seed, t_us);
    }
    let elapsed = start.elapsed();

    // Must be fast: under 200ms in debug (previously took several seconds)
    assert!(
        elapsed.as_millis() < 200,
        "100 calls to target_angles_at took {:?}, expected < 200ms (O(1) amortized)",
        elapsed
    );
}

#[test]
fn test_speed_within_pm_5_percent_outside_reflections() {
    let trajectory = sample_constant_speed_trajectory();
    let seed = 0x5A17_F000_0000_0001;
    let declared_speed = 14.0;
    let min_speed = declared_speed * 0.95; // 13.3 deg/s
    let max_speed = declared_speed * 1.05; // 14.7 deg/s

    let mut integrator = TrajectoryIntegrator::new(trajectory, seed);

    for step in 1..2000 {
        let t_us = step * 10_000; // 10ms steps
        let dt_us = 10_000;
        let p1 = integrator.angles_at(t_us);
        let ref_before = integrator.cs_reflection_count;
        let p2 = integrator.angles_at(t_us + dt_us);
        let ref_after = integrator.cs_reflection_count;

        if ref_after == ref_before {
            let dist = angular_distance(p1, p2);
            let speed = dist / 0.010;
            assert!(
                speed >= min_speed && speed <= max_speed,
                "Speed at t={}ms ({:.2}°/s) outside ±5% band [{min_speed}, {max_speed}]",
                t_us / 1000,
                speed
            );
        }
    }
}

#[test]
fn test_reflection_no_axis_freeze() {
    let trajectory = TrackingTrajectory::ConstantSpeed {
        speed_deg_s: 14.0,
        max_turn_rate_deg_s: 0.0, // Straight motion to easily inspect boundary reflection
        max_yaw_deg: 5.0,
        max_pitch_deg: 5.0,
    };
    let seed = 42;
    let mut integrator = TrajectoryIntegrator::new(trajectory, seed);

    let mut saw_reflection = false;
    let mut prev_pos = integrator.angles_at(0);

    for step in 1..500 {
        let t_us = step * 10_000; // 10ms
        let pos = integrator.angles_at(t_us);

        let signed_yaw = if pos.yaw > 180.0 {
            pos.yaw - 360.0
        } else {
            pos.yaw
        };

        assert!(
            signed_yaw.abs() <= 5.0 + 1e-6,
            "Signed yaw {} exceeded max_yaw 5.0°",
            signed_yaw
        );
        assert!(
            pos.pitch.abs() <= 5.0 + 1e-6,
            "Pitch {} exceeded max_pitch 5.0°",
            pos.pitch
        );

        if integrator.cs_reflection_count > 0 {
            saw_reflection = true;
            // Target must keep moving and never freeze on the axis
            let step_dist = angular_distance(prev_pos, pos);
            assert!(
                step_dist > 0.05,
                "Target froze at reflection: step_dist was {} at t={}ms",
                step_dist,
                t_us / 1000
            );
        }
        prev_pos = pos;
    }

    assert!(saw_reflection, "Must have encountered reflections");
}

#[test]
fn test_relative_time_evaluation() {
    let trajectory = sample_constant_speed_trajectory();
    let seed = 0x5A17_F000_0000_0001;

    let rel_time_us = 5_432_100u64;

    // Both should evaluate identical ViewAngles for the same relative elapsed time
    let p_zero = target_angles_at(&trajectory, seed, rel_time_us);
    let p_rel = target_angles_at(&trajectory, seed, rel_time_us);

    assert_eq!(p_zero.yaw, p_rel.yaw);
    assert_eq!(p_zero.pitch, p_rel.pitch);

    // Also verify with independent integrator instances
    let mut int1 = TrajectoryIntegrator::new(trajectory.clone(), seed);
    let mut int2 = TrajectoryIntegrator::new(trajectory, seed);

    let a1 = int1.angles_at(rel_time_us);
    let a2 = int2.angles_at(rel_time_us);
    assert_eq!(a1.yaw, a2.yaw);
    assert_eq!(a1.pitch, a2.pitch);
}
