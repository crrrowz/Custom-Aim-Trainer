use aim_telemetry::format::{quality_flags, KeyCode};
use trainer::pause::{
    apply_focus_change, should_auto_pause_on_focus_loss, FocusOutcome, PauseCause, PauseManager,
};
use trainer::recorder::{RecorderSetup, SessionRecorder};

fn create_test_recorder(session_id: &str) -> SessionRecorder {
    SessionRecorder::new(RecorderSetup {
        session_id: session_id.to_string(),
        scenario_id: "test".to_string(),
        form: None,
        seed: 42,
        dpi: 800,
        sens: 0.35,
        width: 1920,
        height: 1080,
        hz: 240,
    })
}

#[test]
fn the_click_that_refocuses_the_window_does_not_press_a_button() {
    let mut nav = trainer::pause::PauseMenuNavigation::default();
    assert!(nav.tick_click_guard(0.016));
    nav.on_focus_regained();
    assert!(!nav.tick_click_guard(0.016));
    assert!(nav.tick_click_guard(1.0));
}

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
    assert!(should_auto_pause_on_focus_loss(true, false, false));

    let mut pm = PauseManager::new();
    let mut rec = create_test_recorder("loss-running");
    let t0 = 5_000_000;

    let outcome = apply_focus_change(&mut pm, &mut rec, false, true, false, t0);
    assert_eq!(outcome, FocusOutcome::Paused);
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::FocusLost));

    // One Unknown(0x1B) down record, quality_flags has PAUSED | FOCUS_LOST
    let esc_keys: Vec<_> = rec
        .streams
        .keys
        .records
        .iter()
        .filter(|k| k.key == KeyCode::Unknown(0x1B))
        .collect();
    assert_eq!(esc_keys.len(), 1);
    assert!(esc_keys[0].down);
    assert_eq!(esc_keys[0].t_us, t0);

    let session = rec.build_session();
    assert_ne!(session.header.quality_flags & quality_flags::PAUSED, 0);
    assert_ne!(session.header.quality_flags & quality_flags::FOCUS_LOST, 0);
}

#[test]
fn focus_loss_during_startup_does_not_pause() {
    assert!(!should_auto_pause_on_focus_loss(false, false, false));
    assert!(!should_auto_pause_on_focus_loss(true, true, false));

    let mut pm = PauseManager::new();
    let mut rec = create_test_recorder("loss-startup");
    let t0 = 1_000_000;

    // During startup (startup_done = false)
    let outcome = apply_focus_change(&mut pm, &mut rec, false, false, false, t0);
    assert_eq!(outcome, FocusOutcome::NoChange);
    assert!(!pm.is_paused);
    assert_eq!(rec.streams.keys.records.len(), 0);

    // After completed (completed = true)
    let outcome_completed = apply_focus_change(&mut pm, &mut rec, false, true, true, t0 + 100_000);
    assert_eq!(outcome_completed, FocusOutcome::NoChange);
    assert!(!pm.is_paused);
    assert_eq!(rec.streams.keys.records.len(), 0);
}

#[test]
fn focus_loss_while_paused_is_noop() {
    assert!(!should_auto_pause_on_focus_loss(true, false, true));

    let mut pm = PauseManager::new();
    let mut rec = create_test_recorder("loss-paused");
    let t0 = 2_000_000;

    // First focus loss pauses
    let out1 = apply_focus_change(&mut pm, &mut rec, false, true, false, t0);
    assert_eq!(out1, FocusOutcome::Paused);
    assert_eq!(rec.streams.keys.records.len(), 1);

    // Second focus loss while already paused -> must be NoChange, no second key record
    let out2 = apply_focus_change(&mut pm, &mut rec, false, true, false, t0 + 1_000_000);
    assert_eq!(out2, FocusOutcome::NoChange);
    assert_eq!(rec.streams.keys.records.len(), 1);
}

#[test]
fn focus_regain_does_not_resume() {
    let mut pm = PauseManager::new();
    let mut rec = create_test_recorder("regain-no-resume");
    let t0 = 3_000_000;

    let out1 = apply_focus_change(&mut pm, &mut rec, false, true, false, t0);
    assert_eq!(out1, FocusOutcome::Paused);
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::FocusLost));

    // Focus regained: focused = true
    let out2 = apply_focus_change(&mut pm, &mut rec, true, true, false, t0 + 500_000);
    assert_eq!(out2, FocusOutcome::NoChange);

    // Must still be paused with cause FocusLost, no resume key (up) recorded
    assert!(pm.is_paused);
    assert_eq!(pm.pause_cause, Some(PauseCause::FocusLost));
    let esc_ups: Vec<_> = rec
        .streams
        .keys
        .records
        .iter()
        .filter(|k| k.key == KeyCode::Unknown(0x1B) && !k.down)
        .collect();
    assert_eq!(esc_ups.len(), 0);
}

#[test]
fn focus_pause_interval_excluded_from_active_time() {
    let mut pm = PauseManager::new();
    let mut recorder = create_test_recorder("focus-pause-interval");

    let t0 = 10_000_000; // 10s
    assert_eq!(pm.active_time_us(t0), 10_000_000);

    // Focus lost at t0 -> apply_focus_change triggers pause
    let out = apply_focus_change(&mut pm, &mut recorder, false, true, false, t0);
    assert_eq!(out, FocusOutcome::Paused);

    // While paused for 20 seconds of wall clock (t0 -> t0 + 20_000_000)
    let t_during = t0 + 10_000_000;
    assert_eq!(pm.active_time_us(t_during), 10_000_000);

    // Second focus loss event while already paused -> NoChange
    let out_repeat = apply_focus_change(&mut pm, &mut recorder, false, true, false, t_during);
    assert_eq!(out_repeat, FocusOutcome::NoChange);

    // Regain focus at t0 + 15_000_000 -> does not resume!
    let out_regain = apply_focus_change(&mut pm, &mut recorder, true, true, false, t0 + 15_000_000);
    assert_eq!(out_regain, FocusOutcome::NoChange);
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

#[test]
fn raw_input_worker_focus_state_transitions() {
    use trainer::raw_input::RawInputWorker;

    // Window initially focused
    RawInputWorker::set_window_focused(true);
    assert!(RawInputWorker::is_window_focused());

    // Focus lost -> flag flips to false (triggers THREAD_PRIORITY_NORMAL transition in worker thread)
    RawInputWorker::set_window_focused(false);
    assert!(!RawInputWorker::is_window_focused());

    // Focus regained -> flag flips to true (triggers THREAD_PRIORITY_TIME_CRITICAL transition)
    RawInputWorker::set_window_focused(true);
    assert!(RawInputWorker::is_window_focused());
}

#[test]
fn resume_stays_the_filled_primary_button() {
    use trainer::pause::{button_background, PauseButtonAction};
    use trainer::ui_tokens::COLOR_ACCENT;
    assert_eq!(
        button_background(PauseButtonAction::Resume, false),
        COLOR_ACCENT
    );
    assert_ne!(
        button_background(PauseButtonAction::QuitAndSave, false),
        COLOR_ACCENT
    );
}
