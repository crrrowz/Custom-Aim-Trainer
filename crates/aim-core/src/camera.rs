//! Camera optics, view angles, FOV transformations, and angular distance calculations.

use crate::units::Degrees;

/// Standard VALORANT horizontal field of view in degrees (at 16:9 aspect ratio).
pub const VALORANT_HFOV_DEG: Degrees = 103.0;

/// Compute vertical FOV (degrees) given horizontal FOV (degrees) and aspect ratio (width / height).
pub fn vfov_from_hfov(hfov_deg: Degrees, aspect: f64) -> Degrees {
    let half_h_rad = (hfov_deg / 2.0).to_radians();
    let half_v_rad = (half_h_rad.tan() / aspect).atan();
    (2.0 * half_v_rad).to_degrees()
}

/// Angular radius in degrees of a sphere with radius `r` at distance `d` (in identical linear units).
pub fn angular_radius_deg(r: f64, d: f64) -> Degrees {
    (r / d).atan().to_degrees()
}

/// View orientation in angular degrees.
/// - `yaw`: [0.0, 360.0) degrees, where 0 is forward (+Z in standard FPS camera convention).
/// - `pitch`: [-89.0, 89.0] degrees, clamped to prevent gimbal lock / inversion.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewAngles {
    pub yaw: Degrees,
    pub pitch: Degrees,
}

impl ViewAngles {
    /// Constructs normalized ViewAngles.
    pub fn new(yaw: Degrees, pitch: Degrees) -> Self {
        let mut angles = Self { yaw, pitch };
        angles.normalize();
        angles
    }

    /// Normalizes yaw into [0.0, 360.0) and clamps pitch into [-89.0, 89.0].
    pub fn normalize(&mut self) {
        self.yaw = self.yaw.rem_euclid(360.0);
        self.pitch = self.pitch.clamp(-89.0, 89.0);
    }

    /// Update view angles from raw mouse count deltas `(dx, dy)` and rotational sensitivity factor.
    ///
    /// Following standard FPS input:
    /// - Positive dx turns right (increases yaw).
    /// - Positive dy moves mouse downward / pulls down (decreases pitch in elevation).
    pub fn apply_counts(&mut self, dx: i32, dy: i32, deg_per_count: f64) {
        self.yaw = (self.yaw + (dx as f64) * deg_per_count).rem_euclid(360.0);
        self.pitch = (self.pitch - (dy as f64) * deg_per_count).clamp(-89.0, 89.0);
    }

    /// Convert spherical (yaw, pitch) in degrees to a 3D unit direction vector [x, y, z].
    /// Convention:
    /// - pitch = 0, yaw = 0 points to [0, 0, 1] (forward)
    /// - yaw = 90 points to [1, 0, 0] (right)
    /// - pitch = 90 points to [0, 1, 0] (up)
    pub fn to_unit_vector(&self) -> [f64; 3] {
        let yaw_rad = self.yaw.to_radians();
        let pitch_rad = self.pitch.to_radians();
        let cos_pitch = pitch_rad.cos();

        [
            yaw_rad.sin() * cos_pitch,
            pitch_rad.sin(),
            yaw_rad.cos() * cos_pitch,
        ]
    }
}

/// Great-circle angular distance (in degrees) between two view angles.
///
/// Implemented via the numerically robust Haversine formula on spherical coordinates:
///   haversin(Δσ) = haversin(Δpitch) + cos(pitch1) * cos(pitch2) * haversin(Δyaw)
///
/// This avoids catastrophic cancellation for small angles (critical for micro-corrections)
/// and handles wrap-around across yaw boundaries seamlessly.
pub fn angular_distance(a: ViewAngles, b: ViewAngles) -> Degrees {
    let lat1 = a.pitch.to_radians();
    let lat2 = b.pitch.to_radians();
    let d_lat = (b.pitch - a.pitch).to_radians();
    let d_lon = (b.yaw - a.yaw).to_radians();

    let hav_lat = (d_lat / 2.0).sin().powi(2);
    let hav_lon = (d_lon / 2.0).sin().powi(2);

    let h = hav_lat + lat1.cos() * lat2.cos() * hav_lon;
    let clamped_h = h.clamp(0.0, 1.0);

    let angle_rad = 2.0 * clamped_h.sqrt().asin();
    angle_rad.to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_vfov_at_16_9() {
        let vfov = vfov_from_hfov(VALORANT_HFOV_DEG, 16.0 / 9.0);
        // VALORANT 103 HFOV at 16:9 produces approx 70.53 degrees
        assert!((vfov - 70.53).abs() < 0.05, "vfov = {}", vfov);
    }

    #[test]
    fn test_angular_distance_identity() {
        let v = ViewAngles::new(120.0, 15.0);
        assert!((angular_distance(v, v) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_angular_distance_known_orthogonal() {
        let a = ViewAngles::new(0.0, 0.0);
        let b = ViewAngles::new(90.0, 0.0);
        assert!((angular_distance(a, b) - 90.0).abs() < 1e-9);

        let c = ViewAngles::new(0.0, 45.0);
        let d = ViewAngles::new(0.0, -45.0);
        assert!((angular_distance(c, d) - 90.0).abs() < 1e-9);
    }

    #[test]
    fn test_angular_distance_yaw_wrap() {
        let a = ViewAngles::new(1.0, 0.0);
        let b = ViewAngles::new(359.0, 0.0);
        assert!((angular_distance(a, b) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_view_angles_apply_counts() {
        let mut v = ViewAngles::new(0.0, 0.0);
        v.apply_counts(100, 50, 0.07);
        // dx = 100 * 0.07 = 7.0 deg yaw
        // dy = 50 * 0.07 = -3.5 deg pitch
        assert!((v.yaw - 7.0).abs() < 1e-9);
        assert!((v.pitch - (-3.5)).abs() < 1e-9);
    }

    proptest! {
        #[test]
        fn test_angular_distance_symmetry(
            yaw1 in 0.0f64..360.0f64,
            pitch1 in -89.0f64..89.0f64,
            yaw2 in 0.0f64..360.0f64,
            pitch2 in -89.0f64..89.0f64
        ) {
            let a = ViewAngles::new(yaw1, pitch1);
            let b = ViewAngles::new(yaw2, pitch2);
            let d_ab = angular_distance(a, b);
            let d_ba = angular_distance(b, a);
            prop_assert!((d_ab - d_ba).abs() < 1e-9, "d_ab={}, d_ba={}", d_ab, d_ba);
            prop_assert!(d_ab >= 0.0);
            prop_assert!(d_ab <= 180.000001);
        }

        #[test]
        fn test_triangle_inequality(
            yaw1 in 0.0f64..360.0f64,
            pitch1 in -85.0f64..85.0f64,
            yaw2 in 0.0f64..360.0f64,
            pitch2 in -85.0f64..85.0f64,
            yaw3 in 0.0f64..360.0f64,
            pitch3 in -85.0f64..85.0f64
        ) {
            let a = ViewAngles::new(yaw1, pitch1);
            let b = ViewAngles::new(yaw2, pitch2);
            let c = ViewAngles::new(yaw3, pitch3);

            let ab = angular_distance(a, b);
            let bc = angular_distance(b, c);
            let ac = angular_distance(a, c);

            prop_assert!(ac <= ab + bc + 1e-7, "Triangle inequality failed: ac={}, ab+bc={}", ac, ab + bc);
        }
    }
}
