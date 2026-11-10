use trainer::session_flow::{hud_cells, HudNumbers, HudTone};

fn numbers() -> HudNumbers {
    HudNumbers {
        remaining_s: 61.2,
        hits: 9,
        misses: 3,
        total: 48,
        tracking: false,
        on_target_pct: 62,
    }
}

#[test]
fn shows_time_left_progress_and_accuracy() {
    let cells = hud_cells(numbers());
    assert_eq!(cells[0].value, "01:02");
    assert_eq!(cells[1].value, "12 / 48");
    assert_eq!(cells[2].value, "75%");
    assert_eq!(cells[2].tone, HudTone::Good);
}

#[test]
fn tracking_shows_the_clock_and_time_on_target() {
    let mut n = numbers();
    n.tracking = true;
    let cells = hud_cells(n);
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[1].value, "62%");
    assert_eq!(cells[1].tone, HudTone::Warn);
}

#[test]
fn no_shots_yet_shows_no_percentage() {
    let mut n = numbers();
    n.hits = 0;
    n.misses = 0;
    let cells = hud_cells(n);
    assert_eq!(cells[1].value, "0 / 48");
    assert_eq!(cells[2].value, "—");
}

#[test]
fn the_last_ten_seconds_are_highlighted() {
    let mut n = numbers();
    n.remaining_s = 9.5;
    assert_eq!(hud_cells(n)[0].tone, HudTone::Warn);
}

#[test]
fn the_rest_hint_counts_down_and_names_the_keys() {
    let hint = trainer::session_flow::rest_hint(11.2);
    assert!(hint.contains("12s"), "{hint}");
    assert!(hint.contains("Enter") && hint.contains("Esc"));
}
