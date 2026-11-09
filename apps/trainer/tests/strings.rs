use trainer::session_flow::arabic_instructions;
use trainer::ui_tokens::strings;

#[test]
fn every_ui_string_has_both_languages() {
    for (en, ar) in strings::ALL {
        assert!(!en.is_empty() && !ar.is_empty(), "{en}");
    }
}

#[test]
fn every_drill_has_arabic_instructions() {
    for id in [
        "drill.warmup",
        "drill.deceleration_ladder",
        "drill.commit_flick",
        "drill.consistency_grid",
        "drill.micro_hold",
        "drill.confirm_click",
        "drill.timed_kill",
        "drill.head_level_hold",
        "drill.peek_line",
        "drill.directional_flick",
        "drill.strafe_track_reactive",
        "drill.smooth_track",
        "drill.reflex_dot",
        "drill.precision_first",
        "drill.speed_push",
        "drill.cs_drill",
        "custom.practice",
        "assess.v1.counter_strafe",
    ] {
        assert_ne!(arabic_instructions(id).0, "تمرين", "{id}");
    }
}
