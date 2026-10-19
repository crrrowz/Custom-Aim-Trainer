use aim_core::geometry::SphericalTarget;
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_core::ViewAngles;
use trainer::raw_input::RawInputPayload;

#[test]
fn test_chronological_shot_evaluation_at_exact_click_timestamp() {
    let sens = 0.35;
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * sens;

    let target_angles = ViewAngles::new(10.0, 0.0);
    let target = SphericalTarget::new(target_angles, 1.0);

    let mut current_angles = ViewAngles::new(0.0, 0.0);

    let dx_to_target = (10.0 / deg_per_count).round() as i32;

    let events = vec![
        RawInputPayload::Motion {
            dx: dx_to_target,
            dy: 0,
        },
        RawInputPayload::LeftButtonDown,
        RawInputPayload::Motion {
            dx: dx_to_target * 3,
            dy: 0,
        },
    ];

    let mut shot_evaluated_hit = false;
    let mut hit_distance = 0.0;

    for ev in events {
        match ev {
            RawInputPayload::Motion { dx, dy } => {
                current_angles.apply_counts(dx, dy, deg_per_count);
            }
            RawInputPayload::LeftButtonDown => {
                let hit_res = target.intersect_ray(current_angles);
                shot_evaluated_hit = hit_res.is_hit;
                hit_distance = hit_res.angular_distance_deg;
            }
            _ => {}
        }
    }

    assert!(
        shot_evaluated_hit,
        "Shot should have been evaluated at A (target center), not A + B!"
    );
    assert!(
        hit_distance < 0.1,
        "Hit distance should be near zero, got {hit_distance}"
    );
    assert!(
        (current_angles.yaw - 40.0).abs() < 1.0,
        "Final camera moved past target to ~40°"
    );
}
