//! Session telemetry recorder and .aft exporter.

use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_telemetry::format::{
    quality_flags, AftHeader, AftSession, AftStreams, DisplayConfig, HitZone, InputConfig, KeyCode,
    KeyRecord, MouseConfig, ShotRecord, TargetKind, TargetParams, TargetRecord,
};
use aim_telemetry::write_aft;
use std::path::PathBuf;

pub struct RecorderSetup {
    pub session_id: String,
    pub scenario_id: String,
    pub form: Option<u32>,
    pub seed: u64,
    pub dpi: u32,
    pub sens: f64,
    pub width: u32,
    pub height: u32,
    pub hz: u32,
}

/// A target spawn to append to the telemetry target stream.
pub struct TargetSpawn {
    pub id: u32,
    pub spawn_t_us: u64,
    pub despawn_t_us: u64,
    pub kind: TargetKind,
    pub radius_deg: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub path_spline: Option<String>,
}

pub struct SessionRecorder {
    pub setup: RecorderSetup,
    pub quality_flags: u32,
    pub streams: AftStreams,
}

impl SessionRecorder {
    pub fn new(setup: RecorderSetup) -> Self {
        Self {
            setup,
            quality_flags: 0,
            streams: AftStreams::default(),
        }
    }

    pub fn flag_focus_lost(&mut self) {
        self.quality_flags |= quality_flags::FOCUS_LOST;
    }

    pub fn flag_paused(&mut self) {
        self.quality_flags |= quality_flags::PAUSED;
    }

    pub fn record_key(&mut self, t_us: u64, key: KeyCode, down: bool) {
        self.streams
            .keys
            .records
            .push(KeyRecord { t_us, key, down });
    }

    pub fn record_player(&mut self, t_us: u64, velocity: f64) {
        self.streams.player.t_us.push(t_us);
        self.streams.player.vel_x.push(velocity);
        self.streams.player.vel_z.push(0.0);
    }

    pub fn record_input(&mut self, t_us: u64, dx: i32, dy: i32) {
        self.streams.input.t_us.push(t_us);
        self.streams.input.dx.push(dx);
        self.streams.input.dy.push(dy);
    }

    pub fn record_frame(&mut self, t_us: u64, present_t_us: u64) {
        self.streams.frames.t_us.push(t_us);
        self.streams.frames.present_t_us.push(present_t_us);
    }

    pub fn record_view(&mut self, t_us: u64, yaw_deg: f64, pitch_deg: f64) {
        self.streams.view.t_us.push(t_us);
        self.streams.view.yaw_deg.push(yaw_deg);
        self.streams.view.pitch_deg.push(pitch_deg);
    }

    pub fn record_shot(&mut self, t_us: u64, hit_target_id: Option<u32>, hitzone: HitZone) {
        self.streams.shots.records.push(ShotRecord {
            t_us,
            hit_target_id,
            hitzone,
        });
    }

