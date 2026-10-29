use trainer::pause::PauseMenuNavigation;

#[test]
fn test_show_pause_menu_resets_navigation_state() {
    let mut nav = PauseMenuNavigation {
        focused_index: 3,
        is_confirming_discard: true,
        saving_timer: Some(1.5),
        session_duration_s: 100.0,
        scenario_name: "test_scenario".to_string(),
    };

    nav.focused_index = 0;
    nav.is_confirming_discard = false;
    nav.saving_timer = None;

    assert_eq!(nav.focused_index, 0);
    assert!(!nav.is_confirming_discard);
    assert_eq!(nav.saving_timer, None);
}
