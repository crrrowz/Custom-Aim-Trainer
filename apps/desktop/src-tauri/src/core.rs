//! The training loop: assessment, diagnosis, plan, daily drills, retest, verdict, coach.
//! The trainer process sits behind `TrainerRunner` so the whole loop can be tested without a game window.

use aim_analysis::{
    analyze_sessions, compute_engagement_metrics, session_detail, time_on_target,
    AnalysisThresholds, SessionDetail, SkillId, SkillProfile,
};
use aim_coach::diagnosis::{diagnose, maintenance_drill, plan_focus, Finding};
use aim_coach::drills::{build_custom, build_drill, CustomTargets};
use aim_coach::evaluation::{
    adapt, difference_in_differences, verdict, Adaptation, Staircase, Verdict,
};
use aim_coach::narrative::{drill_label, finding_text, skill_label, verdict_text, Locale};
use aim_coach::plan::{daily_session, PlanEvent, PlanState, SessionItem, BLOCK_DAYS};
use aim_llm::{
    write_report, AnthropicProvider, CoachReply, LlmProvider, OpenAiCompatProvider, Weakness,
};
use aim_scenarios::schema::Scenario;
use aim_store::Store;
use aim_telemetry::format::{quality_flags, AftSession};
use aim_telemetry::read_aft;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const KEY_SETTINGS: &str = "settings";
const KEY_PLAN: &str = "plan";
const KEY_LEVELS: &str = "drill_levels";
const KEY_BLOCK_RESULT: &str = "block_result";
const KEY_AI_KEYS: &str = "ai_keys";
const KEY_REPORTS: &str = "reports";
const MAX_REPORTS: usize = 60;
const COACH_CONFIG_FILE: &str = "coach_config.json";
/// Warm-up excluded: a plan needs real results from these seven tests.
const ASSESSMENT_MEASURED_TESTS: usize = 7;
const STOPPED_EARLY: &str = "stopped-early";
const FORMS: u32 = 5;
const BATTERY: &str = "battery.v1";
/// Tests in the battery, warm-up included.
const BATTERY_STAGES: usize = 8;
/// An interrupted assessment can be resumed for this many days; after that it starts over.
const RESUME_DAYS: u32 = 2;
/// Provisional minimum detectable change in score points, until test-retest data exists.
const PROVISIONAL_MDC: f64 = 8.0;
const GOAL_SCORE: f64 = 60.0;
const STAIRCASE_TARGET: f64 = 0.75;
const STAIRCASE_STEP: f64 = 0.02;
const START_DIFFICULTY: f64 = 0.3;
const TRACKING_UPDATES: usize = 20;
const TEST_MODE_DRILL_SECONDS: f64 = 20.0;
const OPENAI_URL: &str = "https://api.openai.com/v1";
const GEMINI_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";

pub type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn today() -> u32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    (secs / 86_400) as u32
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub onboarded: bool,
    pub dpi: u32,
    pub sens: f64,
    pub locale: String,
    pub windowed: bool,
    /// Short drills and no waiting for rest days, to walk through the whole loop in one sitting.
    pub test_mode: bool,
    /// "template", "claude", "openai", "gemini", "openrouter" or "custom".
    pub ai_provider: String,
    pub ai_base_url: String,
    pub ai_model: String,
    /// 0 turns hit and miss sounds off.
    pub sound_volume: u32,
    /// `rrggbb:length:thickness:gap:dot`, the trainer's crosshair format.
    pub crosshair: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarded: false,
            dpi: 800,
            sens: 0.35,
            locale: "ar".to_string(),
            windowed: false,
            test_mode: false,
            ai_provider: "template".to_string(),
            ai_base_url: String::new(),
            ai_model: String::new(),
            sound_volume: 60,
            crosshair: "00ff4c:0:4:0:1".to_string(),
        }
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

pub fn locale_for_ui_language(langid: u16) -> &'static str {
    if (langid & 0x03ff) == 0x01 {
        "ar"
    } else {
        "en"
    }
}

