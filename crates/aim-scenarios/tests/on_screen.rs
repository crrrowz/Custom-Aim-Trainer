//! Every assessment target must appear on screen: a target the player cannot see measures searching,
//! not aiming.

use aim_scenarios::generator::{
    generate_cluster_candidates, generate_trials, target_angles_from_bearing,
};
use aim_scenarios::schema::{Scenario, TargetLayout, ON_SCREEN_H_DEG, ON_SCREEN_V_DEG};

fn assessment_scenarios() -> Vec<Scenario> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("../../content/scenarios").unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with("assess.v1.") && name.ends_with(".ron") {
            found.push(ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap());
        }
    }
    assert!(found.len() >= 8, "all assessment tests are checked");
    found
}

fn assert_visible(id: &str, distance: f64, bearing: f64, radius: f64) {
    let (yaw, pitch) = target_angles_from_bearing(distance, bearing);
    let yaw = if yaw > 180.0 { yaw - 360.0 } else { yaw };
    assert!(
        yaw.abs() + radius <= ON_SCREEN_H_DEG && pitch.abs() + radius <= ON_SCREEN_V_DEG,
        "{id}: target at {distance} deg, bearing {bearing} is off screen (yaw {yaw:.1}, pitch {pitch:.1})"
    );
}

#[test]
fn every_assessment_target_is_on_screen() {
    for sc in assessment_scenarios() {
        match &sc.layout {
            TargetLayout::Fitts { .. } => {
                for t in generate_trials(&sc, 7) {
                    assert_visible(
                        &sc.id,
                        t.distance_deg,
                        f64::from(t.octant) * 45.0,
                        t.radius_deg,
                    );
                }
            }
            TargetLayout::ConcurrentCluster {
                octants,
                target_radius_deg,
                ..
            } => {
                for (d, o) in generate_cluster_candidates(&sc, 7) {
                    assert_visible(
                        &sc.id,
                        d,
                        f64::from(o) * 360.0 / f64::from(*octants),
                        *target_radius_deg,
                    );
                }
            }
            _ => {}
        }
    }
}
