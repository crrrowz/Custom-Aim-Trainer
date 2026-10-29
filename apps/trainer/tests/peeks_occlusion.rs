use aim_core::camera::ViewAngles;
use aim_core::geometry::SphericalTarget;
use trainer::layout_engine::PeekTargetMarker;

#[test]
fn test_peek_target_starts_fully_behind_occluder_and_is_miss() {
    let distance_m = 15.0f32;
    let target_radius_deg = 0.45;
    let occluder_yaw = 25.0;
    let noisy_pitch = 1.25;
    let peek_speed_mps = 5.73;

    let half_wall_ang = ((0.5 / distance_m as f64).asin()).to_degrees();
    let wall_edge_yaw = occluder_yaw - half_wall_ang;

    let initial_yaw = wall_edge_yaw + target_radius_deg;
    let initial_angles = ViewAngles::new(initial_yaw, noisy_pitch);

    let marker = PeekTargetMarker {
        trial_idx: 0,
        target_id: 1,
        spherical: SphericalTarget::new(initial_angles, target_radius_deg),
        occluder_yaw,
        target_yaw: initial_yaw,
        pitch: noisy_pitch,
        peek_speed_mps,
        spawn_distance_m: distance_m,
        spawn_t_us: 1_000_000,
        emergence_t_us: 1_000_000,
    };

    assert!(
        (marker.wall_edge_yaw() - wall_edge_yaw).abs() < 1e-9,
        "Wall edge yaw mismatch"
    );

    // At t = emergence_t_us (t=0 relative), center is at initial_yaw
    let angles_at_start = marker.target_angles_at(1_000_000);
    assert!(
        (angles_at_start.yaw - initial_yaw).abs() < 1e-9,
        "Initial yaw must be fully behind wall edge: expected {}, got {}",
        initial_yaw,
        angles_at_start.yaw
    );

    // Shot directly at target center at t=0 must be occluded by the wall
    let (hit_center, is_occluded_center) = marker.evaluate_shot(angles_at_start, 1_000_000);
    assert!(
        is_occluded_center,
        "Shot at target center while behind wall must be detected as occluded"
    );
    assert!(!hit_center, "Shot at fully occluded target must be a miss");
}

#[test]
fn test_peek_target_emerged_part_is_hit_and_occluded_part_is_miss() {
    let distance_m = 15.0f32;
    let target_radius_deg = 0.45;
    let occluder_yaw = 25.0;
    let noisy_pitch = 1.25;
    let peek_speed_mps = 5.73;

    let half_wall_ang = ((0.5 / distance_m as f64).asin()).to_degrees();
    let wall_edge_yaw = occluder_yaw - half_wall_ang;

    let initial_yaw = wall_edge_yaw + target_radius_deg;
    let initial_angles = ViewAngles::new(initial_yaw, noisy_pitch);

    let marker = PeekTargetMarker {
        trial_idx: 0,
        target_id: 1,
        spherical: SphericalTarget::new(initial_angles, target_radius_deg),
        occluder_yaw,
        target_yaw: initial_yaw,
        pitch: noisy_pitch,
        peek_speed_mps,
        spawn_distance_m: distance_m,
        spawn_t_us: 1_000_000,
        emergence_t_us: 1_000_000,
    };

    let peek_speed_deg_s = (peek_speed_mps / distance_m as f64).to_degrees();
    // Time needed to emerge by target_radius_deg so center is exactly on wall_edge_yaw
    let dt_s = target_radius_deg / peek_speed_deg_s;
    let click_t_us = 1_000_000 + (dt_s * 1_000_000.0) as u64;

    let center_at_t = marker.target_angles_at(click_t_us);
    assert!(
        (center_at_t.yaw - wall_edge_yaw).abs() < 1e-4,
        "Center should be at wall edge: expected {}, got {}",
        wall_edge_yaw,
        center_at_t.yaw
    );

    // Shot 1: aimed at emerged portion (towards center, yaw < wall_edge_yaw)
    let emerged_point = ViewAngles::new(wall_edge_yaw - 0.2, noisy_pitch);
    let (hit_emerged, occluded_emerged) = marker.evaluate_shot(emerged_point, click_t_us);
    assert!(!occluded_emerged, "Emerged part must not be occluded");
    assert!(hit_emerged, "Shot at emerged part must be a hit");

    // Shot 2: aimed at occluded portion (behind wall, yaw > wall_edge_yaw)
    let occluded_point = ViewAngles::new(wall_edge_yaw + 0.2, noisy_pitch);
    let (hit_occluded, occluded_hidden) = marker.evaluate_shot(occluded_point, click_t_us);
    assert!(occluded_hidden, "Hidden part must be occluded");
    assert!(!hit_occluded, "Shot at occluded part must be a miss");
}

#[test]
fn test_peek_target_emerge_position_matches_peek_speed_mps() {
    let distance_m = 15.0f32;
    let target_radius_deg = 0.45;
    let occluder_yaw = 25.0;
    let noisy_pitch = 1.25;
    let peek_speed_mps = 5.73;

    let half_wall_ang = ((0.5 / distance_m as f64).asin()).to_degrees();
    let wall_edge_yaw = occluder_yaw - half_wall_ang;
    let initial_yaw = wall_edge_yaw + target_radius_deg;
    let initial_angles = ViewAngles::new(initial_yaw, noisy_pitch);

    let marker = PeekTargetMarker {
        trial_idx: 0,
        target_id: 1,
        spherical: SphericalTarget::new(initial_angles, target_radius_deg),
        occluder_yaw,
        target_yaw: initial_yaw,
        pitch: noisy_pitch,
        peek_speed_mps,
        spawn_distance_m: distance_m,
        spawn_t_us: 1_000_000,
        emergence_t_us: 1_000_000,
    };

    let peek_speed_deg_s = (peek_speed_mps / distance_m as f64).to_degrees();

    // Check multiple timestamps
    for dt_ms in [50, 100, 200, 300, 500] {
        let t_us = 1_000_000 + dt_ms * 1_000;
        let pos = marker.target_angles_at(t_us);
        let expected_yaw = initial_yaw - (dt_ms as f64 / 1000.0) * peek_speed_deg_s;
        assert!(
            (pos.yaw - expected_yaw).abs() < 1e-6,
            "Yaw at dt={}ms: expected {}, got {}",
            dt_ms,
            expected_yaw,
            pos.yaw
        );
        assert!(
            (pos.pitch - noisy_pitch).abs() < 1e-9,
            "Pitch must remain constant during horizontal peek"
        );
    }
}
