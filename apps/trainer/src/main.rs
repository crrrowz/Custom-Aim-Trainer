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
    apply_focus_change, despawn_pause_menu, execute_pause, execute_resume, show_pause_menu,
    spawn_pause_menu, FocusOutcome, PauseButtonAction, PauseCause, PauseManager,
    PauseMenuNavigation, PauseOverlayRoot,
};
use trainer::raw_input::RawInputWorker;
use trainer::recorder::{self, SessionRecorder};
use trainer::session_flow::{
    compute_session_results, despawn_flow_overlay, resolve_battery_stages, spawn_countdown_overlay,
    spawn_instructions_overlay, spawn_results_overlay, BatterySequencer, FlowOverlayRoot,
    FlowState, SessionFlowEngine, SessionResultsInput,
};
use trainer::target::{
    setup_target_assets, spawn_cluster_target, spawn_home_target, spawn_peek_target,
    spawn_tracking_target, spawn_trial_target, ActiveTargetMarker, TargetAssets,
};
use trainer::ui_tokens::{COLOR_ACCENT, COLOR_DANGER, COLOR_TEXT_PRIMARY};
use trainer::ScenarioResource;

#[derive(Resource)]
pub struct TrainerConfig {
    pub scenario: String,
    pub battery: Option<String>,
    pub seed: Option<u64>,
    pub form: Option<u32>,
    pub dpi: u32,
    pub sens: f64,
    pub duration_s: f64,
    /// `--duration`, applied to every stage of a battery.
    pub duration_override: Option<f64>,
    /// Seconds of rest between the tests of a battery.
    pub rest_s: f32,
    pub windowed: bool,
    pub fullscreen_exclusive: bool,
    pub backend: String,
    pub present_mode_str: String,
    pub max_frame_latency: u32,
    pub output_path: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
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
    pub reaction_times_s: Vec<f64>,
    pub session_saved: bool,
    pub drill: trainer::drill_rules::DrillRuntime,
    pub rest_left_s: Option<f32>,
    pub flow_engine: SessionFlowEngine,
    pub battery_sequencer: Option<BatterySequencer>,
}

fn parse_cli_args() -> (TrainerConfig, Option<f64>) {
    let mut scenario = "assess.v1.flick_static".to_string();
    let mut battery: Option<String> = None;
    let mut cli_seed: Option<u64> = None;
    let mut form: Option<u32> = None;
    let mut dpi = 800u32;
    let mut sens = 0.35f64;
    let mut duration_override: Option<f64> = None;
    let mut windowed = false;
    let mut fullscreen_exclusive = false;
    let mut backend = "auto".to_string();
    let mut present_mode_str = "auto-novsync".to_string();
    let mut max_frame_latency = 1u32;
    let mut output_path = None;
    let mut output_dir = None;
    let mut rest_s = DEFAULT_REST_S;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--rest" if i + 1 < args.len() => {
                rest_s = args[i + 1].parse().unwrap_or(DEFAULT_REST_S);
                i += 2;
            }
            "--locale" if i + 1 < args.len() => {
                trainer::ui_tokens::strings::set_arabic(args[i + 1] == "ar");
                i += 2;
            }
            "--scenario" if i + 1 < args.len() => {
                scenario = args[i + 1].clone();
                i += 2;
            }
            "--battery" if i + 1 < args.len() => {
                battery = Some(args[i + 1].clone());
                i += 2;
            }
            "--output-dir" if i + 1 < args.len() => {
                output_dir = Some(PathBuf::from(&args[i + 1]));
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
                duration_override = Some(args[i + 1].parse().unwrap_or_else(|e| {
                    eprintln!("ERROR: Invalid --duration '{}': {}", args[i + 1], e);
                    std::process::exit(1);
                }));
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

    (
        TrainerConfig {
            scenario,
            battery,
            seed: cli_seed,
            form,
            dpi,
            sens,
            duration_s: 288.0,
            duration_override: None,
            rest_s,
            windowed,
            fullscreen_exclusive,
            backend,
            present_mode_str,
            max_frame_latency,
            output_path,
            output_dir,
        },
        duration_override,
    )
}

