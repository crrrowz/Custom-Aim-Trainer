use crate::fonts::UiFonts;
use crate::ui_tokens::*;
use bevy::prelude::*;
use bevy::text::FontSource;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowState {
    Instructions,
    Countdown { seconds_remaining: u32 },
    Running,
    Results,
    ExitedWithoutSave,
}

#[derive(Component)]
pub struct FlowOverlayRoot;

#[derive(Component)]
pub struct CountdownDigitMarker;

#[derive(Debug, Clone)]
pub struct SessionFlowEngine {
    scenario_name: String,
    state: FlowState,
    countdown_timer_s: f32,
    clock_started: bool,
}

impl SessionFlowEngine {
    pub fn new(scenario_name: impl Into<String>) -> Self {
        Self {
            scenario_name: scenario_name.into(),
            state: FlowState::Instructions,
            countdown_timer_s: 0.0,
            clock_started: false,
        }
    }

    pub fn state(&self) -> FlowState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        self.state == FlowState::Running
    }

    pub fn clock_started(&self) -> bool {
        self.clock_started
    }

    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Handles trigger (Space or Click) from instructions screen.
    /// Returns true if transition occurred.
    pub fn handle_start_trigger(&mut self) -> bool {
        if self.state == FlowState::Instructions {
            self.state = FlowState::Countdown {
                seconds_remaining: 3,
            };
            self.countdown_timer_s = 0.0;
            true
        } else {
            false
        }
    }

    /// Ticks countdown by dt_s. Advances 3 -> 2 -> 1 -> Running.
    /// When reaching Running, sets `clock_started` to true.
    pub fn tick_countdown(&mut self, dt_s: f32) -> bool {
        if let FlowState::Countdown {
            ref mut seconds_remaining,
        } = self.state
        {
            self.countdown_timer_s += dt_s;
            if self.countdown_timer_s >= 1.0 {
                self.countdown_timer_s -= 1.0;
                if *seconds_remaining > 1 {
                    *seconds_remaining -= 1;
                } else {
                    self.state = FlowState::Running;
                    self.clock_started = true;
                    return true;
                }
            }
        }
        false
    }

    /// Esc handling:
    /// In Instructions or Countdown: exits immediately without saving.
    /// Returns true if exited without save.
    pub fn handle_esc_exit(&mut self) -> bool {
        match self.state {
            FlowState::Instructions | FlowState::Countdown { .. } => {
                self.state = FlowState::ExitedWithoutSave;
                true
            }
            _ => false,
        }
    }

    /// Focus loss handling:
    /// In Instructions or Countdown: ignored (no pause, no error).
    /// Returns true only if Running (pause should happen).
    pub fn handle_focus_loss(&mut self) -> bool {
        match self.state {
            FlowState::Instructions
            | FlowState::Countdown { .. }
            | FlowState::Results
            | FlowState::ExitedWithoutSave => false,
            FlowState::Running => true,
        }
    }

    /// Transition from Running to Results.
    pub fn finish_running(&mut self) {
        if self.state == FlowState::Running {
            self.state = FlowState::Results;
        }
    }

    /// Transition to next scenario in battery mode (back to Instructions).
    pub fn reset_for_next_scenario(&mut self, new_scenario: impl Into<String>) {
        self.scenario_name = new_scenario.into();
        self.state = FlowState::Instructions;
        self.countdown_timer_s = 0.0;
        self.clock_started = false;
    }
}

/// The name a player sees for a scenario. Never shows an id or a file path.
pub fn scenario_title(scenario_id: &str) -> &'static str {
    scenario_instructions(scenario_id).0
}

