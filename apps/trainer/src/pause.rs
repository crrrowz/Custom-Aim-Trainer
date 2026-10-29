use crate::fonts::UiFonts;
use crate::ui_tokens::*;
use bevy::prelude::*;
use bevy::text::FontSource;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCause {
    Manual,
    FocusLost,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct PauseManager {
    pub is_paused: bool,
    pub pause_start_us: Option<u64>,
    pub total_paused_us: u64,
    pub was_paused: bool,
    pub resume_qpc_us: u64,
    pub pause_cause: Option<PauseCause>,
}

impl PauseManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the effective unpaused session time in microseconds.
    #[inline]
    pub fn active_time_us(&self, clock_now_us: u64) -> u64 {
        if let Some(start) = self.pause_start_us {
            start.saturating_sub(self.total_paused_us)
        } else {
            clock_now_us.saturating_sub(self.total_paused_us)
        }
    }

    pub fn toggle_pause(&mut self, now_us: u64) {
        if self.is_paused {
            self.resume(now_us);
        } else {
            self.pause_with_cause(now_us, PauseCause::Manual);
        }
    }

    pub fn pause(&mut self, now_us: u64) {
        self.pause_with_cause(now_us, PauseCause::Manual);
    }

    pub fn pause_with_cause(&mut self, now_us: u64, cause: PauseCause) {
        if !self.is_paused {
            self.is_paused = true;
            self.pause_start_us = Some(now_us);
            self.was_paused = true;
            self.pause_cause = Some(cause);
        }
    }

    pub fn resume(&mut self, now_us: u64) {
        if self.is_paused {
            if let Some(start) = self.pause_start_us.take() {
                self.total_paused_us += now_us.saturating_sub(start);
            }
            self.is_paused = false;
            self.resume_qpc_us = now_us;
            self.pause_cause = None;
        }
    }
}

