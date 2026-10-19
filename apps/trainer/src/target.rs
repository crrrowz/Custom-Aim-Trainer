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

#[derive(Component)]
pub struct CrosshairMarker;

#[derive(Resource)]
pub struct TargetAssets {
    pub home_mesh: Handle<Mesh>,
    pub home_material: Handle<StandardMaterial>,
    pub trial_mesh: Handle<Mesh>,
    pub trial_material: Handle<StandardMaterial>,
    pub hit_material: Handle<StandardMaterial>,
    pub optic_flow_mesh: Handle<Mesh>,
    pub optic_flow_material: Handle<StandardMaterial>,
    pub crosshair_mesh: Handle<Mesh>,
    pub crosshair_material: Handle<StandardMaterial>,
}

pub fn setup_target_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Unlit target materials
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

    // Dark neutral optic-flow background material with double-sided rendering
    let optic_flow_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.07, 0.09), // Subtle dark backdrop (#0f1117)
        unlit: true,
        cull_mode: None, // Inward visible sphere
        ..default()
    });

    // Green crosshair material
    let crosshair_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.3), // Vibrant green
        unlit: true,
        depth_bias: 1000.0,
        ..default()
    });

    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let backdrop_mesh = meshes.add(Sphere::new(40.0).mesh().ico(4).unwrap());
    let dot_mesh = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());

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
        crosshair_mesh: dot_mesh,
        crosshair_material,
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
    let linear_radius = distance_m * (radius_deg as f32).to_radians().tan();

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
    let linear_radius = distance_m * (trial.radius_deg as f32).to_radians().tan();

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
