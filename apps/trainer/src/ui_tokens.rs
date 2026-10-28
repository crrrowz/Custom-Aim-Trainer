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
pub const FONT_SIZE_TITLE: f32 = 32.0;
pub const FONT_SIZE_SUBTITLE: f32 = 14.0;
pub const FONT_SIZE_BUTTON: f32 = 16.0;
pub const FONT_SIZE_FOOTER: f32 = 12.0;

// Element dimensions
pub const CARD_WIDTH: Val = Val::Px(420.0);
pub const BUTTON_WIDTH: Val = Val::Px(320.0);
pub const BUTTON_HEIGHT: Val = Val::Px(44.0);

// UI Strings (English default, centralized for future Arabic localization)
pub mod strings {
    pub const TITLE_PAUSED: &str = "PAUSED";
    pub const BTN_RESUME: &str = "Resume";
    pub const BTN_QUIT_AND_SAVE: &str = "Quit & Save";
    pub const BTN_QUIT_WITHOUT_SAVING: &str = "Quit without saving";
    pub const FOOTER_HINT: &str = "Esc  Resume    ↑↓  Navigate    Enter  Select";
    pub const CONFIRM_DISCARD_PROMPT: &str = "Discard this session? Data will be lost.";
    pub const BTN_CANCEL: &str = "Cancel";
    pub const BTN_DISCARD: &str = "Discard";
    pub const SAVING: &str = "Saving…";
    pub const SAVED: &str = "Saved";
}
