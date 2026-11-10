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

/// The player's crosshair, matching what they use in VALORANT.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct CrosshairStyle {
    pub color: [u8; 3],
    /// Arm length in pixels; 0 draws only the centre dot.
    pub length: f32,
    pub thickness: f32,
    pub gap: f32,
    pub dot: bool,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        Self {
            color: [0, 255, 76],
            length: 0.0,
            thickness: 2.0,
            gap: 3.0,
            dot: true,
        }
    }
}

impl CrosshairStyle {
    /// Parses `rrggbb:length:thickness:gap:dot`, for example `00ff4c:6:2:3:1`.
    pub fn parse(text: &str) -> Option<Self> {
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() != 5 || parts[0].len() != 6 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&parts[0][i..i + 2], 16).ok();
        let number = |s: &str| s.parse::<f32>().ok().filter(|v| (0.0..=40.0).contains(v));
        Some(Self {
            color: [byte(0)?, byte(2)?, byte(4)?],
            length: number(parts[1])?,
            thickness: number(parts[2])?.max(1.0),
            gap: number(parts[3])?,
            dot: parts[4] == "1",
        })
    }

    pub fn bevy_color(&self) -> Color {
        Color::srgb_u8(self.color[0], self.color[1], self.color[2])
    }

    /// The rectangles to draw, as (x, y, width, height) around the screen centre.
    pub fn rects(&self) -> Vec<(f32, f32, f32, f32)> {
        let t = self.thickness;
        let mut rects = Vec::new();
        if self.dot || self.length <= 0.0 {
            rects.push((-t / 2.0, -t / 2.0, t, t));
        }
        if self.length > 0.0 {
            let (g, l) = (self.gap, self.length);
            rects.push((g, -t / 2.0, l, t));
            rects.push((-g - l, -t / 2.0, l, t));
            rects.push((-t / 2.0, g, t, l));
            rects.push((-t / 2.0, -g - l, t, l));
        }
        rects
    }
}

/// Spawns the pixel-aligned 2D crosshair overlay centered on screen.
pub fn spawn_crosshair_ui(mut commands: Commands, style: Res<CrosshairStyle>) {
    let color = style.bevy_color();
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
            parent
                .spawn(Node {
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                })
                .with_children(|centre| {
                    for (x, y, w, h) in style.rects() {
                        centre.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(x),
                                top: Val::Px(y),
                                width: Val::Px(w),
                                height: Val::Px(h),
                                ..default()
                            },
                            BackgroundColor(color),
                        ));
                    }
                });
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
    fn crosshair_text_round_trips() {
        let style = CrosshairStyle::parse("ff0000:6:2:3:1").unwrap();
        assert_eq!(style.color, [255, 0, 0]);
        assert_eq!(style.rects().len(), 5);
        assert!(CrosshairStyle::parse("nope").is_none());
        assert!(CrosshairStyle::parse("ff0000:99:2:3:1").is_none());
    }

    #[test]
    fn a_dot_only_crosshair_draws_one_square() {
        let style = CrosshairStyle::parse("ffffff:0:4:0:1").unwrap();
        assert_eq!(style.rects(), vec![(-2.0, -2.0, 4.0, 4.0)]);
    }

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
