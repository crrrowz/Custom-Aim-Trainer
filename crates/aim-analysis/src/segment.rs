//! Movement segmentation (docs/platform/03_DIAGNOSIS_ENGINE.md section 2).
//!
//! Raw mouse events are irregular, so they are resampled to a 1 kHz grid and differentiated with a
//! Savitzky-Golay filter (15 ms window, degree 2). Yaw is kept unwrapped so that a target at -40 deg
//! and a crosshair at 320 deg compare correctly.

use serde::{Deserialize, Serialize};

/// Thresholds from `content/analysis/thresholds.v1.ron`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisThresholds {
    pub onset_speed_threshold_deg_s: f64,
    pub onset_min_duration_ms: f64,
    pub anticipation_threshold_ms: f64,
    pub primary_end_ratio_to_peak: f64,
    pub submovement_min_peak_deg_s: f64,
    pub stable_max_speed_deg_s: f64,
    pub stable_min_duration_ms: f64,
}

impl Default for AnalysisThresholds {
    fn default() -> Self {
        Self {
            onset_speed_threshold_deg_s: 30.0,
            onset_min_duration_ms: 20.0,
            anticipation_threshold_ms: 100.0,
            primary_end_ratio_to_peak: 0.2,
            submovement_min_peak_deg_s: 15.0,
            stable_max_speed_deg_s: 10.0,
            stable_min_duration_ms: 30.0,
        }
    }
}

const GRID_US: u64 = 1_000;
const SG_HALF: usize = 7;
/// Sum of k^2 for k in -7..=7, the denominator of the Savitzky-Golay first derivative.
const SG_NORM: f64 = 280.0;
/// Longest interval one mouse report can cover (125 Hz). Longer gaps are idle time, not slow motion.
const MAX_REPORT_US: u64 = 8_000;

/// Crosshair state on a uniform 1 kHz grid.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub t0_us: u64,
    pub yaw: Vec<f64>,
    pub pitch: Vec<f64>,
    pub v_yaw: Vec<f64>,
    pub v_pitch: Vec<f64>,
    pub speed: Vec<f64>,
}

impl Trace {
    pub fn build(
        t_us: &[u64],
        dx: &[i32],
        dy: &[i32],
        deg_per_count: f64,
        t_start_us: u64,
        t_end_us: u64,
    ) -> Self {
        let first = t_us.first().copied().unwrap_or(t_start_us);
        let last = t_us.last().copied().unwrap_or(t_end_us);
        let t0_us = t_start_us.min(first);
        let end_us = t_end_us.max(last);
        let n = ((end_us - t0_us) / GRID_US) as usize + 1;

        let mut yaw = Vec::with_capacity(n);
        let mut pitch = Vec::with_capacity(n);
        let (mut prev_t, mut prev_yaw, mut prev_pitch) = (t0_us, 0.0_f64, 0.0_f64);
        let mut next = 0_usize;

        for i in 0..n {
            let g = t0_us + i as u64 * GRID_US;
            while next < t_us.len() && t_us[next] <= g {
                prev_yaw += dx[next] as f64 * deg_per_count;
                prev_pitch = (prev_pitch - dy[next] as f64 * deg_per_count).clamp(-89.0, 89.0);
                prev_t = t_us[next];
                next += 1;
            }
            let (mut y, mut p) = (prev_yaw, prev_pitch);
            if next < t_us.len() {
                let ramp_start = prev_t.max(t_us[next].saturating_sub(MAX_REPORT_US));
                if g > ramp_start {
                    let frac = (g - ramp_start) as f64 / (t_us[next] - ramp_start) as f64;
                    y += frac * dx[next] as f64 * deg_per_count;
                    p -= frac * dy[next] as f64 * deg_per_count;
                }
            }
            yaw.push(y);
            pitch.push(p);
        }

        let v_yaw = derivative(&yaw);
        let v_pitch = derivative(&pitch);
        let speed = v_yaw
            .iter()
            .zip(&v_pitch)
            .map(|(a, b)| (a * a + b * b).sqrt())
            .collect();
        Self {
            t0_us,
            yaw,
            pitch,
            v_yaw,
            v_pitch,
            speed,
        }
    }

    pub fn len(&self) -> usize {
        self.yaw.len()
    }

    pub fn is_empty(&self) -> bool {
        self.yaw.is_empty()
    }

    pub fn index_at(&self, t_us: u64) -> usize {
        let i = (t_us.saturating_sub(self.t0_us) / GRID_US) as usize;
        i.min(self.len().saturating_sub(1))
    }

