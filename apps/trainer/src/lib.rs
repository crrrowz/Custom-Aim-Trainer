//! AimForge native trainer library exposing camera, raw input, and recording subsystems.

pub mod camera;
pub mod clock;
pub mod drill_rules;
pub mod feedback;
pub mod fonts;
pub mod layout_engine;
pub mod pause;
pub mod raw_input;
pub mod recorder;
pub mod replay;
pub mod session_flow;
pub mod target;
pub mod ui_tokens;

use aim_scenarios::schema::{Scenario, TargetLayout};
use bevy::prelude::Resource;
use std::path::{Path, PathBuf};

pub const VALID_ASSESS_SCENARIO_IDS: &[&str] = &[
    "assess.v1.warmup",
    "assess.v1.flick_static",
    "assess.v1.micro_precision",
    "assess.v1.head_level_peeks",
    "assess.v1.smooth_track",
    "assess.v1.strafe_track",
    "assess.v1.multi_switch",
    "assess.v1.counter_strafe",
];

/// Resolves a scenario file path from a scenario name or id.
pub fn resolve_scenario_file(scenario_name: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(scenario_name),
        PathBuf::from(format!("{}.ron", scenario_name)),
        PathBuf::from(format!("content/scenarios/{}.ron", scenario_name)),
        PathBuf::from(format!("content/scenarios/assess.v1.{}.ron", scenario_name)),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Discovers all available valid scenario IDs found in `content/scenarios/`
/// combined with canonical assessment scenarios.
pub fn available_scenario_ids(content_dir: Option<&Path>) -> Vec<String> {
    let mut ids: Vec<String> = VALID_ASSESS_SCENARIO_IDS
        .iter()
        .map(|s| s.to_string())
        .collect();

    let dir = content_dir.unwrap_or_else(|| Path::new("content/scenarios"));
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ron") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let id_str = stem.to_string();
                    if !ids.contains(&id_str) {
                        ids.push(id_str);
                    }
                }
            }
        }
    }
    ids.sort();
    ids
}

/// Loads and validates a scenario from file or fallback, returning an error listing valid IDs if unknown.
pub fn load_scenario_by_id(scenario_name: &str) -> Result<Scenario, String> {
    let resolved = resolve_scenario_file(scenario_name);
    let sc = match resolved {
        Some(path) => {
            let ron_str = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed reading scenario file '{}': {}", path.display(), e))?;
            ron::from_str::<Scenario>(&ron_str)
                .map_err(|e| format!("Failed parsing scenario file '{}': {}", path.display(), e))?
        }
        None => {
            if scenario_name == "assess.v1.flick_static" || scenario_name == "flick_static" {
                const FALLBACK_RON: &str =
                    include_str!("../../../content/scenarios/assess.v1.flick_static.ron");
                ron::from_str(FALLBACK_RON)
                    .map_err(|e| format!("Failed parsing fallback flick_static.ron: {}", e))?
            } else {
                let valid = available_scenario_ids(None);
                return Err(format!(
                    "Unknown scenario '{}'. Valid scenario IDs are: {}",
                    scenario_name,
                    valid.join(", ")
                ));
            }
        }
    };

    sc.validate()
        .map_err(|e| format!("Scenario '{}' validation failed: {}", sc.id, e))?;

    Ok(sc)
}

/// Computes the active session duration / safety cap in seconds according to scenario definition.
///
/// Tracking scenarios (A4/A5) run their `duration_s`.
/// Trial-based scenarios end when their trials are done, with a safety cap = trials * trial_timeout_s + 10.0s.
/// ConcurrentCluster (A6) with a fixed duration_s uses its `duration_s`.
/// Explicit `override_duration_s` takes precedence over the scenario definition if provided.
pub fn resolve_scenario_duration(scenario: &Scenario, override_duration_s: Option<f64>) -> f64 {
    if let Some(dur) = override_duration_s {
        return dur;
    }

    match &scenario.layout {
        TargetLayout::Tracking { duration_s, .. } => *duration_s,
        TargetLayout::ConcurrentCluster { duration_s, .. } => *duration_s,
        TargetLayout::Fitts {
            grid,
            trial_timeout_s,
        } => {
            let total_trials = grid.total_trials() as f64;
            total_trials * trial_timeout_s + 10.0
        }
        TargetLayout::HeadLevelPeeks {
            peeks_count,
            trial_timeout_s,
            ..
        } => {
            let total_trials = *peeks_count as f64;
            total_trials * trial_timeout_s + 10.0
        }
    }
}

/// The active scenario definition, inserted before the app runs so that
/// `Startup` systems in this crate (e.g. `target::setup_target_assets`) can
/// branch on the layout without depending on the binary crate.
#[derive(Resource, Debug, Clone)]
pub struct ScenarioResource(pub Scenario);
