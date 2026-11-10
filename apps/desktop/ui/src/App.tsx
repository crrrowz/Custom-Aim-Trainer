import { Component, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { DayPicker, dayKey } from "./DayPicker";
import { call, type CoachConfig, type CustomTargets, type Detail, type Flick, type Plan, type SessionCard, type Settings, type View } from "./api";
import { AimPath, ReactionTimeline, SERIES, ShotScatter, SkillLines, SkillRadar, Spark, SpeedChart, TrackingChart, TrendLine, ValueBars, type Series } from "./charts";
import { drillInfo, drillNames, evidenceNames, groupNames, scenarioNames, skillNames, type DrillInfo, type Lang } from "./i18n";
import { ON_SCREEN_H_DEG, ON_SCREEN_V_DEG, TargetPreview, onScreen } from "./Preview";

type Tab = "home" | "analysis" | "progress" | "plan" | "practice" | "coach" | "settings";
const RUNS_TRAINER = new Set(["run_assessment", "run_next_item", "run_retest", "run_custom", "run_scenario"]);

interface Ctx {
  view: View;
  lang: Lang;
  T: (ar: string, en: string) => string;
  act: (command: string, args?: Record<string, unknown>) => Promise<void>;
  go: (tab: Tab, recording?: string) => void;
}

const pick = (pair: [string, string] | undefined, lang: Lang, fallback: string) => (pair ? pair[lang === "ar" ? 0 : 1] : fallback);
const NAV: [Tab, string, string, string][] = [
  ["home", "الرئيسية", "Dashboard", "M3 11l9-8 9 8v10h-6v-6H9v6H3z"],
  ["analysis", "التحليل", "Analysis", "M4 20V10m6 10V4m6 16v-7m5 7H2"],
  ["progress", "التقدم", "Progress", "M3 17l6-6 4 4 8-9M21 6v5m0-5h-5"],
  ["plan", "الخطة", "Plan", "M5 4h14v17H5zM8 2v4m8-4v4M5 10h14"],
  ["practice", "التدريب الحر", "Free practice", "M12 3v4m0 10v4M3 12h4m10 0h4M12 8a4 4 0 100 8 4 4 0 000-8z"],
  ["coach", "تقارير المدرب", "Coach reports", "M6 3h9l4 4v14H6zM9 12h7M9 16h7M9 8h3"],
  ["settings", "الإعدادات", "Settings", "M4 7h10m4 0h2M4 17h2m4 0h10M14 5v4m-4 6v4"],
];

const whenText = (ms: number, lang: Lang) =>
  ms ? new Date(ms).toLocaleString(lang === "ar" ? "ar-IQ" : "en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "";

/** The backend reports in English; players should read problems in their own language. */
function friendlyError(raw: string, T: Ctx["T"]): string {
  const known: [string, string, string][] = [
    ["Trainer program not found", "ما لكينا برنامج اللعبة. ابنيه بـ: cargo build --profile play -p trainer", "Game program not found. Build it with: cargo build --profile play -p trainer"],
    ["No target fits", "ماكو هدف يطلع بهذي المساحة. وسّعها أو قرّب الأهداف.", "No target fits inside that area. Widen it or bring the targets closer."],
    ["Add an API key", "ضيف مفتاح API لهذا المصدر من الإعدادات.", "Add an API key for this source in Settings."],
    ["Choose a model", "اكتب اسم الموديل بالإعدادات.", "Enter a model name in Settings."],
    ["Enter the server URL", "اكتب رابط الخادم بالإعدادات.", "Enter the server URL in Settings."],
    ["could not reach", "ما كدرنا نوصل للمصدر. تأكد من الرابط والإنترنت.", "Could not reach the source. Check the URL and your connection."],
    ["returned 401", "المفتاح مرفوض. تأكد منه.", "The key was rejected. Check it."],
    ["returned 403", "المصدر رفض الطلب. تأكد من صلاحيات المفتاح.", "The source refused the request. Check the key's permissions."],
    ["returned 404", "الموديل أو الرابط غلط.", "The model or URL is wrong."],
    ["returned 429", "وصلت حد الاستخدام عند المصدر. جرب بعد شوية.", "You hit the source's usage limit. Try again shortly."],
    ["not in your data", "رد الذكاء الاصطناعي بيه رقم مو من بياناتك، فما استخدمناه.", "The AI used a number that is not in your data, so it was not used."],
    ["The retest opens", "إعادة الاختبار تفتح بعد يوم راحة.", "The retest opens after a rest day."],
  ];
  const match = known.find(([needle]) => raw.includes(needle));
  return match ? T(match[1], match[2]) : raw;
}

export function App() {
  const [view, setView] = useState<View | null>(null);
  const [tab, setTab] = useState<Tab>("home");
  const [recording, setRecording] = useState<string | undefined>();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    call<View>("get_view").then(setView, (e) => setError(String(e)));
  }, []);

  const lang: Lang = view?.settings.locale === "en" ? "en" : "ar";
  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
  }, [lang]);

  const T = (ar: string, en: string) => (lang === "ar" ? ar : en);
  const act = async (command: string, args?: Record<string, unknown>) => {
    setError(null);
    setBusy(command);
    try {
      setView(await call<View>(command, args));
    } catch (e) {
      setError(friendlyError(String(e), T));
    } finally {
      setBusy(null);
    }
  };

  if (!view) return error ? <div className="banner err">{error}</div> : null;
  const ctx: Ctx = { view, lang, T, act, go: (t, r) => { setRecording(r); setTab(t); } };
  if (!view.settings.onboarded) return <Onboarding ctx={ctx} />;

  return (
    <div className="shell">
      <nav className="nav" aria-label="main">
        <div className="brand">Aim<span>Forge</span></div>
        <div className="lang-switch" role="group" aria-label="Language / اللغة">
          {(["ar", "en"] as const).map((l) => (
            <button key={l} className={lang === l ? "on" : ""} aria-pressed={lang === l} onClick={() => lang !== l && act("save_settings", { settings: { ...view.settings, locale: l } })}>
              {l === "ar" ? "العربية" : "English"}
            </button>
          ))}
        </div>
        {NAV.map(([id, ar, en, d]) => (
          <button key={id} className={tab === id ? "on" : ""} onClick={() => ctx.go(id)}>
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={d} /></svg>
            {T(ar, en)}
          </button>
        ))}
        <div className="nav-foot">
          <span>{view.settings.dpi} DPI · {view.settings.sens}</span>
          <span>{view.cm360.toFixed(1)} cm/360</span>
          {view.settings.test_mode && <span className="tag warn">{T("وضع التجربة", "Test mode")}</span>}
        </div>
      </nav>
      <main>
        {!view.trainer_found && <div className="banner">{T("ما لكينا برنامج اللعبة. ابنيه بـ: cargo build --release -p trainer", "Game program not found. Build it with: cargo build --release -p trainer")}</div>}
        {error && <div className="banner err">{T("صار خطأ", "Something went wrong")}: {error}</div>}
        {view.notice === "stopped" && <div className="banner">{T("طلعت قبل ما ينسجل شي، فما انحسبت هذي المرة.", "You left before anything was recorded, so this one did not count.")}</div>}
        {view.notice === "new_best" && <div className="banner good">{T("رقم قياسي جديد لهذا التمرين!", "New personal best for this drill!")}</div>}
        {view.notice === "done_today" && <div className="banner">{T("خلصت جلسة اليوم. الجلسة الجاية تفتح باجر.", "Today's session is done. The next one opens tomorrow.")}</div>}
        {view.notice === "incomplete" && <div className="banner">{T("التقييم وقف قبل ما يخلص، وكل اللي لعبته انحفظ وتشوفه بالتحليل. الاختبار ينحسب بالتقييم بس إذا كمل للآخر، فلما تبدي مرة ثانية تكمل من الاختبار اللي وقفت بي، مو من الأول.", "The assessment stopped before the end, and everything you played is saved and shown in Analysis. A test only counts toward the assessment when it runs to the end, so the next start continues from the test you left, not from the beginning.")}</div>}
        <ScreenGuard key={tab} message={T("هذي الصفحة واجهت خطأ. الصفحات الثانية تشتغل عادي.", "This page hit an error. The other pages still work.")} retry={T("أعد المحاولة", "Try again")}>
          {tab === "home" && <Home ctx={ctx} />}
          {tab === "analysis" && <Analysis ctx={ctx} initial={recording} />}
          {tab === "progress" && <Progress ctx={ctx} />}
          {tab === "plan" && <PlanPage ctx={ctx} />}
          {tab === "practice" && <Practice ctx={ctx} initial={recording} />}
          {tab === "coach" && <Coach ctx={ctx} />}
          {tab === "settings" && <SettingsPage ctx={ctx} />}
        </ScreenGuard>
      </main>
      {busy && RUNS_TRAINER.has(busy) && (
        <div className="overlay">
          <div className="pulse" />
          <b>{T("اللعبة شغالة", "The game is running")}</b>
          <span className="muted">{T("التحليل يطلع هنا أول ما تخلص.", "Your analysis appears here when you finish.")}</span>
        </div>
      )}
    </div>
  );
}

/** One broken screen must never blank the whole app. */
class ScreenGuard extends Component<{ message: string; retry: string; children: ReactNode }, { error: string | null }> {
  state = { error: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { error: String(e) };
  }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="empty">
        <p>{this.props.message}</p>
        <p className="small">{this.state.error}</p>
        <button className="btn" onClick={() => this.setState({ error: null })}>{this.props.retry}</button>
      </div>
    );
  }
}

function Onboarding({ ctx }: { ctx: Ctx }) {
  const { view, T, act } = ctx;
  const [s, setS] = useState(view.settings);
  return (
    <div className="onboard">
      <div className="brand big">Aim<span>Forge</span></div>
      <h1>{T("نطابق حركتك ويا VALORANT", "Match your VALORANT movement")}</h1>
      <p className="muted">{T("دخّل نفس إعدادات لعبتك، لأن كل القياسات مبنية عليها.", "Enter the settings you play with. Every measurement depends on them.")}</p>
      <div className="card grid2">
        <Num label="DPI" value={s.dpi} step={50} min={50} set={(dpi) => setS({ ...s, dpi })} />
        <Num label={T("الحساسية", "Sensitivity")} value={s.sens} step={0.01} min={0.01} set={(sens) => setS({ ...s, sens })} />
        <LangSelect ctx={ctx} value={s.locale} set={(locale) => setS({ ...s, locale })} />
      </div>
      <p className="small muted">{T("تلكاهم بـ VALORANT: Settings ← General ← Mouse Sensitivity، والـ DPI من برنامج ماوسك.", "Find these in VALORANT under Settings, General, Mouse Sensitivity; DPI is in your mouse software.")}</p>
      <button className="btn wide" onClick={() => act("save_settings", { settings: { ...s, onboarded: true } })}>{T("كمل", "Continue")}</button>
    </div>
  );
}

function PageHead({ title, sub, children }: { title: string; sub?: string; children?: ReactNode }) {
  return (
    <header className="head">
      <div>
        <h1>{title}</h1>
        {sub && <p className="muted">{sub}</p>}
      </div>
      {children && <div className="row">{children}</div>}
    </header>
  );
}

function Empty({ text, action }: { text: string; action?: ReactNode }) {
  return <div className="empty"><p>{text}</p>{action}</div>;
}

function Stat({ value, label, hint }: { value: string; label: string; hint?: string }) {
  return (
    <div className="stat">
      <b>{value}</b>
      <span>{label}</span>
      {hint && <small>{hint}</small>}
    </div>
  );
}

const overallOf = (a: View["latest"]) => {
  if (!a || a.profile.skills.length === 0) return null;
  return a.profile.skills.reduce((s, k) => s + k.score, 0) / a.profile.skills.length;
};

/** Word for an overall score. The norms behind the scores are provisional, and the page says so. */
const levelName = (score: number, T: Ctx["T"]) =>
  score < 35 ? T("مبتدئ", "Beginner") : score < 55 ? T("متوسط", "Intermediate") : score < 75 ? T("متقدم", "Advanced") : T("عالي جداً", "Elite");

/** Button text for starting an assessment, which continues an interrupted one instead of restarting. */
const assessCta = (view: View, T: Ctx["T"]) =>
  view.resume ? T(`كمل التقييم (${view.resume.done} من ${view.resume.total})`, `Continue assessment (${view.resume.done} of ${view.resume.total})`) : T("ابدأ التقييم", "Start assessment");

function Home({ ctx }: { ctx: Ctx }) {
  const { view, T, act, go, lang } = ctx;
  const basis = view.latest ?? view.estimate;
  const provisional = !view.latest && !!view.estimate;
  const overall = overallOf(basis);
  const prev = view.history.length > 1 ? overallOf(view.history[view.history.length - 2]) : null;
  const recent = view.sessions.slice(0, 5);
  const plan = view.plan;
  const active = plan && typeof plan.state === "object" && "Active" in plan.state ? plan.state.Active : null;
  const retest = plan && typeof plan.state === "object" && "AwaitingRetest" in plan.state;

  let hero: { title: string; sub: string; cta: string; run: () => void };
  if (!view.latest && view.resume) {
    const left = view.resume.total - view.resume.done;
    hero = { title: T("كمل التقييم من وين وقفت", "Continue your assessment"), sub: T(`خلصت ${view.resume.done} من ${view.resume.total} اختبارات وانحفظت. باقي ${left}، وتبدي من الاختبار الجاي مو من الأول.`, `${view.resume.done} of ${view.resume.total} tests are done and saved. ${left} left, starting from the next one, not from the beginning.`), cta: assessCta(view, T), run: () => act("run_assessment") };
  } else if (!view.latest) {
    hero = { title: T("ابدأ بالتقييم", "Start with an assessment"), sub: T("إحماء و7 اختبارات بحوالي 9 دقايق، وبعدها تعرف وين مشكلتك بالضبط.", "A warm-up and 7 tests in about 9 minutes, then you will know exactly what to fix."), cta: assessCta(view, T), run: () => act("run_assessment") };
  } else if (active && plan!.today_done) {
    hero = { title: T("خلصت جلسة اليوم", "Today's session is done"), sub: T("الجلسة الجاية تفتح باجر. الراحة جزء من التحسن، وتكدر تلعب تدريب حر.", "The next one opens tomorrow. Rest is part of improving; free practice is still open."), cta: T("التدريب الحر", "Free practice"), run: () => go("practice") };
  } else if (active) {
    const left = (plan!.today_items.length - plan!.item_index);
    hero = { title: T(`اليوم ${active.day} من ${plan!.block_days}`, `Day ${active.day} of ${plan!.block_days}`), sub: T(`باقي ${left} تمارين بجلسة اليوم.`, `${left} drills left in today's session.`), cta: T("كمل جلسة اليوم", "Continue today's session"), run: () => go("plan") };
  } else if (retest) {
    hero = { title: T("وقت إعادة الاختبار", "Time to retest"), sub: T("نقيس هل تحسنت فعلاً.", "Measure whether you really improved."), cta: T("افتح الخطة", "Open plan"), run: () => go("plan") };
  } else if (plan?.state === "Draft") {
    hero = { title: T("خطتك جاهزة", "Your plan is ready"), sub: T("6 أيام مركزة على أهم مشكلتين.", "6 days focused on your top two problems."), cta: T("شوف الخطة", "View plan"), run: () => go("plan") };
  } else {
    hero = { title: T("كمل تدريبك", "Keep training"), sub: T("اختار تمرين أو ابني جلستك.", "Pick a drill or build your own session."), cta: T("التدريب الحر", "Free practice"), run: () => go("practice") };
  }

  return (
    <>
      <PageHead title={T("لوحة التحكم", "Dashboard")} />
      <div className="bento">
        <section className="card hero">
          <h2>{hero.title}</h2>
          <p className="muted">{hero.sub}</p>
          <div className="row">
            <button className="btn" onClick={hero.run}>{hero.cta}</button>
            {view.latest && <button className="btn ghost" disabled={!view.trainer_found} onClick={() => act("run_assessment")}>{view.resume ? assessCta(view, T) : T("إعادة التقييم الكامل", "Retake full assessment")}</button>}
          </div>
        </section>
        <section className="card score">
          <span className="label">{T("المستوى العام", "Overall level")}{provisional && <i className="tag">{T("تقديري", "Estimate")}</i>}</span>
          <b className="huge">{overall === null ? "—" : overall.toFixed(0)}<small className="of">/100</small></b>
          {overall !== null && <b className="level-name">{levelName(overall, T)}</b>}
          <small className="muted explain">
            {overall === null
              ? T("يطلع بعد أول اختبار بعد الإحماء. الإحماء وحده ما يقيس مهارة.", "Appears after the first test after the warm-up. The warm-up alone measures no skill.")
              : provisional
                ? T(`معدل ${basis!.profile.skills.length} مهارات من التقييم اللي ما كمل. يصير أدق لما تكمل كل الاختبارات.`, `Average of ${basis!.profile.skills.length} skills from the unfinished assessment. It firms up once all tests are done.`)
                : T(`معدل درجات ${basis!.profile.skills.length} مهارات من 0 لـ 100. 50 يعني لاعب عادي، والمعايير مؤقتة لحد ما تنضبط على لاعبين حقيقيين.`, `Average of ${basis!.profile.skills.length} skill scores from 0 to 100. 50 is a typical player; the norms are provisional until calibrated on real players.`)}
          </small>
          {overall !== null && prev !== null && (
            <span className={overall >= prev ? "up" : "down"}>{overall >= prev ? "▲" : "▼"} {Math.abs(overall - prev).toFixed(1)}</span>
          )}
          <Spark values={view.history.map((a) => overallOf(a) ?? 0)} />
        </section>
        <section className="card radar">
          <span className="label">{T("ملف المهارات", "Skill profile")}</span>
          {basis && basis.profile.skills.length > 0 ? <SkillRadar skills={basis.profile.skills} name={(id) => pick(skillNames[id], lang, id)} /> : <Empty text={view.resume ? T("كمل التقييم حتى تطلع مهاراتك. الإحماء ما يحسب مهارة.", "Continue the assessment to see your skills. The warm-up measures no skill.") : T("يطلع بعد أول تقييم.", "Appears after your first assessment.")} />}
          {provisional && basis && basis.profile.skills.length > 0 && <small className="muted">{T("تقديري من الاختبارات اللي خلصتها بس.", "Estimated from the tests you finished only.")}</small>}
        </section>
        <section className="card problems">
          <span className="label">{T("أهم المشاكل", "Top problems")}</span>
          {view.findings.length === 0 && <Empty text={basis ? T("ما لكينا مشكلة مؤكدة لحد الآن.", "No confirmed problem so far.") : view.resume ? T("المشاكل تطلع من الاختبارات، مو من الإحماء. كمل التقييم.", "Problems come from the tests, not the warm-up. Continue the assessment.") : T("نحتاج تقييم حتى نحدد المشاكل.", "An assessment finds your problems.")} />}
          {view.findings.map((f, i) => (
            <button className="line" key={f.id} onClick={() => go("analysis")}>
              <i className="rank">{i + 1}</i>
              <span><b>{f.headline}</b><small>{f.why}</small></span>
              <Meter value={f.severity} />
            </button>
          ))}
        </section>
        <section className="card recent">
          <span className="label">{T("آخر الجلسات", "Recent sessions")}</span>
          {recent.length === 0 && <Empty text={T("جلساتك تتسجل هنا.", "Your sessions appear here.")} />}
          {recent.map((r) => (
            <button className="line" key={r.path} onClick={() => go("analysis", r.path)}>
              <span><b>{scenarioTitle(r.scenario, lang)}</b><small>{whenText(r.at_ms, lang)}</small></span>
              <b dir="ltr">{pct(sessionScore(r))}</b>
              <span className="chev">›</span>
            </button>
          ))}
        </section>
      </div>
    </>
  );
}

