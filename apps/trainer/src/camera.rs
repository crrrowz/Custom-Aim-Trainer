//! 3D FPS Camera conforming to VALORANT optics and projection geometry.

use aim_core::camera::{vfov_from_hfov, ViewAngles, VALORANT_HFOV_DEG};
use bevy::prelude::*;

/// Component holding the current orientation of the player's view.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct FpsCamera {
    pub angles: ViewAngles,
}

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

/// Computes the camera orientation Quat from ViewAngles.
pub fn camera_rotation_from_angles(angles: ViewAngles) -> Quat {
    let yaw_rad = angles.yaw.to_radians();
    let pitch_rad = angles.pitch.to_radians();
    Quat::from_euler(EulerRot::YXZ, (-yaw_rad) as f32, pitch_rad as f32, 0.0)
}

/// Spawns the VALORANT 3D camera.
pub fn spawn_fps_camera(mut commands: Commands) {
    let aspect = 16.0 / 9.0;
    let proj = valorant_projection(aspect);

    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(proj),
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::new(0.0, 0.0, -1.0), Vec3::Y),
        FpsCamera::default(),
    ));
}

/// Spawns the pixel-aligned 2D crosshair overlay centered on screen.
pub fn spawn_crosshair_ui(mut commands: Commands) {
    // Exact screen-center root container
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            position_type: PositionType::Absolute,
            ..default()
        })
        .with_children(|parent| {
            // 4x4 pixel green crosshair dot (#00ff4c)
            parent.spawn((
                Node {
                    width: Val::Px(4.0),
                    height: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.0, 1.0, 0.3)),
            ));
        });
}

/// Synchronizes the Camera's Transform with its `FpsCamera` orientation.
pub fn sync_camera_transform(mut query: Query<(&FpsCamera, &mut Transform)>) {
    for (fps_cam, mut transform) in &mut query {
        transform.rotation = camera_rotation_from_angles(fps_cam.angles);
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
