use trainer::session_flow::{compute_session_results, SessionResultsInput};

#[test]
fn test_compute_session_results_median_rt_and_accuracy() {
    let input = SessionResultsInput {
        scenario_name: "assess.v1.flick_static".to_string(),
        hits: 24,
        misses: 6,
        reaction_times_s: vec![0.300, 0.250, 0.400, 0.280, 0.320],
        session_duration_s: 45.2,
        saved_path: "session-123.aft".to_string(),
    };

    let summary = compute_session_results(&input);
    assert_eq!(summary.hits, 24);
    assert_eq!(summary.misses, 6);
    assert_eq!(summary.total_shots, 30);
    // Accuracy = 24 / 30 = 80.0%
    assert!((summary.accuracy_pct - 80.0).abs() < 1e-4);
    // Sorted RTs: [0.250, 0.280, 0.300, 0.320, 0.400] -> median is 0.300s (300.0 ms)
    assert_eq!(summary.median_rt_ms, Some(300.0));
    assert_eq!(summary.saved_path, "session-123.aft");
}

#[test]
fn test_compute_session_results_with_pause_excludes_pause_time() {
    // Session had a 30s pause, but trial elapsed reaction times were already
    // computed against active time (e.g. 0.220s, 0.260s), so median RT reflects true motor RT
    let input = SessionResultsInput {
        scenario_name: "assess.v1.flick_static".to_string(),
        hits: 2,
        misses: 0,
        reaction_times_s: vec![0.220, 0.260],
        session_duration_s: 15.0, // active session length
        saved_path: "session-pause.aft".to_string(),
    };

    let summary = compute_session_results(&input);
    // Median of [0.220, 0.260] is (0.220 + 0.260) / 2 = 0.240s = 240.0 ms
    assert_eq!(summary.median_rt_ms, Some(240.0));
    assert!((summary.accuracy_pct - 100.0).abs() < 1e-4);
}
