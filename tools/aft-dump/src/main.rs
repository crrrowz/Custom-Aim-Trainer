use aim_core::camera::{angular_distance, ViewAngles};
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_scenarios::schema::TrackingTrajectory;
use aim_scenarios::trajectory::target_angles_at;
use aim_telemetry::format::AftSession;
use aim_telemetry::read_aft;
use std::env;
use std::fs;
use std::path::Path;

fn dump_session(path: &Path, session: &AftSession) {
    println!("=== AFT Dump: {} ===", path.display());
    println!("Scenario: {}", session.header.scenario_id);
    println!("Form: {:?}", session.header.form);
    println!("Seed: {:#x} ({})", session.header.seed, session.header.seed);
    println!("Target Count: {}", session.streams.targets.records.len());

    let shots_count = session.streams.shots.records.len();
    let hits_count = session
        .streams
        .shots
        .records
        .iter()
        .filter(|s| s.hit_target_id.is_some())
        .count();

    println!("Shots: {}, Hits: {}", shots_count, hits_count);
    println!("Input Samples: {}", session.streams.input.t_us.len());
    println!("View Samples: {}", session.streams.view.t_us.len());

    if let (Some(&first_t), Some(&last_t)) = (
        session.streams.input.t_us.first(),
        session.streams.input.t_us.last(),
    ) {
        println!(
            "Input Time Range: {:.3}s .. {:.3}s (span: {:.3}s)",
            first_t as f64 / 1_000_000.0,
            last_t as f64 / 1_000_000.0,
            (last_t.saturating_sub(first_t)) as f64 / 1_000_000.0
        );
    } else {
        println!("Input Time Range: None");
    }

    // Replay input stream to reconstruct crosshair angles at each shot
    let deg_per_count = if session.header.mouse.deg_per_count > 0.0 {
        session.header.mouse.deg_per_count
    } else {
        VALORANT_YAW_DEG_PER_COUNT * session.header.mouse.sens
    };

    println!("\n--- Shot Evaluations ---");
    let mut current_angles = ViewAngles::new(0.0, 0.0);
    let mut input_idx = 0;
    let input_len = session.streams.input.t_us.len();

    for (shot_idx, shot) in session.streams.shots.records.iter().enumerate() {
        // Advance input stream up to shot.t_us
        while input_idx < input_len && session.streams.input.t_us[input_idx] <= shot.t_us {
            let dx = session.streams.input.dx[input_idx];
            let dy = session.streams.input.dy[input_idx];
            current_angles.apply_counts(dx, dy, deg_per_count);
            input_idx += 1;
        }

        let target_info = if let Some(tid) = shot.hit_target_id {
            if let Some(target) = session.streams.targets.records.iter().find(|t| t.id == tid) {
                let target_angles = if let Some(spline_str) = &target.params.path_spline {
                    if let Ok((traj, seed)) = ron::from_str::<(TrackingTrajectory, u64)>(spline_str)
                    {
                        let rel_t_us = shot.t_us.saturating_sub(target.spawn_t_us);
                        target_angles_at(&traj, seed, rel_t_us)
                    } else {
                        ViewAngles::new(target.params.initial_pos[0], target.params.initial_pos[1])
                    }
                } else {
                    ViewAngles::new(target.params.initial_pos[0], target.params.initial_pos[1])
                };

                let dist = angular_distance(current_angles, target_angles);
                let dyaw = current_angles.yaw - target_angles.yaw;
                let dpitch = current_angles.pitch - target_angles.pitch;

                format!(
                    "HIT Target #{} ({:?}) | offset: {:.2}° (dyaw: {:.2}°, dpitch: {:.2}°) | crosshair: ({:.2}°, {:.2}°) target: ({:.2}°, {:.2}°)",
                    tid, shot.hitzone, dist, dyaw, dpitch, current_angles.yaw, current_angles.pitch, target_angles.yaw, target_angles.pitch
                )
            } else {
                format!("HIT Target #{} (not found in targets stream)", tid)
            }
        } else {
            "MISS".to_string()
        };

        println!(
            "Shot #{}: t={:.3}s -> {}",
            shot_idx + 1,
            shot.t_us as f64 / 1_000_000.0,
            target_info
        );
    }
    println!();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: aft-dump <file.aft> [more_files.aft...]");
        std::process::exit(1);
    }

    for file_arg in &args[1..] {
        let path = Path::new(file_arg);
        match fs::read(path) {
            Ok(bytes) => match read_aft(&bytes) {
                Ok(session) => dump_session(path, &session),
                Err(e) => eprintln!("Failed parsing aft from {}: {}", path.display(), e),
            },
            Err(e) => eprintln!("Failed reading file {}: {}", path.display(), e),
        }
    }
}