fn drill_instructions(scenario_id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let name = scenario_id.strip_prefix("drill.")?;
    Some(match name {
        "deceleration_ladder" => (
            "DECELERATION LADDER",
            "Slow your flick down so the first movement stops on the target.",
            "Targets shrink and get farther as you improve.",
        ),
        "commit_flick" => (
            "COMMIT FLICK",
            "Reach the target in one full movement.",
            "Short time limit: do not stop early and crawl.",
        ),
        "consistency_grid" => (
            "CONSISTENCY GRID",
            "Same distance every time, in every direction.",
            "Make each flick land the same way.",
        ),
        "micro_hold" => (
            "MICRO HOLD",
            "Small, close targets. Settle before you click.",
            "Keep the crosshair still on the target.",
        ),
        "confirm_click" => (
            "CONFIRM CLICK",
            "Arrive first, then click.",
            "Do not fire while the crosshair is still moving.",
        ),
        "timed_kill" => (
            "TIMED KILL",
            "Accurate is not enough: click before the window closes.",
            "The time per target shrinks as you improve.",
        ),
        "head_level_hold" => (
            "HEAD LEVEL HOLD",
            "Keep your crosshair at head height before the target appears.",
            "Do not drift up or down between peeks.",
        ),
        "peek_line" => (
            "PEEK LINE",
            "Targets emerge from the wall edges.",
            "Hold the line and click as they appear.",
        ),
        "directional_flick" => (
            "DIRECTIONAL FLICK",
            "Flicks in every direction.",
            "Pay attention to the direction that feels slowest.",
        ),
        "strafe_track_reactive" => (
            "REACTIVE STRAFE",
            "Stay on a target that keeps changing direction.",
            "React to each reversal as fast as you can.",
        ),
        "smooth_track" => (
            "SMOOTH TRACK",
            "Keep the crosshair on the moving target.",
            "Smooth, steady hand speed.",
        ),
        "reflex_dot" => (
            "REFLEX DOT",
            "Targets appear close to the centre.",
            "Start moving the moment you see one.",
        ),
        "precision_first" => (
            "PRECISION FIRST",
            "Small targets. Accuracy matters more than speed.",
            "Slow down until you stop missing.",
        ),
        "speed_push" => (
            "SPEED PUSH",
            "Large targets, tight time limit.",
            "Move faster than feels comfortable.",
        ),
        "cs_drill" => (
            "COUNTER-STRAFE",
            "Move with A and D, and stop before you click.",
            "Tap the opposite key to stop quickly.",
        ),
        "warmup" => (
            "WARM-UP",
            "Large targets at moderate distances.",
            "Get your hand moving. This one is not measured.",
        ),
        _ => return None,
    })
}

/// What the player sees at the top of the screen during a session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudNumbers {
    pub remaining_s: f64,
    pub hits: u32,
    pub misses: u32,
    pub total: u32,
    pub tracking: bool,
}

pub fn hud_line(n: HudNumbers) -> String {
    let left = n.remaining_s.ceil() as u32;
    let clock = format!("{:02}:{:02}", left / 60, left % 60);
    if n.tracking {
        return clock;
    }
    let shots = n.hits + n.misses;
    let accuracy = (n.hits * 100).checked_div(shots).unwrap_or(100);
    format!("{clock}    {shots} / {}    {accuracy}%", n.total)
}

