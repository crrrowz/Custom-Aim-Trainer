//! AimForge native trainer library exposing camera, raw input, and recording subsystems.

pub mod camera;
pub mod clock;
pub mod layout_engine;
pub mod raw_input;
pub mod recorder;
pub mod replay;
pub mod target;

use aim_scenarios::Scenario;
use bevy::prelude::Resource;

/// The active scenario definition, inserted before the app runs so that
/// `Startup` systems in this crate (e.g. `target::setup_target_assets`) can
/// branch on the layout without depending on the binary crate.
#[derive(Resource, Debug, Clone)]
pub struct ScenarioResource(pub Scenario);
