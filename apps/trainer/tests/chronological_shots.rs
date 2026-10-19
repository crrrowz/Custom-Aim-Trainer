use aim_core::geometry::SphericalTarget;
use aim_core::sensitivity::VALORANT_YAW_DEG_PER_COUNT;
use aim_core::ViewAngles;
use trainer::raw_input::{RawInputEvent, RawInputPayload};
use trainer::replay::{replay_input, ReplayAction};

#[test]
fn test_chronological_shot_evaluation_at_exact_click_timestamp() {
    let sens = 0.35;
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * sens;

    let target_angles = ViewAngles::new(10.0, 0.0);
    let target = SphericalTarget::new(target_angles, 1.0);

    let mut current_angles = ViewAngles::new(0.0, 0.0);

    let dx_to_target = (10.0 / deg_per_count).round() as i32;

    let events = vec![
        RawInputEvent {
            device_id: 1,
            t_us: 1000,
            payload: RawInputPayload::Motion {
                dx: dx_to_target,
                dy: 0,
            },
        },
        RawInputEvent {
            device_id: 1,
            t_us: 2000,
            payload: RawInputPayload::LeftButtonDown,
        },
        RawInputEvent {
            device_id: 1,
            t_us: 3000,
            payload: RawInputPayload::Motion {
                dx: dx_to_target * 3,
                dy: 0,
            },
        },
    ];

    let actions = replay_input(&mut current_angles, &events, Some(1), deg_per_count);

    assert_eq!(actions.len(), 3, "Expected exactly 3 replay actions");

    let mut shot_evaluated_hit = false;
    let mut hit_distance = 0.0;
    let mut click_count = 0;
    let mut motion_count = 0;

    for action in actions {
        match action {
            ReplayAction::Motion { .. } => {
                motion_count += 1;
            }
            ReplayAction::Click {
                angles_at_click, ..
            } => {
                click_count += 1;
                let hit_res = target.intersect_ray(angles_at_click);
                shot_evaluated_hit = hit_res.is_hit;
                hit_distance = hit_res.angular_distance_deg;
            }
        }
    }

    assert_eq!(motion_count, 2, "Expected 2 motion actions");
    assert_eq!(click_count, 1, "Expected 1 click action");
    assert!(
        shot_evaluated_hit,
        "Shot must register hit at intermediate position A (10°), not contaminated by subsequent motion B (40°)!"
    );
    assert!(
        hit_distance < 0.1,
        "Hit distance should be near zero, got {hit_distance}"
    );
    assert!(
        (current_angles.yaw - 40.0).abs() < 1.0,
        "Final camera moved past target to ~40° after all deltas applied"
    );
}

#[test]
fn test_per_event_input_latency_metric() {
    let sens = 0.35;
    let deg_per_count = VALORANT_YAW_DEG_PER_COUNT * sens;
    let mut current_angles = ViewAngles::new(0.0, 0.0);

    // Frame 1: Render start at 5,000 us.
    // Drains 3 input events captured at 2,000 us, 3,000 us, and 4,000 us.
    let events_frame1 = vec![
        RawInputEvent {
            device_id: 1,
            t_us: 2000,
            payload: RawInputPayload::Motion { dx: 10, dy: 0 },
        },
        RawInputEvent {
            device_id: 1,
            t_us: 3000,
            payload: RawInputPayload::Motion { dx: 10, dy: 0 },
        },
        RawInputEvent {
            device_id: 1,
            t_us: 4000,
            payload: RawInputPayload::Motion { dx: 10, dy: 0 },
        },
    ];

    let frame1_render_start_us = 5000u64;
    let mut applied_latencies = Vec::new();

    let actions = replay_input(&mut current_angles, &events_frame1, Some(1), deg_per_count);
    for action in actions {
        if let ReplayAction::Motion { t_us, .. } = action {
            assert!(frame1_render_start_us >= t_us);
            applied_latencies.push(frame1_render_start_us - t_us);
        }
    }

    assert_eq!(applied_latencies.len(), 3);
    // Event 1 latency: 5000 - 2000 = 3000 us (3.0 ms)
    assert_eq!(applied_latencies[0], 3000);
    // Event 2 latency: 5000 - 3000 = 2000 us (2.0 ms)
    assert_eq!(applied_latencies[1], 2000);
    // Event 3 latency: 5000 - 4000 = 1000 us (1.0 ms)
    assert_eq!(applied_latencies[2], 1000);

    // Median latency for this frame
    applied_latencies.sort_unstable();
    let p50 = applied_latencies[applied_latencies.len() / 2];
    assert_eq!(p50, 2000, "Median per-event latency must be 2000 us (2.0 ms)");
}