type Instructions = (&'static str, &'static str, &'static str);

/// Scenario instructions in the player's language. Returns (title, line1, line2).
pub fn scenario_instructions(scenario_id: &str) -> Instructions {
    if strings::is_arabic() {
        arabic_instructions(scenario_id)
    } else {
        english_instructions(scenario_id)
    }
}

pub fn arabic_instructions(scenario_id: &str) -> Instructions {
    let key = scenario_id.rsplit('.').next().unwrap_or(scenario_id);
    match key {
        "warmup" => (
            "إحماء",
            "أهداف كبيرة على مسافات متوسطة.",
            "سخّن يدك. هذا الاختبار ما ينحسب.",
        ),
        "flick_static" => (
            "الفليك الثابت",
            "صيب كل هدف بفليك سريع ومضبوط.",
            "ارجع للمركز بين الأهداف.",
        ),
        "micro_precision" => (
            "الدقة الصغيرة",
            "تصحيحات صغيرة ودقيقة.",
            "أهداف صغيرة وقريبة تقيس تحكم الأصابع.",
        ),
        "head_level_peeks" => (
            "الظهور على مستوى الرأس",
            "خلي مؤشرك على مستوى الرأس وانقر من يطلع الهدف.",
            "لا تحرك مؤشرك لفوق أو جوه.",
        ),
        "smooth_track" => (
            "التتبع السلس",
            "خلي المؤشر على الهدف المتحرك.",
            "حركة يد ناعمة وثابتة.",
        ),
        "strafe_track" => (
            "تتبع strafe",
            "رد بسرعة على تغيير الاتجاه المفاجئ.",
            "ابقى على الهدف بأقل تأخير.",
        ),
        "multi_switch" => (
            "التبديل بين الأهداف",
            "اقتل الأهداف واحد ورا الثاني بسرعة.",
            "ابدي بالأقرب وانتقل بانسيابية.",
        ),
        "counter_strafe" => (
            "الإيقاف قبل الإطلاق",
            "تحرك بـ A و D وتوقف قبل ما تنقر.",
            "الطلقة أثناء الحركة ما تنحسب.",
        ),
        "deceleration_ladder" => (
            "سلّم التخفيف",
            "خفف حركتك حتى توقف الحركة الأولى على الهدف.",
            "إذا تجاوزت الهدف كثير ما تنحسب.",
        ),
        "commit_flick" => (
            "فليك كامل",
            "وصل للهدف بحركة وحدة كاملة.",
            "الوقت قصير: لا توقف قبله وتزحف.",
        ),
        "consistency_grid" => (
            "شبكة الثبات",
            "نفس المسافة بكل مرة وبكل اتجاه.",
            "خلي كل حركة تنتهي بنفس الشكل.",
        ),
        "micro_hold" => (
            "تثبيت دقيق",
            "أهداف صغيرة قريبة. اثبت قبل النقر.",
            "إذا المؤشر يتحرك وقت النقرة ما تنحسب.",
        ),
        "confirm_click" => (
            "نقر بعد الوصول",
            "وصل أول وبعدين انقر.",
            "النقرة والمؤشر يتحرك ما تنحسب.",
        ),
        "timed_kill" => (
            "قتل موقوت",
            "الدقة وحدها ما تكفي: انقر قبل ما ينتهي الوقت.",
            "الوقت يقصر كل ما تتحسن.",
        ),
        "head_level_hold" => (
            "ثبات مستوى الرأس",
            "خلي مؤشرك على مستوى الرأس قبل ما يطلع الهدف.",
            "لا تنزل ولا تصعد بين الأهداف.",
        ),
        "peek_line" => (
            "خط الظهور",
            "الأهداف تطلع من حواف الجدار.",
            "اثبت على الخط وانقر وقت تطلع.",
        ),
        "directional_flick" => (
            "فليك الاتجاهات",
            "أغلب الأهداف باتجاهك الضعيف.",
            "ركز على الاتجاه اللي تحسه أبطأ.",
        ),
        "strafe_track_reactive" => (
            "تتبع strafe",
            "ابقى على هدف يغير اتجاهه.",
            "رد بأسرع ما تكدر على كل تغيير.",
        ),
        "reflex_dot" => (
            "نقطة رد الفعل",
            "الأهداف تطلع قريب من المركز.",
            "تحرك أول ما تشوف الهدف.",
        ),
        "precision_first" => (
            "الدقة أولاً",
            "أهداف صغيرة. الدقة أهم من السرعة.",
            "بطّئ لحد ما توقف الأخطاء.",
        ),
        "speed_push" => (
            "دفع السرعة",
            "أهداف كبيرة ووقت قليل.",
            "تحرك أسرع من المريح.",
        ),
        "cs_drill" => (
            "تمرين الإيقاف",
            "تحرك بـ A و D وتوقف قبل النقر.",
            "اضغط الاتجاه المعاكس حتى توقف بسرعة.",
        ),
        "practice" => ("جلسة مخصصة", "إعدادات أهدافك.", "صيب أكثر ما تكدر."),
        _ => ("تمرين", "صيب الأهداف بسرعة ودقة.", "خلي مؤشرك ثابت."),
    }
}

fn english_instructions(scenario_id: &str) -> Instructions {
    if let Some(drill) = drill_instructions(scenario_id) {
        return drill;
    }
    if scenario_id.starts_with("custom.") {
        (
            "CUSTOM PRACTICE",
            "Your own target settings.",
            "Hit as many as you can.",
        )
    } else if scenario_id.contains("warmup") {
        (
            "WARM-UP",
            "Acclimate to sensor tracking and click rhythm.",
            "Large static targets at moderate distances.",
        )
    } else if scenario_id.contains("flick_static") {
        (
            "STATIC FLICK GRID",
            "Acquire each target with a single rapid, controlled flick.",
            "Return to center between peripheral acquisitions.",
        )
    } else if scenario_id.contains("micro_precision") {
        (
            "MICRO PRECISION",
            "Make tiny, accurate micro-corrections.",
            "Target cluster tests fine fingertip/wrist control.",
        )
    } else if scenario_id.contains("head_level_peeks") {
        (
            "HEAD-LEVEL PEEKS",
            "Pre-aim crosshair at head height and click as peekers emerge.",
            "Maintain crosshair placement with minimal vertical error.",
        )
    } else if scenario_id.contains("smooth_track") {
        (
            "SMOOTH TRACKING",
            "Keep your crosshair centered on the moving target continuously.",
            "Match constant angular velocity without stuttering.",
        )
    } else if scenario_id.contains("strafe_track") {
        (
            "STRAFE TRACKING",
            "React swiftly to sudden erratic directional reversals.",
            "Counter strafing target with minimal latency.",
        )
    } else if scenario_id.contains("counter_strafe") {
        (
            "COUNTER-STRAFE",
            "Move with A and D, and stop before you click.",
            "A shot fired while moving does not count.",
        )
    } else if scenario_id.contains("multi_switch") {
        (
            "MULTI-TARGET SWITCH",
            "Eliminate concurrent targets in rapid succession.",
            "Prioritize nearest target and transition smoothly.",
        )
    } else {
        (
            "ASSESSMENT TARGET",
            "Engage targets promptly with high accuracy.",
            "Maintain steady crosshair control throughout.",
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionResultsInput {
    pub scenario_name: String,
    pub hits: u32,
    pub misses: u32,
    pub reaction_times_s: Vec<f64>,
    pub session_duration_s: f64,
    pub saved_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionResultsSummary {
    pub scenario_name: String,
    pub hits: u32,
    pub misses: u32,
    pub total_shots: u32,
    pub accuracy_pct: f64,
    pub median_rt_ms: Option<f64>,
    pub session_duration_s: f64,
    pub saved_path: String,
}

/// Pure computation function for session results.
pub fn compute_session_results(input: &SessionResultsInput) -> SessionResultsSummary {
    let total_shots = input.hits + input.misses;
    let accuracy_pct = if total_shots > 0 {
        (input.hits as f64 / total_shots as f64) * 100.0
    } else {
        0.0
    };

    let median_rt_ms = if input.reaction_times_s.is_empty() {
        None
    } else {
        let mut sorted = input.reaction_times_s.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        let median_s = if sorted.len() % 2 == 1 {
            sorted[mid]
        } else {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        };
        Some(median_s * 1000.0)
    };

    SessionResultsSummary {
        scenario_name: input.scenario_name.clone(),
        hits: input.hits,
        misses: input.misses,
        total_shots,
        accuracy_pct,
        median_rt_ms,
        session_duration_s: input.session_duration_s,
        saved_path: input.saved_path.clone(),
    }
}

/// Battery sequencing state machine.
#[derive(Debug, Clone)]
pub struct BatterySequencer {
    pub battery_id: String,
    pub stages: Vec<String>,
    pub current_idx: usize,
    pub output_dir: PathBuf,
    pub completed_outputs: Vec<PathBuf>,
    pub is_stopped: bool,
}

impl BatterySequencer {
    pub fn new(battery_id: impl Into<String>, stages: Vec<String>, output_dir: PathBuf) -> Self {
        Self {
            battery_id: battery_id.into(),
            stages,
            current_idx: 0,
            output_dir,
            completed_outputs: Vec::new(),
            is_stopped: false,
        }
    }

    pub fn total_stages(&self) -> usize {
        self.stages.len()
    }

    pub fn current_index(&self) -> usize {
        self.current_idx
    }

    pub fn current_scenario_id(&self) -> Option<&str> {
        self.stages.get(self.current_idx).map(|s| s.as_str())
    }

    pub fn current_output_path(&self) -> PathBuf {
        let sc_id = self.current_scenario_id().unwrap_or("unknown");
        self.output_dir
            .join(format!("{}_{}.aft", self.current_idx, sc_id))
    }

    pub fn record_stage_completed(&mut self) {
        let path = self.current_output_path();
        if !self.completed_outputs.contains(&path) {
            self.completed_outputs.push(path);
        }
    }

    pub fn advance(&mut self) -> bool {
        if self.current_idx + 1 < self.stages.len() {
            self.current_idx += 1;
            true
        } else {
            self.is_stopped = true;
            false
        }
    }

    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    pub fn is_finished(&self) -> bool {
        self.is_stopped || self.current_idx >= self.stages.len()
    }

    pub fn completed_outputs(&self) -> &[PathBuf] {
        &self.completed_outputs
    }
}

/// Resolves battery manifest stages from disk or canonical assessment order.
pub fn resolve_battery_stages(battery_name: &str) -> Result<Vec<String>, String> {
    let manifest_path = if Path::new(battery_name).is_file() {
        PathBuf::from(battery_name)
    } else {
        let p1 = PathBuf::from(format!("content/batteries/{}.ron", battery_name));
        let p2 = PathBuf::from("content/batteries/battery.v1.ron");
        if p1.is_file() {
            p1
        } else if p2.is_file() {
            p2
        } else {
            return Ok(crate::VALID_ASSESS_SCENARIO_IDS
                .iter()
                .map(|s| s.to_string())
                .collect());
        }
    };

    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = ron::from_str::<aim_scenarios::BatteryManifest>(&content) {
            return Ok(manifest.stages.into_iter().map(|st| st.scenario).collect());
        }
    }

    Ok(crate::VALID_ASSESS_SCENARIO_IDS
        .iter()
        .map(|s| s.to_string())
        .collect())
}

// =========================================================================
// UI Spawning & Despawning Helpers
// =========================================================================

pub fn despawn_flow_overlay(
    commands: &mut Commands,
    q_overlay: &Query<Entity, With<FlowOverlayRoot>>,
) {
    for entity in q_overlay.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn spawn_instructions_overlay(
    commands: &mut Commands,
    ui_fonts: &UiFonts,
    scenario_id: &str,
    battery_info: Option<(usize, usize)>,
) {
    let (sc_title, line1, line2) = scenario_instructions(scenario_id);

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
            FlowOverlayRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: CARD_WIDTH_LARGE,
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
                ))
                .with_children(|card| {
                    if let Some((current, total)) = battery_info {
                        card.spawn((
                            Text::new(format!(
                                "{} {}/{}",
                                strings::stage_label(),
                                current + 1,
                                total
                            )),
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
                        Text::new(sc_title),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.semibold.clone()),
                            font_size: FontSize::Px(FONT_SIZE_TITLE),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));

                    // Instructions 1 & 2
                    card.spawn((
                        Text::new(format!("{}\n{}", line1, line2)),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_BUTTON),
                            ..default()
                        },
                        TextLayout::justify(Justify::Center),
                        TextColor(COLOR_TEXT_MUTED),
                    ));

                    // Action hint: Click or press Space to start
                    card.spawn((
                        Text::new(strings::instructions_start_hint()),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.semibold.clone()),
                            font_size: FontSize::Px(FONT_SIZE_BUTTON),
                            ..default()
                        },
                        TextColor(COLOR_ACCENT),
                    ));

                    // Footer hint: Esc Quit without saving
                    card.spawn((
                        Text::new(strings::instructions_esc_hint()),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_FOOTER),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_MUTED),
                    ));
                });
        });
}

