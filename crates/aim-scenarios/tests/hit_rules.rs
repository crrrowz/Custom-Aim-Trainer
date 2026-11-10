use aim_scenarios::schema::{HitRule, Scenario};

#[test]
fn only_shooting_scenarios_accept_shots() {
    assert!(HitRule::SingleShotKill.accepts_shots());
    assert!(!HitRule::ContinuousTracking.accepts_shots());
}

#[test]
fn both_tracking_tests_ignore_clicks() {
    for name in ["smooth_track", "strafe_track"] {
        let path = format!("../../content/scenarios/assess.v1.{name}.ron");
        let scenario = Scenario::from_ron_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(!scenario.hit_rule.accepts_shots(), "{name}");
    }
}
