use aim_core::ViewAngles;
use aim_scenarios::generator::PeekTrial;
use aim_scenarios::schema::{HitRule, Scenario, ScenarioKind, TargetLayout, TargetShape};
use trainer::layout_engine::{LayoutEngineState, PeeksHitOutcome, PeeksSpawnAction};
use trainer::raw_input::{RawInputEvent, RawInputPayload};
use trainer::recorder::{RecorderSetup, SessionRecorder};
use trainer::replay::angles_at_timestamp;

#[test]
fn test_scripted_input_head_level_peeks_pitch_recording() {
    let scenario = Scenario {
        id: "assess.v1.head_level_peeks".to_string(),
        version: 1,
        kind: ScenarioKind::Assessment,
        timeout_s: 150.0,
        spawn_distance_m: 15.0,
        target_shape: TargetShape::WallPeek,
        layout: TargetLayout::HeadLevelPeeks {
            peeks_count: 4,
            occluder_yaws_deg: vec![-25.0, 25.0],
            target_radius_deg: 0.45,
            head_pitch_deg: 1.25,
            pitch_noise_std_deg: 0.4,
            pitch_noise_clamp_deg: 0.8,
            peek_speed_mps: 5.73,
            trial_timeout_s: 2.5,
        },
        hit_rule: HitRule::SingleShotKill,
    };

    let seed = 42;
    let mut engine = LayoutEngineState::new(&scenario, seed, 0);

    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "test-peeks".to_string(),
        scenario_id: scenario.id.clone(),
        form: Some(0),
        seed,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    // Initial camera angles
    let initial_angles = ViewAngles::new(0.0, 0.0);
    let deg_per_count = 0.0245;

    // Scripted input stream:
    // User moves mouse up (+Y in RawInput is pitch down, negative Y is pitch up)
    // 100 counts dy = -100 -> pitch increases by 2.45 deg at t = 10_000 us
    // Another 50 counts dy = -50 -> pitch increases by 1.225 deg at t = 20_000 us
    let scripted_events = vec![
        RawInputEvent {
            t_us: 10_000,
            device_id: 1,
            payload: RawInputPayload::Motion { dx: 0, dy: -100 },
        },
        RawInputEvent {
            t_us: 20_000,
            device_id: 1,
            payload: RawInputPayload::Motion { dx: 0, dy: -50 },
        },
    ];

    // First trial: hit neutral target
    let mut next_tid = 1;
    let neutral_action = engine.handle_peeks_spawn(&mut next_tid, 1_000, 15.0);
    assert!(matches!(
        neutral_action,
        Some(PeeksSpawnAction::SpawnNeutralTarget { .. })
    ));
    let neutral_hit = engine.handle_peeks_hit(true, 5_000);
    assert_eq!(neutral_hit, Some(PeeksHitOutcome::NeutralHit));

    // Peek emerges at QPC time t = 15_000 us (between the two motion events!)
    let peek_spawn_t = 15_000;
    let peek_action = engine.handle_peeks_spawn(&mut next_tid, peek_spawn_t, 15.0);
    let peek_trial: PeekTrial = match peek_action {
        Some(PeeksSpawnAction::SpawnPeekTarget { trial, .. }) => trial,
        _ => panic!("Expected SpawnPeekTarget"),
    };

    // Calculate exact pitch at spawn timestamp using the shared lib function
    let recorded_pitch = trainer::replay::crosshair_pitch_at_qpc_instant(
        &initial_angles,
        &scripted_events,
        peek_spawn_t,
        Some(1),
        deg_per_count,
    );

    let spawn_angles = angles_at_timestamp(
        &initial_angles,
        &scripted_events,
        peek_spawn_t,
        Some(1),
        deg_per_count,
    );

    // At t=15_000, only the first event at t=10_000 (dy = -100) has occurred:
    // pitch should be exactly 0.0 + (-(-100) * 0.0245) = 2.45 degrees!
    assert!(
        (recorded_pitch - 2.45).abs() < 1e-6,
        "Recorded pitch at t=15000us must equal 2.45°, got {}",
        recorded_pitch
    );
    assert!(
        (spawn_angles.pitch - recorded_pitch).abs() < 1e-9,
        "angles_at_timestamp and crosshair_pitch_at_qpc_instant must match"
    );

    recorder.record_view(peek_spawn_t, spawn_angles.yaw, recorded_pitch);
    recorder.record_peek_target_spawn(
        next_tid - 1,
        peek_spawn_t,
        0,
        peek_trial.target_radius_deg,
        peek_trial.occluder_yaw_deg,
        peek_trial.noisy_pitch_deg,
        peek_trial.head_pitch_deg,
    );

    // Verify session recording
    let session = recorder.build_session();
    assert_eq!(session.streams.view.t_us.len(), 1);
    assert_eq!(session.streams.view.t_us[0], 15_000);
    assert!((session.streams.view.pitch_deg[0] - 2.45).abs() < 1e-6);

    let target_rec = &session.streams.targets.records[0];
    assert_eq!(target_rec.kind, aim_telemetry::format::TargetKind::WallPeek);
    assert_eq!(
        target_rec.params.path_spline,
        Some("nominal_head_pitch:1.25".to_string())
    );
    assert!((target_rec.params.initial_pos[1] - peek_trial.noisy_pitch_deg).abs() < 1e-6);
}
