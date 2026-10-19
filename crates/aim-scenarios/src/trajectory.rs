//! Pure, deterministic trajectory and motion functions for tracking, peeks, and multi-switch.

use crate::rng::{create_seeded_rng, sample_range_f64, sample_unit_f64};
use crate::schema::TrackingTrajectory;
use aim_core::ViewAngles;

/// Computes the exact target ViewAngles at microsecond timestamp `t_us` for a tracking trajectory.
///
/// Implements exact constant speed: integrates arc length `s = speed_deg_s * t`,
/// keeping `||v|| = speed_deg_s` everywhere across the continuous path.
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

            // Integrate trajectory with small time step to guarantee exact arc-length speed
            let dt = 0.005; // 5ms step
            let mut cur_t = 0.0;
            let mut x = 0.0f64;
            let mut y = 0.0f64;
            let mut heading_rad = 0.0f64;

            let max_turn_rate_rad = max_turn_rate_deg_s.to_radians();

            while cur_t < t_s {
                let step = (t_s - cur_t).min(dt);
                // Heading rate of change modulated smoothly
                let turn = (2.0 * std::f64::consts::PI * 0.25 * cur_t + heading_noise_seed).sin()
                    * max_turn_rate_rad;
                heading_rad += turn * step;

                let dx = speed_deg_s * heading_rad.cos() * step;
                let dy = speed_deg_s * heading_rad.sin() * step;

                x += dx;
                y += dy;

                cur_t += step;
            }

            // Smooth triangular wave reflection across bounds
            let yaw_span = 2.0 * max_yaw_deg;
            let pitch_span = 2.0 * max_pitch_deg;

            let norm_x = (x % (2.0 * yaw_span) + 2.0 * yaw_span) % (2.0 * yaw_span);
            let yaw = if norm_x > yaw_span {
                2.0 * yaw_span - norm_x - max_yaw_deg
            } else {
                norm_x - max_yaw_deg
            };

            let norm_y = (y % (2.0 * pitch_span) + 2.0 * pitch_span) % (2.0 * pitch_span);
            let pitch = if norm_y > pitch_span {
                2.0 * pitch_span - norm_y - max_pitch_deg
            } else {
                norm_y - max_pitch_deg
            };

            ViewAngles::new(yaw, pitch)
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
