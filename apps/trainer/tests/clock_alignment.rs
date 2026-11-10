use trainer::clock::SessionClock;

const FREQ: i64 = 10_000_000;

#[test]
fn input_time_lines_up_after_the_session_clock_moves() {
    // The input thread started at QPC 0. The session began 4 s later, after the countdown.
    let clock = SessionClock::new(4 * FREQ, FREQ);
    assert_eq!(clock.from_input_us(4_500_000, 0), 500_000);
}

#[test]
fn nothing_shifts_before_the_session_clock_moves() {
    let clock = SessionClock::new(0, FREQ);
    assert_eq!(clock.from_input_us(1_234_567, 0), 1_234_567);
}

#[test]
fn events_from_before_the_session_clamp_to_zero() {
    let clock = SessionClock::new(4 * FREQ, FREQ);
    assert_eq!(clock.from_input_us(1_000_000, 0), 0);
}
