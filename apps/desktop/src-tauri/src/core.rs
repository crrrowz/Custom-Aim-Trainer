//! The training loop: assessment, diagnosis, plan, daily drills, retest, verdict, coach.
//! The trainer process sits behind `TrainerRunner` so the whole loop can be tested without a game window.

use aim_analysis::{
    analyze_sessions, compute_engagement_metrics, time_on_target, AnalysisThresholds, SkillId,
    SkillProfile,
};
use aim_coach::diagnosis::{diagnose, Finding};
use aim_coach::drills::{build_custom, build_drill, CustomTargets};
use aim_coach::evaluation::{
    adapt, difference_in_differences, verdict, Adaptation, Staircase, Verdict,
};
use aim_coach::narrative::{explain_finding, finding_text, verdict_text, Locale};
use aim_coach::plan::{daily_session, PlanEvent, PlanState, SessionItem, BLOCK_DAYS};
use aim_llm::{narrate, AnthropicProvider, CoachReply, LlmProvider, OpenAiCompatProvider};
use aim_scenarios::schema::Scenario;
use aim_store::Store;
use aim_telemetry::format::AftSession;
use aim_telemetry::read_aft;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const KEY_SETTINGS: &str = "settings";
const KEY_PLAN: &str = "plan";
const KEY_LEVELS: &str = "drill_levels";
const KEY_BLOCK_RESULT: &str = "block_result";
const FORMS: u32 = 5;
const BATTERY: &str = "battery.v1";
const MAINTENANCE_DRILL: &str = "smooth_track";
/// Provisional minimum detectable change in score points, until test-retest data exists.
const PROVISIONAL_MDC: f64 = 8.0;
const GOAL_SCORE: f64 = 60.0;
const STAIRCASE_TARGET: f64 = 0.75;
const STAIRCASE_STEP: f64 = 0.02;
const START_DIFFICULTY: f64 = 0.3;
const TRACKING_UPDATES: usize = 20;
const TEST_MODE_DRILL_SECONDS: f64 = 20.0;
const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-5-5";

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
pub struct Settings {
    pub onboarded: bool,
    pub dpi: u32,
    pub sens: f64,
    pub locale: String,
    pub windowed: bool,
    /// Short drills and no waiting for rest days, to walk through the whole loop in one sitting.
    pub test_mode: bool,
    /// "template", "local" or "claude".
    pub ai_provider: String,
    pub ai_base_url: String,
    pub ai_model: String,
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
            ai_base_url: "http://localhost:11434/v1".to_string(),
            ai_model: "llama3.2".to_string(),
        }
    }
}

impl Settings {
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PracticeRecord {
    pub day: u32,
    pub name: String,
    pub hit_rate: f64,
    pub difficulty: Option<f64>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatAnswer {
    pub reply: CoachReply,
    /// "ai" or "template".
    pub source: String,
    /// Why the fixed text was used, when an AI provider was selected.
    pub note: Option<String>,
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

fn read_sessions_in(dir: &Path) -> Res<Vec<AftSession>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(err)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "aft"))
        .collect();
    files.sort();
    files.iter().map(|p| read_session(p)).collect()
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
}

impl<R: TrainerRunner> Engine<R> {
    pub fn open(data_dir: &Path, runner: R) -> Res<Self> {
        std::fs::create_dir_all(data_dir).map_err(err)?;
        Ok(Self {
            store: Store::open(&data_dir.join("aimforge.db")).map_err(err)?,
            runner,
            data_dir: data_dir.to_path_buf(),
            run_counter: 0,
        })
    }

