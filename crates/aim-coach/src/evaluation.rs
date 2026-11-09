//! Staircase difficulty and the statistical verdicts of 04_TRAINING_LOOP.md sections 3 and 5.

use serde::{Deserialize, Serialize};

/// Weighted up/down staircase (Kaernbach 1991) targeting a success rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Staircase {
    pub d: f64,
    step_up: f64,
    step_down: f64,
}

impl Staircase {
    pub fn new(d0: f64, target_p: f64, step: f64) -> Self {
        Self {
            d: d0,
            step_up: step,
            step_down: step * target_p / (1.0 - target_p),
        }
    }

    pub fn update(&mut self, success: bool) {
        let next = if success {
            self.d + self.step_up
        } else {
            self.d - self.step_down
        };
        self.d = next.clamp(0.0, 1.0);
    }
}

/// One-dimensional Kalman filter over a skill score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillTrack {
    pub mean: f64,
    pub var: f64,
}

impl SkillTrack {
    pub fn predict(&mut self, days: f64, q: f64) {
        self.var += q * days;
    }

    pub fn update(&mut self, obs: f64, r: f64) {
        let k = self.var / (self.var + r);
        self.mean += k * (obs - self.mean);
        self.var *= 1.0 - k;
    }
}

pub fn mdc95(sem: f64) -> f64 {
    1.96 * 2.0_f64.sqrt() * sem
}

/// SEM shrunk toward the population value once the player has their own history.
pub fn sem_personal(sem_pop: f64, s_player: f64, n: u32) -> f64 {
    const PRIOR_WEIGHT: f64 = 4.0;
    let n = n as f64;
    ((PRIOR_WEIGHT * sem_pop * sem_pop + n * s_player * s_player) / (PRIOR_WEIGHT + n)).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Improved,
    ProbablyImproved,
    NoChange,
    Regressed,
    /// A change between MDC/2 and MDC downward: not distinguishable from noise, not confirmed either.
    ProbablyRegressed,
}

pub fn verdict(delta: f64, mdc: f64) -> Verdict {
    if delta >= mdc {
        Verdict::Improved
    } else if delta >= mdc / 2.0 {
        Verdict::ProbablyImproved
    } else if delta.abs() < mdc / 2.0 {
        Verdict::NoChange
    } else if delta <= -mdc {
        Verdict::Regressed
    } else {
        Verdict::ProbablyRegressed
    }
}

/// Training effect: change in trained skills minus change in untrained skills.
/// `None` when either side has no skills to compare.
pub fn difference_in_differences(trained: &[f64], untrained: &[f64]) -> Option<f64> {
    if trained.is_empty() || untrained.is_empty() {
        return None;
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    Some(mean(trained) - mean(untrained))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Adaptation {
    Graduate,
    ContinueSameDrill,
    SwitchDrill,
    Rediagnose,
    CheckConditionsLighterBlock,
}

pub fn adapt(verdict: Verdict, reached_goal: bool, previous_no_change: bool) -> Adaptation {
    match verdict {
        Verdict::Improved if reached_goal => Adaptation::Graduate,
        Verdict::Improved => Adaptation::ContinueSameDrill,
        Verdict::Regressed | Verdict::ProbablyRegressed => Adaptation::CheckConditionsLighterBlock,
        Verdict::ProbablyImproved | Verdict::NoChange if previous_no_change => {
            Adaptation::Rediagnose
        }
        Verdict::ProbablyImproved | Verdict::NoChange => Adaptation::SwitchDrill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staircase_settles_near_the_target_success_rate() {
        let mut s = Staircase::new(0.5, 0.75, 0.02);
        // A player who succeeds exactly when d is under 0.6: d must hover around 0.6.
        for _ in 0..500 {
            let success = s.d < 0.6;
            s.update(success);
        }
        assert!((s.d - 0.6).abs() < 0.1, "{}", s.d);
    }

    #[test]
    fn verdict_thresholds_follow_the_spec() {
        assert_eq!(verdict(8.0, 8.0), Verdict::Improved);
        assert_eq!(verdict(4.0, 8.0), Verdict::ProbablyImproved);
        assert_eq!(verdict(3.9, 8.0), Verdict::NoChange);
        assert_eq!(verdict(-8.0, 8.0), Verdict::Regressed);
    }

    #[test]
    fn did_separates_training_from_test_practice() {
        assert_eq!(difference_in_differences(&[14.0], &[3.0]), Some(11.0));
        assert_eq!(difference_in_differences(&[14.0], &[]), None);
    }

    #[test]
    fn kalman_moves_toward_the_observation() {
        let mut t = SkillTrack {
            mean: 50.0,
            var: 25.0,
        };
        t.update(60.0, 25.0);
        assert!((t.mean - 55.0).abs() < 1e-9);
        assert!((t.var - 12.5).abs() < 1e-9);
    }

    #[test]
    fn second_no_change_triggers_rediagnosis() {
        assert_eq!(
            adapt(Verdict::NoChange, false, false),
            Adaptation::SwitchDrill
        );
        assert_eq!(
            adapt(Verdict::NoChange, false, true),
            Adaptation::Rediagnose
        );
        assert_eq!(adapt(Verdict::Improved, true, false), Adaptation::Graduate);
    }
}
