use trainer::session_flow::{hud_line, HudNumbers};

fn numbers() -> HudNumbers {
    HudNumbers {
        remaining_s: 61.2,
        hits: 9,
        misses: 3,
        total: 48,
        tracking: false,
    }
}

#[test]
fn shows_time_left_progress_and_accuracy() {
    assert_eq!(hud_line(numbers()), "01:02    12 / 48    75%");
}

#[test]
fn tracking_shows_only_the_clock() {
    let mut n = numbers();
    n.tracking = true;
    assert_eq!(hud_line(n), "01:02");
}

#[test]
fn no_shots_yet_is_not_zero_percent() {
    let mut n = numbers();
    n.hits = 0;
    n.misses = 0;
    assert_eq!(hud_line(n), "01:02    0 / 48    100%");
}