/// Pure rule governing auto-pause on focus loss:
/// Auto-pause only when startup is done, session is not completed, and not already paused.
#[inline]
pub fn should_auto_pause_on_focus_loss(
    initial_startup_done: bool,
    is_completed: bool,
    is_paused: bool,
) -> bool {
    initial_startup_done && !is_completed && !is_paused
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusOutcome {
    Paused,
    NoChange,
}

/// Pure helper for pause transition (recording key + setting flags + setting cause).
pub fn apply_pause(
    pm: &mut PauseManager,
    recorder: &mut crate::recorder::SessionRecorder,
    now_us: u64,
    cause: PauseCause,
) -> bool {
    if pm.is_paused {
        return false;
    }
    pm.pause_with_cause(now_us, cause);
    recorder.flag_paused();
    recorder.record_key(now_us, aim_telemetry::format::KeyCode::Unknown(0x1B), true);
    true
}

/// Pure per-event decision for window focus changes.
/// Manages pause transitions and recorder quality flags without depending on Bevy UI/world entities.
pub fn apply_focus_change(
    pm: &mut PauseManager,
    recorder: &mut crate::recorder::SessionRecorder,
    focused: bool,
    startup_done: bool,
    completed: bool,
    now_us: u64,
) -> FocusOutcome {
    if focused {
        FocusOutcome::NoChange
    } else {
        if startup_done {
            recorder.flag_focus_lost();
        }
        if should_auto_pause_on_focus_loss(startup_done, completed, pm.is_paused) {
            apply_pause(pm, recorder, now_us, PauseCause::FocusLost);
            FocusOutcome::Paused
        } else {
            FocusOutcome::NoChange
        }
    }
}

#[derive(Component)]
pub struct PauseOverlayRoot;

#[derive(Component)]
pub struct PauseCardRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseButtonAction {
    Resume,
    QuitAndSave,
    QuitWithoutSaving,
    ConfirmCancel,
    ConfirmDiscard,
}

#[derive(Resource, Debug, Clone)]
pub struct PauseMenuNavigation {
    pub focused_index: usize,
    pub is_confirming_discard: bool,
    pub saving_timer: Option<f32>,
    pub session_duration_s: f64,
    pub scenario_name: String,
}

impl Default for PauseMenuNavigation {
    fn default() -> Self {
        Self {
            focused_index: 0,
            is_confirming_discard: false,
            saving_timer: None,
            session_duration_s: 288.0,
            scenario_name: "Scenario".to_string(),
        }
    }
}

#[derive(Component)]
pub struct FocusableButton {
    pub index: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn execute_pause(
    commands: &mut Commands,
    pause_manager: &mut PauseManager,
    nav: &mut PauseMenuNavigation,
    recorder: &mut crate::recorder::SessionRecorder,
    q_cursor: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    ui_fonts: &UiFonts,
    scenario_name: &str,
    duration_s: f64,
    now_us: u64,
    cause: PauseCause,
) {
    if !apply_pause(pause_manager, recorder, now_us, cause) {
        return;
    }
    nav.focused_index = 0;
    nav.is_confirming_discard = false;
    nav.saving_timer = None;
    set_cursor_lock(q_cursor, false);
    let elapsed_s = crate::clock::SessionClock::us_to_s(pause_manager.active_time_us(now_us));
    spawn_pause_menu(
        commands,
        ui_fonts,
        scenario_name,
        elapsed_s,
        duration_s,
        nav,
        Some(cause),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn execute_resume(
    commands: &mut Commands,
    pause_manager: &mut PauseManager,
    recorder: &mut crate::recorder::SessionRecorder,
    layout_engine: &mut crate::layout_engine::LayoutEngineState,
    q_cursor: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    q_overlay: &Query<Entity, With<PauseOverlayRoot>>,
    now_us: u64,
) {
    if !pause_manager.is_paused {
        return;
    }
    pause_manager.resume(now_us);
    recorder.record_key(now_us, aim_telemetry::format::KeyCode::Unknown(0x1B), false);
    let resume_active_us = pause_manager.active_time_us(now_us);
    layout_engine.void_in_flight_trial_on_resume(resume_active_us);
    set_cursor_lock(q_cursor, true);
    despawn_pause_menu(commands, q_overlay);
}

pub fn spawn_pause_menu(
    commands: &mut Commands,
    ui_fonts: &UiFonts,
    scenario_name: &str,
    elapsed_s: f64,
    total_duration_s: f64,
    nav: &PauseMenuNavigation,
    cause: Option<PauseCause>,
) {
    let elapsed_mins = (elapsed_s as u32) / 60;
    let elapsed_secs = (elapsed_s as u32) % 60;
    let total_mins = (total_duration_s as u32) / 60;
    let total_secs = (total_duration_s as u32) % 60;

    let subtitle = format!(
        "{} • Elapsed {:02}:{:02} / {:02}:{:02}",
        scenario_name, elapsed_mins, elapsed_secs, total_mins, total_secs
    );

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(COLOR_DIM_BACKGROUND),
            PauseOverlayRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: CARD_WIDTH,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(SPACE_XL),
                        row_gap: SPACE_LG,
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(RADIUS_CARD),
                        ..default()
                    },
                    BackgroundColor(COLOR_CARD_BACKGROUND),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                    PauseCardRoot,
                ))
                .with_children(|card| {
                    if let Some(saving_t) = nav.saving_timer {
                        let msg = if saving_t > 0.3 {
                            strings::SAVED
                        } else {
                            strings::SAVING
                        };
                        card.spawn((
                            Text::new(msg),
                            TextFont {
                                font: FontSource::Handle(ui_fonts.semibold.clone()),
                                font_size: FontSize::Px(FONT_SIZE_TITLE),
                                ..default()
                            },
                            TextColor(COLOR_ACCENT),
                        ));
                        return;
                    }

                    if nav.is_confirming_discard {
                        card.spawn((
                            Text::new(strings::CONFIRM_DISCARD_PROMPT),
                            TextFont {
                                font: FontSource::Handle(ui_fonts.semibold.clone()),
                                font_size: FontSize::Px(FONT_SIZE_BUTTON),
                                ..default()
                            },
                            TextColor(COLOR_TEXT_PRIMARY),
                        ));

                        spawn_card_button(
                            card,
                            strings::BTN_CANCEL,
                            PauseButtonAction::ConfirmCancel,
                            0,
                            nav.focused_index == 0,
                            false,
                            ui_fonts,
                        );

                        spawn_card_button(
                            card,
                            strings::BTN_DISCARD,
                            PauseButtonAction::ConfirmDiscard,
                            1,
                            nav.focused_index == 1,
                            true,
                            ui_fonts,
                        );
                    } else {
                        // Notice on focus loss
                        if cause == Some(PauseCause::FocusLost) {
                            card.spawn((
                                Text::new(strings::NOTICE_FOCUS_LOST),
                                TextFont {
                                    font: FontSource::Handle(ui_fonts.regular.clone()),
                                    font_size: FontSize::Px(FONT_SIZE_SUBTITLE),
                                    ..default()
                                },
                                TextColor(COLOR_ACCENT),
                            ));
                        }

                        // Title
                        card.spawn((
                            Text::new(strings::TITLE_PAUSED),
                            TextFont {
                                font: FontSource::Handle(ui_fonts.semibold.clone()),
                                font_size: FontSize::Px(FONT_SIZE_TITLE),
                                ..default()
                            },
                            TextColor(COLOR_TEXT_PRIMARY),
                        ));

                        // Subtitle
                        card.spawn((
                            Text::new(subtitle),
                            TextFont {
                                font: FontSource::Handle(ui_fonts.regular.clone()),
                                font_size: FontSize::Px(FONT_SIZE_SUBTITLE),
                                ..default()
                            },
                            TextColor(COLOR_TEXT_MUTED),
                        ));

                        // Buttons
                        card.spawn((Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: SPACE_SM,
                            align_items: AlignItems::Center,
                            margin: UiRect::vertical(SPACE_SM),
                            ..default()
                        },))
                            .with_children(|button_col| {
                                spawn_card_button(
                                    button_col,
                                    strings::BTN_RESUME,
                                    PauseButtonAction::Resume,
                                    0,
                                    nav.focused_index == 0,
                                    false,
                                    ui_fonts,
                                );

                                spawn_card_button(
                                    button_col,
                                    strings::BTN_QUIT_AND_SAVE,
                                    PauseButtonAction::QuitAndSave,
                                    1,
                                    nav.focused_index == 1,
                                    false,
                                    ui_fonts,
                                );

                                spawn_card_button(
                                    button_col,
                                    strings::BTN_QUIT_WITHOUT_SAVING,
                                    PauseButtonAction::QuitWithoutSaving,
                                    2,
                                    nav.focused_index == 2,
                                    true,
                                    ui_fonts,
                                );
                            });

                        // Footer hint
                        card.spawn((
                            Text::new(strings::FOOTER_HINT),
                            TextFont {
                                font: FontSource::Handle(ui_fonts.regular.clone()),
                                font_size: FontSize::Px(FONT_SIZE_FOOTER),
                                ..default()
                            },
                            TextColor(COLOR_TEXT_MUTED),
                        ));
                    }
                });
        });
}

