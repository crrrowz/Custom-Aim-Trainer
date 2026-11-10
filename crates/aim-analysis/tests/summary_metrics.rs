use aim_analysis::{
    compute_session_summary, extract_pause_intervals, format_quality_flags, pause_duration_between,
};
use aim_telemetry::format::{
    quality_flags, HitZone, KeyCode, KeyRecord, ShotRecord, TargetKind, TargetParams, TargetRecord,
};
use aim_telemetry::sample_session;

#[test]
fn test_format_quality_flags() {
    assert_eq!(format_quality_flags(0), "CLEAN");
    let flags = quality_flags::FOCUS_LOST | quality_flags::PAUSED;
    let formatted = format_quality_flags(flags);
    assert!(formatted.contains("FOCUS_LOST"));
    assert!(formatted.contains("PAUSED"));
}

#[test]
fn test_extract_pause_intervals_from_esc_keys() {
    let mut session = sample_session();
    session.streams.keys.records = vec![
        KeyRecord {
            t_us: 1_000_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: 3_000_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
        KeyRecord {
            t_us: 5_000_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: 6_000_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
    ];

    let pauses = extract_pause_intervals(&session);
    assert_eq!(pauses.len(), 2);
    assert_eq!(pauses[0], (1_000_000, 3_000_000));
    assert_eq!(pauses[1], (5_000_000, 6_000_000));

    // Overlapping query
    assert_eq!(pause_duration_between(&pauses, 0, 7_000_000), 3_000_000);
    assert_eq!(
        pause_duration_between(&pauses, 2_000_000, 4_000_000),
        1_000_000
    );
}

#[test]
fn test_summary_on_sample_session() {
    let session = sample_session();
    let summary = compute_session_summary(&session);

    assert_eq!(summary.scenario_id, "assess.v1.flick_static");
    assert_eq!(summary.seed, 987654321);
    assert_eq!(summary.dpi, 800);
    assert_eq!(summary.sens, 0.35);
    assert_eq!(summary.est_polling_hz, 1000);
    assert_eq!(summary.total_shots, 1);
    assert_eq!(summary.total_hits, 1);
    assert_eq!(summary.total_misses, 0);
    assert!((summary.accuracy_pct - 100.0).abs() < 1e-6);
    assert_eq!(summary.trial_reaction_times_us.len(), 1);
    // In sample_session: target 1 spawned at 1000, first movement at 2000 -> 1000 us (1.0 ms)
    assert_eq!(summary.trial_reaction_times_us[0], Some(1000));
}

#[test]
fn test_summary_with_pause_in_middle_of_trial() {
    let mut session = sample_session();
    // Target spawned at 1_000_000, hit at 4_000_000
    // Movement starts at 3_500_000.
    // Pause from 2_000_000 to 3_000_000 (duration 1_000_000 us).
    session.streams.targets.records = vec![TargetRecord {
        id: 1,
        spawn_t_us: 1_000_000,
        despawn_t_us: 4_000_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.5,
            initial_pos: [10.0, 0.0, 15.0],
            path_spline: None,
        },
    }];
    session.streams.input.t_us = vec![1_000_000, 3_500_000, 4_000_000];
    session.streams.input.dx = vec![0, 10, 5];
    session.streams.input.dy = vec![0, 0, 0];
    session.streams.shots.records = vec![ShotRecord {
        t_us: 4_000_000,
        hit_target_id: Some(1),
        hitzone: HitZone::Head,
    }];
    session.streams.keys.records = vec![
        KeyRecord {
            t_us: 2_000_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: 3_000_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
    ];

    let summary = compute_session_summary(&session);
    assert_eq!(summary.pauses.len(), 1);
    assert_eq!(summary.pauses[0], (2_000_000, 3_000_000));
    // Wall time between spawn (1_000_000) and movement (3_500_000) is 2_500_000 us.
    // Minus pause of 1_000_000 us = 1_500_000 us (1.5s).
    assert_eq!(summary.trial_reaction_times_us[0], Some(1_500_000));
}
