use aim_scenarios::schema::{
    DistanceAnchor, FittsGrid, HitRule, Scenario, ScenarioKind, TargetLayout, TargetShape,
};
use aim_telemetry::format::{quality_flags, KeyCode};
use aim_telemetry::read_aft;
use trainer::layout_engine::{FittsHitOutcome, FittsSpawnAction, LayoutEngineState};
use trainer::pause::PauseManager;
use trainer::raw_input::{RawInputEvent, RawInputPayload};
use trainer::recorder::{RecorderSetup, SessionRecorder};
use trainer::replay::replay_input;

#[test]
fn test_pause_reaction_time_accounting() {
    let scenario = Scenario {
        id: "assess.v1.flick_static".to_string(),
        version: 1,
        kind: ScenarioKind::Assessment,
        timeout_s: 288.0,
        spawn_distance_m: 15.0,
        target_shape: TargetShape::Sphere,
        layout: TargetLayout::Fitts {
            grid: FittsGrid {
                distances_deg: vec![8.0],
                radius_sizes_deg: vec![0.8],
                octants: 1,
                repetitions_per_cell: 1,
                anchor: DistanceAnchor::HomeTargetCenter,
            },
            trial_timeout_s: 3.0,
        },
        hit_rule: HitRule::SingleShotKill,
    };

    let seed = 42;
    let mut engine = LayoutEngineState::new(&scenario, seed, 0);
    let mut pm = PauseManager::new();

    let mut next_tid = 1;
    // 1. Home target hit
    engine.handle_fitts_spawn(&mut next_tid, 0, 15.0);
    engine.handle_fitts_hit(true, 500_000);

    // 2. Spawn trial target at QPC 1.0 s (active time 1.0 s)
    let qpc_spawn = 1_000_000u64;
    let active_spawn = pm.active_time_us(qpc_spawn);
    assert_eq!(active_spawn, 1_000_000);
    let action = engine.handle_fitts_spawn(&mut next_tid, active_spawn, 15.0);
    assert!(matches!(action, Some(FittsSpawnAction::SpawnTrial { .. })));

    // 3. Pause at QPC 1.0 s -> resume at QPC 31.0 s (paused for 30.0 s)
    pm.pause(1_000_000);
    pm.resume(31_000_000);
    assert_eq!(pm.total_paused_us, 30_000_000);

    // On resume, the in-flight trial is voided (trial_start reset to resume active time)
    let resume_active = pm.active_time_us(31_000_000);
    assert_eq!(resume_active, 1_000_000);
    engine.void_in_flight_trial_on_resume(resume_active);

    // 4. Hit at QPC 31.5 s
    let qpc_hit = 31_500_000u64;
    let active_hit = pm.active_time_us(qpc_hit);
    assert_eq!(active_hit, 1_500_000);

    let outcome = engine.handle_fitts_hit(false, active_hit);
    match outcome {
        Some(FittsHitOutcome::TrialHit {
            trial_elapsed_s, ..
        }) => {
            // Reaction time must be exactly 0.5 s, NOT 30.5 s!
            assert!(
                (trial_elapsed_s - 0.5).abs() < 1e-4,
                "Expected reaction time ~0.5s after pause, got {}s",
                trial_elapsed_s
            );
        }
        _ => panic!("Expected TrialHit"),
    }
}

#[test]
fn test_aft_streams_monotonic_and_esc_key_records() {
    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "test-session".to_string(),
        scenario_id: "assess.v1.flick_static".to_string(),
        form: Some(0),
        seed: 42,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    // Record sequence with pause
    recorder.record_frame(100_000, 100_000);
    recorder.record_input(100_500, 2, 3);
    recorder.record_view(100_500, 0.0, 0.0);
    recorder.record_target_spawn(1, 100_500, 0, 0.5, 0.0, 0.0);

    // Pause at 1_000_000 us: Esc down
    recorder.flag_paused();
    recorder.record_key(1_000_000, KeyCode::Unknown(0x1B), true);

    // Resume at 5_000_000 us: Esc up
    recorder.record_key(5_000_000, KeyCode::Unknown(0x1B), false);

    // Action post-resume
    recorder.record_frame(5_100_000, 5_100_000);
    recorder.record_input(5_100_500, -2, 1);
    recorder.record_view(5_100_500, 1.0, 0.5);
    recorder.record_shot(5_200_000, Some(1), aim_telemetry::format::HitZone::Body);

    let session = recorder.build_session();
    let bytes = aim_telemetry::write_aft(&session).unwrap();
    let decoded = read_aft(&bytes).unwrap();

    // Check monotonic timestamps in every stream
    assert!(decoded.streams.frames.t_us.windows(2).all(|w| w[0] <= w[1]));
    assert!(decoded.streams.input.t_us.windows(2).all(|w| w[0] <= w[1]));
    assert!(decoded.streams.view.t_us.windows(2).all(|w| w[0] <= w[1]));
    assert!(decoded
        .streams
        .targets
        .records
        .windows(2)
        .all(|w| w[0].spawn_t_us <= w[1].spawn_t_us));
    assert!(decoded
        .streams
        .shots
        .records
        .windows(2)
        .all(|w| w[0].t_us <= w[1].t_us));
    assert!(decoded
        .streams
        .keys
        .records
        .windows(2)
        .all(|w| w[0].t_us <= w[1].t_us));

    // Verify Esc key pause intervals
    assert_eq!(decoded.streams.keys.records.len(), 2);
    assert_eq!(decoded.streams.keys.records[0].t_us, 1_000_000);
    assert_eq!(decoded.streams.keys.records[0].key, KeyCode::Unknown(0x1B));
    assert!(decoded.streams.keys.records[0].down);

    assert_eq!(decoded.streams.keys.records[1].t_us, 5_000_000);
    assert_eq!(decoded.streams.keys.records[1].key, KeyCode::Unknown(0x1B));
    assert!(!decoded.streams.keys.records[1].down);
}

