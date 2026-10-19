use aim_telemetry::*;

fn canonical_v1_session() -> AftSession {
    AftSession {
        header: AftHeader {
            session_id: "018f4a38-5182-7212-bf96-b0ad511b81b2".to_string(),
            created_at_utc: "2026-10-19T08:30:00Z".to_string(),
            app_version: "0.1.0".to_string(),
            trainer_version: "0.1.0".to_string(),
            scenario_id: "assess.v1.flick_static".to_string(),
            scenario_version: 1,
            form: Some(1),
            seed: 42,
            game_profile: "valorant.v1".to_string(),
            mouse: MouseConfig {
                dpi: 800,
                sens: 0.35,
                deg_per_count: 0.07 * 0.35,
            },
            display: DisplayConfig {
                width: 1920,
                height: 1080,
                hz: 240,
                hfov: 103.0,
                vfov: 70.53,
            },
            input: InputConfig {
                est_polling_hz: 1000,
            },
            thresholds_version: "thresholds.v1".to_string(),
            quality_flags: 0,
        },
        streams: AftStreams {
            input: InputStream {
                t_us: vec![1000, 2000, 3000, 4000, 5000],
                dx: vec![0, 2, 4, -1, 0],
                dy: vec![0, -1, 0, 1, 0],
            },
            frames: FrameStream {
                t_us: vec![4166, 8333, 12500],
                present_t_us: vec![4300, 8450, 12620],
            },
            view: ViewStream {
                t_us: vec![1000, 2000, 3000],
                yaw_deg: vec![0.0, 0.049, 0.147],
                pitch_deg: vec![0.0, -0.0245, -0.0245],
            },
            targets: TargetStream {
                records: vec![TargetRecord {
                    id: 1,
                    spawn_t_us: 1000,
                    despawn_t_us: 450000,
                    kind: TargetKind::HeadModel,
                    params: TargetParams {
                        radius_deg: 0.45,
                        initial_pos: [15.0, 0.0, 20.0],
                        path_spline: None,
                    },
                }],
            },
            shots: ShotStream {
                records: vec![ShotRecord {
                    t_us: 450000,
                    hit_target_id: Some(1),
                    hitzone: HitZone::Head,
                }],
            },
            keys: KeyStream {
                records: vec![
                    KeyRecord {
                        t_us: 1000,
                        key: KeyCode::KeyA,
                        down: true,
                    },
                    KeyRecord {
                        t_us: 250000,
                        key: KeyCode::KeyD,
                        down: true,
                    },
                ],
            },
            player: PlayerStream {
                t_us: vec![1000, 2000, 3000],
                vel_x: vec![-1.2, 0.0, 2.5],
                vel_z: vec![0.0, 0.0, 0.0],
            },
        },
    }
}

#[test]
fn test_enum_variant_postcard_indices_lock() {
    // HitZone index assertions (postcard encodes unit variants as 0-indexed varints)
    assert_eq!(postcard::to_allocvec(&HitZone::None).unwrap(), vec![0]);
    assert_eq!(postcard::to_allocvec(&HitZone::Head).unwrap(), vec![1]);
    assert_eq!(postcard::to_allocvec(&HitZone::Body).unwrap(), vec![2]);
    assert_eq!(postcard::to_allocvec(&HitZone::Limb).unwrap(), vec![3]);

    // KeyCode index assertions
    assert_eq!(postcard::to_allocvec(&KeyCode::KeyW).unwrap(), vec![0]);
    assert_eq!(postcard::to_allocvec(&KeyCode::KeyA).unwrap(), vec![1]);
    assert_eq!(postcard::to_allocvec(&KeyCode::KeyS).unwrap(), vec![2]);
    assert_eq!(postcard::to_allocvec(&KeyCode::KeyD).unwrap(), vec![3]);
    assert_eq!(postcard::to_allocvec(&KeyCode::Space).unwrap(), vec![4]);
    assert_eq!(postcard::to_allocvec(&KeyCode::Shift).unwrap(), vec![5]);
    assert_eq!(postcard::to_allocvec(&KeyCode::Ctrl).unwrap(), vec![6]);

    // TargetKind index assertions
    assert_eq!(postcard::to_allocvec(&TargetKind::Sphere).unwrap(), vec![0]);
    assert_eq!(
        postcard::to_allocvec(&TargetKind::HeadModel).unwrap(),
        vec![1]
    );
    assert_eq!(
        postcard::to_allocvec(&TargetKind::WallPeek).unwrap(),
        vec![2]
    );
}

#[test]
fn test_golden_v1_fixture_compatibility() {
    let fixture_path = std::path::Path::new("tests/fixtures/golden_v1.aft");
    let canonical = canonical_v1_session();

    // If fixture does not exist, generate it deterministically
    if !fixture_path.exists() {
        let bytes = write_aft(&canonical).expect("Failed generating golden fixture");
        std::fs::write(fixture_path, &bytes).expect("Failed writing golden fixture file");
    }

    let fixture_bytes = std::fs::read(fixture_path).expect("Failed reading golden fixture file");
    let decoded = read_aft(&fixture_bytes).expect("Failed decoding golden v1 fixture");

    assert_eq!(
        canonical, decoded,
        "Decoded session differed from canonical v1 definition!"
    );
}