function Meter({ value }: { value: number }) {
  return <span className="meter" role="img" aria-label={`${Math.round(value * 100)}%`}><i style={{ width: `${Math.round(value * 100)}%` }} /></span>;
}

const flickSort: Record<string, (a: Flick[]) => Flick[]> = {
  all: (a) => a,
  misses: (a) => a.filter((f) => !f.hit),
  slowest: (a) => [...a].sort((x, y) => (y.time_to_hit_ms ?? 1e9) - (x.time_to_hit_ms ?? 1e9)).slice(0, 12),
  overshoot: (a) => [...a].filter((f) => f.gain !== null).sort((x, y) => (y.gain ?? 0) - (x.gain ?? 0)).slice(0, 12),
};

function flickNote(f: Flick, T: Ctx["T"]): string {
  if (f.onset_ms === null) return T("ما صارت حركة واضحة نحو الهدف.", "No clear movement toward the target.");
  if (f.premature) return T("نقرت والمؤشر بعده يتحرك قرب الهدف.", "You clicked while still moving, just short of the target.");
  if (f.gain !== null && f.gain > 1.05) return T(`الحركة الأولى تجاوزت الهدف ${Math.round((f.gain - 1) * 100)}%.`, `First movement went ${Math.round((f.gain - 1) * 100)}% past the target.`);
  if (f.gain !== null && f.gain < 0.85) return T(`الحركة الأولى وقفت قبل الهدف بـ ${Math.round((1 - f.gain) * 100)}%.`, `First movement stopped ${Math.round((1 - f.gain) * 100)}% short.`);
  return T("الحركة الأولى وصلت قريب من الهدف.", "First movement landed close to the target.");
}


const T2 = (lang: Lang, ar: string, en: string) => (lang === "ar" ? ar : en);
const MODE_NAMES: Record<string, [string, string]> = {
  flick: ["فليك", "Flick"], multi: ["أهداف متعددة", "Multi target"], peek: ["ظهور من الجدار", "Wall peek"],
  track: ["تتبع سلس", "Smooth track"], strafe: ["تتبع strafe", "Strafe track"], practice: ["جلسة", "Session"],
};

function scenarioTitle(id: string, lang: Lang): string {
  const key = id.split(".").pop() ?? id;
  if (id.startsWith("assess.")) return pick(scenarioNames[key], lang, key);
  if (id.startsWith("custom.")) return `${T2(lang, "حر", "Free")}: ${pick(MODE_NAMES[key], lang, key)}`;
  return pick(drillNames[key], lang, key);
}

const avg = (v: number[]) => (v.length ? v.reduce((a, b) => a + b, 0) / v.length : null);
const pct = (v: number | null | undefined) => (v === null || v === undefined ? "—" : `${Math.round(v * 100)}%`);
const ms = (v: number | null | undefined) => (v === null || v === undefined ? "—" : `${Math.round(v)} ms`);
const sessionScore = (c: SessionCard) => c.accuracy ?? c.time_on_target;

type DayMetric = "accuracy" | "reaction" | "time_to_hit" | "hits";

const DAY_METRICS: { id: DayMetric; ar: string; en: string; unit: string; better: "up" | "down"; of: (c: SessionCard) => number | null }[] = [
  { id: "accuracy", ar: "الدقة", en: "Accuracy", unit: "%", better: "up", of: (c) => { const v = sessionScore(c); return v === null ? null : v * 100; } },
  { id: "reaction", ar: "رد الفعل", en: "Reaction", unit: "ms", better: "down", of: (c) => c.median_reaction_ms },
  { id: "time_to_hit", ar: "وقت الإصابة", en: "Time to hit", unit: "ms", better: "down", of: (c) => c.median_time_to_hit_ms },
  { id: "hits", ar: "أهداف أصبتها", en: "Targets hit", unit: "", better: "up", of: (c) => (c.targets > 0 ? c.hits : null) },
];

const PAGE = 10;

function MetricGuide({ T }: { T: Ctx["T"] }) {
  const rows: [string, string][] = [
    [T("الدقة", "Accuracy"), T("كم هدف أصبت من كل الأهداف اللي طلعت. بالتتبع تعني نسبة الوقت اللي كان مؤشرك على الهدف.", "Targets hit out of all targets shown. In tracking it is the share of time your crosshair stayed on the target.")],
    [T("رد الفعل", "Reaction"), T("الوقت من ظهور الهدف لحد ما تبدي يدك تتحرك نحوه. أقل يعني أسرع. اللاعب العادي حوالي 250 ms.", "Time from the target appearing until your hand starts moving toward it. Lower is faster; a typical player is around 250 ms.")],
    [T("وقت الإصابة", "Time to hit"), T("الوقت من ظهور الهدف لحد الرصاصة اللي أصابته: رد الفعل والحركة والتصحيح سوا.", "Time from the target appearing to the shot that hit it: reaction, movement and correction together.")],
    [T("أهداف أصبتها", "Targets hit"), T("عدد الأهداف المصابة بالجلسة. يفيد للمقارنة بين جلسات بنفس الطول.", "Targets hit in the session. Useful for comparing sessions of the same length.")],
    [T("الأرقام بالمعدل", "Averages"), T("كل رقم هو الوسيط بالجلسة، فرصاصة شاذة وحدة ما تخربه.", "Each number is the session median, so one odd shot does not skew it.")],
  ];
  return (
    <details className="card guide">
      <summary>{T("شنو تعني هاي الأرقام؟", "What do these numbers mean?")}</summary>
      <dl>
        {rows.map(([k, v]) => <div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}
      </dl>
    </details>
  );
}

function Analysis({ ctx, initial }: { ctx: Ctx; initial?: string }) {
  const { view, T, lang, go } = ctx;
  const days = useMemo(() => new Set(view.sessions.map((s) => dayKey(s.at_ms))), [view.sessions]);
  const latestDay = useMemo(() => [...days].sort().pop(), [days]);
  const [open, setOpen] = useState<SessionCard | null>(() => view.sessions.find((s) => s.path === initial) ?? null);
  const [day, setDay] = useState(() => (open ? dayKey(open.at_ms) : latestDay ?? dayKey(Date.now())));
  const [metric, setMetric] = useState<DayMetric>("accuracy");
  const [shown, setShown] = useState(PAGE);
  const [selecting, setSelecting] = useState(false);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const togglePick = (path: string) => setPicked((p) => { const n = new Set(p); if (n.has(path)) n.delete(path); else n.add(path); return n; });
  const stopSelecting = () => { setSelecting(false); setPicked(new Set()); };

  if (view.sessions.length === 0) {
    return (
      <>
        <PageHead title={T("التحليل", "Analysis")} />
        <Empty text={T("بعد أول جلسة تشوف هنا كل يوم تدريب وكل حركة سويتها.", "After your first session you will see every training day and every movement here.")} action={<button className="btn" onClick={() => go("home")}>{T("ابدأ", "Get started")}</button>} />
      </>
    );
  }
  if (open) return <SessionView key={open.path} card={open} ctx={ctx} back={() => setOpen(null)} />;

  const locale = lang === "ar" ? "ar-IQ" : "en-GB";
  const today = dayKey(Date.now());
  const yesterday = dayKey(Date.now() - 86400000);
  const choose = (d: string) => { setDay(d); setShown(PAGE); stopSelecting(); };
  const longDate = new Date(`${day}T12:00`).toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long" });
  const cards = view.sessions.filter((s) => dayKey(s.at_ms) === day);
  const scored = cards.map(sessionScore).filter((v): v is number => v !== null);
  const reactions = cards.map((c) => c.median_reaction_ms).filter((v): v is number => v !== null);
  const toHit = cards.map((c) => c.median_time_to_hit_ms).filter((v): v is number => v !== null);
  const hits = cards.reduce((a, c) => a + c.hits, 0);
  const seconds = cards.reduce((a, c) => a + c.duration_s, 0);
  const time = (c: SessionCard) => new Date(c.at_ms).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });

  const m = DAY_METRICS.find((x) => x.id === metric)!;
  const ordered = [...cards].sort((a, b) => a.at_ms - b.at_ms).map((c) => ({ c, v: m.of(c) })).filter((r): r is { c: SessionCard; v: number } => r.v !== null);
  const fmt = (v: number) => `${Math.round(v)}${m.unit ? ` ${m.unit}` : ""}`;
  const points = ordered.map(({ c, v }, i) => {
    const prev = ordered[i - 1]?.v;
    const diff = prev === undefined ? "" : ` · ${v - prev >= 0 ? "+" : "−"}${Math.abs(Math.round(v - prev))} ${T("عن اللي قبلها", "vs previous")}`;
    return { label: time(c), value: v, tip: `${scenarioTitle(c.scenario, lang)} · ${fmt(v)}${diff}` };
  });
  let dayNote = "";
  if (ordered.length >= 2) {
    const best = ordered.reduce((a, b) => ((m.better === "up" ? b.v > a.v : b.v < a.v) ? b : a));
    const worst = ordered.reduce((a, b) => ((m.better === "up" ? b.v < a.v : b.v > a.v) ? b : a));
    const half = Math.floor(ordered.length / 2);
    const early = avg(ordered.slice(0, half).map((r) => r.v))!;
    const late = avg(ordered.slice(half).map((r) => r.v))!;
    const better = m.better === "up" ? late > early : late < early;
    dayNote = T(
      `أحسن جلسة: ${scenarioTitle(best.c.scenario, lang)} (${fmt(best.v)}) الساعة ${time(best.c)}. الأضعف: ${scenarioTitle(worst.c.scenario, lang)} (${fmt(worst.v)}). ${Math.round(early) === Math.round(late) ? "آخر اليوم نفس أوله." : better ? "آخر اليوم كنت أحسن من أوله." : "آخر اليوم نزلت عن أوله، يمكن تعبت."}`,
      `Best: ${scenarioTitle(best.c.scenario, lang)} (${fmt(best.v)}) at ${time(best.c)}. Weakest: ${scenarioTitle(worst.c.scenario, lang)} (${fmt(worst.v)}). ${Math.round(early) === Math.round(late) ? "The end of the day matched the start." : better ? "You finished the day better than you started." : "You finished the day below your start; you may have been tired."}`,
    );
  }

  const pickerLabel = day !== today && day !== yesterday ? new Date(`${day}T12:00`).toLocaleDateString(locale, { day: "numeric", month: "short" }) : T("يوم آخر", "Another day");

  return (
    <>
      <PageHead title={T("التحليل", "Analysis")} sub={T("اختار يوم، وافتح أي جلسة حتى تشوف تفاصيلها.", "Pick a day and open any session for its details.")} />
      <div className="day-bar">
        <div className="segmented">
          <button className={day === today ? "on" : ""} disabled={!days.has(today)} onClick={() => choose(today)}>{T("اليوم", "Today")}</button>
          <button className={day === yesterday ? "on" : ""} disabled={!days.has(yesterday)} onClick={() => choose(yesterday)}>{T("أمس", "Yesterday")}</button>
        </div>
        <DayPicker available={days} selected={[day]} onChange={(d) => d[0] && choose(d[0])} lang={lang} label={pickerLabel} />
        <span className="day-title">{longDate}</span>
      </div>
      <div className="stats">
        <Stat value={String(cards.length)} label={T("جلسات", "Sessions")} hint={`${Math.round(seconds / 60)} ${T("دقيقة تدريب", "minutes trained")}`} />
        <Stat value={pct(avg(scored))} label={T("معدل الدقة", "Average accuracy")} hint={T("أعلى أحسن", "Higher is better")} />
        <Stat value={ms(avg(reactions))} label={T("معدل رد الفعل", "Average reaction")} hint={T("أقل أسرع", "Lower is faster")} />
        <Stat value={ms(avg(toHit))} label={T("معدل وقت الإصابة", "Average time to hit")} hint={T("أقل أسرع", "Lower is faster")} />
        <Stat value={String(hits)} label={T("أهداف أصبتها", "Targets hit")} />
      </div>
      <MetricGuide T={T} />
      {cards.length > 0 && (
        <section className="card">
          <div className="row between wrap">
            <h2>{T("جلساتك خلال اليوم", "Your sessions through the day")}</h2>
            <div className="segmented small">
              {DAY_METRICS.map((x) => <button key={x.id} className={x.id === metric ? "on" : ""} onClick={() => setMetric(x.id)}>{lang === "ar" ? x.ar : x.en}</button>)}
            </div>
          </div>
          {dayNote && <p className="muted small">{dayNote}</p>}
          {points.length > 0
            ? <TrendLine points={points} unit={m.unit} better={m.better} />
            : <Empty text={T("هذا المقياس ما ينطبق على جلسات هذا اليوم (مثلاً التتبع ما بيه رد فعل).", "This measure does not apply to this day's sessions (tracking has no reaction time, for example).")} />}
          <p className="muted small">{T("كل نقطة جلسة. مرر الماوس عليها حتى تشوف اسمها والفرق عن الجلسة اللي قبلها.", "Each point is a session. Hover to see its name and the change from the session before.")}</p>
        </section>
      )}
      <div className="list-head">
        <h2 className="group">{T("جلسات هذا اليوم", "Sessions this day")} <small className="muted">({cards.length})</small></h2>
        {cards.length > 0 && (selecting ? (
          <div className="row">
            <span className="muted small">{T(`محدد ${picked.size}`, `${picked.size} selected`)}</span>
            <button className="btn ghost small" onClick={() => setPicked(picked.size === cards.length ? new Set() : new Set(cards.map((c) => c.path)))}>{picked.size === cards.length ? T("إلغاء الكل", "Select none") : T("حدد كل اليوم", "Select the whole day")}</button>
            {picked.size > 0 && <ConfirmButton label={T(`احذف ${picked.size}`, `Delete ${picked.size}`)} confirm={T("متأكد؟ ما يرجع", "Sure? This is permanent")} onConfirm={async () => { await ctx.act("delete_sessions", { paths: [...picked] }); stopSelecting(); }} />}
            <button className="btn ghost small" onClick={stopSelecting}>{T("خلص", "Done")}</button>
          </div>
        ) : <button className="btn ghost small" onClick={() => setSelecting(true)}>{T("تحديد", "Select")}</button>)}
      </div>
      {cards.length === 0 && <Empty text={T("ما لعبت بهذا اليوم.", "No sessions on this day.")} />}
      <div className="session-grid">
        {cards.slice(0, shown).map((c) => (
          <button key={c.path} className={`session-card ${selecting ? "selectable" : ""} ${picked.has(c.path) ? "picked" : ""}`} aria-pressed={selecting ? picked.has(c.path) : undefined} onClick={() => (selecting ? togglePick(c.path) : setOpen(c))}>
            <span className="row between">
              <b>{selecting && <i className="check" aria-hidden="true" />}{scenarioTitle(c.scenario, lang)}</b>
              <small>{time(c)}</small>
            </span>
            <span className="big-num">{pct(sessionScore(c))}</span>
            <span className="meter wide"><i style={{ width: `${Math.round((sessionScore(c) ?? 0) * 100)}%`, background: "var(--accent)" }} /></span>
            <span className="row between small muted">
              <span>{c.targets > 0 ? `${c.hits}/${c.targets} ${T("هدف", "targets")}` : T("تتبع", "Tracking")}</span>
              <span dir="ltr">{ms(c.median_reaction_ms)}</span>
            </span>
            {c.assessment && <span className="tag good">{T("تقييم", "Assessment")}</span>}
            {c.partial && <span className="tag warn">{T("ما كملت", "Not finished")}</span>}
            {c.timing_suspect && <span className="tag warn">{T("نسخة قديمة، الأوقات غير دقيقة", "Old recording, timing unreliable")}</span>}
          </button>
        ))}
      </div>
      {cards.length > shown && (
        <div className="more">
          <button className="btn ghost" onClick={() => setShown(shown + PAGE)}>{T(`عرض المزيد (باقي ${cards.length - shown})`, `Show more (${cards.length - shown} left)`)}</button>
        </div>
      )}
    </>
  );
}

