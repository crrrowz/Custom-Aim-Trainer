//! Synthetic sessions whose answers are derived by hand from 03_DIAGNOSIS_ENGINE.md.
//! Onset goes through a 15 ms smoothing window, so instants are compared within 10 ms (roadmap 2.3).

use aim_analysis::compute_engagement_metrics;
use aim_telemetry::format::{
    AftSession, FrameStream, HitZone, KeyCode, KeyRecord, ShotRecord, TargetKind, TargetParams,
    TargetRecord,
};
use aim_telemetry::sample_session;

const DPC: f64 = 0.02;
const SPAWN: u64 = 1_000_000;

fn assert_near(actual: u64, expected: u64, tolerance: u64) {
    assert!(
        actual.abs_diff(expected) <= tolerance,
        "{actual} is not within {tolerance} of {expected}"
    );
}

/// One target at `yaw`, 1 kHz input, frames presenting at `present`.
fn session(yaw: f64, present: u64, moves: &[(u64, i32)], shot_t: u64) -> AftSession {
    let mut s = sample_session();
    s.header.mouse.deg_per_count = DPC;
    s.streams.frames = FrameStream {
        t_us: vec![present],
        present_t_us: vec![present],
    };
    s.streams.keys.records.clear();
    s.streams.targets.records = vec![TargetRecord {
        id: 1,
        spawn_t_us: SPAWN,
        despawn_t_us: SPAWN + 1_500_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.5,
            initial_pos: [yaw, 0.0, 15.0],
            path_spline: None,
        },
    }];
    s.streams.input.t_us = moves.iter().map(|m| m.0).collect();
    s.streams.input.dx = moves.iter().map(|m| m.1).collect();
    s.streams.input.dy = vec![0; moves.len()];
    s.streams.shots.records = vec![ShotRecord {
        t_us: shot_t,
        hit_target_id: Some(1),
        hitzone: HitZone::Head,
    }];
    s
}

/// A flick of `counts` over `ms` milliseconds starting at `start`, at 1 kHz.
fn flick(start: u64, ms: u64, counts: i32) -> Vec<(u64, i32)> {
    let per_step = counts / ms as i32;
    (0..ms).map(|i| (start + i * 1_000, per_step)).collect()
}

#[test]
fn slow_drift_below_threshold_is_not_onset() {
    // 2 counts every 2 ms = 20 deg/s, under the 30 deg/s onset speed. The real flick starts at +200 ms.
    let mut moves: Vec<(u64, i32)> = (0..50).map(|i| (SPAWN + i * 2_000, 2)).collect();
    moves.extend(flick(SPAWN + 200_000, 100, 500));
    let s = session(10.0, SPAWN, &moves, SPAWN + 400_000);
    let m = &compute_engagement_metrics(&s)[0];
    assert_near(m.reaction_time_us.unwrap(), 200_000, 10_000);
}

#[test]
fn reaction_is_measured_from_the_presented_frame() {
    // The frame showing the target is presented 33 ms after the simulation spawn.
    let moves = flick(SPAWN + 200_000, 100, 500);
    let s = session(10.0, SPAWN + 33_000, &moves, SPAWN + 400_000);
    let m = &compute_engagement_metrics(&s)[0];
    assert_near(m.reaction_time_us.unwrap(), 167_000, 10_000);
}

#[test]
fn reaction_under_100ms_is_flagged_as_anticipation() {
    let moves = flick(SPAWN + 40_000, 100, 500);
    let s = session(10.0, SPAWN, &moves, SPAWN + 300_000);
    assert!(compute_engagement_metrics(&s)[0].anticipated);
}

#[test]
fn undershoot_with_a_correction_has_gain_below_one() {
    // 400 counts = 8 deg of a 10 deg target, 150 ms idle, then a 2 deg correction: G = 0.8, one pulse.
    let mut moves = flick(SPAWN + 200_000, 80, 400);
    moves.extend(flick(SPAWN + 430_000, 40, 100));
    let s = session(10.0, SPAWN, &moves, SPAWN + 700_000);
    let m = &compute_engagement_metrics(&s)[0];
    assert!(
        (m.gain_ratio.unwrap() - 0.8).abs() < 0.05,
        "{:?}",
        m.gain_ratio
    );
    assert_eq!(m.correction_count, 1);
}

#[test]
fn overshoot_has_gain_above_one() {
    // 600 counts = 12 deg on a 10 deg target, then a return: G = 1.2.
    let mut moves = flick(SPAWN + 200_000, 100, 600);
    moves.extend(flick(SPAWN + 430_000, 40, -100));
    let s = session(10.0, SPAWN, &moves, SPAWN + 700_000);
    let m = &compute_engagement_metrics(&s)[0];
    assert!(
        (m.gain_ratio.unwrap() - 1.2).abs() < 0.05,
        "{:?}",
        m.gain_ratio
    );
    assert!(m.endpoint_error_deg.is_some());
}

#[test]
fn target_left_of_zero_wraps_correctly() {
    // -10 deg target, leftward flick. Comparing wrapped yaw (350) with -10 would report a 360 deg error.
    let moves = flick(SPAWN + 200_000, 100, -500);
    let s = session(-10.0, SPAWN, &moves, SPAWN + 400_000);
    let m = &compute_engagement_metrics(&s)[0];
    assert!((m.distance_deg.unwrap() - 10.0).abs() < 0.2);
    assert!(
        (m.gain_ratio.unwrap() - 1.0).abs() < 0.05,
        "{:?}",
        m.gain_ratio
    );
}

#[test]
fn pause_is_excluded_from_reaction_time() {
    // Paused for 5 s between the spawn and the flick: the 200 ms reaction must not grow.
    let moves = flick(SPAWN + 5_200_000, 100, 500);
    let mut s = session(10.0, SPAWN, &moves, SPAWN + 5_400_000);
    s.streams.targets.records[0].despawn_t_us = SPAWN + 6_000_000;
    s.streams.keys.records = vec![
        KeyRecord {
            t_us: SPAWN + 100_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: SPAWN + 5_100_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
    ];
    let m = &compute_engagement_metrics(&s)[0];
    assert_near(m.reaction_time_us.unwrap(), 200_000, 10_000);
}
