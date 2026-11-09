//! Training blocks: daily session construction and the plan state machine (04_TRAINING_LOOP.md).

use crate::diagnosis::Finding;
use serde::{Deserialize, Serialize};

pub const BLOCK_DAYS: u8 = 6;
/// A block with fewer completed days is extended rather than evaluated.
pub const MIN_DAYS_TO_EVALUATE: u8 = 4;
const ABSENCE_LIMIT_DAYS: u32 = 4;
const MAX_FOCUS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    WarmUp,
    FocusA,
    FocusB,
    Variation,
    Maintenance,
    MiniCheck,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionItem {
    pub kind: ItemKind,
    pub drill: String,
    pub minutes: u32,
    /// Days 1-2 keep the settings fixed; from day 3 distances and sizes vary.
    pub varied: bool,
}

pub fn daily_session(findings: &[Finding], maintenance_drill: &str, day: u8) -> Vec<SessionItem> {
    let varied = day >= 3;
    let item = |kind, drill: &str, minutes, varied| SessionItem {
        kind,
        drill: drill.to_string(),
        minutes,
        varied,
    };
    let focus: Vec<&str> = findings
        .iter()
        .take(MAX_FOCUS)
        .filter_map(|f| f.drills.first().map(String::as_str))
        .collect();

    let mut session = vec![item(ItemKind::WarmUp, "warmup", 2, false)];
    if let Some(a) = focus.first() {
        session.push(item(ItemKind::FocusA, a, 5, varied));
    }
    if let Some(b) = focus.get(1) {
        session.push(item(ItemKind::FocusB, b, 4, varied));
    }
    if let Some(a) = focus.first() {
        session.push(item(ItemKind::Variation, a, 2, true));
    }
    session.push(item(ItemKind::Maintenance, maintenance_drill, 2, false));
    if let Some(a) = focus.first() {
        session.push(item(ItemKind::MiniCheck, a, 1, false));
    }
    session
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanState {
    Draft,
    Active { block: u8, day: u8, completed: u8 },
    AwaitingRetest { block: u8, due_day: u32 },
    Evaluated { block: u8 },
    Paused { block: u8, day: u8, completed: u8 },
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEvent {
    Accept,
    /// A daily session finished on calendar day `today`.
    DayCompleted {
        today: u32,
    },
    /// The app opened on `today` and the last training was on `last_trained`.
    Opened {
        today: u32,
        last_trained: u32,
    },
    RetestFinished,
    NextBlock,
    Finish,
}

impl PlanState {
    /// Applies an event. Events that make no sense in the current state leave it unchanged.
    pub fn apply(self, event: PlanEvent) -> PlanState {
        use PlanEvent as E;
        use PlanState as S;
        match (self, event) {
            (S::Draft, E::Accept) => S::Active {
                block: 1,
                day: 1,
                completed: 0,
            },
            (
                S::Active {
                    block,
                    day,
                    completed,
                },
                E::DayCompleted { today },
            ) => {
                let completed = completed + 1;
                if day >= BLOCK_DAYS && completed >= MIN_DAYS_TO_EVALUATE {
                    // Retest after at least one rest day.
                    S::AwaitingRetest {
                        block,
                        due_day: today + 2,
                    }
                } else {
                    S::Active {
                        block,
                        day: day + 1,
                        completed,
                    }
                }
            }
            (
                S::Active {
                    block,
                    day,
                    completed,
                },
                E::Opened {
                    today,
                    last_trained,
                },
            ) if today.saturating_sub(last_trained) > ABSENCE_LIMIT_DAYS => S::Paused {
                block,
                day,
                completed,
            },
            (
                S::Paused {
                    block,
                    day,
                    completed,
                },
                E::Accept,
            ) => S::Active {
                block,
                day,
                completed,
            },
            (S::AwaitingRetest { block, .. }, E::RetestFinished) => S::Evaluated { block },
            (S::Evaluated { block }, E::NextBlock) => S::Active {
                block: block + 1,
                day: 1,
                completed: 0,
            },
            (S::Evaluated { .. }, E::Finish) => S::Completed,
            (state, _) => state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aim_analysis::SkillId;

    fn finding(drill: &str) -> Finding {
        Finding {
            id: "D2".to_string(),
            skill: SkillId::S04GainControl,
            severity: 0.5,
            priority: 0.5,
            evidence: Vec::new(),
            drills: vec![drill.to_string()],
        }
    }

    #[test]
    fn session_has_the_six_parts_and_16_minutes() {
        let s = daily_session(&[finding("a"), finding("b")], "m", 1);
        assert_eq!(s.len(), 6);
        assert_eq!(s.iter().map(|i| i.minutes).sum::<u32>(), 16);
        assert!(!s[1].varied);
    }

    #[test]
    fn variation_starts_on_day_three() {
        assert!(daily_session(&[finding("a")], "m", 3)[1].varied);
    }

    #[test]
    fn full_block_reaches_retest_after_a_rest_day() {
        let mut p = PlanState::Draft.apply(PlanEvent::Accept);
        for today in 1..=6 {
            p = p.apply(PlanEvent::DayCompleted { today });
        }
        assert_eq!(
            p,
            PlanState::AwaitingRetest {
                block: 1,
                due_day: 8
            }
        );
        assert_eq!(
            p.apply(PlanEvent::RetestFinished),
            PlanState::Evaluated { block: 1 }
        );
    }

    #[test]
    fn long_absence_pauses_the_plan() {
        let p = PlanState::Active {
            block: 1,
            day: 2,
            completed: 1,
        };
        let paused = p.apply(PlanEvent::Opened {
            today: 20,
            last_trained: 10,
        });
        assert!(matches!(paused, PlanState::Paused { .. }));
    }

    #[test]
    fn nonsense_events_do_not_change_state() {
        assert_eq!(
            PlanState::Draft.apply(PlanEvent::RetestFinished),
            PlanState::Draft
        );
    }
}
