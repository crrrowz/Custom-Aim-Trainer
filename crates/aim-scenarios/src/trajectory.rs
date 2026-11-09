//! Pure, deterministic trajectory and motion functions for tracking, peeks, and multi-switch.

use crate::rng::{create_seeded_rng, sample_range_f64, sample_unit_f64, ChaCha8Rng};
use crate::schema::TrackingTrajectory;
use aim_core::ViewAngles;

use std::cell::RefCell;

#[derive(Debug, Clone)]
struct TrajectoryCheckpoint {
    step: u64,
    yaw_deg: f64,
    pitch_deg: f64,
    heading_rad: f64,
    reflection_count: u64,
}

/// An incremental, O(1) per-frame trajectory integrator.
///
/// Maintains simulation state and periodic checkpoints across frames so subsequent
/// calls advance by the elapsed frame time rather than re-integrating from t=0.
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
    pub cs_reflection_count: u64,
    cs_checkpoints: Vec<TrajectoryCheckpoint>,
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

        let initial_checkpoint = TrajectoryCheckpoint {
            step: 0,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            heading_rad: 0.45,
            reflection_count: 0,
        };

        Self {
            trajectory,
            seed,
            cs_heading_noise_seed,
            cs_committed_steps: 0,
            cs_yaw_deg: 0.0,
            cs_pitch_deg: 0.0,
            cs_heading_rad: 0.45,
            cs_reflection_count: 0,
            cs_checkpoints: vec![initial_checkpoint],
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
                    // Restore from closest prior checkpoint
                    let idx = match self
                        .cs_checkpoints
                        .binary_search_by_key(&target_full_steps, |cp| cp.step)
                    {
                        Ok(i) => i,
                        Err(i) => i.saturating_sub(1),
                    };
                    let cp = self.cs_checkpoints[idx].clone();
                    self.cs_committed_steps = cp.step;
                    self.cs_yaw_deg = cp.yaw_deg;
                    self.cs_pitch_deg = cp.pitch_deg;
                    self.cs_heading_rad = cp.heading_rad;
                    self.cs_reflection_count = cp.reflection_count;
                    self.cs_checkpoints.truncate(idx + 1);
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

                    if next_yaw > *max_yaw_deg {
                        self.cs_yaw_deg = 2.0 * max_yaw_deg - next_yaw;
                        self.cs_heading_rad = vy.atan2(-vx);
                        self.cs_reflection_count += 1;
                    } else if next_yaw < -*max_yaw_deg {
                        self.cs_yaw_deg = -2.0 * max_yaw_deg - next_yaw;
                        self.cs_heading_rad = vy.atan2(-vx);
                        self.cs_reflection_count += 1;
                    } else {
                        self.cs_yaw_deg = next_yaw;
                    }

                    if next_pitch > *max_pitch_deg {
                        self.cs_pitch_deg = 2.0 * max_pitch_deg - next_pitch;
                        self.cs_heading_rad = (-vy).atan2(vx);
                        self.cs_reflection_count += 1;
                    } else if next_pitch < -*max_pitch_deg {
                        self.cs_pitch_deg = -2.0 * max_pitch_deg - next_pitch;
                        self.cs_heading_rad = (-vy).atan2(vx);
                        self.cs_reflection_count += 1;
                    } else {
                        self.cs_pitch_deg = next_pitch;
                    }

                    self.cs_committed_steps += 1;

                    // Periodic checkpoints every 250 steps (50ms)
                    if self.cs_committed_steps % 250 == 0
                        && self
                            .cs_checkpoints
                            .last()
                            .map_or(true, |last| last.step < self.cs_committed_steps)
                    {
                        self.cs_checkpoints.push(TrajectoryCheckpoint {
                            step: self.cs_committed_steps,
                            yaw_deg: self.cs_yaw_deg,
                            pitch_deg: self.cs_pitch_deg,
                            heading_rad: self.cs_heading_rad,
                            reflection_count: self.cs_reflection_count,
                        });
                    }
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

                    let yaw = if next_yaw > *max_yaw_deg {
                        2.0 * max_yaw_deg - next_yaw
                    } else if next_yaw < -*max_yaw_deg {
                        -2.0 * max_yaw_deg - next_yaw
                    } else {
                        next_yaw
                    };

                    let pitch = if next_pitch > *max_pitch_deg {
                        2.0 * max_pitch_deg - next_pitch
                    } else if next_pitch < -*max_pitch_deg {
                        -2.0 * max_pitch_deg - next_pitch
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
                    if self.rs_current_yaw > *max_yaw_deg {
                        self.rs_current_yaw = 2.0 * max_yaw_deg - self.rs_current_yaw;
                        self.rs_current_dir = -self.rs_current_dir;
                    } else if self.rs_current_yaw < -*max_yaw_deg {
                        self.rs_current_yaw = -2.0 * max_yaw_deg - self.rs_current_yaw;
                        self.rs_current_dir = -self.rs_current_dir;
                    }
                    self.rs_current_dir = -self.rs_current_dir;
                    self.rs_accum_s += self.rs_segment_dur;

                    let u = sample_unit_f64(&mut self.rs_rng).clamp(1e-7, 1.0 - 1e-7);
                    let raw_dur = -mean_reversal_s * u.ln();
                    self.rs_segment_dur = raw_dur.clamp(*min_reversal_s, *max_reversal_s);
                }

                let dt = t_s - self.rs_accum_s;
                let raw_yaw = self.rs_current_yaw + self.rs_current_dir * speed_deg_s * dt;
                let current_yaw = if raw_yaw > *max_yaw_deg {
                    2.0 * max_yaw_deg - raw_yaw
                } else if raw_yaw < -*max_yaw_deg {
                    -2.0 * max_yaw_deg - raw_yaw
                } else {
                    raw_yaw
                };

                ViewAngles::new(current_yaw, *fixed_pitch_deg)
            }
        }
    }
}

/// Computes the exact target ViewAngles at microsecond timestamp `t_us` for a tracking trajectory.
///
/// Implements exact constant speed: integrates geodesic steps directly on the unit sphere
/// ensuring `||v|| = speed_deg_s` everywhere within ±5% across the entire course.
/// Uses thread-local cached integrators with internal checkpoint tables for fast, O(1) amortized evaluation.
pub fn target_angles_at(trajectory: &TrackingTrajectory, seed: u64, t_us: u64) -> ViewAngles {
    thread_local! {
        static CACHE: RefCell<Vec<(TrackingTrajectory, u64, TrajectoryIntegrator)>> =
            const { RefCell::new(Vec::new()) };
    }

    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, _, integrator)) = cache
            .iter_mut()
            .find(|(t, s, _)| *s == seed && t == trajectory)
        {
            integrator.angles_at(t_us)
        } else {
            let mut integrator = TrajectoryIntegrator::new(trajectory.clone(), seed);
            let angles = integrator.angles_at(t_us);
            if cache.len() >= 8 {
                cache.remove(0);
            }
            cache.push((trajectory.clone(), seed, integrator));
            angles
        }
    })
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