pub fn spawn_countdown_overlay(
    commands: &mut Commands,
    ui_fonts: &UiFonts,
    seconds_remaining: u32,
) {
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
            FlowOverlayRoot,
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
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("GET READY"),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.semibold.clone()),
                            font_size: FontSize::Px(FONT_SIZE_TITLE),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_MUTED),
                    ));

                    // Large countdown digit
                    card.spawn((
                        Text::new(format!("{}", seconds_remaining)),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.semibold.clone()),
                            font_size: FontSize::Px(FONT_SIZE_COUNTDOWN),
                            ..default()
                        },
                        TextColor(COLOR_ACCENT),
                        CountdownDigitMarker,
                    ));

                    card.spawn((
                        Text::new(strings::instructions_esc_hint()),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_FOOTER),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_MUTED),
                    ));
                });
        });
}

fn spawn_stat(parent: &mut ChildSpawnerCommands, ui_fonts: &UiFonts, value: &str, label: &str) {
    parent
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: SPACE_XS,
                padding: UiRect::axes(SPACE_SM, SPACE_LG),
                border_radius: BorderRadius::all(RADIUS_BUTTON),
                ..default()
            },
            BackgroundColor(COLOR_BTN_IDLE),
        ))
        .with_children(|stat| {
            stat.spawn((
                Text::new(value),
                TextFont {
                    font: FontSource::Handle(ui_fonts.semibold.clone()),
                    font_size: FontSize::Px(FONT_SIZE_STAT),
                    ..default()
                },
                TextColor(COLOR_TEXT_PRIMARY),
            ));
            stat.spawn((
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(ui_fonts.regular.clone()),
                    font_size: FontSize::Px(FONT_SIZE_FOOTER),
                    ..default()
                },
                TextColor(COLOR_TEXT_MUTED),
            ));
        });
}

