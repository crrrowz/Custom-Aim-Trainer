use trainer::pause::{PauseCause, PauseManager};

#[test]
fn pause_records_cause() {
    let mut pm = PauseManager::new();
    assert_eq!(pm.pause_cause, None);

    pm.pause_with_cause(1_000_000, PauseCause::Manual);
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::Manual));

    pm.resume(1_500_000);
    assert!(!pm.is_paused);
    assert_eq!(pm.pause_cause, None);

    pm.pause_with_cause(2_000_000, PauseCause::FocusLost);
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::FocusLost));
}
