//! AimForge native 3D trainer sidecar.
//!
//! Entry point for native Bevy simulation and high-rate Raw Input telemetry recording.

use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};
use std::path::PathBuf;
use std::time::Instant;
use trainer::camera::{spawn_fps_camera, sync_camera_transform, FpsCamera};
use trainer::raw_input::RawInputWorker;
use trainer::recorder::{self, SessionRecorder};

#[derive(Resource)]
pub struct TrainerConfig {
    pub scenario: String,
    pub seed: u64,
    pub dpi: u32,
    pub sens: f64,
    pub duration_s: f64,
    pub output_path: Option<PathBuf>,
}

#[derive(Resource)]
pub struct TrainerState {
    pub start_instant: Instant,
    pub worker: RawInputWorker,
    pub recorder: SessionRecorder,
    pub event_count: u64,
    pub last_event_t_us: Option<u64>,
    pub intervals_us: Vec<u64>,
}

fn parse_cli_args() -> TrainerConfig {
    let mut scenario = "assess.v1.flick_static".to_string();
    let mut seed = 1337u64;
    let mut dpi = 800u32;
    let mut sens = 0.35f64;
    let mut duration_s = 60.0f64;
    let mut output_path = None;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scenario" if i + 1 < args.len() => {
                scenario = args[i + 1].clone();
                i += 2;
            }
            "--seed" if i + 1 < args.len() => {
                seed = args[i + 1].parse().unwrap_or(seed);
                i += 2;
            }
            "--dpi" if i + 1 < args.len() => {
                dpi = args[i + 1].parse().unwrap_or(dpi);
                i += 2;
            }
            "--sens" if i + 1 < args.len() => {
                sens = args[i + 1].parse().unwrap_or(sens);
                i += 2;
            }
            "--duration" if i + 1 < args.len() => {
                duration_s = args[i + 1].parse().unwrap_or(duration_s);
                i += 2;
            }
            "--output" if i + 1 < args.len() => {
                output_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    TrainerConfig {
        scenario,
        seed,
        dpi,
        sens,
        duration_s,
        output_path,
    }
}

fn setup_trainer(mut commands: Commands, config: Res<TrainerConfig>) {
    let worker = RawInputWorker::spawn();
    let recorder = SessionRecorder::new(recorder::RecorderSetup {
        session_id: uuid::Uuid::new_v4().to_string(),
        scenario_id: config.scenario.clone(),
        seed: config.seed,
        dpi: config.dpi,
        sens: config.sens,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    commands.insert_resource(TrainerState {
        start_instant: Instant::now(),
        worker,
        recorder,
        event_count: 0,
        last_event_t_us: None,
        intervals_us: Vec::with_capacity(100_000),
    });
}

fn drain_raw_input_system(
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
    mut camera_query: Query<&mut FpsCamera>,
) {
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * config.sens;
    let mut total_dx = 0i32;
    let mut total_dy = 0i32;

    while let Ok(event) = state.worker.receiver.try_recv() {
        state.event_count += 1;
        state.recorder.record_input(event.t_us, event.dx, event.dy);

        // Calculate inter-event intervals for jitter measurement
        if let Some(last_t) = state.last_event_t_us {
            if event.t_us >= last_t {
                state.intervals_us.push(event.t_us - last_t);
            }
        }
        state.last_event_t_us = Some(event.t_us);

        total_dx += event.dx;
        total_dy += event.dy;
    }

    // Apply exact summed deltas to ViewAngles ensuring zero drift
    if total_dx != 0 || total_dy != 0 {
        if let Some(mut fps_cam) = camera_query.iter_mut().next() {
            fps_cam
                .angles
                .apply_counts(total_dx, total_dy, deg_per_count);
        }
    }
}

fn handle_window_focus(
    mut focus_events: MessageReader<WindowFocused>,
    mut state: ResMut<TrainerState>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for ev in focus_events.read() {
        RawInputWorker::set_focused(ev.focused);
        if let Some(mut cursor) = q_cursor.iter_mut().next() {
            if ev.focused {
                cursor.grab_mode = CursorGrabMode::Confined;
                cursor.visible = false;
            } else {
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
                state.recorder.flag_focus_lost();
            }
        }
    }
}

fn session_lifecycle_system(
    config: Res<TrainerConfig>,
    state: Res<TrainerState>,
    mut exit: MessageWriter<AppExit>,
) {
    let elapsed_s = state.start_instant.elapsed().as_secs_f64();
    if elapsed_s >= config.duration_s {
        println!("\n=== AimForge Trainer Session Completed ===");
        println!("Elapsed time: {:.2}s", elapsed_s);
        println!("Raw Input events captured: {}", state.event_count);
        if elapsed_s > 0.0 {
            println!(
                "Average capture rate: {:.1} events/s",
                state.event_count as f64 / elapsed_s
            );
        }

        // Compute jitter stats
        if !state.intervals_us.is_empty() {
            let n = state.intervals_us.len() as f64;
            let mean: f64 = state.intervals_us.iter().map(|&v| v as f64).sum::<f64>() / n;
            let variance: f64 = state
                .intervals_us
                .iter()
                .map(|&v| (v as f64 - mean).powi(2))
                .sum::<f64>()
                / n;
            let std_dev = variance.sqrt();
            println!(
                "Inter-event interval: mean = {:.1}us, jitter (std dev) = {:.1}us (sample count: {})",
                mean, std_dev, state.intervals_us.len()
            );
        }

        // Export .aft telemetry file
        let target_path = config.output_path.clone().unwrap_or_else(|| {
            PathBuf::from(format!("session-{}.aft", state.recorder.setup.session_id))
        });

        match state.recorder.save_to_file(&target_path) {
            Ok(bytes) => {
                println!(
                    "Telemetry successfully written to: {} ({:.1} KB)",
                    target_path.display(),
                    bytes as f64 / 1024.0
                );
            }
            Err(e) => {
                eprintln!(
                    "Failed writing telemetry to {}: {}",
                    target_path.display(),
                    e
                );
            }
        }

        exit.write(AppExit::Success);
    }
}

fn lock_cursor_startup(mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Some(mut cursor) = q_cursor.iter_mut().next() {
        cursor.grab_mode = CursorGrabMode::Confined;
        cursor.visible = false;
    }
}

fn main() {
    let config = parse_cli_args();

    App::new()
        .insert_resource(config)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AimForge Trainer".into(),
                present_mode: bevy::window::PresentMode::Immediate,
                ..default()
            }),
            ..default()
        }))
        .add_systems(
            Startup,
            (setup_trainer, spawn_fps_camera, lock_cursor_startup),
        )
        .add_systems(
            PreUpdate,
            (
                drain_raw_input_system,
                sync_camera_transform,
                handle_window_focus,
            ),
        )
        .add_systems(PostUpdate, session_lifecycle_system)
        .run();
}