    pub fn time_at(&self, index: usize) -> u64 {
        self.t0_us + index as u64 * GRID_US
    }

    /// Shortest (yaw, pitch) vector from the crosshair at `i` to a target.
    pub fn error_to(&self, i: usize, target_yaw: f64, target_pitch: f64) -> (f64, f64) {
        (
            (target_yaw - self.yaw[i] + 180.0).rem_euclid(360.0) - 180.0,
            target_pitch - self.pitch[i],
        )
    }

    pub fn distance_to(&self, i: usize, target_yaw: f64, target_pitch: f64) -> f64 {
        let (dy, dp) = self.error_to(i, target_yaw, target_pitch);
        (dy * target_pitch.to_radians().cos()).hypot(dp)
    }
}

fn derivative(y: &[f64]) -> Vec<f64> {
    let n = y.len();
    let mut out = vec![0.0; n];
    if n < 2 * SG_HALF + 1 {
        return out;
    }
    for i in SG_HALF..n - SG_HALF {
        let mut sum = 0.0;
        for k in 1..=SG_HALF {
            sum += k as f64 * (y[i + k] - y[i - k]);
        }
        out[i] = sum / SG_NORM / (GRID_US as f64 / 1_000_000.0);
    }
    out
}

/// Key instants of one target engagement, as grid indices.
#[derive(Debug, Clone, PartialEq)]
pub struct Segmentation {
    pub onset: usize,
    pub peak: usize,
    pub primary_end: usize,
    pub submovements: u32,
    pub stable: Option<usize>,
}

fn find_onset(
    trace: &Trace,
    th: &AnalysisThresholds,
    from: usize,
    to: usize,
    target: (f64, f64),
) -> Option<usize> {
    let hold = th.onset_min_duration_ms.round() as usize;
    let last = to.min(trace.len().saturating_sub(hold + 1));
    (from..=last).find(|&i| {
        let sustained = (i..=i + hold).all(|j| trace.speed[j] > th.onset_speed_threshold_deg_s);
        let (ey, ep) = trace.error_to(i, target.0, target.1);
        sustained && trace.v_yaw[i] * ey + trace.v_pitch[i] * ep > 0.0
    })
}

/// Segments the crosshair movement toward a target between grid indices `from` and `to`.
/// `None` means the player never started a qualifying movement.
pub fn segment(
    trace: &Trace,
    th: &AnalysisThresholds,
    from: usize,
    to: usize,
    target: (f64, f64),
    radius_deg: f64,
) -> Option<Segmentation> {
    let to = to.min(trace.len().saturating_sub(1));
    let onset = find_onset(trace, th, from, to, target)?;

    let (ey, ep) = trace.error_to(onset, target.0, target.1);
    let norm = ey.hypot(ep).max(1e-9);
    let axis = (ey / norm, ep / norm);

    let mut peak = onset;
    let mut primary_end = to;
    for j in onset + 1..=to {
        if trace.speed[j] > trace.speed[peak] {
            peak = j;
        }
        if j == peak {
            continue;
        }
        let along = trace.v_yaw[j] * axis.0 + trace.v_pitch[j] * axis.1;
        let slowed = trace.speed[j] < th.primary_end_ratio_to_peak * trace.speed[peak]
            && j + 1 < trace.len()
            && trace.speed[j] <= trace.speed[j + 1];
        if along <= 0.0 || slowed {
            primary_end = j;
            break;
        }
    }

    // A pulse counts once speed has fallen to half the pulse floor since the previous one, so the
    // decaying tail of the primary movement is not mistaken for a correction.
    let rearm = th.submovement_min_peak_deg_s / 2.0;
    let mut armed = false;
    let mut submovements = 0;
    for j in primary_end..=to {
        if armed && trace.speed[j] > th.submovement_min_peak_deg_s {
            submovements += 1;
            armed = false;
        } else if !armed && trace.speed[j] < rearm {
            armed = true;
        }
    }

    let hold = th.stable_min_duration_ms.round() as usize;
    let stable = (primary_end..=to).find(|&i| {
        i + hold < trace.len()
            && (i..=i + hold).all(|j| {
                trace.speed[j] < th.stable_max_speed_deg_s
                    && trace.distance_to(j, target.0, target.1) <= radius_deg
            })
    });

    Some(Segmentation {
        onset,
        peak,
        primary_end,
        submovements,
        stable,
    })
}