/// Loads the scenario definition referenced by the CLI args.
/// Validates the scenario and derives the seed.
fn load_scenario(
    scenario_id: &str,
    form: Option<u32>,
    seed: Option<u64>,
) -> (Scenario, ScenarioSeed) {
    let sc = trainer::load_scenario_by_id(scenario_id).unwrap_or_else(|e| {
        eprintln!("ERROR: {}", e);
        std::process::exit(1);
    });

    let scenario_seed = if let Some(form) = form {
        ScenarioSeed {
            seed: aim_scenarios::derive_scenario_seed(form as usize, &sc.id),
            form: Some(form),
        }
    } else if let Some(seed) = seed {
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
    commands.insert_resource(PauseMenuNavigation {
        live: Default::default(),
        focused_index: 0,
        click_guard_s: 0.0,
        is_confirming_discard: false,
        saving_timer: None,
        session_duration_s: config.duration_s,
        scenario_name: scenario_def.id.clone(),
    });

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
        reaction_times_s: Vec::new(),
        session_saved: false,
        drill: Default::default(),
        rest_left_s: None,
        flow_engine: SessionFlowEngine::new(config.scenario.clone()),
        battery_sequencer: if let Some(ref bat) = config.battery {
            let stages = resolve_battery_stages(bat).unwrap_or_else(|e| {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            });
            let out_dir = config
                .output_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from("sessions"));
            Some(BatterySequencer::new(bat.clone(), stages, out_dir))
        } else {
            None
        },
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

#[allow(clippy::too_many_arguments)]
fn handle_window_focus(
    mut commands: Commands,
    mut focus_events: MessageReader<WindowFocused>,
    mut state: ResMut<TrainerState>,
    mut pause_manager: ResMut<PauseManager>,
    mut nav: ResMut<PauseMenuNavigation>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    config: Res<TrainerConfig>,
) {
    for ev in focus_events.read() {
        RawInputWorker::set_window_focused(ev.focused);
        if ev.focused {
            nav.on_focus_regained();
        } else {
            trainer::raw_input::release_cursor_clip();
        }
        if !state.flow_engine.is_running() {
            continue;
        }
        if ev.focused {
            if let Some(mut cursor) = q_cursor.iter_mut().next() {
                if !pause_manager.is_paused {
                    cursor.grab_mode = CursorGrabMode::Confined;
                    cursor.visible = false;
                }
            }
        } else {
            // Free the cursor whenever focus is lost
            if let Some(mut cursor) = q_cursor.iter_mut().next() {
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
            }

            let startup_done = state.initial_startup_done;
            let is_completed = state.layout_engine.is_completed();
            let now_us = state.clock.now_us();
            let outcome = apply_focus_change(
                &mut pause_manager,
                &mut state.recorder,
                false,
                startup_done,
                is_completed,
                now_us,
            );

            if outcome == FocusOutcome::Paused {
                let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
                show_pause_menu(
                    &mut commands,
                    &ui_fonts,
                    &config.scenario,
                    elapsed_s,
                    config.duration_s,
                    &mut nav,
                    Some(PauseCause::FocusLost),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_and_evaluate_input_system(
    mut commands: Commands,
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
    pause_manager: Res<PauseManager>,
    assets: Option<Res<TargetAssets>>,
    mut camera_query: Query<&mut FpsCamera>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
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
    let input_qpc_start = state.worker.qpc_start;
    while let Ok(mut event) = state.worker.receiver.try_recv() {
        if !state.flow_engine.is_running() {
            continue;
        }
        event.t_us = state.clock.from_input_us(event.t_us, input_qpc_start);
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

    // Filter out pre-resume events from batch before replay_input so camera transform,
    // hit-testing, and recorded inputs strictly align and never diverge.
    if pause_manager.resume_qpc_us > 0 {
        batch.retain(|ev| ev.t_us >= pause_manager.resume_qpc_us);
    }

    let actions =
        trainer::replay::replay_input(&mut current_angles, &batch, primary_id, deg_per_count);

    // Update active tracking target positions incrementally BEFORE evaluating this frame's clicks
    if let Some((tracking_id, new_angles, radius_deg)) =
        state.layout_engine.update_tracking_target(now_active_us)
    {
        state.drill.track_frames += 1;
        if aim_core::geometry::SphericalTarget::new(new_angles, radius_deg)
            .intersect_ray(current_angles)
            .is_hit
        {
            state.drill.track_frames_on += 1;
        }
        let spawn_dist = state.scenario_def.spawn_distance_m as f32;
        let new_pos = trainer::target::angles_to_world_pos(new_angles, spawn_dist);
        for (_, mut marker, mut transform, _) in &mut target_query {
            if marker.target_id == tracking_id {
                marker.spherical = aim_core::geometry::SphericalTarget::new(new_angles, radius_deg);
                transform.translation = new_pos;
            }
        }
    }

    let rules = state.scenario_def.rules.clone();
    let shots_enabled = state.scenario_def.hit_rule.accepts_shots();
    if rules.counter_strafe && state.flow_engine.is_running() {
        use aim_telemetry::format::KeyCode as RecordedKey;
        let (left, right) = (KeyCode::KeyA, KeyCode::KeyD);
        for (bevy_key, recorded) in [(left, RecordedKey::KeyA), (right, RecordedKey::KeyD)] {
            if keyboard.just_pressed(bevy_key) {
                state.recorder.record_key(now_qpc_us, recorded, true);
            }
            if keyboard.just_released(bevy_key) {
                state.recorder.record_key(now_qpc_us, recorded, false);
            }
        }
        state.drill.player.step(
            f64::from(time.delta_secs()),
            keyboard.pressed(left),
            keyboard.pressed(right),
        );
        let velocity = state.drill.player.velocity;
        state.recorder.record_player(now_qpc_us, velocity);
    }
    // Drill rules follow the trial target; multi-target scenarios use none of them.
    let active_trial = target_query
        .iter()
        .find(|(_, marker, _, _)| !marker.is_home_target)
        .map(|(_, marker, _, _)| (marker.target_id, marker.spherical.center));
    let mut live_angles = cam_angles_start;

    // Fix 1: True per-event input latency metric: render_start_t_us - event.t_us for events applied in this frame
    for action in actions {
        match action {
            trainer::replay::ReplayAction::Motion { t_us, dx, dy } => {
                state.recorder.record_input(t_us, dx, dy);
                state.drill.motion.push(t_us, dx, dy);
                live_angles.apply_counts(dx, dy, deg_per_count);
                if let Some((id, center)) = active_trial {
                    state.drill.observe(
                        id,
                        (center.yaw, center.pitch),
                        live_angles.yaw,
                        live_angles.pitch,
                    );
                }
                if render_start_t_us >= t_us {
                    state.input_latencies_us.push(render_start_t_us - t_us);
                }
            }
            trainer::replay::ReplayAction::Click {
                t_us,
                angles_at_click,
            } => {
                if !shots_enabled {
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

                    if !marker.is_home_target && rules.show_gain {
                        state.drill.last_reach = state.drill.reach();
                    }
                    let rejected = !marker.is_home_target
                        && (trainer::drill_rules::ShotCheck {
                            rules: &rules,
                            runtime: &state.drill,
                            t_us,
                            deg_per_count,
                        })
                        .rejection()
                        .is_some();

                    if hit_res.is_hit && !rejected {
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
                                    state.reaction_times_s.push(trial_elapsed_s);
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
                                    state.reaction_times_s.push(trial_elapsed_s);
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
    if !state.flow_engine.is_running() {
        return;
    }
    let now_us = state.clock.now_us();
    let frame_us = now_us.saturating_sub(state.last_frame_t_us);
    state.last_frame_t_us = now_us;
    state.frame_times_us.push(frame_us);
    state.recorder.record_frame(now_us, now_us);
}

#[allow(clippy::too_many_arguments)]
fn session_lifecycle_system(
    mut commands: Commands,
    config: Res<TrainerConfig>,
    mut state: ResMut<TrainerState>,
    pause_manager: Res<PauseManager>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    target_query: Query<Entity, With<ActiveTargetMarker>>,
    q_overlay: Query<Entity, With<PauseOverlayRoot>>,
) {
    if !state.flow_engine.is_running() {
        return;
    }

    let now_qpc_us = state.clock.now_us();
    let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_qpc_us));
    let completed_all = state.layout_engine.is_completed();

    if elapsed_s >= config.duration_s || completed_all {
        state.flow_engine.finish_running();

        // 1. Despawn any remaining in-game targets
        for entity in &target_query {
            commands.entity(entity).despawn();
        }

        // 2. Unlock cursor
        if let Some(mut cursor) = q_cursor.iter_mut().next() {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }

        // 3. Despawn pause menu if open
        despawn_pause_menu(&mut commands, &q_overlay);

        let total_count = state.layout_engine.total_trials_or_targets();
        println!("\n=== AimForge Trainer Session Completed ===");
        println!("Elapsed time: {:.2}s", elapsed_s);
        println!(
            "Assessment Status: {}",
            if completed_all {
                format!("COMPLETED (All {} trials finished)", total_count)
            } else {
                "COMPLETED (Time expired)".to_string()
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

        save_session_telemetry(&mut state, &config, false);

        if let Some(ref mut seq) = state.battery_sequencer {
            seq.record_stage_completed();
        }

        let target_path_str = config
            .output_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| format!("session-{}.aft", state.recorder.setup.session_id));

        let results_input = SessionResultsInput {
            scenario_name: state.scenario_def.id.clone(),
            hits: state.hits_count,
            misses: state.misses_count,
            reaction_times_s: state.reaction_times_s.clone(),
            session_duration_s: elapsed_s,
            saved_path: target_path_str,
            time_on_target_pct: state.drill.on_target_percent(),
        };
        let summary = compute_session_results(&results_input);

        let is_battery = state.battery_sequencer.is_some();
        let has_next = state
            .battery_sequencer
            .as_ref()
            .map(|seq| seq.current_index() + 1 < seq.total_stages())
            .unwrap_or(false);

        spawn_results_overlay(&mut commands, &ui_fonts, &summary, is_battery, has_next);
        if has_next && config.rest_s > 0.0 {
            state.rest_left_s = Some(config.rest_s);
        }
    }
}

fn save_session_telemetry(state: &mut TrainerState, config: &TrainerConfig, is_aborted: bool) {
    if state.session_saved {
        return;
    }
    state.session_saved = true;

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

#[allow(clippy::too_many_arguments)]
fn pause_menu_toggle_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pause_manager: ResMut<PauseManager>,
    mut nav: ResMut<PauseMenuNavigation>,
    mut state: ResMut<TrainerState>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: Query<Entity, With<PauseOverlayRoot>>,
    config: Res<TrainerConfig>,
) {
    if !state.flow_engine.is_running() && !pause_manager.is_paused {
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        let now_us = state.clock.now_us();
        if pause_manager.is_paused {
            if nav.is_confirming_discard {
                nav.is_confirming_discard = false;
                nav.focused_index = 0;
                despawn_pause_menu(&mut commands, &q_overlay);
                let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
                spawn_pause_menu(
                    &mut commands,
                    &ui_fonts,
                    &config.scenario,
                    elapsed_s,
                    config.duration_s,
                    &nav,
                    pause_manager.pause_cause,
                );
                return;
            }

            let TrainerState {
                ref mut recorder,
                ref mut layout_engine,
                ..
            } = *state;
            execute_resume(
                &mut commands,
                &mut pause_manager,
                recorder,
                layout_engine,
                &mut q_cursor,
                &q_overlay,
                now_us,
            );
        } else {
            execute_pause(
                &mut commands,
                &mut pause_manager,
                &mut nav,
                &mut state.recorder,
                &mut q_cursor,
                &ui_fonts,
                &config.scenario,
                config.duration_s,
                now_us,
                PauseCause::Manual,
            );
        }
    }
}

#[allow(deprecated)]
#[allow(clippy::too_many_arguments)]
fn pause_menu_button_system(
    mut commands: Commands,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut pause_manager: ResMut<PauseManager>,
    mut nav: ResMut<PauseMenuNavigation>,
    mut state: ResMut<TrainerState>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: Query<Entity, With<PauseOverlayRoot>>,
    mut interaction_query: Query<
        (&Interaction, &PauseButtonAction, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut exit: MessageWriter<AppExit>,
    config: Res<TrainerConfig>,
) {
    if !pause_manager.is_paused {
        return;
    }

    // Handle saving countdown timer
    if let Some(timer) = nav.saving_timer.as_mut() {
        *timer += time.delta_secs();
        if *timer >= 0.6 {
            exit.write(AppExit::Success);
        }
        return;
    }

    let clicks_allowed = nav.tick_click_guard(time.delta_secs());
    let mut action_to_trigger = None;

    // Keyboard navigation: Up/Down arrow moves focus, Enter activates
    let max_index = if nav.is_confirming_discard { 1 } else { 2 };
    let mut focus_changed = false;

    if keyboard.just_pressed(KeyCode::ArrowUp) {
        if nav.focused_index > 0 {
            nav.focused_index -= 1;
        } else {
            nav.focused_index = max_index;
        }
        focus_changed = true;
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        if nav.focused_index < max_index {
            nav.focused_index += 1;
        } else {
            nav.focused_index = 0;
        }
        focus_changed = true;
    } else if keyboard.just_pressed(KeyCode::Enter) {
        if nav.is_confirming_discard {
            action_to_trigger = match nav.focused_index {
                0 => Some(PauseButtonAction::ConfirmCancel),
                1 => Some(PauseButtonAction::ConfirmDiscard),
                _ => None,
            };
        } else {
            action_to_trigger = match nav.focused_index {
                0 => Some(PauseButtonAction::Resume),
                1 => Some(PauseButtonAction::QuitAndSave),
                2 => Some(PauseButtonAction::QuitWithoutSaving),
                _ => None,
            };
        }
    }

    for (interaction, action, mut bg_color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                if clicks_allowed {
                    action_to_trigger = Some(*action);
                }
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(trainer::pause::button_background(*action, true));
            }
            Interaction::None => {
                *bg_color = BackgroundColor(trainer::pause::button_background(*action, false));
            }
        }
    }

    if let Some(action) = action_to_trigger {
        let now_us = state.clock.now_us();
        match action {
            PauseButtonAction::Resume => {
                let TrainerState {
                    ref mut recorder,
                    ref mut layout_engine,
                    ..
                } = *state;
                execute_resume(
                    &mut commands,
                    &mut pause_manager,
                    recorder,
                    layout_engine,
                    &mut q_cursor,
                    &q_overlay,
                    now_us,
                );
            }
            PauseButtonAction::QuitAndSave => {
                println!("\nQuit & Save requested. Saving partial session...");
                save_session_telemetry(&mut state, &config, true);
                nav.saving_timer = Some(0.0);
                despawn_pause_menu(&mut commands, &q_overlay);
                let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
                spawn_pause_menu(
                    &mut commands,
                    &ui_fonts,
                    &config.scenario,
                    elapsed_s,
                    config.duration_s,
                    &nav,
                    pause_manager.pause_cause,
                );
            }
            PauseButtonAction::QuitWithoutSaving => {
                // Show confirmation inside the same card
                nav.is_confirming_discard = true;
                nav.focused_index = 0; // Default focus on Cancel
                despawn_pause_menu(&mut commands, &q_overlay);
                let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
                spawn_pause_menu(
                    &mut commands,
                    &ui_fonts,
                    &config.scenario,
                    elapsed_s,
                    config.duration_s,
                    &nav,
                    pause_manager.pause_cause,
                );
            }
            PauseButtonAction::ConfirmCancel => {
                nav.is_confirming_discard = false;
                nav.focused_index = 0;
                despawn_pause_menu(&mut commands, &q_overlay);
                let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
                spawn_pause_menu(
                    &mut commands,
                    &ui_fonts,
                    &config.scenario,
                    elapsed_s,
                    config.duration_s,
                    &nav,
                    pause_manager.pause_cause,
                );
            }
            PauseButtonAction::ConfirmDiscard => {
                println!("\nSession aborted by user without saving.");
                exit.write(AppExit::Success);
            }
        }
    } else if focus_changed {
        let now_us = state.clock.now_us();
        despawn_pause_menu(&mut commands, &q_overlay);
        let elapsed_s = SessionClock::us_to_s(pause_manager.active_time_us(now_us));
        spawn_pause_menu(
            &mut commands,
            &ui_fonts,
            &config.scenario,
            elapsed_s,
            config.duration_s,
            &nav,
            pause_manager.pause_cause,
        );
    }
}

/// 03_DIAGNOSIS_ENGINE: short rests between tests keep the battery fair.
const DEFAULT_REST_S: f32 = 15.0;

#[derive(Component)]
struct FeedbackText;

#[derive(Component)]
struct SessionHudCell(usize);

#[derive(Component)]
struct SessionHudValue(usize);

#[derive(Component)]
struct SessionHudLabel(usize);

fn spawn_feedback_hud(mut commands: Commands, ui_fonts: Res<trainer::fonts::UiFonts>) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|root| {
            root.spawn(Node {
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                for i in 0..3 {
                    row.spawn((
                        Node {
                            display: Display::None,
                            min_width: Val::Px(128.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(trainer::ui_tokens::COLOR_HUD_PANEL),
                        SessionHudCell(i),
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            Text::new(""),
                            TextFont {
                                font: bevy::text::FontSource::Handle(ui_fonts.semibold.clone()),
                                font_size: bevy::text::FontSize::Px(
                                    trainer::ui_tokens::FONT_SIZE_HUD_VALUE,
                                ),
                                ..default()
                            },
                            TextColor(COLOR_TEXT_PRIMARY),
                            SessionHudValue(i),
                        ));
                        cell.spawn((
                            Text::new(""),
                            TextFont {
                                font: bevy::text::FontSource::Handle(ui_fonts.regular.clone()),
                                font_size: bevy::text::FontSize::Px(
                                    trainer::ui_tokens::FONT_SIZE_HUD_LABEL,
                                ),
                                ..default()
                            },
                            TextColor(trainer::ui_tokens::COLOR_TEXT_MUTED),
                            SessionHudLabel(i),
                        ));
                    });
                }
            });
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(72.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|root| {
            root.spawn((
                Text::new(""),
                TextFont {
                    font: bevy::text::FontSource::Handle(ui_fonts.semibold.clone()),
                    font_size: bevy::text::FontSize::Px(22.0),
                    ..default()
                },
                TextColor(COLOR_TEXT_PRIMARY),
                FeedbackText,
            ));
        });
}

/// One line under the crosshair: how far the last flick went, or whether the player can shoot.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_session_hud(
    mut state: ResMut<TrainerState>,
    config: Res<TrainerConfig>,
    time: Res<Time>,
    pause_manager: Res<PauseManager>,
    mut nav: ResMut<PauseMenuNavigation>,
    mut cells: Query<(&SessionHudCell, &mut Node)>,
    mut values: Query<(&SessionHudValue, &mut Text, &mut TextColor), Without<SessionHudLabel>>,
    mut labels: Query<(&SessionHudLabel, &mut Text), Without<SessionHudValue>>,
    mut rest_hint: Query<
        &mut Text,
        (
            With<trainer::session_flow::RestHint>,
            Without<SessionHudValue>,
            Without<SessionHudLabel>,
        ),
    >,
) {
    use trainer::session_flow::{hud_cells, HudNumbers, HudTone};
    use trainer::ui_tokens::{strings, COLOR_WARN};

    if let Some(left) = state.rest_left_s {
        let left = left - time.delta_secs();
        if let Some(mut hint) = rest_hint.iter_mut().next() {
            hint.0 = if left > 0.0 {
                format!(
                    "{} {}s   •   Esc",
                    strings::rest_label(),
                    left.ceil() as u32
                )
            } else {
                strings::results_next_hint().to_string()
            };
        }
        state.rest_left_s = (left > 0.0).then_some(left);
    }

    let running = state.flow_engine.is_running() && !pause_manager.is_paused;
    let tracking = !state.scenario_def.hit_rule.accepts_shots();
    let on_target = state.drill.on_target_percent().map(|p| p as u32);
    if running {
        nav.live = trainer::pause::LiveStats {
            hits: state.hits_count,
            misses: state.misses_count,
            total: state.layout_engine.total_trials_or_targets() as u32,
            on_target_pct: if tracking {
                Some(on_target.unwrap_or(0))
            } else {
                None
            },
        };
    }
    let shown = if running {
        let elapsed = SessionClock::us_to_s(pause_manager.active_time_us(state.clock.now_us()));
        hud_cells(HudNumbers {
            remaining_s: (config.duration_s - elapsed).max(0.0),
            hits: state.hits_count,
            misses: state.misses_count,
            total: state.layout_engine.total_trials_or_targets() as u32,
            tracking,
            on_target_pct: on_target.unwrap_or(0),
        })
    } else {
        Vec::new()
    };
    for (cell, mut node) in &mut cells {
        let display = if cell.0 < shown.len() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    for (slot, mut text, mut color) in &mut values {
        if let Some(cell) = shown.get(slot.0) {
            if text.0 != cell.value {
                text.0 = cell.value.clone();
            }
            color.0 = match cell.tone {
                HudTone::Normal => COLOR_TEXT_PRIMARY,
                HudTone::Good => COLOR_ACCENT,
                HudTone::Warn => COLOR_WARN,
                HudTone::Bad => COLOR_DANGER,
            };
        }
    }
    for (slot, mut text) in &mut labels {
        if let Some(cell) = shown.get(slot.0) {
            if text.0 != cell.label {
                text.0 = cell.label.to_string();
            }
        }
    }
}

fn update_feedback_hud(
    state: Res<TrainerState>,
    mut text: Query<(&mut Text, &mut TextColor), With<FeedbackText>>,
) {
    use trainer::ui_tokens::strings;
    let Some((mut line, mut color)) = text.iter_mut().next() else {
        return;
    };
    let rules = &state.scenario_def.rules;
    let (value, tint) = if !state.flow_engine.is_running() {
        (String::new(), COLOR_TEXT_PRIMARY)
    } else if rules.counter_strafe {
        let speed = state.drill.player.velocity.abs();
        if state.drill.player.is_accurate() {
            (
                format!("{speed:.1} m/s  {}", strings::hud_ready()),
                COLOR_ACCENT,
            )
        } else {
            (
                format!("{speed:.1} m/s  {}", strings::hud_moving()),
                COLOR_DANGER,
            )
        }
    } else if let (true, Some(reach)) = (rules.show_gain, state.drill.last_reach) {
        let percent = reach * 100.0;
        if reach > 1.05 {
            (
                format!("{percent:.0}%  {}", strings::hud_overshoot()),
                COLOR_DANGER,
            )
        } else if reach < 0.85 {
            (
                format!("{percent:.0}%  {}", strings::hud_short()),
                COLOR_DANGER,
            )
        } else {
            (
                format!("{percent:.0}%  {}", strings::hud_on_target()),
                COLOR_ACCENT,
            )
        }
    } else {
        (String::new(), COLOR_TEXT_PRIMARY)
    };
    if line.0 != value {
        line.0 = value;
    }
    color.0 = tint;
}

fn lock_cursor_startup(
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut commands: Commands,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    state: Res<TrainerState>,
) {
    if let Some(mut cursor) = q_cursor.iter_mut().next() {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    let bat_info = state
        .battery_sequencer
        .as_ref()
        .map(|seq| (seq.current_index(), seq.total_stages()));
    spawn_instructions_overlay(&mut commands, &ui_fonts, &state.scenario_def.id, bat_info);
}

#[allow(clippy::too_many_arguments)]
fn flow_update_system(
    mut commands: Commands,
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<TrainerState>,
    mut config: ResMut<TrainerConfig>,
    ui_fonts: Res<trainer::fonts::UiFonts>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: Query<Entity, With<FlowOverlayRoot>>,
    mut q_digit: Query<&mut Text, With<trainer::session_flow::CountdownDigitMarker>>,
    mut exit: MessageWriter<AppExit>,
) {
    let current_flow = state.flow_engine.state();
    match current_flow {
        FlowState::Instructions => {
            if keyboard.just_pressed(KeyCode::Escape) {
                state.flow_engine.handle_esc_exit();
                despawn_flow_overlay(&mut commands, &q_overlay);
                println!("\nSession cancelled during instructions.");
                exit.write(AppExit::Success);
                return;
            }

            if (keyboard.just_pressed(KeyCode::Space)
                || keyboard.just_pressed(KeyCode::Enter)
                || mouse_buttons.just_pressed(MouseButton::Left))
                && state.flow_engine.handle_start_trigger()
            {
                despawn_flow_overlay(&mut commands, &q_overlay);
                spawn_countdown_overlay(&mut commands, &ui_fonts, 3);
            }
        }
        FlowState::Countdown { seconds_remaining } => {
            if keyboard.just_pressed(KeyCode::Escape) {
                state.flow_engine.handle_esc_exit();
                despawn_flow_overlay(&mut commands, &q_overlay);
                println!("\nSession cancelled during countdown.");
                exit.write(AppExit::Success);
                return;
            }

            let dt = time.delta_secs();
            let transitioned = state.flow_engine.tick_countdown(dt);
            if transitioned {
                // Countdown complete -> Transition to Running!
                despawn_flow_overlay(&mut commands, &q_overlay);
                if let Some(mut cursor) = q_cursor.iter_mut().next() {
                    cursor.grab_mode = CursorGrabMode::Confined;
                    cursor.visible = false;
                }
                // Reset clock start to this exact instant
                state.clock.reset_start();
                println!(
                    "\n--- Session START: {} (Duration: {:.1}s) ---",
                    state.scenario_def.id, config.duration_s
                );
            } else if let FlowState::Countdown {
                seconds_remaining: new_sec,
            } = state.flow_engine.state()
            {
                if new_sec != seconds_remaining {
                    for mut text in &mut q_digit {
                        **text = format!("{}", new_sec);
                    }
                }
            }
        }
        FlowState::Results => {
            let advance_pressed = state.rest_left_s.is_none()
                && (keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::Space));
            let esc_pressed = keyboard.just_pressed(KeyCode::Escape);

            if esc_pressed {
                despawn_flow_overlay(&mut commands, &q_overlay);
                println!("\nTrainer closing from results screen.");
                exit.write(AppExit::Success);
                return;
            }

            if advance_pressed {
                let has_next = if let Some(ref mut seq) = state.battery_sequencer {
                    seq.advance()
                } else {
                    false
                };

                if has_next {
                    let (next_sc_id, next_out_path) = {
                        let seq = state.battery_sequencer.as_ref().unwrap();
                        (
                            seq.current_scenario_id().unwrap().to_string(),
                            seq.current_output_path(),
                        )
                    };
                    println!("\nAdvancing to next battery scenario: {}", next_sc_id);
                    let (next_sc_def, next_sc_seed) =
                        load_scenario(&next_sc_id, config.form, config.seed);
                    config.scenario = next_sc_id.clone();
                    config.output_path = Some(next_out_path.clone());
                    config.duration_s =
                        trainer::resolve_scenario_duration(&next_sc_def, config.duration_override);

                    despawn_flow_overlay(&mut commands, &q_overlay);

                    state.scenario_def = next_sc_def.clone();
                    let new_init_t = state.clock.now_us();
                    state.layout_engine =
                        LayoutEngineState::new(&next_sc_def, next_sc_seed.seed, new_init_t);
                    state.hits_count = 0;
                    state.misses_count = 0;
                    state.reaction_times_s.clear();
                    state.session_saved = false;
                    state.drill = Default::default();
                    state.recorder = SessionRecorder::new(recorder::RecorderSetup {
                        session_id: uuid::Uuid::new_v4().to_string(),
                        scenario_id: next_sc_id.clone(),
                        form: next_sc_seed.form,
                        seed: next_sc_seed.seed,
                        dpi: config.dpi,
                        sens: config.sens,
                        width: state.recorder.setup.width,
                        height: state.recorder.setup.height,
                        hz: state.recorder.setup.hz,
                    });
                    state.flow_engine.reset_for_next_scenario(next_sc_id);

                    let bat_info = state
                        .battery_sequencer
                        .as_ref()
                        .map(|seq| (seq.current_index(), seq.total_stages()));
                    spawn_instructions_overlay(
                        &mut commands,
                        &ui_fonts,
                        &state.scenario_def.id,
                        bat_info,
                    );
                } else {
                    despawn_flow_overlay(&mut commands, &q_overlay);
                    println!("\nSession results accepted. Exiting.");
                    exit.write(AppExit::Success);
                }
            }
        }
        FlowState::Running | FlowState::ExitedWithoutSave => {}
    }
}

fn main() {
    let (mut config, duration_override) = parse_cli_args();
    config.duration_override = duration_override;
    if let Some(ref bat) = config.battery {
        let stages = resolve_battery_stages(bat).unwrap_or_else(|e| {
            eprintln!("ERROR: {}", e);
            std::process::exit(1);
        });
        if let Some(first) = stages.first() {
            config.scenario = first.clone();
        }
        if let Some(ref dir) = config.output_dir {
            let _ = std::fs::create_dir_all(dir);
            config.output_path = Some(dir.join(format!("0_{}.aft", config.scenario)));
        }
    }
    let (scenario_def, scenario_seed) = load_scenario(&config.scenario, config.form, config.seed);
    // --scenario may be a file path; everything after loading works with the scenario's own id.
    config.scenario = scenario_def.id.clone();
    config.duration_s = trainer::resolve_scenario_duration(&scenario_def, duration_override);

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
            unfocused_mode: UpdateMode::reactive_low_power(std::time::Duration::from_millis(100)),
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
        .add_systems(Update, (update_feedback_hud, update_session_hud))
        .add_systems(
            Startup,
            (
                trainer::fonts::setup_ui_fonts,
                setup_trainer,
                setup_target_assets,
                spawn_fps_camera,
                spawn_crosshair_ui,
                lock_cursor_startup,
                spawn_feedback_hud,
            )
                .chain(),
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
                flow_update_system,
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
