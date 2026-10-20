#[test]
fn test_trainer_session_aft_roundtrip() {
    let mut recorder = trainer::recorder::SessionRecorder::new(trainer::recorder::RecorderSetup {
        session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
        scenario_id: "assess.v1.flick_static".to_string(),
        form: Some(0),
        seed: 42,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    for i in 0..1000 {
        recorder.record_input(i * 1000, (i % 5) as i32 - 2, (i % 3) as i32 - 1);
    }

    let session = recorder.build_session();
    let bytes = aim_telemetry::write_aft(&session).expect("write_aft failed");
    let parsed = aim_telemetry::read_aft(&bytes).expect("read_aft failed on trainer output");

    assert_eq!(parsed.header.scenario_id, "assess.v1.flick_static");
    assert_eq!(parsed.streams.input.t_us.len(), 1000);
}
