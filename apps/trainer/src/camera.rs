//! 3D FPS Camera and crosshair conforming to VALORANT optics and projection geometry.

use aim_core::camera::{vfov_from_hfov, ViewAngles, VALORANT_HFOV_DEG};
use bevy::prelude::*;

/// Component holding the current orientation of the player's view.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FpsCamera {
    pub angles: ViewAngles,
}

#[derive(Component)]
pub struct CrosshairEntity;

/// Computes the PerspectiveProjection configured for VALORANT at the given aspect ratio.
pub fn valorant_projection(aspect: f64) -> PerspectiveProjection {
    let vfov_deg = vfov_from_hfov(VALORANT_HFOV_DEG, aspect);
    PerspectiveProjection {
        fov: vfov_deg.to_radians() as f32,
        aspect_ratio: aspect as f32,
        near: 0.1,
        far: 1000.0,
        ..default()
    }
}

/// Spawns the VALORANT 3D camera with child crosshair lines positioned at screen center.
pub fn spawn_fps_camera(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let aspect = 16.0 / 9.0;
    let proj = valorant_projection(aspect);

    let crosshair_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.3), // Vibrant green
        unlit: true,
        depth_bias: 1000.0,
        ..default()
    });

    // Crosshair dot at exact center: small quad positioned in camera view space at z = -1.0
    let dot_mesh = meshes.add(Rectangle::new(0.003, 0.003));

    commands
        .spawn((
            Camera3d::default(),
            Projection::Perspective(proj),
            Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.0, 0.0, 1.0), Vec3::Y),
            FpsCamera::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(dot_mesh),
                MeshMaterial3d(crosshair_material),
                Transform::from_xyz(0.0, 0.0, 1.0), // 1.0 unit forward along camera line-of-sight
                CrosshairEntity,
            ));
        });
}

/// Synchronizes the Camera's Transform with its `FpsCamera` orientation.
pub fn sync_camera_transform(mut query: Query<(&FpsCamera, &mut Transform)>) {
    for (fps_cam, mut transform) in &mut query {
        let yaw_rad = fps_cam.angles.yaw.to_radians() as f32;
        let pitch_rad = fps_cam.angles.pitch.to_radians() as f32;

        transform.rotation = Quat::from_euler(EulerRot::YXZ, -yaw_rad, pitch_rad, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valorant_51_5_deg_projection_screen_edge() {
        let aspect = 16.0 / 9.0;
        let proj = valorant_projection(aspect);

        let theta_rad = 51.5f64.to_radians();
        let z = 10.0f64;
        let x = z * theta_rad.tan();

        let tan_half_vfov = (proj.fov as f64 / 2.0).tan();
        let tan_half_hfov = tan_half_vfov * aspect;

        let ndc_x = (x / z) / tan_half_hfov;

        assert!(
            (ndc_x - 1.0).abs() < 1e-5,
            "Target at 51.5° projected to NDC x = {ndc_x}, expected exactly 1.0"
        );
    }
}
