use aim_scenarios::schema::*;
use aim_scenarios::trajectory::target_angles_at;
use aim_telemetry::*;

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

    // Serialize trajectory + seed as machine-parseable RON
    let spline_ron =
        ron::to_string(&(trajectory.clone(), seed)).expect("Failed serializing spline");

    // Write into an AftSession
    let mut session = AftSession {
        header: AftHeader {
            session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
            created_at_utc: "2026-10-19T18:00:00Z".to_string(),
            app_version: "0.1.0".to_string(),
            trainer_version: "0.1.0".to_string(),
            scenario_id: sc.id.clone(),
            scenario_version: sc.version,
            form: Some(0),
            seed,
            game_profile: "valorant.v1".to_string(),
            mouse: MouseConfig {
                dpi: 800,
                sens: 0.35,
                deg_per_count: 0.0245,
            },
            display: DisplayConfig {
                width: 2560,
                height: 1440,
                hz: 165,
                hfov: 103.0,
                vfov: 70.53,
            },
            input: InputConfig {
                est_polling_hz: 1000,
            },
            thresholds_version: "thresholds.v1".to_string(),
            quality_flags: 0,
        },
        streams: AftStreams::default(),
    };

    session.streams.targets.records.push(TargetRecord {
        id: 1,
        spawn_t_us: 0,
        despawn_t_us: 45_000_000,
        kind: TargetKind::Sphere,
        params: TargetParams {
            radius_deg: 0.6,
            initial_pos: [0.0, 0.0, 15.0],
            path_spline: Some(spline_ron),
        },
    });

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
