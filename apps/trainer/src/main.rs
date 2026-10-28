//! AimForge native 3D trainer sidecar.
//!
//! Entry point for native Bevy simulation, Fitts target state machine, and Raw Input telemetry recording.

use aim_core::geometry::HitZoneType;
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_scenarios::Scenario;
use aim_telemetry::format::{quality_flags, HitZone};
use bevy::prelude::*;
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::window::{
    CursorGrabMode, CursorOptions, Monitor, MonitorSelection, PresentMode, PrimaryMonitor,
    PrimaryWindow, RawHandleWrapper, VideoModeSelection, WindowCloseRequested, WindowFocused,
    WindowMode,
};
use bevy::winit::{UpdateMode, WinitSettings};
use std::collections::HashMap;
use std::num::NonZero;
use std::path::PathBuf;
use trainer::camera::{spawn_crosshair_ui, spawn_fps_camera, sync_camera_transform, FpsCamera};
use trainer::clock::SessionClock;
use trainer::layout_engine::{
    FittsHitOutcome, FittsMissOutcome, FittsSpawnAction, FittsTimeoutOutcome, LayoutEngineState,
    PeeksHitOutcome, PeeksMissOutcome, PeeksSpawnAction, PeeksTimeoutOutcome,
};
use trainer::pause::{
    despawn_pause_menu, set_cursor_lock, spawn_pause_menu, PauseButtonAction, PauseManager,
    PauseOverlayRoot,
};
use trainer::raw_input::RawInputWorker;
use trainer::recorder::{self, SessionRecorder};
use trainer::target::{
    setup_target_assets, spawn_cluster_target, spawn_home_target, spawn_peek_target,
    spawn_tracking_target, spawn_trial_target, ActiveTargetMarker, TargetAssets,
};
use trainer::ScenarioResource;

#[derive(Resource)]
pub struct TrainerConfig {
    pub scenario: String,
    pub seed: Option<u64>,
    pub form: Option<u32>,
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

/// Seed used to generate this session's trials, derived alongside the scenario
/// before the app runs. `form` is the battery parallel-form index the seed was
/// derived from, or `None` when the seed came straight from `--seed`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ScenarioSeed {
    pub seed: u64,
    pub form: Option<u32>,
}

#[derive(Resource)]
pub struct TrainerState {
    pub clock: SessionClock,
    pub worker: RawInputWorker,
    pub recorder: SessionRecorder,
    pub event_count: u64,
    pub frame_times_us: Vec<u64>,
    pub input_latencies_us: Vec<u64>,
    pub last_frame_t_us: u64,
    pub initial_startup_done: bool,

    // Per-device tracking to resolve multi-device shadowing / double counting
    pub device_counts: HashMap<usize, u64>,
    pub device_intervals: HashMap<usize, Vec<u64>>,
    pub device_last_t: HashMap<usize, u64>,
    pub primary_device: Option<usize>,
    pub non_primary_events: u64,
    pub warmup_events_collected: u64,

    // Scenario definition & layout engine state machine
    pub scenario_def: Scenario,
    pub layout_engine: LayoutEngineState,
    pub next_target_id: u32,
    pub hits_count: u32,
    pub misses_count: u32,
}

