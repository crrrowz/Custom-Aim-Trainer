use aim_scenarios::schema::*;
use aim_scenarios::trajectory::target_angles_at;
use aim_telemetry::*;
use trainer::layout_engine::LayoutEngineState;
use trainer::recorder::{RecorderSetup, SessionRecorder};

#[test]
fn test_tracking_path_spline_ron_reproducibility() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.smooth_track.ron"
    ))
    .unwrap();

    let trajectory = match &sc.layout {
        TargetLayout::Tracking { trajectory, .. } => trajectory.clone(),
        _ => panic!("Expected tracking layout"),
    };

    let seed = 0x5A17_F000_0000_0001;

    // Use SessionRecorder through the trainer engine flow
    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
        scenario_id: sc.id.clone(),
        form: Some(0),
        seed,
        dpi: 800,
        sens: 0.35,
        width: 2560,
        height: 1440,
        hz: 165,
    });

    let mut engine = LayoutEngineState::new(&sc, seed, 0);
    let mut next_tid = 1;
    let spawn_action = engine
        .handle_tracking_spawn(&mut next_tid, 0, 15.0)
        .expect("Failed to spawn tracking target");

    // Record via SessionRecorder's real game path method
    recorder.record_tracking_target_spawn(
        spawn_action.target_id,
        0,
        45_000_000,
        spawn_action.radius_deg,
        spawn_action.initial_angles.yaw,
        spawn_action.initial_angles.pitch,
        spawn_action.spline_ron,
    );

    // Build session and write .aft
    let session = recorder.build_session();
    let bytes = write_aft(&session).expect("Failed writing aft");
    let decoded = read_aft(&bytes).expect("Failed reading aft");

    // Deserialize spline RON from decoded .aft
    let decoded_spline = decoded.streams.targets.records[0]
        .params
        .path_spline
        .as_ref()
        .expect("Missing path_spline");

    let (recovered_traj, recovered_seed): (TrackingTrajectory, u64) =
        ron::from_str(decoded_spline).expect("Failed parsing path_spline RON");

    assert_eq!(recovered_traj, trajectory);
    assert_eq!(recovered_seed, seed);

    // Verify exact angle reproduction at every second across the run
    for t_s in 0..45 {
        let t_us = t_s * 1_000_000;
        let p_original = target_angles_at(&trajectory, seed, t_us);
        let p_reproduced = target_angles_at(&recovered_traj, recovered_seed, t_us);

        assert_eq!(p_original.yaw, p_reproduced.yaw);
        assert_eq!(p_original.pitch, p_reproduced.pitch);
    }
}
