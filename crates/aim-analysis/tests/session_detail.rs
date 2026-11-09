//! What the analysis screen draws, checked on sessions with known answers.

use aim_analysis::session_detail;
use aim_telemetry::format::{
    AftSession, FrameStream, HitZone, KeyCode, KeyRecord, ShotRecord, TargetKind, TargetParams,
    TargetRecord,
};
use aim_telemetry::sample_session;

const DPC: f64 = 0.02;
const SPAWN: u64 = 1_000_000;

fn target(id: u32, spawn: u64, yaw: f64, pitch: f64) -> TargetRecord {
    TargetRecord {
        id,
        spawn_t_us: spawn,
        despawn_t_us: spawn + 1_500_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.5,
            initial_pos: [yaw, pitch, 15.0],
            path_spline: None,
        },
    }
}

/// A 10 degree rightward flick landing on a target at yaw 10, with one recorded hit.
fn one_flick() -> AftSession {
    let mut s = sample_session();
    s.header.mouse.deg_per_count = DPC;
    s.streams.keys.records.clear();
    s.streams.frames = FrameStream {
        t_us: vec![SPAWN],
        present_t_us: vec![SPAWN],
    };
    s.streams.targets.records = vec![target(1, SPAWN, 10.0, 0.0)];
    s.streams.input.t_us = (0..100).map(|i| SPAWN + 200_000 + i * 1_000).collect();
    s.streams.input.dx = vec![5; 100];
    s.streams.input.dy = vec![0; 100];
    s.streams.shots.records = vec![ShotRecord {
        t_us: SPAWN + 500_000,
        hit_target_id: Some(1),
        hitzone: HitZone::Head,
    }];
    s
}

#[test]
fn path_starts_at_the_full_distance_and_ends_on_the_target() {
    let d = session_detail(&one_flick());
    let f = &d.flicks[0];
    let (first, last) = (&f.path[0], f.path.last().unwrap());
    assert!(
        (first.x + 10.0).abs() < 0.05,
        "starts 10 deg left of target: {}",
        first.x
    );
    assert!(last.x.abs() < 0.05, "ends on target: {}", last.x);
    assert!(f.path.windows(2).all(|w| w[1].t_ms > w[0].t_ms));
}

#[test]
fn markers_are_in_time_order_and_inside_the_path() {
    let d = session_detail(&one_flick());
    let f = &d.flicks[0];
    let onset = f.onset_ms.unwrap();
    let end = f.primary_end_ms.unwrap();
    let shot = f.shot_ms.unwrap();
    assert!(onset < end && end < shot, "{onset} {end} {shot}");
    assert!(f64::from(f.path.last().unwrap().t_ms) >= shot);
}

#[test]
fn speed_peaks_at_the_true_flick_speed() {
    // 5 counts per ms at 0.02 deg per count is 100 deg/s.
    let d = session_detail(&one_flick());
    let peak = d.flicks[0].path.iter().map(|p| p.speed).fold(0.0, f32::max);
    assert!((peak - 100.0).abs() < 5.0, "{peak}");
}

#[test]
fn summary_counts_hits_and_accuracy() {
    let mut s = one_flick();
    s.streams
        .targets
        .records
        .push(target(2, SPAWN + 2_000_000, 20.0, 0.0));
    let d = session_detail(&s);
    assert_eq!(d.targets, 2);
    assert_eq!(d.hits, 1);
    assert_eq!(d.accuracy, Some(0.5));
}

#[test]
fn endpoint_is_relative_to_the_target() {
    let mut s = one_flick();
    s.streams.targets.records = vec![target(1, SPAWN, 11.0, 0.0)];
    let e = session_detail(&s).flicks[0].endpoint.unwrap();
    assert!((e[0] + 1.0).abs() < 0.05, "1 deg left of target: {}", e[0]);
    assert!(e[1].abs() < 0.05);
}

#[test]
fn home_targets_are_left_out() {
    let mut s = one_flick();
    s.streams
        .targets
        .records
        .push(target(9, SPAWN + 2_000_000, 0.0, 0.0));
    assert_eq!(session_detail(&s).flicks.len(), 1);
}

#[test]
fn pause_time_is_reported_and_excluded_from_timing() {
    let mut s = one_flick();
    s.streams
        .targets
        .records
        .push(target(2, SPAWN + 8_000_000, 20.0, 0.0));
    s.streams.keys.records = vec![
        KeyRecord {
            t_us: SPAWN + 2_000_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: SPAWN + 7_000_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
    ];
    let d = session_detail(&s);
    assert!((d.paused_s - 5.0).abs() < 1e-6, "{}", d.paused_s);
    assert!((d.flicks[1].t_s - 3.0).abs() < 1e-6, "{}", d.flicks[1].t_s);
}

#[test]
fn an_empty_session_does_not_panic() {
    let mut s = sample_session();
    s.streams.targets.records.clear();
    s.streams.shots.records.clear();
    s.streams.keys.records.clear();
    s.streams.input.t_us.clear();
    s.streams.input.dx.clear();
    s.streams.input.dy.clear();
    let d = session_detail(&s);
    assert_eq!(d.targets, 0);
    assert_eq!(d.accuracy, None);
}
