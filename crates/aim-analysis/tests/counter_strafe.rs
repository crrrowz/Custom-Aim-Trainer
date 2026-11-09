use aim_analysis::{analyze_sessions, AnalysisThresholds, SkillId};
use aim_telemetry::format::{
    AftSession, FrameStream, HitZone, PlayerStream, ShotRecord, TargetKind, TargetParams,
    TargetRecord,
};
use aim_telemetry::sample_session;

/// 20 flicks. `moving_every` of them are fired at running speed; the rest 120 ms after stopping.
fn session(moving_every: u64) -> AftSession {
    let mut s = sample_session();
    s.header.scenario_id = "assess.v1.counter_strafe".to_string();
    s.header.mouse.deg_per_count = 0.02;
    s.streams.keys.records.clear();
    s.streams.frames = FrameStream {
        t_us: Vec::new(),
        present_t_us: Vec::new(),
    };
    s.streams.targets.records.clear();
    s.streams.shots.records.clear();
    let mut player = PlayerStream {
        t_us: Vec::new(),
        vel_x: Vec::new(),
        vel_z: Vec::new(),
    };
    let (mut t_us, mut dx) = (Vec::new(), Vec::new());
    for i in 0..20_u64 {
        let spawn = 1_000_000 + i * 1_500_000;
        let shot = spawn + 600_000;
        for k in 0..100 {
            t_us.push(spawn + 200_000 + k * 1_000);
            dx.push(5);
        }
        s.streams.targets.records.push(TargetRecord {
            id: i as u32 + 1,
            spawn_t_us: spawn,
            despawn_t_us: spawn + 1_400_000,
            kind: TargetKind::Sphere,
            params: TargetParams {
                radius_deg: 0.8,
                initial_pos: [(i + 1) as f64 * 10.0, 0.0, 15.0],
                path_spline: None,
            },
        });
        s.streams.shots.records.push(ShotRecord {
            t_us: shot,
            hit_target_id: Some(i as u32 + 1),
            hitzone: HitZone::Head,
        });
        let running_at_shot = moving_every > 0 && i % moving_every == 0;
        let stop = if running_at_shot {
            shot + 100_000
        } else {
            shot - 120_000
        };
        let mut t = spawn;
        while t < spawn + 1_400_000 {
            player.t_us.push(t);
            player.vel_x.push(if t < stop { 5.4 } else { 0.0 });
            player.vel_z.push(0.0);
            t += 4_000;
        }
    }
    s.streams.input.dy = vec![0; dx.len()];
    s.streams.input.t_us = t_us;
    s.streams.input.dx = dx;
    s.streams.player = player;
    s
}

#[test]
fn moving_shot_rate_is_measured() {
    let p = analyze_sessions(&[session(4)], &AnalysisThresholds::default());
    let s = p.skill(SkillId::S12CounterStrafe).unwrap();
    assert!((s.raw - 0.25).abs() < 1e-9, "{}", s.raw);
    assert_eq!(p.evidence.moving_shot_rate, Some(0.25));
}

#[test]
fn stopping_before_every_shot_scores_full_marks() {
    let p = analyze_sessions(&[session(0)], &AnalysisThresholds::default());
    let s = p.skill(SkillId::S12CounterStrafe).unwrap();
    assert_eq!(s.raw, 0.0);
    assert_eq!(s.score, 100.0);
}

#[test]
fn stop_to_shot_time_is_measured() {
    let p = analyze_sessions(&[session(0)], &AnalysisThresholds::default());
    let ms = p.evidence.median_stop_to_shot_ms.unwrap();
    assert!((ms - 120.0).abs() < 8.0, "{ms}");
}

#[test]
fn other_scenarios_do_not_produce_the_skill() {
    let mut s = session(4);
    s.header.scenario_id = "assess.v1.flick_static".to_string();
    let p = analyze_sessions(&[s], &AnalysisThresholds::default());
    assert!(p.skill(SkillId::S12CounterStrafe).is_none());
}
