//! 3D Target visualization, optic-flow background, and state management for trainer scenarios.

use aim_core::geometry::SphericalTarget;
use aim_core::ViewAngles;
use aim_scenarios::Trial;
use bevy::prelude::*;

#[derive(Component)]
pub struct ActiveTargetMarker {
    pub target_id: u32,
    pub spherical: SphericalTarget,
    pub is_home_target: bool,
    pub spawn_time_s: f64,
}

#[derive(Component)]
pub struct OpticFlowSphere;

#[derive(Resource)]
pub struct TargetAssets {
    pub home_mesh: Handle<Mesh>,
    pub home_material: Handle<StandardMaterial>,
    pub trial_mesh: Handle<Mesh>,
    pub trial_material: Handle<StandardMaterial>,
    pub hit_material: Handle<StandardMaterial>,
    pub optic_flow_mesh: Handle<Mesh>,
    pub optic_flow_material: Handle<StandardMaterial>,
    pub occluder_mesh: Handle<Mesh>,
    pub occluder_material: Handle<StandardMaterial>,
}

/// Generates an unlit texture for the optic-flow sphere.
/// When `disable_latitude` is true (e.g. for A3 Head-Level Peeks), horizontal latitudinal lines
/// are omitted to prevent leaking a visual horizon cue to the player.
pub fn generate_optic_flow_image(
    images: &mut Assets<Image>,
    disable_latitude: bool,
) -> Handle<Image> {
    const W: u32 = 1024;
    const H: u32 = 512;
    let mut data = vec![0u8; (W * H * 4) as usize];

    for y in 0..H {
        let lat_deg = (y as f64 / H as f64) * 180.0 - 90.0;
        let is_lat_line = !disable_latitude && (lat_deg.abs() % 10.0) < 0.8;

        for x in 0..W {
            let lon_deg = (x as f64 / W as f64) * 360.0;
            let is_lon_line = (lon_deg % 10.0) < 0.8;

            let idx = ((y * W + x) * 4) as usize;
            if is_lat_line || is_lon_line {
                // Subtle contrast line: #1f2430
                data[idx] = 31;
                data[idx + 1] = 36;
                data[idx + 2] = 48;
                data[idx + 3] = 255;
            } else {
                // Dark background base: #0e1017
                data[idx] = 14;
                data[idx + 1] = 16;
                data[idx + 2] = 23;
                data[idx + 3] = 255;
            }
        }
    }

    let image = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );

    images.add(image)
}

pub fn setup_target_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let optic_flow_texture = generate_optic_flow_image(&mut images, false);

    let home_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 1.0), // Cyan home target
        unlit: true,
        ..default()
    });

    let trial_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.3, 0.2), // Bright red/orange target
        unlit: true,
        ..default()
    });

    let hit_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 1.0, 0.3), // Green flash on hit
        unlit: true,
        ..default()
    });

    let optic_flow_material = materials.add(StandardMaterial {
        base_color_texture: Some(optic_flow_texture),
        unlit: true,
        cull_mode: None, // Inward visible sphere
        ..default()
    });

    // Tall featureless occluder wall material (matches background #0e1017)
    let occluder_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.10, 0.14),
        unlit: true,
        ..default()
    });

    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let backdrop_mesh = meshes.add(Sphere::new(40.0).mesh().ico(4).unwrap());
    let wall_mesh = meshes.add(Cuboid::new(0.5, 30.0, 0.1));

    // Spawn camera-centered optic-flow inward sphere
    commands.spawn((
        Mesh3d(backdrop_mesh.clone()),
        MeshMaterial3d(optic_flow_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        OpticFlowSphere,
    ));

    commands.insert_resource(TargetAssets {
        home_mesh: sphere_mesh.clone(),
        home_material,
        trial_mesh: sphere_mesh,
        trial_material,
        hit_material,
        optic_flow_mesh: backdrop_mesh,
        optic_flow_material,
        occluder_mesh: wall_mesh,
        occluder_material,
    });
}

/// Convert spherical angular coordinates (yaw, pitch) and radial distance into a 3D Vec3 coordinate.
pub fn angles_to_world_pos(angles: ViewAngles, distance_m: f32) -> Vec3 {
    let yaw_rad = angles.yaw.to_radians() as f32;
    let pitch_rad = angles.pitch.to_radians() as f32;
    let cos_pitch = pitch_rad.cos();

    Vec3::new(
        yaw_rad.sin() * cos_pitch * distance_m,
        pitch_rad.sin() * distance_m,
        yaw_rad.cos() * cos_pitch * distance_m,
    )
}

/// Spawns the central home target that the player must shoot to begin each trial.
pub fn spawn_home_target(
    commands: &mut Commands,
    assets: &TargetAssets,
    distance_m: f32,
    target_id: u32,
    now_s: f64,
) {
    let home_angles = ViewAngles::new(0.0, 0.0);
    let radius_deg = 0.5;
    let linear_radius = distance_m * (radius_deg as f32).to_radians().sin();

    commands.spawn((
        Mesh3d(assets.home_mesh.clone()),
        MeshMaterial3d(assets.home_material.clone()),
        Transform::from_translation(angles_to_world_pos(home_angles, distance_m))
            .with_scale(Vec3::splat(linear_radius)),
        ActiveTargetMarker {
            target_id,
            spherical: SphericalTarget::new(home_angles, radius_deg),
            is_home_target: true,
            spawn_time_s: now_s,
        },
    ));
}

/// Spawns a trial target at its prescribed angular distance and octant direction.
pub fn spawn_trial_target(
    commands: &mut Commands,
    assets: &TargetAssets,
    trial: &Trial,
    distance_m: f32,
    target_id: u32,
    now_s: f64,
) {
    let angles = ViewAngles::new(trial.yaw_deg, trial.pitch_deg);
    let linear_radius = distance_m * (trial.radius_deg as f32).to_radians().sin();

    commands.spawn((
        Mesh3d(assets.trial_mesh.clone()),
        MeshMaterial3d(assets.trial_material.clone()),
        Transform::from_translation(angles_to_world_pos(angles, distance_m))
            .with_scale(Vec3::splat(linear_radius)),
        ActiveTargetMarker {
            target_id,
            spherical: SphericalTarget::new(angles, trial.radius_deg).with_head_zone(false),
            is_home_target: false,
            spawn_time_s: now_s,
        },
    ));
}
