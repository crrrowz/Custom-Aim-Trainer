use aim_core::camera::ViewAngles;
use bevy::prelude::*;
use trainer::camera::{camera_rotation_from_angles, valorant_projection};
use trainer::target::angles_to_world_pos;

#[test]
fn test_camera_forward_matches_angles_to_world_pos() {
    let yaws = [-179, -135, -90, -45, 0, 45, 90, 135, 179];
    let pitches = [-80, -60, -30, 0, 30, 60, 80];

    let min_cos = (0.01f64).to_radians().cos() as f32 - 1e-6; // cos(0.01°) with f32 float tolerance

    for &y in &yaws {
        for &p in &pitches {
            let angles = ViewAngles::new(y as f64, p as f64);
            let rot = camera_rotation_from_angles(angles);
            let transform = Transform::from_rotation(rot);

            let forward = transform.forward().as_vec3();
            let world_pos = angles_to_world_pos(angles, 1.0);
            let target_dir = world_pos.normalize();

            let dot = forward.dot(target_dir);

            assert!(
                dot > min_cos,
                "Camera forward {:?} does not match target dir {:?} for yaw={}, pitch={}: dot={}, min_cos={}",
                forward, target_dir, y, p, dot, min_cos
            );
        }
    }
}

#[test]
fn test_target_10_deg_right_projects_to_right_half_of_screen() {
    let aspect = 16.0 / 9.0;
    let proj = valorant_projection(aspect);

    let camera_angles = ViewAngles::new(0.0, 0.0);
    let rot = camera_rotation_from_angles(camera_angles);
    let camera_transform = Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(rot);

    // Target 10° to the right (positive yaw)
    let target_angles = ViewAngles::new(10.0, 0.0);
    let target_pos = angles_to_world_pos(target_angles, 15.0);

    // Transform world to view space: view = camera_transform.to_matrix().inverse() * target_pos
    let view_matrix = camera_transform.to_matrix().inverse();
    let p_view = view_matrix.transform_point3(target_pos);

    // In Bevy view space: camera looks down -Z, +X is right, +Y is up.
    // For a target ahead of camera: p_view.z < 0 (down -Z)
    assert!(
        p_view.z < 0.0,
        "Target ahead must have view space z < 0, got {}",
        p_view.z
    );
    // For target 10° to the right: p_view.x must be positive (+X is right)
    assert!(
        p_view.x > 0.0,
        "Target 10° right must have positive view space x, got {}",
        p_view.x
    );

    // Compute standard perspective NDC x: (x / -z) / (tan(vfov / 2) * aspect)
    let tan_half_vfov = (proj.fov / 2.0).tan();
    let tan_half_hfov = tan_half_vfov * aspect as f32;
    let ndc_x = (p_view.x / -p_view.z) / tan_half_hfov;

    // NDC coordinates: x in [-1, 1], right half of screen is x > 0
    assert!(
        ndc_x > 0.0,
        "Target 10° right must project to ndc_x > 0, got {}",
        ndc_x
    );
    assert!(
        ndc_x < 1.0,
        "Target 10° right must be on screen (ndc_x < 1), got {}",
        ndc_x
    );
}
