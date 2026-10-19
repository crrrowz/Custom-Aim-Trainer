//! 2D and 3D geometry helpers for angular and spatial coordinates.
//!
//! Includes angular-space ray-sphere intersection testing for precision FPS hit registration.

use crate::camera::{angular_distance, ViewAngles};
use crate::units::Degrees;

/// Hit evaluation result against an anatomical target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub is_hit: bool,
    /// Distance from crosshair ray center to target center in angular degrees.
    pub angular_distance_deg: Degrees,
    /// Hit location zone if hit.
    pub hitzone: HitZoneType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitZoneType {
    None,
    Head,
    Body,
}

/// Target spherical hit volume in angular space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalTarget {
    pub center: ViewAngles,
    pub radius_deg: Degrees,
    /// If true, the upper fraction of the sphere is classified as Head, and remainder as Body.
    pub has_head_zone: bool,
}

impl SphericalTarget {
    pub fn new(center: ViewAngles, radius_deg: Degrees) -> Self {
        Self {
            center,
            radius_deg,
            has_head_zone: false,
        }
    }

    pub fn with_head_zone(mut self, has_head: bool) -> Self {
        self.has_head_zone = has_head;
        self
    }

    /// Evaluates whether a view ray (crosshair direction) intersects this spherical target.
    ///
    /// Evaluated purely in angular space using great-circle distance:
    /// - `is_hit`: `angular_distance(view_ray, center) <= radius_deg`
    /// - If `has_head_zone` is true, hits in the upper 40% (relative pitch >= center.pitch + 0.2 * radius_deg)
    ///   are categorized as `HitZoneType::Head`, otherwise `HitZoneType::Body`.
    pub fn intersect_ray(&self, view_ray: ViewAngles) -> HitResult {
        let dist = angular_distance(view_ray, self.center);
        if dist <= self.radius_deg {
            let hitzone = if self.has_head_zone {
                let pitch_diff = view_ray.pitch - self.center.pitch;
                if pitch_diff >= 0.2 * self.radius_deg {
                    HitZoneType::Head
                } else {
                    HitZoneType::Body
                }
            } else {
                HitZoneType::Head
            };

            HitResult {
                is_hit: true,
                angular_distance_deg: dist,
                hitzone,
            }
        } else {
            HitResult {
                is_hit: false,
                angular_distance_deg: dist,
                hitzone: HitZoneType::None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_center_hit() {
        let target = SphericalTarget::new(ViewAngles::new(10.0, 5.0), 0.8);
        let ray = ViewAngles::new(10.0, 5.0);
        let res = target.intersect_ray(ray);

        assert!(res.is_hit);
        assert!((res.angular_distance_deg - 0.0).abs() < 1e-9);
        assert_eq!(res.hitzone, HitZoneType::Head);
    }

    #[test]
    fn test_tangent_edge_hit() {
        let target = SphericalTarget::new(ViewAngles::new(0.0, 0.0), 1.0);
        // Point exactly 1.0 deg along horizontal yaw
        let ray_on_edge = ViewAngles::new(1.0, 0.0);
        let res_edge = target.intersect_ray(ray_on_edge);
        assert!(res_edge.is_hit);
        assert!((res_edge.angular_distance_deg - 1.0).abs() < 1e-9);

        // Point just outside (1.0001 deg)
        let ray_outside = ViewAngles::new(1.0001, 0.0);
        let res_outside = target.intersect_ray(ray_outside);
        assert!(!res_outside.is_hit);
    }

    #[test]
    fn test_yaw_wrap_around_hit() {
        // Target at yaw 359.5°, ray at yaw 0.2° -> distance is 0.7°
        let target = SphericalTarget::new(ViewAngles::new(359.5, 0.0), 0.8);
        let ray = ViewAngles::new(0.2, 0.0);
        let res = target.intersect_ray(ray);

        assert!(res.is_hit);
        assert!((res.angular_distance_deg - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_head_and_body_zones() {
        let target = SphericalTarget::new(ViewAngles::new(0.0, 0.0), 1.0).with_head_zone(true);

        // Ray in upper portion (pitch +0.5°) -> Head
        let ray_head = ViewAngles::new(0.0, 0.5);
        let res_head = target.intersect_ray(ray_head);
        assert!(res_head.is_hit);
        assert_eq!(res_head.hitzone, HitZoneType::Head);

        // Ray in lower portion (pitch -0.2°) -> Body
        let ray_body = ViewAngles::new(0.0, -0.2);
        let res_body = target.intersect_ray(ray_body);
        assert!(res_body.is_hit);
        assert_eq!(res_body.hitzone, HitZoneType::Body);
    }
}
