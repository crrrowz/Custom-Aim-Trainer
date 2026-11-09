use trainer::session_flow::{FlowState, SessionFlowEngine};

#[test]
fn test_session_flow_state_machine_transitions() {
    let mut engine = SessionFlowEngine::new("assess.v1.flick_static");
    assert_eq!(engine.state(), FlowState::Instructions);
    assert!(!engine.is_running());
    assert!(!engine.clock_started());

    // Focus loss during Instructions is ignored
    assert!(!engine.handle_focus_loss());
    assert_eq!(engine.state(), FlowState::Instructions);

    // Advance on Space or Click
    assert!(engine.handle_start_trigger());
    assert_eq!(
        engine.state(),
        FlowState::Countdown {
            seconds_remaining: 3
        }
    );
    assert!(!engine.is_running());
    assert!(!engine.clock_started());

    // Focus loss during Countdown is ignored
    assert!(!engine.handle_focus_loss());
    assert_eq!(
        engine.state(),
        FlowState::Countdown {
            seconds_remaining: 3
        }
    );

    // Tick countdown
    engine.tick_countdown(1.0);
    assert_eq!(
        engine.state(),
        FlowState::Countdown {
            seconds_remaining: 2
        }
    );
    engine.tick_countdown(1.0);
    assert_eq!(
        engine.state(),
        FlowState::Countdown {
            seconds_remaining: 1
        }
    );
    engine.tick_countdown(1.0);

    // Transitions to Running, clock starts
    assert_eq!(engine.state(), FlowState::Running);
    assert!(engine.is_running());
    assert!(engine.clock_started());

    // Focus loss during Running pauses
    assert!(engine.handle_focus_loss());
}

#[test]
fn test_session_flow_esc_during_pre_running_cancels_without_save() {
    let mut engine1 = SessionFlowEngine::new("assess.v1.flick_static");
    assert!(engine1.handle_esc_exit());
    assert_eq!(engine1.state(), FlowState::ExitedWithoutSave);

    let mut engine2 = SessionFlowEngine::new("assess.v1.flick_static");
    engine2.handle_start_trigger();
    assert_eq!(
        engine2.state(),
        FlowState::Countdown {
            seconds_remaining: 3
        }
    );
    assert!(engine2.handle_esc_exit());
    assert_eq!(engine2.state(), FlowState::ExitedWithoutSave);
}
