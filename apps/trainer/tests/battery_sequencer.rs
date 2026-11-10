use std::path::PathBuf;
use trainer::session_flow::BatterySequencer;

#[test]
fn test_battery_sequencer_order_and_output_names() {
    let stages = vec![
        "assess.v1.warmup".to_string(),
        "assess.v1.flick_static".to_string(),
        "assess.v1.micro_precision".to_string(),
        "assess.v1.head_level_peeks".to_string(),
        "assess.v1.smooth_track".to_string(),
        "assess.v1.strafe_track".to_string(),
        "assess.v1.multi_switch".to_string(),
    ];

    let output_dir = PathBuf::from("output/assess_v1");
    let mut seq = BatterySequencer::new("assess.v1", stages, output_dir.clone());

    assert_eq!(seq.total_stages(), 7);
    assert_eq!(seq.current_index(), 0);
    assert_eq!(seq.current_scenario_id(), Some("assess.v1.warmup"));
    assert_eq!(
        seq.current_output_path(),
        output_dir.join("0_assess.v1.warmup.aft")
    );

    // Advance to A1
    seq.record_stage_completed();
    assert!(seq.advance());
    assert_eq!(seq.current_index(), 1);
    assert_eq!(seq.current_scenario_id(), Some("assess.v1.flick_static"));
    assert_eq!(
        seq.current_output_path(),
        output_dir.join("1_assess.v1.flick_static.aft")
    );

    // Advance through remainder
    for i in 2..7 {
        seq.record_stage_completed();
        assert!(seq.advance());
        assert_eq!(seq.current_index(), i);
    }

    assert_eq!(seq.current_scenario_id(), Some("assess.v1.multi_switch"));
    assert_eq!(
        seq.current_output_path(),
        output_dir.join("6_assess.v1.multi_switch.aft")
    );

    seq.record_stage_completed();
    assert!(!seq.advance());
    assert!(seq.is_finished());
    assert_eq!(seq.completed_outputs().len(), 7);
}

#[test]
fn test_battery_sequencer_stop_mid_battery_keeps_saved_scenarios() {
    let stages = vec![
        "assess.v1.warmup".to_string(),
        "assess.v1.flick_static".to_string(),
        "assess.v1.micro_precision".to_string(),
    ];
    let output_dir = PathBuf::from("output/battery");
    let mut seq = BatterySequencer::new("assess.v1", stages, output_dir.clone());

    // Complete stage 0
    seq.record_stage_completed();
    assert!(seq.advance());
    assert_eq!(seq.current_index(), 1);

    // Complete stage 1
    seq.record_stage_completed();

    // User presses Esc during results or instructions of stage 2
    seq.stop();
    assert!(seq.is_finished());
    // Only stages 0 and 1 were saved and remain preserved
    assert_eq!(seq.completed_outputs().len(), 2);
    assert_eq!(
        seq.completed_outputs()[0],
        output_dir.join("0_assess.v1.warmup.aft")
    );
    assert_eq!(
        seq.completed_outputs()[1],
        output_dir.join("1_assess.v1.flick_static.aft")
    );
}

#[test]
fn an_interrupted_battery_resumes_at_the_first_unfinished_stage() {
    let stages: Vec<String> = ["w", "a", "b"].iter().map(|s| s.to_string()).collect();
    let dir = PathBuf::from("out");
    let seq = BatterySequencer::new("b", stages.clone(), dir.clone()).start_at(2);
    assert_eq!(seq.current_scenario_id(), Some("b"));
    assert_eq!(seq.current_output_path(), dir.join("2_b.aft"));
    let past_end = BatterySequencer::new("b", stages, dir).start_at(9);
    assert_eq!(past_end.current_index(), 2);
}
