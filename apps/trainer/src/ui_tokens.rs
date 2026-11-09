use bevy::prelude::*;

// Color Palette
pub const COLOR_DIM_BACKGROUND: Color = Color::srgba(0.059, 0.067, 0.082, 0.70); // #0F1115 @ 70% dim
pub const COLOR_CARD_BACKGROUND: Color = Color::srgb(0.094, 0.106, 0.133); // #181B22
pub const COLOR_TEXT_PRIMARY: Color = Color::srgb(0.910, 0.918, 0.929); // #E8EAED
pub const COLOR_TEXT_MUTED: Color = Color::srgb(0.604, 0.627, 0.651); // #9AA0A6
pub const COLOR_ACCENT: Color = Color::srgb(0.239, 0.863, 0.592); // #3DDC97
pub const COLOR_DANGER: Color = Color::srgb(1.0, 0.361, 0.361); // #FF5C5C

pub const COLOR_BTN_IDLE: Color = Color::srgb(0.141, 0.157, 0.192); // #242831
pub const COLOR_BTN_HOVER: Color = Color::srgb(0.180, 0.200, 0.243); // #2E333E
pub const COLOR_BTN_PRESSED: Color = Color::srgb(0.071, 0.078, 0.098); // #121419

// Spacing Scale
pub const RADIUS_CARD: Val = Val::Px(8.0);
pub const RADIUS_BUTTON: Val = Val::Px(8.0);

pub const SPACE_XS: Val = Val::Px(4.0);
pub const SPACE_SM: Val = Val::Px(8.0);
pub const SPACE_MD: Val = Val::Px(12.0);
pub const SPACE_LG: Val = Val::Px(16.0);
pub const SPACE_XL: Val = Val::Px(24.0);

// Typography
pub const FONT_SIZE_COUNTDOWN: f32 = 72.0;
pub const FONT_SIZE_TITLE: f32 = 32.0;
pub const FONT_SIZE_STAT: f32 = 24.0;
pub const FONT_SIZE_SUBTITLE: f32 = 14.0;
pub const FONT_SIZE_BUTTON: f32 = 16.0;
pub const FONT_SIZE_FOOTER: f32 = 12.0;

// Element dimensions
pub const CARD_WIDTH: Val = Val::Px(420.0);
pub const CARD_WIDTH_LARGE: Val = Val::Px(500.0);
pub const BUTTON_WIDTH: Val = Val::Px(320.0);
pub const BUTTON_HEIGHT: Val = Val::Px(44.0);

// UI Strings. English unless the desktop app launches the game with --locale ar.
pub mod strings {
    use std::sync::atomic::{AtomicBool, Ordering};

    static ARABIC: AtomicBool = AtomicBool::new(false);

    pub fn set_arabic(on: bool) {
        ARABIC.store(on, Ordering::Relaxed);
    }

    pub fn is_arabic() -> bool {
        ARABIC.load(Ordering::Relaxed)
    }

    macro_rules! ui_text {
        ($($name:ident => $en:literal, $ar:literal;)*) => {
            $(pub fn $name() -> &'static str {
                if is_arabic() { $ar } else { $en }
            })*
            /// Every string as (English, Arabic), so a test can check none is missing.
            pub const ALL: &[(&str, &str)] = &[$(($en, $ar)),*];
        };
    }

    ui_text! {
        title_paused => "PAUSED", "متوقف";
        btn_resume => "Resume", "كمل";
        btn_quit_and_save => "Quit & Save", "خروج وحفظ";
        btn_quit_without_saving => "Quit without saving", "خروج بدون حفظ";
        footer_hint => "Esc  Resume    ↑↓  Navigate    Enter  Select", "Esc  كمل    ↑↓  تنقل    Enter  اختيار";
        confirm_discard_prompt => "Discard this session? Data will be lost.", "تحذف هذي الجلسة؟ البيانات راح تضيع.";
        btn_cancel => "Cancel", "إلغاء";
        btn_discard => "Discard", "احذف";
        saving => "Saving…", "يحفظ…";
        saved => "Saved", "انحفظ";
        notice_focus_lost => "Paused — the game window lost focus", "متوقف مؤقتاً — خرجت من نافذة اللعبة";
        instructions_start_hint => "Click or press Space to start", "انقر أو اضغط Space حتى تبدي";
        instructions_esc_hint => "Esc  Quit without saving", "Esc  خروج بدون حفظ";
        results_title => "SESSION RESULTS", "نتائج الجلسة";
        results_footer_hint => "Enter / Space / Esc  Close", "Enter / Space / Esc  إغلاق";
        results_next_hint => "Enter / Space  Next test   •   Esc  Stop", "Enter / Space  الاختبار الجاي   •   Esc  توقف";
        stat_accuracy => "ACCURACY", "الإصابة";
        stat_hits => "HITS", "الأهداف";
        stat_on_target => "ON TARGET", "على الهدف";
        stat_length => "LENGTH", "المدة";
        stat_reaction => "REACTION", "رد الفعل";
        stage_label => "ASSESSMENT • TEST", "التقييم • اختبار";
        hud_ready => "READY", "جاهز";
        hud_moving => "MOVING", "تتحرك";
        hud_overshoot => "OVERSHOOT", "تجاوزت";
        hud_short => "SHORT", "قبل الهدف";
        hud_on_target => "ON TARGET", "مضبوط";
    }
}