#[test]
fn test_resume_click_and_motion_filtered_before_replay() {
    let mut pm = PauseManager::new();
    pm.pause(1_000_000);
    pm.resume(5_000_000);
    assert_eq!(pm.resume_qpc_us, 5_000_000);

    let mut cam_angles = aim_core::camera::ViewAngles::new(0.0, 0.0);
    let events = vec![
        RawInputEvent {
            t_us: 4_998_000, // Motion while paused / clicking UI
            device_id: 1,
            payload: RawInputPayload::Motion { dx: 100, dy: 100 },
        },
        RawInputEvent {
            t_us: 4_999_000, // Click occurred during pause / UI resume button click
            device_id: 1,
            payload: RawInputPayload::LeftButtonDown,
        },
        RawInputEvent {
            t_us: 5_001_000, // Legitimate click in game after resume
            device_id: 1,
            payload: RawInputPayload::LeftButtonDown,
        },
    ];

    // Filter batch before replay_input just as done in drain_and_evaluate_input_system
    let valid_batch: Vec<_> = events
        .into_iter()
        .filter(|ev| ev.t_us >= pm.resume_qpc_us)
        .collect();

    assert_eq!(valid_batch.len(), 1);
    assert_eq!(valid_batch[0].t_us, 5_001_000);

    let actions = replay_input(&mut cam_angles, &valid_batch, Some(1), 0.0245);
    assert_eq!(actions.len(), 1);

    // Camera angles must remain strictly at (0.0, 0.0) because paused motion was dropped before replay
    assert_eq!(cam_angles.yaw, 0.0);
    assert_eq!(cam_angles.pitch, 0.0);

    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "leak-test".to_string(),
        scenario_id: "test".to_string(),
        form: None,
        seed: 1,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    for action in actions {
        if let trainer::replay::ReplayAction::Click { t_us, .. } = action {
            recorder.record_shot(t_us, None, aim_telemetry::format::HitZone::None);
        }
    }

    assert_eq!(
        recorder.streams.shots.records.len(),
        1,
        "Resume click before resume_qpc_us must NOT be recorded as a shot"
    );
    assert_eq!(recorder.streams.shots.records[0].t_us, 5_001_000);
}

#[test]
fn test_timed_normal_end_vs_aborted_flag() {
    // 1. Normal session completion: no ABORTED bit
    let normal_recorder = SessionRecorder::new(RecorderSetup {
        session_id: "normal".to_string(),
        scenario_id: "assess.v1.flick_static".to_string(),
        form: Some(0),
        seed: 123,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });
    // Normal end saves with is_aborted = false
    let normal_session = normal_recorder.build_session();
    assert_eq!(
        normal_session.header.quality_flags & quality_flags::ABORTED,
        0,
        "Normal session end must not have ABORTED flag"
    );

    // 2. Aborted session (Quit & Save): has ABORTED bit
    let mut abort_recorder = SessionRecorder::new(RecorderSetup {
        session_id: "abort".to_string(),
        scenario_id: "assess.v1.flick_static".to_string(),
        form: Some(0),
        seed: 123,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });
    abort_recorder.quality_flags |= quality_flags::ABORTED;
    let abort_session = abort_recorder.build_session();
    assert_eq!(
        abort_session.header.quality_flags & quality_flags::ABORTED,
        quality_flags::ABORTED,
        "Quit & Save session must have ABORTED flag set"
    );
}
