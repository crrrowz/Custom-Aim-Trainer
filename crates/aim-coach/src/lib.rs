//! Decision layer: diagnosis rules, training plan, adaptive difficulty and statistical verdicts.
//! Pure logic only: no I/O, no game engine, no network.

pub mod diagnosis;
pub mod evaluation;
pub mod narrative;
pub mod plan;

pub use diagnosis::{diagnose, Finding};