    pub fn settings(&self) -> Res<Settings> {
        Ok(self
            .store
            .get(KEY_SETTINGS)
            .map_err(err)?
            .unwrap_or_default())
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
        ];
        if s.windowed {
            args.push("--windowed".to_string());
        }
        args
    }

    fn record_assessment(&mut self) -> Res<AssessmentRecord> {
        let s = self.settings()?;
        let done = self.assessments()?.len() as u32;
        let form = done % FORMS;
        let dir = self.new_run_dir("assessment")?;
        let mut args = self.base_args(&s);
        args.extend([
            "--battery".to_string(),
            BATTERY.to_string(),
            "--output-dir".to_string(),
            dir.to_string_lossy().into_owned(),
            "--form".to_string(),
            form.to_string(),
        ]);
        self.runner.run(&args)?;

        let sessions = read_sessions_in(&dir)?;
        if sessions.is_empty() {
            return Err("The assessment was stopped before anything was recorded.".to_string());
        }
        let profile = analyze_sessions(&sessions, &AnalysisThresholds::default());
        let findings = diagnose(&profile);
        let record = AssessmentRecord {
            day: today(),
            form,
            dpi: s.dpi,
            sens: s.sens,
            profile,
            findings,
        };
        self.store.add_assessment(today(), &record).map_err(err)?;
        Ok(record)
    }

    /// First assessment: produces the proposed plan.
    pub fn run_assessment(&mut self) -> Res<()> {
        let record = self.record_assessment()?;
        let baseline = self.assessments()?.len() - 1;
        self.save_plan(&PlanRecord {
            state: PlanState::Draft,
            findings: record.findings,
            baseline,
            item_index: 0,
            last_trained: today(),
            no_change_before: false,
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
            PlanState::Active { day, .. } => daily_session(&plan.findings, MAINTENANCE_DRILL, day),
            _ => Vec::new(),
        }
    }

    fn run_scenario_file(
        &mut self,
        scenario: &Scenario,
        seconds: f64,
        name: &str,
    ) -> Res<Vec<bool>> {
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
            return Err("The drill was stopped before anything was recorded.".to_string());
        }
        Ok(trial_outcomes(&read_session(&output)?))
    }

    /// Plays the next drill of today's session and moves the difficulty for that drill.
    pub fn run_next_item(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("No plan yet.")?;
        let items = Self::today_items(&plan);
        let item = items
            .get(plan.item_index)
            .cloned()
            .ok_or("Today's session is already finished.")?;
        let s = self.settings()?;
        let seconds = if s.test_mode {
            TEST_MODE_DRILL_SECONDS
        } else {
            f64::from(item.minutes) * 60.0
        };

        let mut levels = self.levels()?;
        let start = levels.get(&item.drill).copied().unwrap_or(START_DIFFICULTY);
        let scenario = build_drill(&item.drill, start, item.varied, seconds)
            .ok_or_else(|| format!("Unknown drill: {}", item.drill))?;
        let outcomes = self.run_scenario_file(&scenario, seconds, &item.drill)?;

        let mut staircase = Staircase::new(start, STAIRCASE_TARGET, STAIRCASE_STEP);
        outcomes.iter().for_each(|&w| staircase.update(w));
        levels.insert(item.drill.clone(), staircase.d);
        self.store.set(KEY_LEVELS, &levels).map_err(err)?;
        self.store
            .add_practice(
                today(),
                &PracticeRecord {
                    day: today(),
                    name: item.drill,
                    hit_rate: hit_rate(&outcomes),
                    difficulty: Some(staircase.d),
                },
            )
            .map_err(err)?;

        plan.item_index += 1;
        plan.last_trained = today();
        if plan.item_index >= items.len() {
            plan.state = plan
                .state
                .clone()
                .apply(PlanEvent::DayCompleted { today: today() });
            plan.item_index = 0;
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
        let post = self.record_assessment()?;
        let baseline = self
            .assessments()?
            .get(plan.baseline)
            .cloned()
            .ok_or("Baseline assessment is missing.")?;
        let s = self.settings()?;
        let result = compare(&baseline, &post, &plan, s.lang());

        plan.no_change_before = result.adaptation == Adaptation::SwitchDrill;
        plan.state = plan.state.clone().apply(PlanEvent::RetestFinished);
        self.store.set(KEY_BLOCK_RESULT, &result).map_err(err)?;
        self.save_plan(&plan)
    }

    /// Starts the next block from the newest assessment.
    pub fn next_block(&mut self) -> Res<()> {
        let mut plan = self.plan()?.ok_or("No plan yet.")?;
        let assessments = self.assessments()?;
        let latest = assessments.last().ok_or("Take an assessment first.")?;
        plan.state = plan.state.clone().apply(PlanEvent::NextBlock);
        plan.findings = latest.findings.clone();
        plan.baseline = assessments.len() - 1;
        plan.item_index = 0;
        plan.last_trained = today();
        self.save_plan(&plan)
    }

    pub fn run_custom(&mut self, targets: &CustomTargets) -> Res<()> {
        let scenario = build_custom(targets)?;
        let outcomes = self.run_scenario_file(&scenario, targets.duration_s, "custom")?;
        self.store
            .add_practice(
                today(),
                &PracticeRecord {
                    day: today(),
                    name: "custom".to_string(),
                    hit_rate: hit_rate(&outcomes),
                    difficulty: None,
                },
            )
            .map_err(err)
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
        let outcomes = self.run_scenario_file(&scenario, seconds, name)?;
        self.store
            .add_practice(
                today(),
                &PracticeRecord {
                    day: today(),
                    name: name.to_string(),
                    hit_rate: hit_rate(&outcomes),
                    difficulty: None,
                },
            )
            .map_err(err)
    }

    fn coach_input(&self, s: &Settings) -> Res<Value> {
        let assessments = self.assessments()?;
        let latest = assessments.last();
        let round = |v: f64| (v * 100.0).round() / 100.0;
        let plan = self.plan()?;
        let result: Option<BlockResult> = self.store.get(KEY_BLOCK_RESULT).map_err(err)?;
        Ok(json!({
            "locale": s.locale,
            "player": {"cm360": round(s.cm_per_360()), "dpi": s.dpi, "sens": s.sens},
            "skills": latest.map(|a| a.profile.skills.iter().map(|k| json!({
                "skill": k.skill, "score": round(k.score), "raw": round(k.raw),
            })).collect::<Vec<_>>()),
            "findings": latest.map(|a| a.findings.iter().map(|f| json!({
                "id": f.id,
                "name": finding_text(&f.id, Locale::En).0,
                "severity": round(f.severity),
                "evidence": f.evidence.iter()
                    .map(|(k, v)| (k.clone(), json!(round(*v))))
                    .collect::<serde_json::Map<_, _>>(),
                "drills": f.drills,
            })).collect::<Vec<_>>()),
            "next_plan": {"days": BLOCK_DAYS, "focus": plan.map(|p| p.findings.iter()
                .take(2).map(|f| f.id.clone()).collect::<Vec<_>>())},
            "block_result": result.map(|r| json!({
                "training_effect": r.training_effect.map(round),
                "changes": r.rows.iter().map(|c| json!({
                    "skill": c.skill, "delta": round(c.delta), "trained": c.trained,
                })).collect::<Vec<_>>(),
            })),
        }))
    }

    fn provider(&self, s: &Settings) -> Res<Option<Box<dyn LlmProvider>>> {
        Ok(match s.ai_provider.as_str() {
            "local" => Some(Box::new(OpenAiCompatProvider {
                base_url: s.ai_base_url.clone(),
                model: s.ai_model.clone(),
                api_key: None,
            })),
            "claude" => {
                let key = std::env::var("ANTHROPIC_API_KEY")
                    .map_err(|_| "ANTHROPIC_API_KEY is not set on this computer.".to_string())?;
                let model = if s.ai_model.starts_with("claude") {
                    s.ai_model.clone()
                } else {
                    DEFAULT_CLAUDE_MODEL.to_string()
                };
                Some(Box::new(AnthropicProvider {
                    api_key: key,
                    model,
                }))
            }
            _ => None,
        })
    }

    fn template_reply(&self, s: &Settings) -> Res<CoachReply> {
        let locale = s.lang();
        let assessments = self.assessments()?;
        let findings = assessments
            .last()
            .map(|a| a.findings.as_slice())
            .unwrap_or_default();
        let (ar, headline, none) = (
            locale == Locale::Ar,
            findings.first().map(|f| finding_text(&f.id, locale).0),
            if locale == Locale::Ar {
                "ماكو مشاكل واضحة بعد. سوِّ تقييم أو كمل التدريب."
            } else {
                "No clear problems yet. Take an assessment or keep training."
            },
        );
        Ok(CoachReply {
            headline: headline.unwrap_or(none).to_string(),
            explanation: findings
                .iter()
                .map(|f| explain_finding(f, locale))
                .collect::<Vec<_>>()
                .join(" "),
            tips: findings
                .iter()
                .filter_map(|f| f.drills.first().cloned())
                .collect(),
            next_step: if ar {
                "ابدأ جلسة اليوم من صفحة الخطة.".to_string()
            } else {
                "Start today's session from the plan page.".to_string()
            },
        })
    }

    pub fn ask_coach(&self, question: Option<&str>) -> Res<ChatAnswer> {
        let s = self.settings()?;
        let attempt = self.provider(&s).and_then(|provider| match provider {
            None => Ok(None),
            Some(p) => {
                let input = self.coach_input(&s)?;
                narrate(p.as_ref(), &input, question).map(Some).map_err(err)
            }
        });
        Ok(match attempt {
            Ok(Some(reply)) => ChatAnswer {
                reply,
                source: "ai".to_string(),
                note: None,
            },
            Ok(None) => ChatAnswer {
                reply: self.template_reply(&s)?,
                source: "template".to_string(),
                note: None,
            },
            Err(e) => ChatAnswer {
                reply: self.template_reply(&s)?,
                source: "template".to_string(),
                note: Some(e),
            },
        })
    }

    pub fn view(&self) -> Res<View> {
        let settings = self.settings()?;
        let locale = settings.lang();
        let history = self.assessments()?;
        let latest = history.last().cloned();
        let findings = latest
            .iter()
            .flat_map(|a| a.findings.iter())
            .map(|f| {
                let (headline, why) = finding_text(&f.id, locale);
                FindingView {
                    id: f.id.clone(),
                    headline: headline.to_string(),
                    why: why.to_string(),
                    severity: f.severity,
                    evidence: f.evidence.clone(),
                    drills: f.drills.clone(),
                }
            })
            .collect();
        let plan = match self.plan()? {
            Some(p) => Some(PlanView {
                today_items: Self::today_items(&p),
                item_index: p.item_index,
                retest_ready: self.retest_ready(&p)?,
                state: p.state,
                block_days: BLOCK_DAYS,
                levels: self.levels()?,
            }),
            None => None,
        };
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
            settings,
        })
    }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aim_telemetry::format::{
        FrameStream, HitZone, ShotRecord, TargetKind, TargetParams, TargetRecord,
    };
    use aim_telemetry::{sample_session, write_aft};
    use std::cell::RefCell;

    /// Stands in for the game: writes a recording of an overshooting player wherever the arguments say.
    struct FakeTrainer {
        overshoot: RefCell<f64>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    fn value_after(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].clone())
    }

    /// 96 flicks at growing distances, each with a first movement of `gain` times the distance.
    fn flick_session(scenario: &str, gain: f64) -> AftSession {
        let mut s = sample_session();
        s.header.scenario_id = scenario.to_string();
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
                write(
                    &flick_session("assess.v1.flick_static", gain),
                    &Path::new(&dir).join("1_flick.aft"),
                )?;
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
            },
        )
        .unwrap();
        (e, dir)
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
    fn coach_uses_fixed_text_when_no_ai_is_selected() {
        let (mut e, dir) = engine("coach", 1.15);
        e.run_assessment().unwrap();
        let answer = e.ask_coach(Some("why do I miss?")).unwrap();
        assert_eq!(answer.source, "template");
        assert!(answer.note.is_none());
        assert!(!answer.reply.headline.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unreachable_ai_falls_back_and_says_why() {
        let (mut e, dir) = engine("offline", 1.15);
        let mut s = e.settings().unwrap();
        s.ai_provider = "local".to_string();
        s.ai_base_url = "http://127.0.0.1:9".to_string();
        e.save_settings(&s).unwrap();
        e.run_assessment().unwrap();
        let answer = e.ask_coach(None).unwrap();
        assert_eq!(answer.source, "template");
        assert!(answer.note.is_some());
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
}