    pub fn record_target_spawn(
        &mut self,
        id: u32,
        spawn_t_us: u64,
        despawn_t_us: u64,
        radius_deg: f64,
        yaw_deg: f64,
        pitch_deg: f64,
    ) {
        self.record_target_spawn_full(TargetSpawn {
            id,
            spawn_t_us,
            despawn_t_us,
            kind: TargetKind::Sphere,
            radius_deg,
            yaw_deg,
            pitch_deg,
            path_spline: None,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_peek_target_spawn(
        &mut self,
        id: u32,
        spawn_t_us: u64,
        despawn_t_us: u64,
        radius_deg: f64,
        yaw_deg: f64,
        noisy_pitch_deg: f64,
        nominal_head_pitch: f64,
    ) {
        self.record_target_spawn_full(TargetSpawn {
            id,
            spawn_t_us,
            despawn_t_us,
            kind: TargetKind::WallPeek,
            radius_deg,
            yaw_deg,
            pitch_deg: noisy_pitch_deg,
            path_spline: Some(format!("nominal_head_pitch:{}", nominal_head_pitch)),
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_tracking_target_spawn(
        &mut self,
        id: u32,
        spawn_t_us: u64,
        despawn_t_us: u64,
        radius_deg: f64,
        initial_yaw_deg: f64,
        initial_pitch_deg: f64,
        path_spline: String,
    ) {
        self.record_target_spawn_full(TargetSpawn {
            id,
            spawn_t_us,
            despawn_t_us,
            kind: TargetKind::Sphere,
            radius_deg,
            yaw_deg: initial_yaw_deg,
            pitch_deg: initial_pitch_deg,
            path_spline: Some(path_spline),
        });
    }

    pub fn record_target_spawn_full(&mut self, spawn: TargetSpawn) {
        self.streams.targets.records.push(TargetRecord {
            id: spawn.id,
            spawn_t_us: spawn.spawn_t_us,
            despawn_t_us: spawn.despawn_t_us,
            kind: spawn.kind,
            params: TargetParams {
                radius_deg: spawn.radius_deg,
                initial_pos: [spawn.yaw_deg, spawn.pitch_deg, 15.0],
                path_spline: spawn.path_spline,
            },
        });
    }

    pub fn build_session(&self) -> AftSession {
        let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * self.setup.sens;
        let aspect = self.setup.width as f64 / self.setup.height.max(1) as f64;
        let vfov = aim_core::camera::vfov_from_hfov(aim_core::camera::VALORANT_HFOV_DEG, aspect);

        AftSession {
            header: AftHeader {
                session_id: self.setup.session_id.clone(),
                created_at_utc: chrono::Utc::now().to_rfc3339(),
                app_version: "0.1.0".to_string(),
                trainer_version: "0.1.0".to_string(),
                scenario_id: self.setup.scenario_id.clone(),
                scenario_version: 1,
                form: self.setup.form,
                seed: self.setup.seed,
                game_profile: "valorant.v1".to_string(),
                mouse: MouseConfig {
                    dpi: self.setup.dpi,
                    sens: self.setup.sens,
                    deg_per_count,
                },
                display: DisplayConfig {
                    width: self.setup.width,
                    height: self.setup.height,
                    hz: self.setup.hz,
                    hfov: aim_core::camera::VALORANT_HFOV_DEG,
                    vfov,
                },
                input: InputConfig {
                    est_polling_hz: estimate_polling_hz(&self.streams.input.t_us),
                },
                thresholds_version: "thresholds.v1".to_string(),
                quality_flags: self.quality_flags,
            },
            streams: self.streams.clone(),
        }
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<usize, aim_telemetry::WriteError> {
        let session = self.build_session();
        let bytes = write_aft(&session)?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, &bytes).map_err(aim_telemetry::WriteError::Io)?;
        Ok(bytes.len())
    }
}

/// Minimum number of valid inter-event intervals required to estimate polling rate.
/// If fewer intervals are present, fallback to 1000 Hz.
pub const MIN_POLLING_SAMPLES: usize = 10;

/// Fallback polling rate in Hz when too few events are recorded.
pub const DEFAULT_POLLING_HZ: u32 = 1000;

/// Known standard mouse polling rates in Hz.
pub const STANDARD_POLLING_RATES: [u32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];

/// Estimates the mouse polling rate in Hz from a sequence of motion event timestamps (in microseconds).
///
/// Algorithm:
/// 1. Computes consecutive inter-event intervals (dt = t[i] - t[i-1]).
/// 2. Ignores non-positive intervals and idle gaps where dt > 50,000 us (50 ms).
/// 3. If valid intervals < MIN_POLLING_SAMPLES (10), returns DEFAULT_POLLING_HZ (1000 Hz).
/// 4. Computes the median interval.
/// 5. Derives raw Hz = 1_000_000.0 / median_interval_us.
/// 6. If raw Hz is within 10% of a standard rate (125, 250, 500, 1000, 2000, 4000, 8000),
///    snaps to that standard rate.
/// 7. Otherwise, returns raw Hz rounded to the nearest integer.
pub fn estimate_polling_hz(timestamps: &[u64]) -> u32 {
    if timestamps.len() < 2 {
        return DEFAULT_POLLING_HZ;
    }

    let mut intervals = Vec::with_capacity(timestamps.len() - 1);
    for window in timestamps.windows(2) {
        if window[1] > window[0] {
            let dt = window[1] - window[0];
            // Ignore idle gaps > 50 ms (50,000 us)
            if dt <= 50_000 {
                intervals.push(dt);
            }
        }
    }

    if intervals.len() < MIN_POLLING_SAMPLES {
        return DEFAULT_POLLING_HZ;
    }

    intervals.sort_unstable();
    let n = intervals.len();
    let median_us = if n % 2 == 1 {
        intervals[n / 2] as f64
    } else {
        (intervals[n / 2 - 1] as f64 + intervals[n / 2] as f64) / 2.0
    };

    if median_us <= 0.0 {
        return DEFAULT_POLLING_HZ;
    }

    let raw_hz = 1_000_000.0 / median_us;

    for &std_hz in &STANDARD_POLLING_RATES {
        let diff = (raw_hz - std_hz as f64).abs();
        if diff <= std_hz as f64 * 0.10 {
            return std_hz;
        }
    }

    raw_hz.round().max(1.0) as u32
}