pub fn system_ui_language() -> u16 {
    #[cfg(windows)]
    {
        // SAFETY: GetUserDefaultUILanguage takes no arguments, writes no memory, and returns a 16-bit language ID.
        unsafe { GetUserDefaultUILanguage() }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

impl Settings {
    pub fn first_run(ui_langid: u16) -> Self {
        Self {
            locale: locale_for_ui_language(ui_langid).to_string(),
            ..Self::default()
        }
    }

    fn lang(&self) -> Locale {
        if self.locale == "en" {
            Locale::En
        } else {
            Locale::Ar
        }
    }

    fn cm_per_360(&self) -> f64 {
        // VALORANT: 0.07 degrees per count at sensitivity 1.
        let counts = 360.0 / (0.07 * self.sens);
        counts / self.dpi as f64 * 2.54
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssessmentRecord {
    pub day: u32,
    pub form: u32,
    pub dpi: u32,
    pub sens: f64,
    pub profile: SkillProfile,
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub at_ms: u64,
    /// Every test finished; only a complete assessment can start a plan.
    #[serde(default)]
    pub complete: bool,
    /// Recordings of each test, oldest stage first.
    #[serde(default)]
    pub files: Vec<String>,
    /// Version of each test's scenario. A test whose design changed is not comparable across versions.
    #[serde(default)]
    pub versions: BTreeMap<String, u32>,
}

impl AssessmentRecord {
    /// Scenario versions, read from the recordings for records saved before they were stored.
    fn scenario_versions(&self) -> BTreeMap<String, u32> {
        if !self.versions.is_empty() {
            return self.versions.clone();
        }
        self.files
            .iter()
            .filter_map(|f| read_session(Path::new(f)).ok())
            .map(|s| (s.header.scenario_id, s.header.scenario_version))
            .collect()
    }
}

/// Tests present in both assessments whose scenario version differs.
fn changed_tests(pre: &AssessmentRecord, post: &AssessmentRecord) -> Vec<String> {
    let after = post.scenario_versions();
    pre.scenario_versions()
        .into_iter()
        .filter(|(id, v)| after.get(id).is_some_and(|w| w != v))
        .map(|(id, _)| id)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRecord {
    pub state: PlanState,
    pub findings: Vec<Finding>,
    pub baseline: usize,
    /// How many of today's items are finished.
    pub item_index: usize,
    pub last_trained: u32,
    #[serde(default)]
    pub no_change_before: bool,
    #[serde(default = "default_maintenance")]
    pub maintenance: String,
    /// Calendar day of the last finished daily session.
    #[serde(default)]
    pub last_completed_day: Option<u32>,
}

fn default_maintenance() -> String {
    "consistency_grid".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticeRecord {
    pub day: u32,
    pub name: String,
    pub hit_rate: f64,
    pub difficulty: Option<f64>,
    /// Drill level before this session, to show how far it moved.
    #[serde(default)]
    pub level_before: Option<f64>,
    #[serde(default)]
    pub at_ms: u64,
    #[serde(default)]
    pub file: Option<String>,
    /// Target type, for custom practice.
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillChange {
    pub skill: SkillId,
    pub pre: f64,
    pub post: f64,
    pub delta: f64,
    pub verdict: Verdict,
    pub verdict_text: String,
    pub trained: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockResult {
    pub rows: Vec<SkillChange>,
    pub training_effect: Option<f64>,
    pub adaptation: Adaptation,
    pub conditions_changed: bool,
    #[serde(default)]
    pub settings_before: (u32, f64),
    #[serde(default)]
    pub settings_after: (u32, f64),
    /// Tests whose design changed between the two assessments, so their scores are not like for like.
    #[serde(default)]
    pub changed_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingView {
    pub id: String,
    pub headline: String,
    pub why: String,
    pub severity: f64,
    pub evidence: Vec<(String, f64)>,
    pub drills: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanView {
    pub state: PlanState,
    pub block_days: u8,
    pub today_items: Vec<SessionItem>,
    pub item_index: usize,
    pub retest_ready: bool,
    pub levels: HashMap<String, f64>,
    /// Today's session is finished; the next one opens tomorrow.
    pub today_done: bool,
    /// Why this block trains what it trains.
    pub focus: Vec<FindingView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub settings: Settings,
    pub today: u32,
    pub cm360: f64,
    pub trainer_found: bool,
    pub claude_key_found: bool,
    pub latest: Option<AssessmentRecord>,
    pub findings: Vec<FindingView>,
    pub history: Vec<AssessmentRecord>,
    pub plan: Option<PlanView>,
    pub block_result: Option<BlockResult>,
    pub practice: Vec<PracticeRecord>,
    pub sessions: Vec<SessionCard>,
    pub reports: Vec<CoachReport>,
    pub ai_keys_set: Vec<String>,
    pub coach_config_path: String,
    /// One-off message about the last action, such as a test that was stopped.
    pub notice: Option<String>,
    /// Tests already done in an interrupted assessment that the next start continues.
    pub resume: Option<ResumeView>,
    /// The latest assessment when it stopped part-way: its skills are a first estimate.
    pub estimate: Option<AssessmentRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResumeView {
    pub done: usize,
    pub total: usize,
}

/// One recording as a card on the analysis page. Built from the recording file itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionCard {
    pub path: String,
    pub at_ms: u64,
    pub assessment: bool,
    pub scenario: String,
    pub targets: usize,
    pub hits: usize,
    pub accuracy: Option<f64>,
    pub median_reaction_ms: Option<f64>,
    pub median_time_to_hit_ms: Option<f64>,
    pub time_on_target: Option<f64>,
    pub duration_s: f64,
    pub timing_suspect: bool,
    /// The player left this session early.
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachReport {
    pub at_ms: u64,
    pub scope: String,
    pub sessions: usize,
    pub source: String,
    pub note: Option<String>,
    pub reply: CoachReply,
    /// Language the report was written in.
    #[serde(default)]
    pub locale: String,
}

/// What the AI coach is allowed to see and how it should write. Lives in a JSON file the player can edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoachConfig {
    pub player_settings: bool,
    pub skills: bool,
    pub findings: bool,
    pub recent_sessions: bool,
    pub session_limit: usize,
    pub directions: bool,
    pub plan: bool,
    pub block_result: bool,
    pub extra_instructions: String,
}

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            player_settings: true,
            skills: true,
            findings: true,
            recent_sessions: true,
            session_limit: 30,
            directions: true,
            plan: true,
            block_result: true,
            extra_instructions: String::new(),
        }
    }
}

/// Recordings from the app's early versions store a file path where the id belongs.
fn scenario_key(scenario_id: &str) -> String {
    if !scenario_id.contains(['/', '\\']) {
        return scenario_id.to_string();
    }
    let stem = Path::new(&scenario_id.replace('\\', "/"))
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    if stem == "custom" {
        "custom.practice".to_string()
    } else {
        format!("drill.{stem}")
    }
}

/// A custom session is named after its target type, read from the scenario saved beside the recording.
fn custom_kind(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("custom.ron")).ok()?;
    let kind = if text.contains("ConcurrentCluster") {
        "multi"
    } else if text.contains("HeadLevelPeeks") {
        "peek"
    } else if text.contains("ReactiveStrafe") {
        "strafe"
    } else if text.contains("Tracking") {
        "track"
    } else {
        "flick"
    };
    Some(format!("custom.{kind}"))
}

fn run_started_ms(dir: &Path) -> u64 {
    dir.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.split('-').nth(1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

pub trait TrainerRunner {
    /// Runs the trainer to completion; recordings land where `args` says.
    fn run(&self, args: &[String]) -> Res<()>;
    fn available(&self) -> bool;
}

fn find_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|p| p.join("content").join("scenarios").is_dir())
        .map(Path::to_path_buf)
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "trainer.exe"
    } else {
        "trainer"
    }
}

pub struct ProcessRunner {
    trainer: Option<PathBuf>,
    root: Option<PathBuf>,
}

impl ProcessRunner {
    pub fn locate() -> Self {
        let exe = std::env::current_exe().ok();
        let exe_dir = exe.as_deref().and_then(Path::parent).map(Path::to_path_buf);
        let cwd = std::env::current_dir().ok();
        let root = std::env::var_os("AIMFORGE_ROOT")
            .map(PathBuf::from)
            .or_else(|| cwd.as_deref().and_then(find_root))
            .or_else(|| exe_dir.as_deref().and_then(find_root));

        let mut candidates = Vec::new();
        if let Some(p) = std::env::var_os("AIMFORGE_TRAINER") {
            candidates.push(PathBuf::from(p));
        }
        if let Some(d) = &exe_dir {
            candidates.push(d.join(exe_name()));
        }
        let mut targets = Vec::new();
        if let Some(t) = std::env::var_os("CARGO_TARGET_DIR") {
            targets.push(PathBuf::from(t));
        }
        if let Some(r) = &root {
            targets.push(r.join("target"));
        }
        for t in targets {
            candidates.push(t.join("play").join(exe_name()));
            candidates.push(t.join("release").join(exe_name()));
            candidates.push(t.join("debug").join(exe_name()));
        }
        Self {
            trainer: candidates.into_iter().find(|p| p.is_file()),
            root,
        }
    }
}

impl TrainerRunner for ProcessRunner {
    fn run(&self, args: &[String]) -> Res<()> {
        let trainer = self
            .trainer
            .as_ref()
            .ok_or("Trainer program not found. Build it with: cargo build --release -p trainer")?;
        let mut command = Command::new(trainer);
        command.args(args);
        // The trainer is a console program so developers see its log; started from the app it must
        // not open a console window beside the game.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        if let Some(root) = &self.root {
            command.current_dir(root);
        }
        let output = command.output().map_err(err)?;
        if !output.status.success() {
            return Err(format!(
                "Trainer stopped with an error: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    fn available(&self) -> bool {
        self.trainer.is_some()
    }
}

fn read_session(path: &Path) -> Res<AftSession> {
    let bytes = std::fs::read(path).map_err(err)?;
    read_aft(&bytes).map_err(|e| format!("Could not read {}: {e:?}", path.display()))
}

fn recordings_in(dir: &Path) -> Res<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(err)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "aft"))
        .collect();
    files.sort();
    Ok(files)
}

/// The stage number a battery recording was saved under: `3_name.aft` is stage 3.
fn stage_index(path: &Path) -> Option<usize> {
    path.file_name()?.to_str()?.split('_').next()?.parse().ok()
}

/// True when this run saved a recording in `dir`, new or replacing an earlier one.
fn written_since(dir: &Path, started: SystemTime) -> bool {
    recordings_in(dir).unwrap_or_default().iter().any(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t >= started)
    })
}

/// Stages finished in a row from the start, ignoring a stage the player left part-way.
fn finished_stages(dir: &Path) -> usize {
    let done: Vec<usize> = recordings_in(dir)
        .unwrap_or_default()
        .iter()
        .filter(|p| {
            read_session(p).is_ok_and(|s| s.header.quality_flags & quality_flags::ABORTED == 0)
        })
        .filter_map(|p| stage_index(p))
        .collect();
    (0..).take_while(|i| done.contains(i)).count()
}

/// Where an interrupted assessment picks up: its folder, its test form and the first stage left to do.
struct ResumePoint {
    dir: PathBuf,
    form: u32,
    stage: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Success of each trial in one drill, in order. Tracking has no trials, so its time on target is
/// spread over a fixed number of staircase updates.
fn trial_outcomes(session: &AftSession) -> Vec<bool> {
    if let Some(on_target) = time_on_target(session) {
        let wins = (on_target * TRACKING_UPDATES as f64).round() as usize;
        return (0..TRACKING_UPDATES).map(|i| i < wins).collect();
    }
    compute_engagement_metrics(session)
        .iter()
        .filter(|e| !e.is_home)
        .map(|e| e.shot_hit)
        .collect()
}

fn hit_rate(outcomes: &[bool]) -> f64 {
    if outcomes.is_empty() {
        return 0.0;
    }
    outcomes.iter().filter(|w| **w).count() as f64 / outcomes.len() as f64
}

pub struct Engine<R: TrainerRunner> {
    store: Store,
    runner: R,
    data_dir: PathBuf,
    run_counter: u32,
    notice: RefCell<Option<String>>,
    cards: RefCell<HashMap<String, SessionCard>>,
}

impl<R: TrainerRunner> Engine<R> {
    pub fn open(data_dir: &Path, runner: R) -> Res<Self> {
        std::fs::create_dir_all(data_dir).map_err(err)?;
        Ok(Self {
            store: Store::open(&data_dir.join("aimforge.db")).map_err(err)?,
            runner,
            data_dir: data_dir.to_path_buf(),
            run_counter: 0,
            notice: RefCell::new(None),
            cards: RefCell::new(HashMap::new()),
        })
    }

    pub fn settings(&self) -> Res<Settings> {
        Ok(self
            .store
            .get(KEY_SETTINGS)
            .map_err(err)?
            .unwrap_or_else(|| Settings::first_run(system_ui_language())))
    }

    pub fn save_settings(&self, settings: &Settings) -> Res<()> {
        self.store.set(KEY_SETTINGS, settings).map_err(err)
    }

    fn plan(&self) -> Res<Option<PlanRecord>> {
        self.store.get(KEY_PLAN).map_err(err)
    }

    fn save_plan(&self, plan: &PlanRecord) -> Res<()> {
        self.store.set(KEY_PLAN, plan).map_err(err)
    }

    fn levels(&self) -> Res<HashMap<String, f64>> {
        Ok(self.store.get(KEY_LEVELS).map_err(err)?.unwrap_or_default())
    }

    fn assessments(&self) -> Res<Vec<AssessmentRecord>> {
        self.store.assessments().map_err(err)
    }

    fn new_run_dir(&mut self, kind: &str) -> Res<PathBuf> {
        self.run_counter += 1;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let dir = self
            .data_dir
            .join("sessions")
            .join(format!("{kind}-{stamp}-{}", self.run_counter));
        std::fs::create_dir_all(&dir).map_err(err)?;
        Ok(dir)
    }

    fn base_args(&self, s: &Settings) -> Vec<String> {
        let mut args = vec![
            "--dpi".to_string(),
            s.dpi.to_string(),
            "--sens".to_string(),
            s.sens.to_string(),
            "--locale".to_string(),
            s.locale.clone(),
            "--sound".to_string(),
            s.sound_volume.to_string(),
            "--crosshair".to_string(),
            s.crosshair.clone(),
        ];
        if s.windowed {
            args.push("--windowed".to_string());
        }
        args
    }

    fn tell(&self, notice: &str) {
        *self.notice.borrow_mut() = Some(notice.to_string());
    }

    /// A player leaving a test early is not an error.
    fn absorb_stop<T>(&self, result: Res<T>) -> Res<Option<T>> {
        match result {
            Ok(v) => Ok(Some(v)),
            Err(e) if e == STOPPED_EARLY => {
                self.tell("stopped");
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// The latest assessment, if it stopped part-way recently enough to continue instead of restarting.
    fn resume_point(&self) -> Res<Option<ResumePoint>> {
        let Some(last) = self.assessments()?.pop() else {
            return Ok(None);
        };
        if last.complete || today().saturating_sub(last.day) > RESUME_DAYS {
            return Ok(None);
        }
        let Some(dir) = last.files.first().and_then(|f| Path::new(f).parent()) else {
            return Ok(None);
        };
        let stage = finished_stages(dir);
        Ok((stage > 0 && stage < BATTERY_STAGES).then(|| ResumePoint {
            dir: dir.to_path_buf(),
            form: last.form,
            stage,
        }))
    }

    fn record_assessment(&mut self) -> Res<AssessmentRecord> {
        let s = self.settings()?;
        let (dir, form, start) = match self.resume_point()? {
            Some(r) => (r.dir, r.form, r.stage),
            None => {
                let done = self.assessments()?.len() as u32;
                (self.new_run_dir("assessment")?, done % FORMS, 0)
            }
        };
        let started = SystemTime::now();
        let mut args = self.base_args(&s);
        args.extend([
            "--battery".to_string(),
            BATTERY.to_string(),
            "--output-dir".to_string(),
            dir.to_string_lossy().into_owned(),
            "--form".to_string(),
            form.to_string(),
        ]);
        if start > 0 {
            args.extend(["--battery-start".to_string(), start.to_string()]);
        }
        if s.test_mode {
            args.extend(["--rest".to_string(), "3".to_string()]);
        }
        self.runner.run(&args)?;

        let files = recordings_in(&dir)?;
        // A test left with "save and exit" replaces the earlier partial file of the same test, so count
        // what this run wrote rather than how many files there are.
        if start > 0 && !written_since(&dir, started) {
            return Err(STOPPED_EARLY.to_string());
        }
        let sessions = files
            .iter()
            .map(|p| read_session(p))
            .collect::<Res<Vec<_>>>()?;
        if sessions.is_empty() {
            return Err(STOPPED_EARLY.to_string());
        }
        let profile = analyze_sessions(&sessions, &AnalysisThresholds::default());
        let findings = diagnose(&profile);
        let measured = sessions
            .iter()
            .filter(|x| !x.header.scenario_id.contains("warmup"))
            .count();
        let record = AssessmentRecord {
            complete: measured >= ASSESSMENT_MEASURED_TESTS,
            day: today(),
            form,
            dpi: s.dpi,
            sens: s.sens,
            profile,
            findings,
            at_ms: now_ms(),
            files: files
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            versions: sessions
                .iter()
                .map(|x| (x.header.scenario_id.clone(), x.header.scenario_version))
                .collect(),
        };
        self.store.add_assessment(today(), &record).map_err(err)?;
        Ok(record)
    }

    /// First assessment: produces the proposed plan.
    pub fn run_assessment(&mut self) -> Res<()> {
        let result = self.record_assessment();
        let Some(record) = self.absorb_stop(result)? else {
            return Ok(());
        };
        let baseline = self.assessments()?.len() - 1;
        if !record.complete {
            self.tell("incomplete");
            return Ok(());
        }
        let focus = plan_focus(&record.profile, &record.findings);
        let maintenance = maintenance_drill(&record.profile, &focus).to_string();
        self.save_plan(&PlanRecord {
            state: PlanState::Draft,
            findings: focus,
            baseline,
            item_index: 0,
            last_trained: today(),
            no_change_before: false,
            maintenance,
            last_completed_day: None,
        })
    }

    pub fn accept_plan(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("Take an assessment first.")?;
        plan.state = plan.state.clone().apply(PlanEvent::Accept);
        plan.last_trained = today();
        self.save_plan(&plan)
    }

    fn today_items(plan: &PlanRecord) -> Vec<SessionItem> {
        match plan.state {
            PlanState::Active { day, .. } => daily_session(&plan.findings, &plan.maintenance, day),
            _ => Vec::new(),
        }
    }

    fn run_scenario_file(
        &mut self,
        scenario: &Scenario,
        seconds: f64,
        name: &str,
    ) -> Res<(Vec<bool>, String)> {
        let s = self.settings()?;
        let dir = self.new_run_dir("practice")?;
        let ron_path = dir.join(format!("{name}.ron"));
        std::fs::write(&ron_path, ron::to_string(scenario).map_err(err)?).map_err(err)?;
        let output = dir.join("session.aft");
        let mut args = self.base_args(&s);
        args.extend([
            "--scenario".to_string(),
            ron_path.to_string_lossy().into_owned(),
            "--output".to_string(),
            output.to_string_lossy().into_owned(),
            "--duration".to_string(),
            seconds.to_string(),
            "--seed".to_string(),
            (u64::from(today()) * 1000 + u64::from(self.run_counter)).to_string(),
        ]);
        self.runner.run(&args)?;
        if !output.is_file() {
            return Err(STOPPED_EARLY.to_string());
        }
        Ok((
            trial_outcomes(&read_session(&output)?),
            output.to_string_lossy().into_owned(),
        ))
    }

    /// Plays the next drill of today's session and moves the difficulty for that drill.
    pub fn run_next_item(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("No plan yet.")?;
        let s = self.settings()?;
        if !s.test_mode && plan.last_completed_day == Some(today()) {
            self.tell("done_today");
            return Ok(());
        }
        let items = Self::today_items(&plan);
        let item = items
            .get(plan.item_index)
            .cloned()
            .ok_or("Today's session is already finished.")?;
        let seconds = if s.test_mode {
            TEST_MODE_DRILL_SECONDS
        } else {
            f64::from(item.minutes) * 60.0
        };

        let mut levels = self.levels()?;
        let start = levels.get(&item.drill).copied().unwrap_or(START_DIFFICULTY);
        let mut scenario = build_drill(&item.drill, start, item.varied, seconds)
            .ok_or_else(|| format!("Unknown drill: {}", item.drill))?;
        if item.drill == "directional_flick" {
            scenario.rules.weak_octant = weak_octant(&plan.findings);
        }
        let played = self.run_scenario_file(&scenario, seconds, &item.drill);
        let Some((outcomes, file)) = self.absorb_stop(played)? else {
            return Ok(());
        };

        let mut staircase = Staircase::new(start, STAIRCASE_TARGET, STAIRCASE_STEP);
        outcomes.iter().for_each(|&w| staircase.update(w));
        levels.insert(item.drill.clone(), staircase.d);
        self.store.set(KEY_LEVELS, &levels).map_err(err)?;
        self.add_practice_record(PracticeRecord {
            day: today(),
            name: item.drill,
            hit_rate: hit_rate(&outcomes),
            difficulty: Some(staircase.d),
            level_before: Some(start),
            at_ms: now_ms(),
            file: Some(file),
            mode: None,
        })?;

        plan.item_index += 1;
        plan.last_trained = today();
        if plan.item_index >= items.len() {
            plan.state = plan
                .state
                .clone()
                .apply(PlanEvent::DayCompleted { today: today() });
            plan.item_index = 0;
            plan.last_completed_day = Some(today());
        }
        self.save_plan(&plan)
    }

    fn retest_ready(&self, plan: &PlanRecord) -> Res<bool> {
        Ok(match plan.state {
            PlanState::AwaitingRetest { due_day, .. } => {
                self.settings()?.test_mode || today() >= due_day
            }
            _ => false,
        })
    }

    pub fn run_retest(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("No plan yet.")?;
        if !self.retest_ready(&plan)? {
            return Err("The retest opens after a rest day.".to_string());
        }
        let attempt = self.record_assessment();
        let Some(post) = self.absorb_stop(attempt)? else {
            return Ok(());
        };
        if !post.complete {
            self.tell("incomplete");
            return Ok(());
        }
        let baseline = self
            .assessments()?
            .get(plan.baseline)
            .cloned()
            .ok_or("Baseline assessment is missing.")?;
        let s = self.settings()?;
        let mut result = compare(&baseline, &post, &plan, s.lang());
        result.changed_tests = changed_tests(&baseline, &post);

        plan.no_change_before = result.adaptation == Adaptation::SwitchDrill;
        plan.state = plan.state.clone().apply(PlanEvent::RetestFinished);
        self.store.set(KEY_BLOCK_RESULT, &result).map_err(err)?;
        self.save_plan(&plan)
    }

    /// Starts the next block from the newest assessment.
    pub fn next_block(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("No plan yet.")?;
        let assessments = self.assessments()?;
        let latest = assessments
            .iter()
            .rfind(|a| a.complete)
            .ok_or("Take an assessment first.")?;
        plan.state = plan.state.clone().apply(PlanEvent::NextBlock);
        plan.findings = plan_focus(&latest.profile, &latest.findings);
        plan.maintenance = maintenance_drill(&latest.profile, &plan.findings).to_string();
        plan.baseline = assessments.len() - 1;
        plan.item_index = 0;
        plan.last_trained = today();
        plan.last_completed_day = None;
        self.save_plan(&plan)
    }

    pub fn run_custom(&mut self, targets: &CustomTargets) -> Res<()> {
        let scenario = build_custom(targets)?;
        let played = self.run_scenario_file(&scenario, targets.duration_s, "custom");
        let Some((outcomes, file)) = self.absorb_stop(played)? else {
            return Ok(());
        };
        self.add_practice_record(PracticeRecord {
            day: today(),
            name: "custom".to_string(),
            hit_rate: hit_rate(&outcomes),
            difficulty: None,
            level_before: None,
            at_ms: now_ms(),
            file: Some(file),
            mode: Some(format!("{:?}", targets.mode).to_lowercase()),
        })
    }

    /// Plays one stock drill or assessment scenario outside the plan.
    pub fn run_scenario(&mut self, name: &str) -> Res<()> {
        let level = self
            .levels()?
            .get(name)
            .copied()
            .unwrap_or(START_DIFFICULTY);
        let seconds = 90.0;
        let scenario = build_drill(name, level, true, seconds)
            .ok_or_else(|| format!("Unknown drill: {name}"))?;
        let played = self.run_scenario_file(&scenario, seconds, name);
        let Some((outcomes, file)) = self.absorb_stop(played)? else {
            return Ok(());
        };
        self.add_practice_record(PracticeRecord {
            day: today(),
            name: name.to_string(),
            hit_rate: hit_rate(&outcomes),
            difficulty: None,
            level_before: None,
            at_ms: now_ms(),
            file: Some(file),
            mode: None,
        })
    }

    /// Full analysis of one recording. Only files inside the app's own data folder can be opened.
    pub fn session_detail(&self, path: &str) -> Res<SessionDetail> {
        let file = Path::new(path).canonicalize().map_err(err)?;
        let home = self.data_dir.canonicalize().map_err(err)?;
        if !file.starts_with(home) {
            return Err("That recording is outside the app's data folder.".to_string());
        }
        Ok(session_detail(&read_session(&file)?))
    }

    /// Every recording on disk, newest first. Works from the files, so history is never lost.
    pub fn list_sessions(&self) -> Res<Vec<SessionCard>> {
        let root = self.data_dir.join("sessions");
        let mut out = Vec::new();
        for dir in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let dir = dir.path();
            let name = dir
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            for file in recordings_in(&dir).unwrap_or_default() {
                let path = file.to_string_lossy().into_owned();
                if let Some(card) = self.cards.borrow().get(&path) {
                    out.push(card.clone());
                    continue;
                }
                let Ok(session) = read_session(&file) else {
                    continue;
                };
                let d = session_detail(&session);
                let card = SessionCard {
                    path: path.clone(),
                    at_ms: run_started_ms(&dir),
                    assessment: name.starts_with("assessment"),
                    scenario: custom_kind(&dir).unwrap_or_else(|| scenario_key(&d.scenario_id)),
                    targets: d.targets,
                    hits: d.hits,
                    accuracy: d.accuracy,
                    median_reaction_ms: d.median_reaction_ms,
                    median_time_to_hit_ms: d.median_time_to_hit_ms,
                    time_on_target: d.time_on_target,
                    duration_s: d.duration_s,
                    timing_suspect: d.timing_suspect,
                    partial: session.header.quality_flags & quality_flags::ABORTED != 0,
                };
                self.cards.borrow_mut().insert(path, card.clone());
                out.push(card);
            }
        }
        out.sort_by(|a, b| b.at_ms.cmp(&a.at_ms).then(a.path.cmp(&b.path)));
        Ok(out)
    }

    pub fn coach_config_path(&self) -> PathBuf {
        self.data_dir.join(COACH_CONFIG_FILE)
    }

    /// Reads the data file, creating it with the defaults the first time.
    pub fn coach_config(&self) -> Res<CoachConfig> {
        let path = self.coach_config_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            return serde_json::from_str(&text).map_err(|e| format!("{COACH_CONFIG_FILE}: {e}"));
        }
        let config = CoachConfig::default();
        self.save_coach_config(&config)?;
        Ok(config)
    }

    pub fn save_coach_config(&self, config: &CoachConfig) -> Res<()> {
        let text = serde_json::to_string_pretty(config).map_err(err)?;
        std::fs::write(self.coach_config_path(), text).map_err(err)
    }

    pub fn set_ai_key(&self, provider: &str, key: &str) -> Res<()> {
        let mut keys: HashMap<String, String> = self
            .store
            .get(KEY_AI_KEYS)
            .map_err(err)?
            .unwrap_or_default();
        if key.trim().is_empty() {
            keys.remove(provider);
        } else {
            keys.insert(provider.to_string(), key.trim().to_string());
        }
        self.store.set(KEY_AI_KEYS, &keys).map_err(err)
    }

    fn ai_key(&self, provider: &str) -> Res<Option<String>> {
        let keys: HashMap<String, String> = self
            .store
            .get(KEY_AI_KEYS)
            .map_err(err)?
            .unwrap_or_default();
        Ok(keys.get(provider).cloned())
    }

    fn provider(&self, s: &Settings) -> Res<Option<Box<dyn LlmProvider>>> {
        let compatible = |base: String, key: Option<String>| -> Option<Box<dyn LlmProvider>> {
            Some(Box::new(OpenAiCompatProvider {
                base_url: base,
                model: s.ai_model.clone(),
                api_key: key,
            }))
        };
        let required = |name: &str| -> Res<String> {
            self.ai_key(name)?
                .ok_or_else(|| "Add an API key for this source in Settings.".to_string())
        };
        if s.ai_provider != "template" && s.ai_model.trim().is_empty() {
            return Err("Choose a model name in Settings.".to_string());
        }
        Ok(match s.ai_provider.as_str() {
            "claude" => Some(Box::new(AnthropicProvider {
                api_key: required("claude")?,
                model: s.ai_model.clone(),
            })),
            "openai" => compatible(OPENAI_URL.to_string(), Some(required("openai")?)),
            "gemini" => compatible(GEMINI_URL.to_string(), Some(required("gemini")?)),
            "openrouter" => compatible(OPENROUTER_URL.to_string(), Some(required("openrouter")?)),
            "custom" => {
                if s.ai_base_url.trim().is_empty() {
                    return Err("Enter the server URL in Settings.".to_string());
                }
                compatible(s.ai_base_url.clone(), self.ai_key("custom")?)
            }
            _ => None,
        })
    }

    /// Sends one small request so the player knows the source works before relying on it.
    pub fn test_ai(&self) -> Res<String> {
        let s = self.settings()?;
        match self.provider(&s)? {
            None => Ok("template".to_string()),
            Some(p) => {
                p.complete("Reply with the single word OK.", "Connection test.")
                    .map_err(err)?;
                Ok("ok".to_string())
            }
        }
    }

    fn scoped(&self, scope: &str) -> Res<Vec<SessionCard>> {
        let cards = self.list_sessions()?;
        Ok(match scope {
            "latest" => cards.into_iter().take(1).collect(),
            "today" => {
                let day = 86_400_000;
                let newest = cards.first().map_or(0, |c| c.at_ms / day);
                cards
                    .into_iter()
                    .filter(|c| c.at_ms / day == newest)
                    .collect()
            }
            _ => cards,
        })
    }

    fn direction_stats(&self, cards: &[SessionCard]) -> Vec<Value> {
        let mut totals = [(0_u32, 0_u32, 0.0_f64, 0_u32); 8];
        for card in cards
            .iter()
            .filter(|c| !c.timing_suspect && c.targets > 0)
            .take(10)
        {
            let Ok(session) = read_session(Path::new(&card.path)) else {
                continue;
            };
            for f in session_detail(&session).flicks {
                let Some(o) = f.octant else { continue };
                let t = &mut totals[usize::from(o) % 8];
                t.0 += 1;
                t.1 += u32::from(f.hit);
                if let Some(ms) = f.time_to_hit_ms {
                    t.2 += ms;
                    t.3 += 1;
                }
            }
        }
        const NAMES: [&str; 8] = [
            "right",
            "up-right",
            "up",
            "up-left",
            "left",
            "down-left",
            "down",
            "down-right",
        ];
        totals
            .iter()
            .enumerate()
            .filter(|(_, t)| t.0 > 0)
            .map(|(i, t)| {
                json!({
                    "direction": NAMES[i],
                    "targets": t.0,
                    "accuracy_percent": (f64::from(t.1) * 100.0 / f64::from(t.0)).round(),
                    "average_time_ms": if t.3 > 0 { (t.2 / f64::from(t.3)).round() } else { 0.0 },
                })
            })
            .collect()
    }

    fn coach_input(&self, s: &Settings, cards: &[SessionCard], config: &CoachConfig) -> Res<Value> {
        let assessments = self.assessments()?;
        let latest = assessments.iter().rfind(|a| a.complete);
        let round = |v: f64| (v * 100.0).round() / 100.0;
        let mut data = serde_json::Map::new();
        data.insert("locale".to_string(), json!(s.locale));
        if config.player_settings {
            data.insert(
                "player".to_string(),
                json!({"cm360": round(s.cm_per_360()), "dpi": s.dpi, "sens": s.sens}),
            );
        }
        if config.skills {
            data.insert(
                "skills".to_string(),
                json!(latest.map(|a| a
                    .profile
                    .skills
                    .iter()
                    .map(|k| json!({
                        "skill": k.skill, "score": round(k.score), "raw": round(k.raw),
                    }))
                    .collect::<Vec<_>>())),
            );
        }
        if config.findings {
            data.insert("findings".to_string(), json!(latest.map(|a| a.findings.iter().map(|f| json!({
                "id": f.id,
                "name": finding_text(&f.id, Locale::En).0,
                "severity": round(f.severity),
                "evidence": f.evidence.iter().map(|(k, v)| (k.clone(), json!(round(*v)))).collect::<serde_json::Map<_, _>>(),
                "drills": f.drills,
            })).collect::<Vec<_>>())));
        }
        if config.recent_sessions {
            data.insert("sessions".to_string(), json!(cards.iter().take(config.session_limit).map(|c| json!({
                "scenario": c.scenario,
                "targets": c.targets,
                "hits": c.hits,
                "accuracy_percent": c.accuracy.map(|a| (a * 100.0).round()),
                "reaction_ms": if c.timing_suspect { None } else { c.median_reaction_ms.map(f64::round) },
                "time_to_hit_ms": if c.timing_suspect { None } else { c.median_time_to_hit_ms.map(f64::round) },
                "time_on_target_percent": c.time_on_target.map(|a| (a * 100.0).round()),
            })).collect::<Vec<_>>()));
        }
        if config.directions {
            data.insert("directions".to_string(), json!(self.direction_stats(cards)));
        }
        if config.plan {
            let plan = self.plan()?;
            data.insert("plan".to_string(), json!({"days": BLOCK_DAYS, "focus": plan.map(|p| p.findings.iter().take(2).map(|f| f.id.clone()).collect::<Vec<_>>())}));
        }
        if config.block_result {
            let result: Option<BlockResult> = self.store.get(KEY_BLOCK_RESULT).map_err(err)?;
            data.insert("block_result".to_string(), json!(result.map(|r| json!({
                "training_effect": r.training_effect.map(round),
                "changes": r.rows.iter().map(|c| json!({"skill": c.skill, "delta": round(c.delta), "trained": c.trained})).collect::<Vec<_>>(),
            }))));
        }
        Ok(Value::Object(data))
    }

    /// The report written from the numbers alone, used when no AI source is set or it fails.
    fn built_in_report(&self, s: &Settings, cards: &[SessionCard]) -> Res<CoachReply> {
        let l = s.lang();
        let ar = l == Locale::Ar;
        let assessments = self.assessments()?;
        let latest = assessments.iter().rfind(|a| a.complete);
        let timed: Vec<&SessionCard> = cards.iter().filter(|c| c.accuracy.is_some()).collect();
        let accuracy = (!timed.is_empty()).then(|| {
            timed.iter().filter_map(|c| c.accuracy).sum::<f64>() / timed.len() as f64 * 100.0
        });
        let trend = {
            let n = timed.len();
            (n >= 4).then(|| {
                let avg = |v: &[&SessionCard]| {
                    v.iter().filter_map(|c| c.accuracy).sum::<f64>() / v.len() as f64 * 100.0
                };
                avg(&timed[..n / 2]) - avg(&timed[n / 2..])
            })
        };
        let summary = match (accuracy, trend, ar) {
            (None, _, true) => "ماكو جلسات كافية بعد. سوِّ تقييم كامل حتى يكتب المدرب تقرير مفيد.".to_string(),
            (None, _, false) => "Not enough sessions yet. Finish a full assessment for a useful report.".to_string(),
            (Some(a), Some(t), true) => format!(
                "راجعت {} جلسة. دقتك بالمعدل {:.0}%، و{} بـ {:.0} نقطة بالجلسات الأخيرة.",
                cards.len(), a, if t >= 0.0 { "تحسنت" } else { "نزلت" }, t.abs()
            ),
            (Some(a), Some(t), false) => format!(
                "Reviewed {} sessions. Average accuracy is {:.0}%, {} {:.0} points in the recent half.",
                cards.len(), a, if t >= 0.0 { "up" } else { "down" }, t.abs()
            ),
            (Some(a), None, true) => format!("راجعت {} جلسة. دقتك بالمعدل {:.0}%.", cards.len(), a),
            (Some(a), None, false) => format!("Reviewed {} sessions. Average accuracy is {:.0}%.", cards.len(), a),
        };

        let mut strengths = Vec::new();
        let mut weaknesses = Vec::new();
        if let Some(a) = latest {
            let mut skills: Vec<_> = a.profile.skills.iter().collect();
            skills.sort_by(|x, y| y.score.total_cmp(&x.score));
            for k in skills.iter().filter(|k| k.score >= 55.0).take(3) {
                strengths.push(if ar {
                    format!("{}: درجتك {:.0}", skill_label(k.skill, l), k.score)
                } else {
                    format!("{}: score {:.0}", skill_label(k.skill, l), k.score)
                });
            }
            for f in plan_focus(&a.profile, &a.findings).iter().take(3) {
                let (title, why) = finding_text(&f.id, l);
                let fix = f.drills.first().map_or(String::new(), |d| {
                    if ar {
                        format!("تمرين: {}", drill_label(d, l))
                    } else {
                        format!("Drill: {}", drill_label(d, l))
                    }
                });
                weaknesses.push(Weakness {
                    title: format!("{title} ({})", skill_label(f.skill, l)),
                    detail: why.to_string(),
                    fix,
                });
            }
        }
        let mut by_drill: HashMap<&str, (f64, u32)> = HashMap::new();
        for c in &timed {
            let e = by_drill.entry(c.scenario.as_str()).or_insert((0.0, 0));
            e.0 += c.accuracy.unwrap_or(0.0);
            e.1 += 1;
        }
        let mut ranked: Vec<(&str, f64)> = by_drill
            .iter()
            .map(|(k, v)| (*k, v.0 / f64::from(v.1) * 100.0))
            .collect();
        ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
        if let (Some(worst), true) = (ranked.first(), ranked.len() > 1) {
            let name = drill_label(worst.0.rsplit('.').next().unwrap_or(worst.0), l);
            weaknesses.push(Weakness {
                title: if ar {
                    format!("أضعف تدريب: {name}")
                } else {
                    format!("Weakest session type: {name}")
                },
                detail: if ar {
                    format!("دقتك بيه {:.0}%.", worst.1)
                } else {
                    format!("Your accuracy there is {:.0}%.", worst.1)
                },
                fix: String::new(),
            });
        }
        let plan = self.plan()?;
        let mut next_steps: Vec<String> = plan
            .iter()
            .flat_map(|p| p.findings.iter().take(2))
            .filter_map(|f| f.drills.first())
            .map(|d| drill_label(d, l).to_string())
            .collect();
        if !latest.is_some_and(|a| a.complete) {
            next_steps.insert(
                0,
                if ar {
                    "سوِّ تقييم كامل (8 اختبارات).".to_string()
                } else {
                    "Complete a full assessment (8 tests).".to_string()
                },
            );
        }
        Ok(CoachReply {
            summary,
            strengths,
            weaknesses,
            next_steps,
        })
    }

    /// The coach reads the data for a scope and writes a saved report.
    pub fn generate_report(&mut self, scope: &str) -> Res<()> {
        let s = self.settings()?;
        let cards = self.scoped(scope)?;
        let config = self.coach_config()?;
        let attempt = self.provider(&s).and_then(|provider| match provider {
            None => Ok(None),
            Some(p) => {
                let input = self.coach_input(&s, &cards, &config)?;
                write_report(p.as_ref(), &input, &config.extra_instructions)
                    .map(Some)
                    .map_err(err)
            }
        });
        let (reply, source, note) = match attempt {
            Ok(Some(r)) => (r, "ai", None),
            Ok(None) => (self.built_in_report(&s, &cards)?, "built-in", None),
            Err(e) => (self.built_in_report(&s, &cards)?, "built-in", Some(e)),
        };
        let mut reports = self.reports()?;
        reports.insert(
            0,
            CoachReport {
                at_ms: now_ms(),
                scope: scope.to_string(),
                sessions: cards.len(),
                source: source.to_string(),
                note,
                reply,
                locale: s.locale.clone(),
            },
        );
        reports.truncate(MAX_REPORTS);
        self.store.set(KEY_REPORTS, &reports).map_err(err)
    }

    pub fn delete_report(&self, at_ms: u64) -> Res<()> {
        let mut reports = self.reports()?;
        reports.retain(|r| r.at_ms != at_ms);
        self.store.set(KEY_REPORTS, &reports).map_err(err)
    }

    /// Deletes several reports at once.
    pub fn delete_reports(&self, at_ms: &[u64]) -> Res<()> {
        let mut reports = self.reports()?;
        reports.retain(|r| !at_ms.contains(&r.at_ms));
        self.store.set(KEY_REPORTS, &reports).map_err(err)
    }

    /// Deletes several recordings. Every one is tried; the first failure is reported afterwards.
    pub fn delete_sessions(&self, paths: &[String]) -> Res<()> {
        let failures: Vec<String> = paths
            .iter()
            .filter_map(|p| self.delete_session(p).err())
            .collect();
        failures.into_iter().next().map_or(Ok(()), Err)
    }

    /// Saves a share card picture where the player can find it, and returns its path.
    pub fn save_share_card(&self, png: &[u8], name: &str) -> Res<String> {
        const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        if !png.starts_with(&PNG_SIGNATURE) {
            return Err("The share card is not a PNG picture.".to_string());
        }
        let safe: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(60)
            .collect();
        let dir = std::env::var_os("USERPROFILE")
            .map(|home| PathBuf::from(home).join("Pictures").join("AimForge"))
            .unwrap_or_else(|| self.data_dir.join("share"));
        std::fs::create_dir_all(&dir).map_err(err)?;
        let file = dir.join(format!(
            "{}.png",
            if safe.is_empty() { "aimforge" } else { &safe }
        ));
        std::fs::write(&file, png).map_err(err)?;
        Ok(file.to_string_lossy().into_owned())
    }

    /// Deletes one recording. Only files inside the app's own sessions folder can be removed.
    pub fn delete_session(&self, path: &str) -> Res<()> {
        let file = Path::new(path).canonicalize().map_err(err)?;
        let home = self.data_dir.join("sessions").canonicalize().map_err(err)?;
        if !file.starts_with(&home) || file.extension().is_none_or(|e| e != "aft") {
            return Err("That is not one of the app's recordings.".to_string());
        }
        std::fs::remove_file(&file).map_err(err)?;
        self.cards.borrow_mut().remove(path);
        if let Some(dir) = file.parent() {
            if recordings_in(dir).unwrap_or_default().is_empty() {
                std::fs::remove_dir_all(dir).map_err(err)?;
            }
        }
        Ok(())
    }

    /// Records a practice result and tells the player when it beats their best for that drill.
    fn add_practice_record(&self, record: PracticeRecord) -> Res<()> {
        let best = self
            .store
            .practice::<PracticeRecord>()
            .map_err(err)?
            .iter()
            .filter(|p| p.name == record.name && p.mode == record.mode)
            .map(|p| p.hit_rate)
            .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.max(v))));
        if best.is_some_and(|b| record.hit_rate > b) {
            self.tell("new_best");
        }
        self.store.add_practice(today(), &record).map_err(err)
    }

    fn reports(&self) -> Res<Vec<CoachReport>> {
        Ok(self
            .store
            .get(KEY_REPORTS)
            .map_err(err)?
            .unwrap_or_default())
    }

    pub fn view(&self) -> Res<View> {
        let settings = self.settings()?;
        let locale = settings.lang();
        let all = self.assessments()?;
        // An unfinished assessment newer than the last full one still says something: shown as an estimate.
        let estimate = all
            .last()
            .filter(|a| !a.complete && !a.profile.skills.is_empty())
            .cloned();
        let history: Vec<AssessmentRecord> = all.into_iter().filter(|a| a.complete).collect();
        let latest = history.last().cloned();
        let describe = |f: &Finding| {
            let (headline, why) = finding_text(&f.id, locale);
            FindingView {
                id: f.id.clone(),
                headline: headline.to_string(),
                why: why.to_string(),
                severity: f.severity,
                evidence: f.evidence.clone(),
                drills: f.drills.clone(),
            }
        };
        let findings = latest
            .iter()
            .chain(estimate.iter())
            .take(1)
            .flat_map(|a| a.findings.iter())
            .map(describe)
            .collect();
        let plan = match self.plan()? {
            Some(p) => Some(PlanView {
                today_items: Self::today_items(&p),
                item_index: p.item_index,
                retest_ready: self.retest_ready(&p)?,
                today_done: !settings.test_mode && p.last_completed_day == Some(today()),
                focus: p.findings.iter().map(describe).collect(),
                state: p.state,
                block_days: BLOCK_DAYS,
                levels: self.levels()?,
            }),
            None => None,
        };
        self.coach_config()?;
        Ok(View {
            cm360: settings.cm_per_360(),
            today: today(),
            trainer_found: self.runner.available(),
            claude_key_found: std::env::var_os("ANTHROPIC_API_KEY").is_some(),
            latest,
            findings,
            history,
            plan,
            block_result: self.store.get(KEY_BLOCK_RESULT).map_err(err)?,
            practice: self.store.practice().map_err(err)?,
            sessions: self.list_sessions()?,
            reports: self.reports()?,
            ai_keys_set: self
                .store
                .get::<HashMap<String, String>>(KEY_AI_KEYS)
                .map_err(err)?
                .unwrap_or_default()
                .into_keys()
                .collect(),
            coach_config_path: self.coach_config_path().to_string_lossy().into_owned(),
            notice: self.notice.borrow_mut().take(),
            estimate,
            resume: self.resume_point()?.map(|r| ResumeView {
                done: r.stage,
                total: BATTERY_STAGES,
            }),
            settings,
        })
    }
}

fn weak_octant(findings: &[Finding]) -> Option<u32> {
    findings
        .iter()
        .find(|f| f.id == "D9")?
        .evidence
        .iter()
        .find(|(k, _)| k == "S11.octant")
        .map(|(_, v)| *v as u32)
}

fn compare(
    pre: &AssessmentRecord,
    post: &AssessmentRecord,
    plan: &PlanRecord,
    locale: Locale,
) -> BlockResult {
    let trained_skills: Vec<SkillId> = plan.findings.iter().take(2).map(|f| f.skill).collect();
    let mut rows = Vec::new();
    for after in &post.profile.skills {
        let Some(before) = pre.profile.skill(after.skill) else {
            continue;
        };
        let delta = after.score - before.score;
        let v = verdict(delta, PROVISIONAL_MDC);
        rows.push(SkillChange {
            skill: after.skill,
            pre: before.score,
            post: after.score,
            delta,
            verdict: v,
            verdict_text: verdict_text(v, locale).to_string(),
            trained: trained_skills.contains(&after.skill),
        });
    }
    let deltas = |trained: bool| -> Vec<f64> {
        rows.iter()
            .filter(|r| r.trained == trained)
            .map(|r| r.delta)
            .collect()
    };
    let training_effect = difference_in_differences(&deltas(true), &deltas(false));

    // The headline decision follows the top focus skill.
    let focus = rows.iter().find(|r| r.trained);
    let adaptation = adapt(
        focus.map_or(Verdict::NoChange, |r| r.verdict),
        focus.is_some_and(|r| r.post >= GOAL_SCORE),
        plan.no_change_before,
    );
    BlockResult {
        rows,
        training_effect,
        adaptation,
        conditions_changed: pre.dpi != post.dpi || (pre.sens - post.sens).abs() > 1e-9,
        settings_before: (pre.dpi, pre.sens),
        settings_after: (post.dpi, post.sens),
        changed_tests: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aim_telemetry::format::{
        FrameStream, HitZone, ShotRecord, TargetKind, TargetParams, TargetRecord,
    };
    use aim_telemetry::{sample_session, write_aft};
    use std::cell::{Cell, RefCell};

    /// Stands in for the game: writes a recording of an overshooting player wherever the arguments say.
    struct FakeTrainer {
        overshoot: RefCell<f64>,
        calls: RefCell<Vec<Vec<String>>>,
        write_files: Cell<bool>,
        full_battery: Cell<bool>,
        /// Leave the first remaining stage part-way with "save and exit", recording nothing after it.
        quit_and_save: Cell<bool>,
    }

    fn value_after(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].clone())
    }

    thread_local! {
        /// Scenario version the fake trainer writes into its recordings.
        static SCENARIO_VERSION: Cell<u32> = const { Cell::new(1) };
    }

    /// 96 flicks at growing distances, each with a first movement of `gain` times the distance.
    fn flick_session(scenario: &str, gain: f64) -> AftSession {
        let mut s = sample_session();
        s.header.scenario_id = scenario.to_string();
        s.header.scenario_version = SCENARIO_VERSION.with(Cell::get);
        s.header.mouse.deg_per_count = 0.002;
        s.streams.keys.records.clear();
        s.streams.targets.records.clear();
        s.streams.shots.records.clear();
        s.streams.frames = FrameStream {
            t_us: Vec::new(),
            present_t_us: Vec::new(),
        };
        let (mut t_us, mut dx) = (Vec::new(), Vec::new());
        let mut crosshair = 0.0_f64;
        for i in 0..96_u64 {
            let start = 1_000_000 + i * 1_500_000;
            let distance = [8.0, 20.0, 40.0][(i % 3) as usize];
            let target = crosshair + distance;
            // Overshoot grows with distance, which is what rule D2 looks for.
            let moved = distance * (gain + (distance - 20.0) * 0.004);
            let steps = 100_i32;
            let per_step = (moved / 0.002 / f64::from(steps)).round() as i32;
            for k in 0..u64::from(steps as u32) {
                t_us.push(start + 200_000 + k * 1_000);
                dx.push(per_step);
            }
            let landed = crosshair + f64::from(per_step * steps) * 0.002;
            let back = ((target - landed) / 0.002 / 40.0).round() as i32;
            for k in 0..40_u64 {
                t_us.push(start + 500_000 + k * 1_000);
                dx.push(back);
            }
            crosshair = landed + f64::from(back * 40) * 0.002;
            s.streams.targets.records.push(TargetRecord {
                id: i as u32 + 1,
                spawn_t_us: start,
                despawn_t_us: start + 1_400_000,
                kind: TargetKind::Sphere,
                params: TargetParams {
                    radius_deg: 0.8,
                    initial_pos: [target.rem_euclid(360.0), 0.0, 15.0],
                    path_spline: None,
                },
            });
            s.streams.shots.records.push(ShotRecord {
                t_us: start + 700_000,
                hit_target_id: Some(i as u32 + 1),
                hitzone: HitZone::Head,
            });
        }
        s.streams.input.dy = vec![0; dx.len()];
        s.streams.input.t_us = t_us;
        s.streams.input.dx = dx;
        s
    }

    fn write(session: &AftSession, path: &Path) -> Res<()> {
        let bytes = write_aft(session).map_err(|e| format!("{e:?}"))?;
        std::fs::write(path, bytes).map_err(err)
    }

    impl TrainerRunner for FakeTrainer {
        fn run(&self, args: &[String]) -> Res<()> {
            self.calls.borrow_mut().push(args.to_vec());
            let gain = *self.overshoot.borrow();
            if let Some(dir) = value_after(args, "--output-dir") {
                if !self.write_files.get() {
                    return Ok(());
                }
                let start: usize =
                    value_after(args, "--battery-start").map_or(0, |v| v.parse().unwrap());
                if self.quit_and_save.get() {
                    let mut partial = flick_session("assess.v1.other_a", gain);
                    partial.header.quality_flags |= quality_flags::ABORTED;
                    return write(
                        &partial,
                        &Path::new(&dir).join(format!("{start}_other_a.aft")),
                    );
                }
                if start == 0 {
                    write(
                        &flick_session("assess.v1.warmup", gain),
                        &Path::new(&dir).join("0_warmup.aft"),
                    )?;
                    write(
                        &flick_session("assess.v1.flick_static", gain),
                        &Path::new(&dir).join("1_flick.aft"),
                    )?;
                }
                if self.full_battery.get() {
                    for (i, name) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
                        if i + 2 < start {
                            continue;
                        }
                        let mut extra = flick_session(&format!("assess.v1.other_{name}"), gain);
                        extra.streams.targets.records.truncate(1);
                        extra.streams.shots.records.truncate(1);
                        write(
                            &extra,
                            &Path::new(&dir).join(format!("{}_{name}.aft", i + 2)),
                        )?;
                    }
                }
            }
            if let Some(file) = value_after(args, "--output") {
                write(&flick_session("drill", gain), Path::new(&file))?;
            }
            Ok(())
        }

        fn available(&self) -> bool {
            true
        }
    }

    fn engine(name: &str, gain: f64) -> (Engine<FakeTrainer>, PathBuf) {
        let dir = std::env::temp_dir().join(format!("aimforge-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let e = Engine::open(
            &dir,
            FakeTrainer {
                overshoot: RefCell::new(gain),
                calls: RefCell::new(Vec::new()),
                write_files: Cell::new(true),
                full_battery: Cell::new(true),
                quit_and_save: Cell::new(false),
            },
        )
        .unwrap();
        (e, dir)
    }

    #[test]
    fn a_retest_on_a_newer_test_version_is_flagged() {
        let (mut e, dir) = engine("versions", 1.15);
        let mut s = e.settings().unwrap();
        s.test_mode = true;
        e.save_settings(&s).unwrap();
        e.run_assessment().unwrap();
        e.accept_plan().unwrap();
        for _ in 0..BLOCK_DAYS {
            let items = e.view().unwrap().plan.unwrap().today_items.len();
            for _ in 0..items {
                e.run_next_item().unwrap();
            }
        }
        // The flick test changed between the assessment and the retest.
        SCENARIO_VERSION.with(|v| v.set(2));
        e.run_retest().unwrap();
        let result = e.view().unwrap().block_result.unwrap();
        assert!(
            result
                .changed_tests
                .contains(&"assess.v1.flick_static".to_string()),
            "{:?}",
            result.changed_tests
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn whole_loop_from_assessment_to_verdict() {
        let (mut e, dir) = engine("loop", 1.15);
        let mut s = e.settings().unwrap();
        s.test_mode = true;
        e.save_settings(&s).unwrap();

        e.run_assessment().unwrap();
        let view = e.view().unwrap();
        assert!(
            view.findings.iter().any(|f| f.id == "D2"),
            "{:?}",
            view.findings
        );
        assert!(matches!(
            view.plan.as_ref().unwrap().state,
            PlanState::Draft
        ));

        e.accept_plan().unwrap();
        for _ in 0..BLOCK_DAYS {
            let items = e.view().unwrap().plan.unwrap().today_items.len();
            assert!(items > 0);
            for _ in 0..items {
                e.run_next_item().unwrap();
            }
        }
        let view = e.view().unwrap();
        let plan = view.plan.unwrap();
        assert!(matches!(plan.state, PlanState::AwaitingRetest { .. }));
        assert!(plan.retest_ready);
        assert!(!plan.levels.is_empty());

        // The player's overshoot is gone at the retest.
        *e.runner.overshoot.borrow_mut() = 0.97;
        e.run_retest().unwrap();
        let view = e.view().unwrap();
        let result = view.block_result.unwrap();
        let gain = result
            .rows
            .iter()
            .find(|r| r.skill == SkillId::S04GainControl)
            .unwrap();
        assert!(gain.trained);
        assert_eq!(gain.verdict, Verdict::Improved, "{gain:?}");
        assert!(!result.conditions_changed);
        assert!(result.changed_tests.is_empty());
        assert!(matches!(
            view.plan.unwrap().state,
            PlanState::Evaluated { .. }
        ));

        e.next_block().unwrap();
        assert!(matches!(
            e.view().unwrap().plan.unwrap().state,
            PlanState::Active { block: 2, .. }
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recordings_can_be_opened_for_analysis_but_only_our_own() {
        let (mut e, dir) = engine("detail", 1.1);
        e.run_assessment().unwrap();
        let view = e.view().unwrap();
        let file = view.latest.unwrap().files[0].clone();
        let detail = e.session_detail(&file).unwrap();
        assert_eq!(detail.targets, 96);
        let flick = &detail.flicks[0];
        assert!(!flick.path.is_empty());
        assert!(flick.onset_ms.unwrap() < flick.primary_end_ms.unwrap());
        assert!(flick.endpoint.is_some());
        assert!(e.session_detail("C:/Windows/win.ini").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn retest_waits_for_the_rest_day_outside_test_mode() {
        let (mut e, dir) = engine("rest", 1.15);
        e.run_assessment().unwrap();
        e.accept_plan().unwrap();
        for _ in 0..BLOCK_DAYS {
            let items = e.view().unwrap().plan.unwrap().today_items.len();
            for _ in 0..items {
                e.run_next_item().unwrap();
            }
        }
        assert!(e.run_retest().is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn built_in_report_is_saved_when_no_ai_is_selected() {
        let (mut e, dir) = engine("coach", 1.15);
        e.run_assessment().unwrap();
        e.generate_report("all").unwrap();
        let reports = e.view().unwrap().reports;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].source, "built-in");
        assert!(!reports[0].reply.summary.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreachable_ai_falls_back_and_says_why() {
        let (mut e, dir) = engine("offline", 1.15);
        let mut s = e.settings().unwrap();
        s.ai_provider = "custom".to_string();
        s.ai_base_url = "http://127.0.0.1:9".to_string();
        s.ai_model = "m".to_string();
        e.save_settings(&s).unwrap();
        e.run_assessment().unwrap();
        e.generate_report("latest").unwrap();
        let r = &e.view().unwrap().reports[0];
        assert_eq!(r.source, "built-in");
        assert!(r.note.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_key_is_explained() {
        let (e, dir) = engine("nokey", 1.0);
        let mut s = e.settings().unwrap();
        s.ai_provider = "openai".to_string();
        s.ai_model = "m".to_string();
        e.save_settings(&s).unwrap();
        assert!(e.test_ai().unwrap_err().contains("API key"));
        e.set_ai_key("openai", "abc").unwrap();
        assert_eq!(e.view().unwrap().ai_keys_set, vec!["openai".to_string()]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_data_file_controls_what_the_ai_sees() {
        let (mut e, dir) = engine("config", 1.15);
        e.run_assessment().unwrap();
        let mut c = e.coach_config().unwrap();
        assert!(e.coach_config_path().is_file());
        c.skills = false;
        c.directions = false;
        e.save_coach_config(&c).unwrap();
        let s = e.settings().unwrap();
        let cards = e.list_sessions().unwrap();
        let input = e
            .coach_input(&s, &cards, &e.coach_config().unwrap())
            .unwrap();
        assert!(input.get("skills").is_none());
        assert!(input.get("directions").is_none());
        assert!(input.get("sessions").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sessions_are_listed_from_the_recordings() {
        let (mut e, dir) = engine("cards", 1.0);
        e.run_assessment().unwrap();
        let cards = e.view().unwrap().sessions;
        assert_eq!(cards.len(), 8);
        assert!(cards.iter().all(|c| c.assessment));
        assert!(cards.iter().any(|c| c.scenario == "assess.v1.flick_static"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn one_training_day_per_calendar_day_outside_test_mode() {
        let (mut e, dir) = engine("oneday", 1.15);
        e.run_assessment().unwrap();
        e.accept_plan().unwrap();
        let items = e.view().unwrap().plan.unwrap().today_items.len();
        for _ in 0..items {
            e.run_next_item().unwrap();
        }
        let view = e.view().unwrap();
        assert!(view.plan.as_ref().unwrap().today_done);
        let calls = e.runner.calls.borrow().len();
        e.run_next_item().unwrap();
        assert_eq!(e.runner.calls.borrow().len(), calls, "no game is launched");
        assert_eq!(e.view().unwrap().notice.as_deref(), Some("done_today"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_partial_assessment_does_not_replace_results() {
        let (mut e, dir) = engine("keep", 1.15);
        e.run_assessment().unwrap();
        let before = e.view().unwrap().latest.unwrap().at_ms;
        e.runner.full_battery.set(false);
        e.run_assessment().unwrap();
        assert_eq!(e.view().unwrap().latest.unwrap().at_ms, before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_plan_explains_itself() {
        let (mut e, dir) = engine("why", 0.97);
        e.run_assessment().unwrap();
        assert!(!e.view().unwrap().plan.unwrap().focus.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_sessions_are_named_by_target_type() {
        let dir = std::env::temp_dir().join(format!("aimforge-kind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("custom.ron"), "(layout:ConcurrentCluster(x:1))").unwrap();
        assert_eq!(custom_kind(&dir).as_deref(), Some("custom.multi"));
        std::fs::write(
            dir.join("custom.ron"),
            "(layout:Tracking(trajectory:ReactiveStrafe()))",
        )
        .unwrap();
        assert_eq!(custom_kind(&dir).as_deref(), Some("custom.strafe"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_recording_can_be_deleted_but_only_a_recording() {
        let (mut e, dir) = engine("del", 1.0);
        e.run_assessment().unwrap();
        let cards = e.list_sessions().unwrap();
        e.delete_session(&cards[0].path).unwrap();
        assert_eq!(e.list_sessions().unwrap().len(), cards.len() - 1);
        assert!(e
            .delete_session(&dir.join("aimforge.db").to_string_lossy())
            .is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn beating_your_best_is_announced() {
        let (mut e, dir) = engine("best", 1.0);
        let targets = CustomTargets {
            mode: aim_coach::drills::CustomMode::Flick,
            radius_deg: 0.7,
            min_distance_deg: 10.0,
            max_distance_deg: 30.0,
            speed_deg_s: 20.0,
            time_limit_s: 2.0,
            concurrent: 3,
            duration_s: 45.0,
            area_h_deg: 35.0,
            area_v_deg: 12.0,
            reversal_s: 0.55,
        };
        e.store
            .add_practice(
                today(),
                &PracticeRecord {
                    day: today(),
                    name: "custom".to_string(),
                    hit_rate: 0.1,
                    difficulty: None,
                    level_before: None,
                    at_ms: 0,
                    file: None,
                    mode: Some("flick".to_string()),
                },
            )
            .unwrap();
        e.run_custom(&targets).unwrap();
        assert_eq!(e.view().unwrap().notice.as_deref(), Some("new_best"));
        e.run_custom(&targets).unwrap();
        assert_eq!(
            e.view().unwrap().notice,
            None,
            "matching a record is not a new one"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn game_settings_reach_the_trainer() {
        let (mut e, dir) = engine("args", 1.0);
        let mut s = e.settings().unwrap();
        s.sound_volume = 0;
        s.crosshair = "ff0000:6:2:3:1".to_string();
        e.save_settings(&s).unwrap();
        e.run_scenario("warmup").unwrap();
        let calls = e.runner.calls.borrow();
        let args = &calls[0];
        assert_eq!(value_after(args, "--sound").as_deref(), Some("0"));
        assert_eq!(
            value_after(args, "--crosshair").as_deref(),
            Some("ff0000:6:2:3:1")
        );
        drop(calls);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn old_settings_without_new_fields_still_load() {
        let old = r#"{"onboarded":true,"dpi":1600,"sens":0.2,"locale":"ar","windowed":false,"test_mode":false,"ai_provider":"template","ai_base_url":"","ai_model":""}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.dpi, 1600);
        assert_eq!(s.sound_volume, 60);
    }

    #[test]
    fn a_report_can_be_deleted() {
        let (mut e, dir) = engine("delrep", 1.0);
        e.run_assessment().unwrap();
        e.generate_report("all").unwrap();
        let at = e.view().unwrap().reports[0].at_ms;
        e.delete_report(at).unwrap();
        assert!(e.view().unwrap().reports.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn old_file_path_ids_become_proper_names() {
        assert_eq!(scenario_key("C:\\a\\b\\warmup.ron"), "drill.warmup");
        assert_eq!(scenario_key("C:\\a\\custom.ron"), "custom.practice");
        assert_eq!(scenario_key("assess.v1.warmup"), "assess.v1.warmup");
    }

    #[test]
    fn leaving_a_test_early_is_not_an_error() {
        let (mut e, dir) = engine("stop", 1.0);
        e.runner.write_files.set(false);
        e.run_assessment().unwrap();
        let view = e.view().unwrap();
        assert_eq!(view.notice.as_deref(), Some("stopped"));
        assert!(view.plan.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_interrupted_assessment_continues_from_where_it_stopped() {
        let (mut e, dir) = engine("resume", 1.0);
        e.runner.full_battery.set(false);
        e.run_assessment().unwrap();
        let partial = e.view().unwrap();
        let resume = partial.resume.unwrap();
        assert_eq!((resume.done, resume.total), (2, 8));
        assert!(partial.latest.is_none());
        assert!(
            partial.estimate.is_some(),
            "the tests done so far give a first estimate"
        );
        assert!(!partial.findings.is_empty() || partial.estimate.unwrap().findings.is_empty());

        e.runner.full_battery.set(true);
        e.run_assessment().unwrap();
        let last = e.runner.calls.borrow().last().unwrap().clone();
        assert_eq!(value_after(&last, "--battery-start").as_deref(), Some("2"));
        let view = e.view().unwrap();
        assert!(
            view.plan.is_some(),
            "the finished stages and the rest make a full assessment"
        );
        assert!(view.resume.is_none());
        assert!(
            view.estimate.is_none(),
            "a full assessment replaces the estimate"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn several_sessions_and_reports_are_deleted_together() {
        let (mut e, dir) = engine("bulk", 1.0);
        e.run_assessment().unwrap();
        let paths: Vec<String> = e
            .view()
            .unwrap()
            .sessions
            .iter()
            .take(3)
            .map(|c| c.path.clone())
            .collect();
        e.delete_sessions(&paths).unwrap();
        let left = e.view().unwrap().sessions;
        assert!(paths.iter().all(|p| left.iter().all(|c| &c.path != p)));
        assert_eq!(left.len(), 5);
        assert!(
            e.delete_sessions(&["C:/Windows/notepad.exe".to_string()])
                .is_err(),
            "outside files are refused"
        );

        for _ in 0..3 {
            e.generate_report("all").unwrap();
        }
        let reports = e.view().unwrap().reports;
        e.delete_reports(&[reports[0].at_ms, reports[1].at_ms])
            .unwrap();
        assert_eq!(e.view().unwrap().reports.len(), reports.len() - 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_share_card_must_be_a_png() {
        let (e, dir) = engine("share", 1.0);
        assert!(e.save_share_card(b"not a picture", "x").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_exit_in_a_resumed_test_is_kept_every_time() {
        let (mut e, dir) = engine("quit-save", 1.0);
        e.runner.full_battery.set(false);
        e.run_assessment().unwrap();
        e.runner.quit_and_save.set(true);
        for attempt in 0..2 {
            // The second attempt replaces the first one's partial file for the same test.
            e.run_assessment().unwrap();
            let view = e.view().unwrap();
            assert_eq!(
                view.notice.as_deref(),
                Some("incomplete"),
                "attempt {attempt}"
            );
            assert_eq!(
                view.resume.map(|r| r.done),
                Some(2),
                "the part-way test is redone"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn leaving_a_resumed_assessment_at_once_is_not_counted() {
        let (mut e, dir) = engine("resume-stop", 1.0);
        e.runner.full_battery.set(false);
        e.run_assessment().unwrap();
        e.runner.write_files.set(false);
        e.run_assessment().unwrap();
        let view = e.view().unwrap();
        assert_eq!(view.notice.as_deref(), Some("stopped"));
        assert_eq!(view.resume.map(|r| r.done), Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_partial_assessment_does_not_start_a_plan() {
        let (mut e, dir) = engine("partial", 1.0);
        e.runner.full_battery.set(false);
        e.run_assessment().unwrap();
        let view = e.view().unwrap();
        assert_eq!(view.notice.as_deref(), Some("incomplete"));
        assert!(view.plan.is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn custom_practice_passes_the_settings_to_the_trainer() {
        let (mut e, dir) = engine("custom", 1.0);
        let targets = CustomTargets {
            mode: aim_coach::drills::CustomMode::Flick,
            radius_deg: 0.7,
            min_distance_deg: 10.0,
            max_distance_deg: 30.0,
            speed_deg_s: 20.0,
            time_limit_s: 2.0,
            concurrent: 3,
            duration_s: 45.0,
            area_h_deg: 35.0,
            area_v_deg: 12.0,
            reversal_s: 0.55,
        };
        e.run_custom(&targets).unwrap();
        let calls = e.runner.calls.borrow();
        let ron_path = value_after(&calls[0], "--scenario").unwrap();
        let text = std::fs::read_to_string(ron_path).unwrap();
        assert!(text.contains("0.7"), "{text}");
        assert_eq!(e.view().unwrap().practice.len(), 1);
        drop(calls);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ui_language_maps_to_locale() {
        for langid in [0x0801, 0x0401, 0x3c01] {
            assert_eq!(
                locale_for_ui_language(langid),
                "ar",
                "langid {langid:#06x} should map to ar"
            );
        }
        for langid in [0x0409, 0x0809, 0x0407, 0x0429, 0] {
            assert_eq!(
                locale_for_ui_language(langid),
                "en",
                "langid {langid:#06x} should map to en"
            );
        }
    }

    #[test]
    fn first_run_settings_follow_ui_language() {
        let mut en = Settings::first_run(0x0409);
        assert_eq!(en.locale, "en");
        en.locale = Settings::default().locale;
        assert_eq!(en, Settings::default());

        let mut ar = Settings::first_run(0x0801);
        assert_eq!(ar.locale, "ar");
        ar.locale = Settings::default().locale;
        assert_eq!(ar, Settings::default());
    }

    #[test]
    fn fresh_engine_uses_windows_language() {
        let (e, dir) = engine("fresh_engine_lang", 1.0);
        assert_eq!(
            e.settings().unwrap().locale,
            locale_for_ui_language(system_ui_language())
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn saved_locale_wins_over_windows_language() {
        let (e, dir) = engine("saved_locale_wins", 1.0);
        let mut s = e.settings().unwrap();
        s.locale = "ar".to_string();
        e.save_settings(&s).unwrap();
        assert_eq!(e.settings().unwrap().locale, "ar");

        s.locale = "en".to_string();
        e.save_settings(&s).unwrap();
        assert_eq!(e.settings().unwrap().locale, "en");
        let _ = std::fs::remove_dir_all(dir);
    }
}
