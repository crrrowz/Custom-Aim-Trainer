//! AimForge native 3D trainer sidecar.
//!
//! Entry point for native Bevy simulation, Fitts target state machine, and Raw Input telemetry recording.

use aim_core::geometry::HitZoneType;
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_scenarios::{generate_trials, Scenario, Trial};
use aim_telemetry::format::{quality_flags, HitZone};
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::window::{
    CursorGrabMode, CursorOptions, Monitor, MonitorSelection, PresentMode, PrimaryMonitor,
    PrimaryWindow, RawHandleWrapper, VideoModeSelection, WindowFocused, WindowMode,
};
use bevy::winit::{UpdateMode, WinitSettings};
use std::collections::HashMap;
use std::num::NonZero;
use std::path::PathBuf;
use std::time::Instant;
use trainer::camera::{spawn_crosshair_ui, spawn_fps_camera, sync_camera_transform, FpsCamera};
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
    pub windowed: bool,
    pub fullscreen_exclusive: bool,
    pub backend: String,
    pub present_mode_str: String,
    pub max_frame_latency: u32,
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
    pub frame_times_us: Vec<u64>,
    pub input_ages_us: Vec<u64>,
    pub last_frame_instant: Instant,
    pub initial_startup_done: bool,

    // Per-device tracking to resolve multi-device shadowing / double counting
    pub device_counts: HashMap<usize, u64>,
    pub device_intervals: HashMap<usize, Vec<u64>>,
    pub device_last_t: HashMap<usize, u64>,
    pub primary_device: Option<usize>,
    pub non_primary_events: u64,
    pub warmup_events_collected: u64,
    pub latest_primary_input_t_us: Option<u64>,

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
    let mut duration_s = 288.0f64;
    let mut windowed = false;
    let mut fullscreen_exclusive = false;
    let mut backend = "auto".to_string();
    let mut present_mode_str = "auto-novsync".to_string();
    let mut max_frame_latency = 1u32;
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
            "--backend" if i + 1 < args.len() => {
                backend = args[i + 1].to_lowercase();
                i += 2;
            }
            "--present-mode" if i + 1 < args.len() => {
                present_mode_str = args[i + 1].to_lowercase();
                i += 2;
            }
            "--max-frame-latency" if i + 1 < args.len() => {
                max_frame_latency = args[i + 1].parse().unwrap_or(1).clamp(1, 3);
                i += 2;
            }
            "--windowed" => {
                windowed = true;
                i += 1;
            }
            "--exclusive" => {
                fullscreen_exclusive = true;
                i += 1;
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
        windowed,
        fullscreen_exclusive,
        backend,
        present_mode_str,
        max_frame_latency,
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

    let ron_str = include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
    let scenario_def: Scenario =
        ron::from_str(ron_str).expect("Failed parsing assess.v1.flick_static.ron");
    let trials = generate_trials(&scenario_def, config.seed);

    commands.insert_resource(TrainerState {
        start_instant: Instant::now(),
        worker,
        recorder,
        event_count: 0,
        frame_times_us: Vec::with_capacity(20_000),
        input_ages_us: Vec::with_capacity(20_000),
        last_frame_instant: Instant::now(),
        initial_startup_done: false,

        device_counts: HashMap::new(),
        device_intervals: HashMap::new(),
        device_last_t: HashMap::new(),
        primary_device: None,
        non_primary_events: 0,
        warmup_events_collected: 0,
        latest_primary_input_t_us: None,

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

fn drain_and_evaluate_input_system(
    mut commands: Commands,
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
    assets: Option<Res<TargetAssets>>,
    mut camera_query: Query<&mut FpsCamera>,
    target_query: Query<(Entity, &ActiveTargetMarker)>,
) {
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * config.sens;

    let mut batch = Vec::new();
    while let Ok(event) = state.worker.receiver.try_recv() {
        state.event_count += 1;
        *state.device_counts.entry(event.device_id).or_insert(0) += 1;

        if let Some(&last_t) = state.device_last_t.get(&event.device_id) {
            if event.t_us >= last_t {
                state
                    .device_intervals
                    .entry(event.device_id)
                    .or_default()
                    .push(event.t_us - last_t);
            }
        }
        state.device_last_t.insert(event.device_id, event.t_us);
        batch.push(event);
    }

    if state.primary_device.is_none() {
        state.warmup_events_collected += batch.len() as u64;
        if state.warmup_events_collected >= 200
            || state.start_instant.elapsed().as_secs_f64() >= 1.0
        {
            let best_dev = state
                .device_counts
                .iter()
                .max_by_key(|&(_, count)| *count)
                .map(|(&dev, _)| dev);
            state.primary_device = best_dev;
        }
    }

    let primary_id = state.primary_device;
    let mut current_angles = camera_query
        .iter()
        .next()
        .map(|c| c.angles)
        .unwrap_or_default();

    let actions =
        trainer::replay::replay_input(&mut current_angles, &batch, primary_id, deg_per_count);

    for action in actions {
        match action {
            trainer::replay::ReplayAction::Motion { t_us, dx, dy } => {
                state.recorder.record_input(t_us, dx, dy);
                state.latest_primary_input_t_us = Some(t_us);
            }
            trainer::replay::ReplayAction::Click {
                t_us,
                angles_at_click,
            } => {
                state.latest_primary_input_t_us = Some(t_us);
                let mut hit_target_id = None;
                let mut hitzone = HitZone::None;

                for (entity, marker) in &target_query {
                    let hit_res = marker.spherical.intersect_ray(angles_at_click);

                    if hit_res.is_hit {
                        hit_target_id = Some(marker.target_id);
                        hitzone = match hit_res.hitzone {
                            HitZoneType::Head => HitZone::Head,
                            HitZoneType::Body => HitZone::Body,
                            HitZoneType::None => HitZone::None,
                        };

                        commands.entity(entity).despawn();

                        if marker.is_home_target {
                            state.phase = AssessmentPhase::AwaitingTrialTarget;
                        } else {
                            state.hits_count += 1;
                            let trial_elapsed = state.trial_start_instant.elapsed().as_secs_f64();
                            println!(
                                "[Trial {}/96] HIT! Time: {:.3}s (Dist: {:.2}°). Hits: {}, Misses: {}",
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

                if hit_target_id.is_none() && state.phase == AssessmentPhase::AwaitingTrialTarget {
                    state.misses_count += 1;
                    println!(
                        "[Trial {}/96] MISS! Total Hits: {}, Misses: {}",
                        state.current_trial_idx + 1,
                        state.hits_count,
                        state.misses_count
                    );

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

                state.recorder.record_shot(t_us, hit_target_id, hitzone);
            }
        }
    }

    if let Some(mut fps_cam) = camera_query.iter_mut().next() {
        fps_cam.angles = current_angles;
    }

    let assets = match assets {
        Some(a) => a,
        None => return,
    };

    let now_s = state.start_instant.elapsed().as_secs_f64();
    let current_qpc_t = state.start_instant.elapsed().as_micros() as u64;

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
                    .record_target_spawn(tid, current_qpc_t, 0, 0.5, 0.0, 0.0);
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
                        current_qpc_t,
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

    if state.phase == AssessmentPhase::AwaitingTrialTarget {
        let trial_elapsed_s = state.trial_start_instant.elapsed().as_secs_f64();
        if trial_elapsed_s >= state.scenario_def.trial_timeout_s {
            state.misses_count += 1;
            println!(
                "[Trial {}/96] TIMEOUT ({:.2}s). Total Hits: {}, Misses: {}",
                state.current_trial_idx + 1,
                trial_elapsed_s,
                state.hits_count,
                state.misses_count
            );

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
    }
}

/// Item 3: Frame time and Input Age tracking system.
///
/// Input age = (current render-start QPC t_us) - (t_us of newest applied raw input event).
fn track_frame_and_input_age_system(mut state: ResMut<TrainerState>) {
    let now = Instant::now();
    let frame_us = now.duration_since(state.last_frame_instant).as_micros() as u64;
    state.last_frame_instant = now;
    state.frame_times_us.push(frame_us);

    let render_start_qpc_t = state.start_instant.elapsed().as_micros() as u64;
    if let Some(newest_input_t) = state.latest_primary_input_t_us {
        if render_start_qpc_t >= newest_input_t {
            let input_age = render_start_qpc_t - newest_input_t;
            state.input_ages_us.push(input_age);
        }
    }
}

fn session_lifecycle_system(
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
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
        println!("Raw Input events total: {}", state.event_count);
        println!(
            "Absolute mouse events discarded: {}",
            RawInputWorker::absolute_events_discarded()
        );
        println!(
            "Non-primary device events ignored: {}",
            state.non_primary_events
        );
        println!(
            "Display configured: {}x{} @ {}Hz",
            state.recorder.setup.width, state.recorder.setup.height, state.recorder.setup.hz
        );

        let mut rate_exceeded = false;
        println!("\n--- Raw Input Device Breakdown ---");
        for (&dev_id, &count) in &state.device_counts {
            let dev_name = RawInputWorker::get_device_name(dev_id);
            let is_primary = Some(dev_id) == state.primary_device;
            println!(
                "Device {:#x} [{}]: {} events (Primary: {})",
                dev_id, dev_name, count, is_primary
            );

            if let Some(intervals) = state.device_intervals.get(&dev_id) {
                let active: Vec<u64> = intervals
                    .iter()
                    .copied()
                    .filter(|&dt| dt <= 50_000)
                    .collect();
                if !active.is_empty() {
                    let mut sorted = active.clone();
                    sorted.sort_unstable();
                    let c = sorted.len();
                    let p50 = sorted[c * 50 / 100];
                    let p95 = sorted[c * 95 / 100];
                    let p99 = sorted[c * 99 / 100];
                    let max = sorted[c - 1];
                    let u500 = sorted.iter().filter(|&&dt| dt < 500).count();
                    let o2000 = sorted.iter().filter(|&&dt| dt > 2000).count();
                    let act_s = sorted.iter().sum::<u64>() as f64 / 1_000_000.0;
                    let rate = if act_s > 0.0 { c as f64 / act_s } else { 0.0 };

                    println!(
                        "  -> Active Rate: {:.1} ev/s | p50={}us, p95={}us, p99={}us, max={}us",
                        rate, p50, p95, p99, max
                    );
                    println!(
                        "  -> Intervals <500us: {} ({:.1}%), >2000us: {} ({:.1}%)",
                        u500,
                        u500 as f64 * 100.0 / c as f64,
                        o2000,
                        o2000 as f64 * 100.0 / c as f64
                    );

                    // Polling rate anomaly check: rate > 1.1 * 1000Hz
                    if is_primary && rate > 1100.0 {
                        rate_exceeded = true;
                    }
                }
            }
        }

        if rate_exceeded {
            state.recorder.quality_flags |= quality_flags::POLLING_RATE_ANOMALY;
        }

        println!("\nQuality flags: {:#010b}", state.recorder.quality_flags);

        if state.frame_times_us.len() > 10 {
            let mut frames = state.frame_times_us[10..].to_vec();
            frames.sort_unstable();
            let f_len = frames.len();
            let f_p50 = frames[f_len * 50 / 100] as f64 / 1000.0;
            let f_p99 = frames[f_len * 99 / 100] as f64 / 1000.0;
            println!("\n--- Frame Time Metrics ---");
            println!(
                "Frame times (ms): p50 = {:.2}ms, p99 = {:.2}ms",
                f_p50, f_p99
            );
            if (f_p50 - 6.06).abs() < 0.2 {
                println!("NOTE: Frame time p50 ~6.06ms indicates an external display/driver cap (e.g. NVIDIA Control Panel Vertical Sync, G-Sync, or DWM scanout lock at 165Hz).");
            }
        }

        if !state.input_ages_us.is_empty() {
            let mut ages = state.input_ages_us.clone();
            ages.sort_unstable();
            let a_len = ages.len();
            let a_p50 = ages[a_len * 50 / 100];
            let a_p99 = ages[a_len * 99 / 100];
            println!("\n--- Input Age Metrics (Render Start - Newest Input) ---");
            println!(
                "Input age (us): p50 = {}us ({:.2}ms), p99 = {}us ({:.2}ms) (sample count: {})",
                a_p50,
                a_p50 as f64 / 1000.0,
                a_p99,
                a_p99 as f64 / 1000.0,
                a_len
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

    let window_mode = if config.windowed {
        WindowMode::Windowed
    } else if config.fullscreen_exclusive {
        WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
    } else {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    };

    let backends = match config.backend.as_str() {
        "dx12" => Some(Backends::DX12),
        "vulkan" => Some(Backends::VULKAN),
        _ => Some(Backends::PRIMARY),
    };

    let (present_mode, requested_desc) = match config.present_mode_str.as_str() {
        "immediate" => (PresentMode::Immediate, "Immediate (Uncapped / Tearing)"),
        "mailbox" => (PresentMode::Mailbox, "Mailbox (Fast VSync / Non-blocking)"),
        "fifo" => (PresentMode::Fifo, "Fifo (Standard VSync)"),
        _ => (
            PresentMode::AutoNoVsync,
            "AutoNoVsync (Immediate -> Mailbox -> Fifo)",
        ),
    };

    let max_latency = NonZero::new(config.max_frame_latency);

    println!(
        "[AimForge] Graphics Init: backend='{}' ({:?}), window_mode={:?}, requested present_mode={}, max_frame_latency={:?}",
        config.backend, backends, window_mode, requested_desc, config.max_frame_latency
    );

    App::new()
        .insert_resource(config)
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "AimForge Trainer".into(),
                        mode: window_mode,
                        present_mode,
                        desired_maximum_frame_latency: max_latency,
                        ..default()
                    }),
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        backends,
                        power_preference: bevy::render::settings::PowerPreference::HighPerformance,
                        ..default()
                    })),
                    ..default()
                }),
        )
        .add_systems(
            Startup,
            (
                setup_trainer,
                setup_target_assets,
                spawn_fps_camera,
                spawn_crosshair_ui,
                lock_cursor_startup,
            ),
        )
        .add_systems(
            PreUpdate,
            (
                sync_display_info_system,
                (
                    handle_window_focus,
                    drain_and_evaluate_input_system,
                    sync_camera_transform,
                )
                    .chain(),
            ),
        )
        .add_systems(
            PostUpdate,
            (track_frame_and_input_age_system, session_lifecycle_system),
        )
        .run();
}
