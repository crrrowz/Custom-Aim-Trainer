//! aim-scenarios: Scenario definitions & seeded generator

pub mod generator;
pub mod rng;
pub mod schema;

pub use generator::{generate_trials, Trial};
pub use rng::{create_seeded_rng, sample_range_f64, sample_range_u32, sample_unit_f64};
pub use schema::{DistanceAnchor, FittsGrid, HitRule, Scenario, ScenarioKind, TargetShape};
