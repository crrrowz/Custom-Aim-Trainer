use aim_core::ViewAngles;
use aim_scenarios::rng::{create_seeded_rng, sample_range_f64};
use aim_scenarios::schema::*;
use aim_scenarios::trajectory::TrajectoryIntegrator;

/// Independent ground-truth pre-integrator loop from t=0.
fn reference_constant_speed_from_zero(
    speed_deg_s: f64,
    max_turn_rate_deg_s: f64,
    max_yaw_deg: f64,
    max_pitch_deg: f64,
    seed: u64,
    t_us: u64,
) -> ViewAngles {
    let mut rng = create_seeded_rng(seed);
    let heading_noise_seed = sample_range_f64(&mut rng, 0.0, 1000.0);

    let dt = 0.0002;
    let max_turn_rad_s = max_turn_rate_deg_s.to_radians();
    let target_full_steps = t_us / 200;
    let remainder_s = (t_us % 200) as f64 / 1_000_000.0;

    let mut heading_rad: f64 = 0.45;
    let mut yaw_deg: f64 = 0.0;
    let mut pitch_deg: f64 = 0.0;

    for step in 0..target_full_steps {
        let cur_t = step as f64 * dt;
        let turn =
            (2.0 * std::f64::consts::PI * 0.15 * cur_t + heading_noise_seed).sin() * max_turn_rad_s;
        heading_rad += turn * dt;

        let vx = heading_rad.cos();
        let vy = heading_rad.sin();

        let dpitch = speed_deg_s * vy * dt;
        let cos_p = pitch_deg.to_radians().cos().max(0.1);
        let dyaw = (speed_deg_s * vx / cos_p) * dt;

        let next_yaw = yaw_deg + dyaw;
        let next_pitch = pitch_deg + dpitch;

        if next_yaw > max_yaw_deg {
            yaw_deg = 2.0 * max_yaw_deg - next_yaw;
            heading_rad = vy.atan2(-vx);
        } else if next_yaw < -max_yaw_deg {
            yaw_deg = -2.0 * max_yaw_deg - next_yaw;
            heading_rad = vy.atan2(-vx);
        } else {
            yaw_deg = next_yaw;
        }

        if next_pitch > max_pitch_deg {
            pitch_deg = 2.0 * max_pitch_deg - next_pitch;
            heading_rad = (-vy).atan2(vx);
        } else if next_pitch < -max_pitch_deg {
            pitch_deg = -2.0 * max_pitch_deg - next_pitch;
            heading_rad = (-vy).atan2(vx);
        } else {
            pitch_deg = next_pitch;
        }
    }

    if remainder_s > 0.0 {
        let cur_t = target_full_steps as f64 * dt;
        let turn =
            (2.0 * std::f64::consts::PI * 0.15 * cur_t + heading_noise_seed).sin() * max_turn_rad_s;
        let h_rad = heading_rad + turn * remainder_s;
        let vx = h_rad.cos();
        let vy = h_rad.sin();
        let dpitch = speed_deg_s * vy * remainder_s;
        let cos_p = pitch_deg.to_radians().cos().max(0.1);
        let dyaw = (speed_deg_s * vx / cos_p) * remainder_s;

        let next_yaw = yaw_deg + dyaw;
        let next_pitch = pitch_deg + dpitch;

        let yaw = if next_yaw > max_yaw_deg {
            2.0 * max_yaw_deg - next_yaw
        } else if next_yaw < -max_yaw_deg {
            -2.0 * max_yaw_deg - next_yaw
        } else {
            next_yaw
        };

        let pitch = if next_pitch > max_pitch_deg {
            2.0 * max_pitch_deg - next_pitch
        } else if next_pitch < -max_pitch_deg {
            -2.0 * max_pitch_deg - next_pitch
        } else {
            next_pitch
        };

        ViewAngles::new(yaw, pitch)
    } else {
        ViewAngles::new(yaw_deg, pitch_deg)
    }
}

#[test]
fn test_incremental_integrator_matches_reference_within_1e_9() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.smooth_track.ron"
    ))
    .unwrap();

    let (speed_deg_s, max_turn_rate_deg_s, max_yaw_deg, max_pitch_deg, trajectory) =
        match &sc.layout {
            TargetLayout::Tracking {
                trajectory:
                    t @ TrackingTrajectory::ConstantSpeed {
                        speed_deg_s,
                        max_turn_rate_deg_s,
                        max_yaw_deg,
                        max_pitch_deg,
                    },
                ..
            } => (
                *speed_deg_s,
                *max_turn_rate_deg_s,
                *max_yaw_deg,
                *max_pitch_deg,
                t.clone(),
            ),
            _ => panic!("Expected tracking layout with ConstantSpeed"),
        };

    let seed = 0x5A17_F000_0000_0001;

    let mut integrator = TrajectoryIntegrator::new(trajectory, seed);

    // Advance frame-by-frame like a 240Hz simulation (~4166us per frame)
    for frame in 1..=1200 {
        let t_us = frame * 4166;
        let incremental_angles = integrator.angles_at(t_us);
        let reference_angles = reference_constant_speed_from_zero(
            speed_deg_s,
            max_turn_rate_deg_s,
            max_yaw_deg,
            max_pitch_deg,
            seed,
            t_us,
        );

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
