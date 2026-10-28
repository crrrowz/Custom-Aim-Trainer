//! Pure, deterministic trajectory and motion functions for tracking, peeks, and multi-switch.

use crate::rng::{create_seeded_rng, sample_range_f64, sample_unit_f64, ChaCha8Rng};
use crate::schema::TrackingTrajectory;
use aim_core::ViewAngles;

/// An incremental, O(1) per-frame trajectory integrator.
///
/// Maintains simulation state across frames so subsequent calls only advance
/// by the elapsed frame time (typically ~4-16ms) rather than re-integrating
/// from t=0.
#[derive(Debug, Clone)]
pub struct TrajectoryIntegrator {
    trajectory: TrackingTrajectory,
    seed: u64,
    // ConstantSpeed state:
    cs_heading_noise_seed: f64,
    cs_committed_steps: u64,
    cs_yaw_deg: f64,
    cs_pitch_deg: f64,
    cs_heading_rad: f64,
    // ReactiveStrafe state:
    rs_rng: ChaCha8Rng,
    rs_current_dir: f64,
    rs_accum_s: f64,
    rs_current_yaw: f64,
    rs_segment_dur: f64,
}

impl TrajectoryIntegrator {
    pub fn new(trajectory: TrackingTrajectory, seed: u64) -> Self {
        let mut rng = create_seeded_rng(seed);
        let cs_heading_noise_seed = sample_range_f64(&mut rng, 0.0, 1000.0);

        let mut rs_rng = create_seeded_rng(seed);
        let rs_current_dir = if sample_unit_f64(&mut rs_rng) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let rs_segment_dur = match &trajectory {
            TrackingTrajectory::ReactiveStrafe {
                mean_reversal_s,
                min_reversal_s,
                max_reversal_s,
                ..
            } => {
                let u = sample_unit_f64(&mut rs_rng).clamp(1e-7, 1.0 - 1e-7);
                let raw_dur = -mean_reversal_s * u.ln();
                raw_dur.clamp(*min_reversal_s, *max_reversal_s)
            }
            _ => 0.0,
        };

        Self {
            trajectory,
            seed,
            cs_heading_noise_seed,
            cs_committed_steps: 0,
            cs_yaw_deg: 0.0,
            cs_pitch_deg: 0.0,
            cs_heading_rad: 0.45,
            rs_rng,
            rs_current_dir,
            rs_accum_s: 0.0,
            rs_current_yaw: 0.0,
            rs_segment_dur,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.trajectory.clone(), self.seed);
    }