const DIRECTION_NAMES: [string, string, string][] = [
  ["يمين", "Right", "→"], ["يمين فوق", "Up-right", "↗"], ["فوق", "Up", "↑"], ["يسار فوق", "Up-left", "↖"],
  ["يسار", "Left", "←"], ["يسار جوه", "Down-left", "↙"], ["جوه", "Down", "↓"], ["يمين جوه", "Down-right", "↘"],
];

/** Destructive actions take two deliberate clicks. */
function ConfirmButton({ label, confirm, onConfirm }: { label: string; confirm: string; onConfirm: () => void }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return <button className={`btn ghost ${armed ? "danger" : ""}`} onClick={() => (armed ? onConfirm() : setArmed(true))}>{armed ? confirm : label}</button>;
}

async function saveShareCard(card: SessionCard, d: Detail, lang: Lang): Promise<string> {
  const c = document.createElement("canvas");
  c.width = 1080;
  c.height = 600;
  const g = c.getContext("2d")!;
  const font = (px: number, weight = 400) => `${weight} ${px}px "Plex Arabic", Inter, sans-serif`;
  g.fillStyle = "#0a0c0f";
  g.fillRect(0, 0, 1080, 600);
  g.fillStyle = "#3ddc97";
  g.fillRect(0, 0, 1080, 8);
  g.fillStyle = "#e8edf3";
  g.font = font(40, 600);
  g.fillText("AimForge", 64, 96);
  g.font = font(52, 600);
  g.direction = lang === "ar" ? "rtl" : "ltr";
  g.textAlign = lang === "ar" ? "right" : "left";
  const x = lang === "ar" ? 1016 : 64;
  g.fillText(scenarioTitle(card.scenario, lang), x, 200);
  g.fillStyle = "#8a97a8";
  g.font = font(28);
  g.fillText(new Date(card.at_ms).toLocaleDateString(lang === "ar" ? "ar-IQ" : "en-GB"), x, 250);
  g.direction = "ltr";
  g.textAlign = "left";
  const stats: [string, string][] = d.tracking.length
    ? [[pct(d.time_on_target), T2(lang, "على الهدف", "On target")]]
    : [[pct(d.accuracy), T2(lang, "الدقة", "Accuracy")], [ms(d.median_reaction_ms), T2(lang, "رد الفعل", "Reaction")], [ms(d.median_time_to_hit_ms), T2(lang, "وقت الإصابة", "Time to hit")]];
  stats.forEach(([value, label], i) => {
    const sx = 64 + i * 330;
    g.fillStyle = "#3ddc97";
    g.font = font(72, 600);
    g.fillText(value, sx, 410);
    g.fillStyle = "#8a97a8";
    g.font = font(28);
    g.fillText(label, sx, 460);
  });
  const blob = await new Promise<Blob>((ok, fail) => c.toBlob((b) => (b ? ok(b) : fail(new Error("png"))), "image/png"));
  const png = Array.from(new Uint8Array(await blob.arrayBuffer()));
  const stamp = new Date(card.at_ms);
  const name = `aimforge-${dayKey(card.at_ms)}-${String(stamp.getHours()).padStart(2, "0")}${String(stamp.getMinutes()).padStart(2, "0")}`;
  return call<string>("save_share_card", { png, name });
}

