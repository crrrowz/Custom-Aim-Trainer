//! AimForge native 3D trainer sidecar.
//!
//! Entry point for native Bevy simulation, Fitts target state machine, and Raw Input telemetry recording.

use aim_core::geometry::HitZoneType;
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_scenarios::{generate_trials, Scenario, Trial};
use aim_telemetry::format::HitZone;
use bevy::input::mouse::MouseButton;
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
use trainer::target::{
    setup_target_assets, spawn_home_target, spawn_trial_target, ActiveTargetMarker, TargetAssets,
};

#[derive(Resource)]
pub struct TrainerConfig {
    pub scenario: String,
    pub seed: u64,
    pub dpi: u32,
    pub sens: f64,
    pub duration_s: f64,
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentPhase {
    AwaitingHomeTarget,
    AwaitingTrialTarget,
    Completed,
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

    // Task 1.9 & Scenario state machine
    pub scenario_def: Scenario,
    pub trials: Vec<Trial>,
    pub current_trial_idx: usize,
    pub phase: AssessmentPhase,
    pub next_target_id: u32,
    pub hits_count: u32,
    pub misses_count: u32,
    pub trial_start_instant: Instant,
}

fn parse_cli_args() -> TrainerConfig {
    let mut scenario = "assess.v1.flick_static".to_string();
    let mut seed = 1337u64;
    let mut dpi = 800u32;
    let mut sens = 0.35f64;
    let mut duration_s = 288.0f64; // Default to full assessment ceiling
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

    // Load scenario definition
    let ron_str = include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
    let scenario_def: Scenario =
        ron::from_str(ron_str).expect("Failed parsing assess.v1.flick_static.ron");
    let trials = generate_trials(&scenario_def, config.seed);

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

        scenario_def,
        trials,
        current_trial_idx: 0,
        phase: AssessmentPhase::AwaitingHomeTarget,
        next_target_id: 1,
        hits_count: 0,
        misses_count: 0,
        trial_start_instant: Instant::now(),
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

/// Task 1.9: Ray-sphere hit registration & SingleShotKill trial management
fn shooting_and_target_lifecycle_system(
    mut commands: Commands,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<TrainerState>,
    assets: Option<Res<TargetAssets>>,
    camera_query: Query<&FpsCamera>,
    target_query: Query<(Entity, &ActiveTargetMarker)>,
) {
    let assets = match assets {
        Some(a) => a,
        None => return,
    };

    let now_s = state.start_instant.elapsed().as_secs_f64();
    let current_qpc_diff = state.start_instant.elapsed().as_micros() as u64;

    // 1. Initial spawn if no target exists
    if target_query.iter().next().is_none() && state.phase != AssessmentPhase::Completed {
        let tid = state.next_target_id;
        state.next_target_id += 1;

        match state.phase {
            AssessmentPhase::AwaitingHomeTarget => {
                spawn_home_target(
                    &mut commands,
                    &assets,
                    state.scenario_def.spawn_distance_m as f32,
                    tid,
                    now_s,
                );
                state
                    .recorder
                    .record_target_spawn(tid, current_qpc_diff, 0, 0.5, 0.0, 0.0);
            }
            AssessmentPhase::AwaitingTrialTarget => {
                if state.current_trial_idx < state.trials.len() {
                    let trial = state.trials[state.current_trial_idx].clone();
                    spawn_trial_target(
                        &mut commands,
                        &assets,
                        &trial,
                        state.scenario_def.spawn_distance_m as f32,
                        tid,
                        now_s,
                    );
                    state.recorder.record_target_spawn(
                        tid,
                        current_qpc_diff,
                        0,
                        trial.radius_deg,
                        trial.yaw_deg,
                        trial.pitch_deg,
                    );
                    state.trial_start_instant = Instant::now();
                } else {
                    state.phase = AssessmentPhase::Completed;
                }
            }
            AssessmentPhase::Completed => {}
        }
    }

    // 2. Check per-trial timeout (3.0s) during AwaitingTrialTarget
    if state.phase == AssessmentPhase::AwaitingTrialTarget {
        let trial_elapsed_s = state.trial_start_instant.elapsed().as_secs_f64();
        if trial_elapsed_s >= state.scenario_def.trial_timeout_s {
            // Timeout count as miss under SingleShotKill rule
            state.misses_count += 1;
            println!(
                "[Trial {}/96] TIMEOUT ({:.2}s). Total Hits: {}, Misses: {}",
                state.current_trial_idx + 1,
                trial_elapsed_s,
                state.hits_count,
                state.misses_count
            );

            // Despawn current trial target and transition back to home target
            for (entity, _) in &target_query {
                commands.entity(entity).despawn();
            }

            state.current_trial_idx += 1;
            if state.current_trial_idx < state.trials.len() {
                state.phase = AssessmentPhase::AwaitingHomeTarget;
            } else {
                state.phase = AssessmentPhase::Completed;
            }
            return;
        }
    }

    // 3. Process mouse clicks for hit registration
    if mouse_button.just_pressed(MouseButton::Left) {
        let view_angles = camera_query
            .iter()
            .next()
            .map(|c| c.angles)
            .unwrap_or_default();

        let mut hit_target_id = None;
        let mut hitzone = HitZone::None;

        for (entity, marker) in &target_query {
            let hit_res = marker.spherical.intersect_ray(view_angles);

            if hit_res.is_hit {
                hit_target_id = Some(marker.target_id);
                hitzone = match hit_res.hitzone {
                    HitZoneType::Head => HitZone::Head,
                    HitZoneType::Body => HitZone::Body,
                    HitZoneType::None => HitZone::None,
                };

                // Despawn hit target
                commands.entity(entity).despawn();

                if marker.is_home_target {
                    // Home target hit: proceed to trial target
                    state.phase = AssessmentPhase::AwaitingTrialTarget;
                } else {
                    // Trial target hit under SingleShotKill
                    state.hits_count += 1;
                    let trial_elapsed = state.trial_start_instant.elapsed().as_secs_f64();
                    println!(
                        "[Trial {}/96] HIT! Time: {:.3}s (Ang Dist: {:.2}°). Total Hits: {}, Misses: {}",
                        state.current_trial_idx + 1,
                        trial_elapsed,
                        hit_res.angular_distance_deg,
                        state.hits_count,
                        state.misses_count
                    );

                    state.current_trial_idx += 1;
                    if state.current_trial_idx < state.trials.len() {
                        state.phase = AssessmentPhase::AwaitingHomeTarget;
                    } else {
                        state.phase = AssessmentPhase::Completed;
                    }
                }
                break;
            }
        }

        // SingleShotKill rule: if shot fired during trial target phase and missed all targets
        if hit_target_id.is_none() && state.phase == AssessmentPhase::AwaitingTrialTarget {
            state.misses_count += 1;
            println!(
                "[Trial {}/96] MISS! Total Hits: {}, Misses: {}",
                state.current_trial_idx + 1,
                state.hits_count,
                state.misses_count
            );

            // Despawn current trial target and transition back to home target
            for (entity, _) in &target_query {
                commands.entity(entity).despawn();
            }

            state.current_trial_idx += 1;
            if state.current_trial_idx < state.trials.len() {
                state.phase = AssessmentPhase::AwaitingHomeTarget;
            } else {
                state.phase = AssessmentPhase::Completed;
            }
        }

        // Record shot event in telemetry
        state
            .recorder
            .record_shot(current_qpc_diff, hit_target_id, hitzone);
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
    let completed_all = state.phase == AssessmentPhase::Completed;

    if elapsed_s >= config.duration_s || completed_all {
        println!("\n=== AimForge Trainer Session Completed ===");
        println!("Elapsed time: {:.2}s", elapsed_s);
        println!(
            "Assessment Status: {}",
            if completed_all {
                "COMPLETED (All 96 trials finished)"
            } else {
                "TIMEOUT / ABORTED"
            }
        );
        println!(
            "Performance: Hits = {}, Misses = {}, Accuracy = {:.1}%",
            state.hits_count,
            state.misses_count,
            if state.hits_count + state.misses_count > 0 {
                (state.hits_count as f64 * 100.0) / (state.hits_count + state.misses_count) as f64
            } else {
                0.0
            }
        );
        println!("Raw Input events captured: {}", state.event_count);
        println!(
            "Shots recorded: {}",
            state.recorder.streams.shots.records.len()
        );
        println!(
            "Targets recorded: {}",
            state.recorder.streams.targets.records.len()
        );
        println!("Quality flags: {:#010b}", state.recorder.quality_flags);

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
            (
                setup_trainer,
                setup_target_assets,
                spawn_fps_camera,
                lock_cursor_startup,
            ),
        )
        .add_systems(
            PreUpdate,
            (
                sync_display_info_system,
                (
                    handle_window_focus,
                    drain_raw_input_system,
                    sync_camera_transform,
                    shooting_and_target_lifecycle_system,
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
