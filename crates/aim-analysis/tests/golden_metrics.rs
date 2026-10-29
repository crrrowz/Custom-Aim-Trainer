use aim_analysis::compute_engagement_metrics;
use aim_telemetry::format::{
    HitZone, KeyCode, KeyRecord, ShotRecord, TargetKind, TargetParams, TargetRecord,
};
use aim_telemetry::sample_session;

#[test]
fn test_golden_synthetic_flick_no_pause() {
    let mut session = sample_session();
    // Setup target 1: spawn at 1_000_000, target at (10.0, 0.0) -> distance D = 10.0 deg.
    session.header.mouse.sens = 1.0;
    session.header.mouse.deg_per_count = 0.022; // 0.022 deg per count

    session.streams.targets.records = vec![TargetRecord {
        id: 1,
        spawn_t_us: 1_000_000,
        despawn_t_us: 1_500_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.5,
            initial_pos: [10.0, 0.0, 15.0],
            path_spline: None,
        },
    }];

    // Synthetic trajectory:
    // 1_000_000: spawn (stationary at 0.0)
    // 1_200_000: reaction time = 200 ms (first movement starts here)
    // 1_200_000 -> 1_350_000: primary flick moving towards target
    // At 1_350_000: crosshair reaches yaw 11.0 deg (OVERSHOOT: 11.0 / 10.0 = 1.10 gain)
    // 1_350_000 -> 1_450_000: correction submovement moving back from 11.0 to 10.0 deg
    // 1_500_000: shot fired and hits target 1 (time to hit = 500 ms)

    session.streams.input.t_us = vec![
        1_000_000, 1_200_000, 1_250_000, 1_300_000, 1_350_000, // end of primary flick
        1_400_000, 1_450_000, // correction finished
        1_500_000, // shot
    ];
    session.streams.input.dx = vec![
        0, 100, 200, 150, 50, // total = 500 counts = 11.0 deg
        -25, -20, // total correction = -45 counts = -0.99 deg (~10.0 deg)
        0,
    ];
    session.streams.input.dy = vec![0; 8];

    session.streams.shots.records = vec![ShotRecord {
        t_us: 1_500_000,
        hit_target_id: Some(1),
        hitzone: HitZone::Head,
    }];

    let metrics = compute_engagement_metrics(&session);
    assert_eq!(metrics.len(), 1);
    let m = &metrics[0];

    assert_eq!(m.target_id, 1);
    // Reaction time: 1_200_000 - 1_000_000 = 200_000 us (200 ms)
    assert_eq!(m.reaction_time_us, Some(200_000));
    // Time to hit: 1_500_000 - 1_000_000 = 500_000 us (500 ms)
    assert_eq!(m.time_to_hit_us, Some(500_000));
    // Overshoot: primary reached 11.0 deg on a 10.0 deg target -> gain = 1.10
    assert!(m.gain_ratio.is_some());
    let gain = m.gain_ratio.unwrap();
    assert!(
        (gain - 1.10).abs() < 0.02,
        "Expected gain ~1.10, got {}",
        gain
    );
    assert_eq!(m.correction_count, 1);
}

#[test]
fn test_golden_synthetic_flick_with_pause_exclusion() {
    let mut session = sample_session();
    // Same target as above, but with a 2-second pause in the middle!
    session.header.mouse.sens = 1.0;
    session.header.mouse.deg_per_count = 0.022;

    session.streams.targets.records = vec![TargetRecord {
        id: 1,
        spawn_t_us: 1_000_000,
        despawn_t_us: 3_500_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.5,
            initial_pos: [10.0, 0.0, 15.0],
            path_spline: None,
        },
    }];

    // Pause between 1_100_000 and 3_100_000 (2_000_000 us)
    session.streams.keys.records = vec![
        KeyRecord {
            t_us: 1_100_000,
            key: KeyCode::Unknown(0x1B),
            down: true,
        },
        KeyRecord {
            t_us: 3_100_000,
            key: KeyCode::Unknown(0x1B),
            down: false,
        },
    ];

    // First movement at 3_300_000 (wall time diff = 2_300_000 us; minus 2_000_000 pause = 300_000 us = 300 ms)
    // Shot at 3_500_000 (wall time diff = 2_500_000 us; minus 2_000_000 pause = 500_000 us = 500 ms)
    session.streams.input.t_us = vec![
        1_000_000, 3_300_000, 3_350_000, 3_400_000, 3_450_000, 3_500_000,
    ];
    session.streams.input.dx = vec![0, 200, 150, 50, 0, 0]; // 400 counts = 8.8 deg -> undershoot (8.8 / 10.0 = 0.88)
    session.streams.input.dy = vec![0; 6];

    session.streams.shots.records = vec![ShotRecord {
        t_us: 3_500_000,
        hit_target_id: Some(1),
        hitzone: HitZone::Body,
    }];

    let metrics = compute_engagement_metrics(&session);
    assert_eq!(metrics.len(), 1);
    let m = &metrics[0];

    assert_eq!(m.reaction_time_us, Some(300_000));
    assert_eq!(m.time_to_hit_us, Some(500_000));
    assert!(m.gain_ratio.is_some());
    let gain = m.gain_ratio.unwrap();
    assert!(
        (gain - 0.88).abs() < 0.02,
        "Expected undershoot gain ~0.88, got {}",
        gain
    );
    assert_eq!(m.correction_count, 0); // Reached 8.8 and stayed, no reversal
}
