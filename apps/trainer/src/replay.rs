//! Input replay and chronological evaluation logic.

use crate::raw_input::{RawInputEvent, RawInputPayload};
use aim_core::ViewAngles;

pub enum ReplayAction {
    Motion {
        t_us: u64,
        dx: i32,
        dy: i32,
    },
    Click {
        t_us: u64,
        angles_at_click: ViewAngles,
    },
}

/// Replays a slice of chronological raw input events against an initial ViewAngles.
///
/// For each motion event, updates `current_angles` using `apply_counts` and records the action.
/// For each click (LeftButtonDown), records a Click action with the exact camera orientation
/// accumulated up to that exact microsecond.
///
/// Returns the ordered list of replay actions and leaves `current_angles` with the final orientation.
pub fn replay_input(
    current_angles: &mut ViewAngles,
    events: &[RawInputEvent],
    primary_id: Option<usize>,
    deg_per_count: f64,
) -> Vec<ReplayAction> {
    let mut actions = Vec::with_capacity(events.len());

    for event in events {
        if primary_id.is_some() && Some(event.device_id) != primary_id {
            continue;
        }

        match event.payload {
            RawInputPayload::Motion { dx, dy } => {
                actions.push(ReplayAction::Motion {
                    t_us: event.t_us,
                    dx,
                    dy,
                });
                current_angles.apply_counts(dx, dy, deg_per_count);
            }
            RawInputPayload::LeftButtonDown => {
                actions.push(ReplayAction::Click {
                    t_us: event.t_us,
                    angles_at_click: *current_angles,
                });
            }
            _ => {}
        }
    }

    actions
}

/// Computes the exact ViewAngles at timestamp `target_t_us` by replaying events up to that instant.
pub fn angles_at_timestamp(
    initial_angles: &ViewAngles,
    events: &[RawInputEvent],
    target_t_us: u64,
    primary_id: Option<usize>,
    deg_per_count: f64,
) -> ViewAngles {
    let mut angles = *initial_angles;
    for event in events {
        if primary_id.is_some() && Some(event.device_id) != primary_id {
            continue;
        }
        if event.t_us > target_t_us {
            break;
        }
        if let RawInputPayload::Motion { dx, dy } = event.payload {
            angles.apply_counts(dx, dy, deg_per_count);
        }
    }
    angles
}

/// Computes the crosshair pitch at an exact QPC timestamp by replaying raw input events
/// up to that timestamp against an initial ViewAngles.
pub fn crosshair_pitch_at_qpc_instant(
    initial_angles: &ViewAngles,
    events: &[RawInputEvent],
    target_qpc_us: u64,
    primary_id: Option<usize>,
    deg_per_count: f64,
) -> f64 {
    let angles = angles_at_timestamp(
        initial_angles,
        events,
        target_qpc_us,
        primary_id,
        deg_per_count,
    );
    angles.pitch
}
