use aim_scenarios::schema::DrillRules;
use trainer::drill_rules::{
    DrillRuntime, FlickWatch, MotionWindow, PlayerMotion, Rejection, ShotCheck,
};

const DPC: f64 = 0.02;

#[test]
fn still_mouse_is_settled_and_moving_mouse_is_not() {
    let mut m = MotionWindow::default();
    for i in 0..100 {
        m.push(1_000_000 + i * 1_000, 10, 0);
    }
    assert!(!m.settled(1_100_000, 40.0, DPC));
    assert!(m.settled(1_300_000, 40.0, DPC));
}

#[test]
fn speed_matches_known_motion() {
    let mut m = MotionWindow::default();
    // 5 counts per ms at 0.02 deg per count is 100 deg/s.
    for i in 0..100 {
        m.push(1_000_000 + i * 1_000, 5, 0);
    }
    assert!((m.speed_deg_s(1_099_000, 50.0, DPC) - 100.0).abs() < 3.0);
}

#[test]
fn old_motion_is_forgotten() {
    let mut m = MotionWindow::default();
    m.push(1_000_000, 1000, 0);
    m.push(5_000_000, 0, 0);
    assert_eq!(m.speed_deg_s(5_000_000, 10_000.0, DPC), 0.0);
}

#[test]
fn reach_measures_the_furthest_point_along_the_flick() {
    let mut w = FlickWatch::new(1, (0.0, 0.0), (10.0, 0.0));
    w.update(5.0, 0.0);
    assert!((w.reach() - 0.5).abs() < 1e-9);
    w.update(12.0, 0.0);
    w.update(10.0, 0.0);
    assert!((w.reach() - 1.2).abs() < 1e-9, "keeps the peak");
}

#[test]
fn reach_handles_the_360_degree_wrap() {
    let mut w = FlickWatch::new(1, (355.0, 0.0), (5.0, 0.0));
    w.update(0.0, 0.0);
    assert!((w.reach() - 0.5).abs() < 1e-9, "{}", w.reach());
}

#[test]
fn new_target_starts_a_new_watch() {
    let mut r = DrillRuntime::default();
    r.observe(1, (10.0, 0.0), 12.0, 0.0);
    r.observe(2, (40.0, 0.0), 12.0, 0.0);
    assert_eq!(r.watch.as_ref().unwrap().target_id, 2);
}

#[test]
fn counter_strafing_stops_faster_than_letting_go() {
    let (mut released, mut countered) = (PlayerMotion::default(), PlayerMotion::default());
    for _ in 0..30 {
        released.step(0.01, false, true);
        countered.step(0.01, false, true);
    }
    let mut steps = (0, 0);
    while !released.is_accurate() {
        released.step(0.001, false, false);
        steps.0 += 1;
    }
    while !countered.is_accurate() {
        countered.step(0.001, true, false);
        steps.1 += 1;
    }
    assert!(steps.1 < steps.0, "{steps:?}");
}

#[test]
fn standing_still_is_accurate_and_full_speed_is_not() {
    let mut p = PlayerMotion::default();
    assert!(p.is_accurate());
    for _ in 0..50 {
        p.step(0.01, true, false);
    }
    assert!(!p.is_accurate());
}

fn check(rules: &DrillRules, runtime: &DrillRuntime) -> Option<Rejection> {
    ShotCheck {
        rules,
        runtime,
        t_us: 2_000_000,
        deg_per_count: DPC,
    }
    .rejection()
}

#[test]
fn no_rules_means_every_hit_counts() {
    let mut r = DrillRuntime::default();
    r.player.velocity = 5.0;
    assert_eq!(check(&DrillRules::default(), &r), None);
}

#[test]
fn each_rule_rejects_its_own_mistake() {
    let mut r = DrillRuntime::default();
    r.observe(1, (10.0, 0.0), 0.0, 0.0);
    r.observe(1, (10.0, 0.0), 13.0, 0.0);
    let overshoot = DrillRules {
        overshoot_fail_ratio: Some(1.1),
        ..Default::default()
    };
    assert_eq!(check(&overshoot, &r), Some(Rejection::Overshot));

    let mut moving = DrillRuntime::default();
    moving.motion.push(1_999_000, 50, 0);
    let settle = DrillRules {
        settle_ms: Some(40.0),
        ..Default::default()
    };
    assert_eq!(check(&settle, &moving), Some(Rejection::NotSettled));

    let mut running = DrillRuntime::default();
    running.player.velocity = 3.0;
    let strafe = DrillRules {
        counter_strafe: true,
        ..Default::default()
    };
    assert_eq!(check(&strafe, &running), Some(Rejection::Moving));
}
