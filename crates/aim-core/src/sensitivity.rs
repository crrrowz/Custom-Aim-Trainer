//! VALORANT mouse sensitivity conversion and angular kinematics.

/// VALORANT yaw angular resolution: 0.07 degrees of yaw per mouse count at sensitivity 1.0.
pub const VALORANT_YAW_DEG_PER_COUNT: f64 = 0.07;

/// Exact conversion factor from inches to centimeters.
pub const CM_PER_INCH: f64 = 2.54;

/// Physical and in-game mouse configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MouseSetup {
    pub dpi: u32,
    pub in_game_sens: f64,
}

impl MouseSetup {
    /// Constructs a new MouseSetup.
    pub const fn new(dpi: u32, in_game_sens: f64) -> Self {
        Self { dpi, in_game_sens }
    }

    /// Degrees of camera rotation per raw mouse count.
    #[inline]
    pub fn deg_per_count(&self) -> f64 {
        VALORANT_YAW_DEG_PER_COUNT * self.in_game_sens
    }

    /// Effective DPI (DPI * sensitivity).
    #[inline]
    pub fn edpi(&self) -> f64 {
        self.dpi as f64 * self.in_game_sens
    }

    /// Physical mouse travel required for a complete 360° turn, in centimeters.
    pub fn cm_per_360(&self) -> f64 {
        let counts = 360.0 / self.deg_per_count();
        counts / self.dpi as f64 * CM_PER_INCH
    }

    /// Calculate the VALORANT in-game sensitivity required for a target cm/360 at a given DPI.
    pub fn sens_for_cm360(dpi: u32, cm360: f64) -> f64 {
        let counts = cm360 / CM_PER_INCH * dpi as f64;
        360.0 / counts / VALORANT_YAW_DEG_PER_COUNT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_known_values() {
        let s = MouseSetup::new(800, 0.35);
        assert!((s.edpi() - 280.0).abs() < 1e-9);
        assert!((s.cm_per_360() - 46.65).abs() < 0.01);
        let reconstructed_sens = MouseSetup::sens_for_cm360(800, s.cm_per_360());
        assert!((reconstructed_sens - 0.35).abs() < 1e-9);
    }

    #[test]
    fn test_deg_per_count_baseline() {
        let s = MouseSetup::new(1600, 1.0);
        assert!((s.deg_per_count() - VALORANT_YAW_DEG_PER_COUNT).abs() < 1e-9);
    }

    proptest! {
        #[test]
        fn test_cm360_roundtrip(
            dpi in 200u32..6400u32,
            sens in 0.05f64..3.0f64
        ) {
            let setup = MouseSetup::new(dpi, sens);
            let cm360 = setup.cm_per_360();
            prop_assert!(cm360 > 0.0);
            let recovered_sens = MouseSetup::sens_for_cm360(dpi, cm360);
            prop_assert!((recovered_sens - sens).abs() < 1e-8, "Failed roundtrip: original={}, recovered={}", sens, recovered_sens);
        }

        #[test]
        fn test_edpi_monotonicity(
            dpi in 400u32..3200u32,
            sens_low in 0.1f64..1.0f64,
            delta in 0.01f64..1.0f64
        ) {
            let s1 = MouseSetup::new(dpi, sens_low);
            let s2 = MouseSetup::new(dpi, sens_low + delta);
            prop_assert!(s2.edpi() > s1.edpi());
            prop_assert!(s2.deg_per_count() > s1.deg_per_count());
            prop_assert!(s1.cm_per_360() > s2.cm_per_360());
        }
    }
}
