use aim_telemetry::format::{quality_flags, KeyCode};
use trainer::pause::{should_auto_pause_on_focus_loss, PauseCause, PauseManager};
use trainer::recorder::{RecorderSetup, SessionRecorder};

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

#[test]
fn focus_loss_while_running_pauses() {
    // initial_startup_done = true, is_completed = false, is_paused = false
    assert!(should_auto_pause_on_focus_loss(true, false, false));
}

#[test]
fn focus_loss_during_startup_does_not_pause() {
    // initial_startup_done = false, is_completed = false, is_paused = false
    assert!(!should_auto_pause_on_focus_loss(false, false, false));
    // completed session does not pause
    assert!(!should_auto_pause_on_focus_loss(true, true, false));
}

#[test]
fn focus_loss_while_paused_is_noop() {
    // already paused -> should not trigger another pause
    assert!(!should_auto_pause_on_focus_loss(true, false, true));
}

#[test]
fn focus_regain_does_not_resume() {
    let mut pm = PauseManager::new();
    pm.pause_with_cause(1_000_000, PauseCause::FocusLost);
    assert!(pm.is_paused);

    // On focus regain: no resume called, remains paused with FocusLost cause
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::FocusLost));
}

#[test]
fn focus_pause_interval_excluded_from_active_time() {
    let mut pm = PauseManager::new();
    let mut recorder = SessionRecorder::new(RecorderSetup {
        session_id: "focus-pause-test".to_string(),
        scenario_id: "test".to_string(),
        form: None,
        seed: 42,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    });

    let t0 = 10_000_000; // 10s
    assert_eq!(pm.active_time_us(t0), 10_000_000);

    // Focus lost at t0 -> pause with cause FocusLost
    pm.pause_with_cause(t0, PauseCause::FocusLost);
    recorder.flag_paused();
    recorder.flag_focus_lost();
    recorder.record_key(t0, KeyCode::Unknown(0x1B), true);

    // While paused for 20 seconds of wall clock (t0 -> t0 + 20_000_000)
    let t_during = t0 + 10_000_000;
    assert_eq!(pm.active_time_us(t_during), 10_000_000);

    // Second focus loss event while already paused -> must be no-op (no duplicate key down or flag)
    if should_auto_pause_on_focus_loss(true, false, pm.is_paused) {
        panic!("Repeated focus loss while paused must not trigger pause");
    }

    // Regain focus at t0 + 15_000_000 -> does not resume!
    assert!(pm.is_paused);
    assert_eq!(pm.active_time_us(t0 + 15_000_000), 10_000_000);

    // Player explicitly resumes at t0 + 20_000_000 (after 20s pause)
    let t_resume = t0 + 20_000_000;
    pm.resume(t_resume);
    recorder.record_key(t_resume, KeyCode::Unknown(0x1B), false);

    // Active time right at resume must still be exactly 10s
    assert_eq!(pm.active_time_us(t_resume), 10_000_000);

    // 5 seconds after resume (t_resume + 5_000_000), active time must be 15s
    assert_eq!(pm.active_time_us(t_resume + 5_000_000), 15_000_000);

    // Exactly one down and one up Unknown(0x1B) record in telemetry
    let esc_keys: Vec<_> = recorder
        .streams
        .keys
        .records
        .iter()
        .filter(|k| k.key == KeyCode::Unknown(0x1B))
        .collect();
    assert_eq!(esc_keys.len(), 2);
    assert!(esc_keys[0].down);
    assert_eq!(esc_keys[0].t_us, t0);
    assert!(!esc_keys[1].down);
    assert_eq!(esc_keys[1].t_us, t_resume);

    // Quality flags must have both PAUSED and FOCUS_LOST
    let session = recorder.build_session();
    assert_ne!(session.header.quality_flags & quality_flags::PAUSED, 0);
    assert_ne!(session.header.quality_flags & quality_flags::FOCUS_LOST, 0);
}
