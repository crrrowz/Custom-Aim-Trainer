use aim_telemetry::format::{quality_flags, HitZone};
use aim_telemetry::{read_aft, write_aft};
use trainer::pause::PauseManager;
use trainer::recorder::{RecorderSetup, SessionRecorder};

#[test]
fn test_pause_resume_pure_state_machine_time_accounting() {
    let mut pm = PauseManager::new();
    assert!(!pm.is_paused);
    assert!(!pm.was_paused);
    assert_eq!(pm.total_paused_us, 0);

    // Initial running period: t = 0 to t = 2_000_000 us (2.0s)
    assert_eq!(pm.active_time_us(0), 0);
    assert_eq!(pm.active_time_us(1_000_000), 1_000_000);
    assert_eq!(pm.active_time_us(2_000_000), 2_000_000);

    // Pause at t = 2_000_000 us
    pm.pause(2_000_000);
    assert!(pm.is_paused);
    assert!(pm.was_paused);

    // While paused, active time must remain strictly frozen at 2_000_000 us
    assert_eq!(pm.active_time_us(2_100_000), 2_000_000);
    assert_eq!(pm.active_time_us(2_500_000), 2_000_000);
    assert_eq!(pm.active_time_us(3_000_000), 2_000_000);

    // Resume at t = 3_000_000 us (exactly 1.0s paused)
    pm.resume(3_000_000);
    assert!(!pm.is_paused);
    assert_eq!(pm.total_paused_us, 1_000_000);

    // Active time resumes immediately without time jump
    assert_eq!(pm.active_time_us(3_000_000), 2_000_000);
    assert_eq!(pm.active_time_us(3_500_000), 2_500_000);
    assert_eq!(pm.active_time_us(4_000_000), 3_000_000);

    // Second pause cycle: pause at t = 5_000_000 us (active time = 4_000_000)
    pm.pause(5_000_000);
    assert!(pm.is_paused);
    assert_eq!(pm.active_time_us(5_500_000), 4_000_000);
    assert_eq!(pm.active_time_us(7_000_000), 4_000_000);

    // Resume at t = 7_000_000 us (2.0s paused)
    pm.resume(7_000_000);
    assert!(!pm.is_paused);
    assert_eq!(pm.total_paused_us, 3_000_000);

    // Advance 1s active time to t = 8_000_000 us
    // Active time = 8_000_000 - 3_000_000 = 5_000_000 us (exactly 5.0s active time)
    assert_eq!(pm.active_time_us(8_000_000), 5_000_000);
}

#[test]
fn test_partial_session_save_produces_readable_aft() {
    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
        scenario_id: "assess.v1.flick_static".to_string(),
        form: Some(0),
        seed: 0x1337_CAFE,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    // Simulate partial session: 3 targets spawned, 2 shots, input events
    recorder.record_target_spawn(1, 100_000, 0, 0.5, 0.0, 0.0);
    recorder.record_input(150_000, 10, -5);
    recorder.record_shot(160_000, Some(1), HitZone::Body);
    recorder.record_target_spawn(2, 200_000, 0, 0.8, 15.0, 5.0);
    recorder.record_input(250_000, -20, 15);
    recorder.record_shot(260_000, None, HitZone::None);
    recorder.record_target_spawn(3, 300_000, 0, 0.4, -20.0, -10.0);

    // Mark as partial session with paused and aborted flags
    recorder.flag_paused();
    recorder.quality_flags |= quality_flags::ABORTED;

    // Build session and write to AFT v1 format
    let session = recorder.build_session();
    let bytes = write_aft(&session).expect("Failed to write partial aft");
    assert!(!bytes.is_empty());

    // Read back through read_aft to verify full roundtrip readability
    let decoded = read_aft(&bytes).expect("Failed to decode partial aft with read_aft");

    assert_eq!(decoded.header.scenario_id, "assess.v1.flick_static");
    assert_eq!(decoded.header.form, Some(0));
    assert_eq!(decoded.header.seed, 0x1337_CAFE);
    assert_eq!(
        decoded.header.quality_flags & quality_flags::PAUSED,
        quality_flags::PAUSED
    );
    assert_eq!(
        decoded.header.quality_flags & quality_flags::ABORTED,
        quality_flags::ABORTED
    );

    // Verify streams are intact in partial session
    assert_eq!(decoded.streams.targets.records.len(), 3);
    assert_eq!(decoded.streams.shots.records.len(), 2);
    assert_eq!(decoded.streams.input.t_us.len(), 2);
    assert_eq!(decoded.streams.input.dx, vec![10, -20]);
    assert_eq!(decoded.streams.input.dy, vec![-5, 15]);
}
