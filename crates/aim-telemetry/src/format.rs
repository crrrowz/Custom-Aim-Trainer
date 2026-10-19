//! Telemetry data structures and constants for the .aft format (v1).

use serde::{Deserialize, Serialize};

/// 4-byte magic signature for AimForge Telemetry files.
pub const AFT_MAGIC: &[u8; 4] = b"AFT\0";

/// Current format version (v1).
pub const AFT_FORMAT_VERSION_V1: u16 = 1;

/// Maximum allowable uncompressed header size (64 KiB).
pub const MAX_HEADER_UNCOMPRESSED_SIZE: usize = 64 * 1024;

/// Maximum allowable uncompressed stream size (32 MiB).
///
/// Justification:
/// A realistic maximum session in AimForge is 10 minutes (600 seconds) recorded with an
/// extreme 8,000 Hz mouse polling rate (4,800,000 raw input samples).
/// Delta-encoded timestamps and coordinates in postcard binary require ~15-25 MiB uncompressed.
/// Setting the per-stream ceiling to 32 MiB provides ample headroom for extreme high-frequency
/// hardware while preventing unbounded memory consumption.
pub const MAX_STREAM_UNCOMPRESSED_SIZE: usize = 32 * 1024 * 1024;

/// Maximum allowable cumulative uncompressed size across header and all streams (64 MiB).
///
/// Prevents memory exhaustion attacks where multiple streams each claim up to the per-stream limit.
pub const MAX_TOTAL_UNCOMPRESSED_SIZE: usize = 64 * 1024 * 1024;

/// Maximum plausible decompression ratio (1000:1) to reject decompression bombs prior to allocation.
pub const MAX_DECOMPRESSION_RATIO: usize = 1000;

/// Number of columnar streams in v1 format.
pub const NUM_STREAMS_V1: usize = 7;

/// Mouse hardware and in-game sensitivity configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseConfig {
    pub dpi: u32,
    pub sens: f64,
    pub deg_per_count: f64,
}

/// Display resolution, refresh rate, and field of view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub hz: u32,
    pub hfov: f64,
    pub vfov: f64,
}

/// Input polling configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputConfig {
    pub est_polling_hz: u32,
}

/// Header containing session metadata and environment setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AftHeader {
    pub session_id: String,
    pub created_at_utc: String,
    pub app_version: String,
    pub trainer_version: String,
    pub scenario_id: String,
    pub scenario_version: u32,
    pub form: Option<u32>,
    pub seed: u64,
    pub game_profile: String,
    pub mouse: MouseConfig,
    pub display: DisplayConfig,
    pub input: InputConfig,
    pub thresholds_version: String,
}

/// Target category/mesh archetype.
///
/// APPEND-ONLY: never reorder, rename-with-new-meaning, or remove variants;
/// .aft v1 postcard serialization depends strictly on variant indices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetKind {
    Sphere,
    HeadModel,
    WallPeek,
    Custom(String),
}

/// Parameters defining physical dimensions and trajectory for a target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetParams {
    pub radius_deg: f64,
    pub initial_pos: [f64; 3],
    pub path_spline: Option<String>,
}

/// Target lifecycle event record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetRecord {
    pub id: u32,
    pub spawn_t_us: u64,
    pub despawn_t_us: u64,
    pub kind: TargetKind,
    pub params: TargetParams,
}

/// Anatomical hit location.
///
/// APPEND-ONLY: never reorder, rename-with-new-meaning, or remove variants;
/// .aft v1 postcard serialization depends strictly on variant indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitZone {
    None,
    Head,
    Body,
    Limb,
}

/// Discrete shot event record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShotRecord {
    pub t_us: u64,
    pub hit_target_id: Option<u32>,
    pub hitzone: HitZone,
}

/// Hardware/virtual key identifier for movement mechanics.
///
/// APPEND-ONLY: never reorder, rename-with-new-meaning, or remove variants;
/// .aft v1 postcard serialization depends strictly on variant indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    Space,
    Shift,
    Ctrl,
    Unknown(u16),
}

/// Keyboard key state change event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyRecord {
    pub t_us: u64,
    pub key: KeyCode,
    pub down: bool,
}

/// Raw input motion stream (delta-encoded relative to previous sample).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct InputStream {
    pub t_us: Vec<u64>,
    pub dx: Vec<i32>,
    pub dy: Vec<i32>,
}

/// Video frame presentation stream.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FrameStream {
    pub t_us: Vec<u64>,
    pub present_t_us: Vec<u64>,
}

/// Observed or rendered camera view angles.
///
/// NOTE on ViewStream representation:
/// `yaw_deg` and `pitch_deg` are stored as raw `f64` (without delta encoding)
/// to avoid numerical drift and rounding degradation over extended sessions.
/// Note also that this view stream can be deterministically re-derived from
/// `InputStream` combined with `MouseConfig::deg_per_count`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewStream {
    pub t_us: Vec<u64>,
    pub yaw_deg: Vec<f64>,
    pub pitch_deg: Vec<f64>,
}

/// Target lifecycle stream.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TargetStream {
    pub records: Vec<TargetRecord>,
}

/// Player weapon shot stream.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ShotStream {
    pub records: Vec<ShotRecord>,
}

/// Player keyboard input stream.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct KeyStream {
    pub records: Vec<KeyRecord>,
}

/// Player kinematic movement state (for counter-strafe evaluation).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PlayerStream {
    pub t_us: Vec<u64>,
    pub vel_x: Vec<f64>,
    pub vel_z: Vec<f64>,
}

/// Container for all recorded event streams.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AftStreams {
    pub input: InputStream,
    pub frames: FrameStream,
    pub view: ViewStream,
    pub targets: TargetStream,
    pub shots: ShotStream,
    pub keys: KeyStream,
    pub player: PlayerStream,
}

/// Complete in-memory representation of an AimForge Telemetry session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AftSession {
    pub header: AftHeader,
    pub streams: AftStreams,
}

/// Stream offset directory entry in footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamIndexEntry {
    pub offset: u32,
    pub compressed_len: u32,
    pub uncompressed_len: u32,
}

/// Binary footer for verification and stream indexing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AftFooter {
    pub header_entry: StreamIndexEntry,
    pub stream_entries: [StreamIndexEntry; NUM_STREAMS_V1],
    pub crc32: u32,
}