pub fn spawn_results_overlay(
    commands: &mut Commands,
    ui_fonts: &UiFonts,
    summary: &SessionResultsSummary,
    is_battery: bool,
    has_next: bool,
) {
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
            FlowOverlayRoot,
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: CARD_WIDTH_LARGE,
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
                ))
                .with_children(|card| {
                    // Header
                    card.spawn((
                        Text::new(strings::results_title()),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.semibold.clone()),
                            font_size: FontSize::Px(FONT_SIZE_TITLE),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_PRIMARY),
                    ));

                    // Scenario name
                    card.spawn((
                        Text::new(scenario_title(&summary.scenario_name)),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_SUBTITLE),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_MUTED),
                    ));

                    let rt_str = match summary.median_rt_ms {
                        Some(ms) => format!("{:.0} ms", ms),
                        None => "—".to_string(),
                    };
                    let accuracy = format!("{:.0}%", summary.accuracy_pct);
                    let hits = format!("{} / {}", summary.hits, summary.total_shots);

                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: SPACE_SM,
                        ..default()
                    })
                    .with_children(|row| {
                        spawn_stat(row, ui_fonts, &accuracy, strings::stat_accuracy());
                        spawn_stat(row, ui_fonts, &hits, strings::stat_hits());
                        spawn_stat(row, ui_fonts, &rt_str, strings::stat_reaction());
                    });

                    card.spawn((
                        Text::new(format!(
                            "{}  •  {:.0}s",
                            strings::saved(),
                            summary.session_duration_s
                        )),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_FOOTER),
                            ..default()
                        },
                        TextColor(COLOR_ACCENT),
                    ));

                    // Footer hint
                    let hint = if is_battery && has_next {
                        strings::results_next_hint()
                    } else {
                        strings::results_footer_hint()
                    };

                    card.spawn((
                        Text::new(hint),
                        TextFont {
                            font: FontSource::Handle(ui_fonts.regular.clone()),
                            font_size: FontSize::Px(FONT_SIZE_FOOTER),
                            ..default()
                        },
                        TextColor(COLOR_TEXT_MUTED),
                    ));
                });
        });
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn players_never_see_an_id() {
        for id in [
            "drill.warmup",
            "drill.confirm_click",
            "custom.practice",
            "assess.v1.flick_static",
        ] {
            let title = english_instructions(id).0;
            assert!(!title.contains('.') && !title.contains('\\'), "{title}");
        }
        assert_eq!(
            english_instructions("drill.confirm_click").0,
            "CONFIRM CLICK"
        );
    }
}
