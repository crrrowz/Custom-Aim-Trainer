//! Session telemetry recorder and .aft exporter.

use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_telemetry::format::{
    quality_flags, AftHeader, AftSession, AftStreams, DisplayConfig, HitZone, InputConfig,
    MouseConfig, ShotRecord, TargetKind, TargetParams, TargetRecord,
};
use aim_telemetry::write_aft;
use std::path::PathBuf;

pub struct RecorderSetup {
    pub session_id: String,
    pub scenario_id: String,
    pub seed: u64,
    pub dpi: u32,
    pub sens: f64,
    pub width: u32,
    pub height: u32,
    pub hz: u32,
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
        self.streams.targets.records.push(TargetRecord {
            id,
            spawn_t_us,
            despawn_t_us,
            kind: TargetKind::Sphere,
            params: TargetParams {
                radius_deg,
                initial_pos: [yaw_deg, pitch_deg, 15.0],
                path_spline: None,
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
                form: Some(1),
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
                    est_polling_hz: 1000,
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