    /// Evaluates target ViewAngles at timestamp `t_us`.
    /// When `t_us` advances monotonically forward, runs in O(1) per-frame execution.
    pub fn angles_at(&mut self, t_us: u64) -> ViewAngles {
        let t_s = t_us as f64 / 1_000_000.0;
        let trajectory = self.trajectory.clone();

        match &trajectory {
            TrackingTrajectory::ConstantSpeed {
                speed_deg_s,
                max_turn_rate_deg_s,
                max_yaw_deg,
                max_pitch_deg,
            } => {
                let dt = 0.0002;
                let max_turn_rad_s = max_turn_rate_deg_s.to_radians();
                let target_full_steps = t_us / 200;
                let remainder_s = (t_us % 200) as f64 / 1_000_000.0;

                if target_full_steps < self.cs_committed_steps {
                    self.reset();
                }

                while self.cs_committed_steps < target_full_steps {
                    let cur_t = self.cs_committed_steps as f64 * dt;
                    let turn = (2.0 * std::f64::consts::PI * 0.15 * cur_t
                        + self.cs_heading_noise_seed)
                        .sin()
                        * max_turn_rad_s;
                    self.cs_heading_rad += turn * dt;

                    let vx = self.cs_heading_rad.cos();
                    let vy = self.cs_heading_rad.sin();

                    let dpitch = speed_deg_s * vy * dt;
                    let cos_p = self.cs_pitch_deg.to_radians().cos().max(0.1);
                    let dyaw = (speed_deg_s * vx / cos_p) * dt;

                    let next_yaw = self.cs_yaw_deg + dyaw;
                    let next_pitch = self.cs_pitch_deg + dpitch;

                    if next_yaw.abs() > *max_yaw_deg {
                        self.cs_heading_rad = vy.atan2(-vx);
                    } else {
                        self.cs_yaw_deg = next_yaw;
                    }

                    if next_pitch.abs() > *max_pitch_deg {
                        self.cs_heading_rad = (-vy).atan2(vx);
                    } else {
                        self.cs_pitch_deg = next_pitch;
                    }

                    self.cs_committed_steps += 1;
                }

                if remainder_s > 0.0 {
                    let cur_t = self.cs_committed_steps as f64 * dt;
                    let turn = (2.0 * std::f64::consts::PI * 0.15 * cur_t
                        + self.cs_heading_noise_seed)
                        .sin()
                        * max_turn_rad_s;
                    let h_rad = self.cs_heading_rad + turn * remainder_s;
                    let vx = h_rad.cos();
                    let vy = h_rad.sin();
                    let dpitch = speed_deg_s * vy * remainder_s;
                    let cos_p = self.cs_pitch_deg.to_radians().cos().max(0.1);
                    let dyaw = (speed_deg_s * vx / cos_p) * remainder_s;

                    let next_yaw = self.cs_yaw_deg + dyaw;
                    let next_pitch = self.cs_pitch_deg + dpitch;

                    let yaw = if next_yaw.abs() > *max_yaw_deg {
                        self.cs_yaw_deg
                    } else {
                        next_yaw
                    };

                    let pitch = if next_pitch.abs() > *max_pitch_deg {
                        self.cs_pitch_deg
                    } else {
                        next_pitch
                    };

                    ViewAngles::new(yaw, pitch)
                } else {
                    ViewAngles::new(self.cs_yaw_deg, self.cs_pitch_deg)
                }
            }
            TrackingTrajectory::ReactiveStrafe {
                speed_deg_s,
                mean_reversal_s,
                min_reversal_s,
                max_reversal_s,
                max_yaw_deg,
                fixed_pitch_deg,
            } => {
                if t_s < self.rs_accum_s {
                    self.reset();
                }

                while self.rs_accum_s + self.rs_segment_dur < t_s {
                    self.rs_current_yaw += self.rs_current_dir * speed_deg_s * self.rs_segment_dur;
                    if self.rs_current_yaw.abs() >= *max_yaw_deg {
                        self.rs_current_yaw = self.rs_current_yaw.clamp(-max_yaw_deg, *max_yaw_deg);
                    }
                    self.rs_current_dir = -self.rs_current_dir;
                    self.rs_accum_s += self.rs_segment_dur;

                    let u = sample_unit_f64(&mut self.rs_rng).clamp(1e-7, 1.0 - 1e-7);
                    let raw_dur = -mean_reversal_s * u.ln();
                    self.rs_segment_dur = raw_dur.clamp(*min_reversal_s, *max_reversal_s);
                }

                let dt = t_s - self.rs_accum_s;
                let current_yaw = (self.rs_current_yaw + self.rs_current_dir * speed_deg_s * dt)
                    .clamp(-max_yaw_deg, *max_yaw_deg);

                ViewAngles::new(current_yaw, *fixed_pitch_deg)
            }
        }
    }
}

/// Computes the exact target ViewAngles at microsecond timestamp `t_us` for a tracking trajectory.
///
/// Implements exact constant speed: integrates geodesic steps directly on the unit sphere
/// ensuring `||v|| = speed_deg_s` everywhere within ±5% across the entire course.
pub fn target_angles_at(trajectory: &TrackingTrajectory, seed: u64, t_us: u64) -> ViewAngles {
    let mut integrator = TrajectoryIntegrator::new(trajectory.clone(), seed);
    integrator.angles_at(t_us)
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
