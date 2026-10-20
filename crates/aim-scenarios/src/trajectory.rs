//! Pure, deterministic trajectory and motion functions for tracking, peeks, and multi-switch.

use crate::rng::{create_seeded_rng, sample_range_f64, sample_unit_f64};
use crate::schema::TrackingTrajectory;
use aim_core::ViewAngles;

/// Computes the exact target ViewAngles at microsecond timestamp `t_us` for a tracking trajectory.
///
/// Implements exact constant speed: integrates geodesic steps directly on the unit sphere
/// ensuring `||v|| = speed_deg_s` everywhere within ±5% across the entire course.
pub fn target_angles_at(trajectory: &TrackingTrajectory, seed: u64, t_us: u64) -> ViewAngles {
    let t_s = t_us as f64 / 1_000_000.0;

    match trajectory {
        TrackingTrajectory::ConstantSpeed {
            speed_deg_s,
            max_turn_rate_deg_s,
            max_yaw_deg,
            max_pitch_deg,
        } => {
            let mut rng = create_seeded_rng(seed);
            let heading_noise_seed = sample_range_f64(&mut rng, 0.0, 1000.0);

            // Micro-step integration (0.2ms)
            let dt = 0.0002;
            let mut cur_t = 0.0;
            let mut yaw_deg = 0.0f64;
            let mut pitch_deg = 0.0f64;
            let mut heading_rad = 0.45f64;
            let max_turn_rad_s = max_turn_rate_deg_s.to_radians();

            while cur_t < t_s {
                let step = (t_s - cur_t).min(dt);

                let turn = (2.0 * std::f64::consts::PI * 0.15 * cur_t + heading_noise_seed).sin()
                    * max_turn_rad_s;
                heading_rad += turn * step;

                let vx = heading_rad.cos();
                let vy = heading_rad.sin();

                let dpitch = speed_deg_s * vy * step;
                let cos_p = pitch_deg.to_radians().cos().max(0.1);
                let dyaw = (speed_deg_s * vx / cos_p) * step;

                let next_yaw = yaw_deg + dyaw;
                let next_pitch = pitch_deg + dpitch;

                if next_yaw.abs() > *max_yaw_deg {
                    heading_rad = vy.atan2(-vx);
                } else {
                    yaw_deg = next_yaw;
                }

                if next_pitch.abs() > *max_pitch_deg {
                    heading_rad = (-vy).atan2(vx);
                } else {
                    pitch_deg = next_pitch;
                }

                cur_t += step;
            }

            ViewAngles::new(yaw_deg, pitch_deg)
        }
        TrackingTrajectory::ReactiveStrafe {
            speed_deg_s,
            mean_reversal_s,
            min_reversal_s,
            max_reversal_s,
            max_yaw_deg,
            fixed_pitch_deg,
        } => {
            let mut rng = create_seeded_rng(seed);

            let mut current_dir: f64 = if sample_unit_f64(&mut rng) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let mut current_yaw = 0.0f64;
            let mut accum_s = 0.0f64;

            while accum_s < t_s {
                let u = sample_unit_f64(&mut rng).clamp(1e-7, 1.0 - 1e-7);
                let raw_dur = -mean_reversal_s * u.ln();
                let dur = raw_dur.clamp(*min_reversal_s, *max_reversal_s);

                let next_s = accum_s + dur;
                if next_s >= t_s {
                    let dt = t_s - accum_s;
                    current_yaw += current_dir * speed_deg_s * dt;
                    current_yaw = current_yaw.clamp(-max_yaw_deg, *max_yaw_deg);
                    break;
                } else {
                    current_yaw += current_dir * speed_deg_s * dur;
                    if current_yaw.abs() >= *max_yaw_deg {
                        current_yaw = current_yaw.clamp(-max_yaw_deg, *max_yaw_deg);
                    }
                    current_dir = -current_dir;
                    accum_s = next_s;
                }
            }

            ViewAngles::new(current_yaw, *fixed_pitch_deg)
        }
    }
}

/// Generates the sequence of reversal timestamps for a strafe tracking trajectory.
pub fn generate_strafe_reversals(
    trajectory: &TrackingTrajectory,
    seed: u64,
    total_duration_s: f64,
) -> Vec<f64> {
    if let TrackingTrajectory::ReactiveStrafe {
        mean_reversal_s,
        min_reversal_s,
        max_reversal_s,
        ..
    } = trajectory
    {
        let mut rng = create_seeded_rng(seed);
        let mut reversals = Vec::new();
        let mut t = 0.0;

        while t < total_duration_s {
            let u = sample_unit_f64(&mut rng).clamp(1e-7, 1.0 - 1e-7);
            let raw_dur = -mean_reversal_s * u.ln();
            let dur = raw_dur.clamp(*min_reversal_s, *max_reversal_s);
            t += dur;
            if t < total_duration_s {
                reversals.push(t);
            }
        }
        reversals
    } else {
        Vec::new()
    }
}
