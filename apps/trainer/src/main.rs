//! AimForge native 3D trainer sidecar.
//!
//! Entry point for native Bevy simulation and high-rate Raw Input telemetry recording.

use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use bevy::prelude::*;
use bevy::window::{
    CursorGrabMode, CursorOptions, Monitor, PrimaryMonitor, PrimaryWindow, RawHandleWrapper,
    WindowFocused,
};
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
    pub frame_times_us: Vec<u64>,
    pub last_frame_instant: Instant,
    pub initial_startup_done: bool,
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
        intervals_us: Vec::with_capacity(200_000),
        frame_times_us: Vec::with_capacity(10_000),
        last_frame_instant: Instant::now(),
        initial_startup_done: false,
    });
}

fn sync_display_info_system(
    mut state: ResMut<TrainerState>,
    q_window: Query<(&Window, Option<&RawHandleWrapper>), With<PrimaryWindow>>,
    q_monitor: Query<&Monitor, With<PrimaryMonitor>>,
) {
    if state.initial_startup_done {
        return;
    }

    if let Some((window, raw_handle)) = q_window.iter().next() {
        let width = window.physical_width();
        let height = window.physical_height();

        if width > 0 && height > 0 {
            state.recorder.setup.width = width;
            state.recorder.setup.height = height;

            if let Some(monitor) = q_monitor.iter().next() {
                if let Some(mhz) = monitor.refresh_rate_millihertz {
                    state.recorder.setup.hz = (mhz + 500) / 1000;
                }
            }

            if let Some(wrapper) = raw_handle {
                #[cfg(target_os = "windows")]
                if let raw_window_handle::RawWindowHandle::Win32(win32_handle) =
                    wrapper.get_window_handle()
                {
                    let hwnd = win32_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
                    RawInputWorker::set_target_hwnd(hwnd);
                }
            }

            state.initial_startup_done = true;
        }
    }
}

fn handle_window_focus(
    mut focus_events: MessageReader<WindowFocused>,
    mut state: ResMut<TrainerState>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for ev in focus_events.read() {
        if let Some(mut cursor) = q_cursor.iter_mut().next() {
            if ev.focused {
                cursor.grab_mode = CursorGrabMode::Confined;
                cursor.visible = false;
            } else {
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
                if state.initial_startup_done {
                    state.recorder.flag_focus_lost();
                }
            }
        }
    }
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

        if let Some(last_t) = state.last_event_t_us {
            if event.t_us >= last_t {
                state.intervals_us.push(event.t_us - last_t);
            }
        }
        state.last_event_t_us = Some(event.t_us);

        total_dx += event.dx;
        total_dy += event.dy;
    }

    if total_dx != 0 || total_dy != 0 {
        if let Some(mut fps_cam) = camera_query.iter_mut().next() {
            fps_cam
                .angles
                .apply_counts(total_dx, total_dy, deg_per_count);
        }
    }
}

fn track_frame_time_system(mut state: ResMut<TrainerState>) {
    let now = Instant::now();
    let frame_us = now.duration_since(state.last_frame_instant).as_micros() as u64;
    state.last_frame_instant = now;
    state.frame_times_us.push(frame_us);
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
        println!(
            "Absolute mouse events discarded: {}",
            RawInputWorker::absolute_events_discarded()
        );
        println!("Quality flags: {:#010b}", state.recorder.quality_flags);
        println!(
            "Display configured: {}x{} @ {}Hz",
            state.recorder.setup.width, state.recorder.setup.height, state.recorder.setup.hz
        );

        let active_intervals: Vec<u64> = state
            .intervals_us
            .iter()
            .copied()
            .filter(|&dt| dt <= 50_000)
            .collect();

        if !active_intervals.is_empty() {
            let mut sorted = active_intervals.clone();
            sorted.sort_unstable();

            let count = sorted.len();
            let p50 = sorted[count * 50 / 100];
            let p95 = sorted[count * 95 / 100];
            let p99 = sorted[count * 99 / 100];
            let max = sorted[count - 1];

            let under_500 = sorted.iter().filter(|&&dt| dt < 500).count();
            let over_2000 = sorted.iter().filter(|&&dt| dt > 2000).count();

            let active_time_s = sorted.iter().sum::<u64>() as f64 / 1_000_000.0;
            let active_rate = if active_time_s > 0.0 {
                count as f64 / active_time_s
            } else {
                0.0
            };

            println!("\n--- Raw Input Timing Metrics (during movement) ---");
            println!("Active movement event count: {}", count);
            println!("Active movement capture rate: {:.1} events/s", active_rate);
            println!(
                "Intervals (us): p50 = {}us, p95 = {}us, p99 = {}us, max = {}us",
                p50, p95, p99, max
            );
            println!(
                "Intervals < 500us: {} ({:.2}%)",
                under_500,
                under_500 as f64 * 100.0 / count as f64
            );
            println!(
                "Intervals > 2000us: {} ({:.2}%)",
                over_2000,
                over_2000 as f64 * 100.0 / count as f64
            );
        }

        if state.frame_times_us.len() > 10 {
            let mut frames = state.frame_times_us[5..].to_vec();
            frames.sort_unstable();
            let f_len = frames.len();
            let f_p50 = frames[f_len * 50 / 100] as f64 / 1000.0;
            let f_p99 = frames[f_len * 99 / 100] as f64 / 1000.0;
            println!("\n--- Frame Time Metrics ---");
            println!(
                "Frame times (ms): p50 = {:.2}ms, p99 = {:.2}ms",
                f_p50, f_p99
            );
        }

        let target_path = config.output_path.clone().unwrap_or_else(|| {
            PathBuf::from(format!("session-{}.aft", state.recorder.setup.session_id))
        });

        match state.recorder.save_to_file(&target_path) {
            Ok(bytes) => {
                println!(
                    "\nTelemetry successfully written to: {} ({:.1} KB)",
                    target_path.display(),
                    bytes as f64 / 1024.0
                );
            }
            Err(e) => {
                eprintln!(
                    "\nFailed writing telemetry to {}: {}",
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
                sync_display_info_system,
                (
                    handle_window_focus,
                    drain_raw_input_system,
                    sync_camera_transform,
                )
                    .chain(),
            ),
        )
        .add_systems(
            PostUpdate,
            (track_frame_time_system, session_lifecycle_system),
        )
        .run();
}
