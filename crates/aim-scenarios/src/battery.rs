//! Deterministic parallel form seed derivation and battery manifests.

use serde::{Deserialize, Serialize};

/// Canonical 64-bit seed table for Battery Parallel Forms 0–4.
pub const BATTERY_FORM_SEEDS: [u64; 5] = [
    0x5A17_F000_0000_0001,
    0x5A17_F000_0000_0002,
    0x5A17_F000_0000_0003,
    0x5A17_F000_0000_0004,
    0x5A17_F000_0000_0005,
];

/// Derives a stable, deterministic scenario seed from form index (0..4) and scenario ID.
///
/// Ensures different tests within the same form are statistically uncorrelated
/// while remaining strictly deterministic across runs.
pub fn derive_scenario_seed(form_idx: usize, scenario_id: &str) -> u64 {
    let base_seed = BATTERY_FORM_SEEDS[form_idx % BATTERY_FORM_SEEDS.len()];
    let mut h: u64 = base_seed ^ 0xCBF29CE484222325;
    for &b in scenario_id.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001B3);
    }
    h
}

/// Manifest entry for an individual stage within the assessment battery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryStage {
    pub scenario: String,
    pub rest_after_s: f64,
}

/// Complete assessment battery manifest defining the sequence of tests and rest intervals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatteryManifest {
    pub id: String,
    pub version: u32,
    pub stages: Vec<BatteryStage>,
}