fn parse_cli_args() -> TrainerConfig {
    let mut scenario = "assess.v1.flick_static".to_string();
    let mut cli_seed: Option<u64> = None;
    let mut form: Option<u32> = None;
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
                cli_seed = Some(args[i + 1].parse().unwrap_or_else(|e| {
                    eprintln!("ERROR: Invalid --seed '{}': {}", args[i + 1], e);
                    std::process::exit(1);
                }));
                i += 2;
            }
            "--form" if i + 1 < args.len() => {
                let parsed_form: u32 = args[i + 1].parse().unwrap_or_else(|e| {
                    eprintln!("ERROR: Invalid --form '{}': {}", args[i + 1], e);
                    std::process::exit(1);
                });
                if parsed_form > 4 {
                    eprintln!("ERROR: --form must be in range 0..4, got {}", parsed_form);
                    std::process::exit(1);
                }
                form = Some(parsed_form);
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

    if cli_seed.is_some() && form.is_some() {
        eprintln!("ERROR: Arguments '--form' and '--seed' are mutually exclusive. Provide either '--form <0..4>' or '--seed <u64>', not both.");
        std::process::exit(1);
    }

    // Fix 4: Validate mutually incompatible configurations
    if backend == "dx12" && fullscreen_exclusive {
        eprintln!(
            "ERROR: The combination '--backend dx12 --exclusive' is unsupported by wgpu_hal::dx12 on Windows (causes DXGI_ERROR_INVALID_CALL 0x887A0001). Please use Borderless Fullscreen (default) or select '--backend vulkan' for exclusive fullscreen."
        );
        std::process::exit(1);
    }

    TrainerConfig {
        scenario,
        seed: cli_seed,
        form,
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

fn resolve_scenario_file(scenario_name: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(scenario_name),
        PathBuf::from(format!("{}.ron", scenario_name)),
        PathBuf::from(format!("content/scenarios/{}.ron", scenario_name)),
        PathBuf::from(format!("content/scenarios/assess.v1.{}.ron", scenario_name)),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Loads the scenario definition referenced by the CLI args.
/// Validates the scenario and derives the seed.
fn load_scenario(config: &TrainerConfig) -> (Scenario, ScenarioSeed) {
    let sc = match resolve_scenario_file(&config.scenario) {
        Some(path) => {
            let ron_str = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!(
                    "ERROR: Failed reading scenario file '{}': {}",
                    path.display(),
                    e
                );
                std::process::exit(1);
            });
            ron::from_str::<Scenario>(&ron_str).unwrap_or_else(|e| {
                eprintln!(
                    "ERROR: Failed parsing scenario file '{}': {}",
                    path.display(),
                    e
                );
                std::process::exit(1);
            })
        }
        None => {
            if config.scenario == "assess.v1.flick_static" || config.scenario == "flick_static" {
                const FALLBACK_RON: &str =
                    include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
                ron::from_str(FALLBACK_RON).expect("Failed parsing fallback flick_static.ron")
            } else {
                eprintln!(
                    "ERROR: Unknown scenario '{}'. Scenario definition not found in content/scenarios/.",
                    config.scenario
                );
                std::process::exit(1);
            }
        }
    };

    if let Err(err) = sc.validate() {
        eprintln!("ERROR: Scenario '{}' validation failed: {}", sc.id, err);
        std::process::exit(1);
    }

    let scenario_seed = if let Some(form) = config.form {
        ScenarioSeed {
            seed: aim_scenarios::derive_scenario_seed(form as usize, &sc.id),
            form: Some(form),
        }
    } else if let Some(seed) = config.seed {
        ScenarioSeed { seed, form: None }
    } else {
        ScenarioSeed {
            seed: aim_scenarios::derive_scenario_seed(0, &sc.id),
            form: Some(0),
        }
    };

    (sc, scenario_seed)
}

fn setup_trainer(
    mut commands: Commands,
    config: Res<TrainerConfig>,
    scenario_res: Res<ScenarioResource>,
    scenario_seed: Res<ScenarioSeed>,
) {
    let worker = RawInputWorker::spawn();
    let clock = SessionClock::new(worker.qpc_start, worker.qpc_freq);

    let recorder = SessionRecorder::new(recorder::RecorderSetup {
        session_id: uuid::Uuid::new_v4().to_string(),
        scenario_id: config.scenario.clone(),
        form: scenario_seed.form,
        seed: scenario_seed.seed,
        dpi: config.dpi,
        sens: config.sens,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    // The scenario was resolved before the app ran so that this crate's
    // Startup systems (asset setup) can branch on its layout.
    let scenario_def = scenario_res.0.clone();
    let initial_t = clock.now_us();
    let layout_engine = LayoutEngineState::new(&scenario_def, scenario_seed.seed, initial_t);

    commands.insert_resource(PauseManager::new());

    commands.insert_resource(TrainerState {
        clock,
        worker,
        recorder,
        event_count: 0,
        frame_times_us: Vec::with_capacity(20_000),
        input_latencies_us: Vec::with_capacity(50_000),
        last_frame_t_us: initial_t,
        initial_startup_done: false,

        device_counts: HashMap::new(),
        device_intervals: HashMap::new(),
        device_last_t: HashMap::new(),
        primary_device: None,
        non_primary_events: 0,
        warmup_events_collected: 0,

        scenario_def,
        layout_engine,
        next_target_id: 1,
        hits_count: 0,
        misses_count: 0,
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
    pause_manager: Res<PauseManager>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for ev in focus_events.read() {
        if let Some(mut cursor) = q_cursor.iter_mut().next() {
            if ev.focused {
                if !pause_manager.is_paused {
                    cursor.grab_mode = CursorGrabMode::Confined;
                    cursor.visible = false;
                }
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
    pause_manager: Res<PauseManager>,
    assets: Option<Res<TargetAssets>>,
    mut camera_query: Query<&mut FpsCamera>,
    mut target_query: Query<(
        Entity,
        &mut ActiveTargetMarker,
        &mut Transform,
        Option<&mut trainer::layout_engine::PeekTargetMarker>,
    )>,
) {
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * config.sens;
    let now_qpc_us = state.clock.now_us();
    let render_start_t_us = now_qpc_us;
    let now_active_us = pause_manager.active_time_us(now_qpc_us);
    let now_active_s = SessionClock::us_to_s(now_active_us);

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

    if pause_manager.is_paused {
        return;
    }

    if state.primary_device.is_none() {
        state.warmup_events_collected += batch.len() as u64;
        let elapsed_s = SessionClock::us_to_s(state.clock.now_us());
        if state.warmup_events_collected >= 200 || elapsed_s >= 1.0 {
            let best_dev = state
                .device_counts
                .iter()
                .max_by_key(|&(_, count)| *count)
                .map(|(&dev, _)| dev);
            state.primary_device = best_dev;
        }
    }

    let primary_id = state.primary_device;
    let cam_angles_start = camera_query
        .iter()
        .next()
        .map(|c| c.angles)
        .unwrap_or_default();
    let mut current_angles = cam_angles_start;

    let actions =
        trainer::replay::replay_input(&mut current_angles, &batch, primary_id, deg_per_count);

    // Update active tracking target positions incrementally BEFORE evaluating this frame's clicks
    if let Some((tracking_id, new_angles, radius_deg)) =
        state.layout_engine.update_tracking_target(now_active_us)
    {
        let spawn_dist = state.scenario_def.spawn_distance_m as f32;
        let new_pos = trainer::target::angles_to_world_pos(new_angles, spawn_dist);
        for (_, mut marker, mut transform, _) in &mut target_query {
            if marker.target_id == tracking_id {
                marker.spherical = aim_core::geometry::SphericalTarget::new(new_angles, radius_deg);
                transform.translation = new_pos;
            }
        }
    }

    // Fix 1: True per-event input latency metric: render_start_t_us - event.t_us for events applied in this frame
    for action in actions {
        match action {
            trainer::replay::ReplayAction::Motion { t_us, dx, dy } => {
                if t_us < pause_manager.resume_qpc_us {
                    continue;
                }
                state.recorder.record_input(t_us, dx, dy);
                if render_start_t_us >= t_us {
                    state.input_latencies_us.push(render_start_t_us - t_us);
                }
            }
            trainer::replay::ReplayAction::Click {
                t_us,
                angles_at_click,
            } => {
                if t_us < pause_manager.resume_qpc_us {
                    continue;
                }
                if render_start_t_us >= t_us {
                    state.input_latencies_us.push(render_start_t_us - t_us);
                }
                let mut hit_target_id = None;
                let mut hitzone = HitZone::None;

                for (entity, marker, _, peek_opt) in &target_query {
                    // Respect wall occlusion for peek targets
                    if let Some(peek) = peek_opt {
                        let half_wall_ang =
                            ((0.5 / peek.spawn_distance_m as f64).asin()).to_degrees();
                        let is_occluded = if peek.occluder_yaw > 0.0 {
                            angles_at_click.yaw >= peek.occluder_yaw - half_wall_ang
                        } else {
                            angles_at_click.yaw <= peek.occluder_yaw + half_wall_ang
                        };
                        if is_occluded {
                            continue;
                        }
                    }

                    let hit_res = marker.spherical.intersect_ray(angles_at_click);

                    if hit_res.is_hit {
                        hit_target_id = Some(marker.target_id);
                        hitzone = match hit_res.hitzone {
                            HitZoneType::Head => HitZone::Head,
                            HitZoneType::Body => HitZone::Body,
                            HitZoneType::None => HitZone::None,
                        };

                        commands.entity(entity).despawn();

                        let total_trials = state.layout_engine.total_trials_or_targets();
                        let active_click_us = pause_manager.active_time_us(t_us);
                        if let Some(fitts_outcome) = state
                            .layout_engine
                            .handle_fitts_hit(marker.is_home_target, active_click_us)
                        {
                            match fitts_outcome {
                                FittsHitOutcome::HomeHit => {}
                                FittsHitOutcome::TrialHit {
                                    trial_idx,
                                    trial_elapsed_s,
                                    completed: _,
                                } => {
                                    state.hits_count += 1;
                                    println!(
                                        "[Trial {}/{}] HIT! Time: {:.3}s (Dist: {:.2}°). Hits: {}, Misses: {}",
                                        trial_idx + 1,
                                        total_trials,
                                        trial_elapsed_s,
                                        hit_res.angular_distance_deg,
                                        state.hits_count,
                                        state.misses_count
                                    );
                                }
                            }
                        } else if let Some(peek_outcome) = state
                            .layout_engine
                            .handle_peeks_hit(marker.is_home_target, active_click_us)
                        {
                            match peek_outcome {
                                PeeksHitOutcome::NeutralHit => {}
                                PeeksHitOutcome::PeekHit {
                                    trial_idx,
                                    trial_elapsed_s,
                                    completed: _,
                                } => {
                                    state.hits_count += 1;
                                    println!(
                                        "[Trial {}/{}] PEEK HIT! Time: {:.3}s. Hits: {}, Misses: {}",
                                        trial_idx + 1,
                                        total_trials,
                                        trial_elapsed_s,
                                        state.hits_count,
                                        state.misses_count
                                    );
                                }
                            }
                        } else if let Some(cluster_outcome) =
                            state.layout_engine.handle_cluster_hit(marker.target_id)
                        {
                            state.hits_count += 1;
                            println!(
                                "[Target {}/{}] CLUSTER HIT! Total Hits: {}, Misses: {}",
                                cluster_outcome.target_id,
                                total_trials,
                                state.hits_count,
                                state.misses_count
                            );
                        }
                        break;
                    }
                }

                if hit_target_id.is_none() {
                    let total_trials = state.layout_engine.total_trials_or_targets();
                    if let FittsMissOutcome::TrialMiss {
                        trial_idx,
                        completed: _,
                    } = state.layout_engine.handle_fitts_miss()
                    {
                        state.misses_count += 1;
                        println!(
                            "[Trial {}/{}] MISS! Total Hits: {}, Misses: {}",
                            trial_idx + 1,
                            total_trials,
                            state.hits_count,
                            state.misses_count
                        );

                        for (entity, _, _, _) in &target_query {
                            commands.entity(entity).despawn();
                        }
                    } else if let PeeksMissOutcome::PeekMiss {
                        trial_idx,
                        completed: _,
                    } = state.layout_engine.handle_peeks_miss()
                    {
                        state.misses_count += 1;
                        println!(
                            "[Trial {}/{}] PEEK MISS! Total Hits: {}, Misses: {}",
                            trial_idx + 1,
                            total_trials,
                            state.hits_count,
                            state.misses_count
                        );

                        for (entity, _, _, _) in &target_query {
                            commands.entity(entity).despawn();
                        }
                    }
                }

                state.recorder.record_shot(t_us, hit_target_id, hitzone);
            }
        }
    }

    if let Some(mut fps_cam) = camera_query.iter_mut().next() {
        fps_cam.angles = current_angles;
    }

    // Record the view stream every frame (camera yaw/pitch at render_start_t_us)
    state
        .recorder
        .record_view(render_start_t_us, current_angles.yaw, current_angles.pitch);

    let assets = match assets {
        Some(a) => a,
        None => return,
    };

    // Update active tracking target positions incrementally
    if let Some((tracking_id, new_angles, radius_deg)) =
        state.layout_engine.update_tracking_target(now_active_us)
    {
        let spawn_dist = state.scenario_def.spawn_distance_m as f32;
        let new_pos = trainer::target::angles_to_world_pos(new_angles, spawn_dist);
        for (_, mut marker, mut transform, _) in &mut target_query {
            if marker.target_id == tracking_id {
                marker.spherical = aim_core::geometry::SphericalTarget::new(new_angles, radius_deg);
                transform.translation = new_pos;
            }
        }
    }

    // Update active peek target emergence positions
    for (_, mut marker, mut transform, peek_opt) in &mut target_query {
        if let Some(peek) = peek_opt {
            let elapsed_s =
                (now_active_us.saturating_sub(peek.emergence_t_us) as f64) / 1_000_000.0;
            let peek_speed_deg_s =
                (peek.peek_speed_mps / peek.spawn_distance_m as f64).to_degrees();
            let inward_sign = if peek.occluder_yaw > 0.0 { -1.0 } else { 1.0 };
            let current_yaw = peek.target_yaw + inward_sign * elapsed_s * peek_speed_deg_s;
            let current_angles = aim_core::camera::ViewAngles::new(current_yaw, peek.pitch);
            marker.spherical = aim_core::geometry::SphericalTarget::new(
                current_angles,
                marker.spherical.radius_deg,
            )
            .with_head_zone(false);
            transform.translation =
                trainer::target::angles_to_world_pos(current_angles, peek.spawn_distance_m);
        }
    }

    if !state.layout_engine.is_completed() {
        let spawn_dist = state.scenario_def.spawn_distance_m as f32;
        let has_no_targets = target_query.iter().next().is_none();

        if has_no_targets {
            let mut next_tid = state.next_target_id;
            let maybe_fitts =
                state
                    .layout_engine
                    .handle_fitts_spawn(&mut next_tid, now_active_us, spawn_dist);
            let maybe_peeks = if maybe_fitts.is_none() {
                state
                    .layout_engine
                    .handle_peeks_spawn(&mut next_tid, now_active_us, spawn_dist)
            } else {
                None
            };
            let maybe_tracking = if maybe_fitts.is_none() && maybe_peeks.is_none() {
                state
                    .layout_engine
                    .handle_tracking_spawn(&mut next_tid, now_active_us, spawn_dist)
            } else {
                None
            };
            state.next_target_id = next_tid;

            if let Some(action) = maybe_fitts {
                match action {
                    FittsSpawnAction::SpawnHome {
                        target_id,
                        distance_m,
                    } => {
                        spawn_home_target(
                            &mut commands,
                            &assets,
                            distance_m,
                            target_id,
                            now_active_s,
                        );
                        state
                            .recorder
                            .record_target_spawn(target_id, now_qpc_us, 0, 0.5, 0.0, 0.0);
                    }
                    FittsSpawnAction::SpawnTrial {
                        target_id,
                        trial,
                        distance_m,
                    } => {
                        spawn_trial_target(
                            &mut commands,
                            &assets,
                            &trial,
                            distance_m,
                            target_id,
                            now_active_s,
                        );
                        state.recorder.record_target_spawn(
                            target_id,
                            now_qpc_us,
                            0,
                            trial.radius_deg,
                            trial.yaw_deg,
                            trial.pitch_deg,
                        );
                    }
                }
            } else if let Some(action) = maybe_peeks {
                match action {
                    PeeksSpawnAction::SpawnNeutralTarget {
                        target_id,
                        distance_m,
                    } => {
                        spawn_home_target(
                            &mut commands,
                            &assets,
                            distance_m,
                            target_id,
                            now_active_s,
                        );
                        state
                            .recorder
                            .record_target_spawn(target_id, now_qpc_us, 0, 0.5, 0.0, 0.0);
                    }
                    PeeksSpawnAction::SpawnPeekTarget {
                        target_id,
                        trial,
                        distance_m,
                    } => {
                        spawn_peek_target(
                            &mut commands,
                            &assets,
                            &trial,
                            distance_m,
                            target_id,
                            now_active_s,
                            now_active_us,
                        );
                        let spawn_pitch = trainer::replay::crosshair_pitch_at_qpc_instant(
                            &cam_angles_start,
                            &batch,
                            now_qpc_us,
                            primary_id,
                            deg_per_count,
                        );
                        let spawn_angles = trainer::replay::angles_at_timestamp(
                            &cam_angles_start,
                            &batch,
                            now_qpc_us,
                            primary_id,
                            deg_per_count,
                        );
                        state
                            .recorder
                            .record_view(now_qpc_us, spawn_angles.yaw, spawn_pitch);
                        state.recorder.record_peek_target_spawn(
                            target_id,
                            now_qpc_us,
                            0,
                            trial.target_radius_deg,
                            trial.occluder_yaw_deg,
                            trial.noisy_pitch_deg,
                            trial.head_pitch_deg,
                        );
                    }
                }
            } else if let Some(action) = maybe_tracking {
                spawn_tracking_target(
                    &mut commands,
                    &assets,
                    action.initial_angles,
                    action.radius_deg,
                    action.distance_m,
                    action.target_id,
                    now_active_s,
                );
                state.recorder.record_tracking_target_spawn(
                    action.target_id,
                    now_qpc_us,
                    action.despawn_t_us,
                    action.radius_deg,
                    action.initial_angles.yaw,
                    action.initial_angles.pitch,
                    action.spline_ron,
                );
            }
        }

        // Replenish concurrent cluster targets up to concurrent_count (3)
        while let Some(cluster_action) = {
            let mut next_tid = state.next_target_id;
            let act = state
                .layout_engine
                .handle_cluster_spawn(&mut next_tid, spawn_dist);
            state.next_target_id = next_tid;
            act
        } {
            spawn_cluster_target(
                &mut commands,
                &assets,
                cluster_action.angles,
                cluster_action.radius_deg,
                cluster_action.distance_m,
                cluster_action.target_id,
                now_active_s,
            );
            state.recorder.record_target_spawn(
                cluster_action.target_id,
                now_qpc_us,
                0,
                cluster_action.radius_deg,
                cluster_action.angles.yaw,
                cluster_action.angles.pitch,
            );
        }
    }

    let total_trials = state.layout_engine.total_trials_or_targets();
    if let FittsTimeoutOutcome::TimedOut {
        trial_idx,
        trial_elapsed_s,
        completed: _,
    } = state.layout_engine.handle_fitts_timeout(now_active_us)
    {
        state.misses_count += 1;
        println!(
            "[Trial {}/{}] TIMEOUT ({:.2}s). Total Hits: {}, Misses: {}",
            trial_idx + 1,
            total_trials,
            trial_elapsed_s,
            state.hits_count,
            state.misses_count
        );

        for (entity, _, _, _) in &target_query {
            commands.entity(entity).despawn();
        }
    } else if let PeeksTimeoutOutcome::TimedOut {
        trial_idx,
        trial_elapsed_s,
        completed: _,
    } = state.layout_engine.handle_peeks_timeout(now_active_us)
    {
        state.misses_count += 1;
        println!(
            "[Trial {}/{}] PEEK TIMEOUT ({:.2}s). Total Hits: {}, Misses: {}",
            trial_idx + 1,
            total_trials,
            trial_elapsed_s,
            state.hits_count,
            state.misses_count
        );

        for (entity, _, _, _) in &target_query {
            commands.entity(entity).despawn();
        }
    }
}

fn track_frame_time_system(mut state: ResMut<TrainerState>) {
    let now_us = state.clock.now_us();
    let frame_us = now_us.saturating_sub(state.last_frame_t_us);
    state.last_frame_t_us = now_us;
    state.frame_times_us.push(frame_us);
    state.recorder.record_frame(now_us, now_us);
}

fn session_lifecycle_system(
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
    pause_manager: Res<PauseManager>,
    mut exit: MessageWriter<AppExit>,
) {
    let now_qpc_us = state.clock.now_us();
    let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_qpc_us));
    let completed_all = state.layout_engine.is_completed();

    if elapsed_s >= config.duration_s || completed_all {
        let total_count = state.layout_engine.total_trials_or_targets();
        println!("\n=== AimForge Trainer Session Completed ===");
        println!("Elapsed time: {:.2}s", elapsed_s);
        println!(
            "Assessment Status: {}",
            if completed_all {
                format!("COMPLETED (All {} trials finished)", total_count)
            } else {
                "TIMEOUT / ABORTED".to_string()
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
                println!(
                    "NOTE: Frame time p50 ~6.06ms indicates an external display/driver cap (e.g. NVIDIA Control Panel Vertical Sync, G-Sync, or DWM scanout lock at 165Hz)."
                );
            }
        }

        // True per-event input latency (render start - event capture time)
        if !state.input_latencies_us.is_empty() {
            let mut lats = state.input_latencies_us.clone();
            lats.sort_unstable();
            let l_len = lats.len();
            let l_p50 = lats[l_len * 50 / 100];
            let l_p95 = lats[l_len * 95 / 100];
            let l_p99 = lats[l_len * 99 / 100];
            let l_max = lats[l_len - 1];
            println!("\n--- Per-Event Input Latency (Render Start - Event Timestamp) ---");
            println!(
                "Latency (us): p50 = {}us ({:.2}ms), p95 = {}us ({:.2}ms), p99 = {}us ({:.2}ms), max = {}us ({:.2}ms) (sample count: {})",
                l_p50,
                l_p50 as f64 / 1000.0,
                l_p95,
                l_p95 as f64 / 1000.0,
                l_p99,
                l_p99 as f64 / 1000.0,
                l_max,
                l_max as f64 / 1000.0,
                l_len
            );
        }

        save_session_telemetry(&mut state, &config, !completed_all);

        exit.write(AppExit::Success);
    }
}

fn save_session_telemetry(state: &mut TrainerState, config: &TrainerConfig, is_aborted: bool) {
    if is_aborted {
        state.recorder.quality_flags |= quality_flags::ABORTED;
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
}

fn window_close_system(
    mut close_events: MessageReader<WindowCloseRequested>,
    mut state: ResMut<TrainerState>,
    config: Res<TrainerConfig>,
    mut exit: MessageWriter<AppExit>,
) {
    for _ in close_events.read() {
        println!("\nWindow close requested. Saving partial session...");
        save_session_telemetry(&mut state, &config, true);
        exit.write(AppExit::Success);
    }
}

fn pause_menu_toggle_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pause_manager: ResMut<PauseManager>,
    mut state: ResMut<TrainerState>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: Query<Entity, With<PauseOverlayRoot>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        let now_us = state.clock.now_us();
        pause_manager.toggle_pause(now_us);
        if pause_manager.is_paused {
            state.recorder.flag_paused();
            state
                .recorder
                .record_key(now_us, aim_telemetry::format::KeyCode::Unknown(0x1B), true);
            set_cursor_lock(&mut q_cursor, false);
            spawn_pause_menu(&mut commands, &ui_fonts);
        } else {
            state
                .recorder
                .record_key(now_us, aim_telemetry::format::KeyCode::Unknown(0x1B), false);
            let resume_active_us = pause_manager.active_time_us(now_us);
            state
                .layout_engine
                .void_in_flight_trial_on_resume(resume_active_us);
            set_cursor_lock(&mut q_cursor, true);
            despawn_pause_menu(&mut commands, &q_overlay);
        }
    }
}

#[allow(deprecated)]
#[allow(clippy::too_many_arguments)]
fn pause_menu_button_system(
    mut commands: Commands,
    mut pause_manager: ResMut<PauseManager>,
    mut state: ResMut<TrainerState>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: Query<Entity, With<PauseOverlayRoot>>,
    mut interaction_query: Query<
        (&Interaction, &PauseButtonAction, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut exit: MessageWriter<AppExit>,
    config: Res<TrainerConfig>,
) {
    for (interaction, action, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => match action {
                PauseButtonAction::Resume => {
                    let now_us = state.clock.now_us();
                    pause_manager.resume(now_us);
                    state.recorder.record_key(
                        now_us,
                        aim_telemetry::format::KeyCode::Unknown(0x1B),
                        false,
                    );
                    let resume_active_us = pause_manager.active_time_us(now_us);
                    state
                        .layout_engine
                        .void_in_flight_trial_on_resume(resume_active_us);
                    set_cursor_lock(&mut q_cursor, true);
                    despawn_pause_menu(&mut commands, &q_overlay);
                }
                PauseButtonAction::QuitAndSave => {
                    println!("\nQuit & Save requested. Saving partial session...");
                    save_session_telemetry(&mut state, &config, true);
                    exit.write(AppExit::Success);
                }
                PauseButtonAction::QuitWithoutSaving => {
                    println!("\nSession aborted by user without saving.");
                    exit.write(AppExit::Success);
                }
            },
            Interaction::Hovered => {
                *bg_color = BackgroundColor(Color::srgb(0.35, 0.35, 0.35));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
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
    let (scenario_def, scenario_seed) = load_scenario(&config);

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
        .insert_resource(ScenarioResource(scenario_def))
        .insert_resource(scenario_seed)
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
                trainer::fonts::setup_ui_fonts,
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
            Update,
            (
                window_close_system,
                pause_menu_toggle_system,
                pause_menu_button_system,
            ),
        )
        .add_systems(
            PostUpdate,
            (track_frame_time_system, session_lifecycle_system),
        )
        .run();
}