function SessionView({ card, ctx, back }: { card: SessionCard; ctx: Ctx; back: () => void }) {
  const { T, lang } = ctx;
  const [detail, setDetail] = useState<Detail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"replay" | "trend" | "map" | "dirs">("replay");
  const [sharedTo, setSharedTo] = useState<string | null>(null);
  const share = async () => {
    try { setSharedTo(await saveShareCard(card, detail!, lang)); } catch (e) { setError(String(e)); }
  };

  useEffect(() => {
    call<Detail>("session_detail", { path: card.path }).then(setDetail, (e) => setError(String(e)));
  }, [card.path]);

  const flicks = detail?.flicks.filter((f) => f.path.length > 0) ?? [];
  const isTracking = detail ? detail.tracking.length > 0 : false;
  const tabs: [typeof tab, string][] = [["replay", T("إعادة الحركة", "Aim replay")], ["trend", T("أداؤك خلال الجلسة", "During the session")], ["map", T("وين نزلت طلقاتك", "Where shots landed")], ["dirs", T("الدقة والسرعة بالاتجاهات", "Accuracy & speed by direction")]];

  const third = Math.max(1, Math.floor(flicks.length / 3));
  const seg = (list: Flick[]) => ({
    acc: list.length ? list.filter((f) => f.hit).length / list.length : null,
    time: avg(list.map((f) => f.time_to_hit_ms).filter((v): v is number => v !== null)),
  });
  const [early, late] = [seg(flicks.slice(0, third)), seg(flicks.slice(-third))];

  const shots = flicks.filter((f) => f.endpoint && f.octant !== null);
  const offX = avg(shots.map((f) => f.endpoint![0])) ?? 0;
  const offY = avg(shots.map((f) => f.endpoint![1])) ?? 0;
  const spread = avg(shots.map((f) => Math.hypot(f.endpoint![0], f.endpoint![1]))) ?? 0;
  const inside = shots.length ? shots.filter((f) => Math.hypot(f.endpoint![0], f.endpoint![1]) <= f.radius_deg).length / shots.length : null;
  const bias = [
    Math.abs(offX) >= 0.1 ? `${Math.abs(offX).toFixed(2)}° ${offX > 0 ? T("يمين", "right") : T("يسار", "left")}` : null,
    Math.abs(offY) >= 0.1 ? `${Math.abs(offY).toFixed(2)}° ${offY > 0 ? T("فوق", "up") : T("جوه", "down")}` : null,
  ].filter(Boolean).join(" · ");

  // Error along the movement: positive went past the target, negative stopped short of it.
  const along = shots.map((f) => {
    const a = ((f.octant ?? 0) * Math.PI) / 4;
    return f.endpoint![0] * Math.cos(a) + f.endpoint![1] * Math.sin(a);
  });
  const reach = avg(along) ?? 0;
  const misses = shots.filter((f) => !f.hit);
  const missAlong = misses.map((f) => { const a = ((f.octant ?? 0) * Math.PI) / 4; return f.endpoint![0] * Math.cos(a) + f.endpoint![1] * Math.sin(a); });
  const pastShare = missAlong.length ? missAlong.filter((v) => v > 0).length / missAlong.length : null;
  const radius = avg(shots.map((f) => f.radius_deg)) ?? 0.5;
  let shotVerdict: string;
  if (shots.length < 5) shotVerdict = T("طلقات قليلة، العب جلسة أطول حتى يطلع نمط واضح.", "Too few shots for a clear pattern; play a longer session.");
  else if (Math.abs(reach) > radius * 0.35) shotVerdict = reach > 0
    ? T(`مشكلتك الأوضح: تعبر الهدف. طلقاتك توقف بعد مركزه بـ ${reach.toFixed(2)}° بالمعدل باتجاه حركتك. خفف قبل الوصول.`, `Your clearest problem: you go past the target. Shots land ${reach.toFixed(2)}° beyond its centre along your movement. Slow down before arriving.`)
    : T(`مشكلتك الأوضح: توقف قبل الهدف. طلقاتك تنزل قبل مركزه بـ ${Math.abs(reach).toFixed(2)}° بالمعدل. كمّل الحركة للمركز.`, `Your clearest problem: you stop short. Shots land ${Math.abs(reach).toFixed(2)}° before the centre on average. Carry the movement to the centre.`);
  else if (bias) shotVerdict = T(`طلقاتك مايلة ${bias} بالمعدل، وهذا ميلان ثابت تكدر تصححه.`, `Your shots lean ${bias} on average, a steady lean you can correct.`);
  else if (spread > radius * 1.2) shotVerdict = T("طلقاتك متمركزة بس منتشرة. الثبات قبل النقرة يجمعها.", "Your shots are centred but scattered. Settling before the click tightens them.");
  else shotVerdict = T("طلقاتك متمركزة ومجموعة. حلو.", "Your shots are centred and tight. Good.");

  const dirs = DIRECTION_NAMES.map(([ar, en, arrow], o) => {
    const list = flicks.filter((f) => f.octant === o);
    return { name: `${arrow}  ${T(ar, en)}`, n: list.length, acc: list.length ? list.filter((f) => f.hit).length / list.length : 0, time: avg(list.map((f) => f.time_to_hit_ms).filter((v): v is number => v !== null)) };
  }).filter((d) => d.n > 0);
  const maxTime = Math.max(...dirs.map((d) => d.time ?? 0), 1);
  const rated = dirs.filter((d) => d.n >= 3);
  const weakest = rated.length > 1 ? rated.reduce((a, b) => (b.acc < a.acc ? b : a)) : null;
  const slowest = rated.length > 1 ? rated.reduce((a, b) => ((b.time ?? 0) > (a.time ?? 0) ? b : a)) : null;

  return (
    <>
      <PageHead title={scenarioTitle(card.scenario, lang)} sub={new Date(card.at_ms).toLocaleString(lang === "ar" ? "ar-IQ" : "en-GB", { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" })}>
        {detail && !detail.timing_suspect && <button className="btn ghost" onClick={share}>{T("بطاقة مشاركة", "Share card")}</button>}
        <ConfirmButton label={T("احذف", "Delete")} confirm={T("متأكد؟ ما يرجع", "Sure? This is permanent")} onConfirm={async () => { await ctx.act("delete_session", { path: card.path }); back(); }} />
        <button className="btn ghost" onClick={back}>{T("كل الجلسات", "All sessions")}</button>
      </PageHead>
      {error && <div className="banner err">{error}</div>}
      {sharedTo && <div className="banner good">{T("انحفظت بطاقة المشاركة هنا:", "Share card saved to:")} <code dir="ltr">{sharedTo}</code></div>}
      {!detail && !error && <div className="skeleton" />}
      {detail?.timing_suspect && <div className="banner">{T("هذي الجلسة انسجلت قبل إصلاح الساعة، فأرقام الوقت مخفية.", "This session was recorded before the clock fix, so timing numbers are hidden.")}</div>}
      {detail && (
        <>
          <div className="stats">
            {isTracking ? (
              <>
                <Stat value={pct(detail.time_on_target)} label={T("وقتك على الهدف", "Time on target")} />
                <Stat value={`${(detail.tracking.reduce((a, b) => a + b.error_deg, 0) / detail.tracking.length).toFixed(2)}°`} label={T("متوسط الابتعاد", "Average error")} />
                <Stat value={`${detail.duration_s.toFixed(0)} s`} label={T("المدة", "Length")} />
              </>
            ) : (
              <>
                <Stat value={pct(detail.accuracy)} label={T("الدقة", "Accuracy")} hint={`${detail.hits} / ${detail.targets}`} />
                <Stat value={ms(detail.median_reaction_ms)} label={T("رد الفعل", "Reaction")} />
                <Stat value={ms(detail.median_time_to_hit_ms)} label={T("وقت الإصابة", "Time to hit")} />
                <Stat value={detail.median_gain === null ? "—" : `${Math.round(detail.median_gain * 100)}%`} label={T("وين توصل حركتك الأولى", "First movement reach")} hint={T("المثالي 92–100%", "Ideal 92–100%")} />
              </>
            )}
          </div>
          {isTracking ? (
            <section className="card">
              <h2>{T("بُعد مؤشرك عن الهدف على الزمن", "Crosshair distance from target over time")}</h2>
              <p className="muted small">{T("الشريط الأخضر يعني أنت على الهدف. كل ما نزل الخط كان تتبعك أحسن.", "The green band means on target. A lower line is better tracking.")}</p>
              <TrackingChart bins={detail.tracking} radius={detail.tracking_radius_deg ?? 0.6} yLabel="°" />
            </section>
          ) : flicks.length === 0 ? (
            <Empty text={T("ما انسجلت أهداف بهذي الجلسة.", "No targets were recorded in this session.")} />
          ) : (
            <>
              <div className="tabs" role="tablist">
                {tabs.map(([id, name]) => <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "on" : ""} onClick={() => setTab(id)}>{name}</button>)}
              </div>
              <div className="tab-pane">
              {tab === "replay" && <Replay detail={detail} ctx={ctx} />}
              {tab === "trend" && (
                <>
                  <div className="stats">
                    <Stat value={pct(early.acc)} label={T("دقة أول الجلسة", "Accuracy, first third")} hint={ms(early.time)} />
                    <Stat value={pct(late.acc)} label={T("دقة آخر الجلسة", "Accuracy, last third")} hint={ms(late.time)} />
                    <Stat value={early.acc !== null && late.acc !== null ? `${late.acc >= early.acc ? "+" : ""}${Math.round((late.acc - early.acc) * 100)}` : "—"} label={T("التغيير بالنقاط", "Change, points")} />
                  </div>
                  <section className="card">
                    <h2>{T("وقت كل هدف من بداية الجلسة لنهايتها", "Time per target, start to finish")}</h2>
                    <p className="muted small">{T("كل نقطة هدف. الخط الأبيض هو اتجاهك: إذا نزل تتحسن، وإذا صعد تتعب.", "Each dot is a target. The white line is your trend: falling is improving, rising is tiring.")}</p>
                    <ReactionTimeline flicks={flicks} yLabel="ms" hitLabel={T("إصابة", "Hit")} missLabel={T("خطأ", "Miss")} />
                  </section>
                </>
              )}
              {tab === "map" && (
                <section className="card shotmap">
                  <div className="shotmap-chart">
                    <ShotScatter flicks={flicks} hitLabel={T("إصابة", "Hit")} missLabel={T("خطأ", "Miss")} />
                    <Legend items={[["#3ddc97", T("إصابة", "Hit")], ["#ff6b6b", T("خطأ", "Miss")]]} />
                  </div>
                  <div className="shotmap-info">
                    <h2>{T("وين نزلت طلقاتك", "Where your shots landed")}</h2>
                    <p className="verdict">{shotVerdict}</p>
                    <div className="mini-stats">
                      <div><b>{pct(inside)}</b><span>{T("داخل الهدف", "Inside the target")}</span></div>
                      <div><b dir="ltr">{`${reach >= 0 ? "+" : "−"}${Math.abs(reach).toFixed(2)}°`}</b><span>{T("بعد/قبل المركز باتجاه الحركة", "Past (+) or short (−) of centre")}</span></div>
                      <div><b dir="ltr">{`${spread.toFixed(2)}°`}</b><span>{T("متوسط البعد عن المركز", "Average distance from centre")}</span></div>
                      <div><b>{pastShare === null ? "—" : pct(pastShare)}</b><span>{T("من الأخطاء عبرت الهدف", "Of misses went past")}</span></div>
                    </div>
                    <p className="muted small">{T("كل نقطة طلقة، مرسومة حول مركز هدفها. الدائرة الخضراء حجم الهدف، والأكبر ضعفه. علامة + مركز كل طلقاتك.", "Each dot is a shot drawn around its target's centre. The green circle is the target size and the larger one twice that. + marks the centre of all your shots.")}</p>
                  </div>
                </section>
              )}
              {tab === "dirs" && (
                <section className="card">
                  <h2>{T("الدقة والسرعة بكل اتجاه", "Accuracy and speed in each direction")}</h2>
                  <p className="muted small">{T("الاتجاه هو وين تحركت حتى توصل الهدف.", "Direction is where you had to move to reach the target.")}</p>
                  {weakest && slowest && (
                    <p className="verdict">{weakest.name === slowest.name
                      ? T(`أضعف اتجاه عندك: ${weakest.name}، أقل دقة وأبطأ. تمرين الاتجاه الضعيف يركز عليه.`, `Your weakest direction is ${weakest.name}: least accurate and slowest. The weak-direction drill focuses on it.`)
                      : T(`أقل دقة: ${weakest.name} · أبطأ: ${slowest.name}.`, `Least accurate: ${weakest.name} · Slowest: ${slowest.name}.`)}</p>
                  )}
                  <table className="dir-table">
                    <thead><tr><th>{T("الاتجاه", "Direction")}</th><th>{T("الدقة", "Accuracy")}</th><th>{T("الوقت", "Time")}</th><th>{T("أهداف", "Targets")}</th></tr></thead>
                    <tbody>
                      {dirs.map((d) => (
                        <tr key={d.name} className={d === weakest || d === slowest ? "weak" : ""}>
                          <td>{d.name}</td>
                          <td><span className="bar-cell"><span className="meter wide"><i style={{ width: `${Math.round(d.acc * 100)}%`, background: "var(--accent)" }} /></span><b dir="ltr">{Math.round(d.acc * 100)}%</b></span></td>
                          <td><span className="bar-cell"><span className="meter wide"><i style={{ width: `${Math.round(((d.time ?? 0) / maxTime) * 100)}%` }} /></span><b dir="ltr">{ms(d.time)}</b></span></td>
                          <td>{d.n}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </section>
              )}
              </div>
            </>
          )}
        </>
      )}
    </>
  );
}

function Legend({ items }: { items: [string, string][] }) {
  return <div className="legend">{items.map(([c, n]) => <span key={n}><i style={{ background: c }} />{n}</span>)}</div>;
}

function Replay({ detail, ctx }: { detail: Detail; ctx: Ctx }) {
  const { T } = ctx;
  const [filter, setFilter] = useState("all");
  const list = useMemo(() => flickSort[filter](detail.flicks.filter((f) => f.path.length > 0)), [detail, filter]);
  const [index, setIndex] = useState(0);
  const flick = list[Math.min(index, list.length - 1)];
  const end = flick ? flick.path[flick.path.length - 1].t_ms : 0;
  const [ms, setMs] = useState(end);
  const [playing, setPlaying] = useState(false);
  const [auto, setAuto] = useState(false);
  const [rate, setRate] = useState(0.25);
  const raf = useRef(0);
  const autoRef = useRef(auto);
  autoRef.current = auto;
  // Kept apart from the animation's cleanup: finishing a target stops the animation, and that cleanup
  // must not cancel the move to the next target.
  const advance = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const cancelAdvance = () => { if (advance.current) clearTimeout(advance.current); advance.current = undefined; };
  useEffect(() => cancelAdvance, []);
  useEffect(() => { if (!auto) cancelAdvance(); }, [auto]);

  // A new target starts from the beginning when auto play is on, and shows the whole path otherwise.
  useEffect(() => {
    if (autoRef.current) { setMs(0); setPlaying(true); } else { setMs(end); setPlaying(false); }
  }, [flick?.id, end]);
  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    const step = (now: number) => {
      const dt = (now - last) * rate;
      last = now;
      let done = false;
      setMs((m) => {
        const v = m + dt;
        if (v >= end) { done = true; return end; }
        return v;
      });
      if (!done) { raf.current = requestAnimationFrame(step); return; }
      setPlaying(false);
      // Auto play waits a moment on the finished path, then moves to the next target.
      if (autoRef.current) {
        cancelAdvance();
        advance.current = setTimeout(() => setIndex((i) => {
          if (i + 1 >= list.length) { setAuto(false); return i; }
          return i + 1;
        }), 700);
      }
    };
    raf.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf.current);
  }, [playing, rate, end, list.length]);

  const move = (d: number) => setIndex((i) => Math.max(0, Math.min(list.length - 1, i + d)));
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        e.preventDefault();
        setIndex((i) => Math.max(0, Math.min(list.length - 1, i + ((e.key === "ArrowRight") === (document.documentElement.dir !== "rtl") ? 1 : -1))));
      }
      if (e.key === " ") { e.preventDefault(); setPlaying((p) => !p); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [list.length]);
  const labels = { onset: T("بداية", "Start"), primary: T("نهاية الحركة", "First stop"), shot: T("نقرة", "Click") };
  const filters: [string, string][] = [["all", T("الكل", "All")], ["misses", T("الأخطاء", "Misses")], ["slowest", T("الأبطأ", "Slowest")], ["overshoot", T("أكبر تجاوز", "Biggest overshoot")]];
  const playOne = () => { setAuto(false); if (ms >= end) setMs(0); setPlaying(!playing); };
  const playAll = () => {
    if (auto) { setAuto(false); setPlaying(false); return; }
    setAuto(true);
    setMs(0);
    setPlaying(true);
  };
  const rates: [number, string][] = [[0.1, "0.1×"], [0.25, "0.25×"], [0.5, "0.5×"], [1, "1×"]];
  const position = list.length ? Math.min(index, list.length - 1) + 1 : 0;

  return (
    <section className="card replay-card">
      <header className="replay-head">
        <div className="seg">
          {filters.map(([id, name]) => <button key={id} className={filter === id ? "on" : ""} onClick={() => { setFilter(id); setIndex(0); setAuto(false); }}>{name}</button>)}
        </div>
        <span className="muted small" dir="ltr">{position} / {list.length}</span>
      </header>
      {!flick ? <Empty text={T("ماكو أهداف بهذا الفلتر.", "No targets match this filter.")} /> : (
        <>
          <div className="strip" dir="ltr">
            {list.map((f, i) => <button key={f.id} aria-label={`${i + 1}`} className={`${f.hit ? "h" : "m"} ${f.id === flick.id ? "on" : ""}`} onClick={() => { setAuto(false); setIndex(i); }} />)}
          </div>
          <div className="replay">
            <div className="replay-stage">
              <AimPath flick={flick} progressMs={ms} labels={labels} />
              <div className="transport" dir="ltr">
                <button className="btn ghost sq" aria-label={T("السابق", "Previous")} onClick={() => { setAuto(false); move(-1); }}>‹</button>
                <button className="btn" onClick={playOne}>{playing && !auto ? T("إيقاف", "Pause") : T("شغّل هذا", "Play this")}</button>
                <button className={`btn ${auto ? "" : "ghost"}`} onClick={playAll} aria-pressed={auto}>{auto ? T("أوقف التلقائي", "Stop auto play") : T("تشغيل تلقائي", "Auto play")}</button>
                <button className="btn ghost sq" aria-label={T("التالي", "Next")} onClick={() => { setAuto(false); move(1); }}>›</button>
                <div className="seg speed" role="group" aria-label={T("السرعة", "Speed")}>
                  {rates.map(([r, name]) => <button key={r} className={rate === r ? "on" : ""} onClick={() => setRate(r)}>{name}</button>)}
                </div>
              </div>
            </div>
            <aside className="replay-info">
              <div className="row between">
                <b>{T(`الهدف ${position}`, `Target ${position}`)}</b>
                <span className={`tag ${flick.hit ? "good" : "bad"}`}>{flick.hit ? T("إصابة", "Hit") : T("خطأ", "Miss")}</span>
              </div>
              <p className="note">{flickNote(flick, T)}</p>
              <dl>
                <dt>{T("المسافة", "Distance")}</dt><dd>{flick.distance_deg === null ? "—" : `${flick.distance_deg.toFixed(1)}°`}</dd>
                <dt>{T("رد الفعل", "Reaction")}</dt><dd>{flick.reaction_ms === null ? "—" : `${flick.reaction_ms.toFixed(0)} ms`}</dd>
                <dt>{T("وقت النقرة", "Click at")}</dt><dd>{flick.shot_ms === null ? "—" : `${flick.shot_ms.toFixed(0)} ms`}</dd>
                <dt>{T("الحركة الأولى", "First movement")}</dt><dd>{flick.gain === null ? "—" : `${(flick.gain * 100).toFixed(0)}%`}</dd>
                <dt>{T("التصحيحات", "Corrections")}</dt><dd>{flick.corrections}</dd>
              </dl>
              <small className="muted">{T("مسافة: تشغيل/إيقاف · الأسهم: الهدف السابق والتالي", "Space: play/pause · Arrows: previous and next target")}</small>
            </aside>
          </div>
          <SpeedChart flick={flick} progressMs={ms} onScrub={(v) => { setAuto(false); setPlaying(false); setMs(v); }} labels={{ y: T("السرعة °/ث", "Speed °/s"), ...labels }} />
        </>
      )}
    </section>
  );
}

type Range = "today" | "week" | "month" | "year" | "all";
type Kind = "all" | "assessment" | "plan" | "free";
type Bucket = { label: string; tick: boolean; cards: SessionCard[] };

const kindOf = (c: SessionCard): Kind => (c.assessment ? "assessment" : c.scenario.startsWith("custom.") ? "free" : "plan");

/** Groups sessions for a chart. Fixed periods show every day (or month) on the axis, played or not. */
function bucketize(cards: SessionCard[], range: Range, lang: Lang): Bucket[] {
  const locale = lang === "ar" ? "ar-IQ" : "en-GB";
  const sorted = [...cards].sort((a, b) => a.at_ms - b.at_ms);
  if (range === "today") {
    const every = Math.ceil(sorted.length / 8);
    return sorted.map((c, i) => ({ label: new Date(c.at_ms).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" }), tick: i % every === 0 || i === sorted.length - 1, cards: [c] }));
  }
  const group = (keyOf: (ms: number) => string) => {
    const m = new Map<string, SessionCard[]>();
    for (const c of sorted) m.set(keyOf(c.at_ms), [...(m.get(keyOf(c.at_ms)) ?? []), c]);
    return m;
  };
  const monthKey = (ms: number) => dayKey(ms).slice(0, 7);
  // Month name only; the year is added in January, where it changes.
  const monthLabel = (k: string) => new Date(`${k}-15T12:00`).toLocaleDateString(locale, k.endsWith("-01") ? { month: "short", year: "numeric" } : { month: "short" });
  if (range === "week" || range === "month") {
    const n = range === "week" ? 7 : 30;
    const byDay = group(dayKey);
    const weekStart = lang === "ar" ? 6 : 1;
    return Array.from({ length: n }, (_, i) => {
      const d = new Date(Date.now() - (n - 1 - i) * 86400000);
      const k = dayKey(d.getTime());
      const label = range === "week" ? d.toLocaleDateString(locale, { weekday: "short", day: "numeric" }) : d.toLocaleDateString(locale, { day: "numeric", month: "short" });
      return { label, tick: range === "week" || d.getDay() === weekStart || i === n - 1, cards: byDay.get(k) ?? [] };
    });
  }
  if (range === "year") {
    const byMonth = group(monthKey);
    const now = new Date();
    return Array.from({ length: 12 }, (_, i) => {
      const k = monthKey(new Date(now.getFullYear(), now.getMonth() - 11 + i, 15).getTime());
      return { label: monthLabel(k), tick: true, cards: byMonth.get(k) ?? [] };
    });
  }
  if (sorted.length === 0) return [];
  const span = (sorted[sorted.length - 1].at_ms - sorted[0].at_ms) / 86400000;
  const byMonth = span > 62;
  const groups = group(byMonth ? monthKey : dayKey);
  return [...groups.entries()].map(([k, cs]) => ({
    label: byMonth ? monthLabel(k) : new Date(`${k}T12:00`).toLocaleDateString(locale, { day: "numeric", month: "short" }),
    tick: true,
    cards: cs,
  }));
}

const accuracyOf = (c: SessionCard) => { const v = sessionScore(c); return v === null ? null : v * 100; };

function Progress({ ctx }: { ctx: Ctx }) {
  const { view, T, lang } = ctx;
  const ids = view.latest?.profile.skills.map((s) => s.skill) ?? [];
  const [picked, setPicked] = useState<string[]>(() => ids.slice(0, 2));
  const [kind, setKind] = useState<Kind>("all");
  const [scenario, setScenario] = useState("all");
  const [range, setRange] = useState<Range>("month");
  const [compare, setCompare] = useState<string[]>([]);
  if (view.sessions.length === 0) {
    return (<><PageHead title={T("التقدم", "Progress")} /><Empty text={T("تقدمك يطلع هنا بعد أول جلسة.", "Your progress appears here after your first session.")} /></>);
  }
  const locale = lang === "ar" ? "ar-IQ" : "en-GB";
  const now = Date.now();
  const since = { today: new Date(new Date().setHours(0, 0, 0, 0)).getTime(), week: now - 7 * 86400000, month: now - 30 * 86400000, year: now - 365 * 86400000, all: 0 }[range];
  const ofKind = view.sessions.filter((s) => kind === "all" || kindOf(s) === kind);
  const scenarios = [...new Set(ofKind.map((s) => s.scenario))];
  const list = ofKind.filter((s) => (scenario === "all" || s.scenario === scenario) && s.at_ms >= since);
  const days = new Set(view.sessions.map((s) => dayKey(s.at_ms)));
  const buckets = bucketize(list, range, lang);
  const posOf = (i: number) => (buckets.length === 1 ? 0.5 : i / (buckets.length - 1));
  const axis = buckets.map((b, i) => ({ pos: posOf(i), label: b.label, tick: b.tick })).filter((t) => t.tick);
  const seriesOf = (f: (c: SessionCard) => number | null, unit: string) => buckets.flatMap((b, i) => {
    const v = avg(b.cards.map(f).filter((x): x is number => x !== null));
    return v === null ? [] : [{ label: b.label, pos: posOf(i), value: v, tip: `${b.label}: ${Math.round(v)} ${unit} · ${b.cards.length} ${T("جلسة", "sessions")}` }];
  });
  const accuracy = seriesOf(accuracyOf, "%");
  const reaction = seriesOf((c) => c.median_reaction_ms, "ms");
  const volume = buckets.map((b) => ({ label: b.label, tick: b.tick, value: Math.round(b.cards.reduce((a, c) => a + c.duration_s, 0) / 60), tip: `${b.label}: ${Math.round(b.cards.reduce((a, c) => a + c.duration_s, 0) / 60)} ${T("دقيقة", "min")} · ${b.cards.length} ${T("جلسة", "sessions")}` }));
  const first = accuracy[0]?.value;
  const last = accuracy[accuracy.length - 1]?.value;
  const reactFirst = reaction[0]?.value;
  const reactLast = reaction[reaction.length - 1]?.value;
  const spanDays = list.length ? (Math.max(...list.map((c) => c.at_ms)) - Math.min(...list.map((c) => c.at_ms))) / 86400000 : 0;
  const unitWord = range === "today" ? T("جلسة", "session") : range === "year" || (range === "all" && spanDays > 62) ? T("شهر", "month") : T("يوم", "day");
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : p.length >= 4 ? p : [...p, id]));
  const series: Series[] = picked.map((id) => ({
    id,
    name: pick(skillNames[id], lang, id),
    points: view.history.map((a, i) => {
      const s = a.profile.skills.find((k) => k.skill === id);
      return { label: `#${i + 1}`, v: s?.score ?? 0, lo: s?.ci95[0] ?? 0, hi: s?.ci95[1] ?? 0 };
    }),
  }));
  const ranges: [Range, string][] = [["today", T("اليوم", "Today")], ["week", T("7 أيام", "7 days")], ["month", T("30 يوم", "30 days")], ["year", T("سنة", "Year")], ["all", T("الكل", "All")]];
  const kinds: [Kind, string][] = [["all", T("كل الجلسات", "All sessions")], ["assessment", T("التقييمات", "Assessments")], ["plan", T("تمارين الخطة", "Plan drills")], ["free", T("التدريب الحر", "Free practice")]];
  const compareRows = compare.map((d) => {
    const cs = view.sessions.filter((s) => dayKey(s.at_ms) === d && (kind === "all" || kindOf(s) === kind) && (scenario === "all" || s.scenario === scenario));
    return {
      d,
      label: new Date(`${d}T12:00`).toLocaleDateString(locale, { weekday: "short", day: "numeric", month: "short" }),
      n: cs.length,
      acc: avg(cs.map(accuracyOf).filter((x): x is number => x !== null)),
      react: avg(cs.map((c) => c.median_reaction_ms).filter((x): x is number => x !== null)),
      hit: avg(cs.map((c) => c.median_time_to_hit_ms).filter((x): x is number => x !== null)),
      min: Math.round(cs.reduce((a, c) => a + c.duration_s, 0) / 60),
    };
  });
  const bestOf = (k: "acc" | "react" | "hit", up: boolean) => {
    const vals = compareRows.map((r) => r[k]).filter((v): v is number => v !== null);
    return vals.length < 2 ? null : up ? Math.max(...vals) : Math.min(...vals);
  };
  const best = { acc: bestOf("acc", true), react: bestOf("react", false), hit: bestOf("hit", false) };

  return (
    <>
      <PageHead title={T("التقدم", "Progress")} sub={T("كل جلسة لعبتها محسوبة هنا. اختار الفترة ونوع الجلسات.", "Every session you played counts here. Pick a period and the kind of sessions.")} />
      <div className="filters card">
        <div className="filter-row">
          <span className="label">{T("الفترة", "Period")}</span>
          <div className="segmented">
            {ranges.map(([r, name]) => <button key={r} className={r === range ? "on" : ""} onClick={() => setRange(r)}>{name}</button>)}
          </div>
          <DayPicker multi available={days} selected={compare} onChange={setCompare} lang={lang} label={compare.length ? T(`قارن ${compare.length} أيام`, `Compare ${compare.length} days`) : T("قارن أيام", "Compare days")} />
        </div>
        <div className="filter-row">
          <span className="label">{T("النوع", "Kind")}</span>
          <div className="segmented">
            {kinds.map(([k, name]) => <button key={k} className={k === kind ? "on" : ""} onClick={() => { setKind(k); setScenario("all"); }}>{name}</button>)}
          </div>
        </div>
        {kind !== "all" && scenarios.length > 1 && (
          <div className="chips">
            <button className={scenario === "all" ? "on" : ""} onClick={() => setScenario("all")}>{T("الكل", "All")}</button>
            {scenarios.map((k) => <button key={k} className={k === scenario ? "on" : ""} onClick={() => setScenario(k)}>{scenarioTitle(k, lang)}</button>)}
          </div>
        )}
      </div>

      {compare.length > 0 && (
        <section className="card">
          <div className="row between">
            <h2>{T("مقارنة الأيام", "Day comparison")}</h2>
            <button className="btn ghost small" onClick={() => setCompare([])}>{T("امسح", "Clear")}</button>
          </div>
          {compare.length === 1 && <p className="muted small">{T("اختار يوم ثاني من التقويم حتى تقارن.", "Pick another day in the calendar to compare.")}</p>}
          <div className="table-wrap">
            <table className="compare">
              <thead><tr><th>{T("اليوم", "Day")}</th><th>{T("جلسات", "Sessions")}</th><th>{T("الدقة", "Accuracy")}</th><th>{T("رد الفعل", "Reaction")}</th><th>{T("وقت الإصابة", "Time to hit")}</th><th>{T("دقايق", "Minutes")}</th></tr></thead>
              <tbody>
                {compareRows.map((r) => (
                  <tr key={r.d}>
                    <td>{r.label}</td>
                    <td>{r.n}</td>
                    <td className={r.acc !== null && r.acc === best.acc ? "best" : ""}>{r.acc === null ? "—" : `${Math.round(r.acc)}%`}</td>
                    <td className={r.react !== null && r.react === best.react ? "best" : ""} dir="ltr">{ms(r.react)}</td>
                    <td className={r.hit !== null && r.hit === best.hit ? "best" : ""} dir="ltr">{ms(r.hit)}</td>
                    <td>{r.min}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {compare.length > 1 && <p className="muted small">{T("الأخضر هو الأحسن بكل عمود.", "Green marks the best in each column.")}</p>}
          {compare.length > 1 && (
            <div className="compare-charts">
              {([
                ["acc", T("الدقة", "Accuracy"), "%", T("أعلى أحسن", "Higher is better")],
                ["react", T("رد الفعل", "Reaction"), "ms", T("أقل أسرع", "Lower is faster")],
                ["hit", T("وقت الإصابة", "Time to hit"), "ms", T("أقل أسرع", "Lower is faster")],
                ["min", T("دقايق التدريب", "Minutes trained"), T("د", "min"), T("كم تدربت", "How long you trained")],
              ] as const).map(([k, title, unit, note]) => (
                <div key={k} className="compare-chart">
                  <div className="row between"><b>{title}</b><small className="muted">{note}</small></div>
                  <ValueBars height={180} unit={unit} max={k === "acc" ? 100 : undefined} rows={compareRows.map((r) => {
                    const v = r[k] ?? 0;
                    return { label: new Date(`${r.d}T12:00`).toLocaleDateString(locale, { day: "numeric", month: "short" }), value: Math.round(v), tip: `${r.label}: ${Math.round(v)} ${unit}` };
                  })} />
                </div>
              ))}
            </div>
          )}
        </section>
      )}

      {list.length === 0 ? (
        <Empty text={range === "today" ? T("ما لعبت اليوم بعد. اختار فترة أطول.", "No sessions today yet. Pick a longer period.") : T("ماكو جلسات بهذي الفترة وهذا النوع.", "No sessions in this period and kind.")} />
      ) : (
        <>
          <div className="stats">
            <Stat value={String(list.length)} label={T("جلسات", "Sessions")} />
            <Stat value={String(list.reduce((a, c) => a + c.hits, 0))} label={T("أهداف أصبتها", "Targets hit")} />
            <Stat value={`${Math.round(list.reduce((a, c) => a + c.duration_s, 0) / 60)}`} label={T("دقايق تدريب", "Minutes trained")} />
            <Stat value={last === undefined ? "—" : `${Math.round(last)}%`} label={T("الدقة بآخر الفترة", "Accuracy at the end")} hint={first !== undefined && accuracy.length > 1 ? `${last! >= first ? "+" : ""}${Math.round(last! - first)} ${T("عن بدايتها", "vs the start")}` : undefined} />
          </div>
          <section className="card">
            <h2>{T("الدقة", "Accuracy")} <small className="muted">· {T(`كل نقطة ${unitWord}`, `each point is a ${unitWord}`)}</small></h2>
            {accuracy.length < 2 && <p className="muted small">{T("تحتاج نقطتين على الأقل حتى يطلع خط. اختار فترة أطول أو العب أكثر.", "At least two points are needed for a line. Pick a longer period or play more.")}</p>}
            <TrendLine points={accuracy} unit="%" better="up" axis={axis} />
          </section>
          <div className="split-2">
            <section className="card">
              <h2>{T("رد الفعل", "Reaction")}</h2>
              {reaction.length === 0 ? <p className="muted small">{T("ماكو أوقات موثوقة بهذي الفترة. التتبع ما بيه رد فعل.", "No reliable timing in this period. Tracking has no reaction time.")}</p> : (
                <>
                  <p className="muted small">{reaction.length > 1 && reactFirst !== undefined && reactLast !== undefined
                    ? reactLast <= reactFirst
                      ? T(`صرت أسرع بـ ${Math.round(reactFirst - reactLast)} ms. أقل أحسن.`, `${Math.round(reactFirst - reactLast)} ms faster. Lower is better.`)
                      : T(`صرت أبطأ بـ ${Math.round(reactLast - reactFirst)} ms. يمكن تعب أو قلة نوم.`, `${Math.round(reactLast - reactFirst)} ms slower. Fatigue or sleep can do this.`)
                    : T("أقل أحسن.", "Lower is better.")}</p>
                  <TrendLine points={reaction} unit="ms" better="down" axis={axis} height={220} />
                </>
              )}
            </section>
            <section className="card">
              <h2>{T("دقايق التدريب", "Minutes trained")}</h2>
              <p className="muted small">{T(`المعدل ${Math.round(avg(volume.map((v) => v.value)) ?? 0)} دقيقة لكل ${unitWord}. التدريب القصير المنتظم أحسن من الطويل المتقطع.`, `Average ${Math.round(avg(volume.map((v) => v.value)) ?? 0)} minutes per ${unitWord}. Short regular practice beats long irregular sessions.`)}</p>
              <ValueBars rows={volume} unit={T("د", "min")} height={220} />
            </section>
          </div>
        </>
      )}
      {view.latest && (
        <section className="card">
          <h2>{T("درجات مهاراتك الآن", "Your skill scores now")}</h2>
          {[...view.latest.profile.skills].sort((a, b) => a.score - b.score).map((s) => {
            const color = s.score >= 60 ? "var(--good)" : s.score >= 40 ? "var(--warn)" : "var(--bad)";
            return (
              <div className="skill" key={s.skill}>
                <div>{pick(skillNames[s.skill], lang, s.skill)}{s.reliability === "Low" && <small className="muted">{T("بيانات قليلة", "little data")}</small>}</div>
                <div className="track" title={`${s.ci95[0].toFixed(0)}–${s.ci95[1].toFixed(0)}`}>
                  <div className="ci" style={{ left: `${s.ci95[0]}%`, width: `${Math.max(s.ci95[1] - s.ci95[0], 1)}%` }} />
                  <div className="val" style={{ left: `${s.score}%`, background: color }} />
                </div>
                <div className="num">{s.score.toFixed(0)}</div>
              </div>
            );
          })}
          <p className="small muted">{T("الشريط الفاتح هو نطاق الثقة. الدرجات أولية لحد ما تتجمع بيانات لاعبين.", "The light band is the confidence range. Scores are provisional until player data exists.")}</p>
        </section>
      )}
      {view.history.length > 0 && (
        <section className="card">
          <h2>{T("مهاراتك بالتقييمات", "Your skills across assessments")}</h2>
          <div className="chips">
            {ids.map((id) => <button key={id} className={picked.includes(id) ? "on" : ""} onClick={() => toggle(id)}>{pick(skillNames[id], lang, id)}</button>)}
          </div>
          {series.length > 1 && <Legend items={series.map((s, i) => [SERIES[i % SERIES.length], s.name])} />}
          {view.history.length === 1 && <p className="muted small">{T("تحتاج تقييمين حتى يطلع منحنى.", "Two assessments are needed for a curve.")}</p>}
          <SkillLines series={series} />
        </section>
      )}
    </>
  );
}

function PlanPage({ ctx }: { ctx: Ctx }) {
  const { view, T, act, lang, go } = ctx;
  const plan = view.plan;
  return (
    <>
      <PageHead title={T("خطة التدريب", "Training plan")} />
      {!plan && <Empty text={T("الخطة تتبنى من نتائج تقييمك.", "Your plan is built from your assessment.")} action={<button className="btn" onClick={() => go("home")}>{T("ابدأ التقييم", "Start assessment")}</button>} />}
      {plan && <PlanBody ctx={ctx} plan={plan} />}
      {plan && plan.state !== "Completed" && <HowToImprove ctx={ctx} plan={plan} />}
      {plan && plan.focus.length > 0 && (
        <section className="card">
          <h2>{T("ليش هذي الخطة", "Why this plan")}</h2>
          {plan.focus.map((f) => (
            <div className="finding" key={f.id}>
              <b>{f.headline}</b>
              <p className="muted">{f.why}</p>
              <div className="evidence">
                {f.evidence.map(([k, v]) => {
                  if (k === "score") return <span key={k}><b>{v.toFixed(0)}</b>{T("درجتك بهذي المهارة", "Your score in this skill")}</span>;
                  const e = evidenceNames[k];
                  return <span key={k}><b>{e?.pct ? `${(v * 100).toFixed(0)}%` : `${v.toFixed(2)}${e?.unit ? ` ${e.unit}` : ""}`}</b>{pick(e?.name, lang, k)}</span>;
                })}
              </div>
            </div>
          ))}
        </section>
      )}
      {view.block_result && plan && typeof plan.state === "object" && "Evaluated" in plan.state && (
        <section className="card">
          <h2>{T("نتيجة الكتلة", "Block result")}</h2>
          {view.block_result.conditions_changed && view.block_result.settings_before[0] === 0 && <div className="banner">{T("إعدادات الماوس اختلفت بين الاختبارين، فالمقارنة غير دقيقة.", "Mouse settings differ between the tests, so the comparison is unreliable.")}</div>}
          {view.block_result.conditions_changed && view.block_result.settings_before[0] > 0 && <div className="banner">{T(`إعدادات الماوس اختلفت بين الاختبارين: DPI ${view.block_result.settings_before[0]} وحساسية ${view.block_result.settings_before[1]} قبل، و DPI ${view.block_result.settings_after[0]} وحساسية ${view.block_result.settings_after[1]} بعد. المقارنة غير دقيقة.`, `Mouse settings differ between the tests: ${view.block_result.settings_before[0]} DPI / ${view.block_result.settings_before[1]} before, ${view.block_result.settings_after[0]} DPI / ${view.block_result.settings_after[1]} after. The comparison is unreliable.`)}</div>}
          {(view.block_result.changed_tests?.length ?? 0) > 0 && (
            <div className="banner">
              {T(
                `تصميم ${view.block_result.changed_tests!.length === 1 ? "اختبار" : "اختبارات"} تغيّر بين التقييم الأول وإعادة الاختبار: ${view.block_result.changed_tests!.map((id) => scenarioTitle(id, lang)).join("، ")}. نتائجها ما تنقارن بدقة، لأن المسافات صارت مختلفة. اعتبر التغيير بهذي المهارات تقريبي، والتقييم الجاي يكون أساس جديد صحيح.`,
                `The design of ${view.block_result.changed_tests!.length === 1 ? "a test" : "some tests"} changed between your first assessment and the retest: ${view.block_result.changed_tests!.map((id) => scenarioTitle(id, lang)).join(", ")}. Their results are not directly comparable because the distances differ. Treat the change in those skills as approximate; your next assessment becomes a clean baseline.`,
              )}
            </div>
          )}
          {view.block_result.training_effect !== null && (
            <div className="stats"><Stat value={`${view.block_result.training_effect > 0 ? "+" : ""}${view.block_result.training_effect.toFixed(1)}`} label={T("أثر التدريب", "Training effect")} hint={T("تحسن المهارات المدربة ناقص غيرها", "Trained skills minus the rest")} /></div>
          )}
          {view.block_result.rows.length === 0 && <p className="muted">{T("ما لكينا مهارات تنقارن بين الاختبارين. سوِّ إعادة اختبار كاملة.", "No skills could be compared between the tests. Take a full retest.")}</p>}
          {view.block_result.rows.length > 0 && <table>
            <thead><tr><th>{T("المهارة", "Skill")}</th><th>{T("قبل", "Before")}</th><th>{T("بعد", "After")}</th><th>{T("التغيير", "Change")}</th><th /></tr></thead>
            <tbody>
              {view.block_result.rows.map((r) => (
                <tr key={r.skill}>
                  <td>{pick(skillNames[r.skill], lang, r.skill)} {r.trained && <span className="tag good">{T("مركّز عليها", "Focus")}</span>}</td>
                  <td>{r.pre.toFixed(0)}</td><td>{r.post.toFixed(0)}</td>
                  <td className={r.delta >= 0 ? "up" : "down"}>{r.delta > 0 ? "+" : ""}{r.delta.toFixed(0)}</td>
                  <td className="small">{r.verdict_text}</td>
                </tr>
              ))}
            </tbody>
          </table>}
          {typeof plan?.state === "object" && "Evaluated" in plan.state && <p><button className="btn" onClick={() => act("next_block")}>{T("ابدأ الكتلة الجاية", "Start next block")}</button></p>}
        </section>
      )}
    </>
  );
}

/** What the player does to improve: the rhythm of the plan and what to practise for each problem. */
function HowToImprove({ ctx, plan }: { ctx: Ctx; plan: Plan }) {
  const { T, lang, go } = ctx;
  const steps: [string, string][] = [
    [T("جلسة وحدة باليوم", "One session a day"), T("حوالي 16 دقيقة، ويفضل بنفس الوقت. ابدأ بالإحماء وبعدين التمارين بالترتيب.", "About 16 minutes, ideally at the same time. Warm up first, then the drills in order.")],
    [T("الجودة قبل السرعة", "Quality before speed"), T("كل تمرين بيه قاعدة (مثلاً لا تتجاوز الهدف). الإصابة اللي تكسر القاعدة ما تنحسب، فركز على الحركة الصحيحة.", "Each drill has a rule (for example, do not pass the target). Hits that break it do not count, so focus on the right movement.")],
    [T("الراحة جزء من التدريب", "Rest is part of training"), T("المخ يثبت الحركة وانت نايم. جلستين بنفس اليوم ما تسرّع التحسن.", "Your brain locks in movement while you sleep. Two plan sessions in one day do not speed it up.")],
    [T("إعادة الاختبار تقيس الحقيقة", "The retest tells the truth"), T(`بعد ${plan.block_days} أيام تسوي نفس الاختبارات بنموذج جديد، ونقارن بالتقييم الأول حتى نشوف هل تحسنت فعلاً.`, `After ${plan.block_days} days you take the same tests in a new form, compared with your first assessment to see real improvement.`)],
  ];
  return (
    <section className="card">
      <h2>{T("شلون تتحسن", "How to improve")}</h2>
      <ol className="how">
        {steps.map(([title, body], i) => <li key={i}><b>{title}</b><span className="muted">{body}</span></li>)}
      </ol>
      {plan.focus.length > 0 && (
        <>
          <h3 className="sub">{T("شنو تركز عليه", "What to focus on")}</h3>
          <div className="focus-list">
            {plan.focus.map((f) => {
              const d = f.drills[0];
              return (
                <div className="focus-item" key={f.id}>
                  <div className="grow">
                    <b>{f.headline}</b>
                    <small className="muted">{f.why}</small>
                    {d && <small>{T("التمرين: ", "Drill: ")}<b>{pick(drillNames[d], lang, d)}</b> · {pick(drillInfo[d]?.purpose, lang, "")}</small>}
                  </div>
                  {d && <button className="btn ghost small" onClick={() => go("practice", `drill:${d}`)}>{T("تدرب عليه حر", "Practise freely")}</button>}
                </div>
              );
            })}
          </div>
          <p className="muted small">{T("التدريب الحر ما يأثر على الخطة، بس يزيد التكرار. لا تبالغ بيه بأيام الراحة.", "Free practice does not change the plan; it only adds repetitions. Do not overdo it on rest days.")}</p>
        </>
      )}
    </section>
  );
}

function PlanBody({ ctx, plan }: { ctx: Ctx; plan: Plan }) {
  const { T, act, lang, view } = ctx;
  const state = plan.state;
  if (state === "Draft") {
    return (
      <section className="card hero">
        <h2>{T("خطة 6 أيام جاهزة", "A 6-day plan is ready")}</h2>
        <p className="muted">{T("كل يوم حوالي 16 دقيقة، وتركز على أهم مشكلتين بس.", "About 16 minutes a day, focused only on your top two problems.")}</p>
        <div className="evidence spaced-b">
          {plan.focus.flatMap((f) => f.drills.slice(0, 1)).map((d) => <span key={d}><b>{pick(drillNames[d], lang, d)}</b>{pick(drillInfo[d]?.purpose, lang, "")}</span>)}
        </div>
        <button className="btn" onClick={() => act("accept_plan")}>{T("ابدأ الخطة", "Start plan")}</button>
      </section>
    );
  }
  if (state === "Completed") return <Empty text={T("خلصت الخطة.", "Plan finished.")} />;
  if ("Paused" in state) {
    return <section className="card hero"><h2>{T("غبت أكثر من 4 أيام", "You were away for over 4 days")}</h2><button className="btn" onClick={() => act("accept_plan")}>{T("كمل من وين وقفت", "Continue where you stopped")}</button></section>;
  }
  if ("AwaitingRetest" in state) {
    return (
      <section className="card hero">
        <h2>{T("وقت إعادة الاختبار", "Time to retest")}</h2>
        <div className="days">{Array.from({ length: plan.block_days }, (_, i) => <i key={i} className="on" />)}</div>
        <p>{T(`خلصت ${plan.block_days} أيام تدريب. هسه نقيس هل تحسنت المهارات اللي ركزنا عليها، ونقارنها بالمهارات الثانية حتى نعرف إن التحسن من التدريب مو صدفة.`, `You finished ${plan.block_days} training days. Now we measure whether the skills we focused on improved, against the others, so we know the change came from training and not chance.`)}</p>
        <p className="muted">{plan.retest_ready ? T("نفس الاختبارات بنموذج جديد، حتى نقيس التحسن الحقيقي.", "The same tests in a new form, to measure real improvement.") : T("يفتح بعد يوم راحة. الراحة جزء من التعلم.", "Opens after a rest day. Rest is part of learning.")}</p>
        <button className="btn" disabled={!plan.retest_ready || !view.trainer_found} onClick={() => act("run_retest")}>{view.resume ? T(`كمل (${view.resume.done} من ${view.resume.total})`, `Continue (${view.resume.done} of ${view.resume.total})`) : T("ابدأ", "Start")}</button>
      </section>
    );
  }
  if (!("Active" in state)) return null;
  const { block, day } = state.Active;
  const minutes = plan.today_items.reduce((a, b) => a + b.minutes, 0);
  return (
    <section className="card">
      <div className="row between">
        <div>
          <span className="label">{T("الكتلة", "Block")} {block}</span>
          <h2 className="tight">{T(`اليوم ${day} من ${plan.block_days}`, `Day ${day} of ${plan.block_days}`)} · {minutes} {T("دقيقة", "min")}</h2>
        </div>
        <button className="btn" disabled={!view.trainer_found || plan.today_done || plan.item_index >= plan.today_items.length} onClick={() => act("run_next_item")}>{plan.today_done ? T("خلصت اليوم، تعال باجر", "Done for today") : T("شغّل التمرين الجاي", "Play next drill")}</button>
      </div>
      <div className="days">{Array.from({ length: plan.block_days }, (_, i) => <i key={i} className={i < day - 1 ? "on" : i === day - 1 ? "now" : ""} />)}</div>
      <ol className="steps">
        {plan.today_items.map((item, i) => (
          <li key={i} className={i < plan.item_index ? "done" : i === plan.item_index ? "now" : ""}>
            <span className="dot">{i < plan.item_index ? "✓" : i + 1}</span>
            <span className="grow">
              <b>{pick(drillNames[item.drill], lang, item.drill)}</b>
              <small>{pick(drillInfo[item.drill]?.purpose, lang, "")}</small>
            </span>
            {plan.levels[item.drill] !== undefined && <LevelBadge ctx={ctx} drill={item.drill} level={plan.levels[item.drill]} />}
            <span className="muted small">{item.minutes} {T("د", "min")}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

/** The drill level and how far the last session moved it. */
function LevelBadge({ ctx, drill, level }: { ctx: Ctx; drill: string; level: number }) {
  const last = [...ctx.view.practice].reverse().find((p) => p.name === drill && p.level_before != null && p.difficulty != null);
  const change = last ? Math.round((last.difficulty! - last.level_before!) * 100) : 0;
  return (
    <span className="muted small">
      {ctx.T("المستوى", "Level")} {(level * 100).toFixed(0)}
      {change !== 0 && <b className={change > 0 ? "up" : "down"}> {change > 0 ? "▲" : "▼"}{Math.abs(change)}</b>}
    </span>
  );
}

const MODES: { id: CustomTargets["mode"]; name: [string, string]; desc: [string, string] }[] = [
  { id: "flick", name: ["فليك", "Flick"], desc: ["هدف واحد بكل مرة بمسافات مختلفة.", "One target at a time at varied distances."] },
  { id: "multi", name: ["أهداف متعددة", "Multi target"], desc: ["عدة أهداف سوه وتبدل بينها.", "Several at once; switch between them."] },
  { id: "peek", name: ["ظهور من الجدار", "Wall peek"], desc: ["أهداف تطلع من حواف الجدران.", "Targets emerge from wall edges."] },
  { id: "track", name: ["تتبع سلس", "Smooth track"], desc: ["ابقى على هدف يتحرك بانسيابية.", "Stay on a smoothly moving target."] },
  { id: "strafe", name: ["تتبع strafe", "Strafe track"], desc: ["هدف يغير اتجاهه فجأة.", "A target that reverses suddenly."] },
];

type Tier = "easy" | "medium" | "hard" | "pro";
const TIERS: { id: Tier; name: [string, string]; p: Pick<CustomTargets, "radius_deg" | "min_distance_deg" | "max_distance_deg" | "speed_deg_s" | "time_limit_s"> }[] = [
  { id: "easy", name: ["سهل", "Easy"], p: { radius_deg: 1.2, min_distance_deg: 8, max_distance_deg: 20, speed_deg_s: 12, time_limit_s: 3 } },
  { id: "medium", name: ["متوسط", "Medium"], p: { radius_deg: 0.8, min_distance_deg: 10, max_distance_deg: 30, speed_deg_s: 18, time_limit_s: 2 } },
  { id: "hard", name: ["صعب", "Hard"], p: { radius_deg: 0.55, min_distance_deg: 15, max_distance_deg: 40, speed_deg_s: 24, time_limit_s: 1.2 } },
  { id: "pro", name: ["محترف", "Pro"], p: { radius_deg: 0.4, min_distance_deg: 20, max_distance_deg: 50, speed_deg_s: 30, time_limit_s: 0.8 } },
];

interface Preset { name: string; targets: CustomTargets }
const PRESETS_KEY = "aimforge.presets";
function readPresets(): Preset[] {
  try { return JSON.parse(localStorage.getItem(PRESETS_KEY) ?? "[]"); } catch { return []; }
}
function writePresets(presets: Preset[]) {
  try { localStorage.setItem(PRESETS_KEY, JSON.stringify(presets)); } catch { /* presets stay for this visit */ }
}

/** A drill from the library expressed as builder settings, at middle difficulty. The drill itself also
 * has rules (for example, no overshooting) that only the original keeps. */
const BASE: CustomTargets = { mode: "flick", concurrent: 3, duration_s: 60, area_h_deg: 35, area_v_deg: 12, reversal_s: 0.55, ...TIERS[1].p };
const DRILL_SETUPS: Record<string, Partial<CustomTargets>> = {
  warmup: { radius_deg: 1, min_distance_deg: 10, max_distance_deg: 20, time_limit_s: 3.5 },
  deceleration_ladder: { radius_deg: 0.8, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 1.2 },
  commit_flick: { radius_deg: 1.1, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 0.9 },
  consistency_grid: { radius_deg: 0.75, min_distance_deg: 15, max_distance_deg: 25, time_limit_s: 2 },
  directional_flick: { radius_deg: 0.75, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 2 },
  speed_push: { radius_deg: 0.8, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 0.7 },
  micro_hold: { radius_deg: 0.45, min_distance_deg: 3, max_distance_deg: 6, time_limit_s: 3 },
  precision_first: { radius_deg: 0.45, min_distance_deg: 3, max_distance_deg: 6, time_limit_s: 3 },
  confirm_click: { radius_deg: 0.75, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 2.5 },
  timed_kill: { radius_deg: 0.8, min_distance_deg: 18, max_distance_deg: 30, time_limit_s: 0.7 },
  reflex_dot: { radius_deg: 0.8, min_distance_deg: 4, max_distance_deg: 4, time_limit_s: 0.65 },
  cs_drill: { radius_deg: 0.9, min_distance_deg: 8, max_distance_deg: 20, time_limit_s: 2.2 },
  head_level_hold: { mode: "peek", radius_deg: 0.55, time_limit_s: 1.8 },
  peek_line: { mode: "peek", radius_deg: 0.55, time_limit_s: 1.8 },
  smooth_track: { mode: "track", radius_deg: 0.7, speed_deg_s: 18, area_h_deg: 35, area_v_deg: 6 },
  strafe_track_reactive: { mode: "strafe", radius_deg: 0.7, speed_deg_s: 21, reversal_s: 0.6, area_h_deg: 35 },
};
const setupOf = (drill: string): CustomTargets => onScreen({ ...BASE, ...DRILL_SETUPS[drill] });

function Practice({ ctx, initial }: { ctx: Ctx; initial?: string }) {
  const { view, T, act, lang, go } = ctx;
  const fromPlan = initial?.startsWith("drill:") ? initial.slice(6) : null;
  const [c, setC] = useState<CustomTargets>(() => (fromPlan && DRILL_SETUPS[fromPlan] ? setupOf(fromPlan) : BASE));
  const [tier, setTier] = useState<Tier | null>(fromPlan ? null : "medium");
  const [source, setSource] = useState<string | null>(fromPlan);
  const [section, setSection] = useState<"build" | "library">("build");
  const [presets, setPresets] = useState<Preset[]>(readPresets);
  const [presetName, setPresetName] = useState("");
  const addPreset = () => {
    const name = presetName.trim();
    if (!name) return;
    const next = [...presets.filter((p) => p.name !== name), { name, targets: c }];
    setPresets(next);
    writePresets(next);
    setPresetName("");
  };
  const removePreset = (name: string) => {
    const next = presets.filter((p) => p.name !== name);
    setPresets(next);
    writePresets(next);
  };
  const set = (patch: Partial<CustomTargets>) => { setTier(null); setC(onScreen({ ...c, ...patch })); };
  const loadDrill = (d: string) => { setC(setupOf(d)); setTier(null); setSource(d); setSection("build"); };
  const usesDistance = c.mode === "flick" || c.mode === "multi";
  const usesSpeed = c.mode === "track" || c.mode === "strafe";
  const usesLimit = c.mode === "flick" || c.mode === "peek";
  const usesArea = c.mode === "multi" || c.mode === "track" || c.mode === "strafe";
  const history = view.practice.filter((p) => p.mode === c.mode);
  const best = history.length ? Math.max(...history.map((p) => p.hit_rate)) : null;
  const modeAvg = avg(history.map((p) => p.hit_rate));
  const modeTrend = history.map((p, i) => ({ label: p.at_ms ? new Date(p.at_ms).toLocaleDateString(lang === "ar" ? "ar-IQ" : "en-GB", { day: "numeric", month: "short" }) : `#${i + 1}`, value: p.hit_rate * 100 }));
  const reach = c.mode === "flick" ? ON_SCREEN_V_DEG : ON_SCREEN_H_DEG;
  const distMax = Math.floor(reach - c.radius_deg);
  const groups = (Object.keys(groupNames) as DrillInfo["group"][]).map((g) => ({ g, drills: Object.keys(drillInfo).filter((d) => drillInfo[d].group === g) }));

  return (
    <>
      <PageHead title={T("التدريب الحر", "Free practice")}>
        <div className="seg">
          <button className={section === "build" ? "on" : ""} onClick={() => setSection("build")}>{T("ابني جلستك", "Build a session")}</button>
          <button className={section === "library" ? "on" : ""} onClick={() => setSection("library")}>{T("مكتبة التمارين", "Drill library")}</button>
        </div>
      </PageHead>
      {section === "build" && (
        <>
          <div className="modes">
            {MODES.map((m) => (
              <button key={m.id} className={c.mode === m.id ? "on" : ""} onClick={() => { setSource(null); setC(onScreen({ ...c, mode: m.id })); }}>
                <b>{pick(m.name, lang, m.id)}</b>
                <small>{pick(m.desc, lang, "")}</small>
              </button>
            ))}
          </div>
          {source && (
            <div className="banner row between">
              <span>{T(`محمّلة من تمرين «${pick(drillNames[source], lang, source)}». غيّر اللي تريده، أو العب التمرين الأصلي بقواعده.`, `Loaded from the “${pick(drillNames[source], lang, source)}” drill. Change what you like, or play the original with its rules.`)}</span>
              <button className="btn ghost small" disabled={!view.trainer_found} onClick={() => act("run_scenario", { scenario: source })}>{T("العب الأصلي", "Play original")}</button>
            </div>
          )}
          <div className="builder">
            <section className="card">
              <span className="label">{T("المعاينة بنفس قياسات اللعبة", "Preview at true game scale")}</span>
              <TargetPreview c={c} />
              <div className="row between foot">
                <div>
                  <span className="label">{T("أفضل نتيجة", "Best")}</span>
                  <b className="mid">{best === null ? "—" : `${(best * 100).toFixed(0)}%`}</b>
                </div>
                <Spark values={history.map((p) => p.hit_rate)} />
                <button className="btn big" disabled={!view.trainer_found} onClick={() => act("run_custom", { targets: c })}>{T("ابدأ الجلسة", "Start session")}</button>
              </div>
            </section>
            <section className="card">
              <span className="label">{T("الصعوبة", "Difficulty")}</span>
              <div className="seg full">
                {TIERS.map((t) => <button key={t.id} className={tier === t.id ? "on" : ""} onClick={() => { setTier(t.id); setC(onScreen({ ...c, ...t.p })); }}>{pick(t.name, lang, t.id)}</button>)}
              </div>
              <Slider label={T("حجم الهدف", "Target size")} value={c.radius_deg} min={0.2} max={2} step={0.05} unit="°" set={(radius_deg) => set({ radius_deg })} />
              {usesDistance && <Slider label={T("أقرب هدف من المركز", "Nearest to centre")} value={c.min_distance_deg} min={2} max={distMax} step={1} unit="°" set={(v) => set({ min_distance_deg: v, max_distance_deg: Math.max(v, c.max_distance_deg) })} />}
              {usesDistance && <Slider label={T("أبعد هدف من المركز", "Farthest from centre")} value={c.max_distance_deg} min={2} max={distMax} step={1} unit="°" set={(v) => set({ max_distance_deg: v, min_distance_deg: Math.min(v, c.min_distance_deg) })} />}
              {usesDistance && <small className="muted hint">{T(`أبعد مسافة ${distMax}° حتى يبقى الهدف داخل الشاشة${c.mode === "flick" ? " حتى لو طلع فوق أو جوه" : ""}.`, `At most ${distMax}° so the target stays on screen${c.mode === "flick" ? ", even straight up or down" : ""}.`)}</small>}
              {usesArea && <Slider label={T("مساحة الحركة يمين ويسار", "Area left and right")} value={c.area_h_deg} min={5} max={Math.floor(ON_SCREEN_H_DEG - c.radius_deg)} step={1} unit="°" set={(area_h_deg) => set({ area_h_deg })} />}
              {usesArea && c.mode !== "strafe" && <Slider label={T("مساحة الحركة فوق وجوه", "Area up and down")} value={c.area_v_deg} min={2} max={Math.floor(ON_SCREEN_V_DEG - c.radius_deg)} step={1} unit="°" set={(area_v_deg) => set({ area_v_deg })} />}
              {c.mode === "strafe" && <Slider label={T("كل كم يغير اتجاهه", "Changes direction every")} value={c.reversal_s} min={0.25} max={1.5} step={0.05} unit="s" set={(reversal_s) => set({ reversal_s })} />}
              {usesSpeed && <Slider label={T("سرعة الهدف", "Target speed")} value={c.speed_deg_s} min={5} max={40} step={1} unit="°/s" set={(speed_deg_s) => set({ speed_deg_s })} />}
              {usesLimit && <Slider label={T("وقت كل هدف", "Time per target")} value={c.time_limit_s} min={0.3} max={5} step={0.1} unit="s" set={(time_limit_s) => set({ time_limit_s })} />}
              {c.mode === "multi" && <Slider label={T("عدد الأهداف سوه", "Targets at once")} value={c.concurrent} min={2} max={6} step={1} unit="" set={(concurrent) => set({ concurrent })} />}
              <Slider label={T("مدة الجلسة", "Session length")} value={c.duration_s} min={15} max={300} step={15} unit="s" set={(duration_s) => set({ duration_s })} />
          <div className="presets-inline">
            <span className="label">{T("إعداداتك المحفوظة", "Your saved setups")}</span>
            <div className="chips">
              {presets.map((p) => (
                <span className="preset" key={p.name}>
                  <button onClick={() => { setTier(null); setC(p.targets); }}>{p.name}</button>
                  <button aria-label={T("احذف", "Delete")} onClick={() => removePreset(p.name)}>×</button>
                </span>
              ))}
            </div>
            <form className="row composer-sm" onSubmit={(e) => { e.preventDefault(); addPreset(); }}>
              <input className="grow" value={presetName} placeholder={T("اسم لهذي الإعدادات", "Name this setup")} onChange={(e) => setPresetName(e.target.value)} />
              <button className="btn ghost" disabled={!presetName.trim()}>{T("احفظ", "Save")}</button>
            </form>
          </div>
            </section>
          </div>
          {history.length > 0 && (
            <section className="card">
              <h2>{T(`تقدمك بـ ${pick(MODES.find((m) => m.id === c.mode)?.name, lang, c.mode)}`, `Your progress in ${pick(MODES.find((m) => m.id === c.mode)?.name, lang, c.mode)}`)}</h2>
              <div className="stats">
                <Stat value={String(history.length)} label={T("جلسات", "Sessions")} />
                <Stat value={pct(modeAvg)} label={T("المعدل", "Average")} />
                <Stat value={pct(best)} label={T("أحسن نتيجة", "Best")} />
                <Stat value={pct(history[history.length - 1].hit_rate)} label={T("آخر جلسة", "Last session")} hint={history.length > 1 ? `${history[history.length - 1].hit_rate >= history[0].hit_rate ? "+" : "−"}${Math.abs(Math.round((history[history.length - 1].hit_rate - history[0].hit_rate) * 100))} ${T("عن أول جلسة", "vs first")}` : undefined} />
              </div>
              {modeTrend.length > 1 ? <TrendLine points={modeTrend} unit="%" better="up" /> : <p className="muted small">{T("العب جلسة ثانية بهذا النوع حتى يطلع الخط.", "Play another session in this mode to see a line.")}</p>}
              <p className="muted small">{T("النتيجة نسبة الإصابة. غيّر الإعدادات بحذر، لأن الأرقام بين إعدادات مختلفة ما تنقارن تماماً.", "The score is the hit rate. Settings changes make sessions less comparable.")}</p>
              <details className="recent">
                <summary>{T(`آخر جلساتك بهذا النوع (${Math.min(history.length, 5)})`, `Your recent sessions in this mode (${Math.min(history.length, 5)})`)}</summary>
                {history.slice(-5).reverse().map((p, i) => (
                  <button className="line" key={i} onClick={() => p.file && go("analysis", p.file)}>
                    <span><b>{(p.hit_rate * 100).toFixed(0)}%</b><small>{whenText(p.at_ms ?? 0, lang)}</small></span>
                    <span className="chev">›</span>
                  </button>
                ))}
              </details>
            </section>
          )}
        </>
      )}
      {section === "library" && (
        <p className="muted">{T("اضغط على أي تمرين حتى تنفتح إعداداته بـ «ابني جلستك» وتعدّلها، أو العبه مباشرة بقواعده الأصلية.", "Click a drill to open its settings in “Build a session” and adjust them, or play it straight away with its own rules.")}</p>
      )}
      {section === "library" && groups.map(({ g, drills }) => (
        <section key={g}>
          <h2 className="group">{pick(groupNames[g], lang, g)}</h2>
          <div className="library">
            {drills.map((d) => {
              const level = view.plan?.levels[d];
              const runs = view.practice.filter((p) => p.name === d).length;
              const inPlan = view.plan?.focus.some((f) => f.drills.includes(d));
              return (
                <article key={d} className="drill-card">
                  <button className="drill-open" onClick={() => loadDrill(d)} aria-label={T(`افتح ${pick(drillNames[d], lang, d)} بالمُنشئ`, `Open ${pick(drillNames[d], lang, d)} in the builder`)}>
                    <TargetPreview c={setupOf(d)} still />
                    <span className="drill-body">
                      <span className="row between">
                        <b>{pick(drillNames[d], lang, d)}</b>
                        {inPlan && <span className="tag good">{T("بخطتك", "In your plan")}</span>}
                      </span>
                      <small>{pick(drillInfo[d].purpose, lang, "")}</small>
                      <span className="muted small">{level !== undefined ? `${T("مستواك", "Your level")} ${(level * 100).toFixed(0)}` : T("ما لعبته بالخطة بعد", "Not played in your plan yet")}{runs > 0 ? ` · ${T("لعبته", "played")} ${runs}×` : ""}</span>
                    </span>
                  </button>
                  <div className="drill-actions">
                    <button className="btn ghost small" onClick={() => loadDrill(d)}>{T("عدّل الإعدادات", "Adjust settings")}</button>
                    <button className="btn small" disabled={!view.trainer_found} onClick={() => act("run_scenario", { scenario: d })}>{T("العب", "Play")}</button>
                  </div>
                </article>
              );
            })}
          </div>
        </section>
      ))}
    </>
  );
}

function Slider(props: { label: string; value: number; min: number; max: number; step: number; unit: string; set: (v: number) => void }) {
  const decimals = props.step < 1 ? (props.step < 0.1 ? 2 : 1) : 0;
  return (
    <label className="slider">
      <span className="row between"><span>{props.label}</span><b dir="ltr">{props.value.toFixed(decimals)} {props.unit}</b></span>
      <input type="range" min={props.min} max={props.max} step={props.step} value={props.value} onChange={(e) => props.set(e.target.valueAsNumber)} />
    </label>
  );
}

const AI_SOURCES: { id: string; name: string; model: string; needsKey: boolean }[] = [
  { id: "template", name: "", model: "", needsKey: false },
  { id: "claude", name: "Claude", model: "claude-haiku-5-5", needsKey: true },
  { id: "openai", name: "OpenAI", model: "gpt-4o-mini", needsKey: true },
  { id: "gemini", name: "Gemini", model: "gemini-2.5-flash", needsKey: true },
  { id: "openrouter", name: "OpenRouter", model: "openrouter/auto", needsKey: true },
  { id: "custom", name: "", model: "", needsKey: false },
];

/** Drills a report names, found by their display names in either language. */
function drillsIn(texts: string[]): string[] {
  const found = Object.keys(drillNames).filter((id) => texts.some((t) => drillNames[id].some((n) => t.includes(n))));
  return found.slice(0, 3);
}

function ReportLinks({ ctx, report }: { ctx: Ctx; report: View["reports"][number] }) {
  const { view, T, lang, go } = ctx;
  const drills = drillsIn([...report.reply.next_steps, ...report.reply.weaknesses.map((w) => `${w.fix ?? ""} ${w.title}`)]);
  const latest = view.sessions[0];
  return (
    <section className="report-links">
      <h2>{T("كمل من هنا", "Take it from here")}</h2>
      <div className="link-grid">
        {drills.map((d) => (
          <button key={d} className="link-card" onClick={() => go("practice", `drill:${d}`)}>
            <b>{T("جهّز تدريب حر", "Set up free practice")}</b>
            <small>{pick(drillNames[d], lang, d)} · {pick(drillInfo[d]?.purpose, lang, "")}</small>
          </button>
        ))}
        {view.plan && (
          <button className="link-card" onClick={() => go("plan")}>
            <b>{T("افتح خطتك", "Open your plan")}</b>
            <small>{T("التقرير يكمّل الخطة، والخطة هي اللي تقيس التحسن.", "The report supports the plan; the plan measures the improvement.")}</small>
          </button>
        )}
        {report.scope === "latest" && latest && (
          <button className="link-card" onClick={() => go("analysis", latest.path)}>
            <b>{T("شوف الجلسة بالتفصيل", "See the session in detail")}</b>
            <small>{T("كل حركة ورصاصة بالتحليل.", "Every movement and shot in Analysis.")}</small>
          </button>
        )}
        {report.scope !== "latest" && (
          <button className="link-card" onClick={() => go("progress")}>
            <b>{T("شوف تقدمك", "See your progress")}</b>
            <small>{T("الأرقام اللي يعتمد عليها التقرير، يوم بعد يوم.", "The numbers behind this report, day by day.")}</small>
          </button>
        )}
      </div>
    </section>
  );
}

function Coach({ ctx }: { ctx: Ctx }) {
  const { view, T, act, lang, go } = ctx;
  const [selected, setSelected] = useState(0);
  const [working, setWorking] = useState<string | null>(null);
  const [selecting, setSelecting] = useState(false);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const togglePick = (at: number) => setPicked((p) => { const n = new Set(p); if (n.has(at)) n.delete(at); else n.add(at); return n; });
  const reports = view.reports;
  const report = reports[Math.min(selected, reports.length - 1)];
  const make = async (scope: string) => {
    setWorking(scope);
    await act("generate_report", { scope });
    setSelected(0);
    setWorking(null);
  };
  const scopes: [string, string, string][] = [
    ["latest", T("آخر جلسة", "Latest session"), T("شنو صار بجلستك الأخيرة", "What happened in your last session")],
    ["today", T("آخر يوم تدريب", "Latest training day"), T("تقرير يومك كامل", "A report on the whole day")],
    ["all", T("كل بياناتي", "All my data"), T("نقاط قوتك وضعفك على المدى الطويل", "Your long-term strengths and weaknesses")],
  ];
  const source = AI_SOURCES.find((a) => a.id === view.settings.ai_provider);
  const scopeName = (s: string) => scopes.find((x) => x[0] === s)?.[1] ?? s;
  const sourceName = view.settings.ai_provider === "template" ? T("التحليل المدمج", "Built-in analysis") : view.settings.ai_provider === "custom" ? T("مصدر مخصص", "Custom source") : source?.name;
  return (
    <>
      <PageHead title={T("تقارير المدرب", "Coach reports")} sub={T("المدرب يقرأ بياناتك ويكتب لك وين مشكلتك وشنو تسوي.", "The coach reads your data and tells you what is wrong and what to do.")}>
        <button className="tag link" onClick={() => go("settings")}>{sourceName}</button>
      </PageHead>
      <div className="scope-cards">
        {scopes.map(([id, name, desc]) => (
          <button key={id} disabled={working !== null || view.sessions.length === 0} onClick={() => make(id)}>
            <b>{working === id ? T("يكتب التقرير...", "Writing report...") : name}</b>
            <small>{desc}</small>
          </button>
        ))}
      </div>
      {reports.length === 0 && <Empty text={view.sessions.length === 0 ? T("العب أول جلسة حتى يكون عند المدرب شي يحلله.", "Play a session so the coach has something to analyse.") : T("اختار شنو تريد المدرب يحلل، وتقريرك يطلع هنا ويبقى محفوظ.", "Choose what the coach should analyse. Your report appears here and stays saved.")} />}
      {report && (
        <div className="reports">
          <aside className="card report-list">
            <div className="row between">
              <span className="label">{T("تقاريرك", "Your reports")}</span>
              {selecting
                ? <button className="btn ghost small" onClick={() => { setSelecting(false); setPicked(new Set()); }}>{T("خلص", "Done")}</button>
                : reports.length > 1 && <button className="btn ghost small" onClick={() => setSelecting(true)}>{T("تحديد", "Select")}</button>}
            </div>
            {selecting && (
              <div className="row">
                <button className="btn ghost small" onClick={() => setPicked(picked.size === reports.length ? new Set() : new Set(reports.map((r) => r.at_ms)))}>{picked.size === reports.length ? T("إلغاء الكل", "Select none") : T("حدد الكل", "Select all")}</button>
                {picked.size > 0 && <ConfirmButton label={T(`احذف ${picked.size}`, `Delete ${picked.size}`)} confirm={T("متأكد؟", "Sure?")} onConfirm={async () => { await act("delete_reports", { atMs: [...picked] }); setPicked(new Set()); setSelecting(false); setSelected(0); }} />}
              </div>
            )}
            {reports.map((r, i) => (
              <button key={r.at_ms} className={`${!selecting && i === selected ? "on" : ""} ${picked.has(r.at_ms) ? "picked" : ""}`} aria-pressed={selecting ? picked.has(r.at_ms) : undefined} onClick={() => (selecting ? togglePick(r.at_ms) : setSelected(i))}>
                <b>{selecting && <i className="check" aria-hidden="true" />}{scopeName(r.scope)}</b>
                <small>{whenText(r.at_ms, lang)} · {r.sessions} {T("جلسة", "sessions")}</small>
              </button>
            ))}
          </aside>
          <article className="card report">
            <div className="row between">
              <span className="muted small">{scopeName(report.scope)} · {whenText(report.at_ms, lang)}</span>
              <span className="row">
                <span className={`tag ${report.source === "ai" ? "good" : ""}`}>{report.source === "ai" ? T("ذكاء اصطناعي", "AI") : T("تحليل مدمج", "Built-in")}</span>
                <ConfirmButton label={T("احذف", "Delete")} confirm={T("متأكد؟", "Sure?")} onConfirm={() => { act("delete_report", { atMs: report.at_ms }); setSelected(0); }} />
              </span>
            </div>
            {report.locale && report.locale !== view.settings.locale && (
              <div className="banner row between">
                <span>{T("هذا التقرير مكتوب بلغة ثانية.", "This report is written in another language.")}</span>
                <button className="btn ghost" disabled={working !== null} onClick={() => make(report.scope)}>{T("اكتبه من جديد بلغتي", "Rewrite in my language")}</button>
              </div>
            )}
            <p className="lead">{report.reply.summary}</p>
            {report.note && <div className="banner">{T("استخدمنا التحليل المدمج لأن:", "Used the built-in analysis because:")} {friendlyError(report.note, T)}</div>}
            {report.reply.strengths.length > 0 && (
              <>
                <h2>{T("نقاط قوتك", "Your strengths")}</h2>
                <ul className="ticks">{report.reply.strengths.map((s) => <li key={s}>{s}</li>)}</ul>
              </>
            )}
            {report.reply.weaknesses.length > 0 && (
              <>
                <h2>{T("وين تخسر", "Where you lose")}</h2>
                {report.reply.weaknesses.map((w) => (
                  <div className="finding" key={w.title}>
                    <b>{w.title}</b>
                    <p className="muted">{w.detail}</p>
                    {w.fix && <span className="tag good">{w.fix}</span>}
                  </div>
                ))}
              </>
            )}
            {report.reply.next_steps.length > 0 && (
              <>
                <h2>{T("تسوي هسه", "Do next")}</h2>
                <ol className="next">{report.reply.next_steps.map((s) => <li key={s}>{s}</li>)}</ol>
              </>
            )}
            <ReportLinks ctx={ctx} report={report} />
          </article>
        </div>
      )}
    </>
  );
}

const DATA_TOGGLES: [keyof CoachConfig, string, string][] = [
  ["player_settings", "إعدادات الماوس", "Mouse settings"],
  ["skills", "درجات المهارات", "Skill scores"],
  ["findings", "التشخيصات", "Diagnoses"],
  ["recent_sessions", "نتائج الجلسات", "Session results"],
  ["directions", "الدقة بالاتجاهات", "Accuracy by direction"],
  ["plan", "خطة التدريب", "Training plan"],
  ["block_result", "نتيجة آخر كتلة", "Last block result"],
];

function SettingsPage({ ctx }: { ctx: Ctx }) {
  const { view, T, act } = ctx;
  const [s, setS] = useState<Settings>(view.settings);
  const [saved, setSaved] = useState(false);
  const first = useRef(true);
  const [key, setKey] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [testing, setTesting] = useState(false);
  const [config, setConfig] = useState<CoachConfig | null>(null);
  useEffect(() => { call<CoachConfig>("get_coach_config").then(setConfig, () => undefined); }, []);
  // Everything except the API key saves itself shortly after a change.
  const snapshot = JSON.stringify([s, config]);
  useEffect(() => {
    if (first.current) { first.current = false; return; }
    const t = setTimeout(async () => {
      await act("save_settings", { settings: s });
      if (config) await call("save_coach_config", { config });
      setSaved(true);
    }, 500);
    return () => clearTimeout(t);
  }, [snapshot]);
  useEffect(() => { setS((p) => (p.locale === view.settings.locale ? p : { ...p, locale: view.settings.locale })); }, [view.settings.locale]);
  const set = (patch: Partial<Settings>) => { setSaved(false); setTest(null); setS({ ...s, ...patch }); };
  const source = AI_SOURCES.find((a) => a.id === s.ai_provider) ?? AI_SOURCES[0];
  const needsAi = s.ai_provider !== "template";
  const hasKey = view.ai_keys_set.includes(s.ai_provider);
  const saveKey = async () => {
    if (key.trim()) {
      await act("set_ai_key", { provider: s.ai_provider, key });
      setKey("");
    }
  };
  const saveAll = async () => {
    await act("save_settings", { settings: s });
    await saveKey();
    if (config) await call("save_coach_config", { config });
  };
  const runTest = async () => {
    setTesting(true);
    setTest(null);
    await saveAll();
    try {
      await call<string>("test_ai");
      setTest({ ok: true, text: T("المصدر يشتغل.", "The source works.") });
    } catch (e) {
      setTest({ ok: false, text: friendlyError(String(e), T) });
    }
    setTesting(false);
  };
  const sections: [string, string][] = [["set-mouse", T("الماوس", "Mouse")], ["set-game", T("داخل اللعبة", "In the game")], ["set-app", T("التطبيق", "App")], ["set-coach", T("المدرب", "Coach")]];
  return (
    <>
      <PageHead title={T("الإعدادات", "Settings")} sub={T("كل تغيير ينحفظ لحاله.", "Every change saves by itself.")} />
      <div className="settings-layout">
        <nav className="settings-nav" aria-label={T("أقسام الإعدادات", "Settings sections")}>
          {sections.map(([id, name]) => <a key={id} href={`#${id}`}>{name}</a>)}
          <span className={`save-state ${saved ? "ok" : ""}`}>{saved ? T("✓ محفوظ", "✓ Saved") : T("يحفظ تلقائياً", "Saves automatically")}</span>
        </nav>
        <div className="settings-body">
          <section className="card settings-card" id="set-mouse">
            <h2>{T("الماوس", "Mouse")}</h2>
            <SettingRow title="DPI" help={T("نفس رقم الماوس باللعبة.", "The same number as in your game.")}>
              <input type="number" dir="ltr" className="num" value={s.dpi} step={50} min={50} onChange={(e) => { const v = e.target.valueAsNumber; if (Number.isFinite(v) && v >= 50) set({ dpi: v }); }} />
            </SettingRow>
            <SettingRow title={T("الحساسية", "Sensitivity")} help={T("حساسية VALORANT بالضبط.", "Your exact VALORANT sensitivity.")}>
              <input type="number" dir="ltr" className="num" value={s.sens} step={0.01} min={0.01} onChange={(e) => { const v = e.target.valueAsNumber; if (Number.isFinite(v) && v >= 0.01) set({ sens: v }); }} />
            </SettingRow>
            <SettingRow title={T("لفة كاملة", "Full turn")} help={T("غيرها بس إذا غيرتها باللعبة، لأن المقارنات تعتمد عليها.", "Change it only when you change it in the game; comparisons depend on it.")}>
              <b dir="ltr" className="readout">{view.cm360.toFixed(1)} cm/360</b>
            </SettingRow>
          </section>

          <section className="card settings-card" id="set-game">
            <h2>{T("داخل اللعبة", "In the game")}</h2>
            <SettingRow title={T("صوت الإصابة والخطأ", "Hit and miss sound")} help={T("صفر يطفي الصوت.", "Zero turns it off.")}>
              <div className="inline-slider">
                <input type="range" min={0} max={100} step={5} value={s.sound_volume} onChange={(e) => set({ sound_volume: e.target.valueAsNumber })} aria-label={T("الصوت", "Volume")} />
                <b dir="ltr">{s.sound_volume}%</b>
              </div>
            </SettingRow>
            <SettingRow title={T("شغّل اللعبة بنافذة", "Run the game in a window")} help={T("ملء الشاشة أدق بالتوقيت. النافذة أسهل إذا تبدل بين البرامج.", "Full screen has steadier timing; a window is easier when switching apps.")}>
              <Switch checked={s.windowed} set={(windowed) => set({ windowed })} label={T("شغّل اللعبة بنافذة", "Run the game in a window")} />
            </SettingRow>
            <div className="setting-block">
              <CrosshairEditor ctx={ctx} value={s.crosshair} set={(crosshair) => set({ crosshair })} />
            </div>
          </section>

          <section className="card settings-card" id="set-app">
            <h2>{T("التطبيق", "App")}</h2>
            <SettingRow title={T("اللغة", "Language")} help={T("تكدر تغيرها كمان من أعلى القائمة الجانبية.", "You can also switch it at the top of the side menu.")}>
              <div className="seg">
                <button className={s.locale !== "en" ? "on" : ""} onClick={() => set({ locale: "ar" })}>العربية</button>
                <button className={s.locale === "en" ? "on" : ""} onClick={() => set({ locale: "en" })}>English</button>
              </div>
            </SettingRow>
            <SettingRow title={T("وضع التجربة", "Test mode")} help={T("تمارين 20 ثانية وبدون انتظار يوم الراحة. للتجربة بس، لأن النتائج ما تعتبر قياس حقيقي.", "20-second drills and no rest-day wait. For trying things only; results are not a real measurement.")}>
              <Switch checked={s.test_mode} set={(test_mode) => set({ test_mode })} label={T("وضع التجربة", "Test mode")} />
            </SettingRow>
          </section>

          <section className="card settings-card" id="set-coach">
            <h2>{T("المدرب", "Coach")}</h2>
            <p className="muted small">{T("التقارير تنكتب بالتحليل المدمج بدون إنترنت، أو بذكاء اصطناعي تختاره انت.", "Reports are written by the built-in analysis offline, or by an AI you choose.")}</p>
            <SettingRow title={T("مصدر التقارير", "Report source")} help={needsAi ? T("المفتاح ينحفظ على جهازك بس، ويروح للمصدر اللي اخترته فقط.", "The key stays on this computer and is sent only to the source you chose.") : T("بدون إنترنت وبدون مفتاح.", "Offline, no key needed.")}>
              <select value={s.ai_provider} onChange={(e) => { const a = AI_SOURCES.find((x) => x.id === e.target.value)!; set({ ai_provider: a.id, ai_model: a.model, ai_base_url: a.id === "custom" ? s.ai_base_url : "" }); setKey(""); }}>
                <option value="template">{T("التحليل المدمج (بدون AI)", "Built-in analysis (no AI)")}</option>
                <option value="claude">Claude</option>
                <option value="openai">OpenAI</option>
                <option value="gemini">Gemini</option>
                <option value="openrouter">OpenRouter</option>
                <option value="custom">{T("مخصص (Custom)", "Custom")}</option>
              </select>
            </SettingRow>
            {needsAi && (
              <SettingRow title={T("اسم الموديل", "Model name")}>
                <input dir="ltr" value={s.ai_model} onChange={(e) => set({ ai_model: e.target.value })} />
              </SettingRow>
            )}
            {s.ai_provider === "custom" && (
              <SettingRow title={T("رابط الخادم", "Server URL")} help="Base URL">
                <input dir="ltr" placeholder="http://localhost:11434/v1" value={s.ai_base_url} onChange={(e) => set({ ai_base_url: e.target.value })} />
              </SettingRow>
            )}
            {needsAi && (
              <SettingRow title={`API key${source.needsKey ? "" : T(" (اختياري)", " (optional)")}`} help={test ? undefined : hasKey ? T("محفوظ. اكتب مفتاح جديد لتغييره.", "Saved. Type a new key to replace it.") : undefined}>
                <div className="key-row">
                  <input dir="ltr" type="password" autoComplete="off" placeholder={hasKey ? "••••••••" : ""} value={key} onChange={(e) => setKey(e.target.value)} onBlur={saveKey} aria-label="API key" />
                  <button className="btn ghost" disabled={testing} onClick={runTest}>{testing ? T("يفحص...", "Testing...") : T("افحص الاتصال", "Test connection")}</button>
                </div>
                {test && <small className={test.ok ? "up" : "down"}>{test.text}</small>}
              </SettingRow>
            )}
            {config && (
              <>
                <h3 className="sub">{T("شنو يشوف المدرب من بياناتك", "What the coach can see")}</h3>
                <p className="muted small">{T("كل شي تطفيه ما يوصل للمدرب. الأرقام بس، بدون تسجيلات الحركة.", "Anything switched off never reaches the coach. Numbers only, never the movement recordings.")}</p>
                <div className="toggle-grid">
                  {DATA_TOGGLES.map(([k, ar, en]) => <Toggle key={k} checked={Boolean(config[k])} set={(v) => { setSaved(false); setConfig({ ...config, [k]: v }); }} label={T(ar, en)} />)}
                </div>
                <h3 className="sub">{T("تعليمات إضافية للمدرب", "Extra instructions for the coach")}</h3>
                <p className="muted small">{T("اكتب شي يخص أسلوبك أو هدفك، والمدرب ياخذه بعين الاعتبار بكل تقرير. أمثلة:", "Write anything about your style or goal; the coach considers it in every report. Examples:")}</p>
                <ul className="examples">
                  <li>{T("ألعب Duelist وأعتمد على الفليك، ركز على سرعتي.", "I play Duelist and rely on flicks; focus on my speed.")}</li>
                  <li>{T("عندي 30 دقيقة باليوم بس.", "I only have 30 minutes a day.")}</li>
                  <li>{T("خلي التقرير قصير ومباشر.", "Keep reports short and direct.")}</li>
                </ul>
                <textarea rows={3} aria-label={T("تعليمات إضافية للمدرب", "Extra instructions for the coach")} placeholder={T("مثلاً: ألعب Sentinel وأريد أحسن ثبات مستوى الرأس.", "For example: I play Sentinel and want steadier head level.")} value={config.extra_instructions} onChange={(e) => { setSaved(false); setConfig({ ...config, extra_instructions: e.target.value }); }} />
                <p className="small muted">{T("نفس الإعدادات بملف JSON وتكدر تعدله:", "The same settings live in a JSON file you can edit:")} <code dir="ltr">{view.coach_config_path}</code></p>
              </>
            )}
          </section>
        </div>
      </div>
    </>
  );
}

/** One setting: what it is on one side, its control on the other. */
function SettingRow({ title, help, children }: { title: string; help?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-text"><b>{title}</b>{help && <small>{help}</small>}</div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

/** An on/off switch whose label is read out but shown by the row beside it. */
function Switch({ checked, set, label }: { checked: boolean; set: (v: boolean) => void; label: string }) {
  return (
    <label className="toggle bare">
      <input type="checkbox" checked={checked} onChange={(e) => set(e.target.checked)} aria-label={label} />
      <i />
    </label>
  );
}

const CROSSHAIR_COLORS = ["00ff4c", "ffffff", "00ffff", "ffff00", "ff4cff", "ff3b3b"];

function CrosshairEditor({ ctx, value, set }: { ctx: Ctx; value: string; set: (v: string) => void }) {
  const { T } = ctx;
  const [color, length, thickness, gap, dot] = value.split(":");
  const parts = { color, length: Number(length), thickness: Number(thickness), gap: Number(gap), dot: dot === "1" };
  const write = (p: Partial<typeof parts>) => {
    const n = { ...parts, ...p };
    set(`${n.color}:${n.length}:${n.thickness}:${n.gap}:${n.dot ? 1 : 0}`);
  };
  const c = `#${parts.color}`;
  const t = parts.thickness, g = parts.gap, l = parts.length;
  const arm = (x: number, y: number, w: number, h: number) => <i style={{ left: 40 + x, top: 40 + y, width: w, height: h, background: c }} />;
  return (
    <div className="crosshair-editor">
      <span className="label">{T("المؤشر", "Crosshair")}</span>
      <div className="row">
        <div className="crosshair-preview" aria-hidden="true">
          {(parts.dot || l === 0) && arm(-t / 2, -t / 2, t, t)}
          {l > 0 && <>{arm(g, -t / 2, l, t)}{arm(-g - l, -t / 2, l, t)}{arm(-t / 2, g, t, l)}{arm(-t / 2, -g - l, t, l)}</>}
        </div>
        <div className="grow">
          <div className="swatches">
            {CROSSHAIR_COLORS.map((x) => <button key={x} aria-label={x} className={x === parts.color ? "on" : ""} style={{ background: `#${x}` }} onClick={() => write({ color: x })} />)}
          </div>
          <Toggle checked={parts.dot} set={(v) => write({ dot: v })} label={T("نقطة بالوسط", "Centre dot")} />
        </div>
      </div>
      <Slider label={T("طول الخطوط", "Line length")} value={l} min={0} max={12} step={1} unit="px" set={(v) => write({ length: v })} />
      <Slider label={T("السماكة", "Thickness")} value={t} min={1} max={6} step={1} unit="px" set={(v) => write({ thickness: v })} />
      <Slider label={T("الفراغ بالوسط", "Centre gap")} value={g} min={0} max={10} step={1} unit="px" set={(v) => write({ gap: v })} />
    </div>
  );
}

function Toggle({ checked, set, label, help }: { checked: boolean; set: (v: boolean) => void; label: string; help?: string }) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={(e) => set(e.target.checked)} />
      <i />
      <span>{label}{help && <small className="muted">{help}</small>}</span>
    </label>
  );
}

function LangSelect({ ctx, value, set }: { ctx: Ctx; value: string; set: (v: string) => void }) {
  return (
    <label className="field">{ctx.T("اللغة", "Language")}
      <select value={value} onChange={(e) => set(e.target.value)}><option value="ar">العربية</option><option value="en">English</option></select>
    </label>
  );
}

function Num(props: { label: string; value: number; step: number; min?: number; set: (v: number) => void }) {
  return (
    <label className="field">{props.label}
      <input type="number" dir="ltr" value={props.value} step={props.step} min={props.min} onChange={(e) => { const v = e.target.valueAsNumber; if (Number.isFinite(v) && v >= (props.min ?? 0)) props.set(v); }} />
    </label>
  );
}
