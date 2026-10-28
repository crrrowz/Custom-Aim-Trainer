use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

#[derive(Resource, Debug, Clone, Default)]
pub struct PauseManager {
    pub is_paused: bool,
    pub pause_start_us: Option<u64>,
    pub total_paused_us: u64,
    pub was_paused: bool,
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
            self.pause(now_us);
        }
    }

    pub fn pause(&mut self, now_us: u64) {
        if !self.is_paused {
            self.is_paused = true;
            self.pause_start_us = Some(now_us);
            self.was_paused = true;
        }
    }

    pub fn resume(&mut self, now_us: u64) {
        if self.is_paused {
            if let Some(start) = self.pause_start_us.take() {
                self.total_paused_us += now_us.saturating_sub(start);
            }
            self.is_paused = false;
        }
    }
}

#[derive(Component)]
pub struct PauseOverlayRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseButtonAction {
    Resume,
    QuitAndSave,
    QuitWithoutSaving,
}

pub fn spawn_pause_menu(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            PauseOverlayRoot,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: FontSize::Px(36.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            spawn_button(parent, "Resume", PauseButtonAction::Resume);
            spawn_button(parent, "Quit & Save", PauseButtonAction::QuitAndSave);
            spawn_button(
                parent,
                "Quit without saving",
                PauseButtonAction::QuitWithoutSaving,
            );
        });
}

#[allow(deprecated)]
fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: PauseButtonAction) {
    parent
        .spawn((
            Node {
                width: Val::Px(240.0),
                height: Val::Px(48.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            Interaction::default(),
            action,
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::WHITE),
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