#[allow(deprecated)]
fn spawn_card_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: PauseButtonAction,
    index: usize,
    is_focused: bool,
    is_danger: bool,
    ui_fonts: &UiFonts,
) {
    let (bg_color, border_color, text_color) = if is_focused {
        (
            COLOR_BTN_HOVER,
            if is_danger {
                COLOR_DANGER
            } else {
                COLOR_ACCENT
            },
            COLOR_TEXT_PRIMARY,
        )
    } else if is_danger {
        (
            Color::srgba(1.0, 0.361, 0.361, 0.08),
            COLOR_DANGER,
            COLOR_DANGER,
        )
    } else if action == PauseButtonAction::Resume {
        (
            Color::srgba(0.239, 0.863, 0.592, 0.15),
            COLOR_ACCENT,
            COLOR_ACCENT,
        )
    } else {
        (
            COLOR_BTN_IDLE,
            Color::srgba(1.0, 1.0, 1.0, 0.05),
            COLOR_TEXT_PRIMARY,
        )
    };

    parent
        .spawn((
            Node {
                width: BUTTON_WIDTH,
                height: BUTTON_HEIGHT,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(
                    if is_focused || is_danger || action == PauseButtonAction::Resume {
                        1.5
                    } else {
                        1.0
                    },
                )),
                border_radius: BorderRadius::all(RADIUS_BUTTON),
                ..default()
            },
            BackgroundColor(bg_color),
            BorderColor::all(border_color),
            Interaction::default(),
            FocusableButton { index },
            action,
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: FontSource::Handle(if is_focused {
                    ui_fonts.semibold.clone()
                } else {
                    ui_fonts.regular.clone()
                }),
                font_size: FontSize::Px(FONT_SIZE_BUTTON),
                ..default()
            },
            TextColor(text_color),
        ));
}

pub fn despawn_pause_menu(
    commands: &mut Commands,
    q_overlay: &Query<Entity, With<PauseOverlayRoot>>,
) {
    for entity in q_overlay {
        commands.entity(entity).despawn();
    }
}

pub fn set_cursor_lock(
    q_cursor: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
    locked: bool,
) {
    if let Some(mut cursor) = q_cursor.iter_mut().next() {
        if locked {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}
