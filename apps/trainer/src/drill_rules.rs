//! Rules that make a drill train one specific thing: pure logic, driven by the input system.

use aim_scenarios::schema::ACCURATE_SPEED_MPS;
use std::collections::VecDeque;

/// Crosshair speed below this counts as settled (matches the "stable" definition in the analysis).
const SETTLED_SPEED_DEG_S: f64 = 10.0;
const HISTORY_US: u64 = 500_000;

/// Provisional movement values, to be calibrated against VALORANT.
const MAX_RUN_SPEED_MPS: f64 = 5.4;
const ACCELERATION_MPS2: f64 = 36.0;
const FRICTION_MPS2: f64 = 30.0;

/// Recent mouse motion, used to tell whether the crosshair has come to rest.
#[derive(Debug, Clone, Default)]
pub struct MotionWindow {
    samples: VecDeque<(u64, i32, i32)>,
}

impl MotionWindow {
    pub fn push(&mut self, t_us: u64, dx: i32, dy: i32) {
        self.samples.push_back((t_us, dx, dy));
        while self
            .samples
            .front()
            .is_some_and(|s| s.0 + HISTORY_US < t_us)
        {
            self.samples.pop_front();
        }
    }

    /// Average crosshair speed over the `window_ms` before `t_us`.
    pub fn speed_deg_s(&self, t_us: u64, window_ms: f64, deg_per_count: f64) -> f64 {
        let start = t_us.saturating_sub((window_ms * 1000.0) as u64);
        let degrees: f64 = self
            .samples
            .iter()
            .filter(|s| s.0 > start && s.0 <= t_us)
            .map(|s| f64::from(s.1).hypot(f64::from(s.2)) * deg_per_count)
            .sum();
        degrees / (window_ms / 1000.0)
    }

    pub fn settled(&self, t_us: u64, window_ms: f64, deg_per_count: f64) -> bool {
        self.speed_deg_s(t_us, window_ms, deg_per_count) < SETTLED_SPEED_DEG_S
    }
}

fn wrap_deg(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// Follows one flick: how far along the line to the target the crosshair has reached.
#[derive(Debug, Clone, PartialEq)]
pub struct FlickWatch {
    pub target_id: u32,
    start: (f64, f64),
    axis: (f64, f64),
    distance: f64,
    peak: f64,
}

impl FlickWatch {
    pub fn new(target_id: u32, start: (f64, f64), target: (f64, f64)) -> Self {
        let (dy, dp) = (wrap_deg(target.0 - start.0), target.1 - start.1);
        let distance = dy.hypot(dp).max(1e-6);
        Self {
            target_id,
            start,
            axis: (dy / distance, dp / distance),
            distance,
            peak: 0.0,
        }
    }

    pub fn update(&mut self, yaw: f64, pitch: f64) {
        let along =
            wrap_deg(yaw - self.start.0) * self.axis.0 + (pitch - self.start.1) * self.axis.1;
        self.peak = self.peak.max(along);
    }

    /// Furthest point reached, as a share of the distance. 1.0 is exactly on the target.
    pub fn reach(&self) -> f64 {
        self.peak / self.distance
    }
}

/// Strafe movement. There is no world to walk through: only the speed matters, and it is shown on the HUD.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerMotion {
    pub velocity: f64,
}

impl PlayerMotion {
    pub fn step(&mut self, dt_s: f64, left: bool, right: bool) {
        let wish = f64::from(u8::from(right)) - f64::from(u8::from(left));
        if wish == 0.0 {
            let drop = FRICTION_MPS2 * dt_s;
            self.velocity = if self.velocity.abs() <= drop {
                0.0
            } else {
                self.velocity - drop * self.velocity.signum()
            };
        } else {
            self.velocity = (self.velocity + wish * ACCELERATION_MPS2 * dt_s)
                .clamp(-MAX_RUN_SPEED_MPS, MAX_RUN_SPEED_MPS);
        }
    }

    pub fn is_accurate(&self) -> bool {
        self.velocity.abs() <= ACCURATE_SPEED_MPS
    }
}

/// Per-session state for the rules above.
#[derive(Debug, Clone, Default)]
pub struct DrillRuntime {
    pub motion: MotionWindow,
    pub watch: Option<FlickWatch>,
    pub last_reach: Option<f64>,
    pub player: PlayerMotion,
    pub track_frames: u32,
    pub track_frames_on: u32,
}

impl DrillRuntime {
    /// Records the crosshair against the current target, starting a new watch when the target changes.
    pub fn observe(&mut self, target_id: u32, target: (f64, f64), yaw: f64, pitch: f64) {
        if self.watch.as_ref().map(|w| w.target_id) != Some(target_id) {
            self.watch = Some(FlickWatch::new(target_id, (yaw, pitch), target));
        }
        if let Some(w) = &mut self.watch {
            w.update(yaw, pitch);
        }
    }

    /// Share of tracking frames with the crosshair on the target, as a percentage.
    pub fn on_target_percent(&self) -> Option<f64> {
        (self.track_frames > 0)
            .then(|| f64::from(self.track_frames_on) * 100.0 / f64::from(self.track_frames))
    }

    pub fn reach(&self) -> Option<f64> {
        self.watch.as_ref().map(FlickWatch::reach)
    }
}

/// Why a shot that touched the target still does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    NotSettled,
    Overshot,
    Moving,
}

pub struct ShotCheck<'a> {
    pub rules: &'a aim_scenarios::schema::DrillRules,
    pub runtime: &'a DrillRuntime,
    pub t_us: u64,
    pub deg_per_count: f64,
}

impl ShotCheck<'_> {
    pub fn rejection(&self) -> Option<Rejection> {
        if self.rules.counter_strafe && !self.runtime.player.is_accurate() {
            return Some(Rejection::Moving);
        }
        if let (Some(limit), Some(reach)) = (self.rules.overshoot_fail_ratio, self.runtime.reach())
        {
            if reach > limit {
                return Some(Rejection::Overshot);
            }
        }
        if let Some(ms) = self.rules.settle_ms {
            if !self
                .runtime
                .motion
                .settled(self.t_us, ms, self.deg_per_count)
            {
                return Some(Rejection::NotSettled);
            }
        }
        None
    }
}
