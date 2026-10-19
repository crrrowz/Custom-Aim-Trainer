//! aim-scenarios: Scenario definitions & seeded generator

pub mod battery;
pub mod generator;
pub mod rng;
pub mod schema;
pub mod trajectory;

pub use battery::{derive_scenario_seed, BatteryManifest, BatteryStage, BATTERY_FORM_SEEDS};
pub use generator::{
    generate_cluster_candidates, generate_fitts_trials, generate_peek_trials, generate_trials,
    target_angles_from_bearing, PeekTrial, Trial,
};
pub use rng::{create_seeded_rng, sample_range_f64, sample_range_u32, sample_unit_f64};
pub use schema::{
    DistanceAnchor, FittsGrid, HitRule, Scenario, ScenarioKind, TargetLayout, TargetShape,
    TrackingTrajectory,
};
pub use trajectory::{generate_strafe_reversals, target_angles_at};
