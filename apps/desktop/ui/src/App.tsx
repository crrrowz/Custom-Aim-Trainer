import { Component, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { call, type CoachConfig, type CustomTargets, type Detail, type Flick, type Plan, type SessionCard, type Settings, type View } from "./api";
import { AimPath, ReactionTimeline, SERIES, ShotScatter, SkillLines, SkillRadar, Spark, SpeedChart, TrackingChart, TrendLine, ValueBars, type Series } from "./charts";
import { drillInfo, drillNames, evidenceNames, groupNames, scenarioNames, skillNames, type DrillInfo, type Lang } from "./i18n";
import { TargetPreview } from "./Preview";

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
        {view.notice === "done_today" && <div className="banner">{T("خلصت جلسة اليوم. الجلسة الجاية تفتح باجر.", "Today's session is done. The next one opens tomorrow.")}</div>}
        {view.notice === "incomplete" && <div className="banner">{T("التقييم ما كمل. نتائجك انحفظت بالتحليل، بس الخطة تحتاج كل الاختبارات الثمانية.", "The assessment was not finished. Your results are in Analysis, but a plan needs all eight tests.")}</div>}
        <ScreenGuard key={tab} message={T("هذي الصفحة واجهت خطأ. الصفحات الثانية تشتغل عادي.", "This page hit an error. The other pages still work.")} retry={T("أعد المحاولة", "Try again")}>
          {tab === "home" && <Home ctx={ctx} />}
          {tab === "analysis" && <Analysis ctx={ctx} initial={recording} />}
          {tab === "progress" && <Progress ctx={ctx} />}
          {tab === "plan" && <PlanPage ctx={ctx} />}
          {tab === "practice" && <Practice ctx={ctx} />}
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

function Home({ ctx }: { ctx: Ctx }) {
  const { view, T, act, go, lang } = ctx;
  const overall = overallOf(view.latest);
  const prev = view.history.length > 1 ? overallOf(view.history[view.history.length - 2]) : null;
  const recent = view.sessions.slice(0, 5);
  const plan = view.plan;
  const active = plan && typeof plan.state === "object" && "Active" in plan.state ? plan.state.Active : null;
  const retest = plan && typeof plan.state === "object" && "AwaitingRetest" in plan.state;

  let hero: { title: string; sub: string; cta: string; run: () => void };
  if (!view.latest) {
    hero = { title: T("ابدأ بالتقييم", "Start with an assessment"), sub: T("7 اختبارات بحوالي 9 دقايق، وبعدها تعرف وين مشكلتك بالضبط.", "7 tests in about 9 minutes, then you will know exactly what to fix."), cta: T("ابدأ التقييم", "Start assessment"), run: () => act("run_assessment") };
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
            {view.latest && <button className="btn ghost" disabled={!view.trainer_found} onClick={() => act("run_assessment")}>{T("إعادة التقييم الكامل", "Retake full assessment")}</button>}
          </div>
        </section>
        <section className="card score">
          <span className="label">{T("المستوى العام", "Overall")}</span>
          <b className="huge">{overall === null ? "—" : overall.toFixed(0)}</b>
          {overall !== null && prev !== null && (
            <span className={overall >= prev ? "up" : "down"}>{overall >= prev ? "▲" : "▼"} {Math.abs(overall - prev).toFixed(1)}</span>
          )}
          <Spark values={view.history.map((a) => overallOf(a) ?? 0)} />
        </section>
        <section className="card radar">
          <span className="label">{T("ملف المهارات", "Skill profile")}</span>
          {view.latest ? <SkillRadar skills={view.latest.profile.skills} name={(id) => pick(skillNames[id], lang, id)} /> : <Empty text={T("يطلع بعد أول تقييم.", "Appears after your first assessment.")} />}
        </section>
        <section className="card problems">
          <span className="label">{T("أهم المشاكل", "Top problems")}</span>
          {view.findings.length === 0 && <Empty text={view.latest ? T("ما لكينا مشكلة مؤكدة.", "No confirmed problem.") : T("نحتاج تقييم حتى نحدد المشاكل.", "An assessment finds your problems.")} />}
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

const dayKey = (ms: number) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

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

function Analysis({ ctx, initial }: { ctx: Ctx; initial?: string }) {
  const { view, T, lang, go } = ctx;
  const days = useMemo(() => [...new Set(view.sessions.map((s) => dayKey(s.at_ms)))], [view.sessions]);
  const [open, setOpen] = useState<SessionCard | null>(() => view.sessions.find((s) => s.path === initial) ?? null);
  const [day, setDay] = useState(() => (open ? dayKey(open.at_ms) : days[0]));

  if (view.sessions.length === 0) {
    return (
      <>
        <PageHead title={T("التحليل", "Analysis")} />
        <Empty text={T("بعد أول جلسة تشوف هنا كل يوم تدريب وكل حركة سويتها.", "After your first session you will see every training day and every movement here.")} action={<button className="btn" onClick={() => go("home")}>{T("ابدأ", "Get started")}</button>} />
      </>
    );
  }
  if (open) return <SessionView key={open.path} card={open} ctx={ctx} back={() => setOpen(null)} />;

  const today = dayKey(Date.now());
  const yesterday = dayKey(Date.now() - 86400000);
  const dayName = (d: string) => (d === today ? T("اليوم", "Today") : d === yesterday ? T("أمس", "Yesterday") : new Date(`${d}T12:00`).toLocaleDateString(lang === "ar" ? "ar-IQ" : "en-GB", { day: "numeric", month: "short" }));
  const cards = view.sessions.filter((s) => dayKey(s.at_ms) === day);
  const scored = cards.map(sessionScore).filter((v): v is number => v !== null);
  const reactions = cards.map((c) => c.median_reaction_ms).filter((v): v is number => v !== null);
  const hits = cards.reduce((a, c) => a + c.hits, 0);
  const seconds = cards.reduce((a, c) => a + c.duration_s, 0);
  const hours = new Map<number, number[]>();
  for (const c of cards) {
    const v = sessionScore(c);
    if (v === null) continue;
    const h = new Date(c.at_ms).getHours();
    hours.set(h, [...(hours.get(h) ?? []), v]);
  }
  const hourly = [...hours.entries()].sort((a, b) => a[0] - b[0]).map(([h, v]) => ({ label: `${String(h).padStart(2, "0")}:00`, value: Math.round(avg(v)! * 100), n: v.length }));
  let hourNote = "";
  if (hourly.length >= 2) {
    const [a, b] = [hourly[hourly.length - 2], hourly[hourly.length - 1]];
    const d = b.value - a.value;
    hourNote = d === 0
      ? T(`مستواك بـ ${b.label} نفس ${a.label}.`, `${b.label} matched ${a.label}.`)
      : T(`${d > 0 ? "تحسنت" : "نزلت"} ${Math.abs(d)} نقطة بين ${a.label} و ${b.label}.`, `${d > 0 ? "Up" : "Down"} ${Math.abs(d)} points from ${a.label} to ${b.label}.`);
  }

  return (
    <>
      <PageHead title={T("التحليل", "Analysis")} sub={T("اختار يوم، وافتح أي جلسة حتى تشوف تفاصيلها.", "Pick a day and open any session for its details.")} />
      <div className="chips days-strip">
        {days.slice(0, 10).map((d) => <button key={d} className={d === day ? "on" : ""} onClick={() => setDay(d)}>{dayName(d)}</button>)}
        <input type="date" aria-label={T("اختار يوم", "Pick a day")} value={day} max={today} onChange={(e) => e.target.value && setDay(e.target.value)} />
      </div>
      <div className="stats">
        <Stat value={String(cards.length)} label={T("جلسات", "Sessions")} />
        <Stat value={pct(avg(scored))} label={T("معدل الدقة", "Average accuracy")} />
        <Stat value={ms(avg(reactions))} label={T("معدل رد الفعل", "Average reaction")} />
        <Stat value={String(hits)} label={T("أهداف أصبتها", "Targets hit")} hint={`${Math.round(seconds / 60)} ${T("دقيقة تدريب", "minutes trained")}`} />
      </div>
      {hourly.length > 0 && (
        <section className="card">
          <h2>{T("مستواك خلال اليوم", "Your level through the day")}</h2>
          {hourNote && <p className="muted small">{hourNote}</p>}
          <ValueBars rows={hourly.map((h) => ({ label: h.label, value: h.value, tip: `${h.value}% · ${h.n} ${T("جلسة", "sessions")}` }))} max={100} unit="%" />
        </section>
      )}
      <h2 className="group">{T("جلسات هذا اليوم", "Sessions this day")}</h2>
      {cards.length === 0 && <Empty text={T("ما لعبت بهذا اليوم.", "No sessions on this day.")} />}
      <div className="session-grid">
        {cards.map((c) => (
          <button key={c.path} className="session-card" onClick={() => setOpen(c)}>
            <span className="row between">
              <b>{scenarioTitle(c.scenario, lang)}</b>
              <small>{new Date(c.at_ms).toLocaleTimeString(lang === "ar" ? "ar-IQ" : "en-GB", { hour: "2-digit", minute: "2-digit" })}</small>
            </span>
            <span className="big-num">{pct(sessionScore(c))}</span>
            <span className="meter wide"><i style={{ width: `${Math.round((sessionScore(c) ?? 0) * 100)}%`, background: "var(--accent)" }} /></span>
            <span className="row between small muted">
              <span>{c.targets > 0 ? `${c.hits}/${c.targets} ${T("هدف", "targets")}` : T("تتبع", "Tracking")}</span>
              <span dir="ltr">{ms(c.median_reaction_ms)}</span>
            </span>
            {c.assessment && <span className="tag good">{T("تقييم", "Assessment")}</span>}
            {c.timing_suspect && <span className="tag warn">{T("نسخة قديمة، الأوقات غير دقيقة", "Old recording, timing unreliable")}</span>}
          </button>
        ))}
      </div>
    </>
  );
}

const DIRECTION_NAMES: [string, string, string][] = [
  ["يمين", "Right", "→"], ["يمين فوق", "Up-right", "↗"], ["فوق", "Up", "↑"], ["يسار فوق", "Up-left", "↖"],
  ["يسار", "Left", "←"], ["يسار جوه", "Down-left", "↙"], ["جوه", "Down", "↓"], ["يمين جوه", "Down-right", "↘"],
];

function SessionView({ card, ctx, back }: { card: SessionCard; ctx: Ctx; back: () => void }) {
  const { T, lang } = ctx;
  const [detail, setDetail] = useState<Detail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"replay" | "trend" | "map" | "dirs">("replay");

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

  const shots = flicks.filter((f) => f.endpoint);
  const offX = avg(shots.map((f) => f.endpoint![0])) ?? 0;
  const offY = avg(shots.map((f) => f.endpoint![1])) ?? 0;
  const spread = avg(shots.map((f) => Math.hypot(f.endpoint![0], f.endpoint![1]))) ?? 0;
  const inside = shots.length ? shots.filter((f) => Math.hypot(f.endpoint![0], f.endpoint![1]) <= f.radius_deg).length / shots.length : null;
  const bias = [
    Math.abs(offX) >= 0.1 ? `${Math.abs(offX).toFixed(2)}° ${offX > 0 ? T("يمين", "right") : T("يسار", "left")}` : null,
    Math.abs(offY) >= 0.1 ? `${Math.abs(offY).toFixed(2)}° ${offY > 0 ? T("فوق", "up") : T("جوه", "down")}` : null,
  ].filter(Boolean).join(" · ");

  const dirs = DIRECTION_NAMES.map(([ar, en, arrow], o) => {
    const list = flicks.filter((f) => f.octant === o);
    return { name: `${arrow}  ${T(ar, en)}`, n: list.length, acc: list.length ? list.filter((f) => f.hit).length / list.length : 0, time: avg(list.map((f) => f.time_to_hit_ms).filter((v): v is number => v !== null)) };
  }).filter((d) => d.n > 0);
  const maxTime = Math.max(...dirs.map((d) => d.time ?? 0), 1);

  return (
    <>
      <PageHead title={scenarioTitle(card.scenario, lang)} sub={new Date(card.at_ms).toLocaleString(lang === "ar" ? "ar-IQ" : "en-GB", { weekday: "long", day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" })}>
        <button className="btn ghost" onClick={back}>{T("كل الجلسات", "All sessions")}</button>
      </PageHead>
      {error && <div className="banner err">{error}</div>}
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
                <section className="card split">
                  <div>
                    <h2>{T("خريطة طلقاتك", "Your shot map")}</h2>
                    <div className="stats">
                      <Stat value={pct(inside)} label={T("داخل الهدف", "Inside the target")} />
                      <Stat value={`${spread.toFixed(2)}°`} label={T("متوسط البعد عن المركز", "Average distance from centre")} />
                    </div>
                    <p>{bias ? T(`طلقاتك مايلة ${bias} بالمعدل.`, `Your shots lean ${bias} on average.`) : T("طلقاتك متمركزة، ماكو ميلان واضح.", "Your shots are centred, with no clear lean.")}</p>
                    <p className="muted small">{T("الدائرة الخضراء هي الهدف، والدائرة الأكبر ضعف حجمه. علامة + هي مركز كل طلقاتك.", "The green circle is the target; the larger ring is twice its size. + marks the centre of all your shots.")}</p>
                    <Legend items={[["#3ddc97", T("إصابة", "Hit")], ["#ff6b6b", T("خطأ", "Miss")]]} />
                  </div>
                  <ShotScatter flicks={flicks} hitLabel={T("إصابة", "Hit")} missLabel={T("خطأ", "Miss")} />
                </section>
              )}
              {tab === "dirs" && (
                <section className="card">
                  <h2>{T("الدقة والسرعة بكل اتجاه", "Accuracy and speed in each direction")}</h2>
                  <p className="muted small">{T("الاتجاه هو وين تحركت حتى توصل الهدف.", "Direction is where you had to move to reach the target.")}</p>
                  <table className="dir-table">
                    <thead><tr><th>{T("الاتجاه", "Direction")}</th><th>{T("الدقة", "Accuracy")}</th><th>{T("الوقت", "Time")}</th><th>{T("أهداف", "Targets")}</th></tr></thead>
                    <tbody>
                      {dirs.map((d) => (
                        <tr key={d.name}>
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
  const [rate, setRate] = useState(0.25);
  const raf = useRef(0);

  useEffect(() => { setMs(end); setPlaying(false); }, [flick?.id, end]);
  useEffect(() => {
    if (!playing) return;
    let last = performance.now();
    const step = (now: number) => {
      const dt = (now - last) * rate;
      last = now;
      let done = false;
      setMs((m) => {
        const next = m + dt;
        if (next >= end) { done = true; return end; }
        return next;
      });
      if (done) setPlaying(false);
      else raf.current = requestAnimationFrame(step);
    };
    raf.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf.current);
  }, [playing, rate, end]);

  const move = (d: number) => setIndex((i) => Math.max(0, Math.min(list.length - 1, i + d)));
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
      if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
        e.preventDefault();
        setIndex((i) => Math.max(0, Math.min(list.length - 1, i + ((e.key === "ArrowRight") === (document.documentElement.dir !== "rtl") ? 1 : -1))));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [list.length]);
  const labels = { onset: T("بداية", "Start"), primary: T("نهاية الحركة", "First stop"), shot: T("نقرة", "Click") };
  const filters: [string, string][] = [["all", T("الكل", "All")], ["misses", T("الأخطاء", "Misses")], ["slowest", T("الأبطأ", "Slowest")], ["overshoot", T("أكبر تجاوز", "Biggest overshoot")]];

  return (
    <section className="card">
      <div className="row between">
        <div className="seg">
          {filters.map(([id, name]) => <button key={id} className={filter === id ? "on" : ""} onClick={() => { setFilter(id); setIndex(0); }}>{name}</button>)}
        </div>
        <span className="muted small">{list.length ? `${Math.min(index, list.length - 1) + 1} / ${list.length}` : ""}</span>
      </div>
      {!flick ? <Empty text={T("ماكو أهداف بهذا الفلتر.", "No targets match this filter.")} /> : (
        <>
          <div className="strip" dir="ltr">
            {list.map((f, i) => <button key={f.id} aria-label={`${i + 1}`} className={`${f.hit ? "h" : "m"} ${f.id === flick.id ? "on" : ""}`} onClick={() => setIndex(i)} />)}
          </div>
          <div className="replay">
            <AimPath flick={flick} progressMs={ms} labels={labels} />
            <aside>
              <span className={`tag ${flick.hit ? "good" : "bad"}`}>{flick.hit ? T("إصابة", "Hit") : T("خطأ", "Miss")}</span>
              <p className="note">{flickNote(flick, T)}</p>
              <dl>
                <dt>{T("المسافة", "Distance")}</dt><dd>{flick.distance_deg === null ? "—" : `${flick.distance_deg.toFixed(1)}°`}</dd>
                <dt>{T("رد الفعل", "Reaction")}</dt><dd>{flick.reaction_ms === null ? "—" : `${flick.reaction_ms.toFixed(0)} ms`}</dd>
                <dt>{T("وقت النقرة", "Click at")}</dt><dd>{flick.shot_ms === null ? "—" : `${flick.shot_ms.toFixed(0)} ms`}</dd>
                <dt>{T("الحركة الأولى", "First movement")}</dt><dd>{flick.gain === null ? "—" : `${(flick.gain * 100).toFixed(0)}%`}</dd>
                <dt>{T("التصحيحات", "Corrections")}</dt><dd>{flick.corrections}</dd>
              </dl>
              <div className="row">
                <button className="btn ghost sq" aria-label={T("السابق", "Previous")} onClick={() => move(-1)}>‹</button>
                <button className="btn" onClick={() => { if (ms >= end) setMs(0); setPlaying(!playing); }}>{playing ? T("إيقاف", "Pause") : T("شغّل", "Play")}</button>
                <button className="btn ghost sq" aria-label={T("التالي", "Next")} onClick={() => move(1)}>›</button>
                <select aria-label={T("السرعة", "Speed")} value={rate} onChange={(e) => setRate(Number(e.target.value))}>
                  <option value={0.1}>0.1×</option><option value={0.25}>0.25×</option><option value={1}>1×</option>
                </select>
              </div>
            </aside>
          </div>
          <SpeedChart flick={flick} progressMs={ms} onScrub={(v) => { setPlaying(false); setMs(v); }} labels={{ y: T("السرعة °/ث", "Speed °/s"), ...labels }} />
        </>
      )}
    </section>
  );
}

function Progress({ ctx }: { ctx: Ctx }) {
  const { view, T, lang } = ctx;
  const ids = view.latest?.profile.skills.map((s) => s.skill) ?? [];
  const [picked, setPicked] = useState<string[]>(() => ids.slice(0, 2));
  const [kind, setKind] = useState("all");
  if (view.sessions.length === 0) {
    return (<><PageHead title={T("التقدم", "Progress")} /><Empty text={T("تقدمك يطلع هنا بعد أول جلسة.", "Your progress appears here after your first session.")} /></>);
  }
  const kinds = [...new Set(view.sessions.map((s) => s.scenario))];
  const list = view.sessions.filter((s) => kind === "all" || s.scenario === kind);
  const byDay = new Map<string, SessionCard[]>();
  for (const s of [...list].reverse()) byDay.set(dayKey(s.at_ms), [...(byDay.get(dayKey(s.at_ms)) ?? []), s]);
  const dayLabel = (d: string) => new Date(`${d}T12:00`).toLocaleDateString(lang === "ar" ? "ar-IQ" : "en-GB", { day: "numeric", month: "numeric" });
  const seriesOf = (f: (c: SessionCard) => number | null) => [...byDay.entries()].flatMap(([d, cards]) => {
    const v = avg(cards.map(f).filter((x): x is number => x !== null));
    return v === null ? [] : [{ label: dayLabel(d), value: v }];
  });
  const accuracy = seriesOf((c) => { const v = sessionScore(c); return v === null ? null : v * 100; });
  const reaction = seriesOf((c) => c.median_reaction_ms);
  const volume = [...byDay.entries()].map(([d, cards]) => ({ label: dayLabel(d), value: Math.round(cards.reduce((a, c) => a + c.duration_s, 0) / 60), tip: `${cards.length} ${T("جلسة", "sessions")}` }));
  const first = accuracy[0]?.value;
  const last = accuracy[accuracy.length - 1]?.value;
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : p.length >= 4 ? p : [...p, id]));
  const series: Series[] = picked.map((id) => ({
    id,
    name: pick(skillNames[id], lang, id),
    points: view.history.map((a, i) => {
      const s = a.profile.skills.find((k) => k.skill === id);
      return { label: `#${i + 1}`, v: s?.score ?? 0, lo: s?.ci95[0] ?? 0, hi: s?.ci95[1] ?? 0 };
    }),
  }));
  return (
    <>
      <PageHead title={T("التقدم", "Progress")} sub={T("كل جلسة لعبتها محسوبة هنا.", "Every session you played counts here.")}>
        <select aria-label={T("النوع", "Type")} value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="all">{T("كل الجلسات", "All sessions")}</option>
          {kinds.map((k) => <option key={k} value={k}>{scenarioTitle(k, lang)}</option>)}
        </select>
      </PageHead>
      <div className="stats">
        <Stat value={String(list.length)} label={T("جلسات", "Sessions")} />
        <Stat value={String(list.reduce((a, c) => a + c.hits, 0))} label={T("أهداف أصبتها", "Targets hit")} />
        <Stat value={`${Math.round(list.reduce((a, c) => a + c.duration_s, 0) / 60)}`} label={T("دقايق تدريب", "Minutes trained")} />
        <Stat value={last === undefined ? "—" : `${Math.round(last)}%`} label={T("دقة آخر يوم", "Latest day accuracy")} hint={first !== undefined && accuracy.length > 1 ? `${last! >= first ? "+" : ""}${Math.round(last! - first)} ${T("عن أول يوم", "vs first day")}` : undefined} />
      </div>
      <section className="card">
        <h2>{T("الدقة يوم بعد يوم", "Accuracy day by day")}</h2>
        {accuracy.length < 2 && <p className="muted small">{T("العب بيوم ثاني حتى يطلع الخط. لحد هسه عندك يوم واحد.", "Play on another day to see a line. So far you have one day.")}</p>}
        <TrendLine points={accuracy} unit="%" better="up" />
      </section>
      <div className="split-2">
        <section className="card">
          <h2>{T("رد الفعل", "Reaction")}</h2>
          {reaction.length === 0 ? <p className="muted small">{T("ماكو أوقات موثوقة بعد. العب جلسة جديدة.", "No reliable timing yet. Play a new session.")}</p> : <TrendLine points={reaction} unit="ms" better="down" />}
        </section>
        <section className="card">
          <h2>{T("دقايق التدريب", "Minutes trained")}</h2>
          <ValueBars rows={volume} unit={T("د", "min")} />
        </section>
      </div>
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
      {view.block_result && (
        <section className="card">
          <h2>{T("نتيجة الكتلة", "Block result")}</h2>
          {view.block_result.conditions_changed && <div className="banner">{T(`إعدادات الماوس اختلفت بين الاختبارين: DPI ${view.block_result.settings_before[0]} وحساسية ${view.block_result.settings_before[1]} قبل، و DPI ${view.block_result.settings_after[0]} وحساسية ${view.block_result.settings_after[1]} بعد. المقارنة غير دقيقة.`, `Mouse settings differ between the tests: ${view.block_result.settings_before[0]} DPI / ${view.block_result.settings_before[1]} before, ${view.block_result.settings_after[0]} DPI / ${view.block_result.settings_after[1]} after. The comparison is unreliable.`)}</div>}
          {view.block_result.training_effect !== null && (
            <div className="stats"><Stat value={`${view.block_result.training_effect > 0 ? "+" : ""}${view.block_result.training_effect.toFixed(1)}`} label={T("أثر التدريب", "Training effect")} hint={T("تحسن المهارات المدربة ناقص غيرها", "Trained skills minus the rest")} /></div>
          )}
          <table>
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
          </table>
          {typeof plan?.state === "object" && "Evaluated" in plan.state && <p><button className="btn" onClick={() => act("next_block")}>{T("ابدأ الكتلة الجاية", "Start next block")}</button></p>}
        </section>
      )}
    </>
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
        <p className="muted">{plan.retest_ready ? T("نفس الاختبارات بنموذج جديد، حتى نقيس التحسن الحقيقي.", "The same tests in a new form, to measure real improvement.") : T("يفتح بعد يوم راحة. الراحة جزء من التعلم.", "Opens after a rest day. Rest is part of learning.")}</p>
        <button className="btn" disabled={!plan.retest_ready || !view.trainer_found} onClick={() => act("run_retest")}>{T("ابدأ", "Start")}</button>
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
            {plan.levels[item.drill] !== undefined && <span className="muted small">{T("المستوى", "Level")} {(plan.levels[item.drill] * 100).toFixed(0)}</span>}
            <span className="muted small">{item.minutes} {T("د", "min")}</span>
          </li>
        ))}
      </ol>
    </section>
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

function Practice({ ctx }: { ctx: Ctx }) {
  const { view, T, act, lang, go } = ctx;
  const [c, setC] = useState<CustomTargets>({ mode: "flick", concurrent: 3, duration_s: 60, area_h_deg: 35, area_v_deg: 12, reversal_s: 0.55, ...TIERS[1].p });
  const [tier, setTier] = useState<Tier | null>("medium");
  const [section, setSection] = useState<"build" | "library">("build");
  const set = (patch: Partial<CustomTargets>) => { setTier(null); setC({ ...c, ...patch }); };
  const usesDistance = c.mode === "flick" || c.mode === "multi";
  const usesSpeed = c.mode === "track" || c.mode === "strafe";
  const usesLimit = c.mode === "flick" || c.mode === "peek";
  const usesArea = c.mode === "multi" || c.mode === "track" || c.mode === "strafe";
  const history = view.practice.filter((p) => p.mode === c.mode);
  const best = history.length ? Math.max(...history.map((p) => p.hit_rate)) : null;
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
              <button key={m.id} className={c.mode === m.id ? "on" : ""} onClick={() => setC({ ...c, mode: m.id })}>
                <b>{pick(m.name, lang, m.id)}</b>
                <small>{pick(m.desc, lang, "")}</small>
              </button>
            ))}
          </div>
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
                {TIERS.map((t) => <button key={t.id} className={tier === t.id ? "on" : ""} onClick={() => { setTier(t.id); setC({ ...c, ...t.p }); }}>{pick(t.name, lang, t.id)}</button>)}
              </div>
              <Slider label={T("حجم الهدف", "Target size")} value={c.radius_deg} min={0.2} max={2} step={0.05} unit="°" set={(radius_deg) => set({ radius_deg })} />
              {usesDistance && <Slider label={T("أقرب هدف من المركز", "Nearest to centre")} value={c.min_distance_deg} min={2} max={60} step={1} unit="°" set={(v) => set({ min_distance_deg: v, max_distance_deg: Math.max(v, c.max_distance_deg) })} />}
              {usesDistance && <Slider label={T("أبعد هدف من المركز", "Farthest from centre")} value={c.max_distance_deg} min={2} max={60} step={1} unit="°" set={(v) => set({ max_distance_deg: v, min_distance_deg: Math.min(v, c.min_distance_deg) })} />}
              {usesArea && <Slider label={T("مساحة الحركة يمين ويسار", "Area left and right")} value={c.area_h_deg} min={5} max={60} step={1} unit="°" set={(area_h_deg) => set({ area_h_deg })} />}
              {usesArea && c.mode !== "strafe" && <Slider label={T("مساحة الحركة فوق وجوه", "Area up and down")} value={c.area_v_deg} min={2} max={35} step={1} unit="°" set={(area_v_deg) => set({ area_v_deg })} />}
              {c.mode === "strafe" && <Slider label={T("كل كم يغير اتجاهه", "Changes direction every")} value={c.reversal_s} min={0.25} max={1.5} step={0.05} unit="s" set={(reversal_s) => set({ reversal_s })} />}
              {usesSpeed && <Slider label={T("سرعة الهدف", "Target speed")} value={c.speed_deg_s} min={5} max={40} step={1} unit="°/s" set={(speed_deg_s) => set({ speed_deg_s })} />}
              {usesLimit && <Slider label={T("وقت كل هدف", "Time per target")} value={c.time_limit_s} min={0.3} max={5} step={0.1} unit="s" set={(time_limit_s) => set({ time_limit_s })} />}
              {c.mode === "multi" && <Slider label={T("عدد الأهداف سوه", "Targets at once")} value={c.concurrent} min={2} max={6} step={1} unit="" set={(concurrent) => set({ concurrent })} />}
              <Slider label={T("مدة الجلسة", "Session length")} value={c.duration_s} min={15} max={300} step={15} unit="s" set={(duration_s) => set({ duration_s })} />
            </section>
          </div>
          {history.length > 0 && (
            <section className="card">
              <span className="label">{T("آخر جلساتك بهذا النوع", "Your recent sessions in this mode")}</span>
              {history.slice(-5).reverse().map((p, i) => (
                <button className="line" key={i} onClick={() => p.file && go("analysis", p.file)}>
                  <span><b>{(p.hit_rate * 100).toFixed(0)}%</b><small>{whenText(p.at_ms ?? 0, lang)}</small></span>
                  <span className="chev">›</span>
                </button>
              ))}
            </section>
          )}
        </>
      )}
      {section === "library" && groups.map(({ g, drills }) => (
        <section key={g}>
          <h2 className="group">{pick(groupNames[g], lang, g)}</h2>
          <div className="drills">
            {drills.map((d) => {
              const level = view.plan?.levels[d];
              const runs = view.practice.filter((p) => p.name === d).length;
              return (
                <button key={d} disabled={!view.trainer_found} onClick={() => act("run_scenario", { scenario: d })}>
                  <b>{pick(drillNames[d], lang, d)}</b>
                  <small>{pick(drillInfo[d].purpose, lang, "")}</small>
                  <span className="muted small">{level !== undefined ? `${T("المستوى", "Level")} ${(level * 100).toFixed(0)}` : T("ما انلعب بالخطة", "Not in your plan yet")}{runs > 0 ? ` · ${runs}×` : ""}</span>
                </button>
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

function Coach({ ctx }: { ctx: Ctx }) {
  const { view, T, act, lang, go } = ctx;
  const [selected, setSelected] = useState(0);
  const [working, setWorking] = useState<string | null>(null);
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
            <span className="label">{T("تقاريرك", "Your reports")}</span>
            {reports.map((r, i) => (
              <button key={r.at_ms} className={i === selected ? "on" : ""} onClick={() => setSelected(i)}>
                <b>{scopeName(r.scope)}</b>
                <small>{whenText(r.at_ms, lang)} · {r.sessions} {T("جلسة", "sessions")}</small>
              </button>
            ))}
          </aside>
          <article className="card report">
            <div className="row between">
              <span className="muted small">{scopeName(report.scope)} · {whenText(report.at_ms, lang)}</span>
              <span className={`tag ${report.source === "ai" ? "good" : ""}`}>{report.source === "ai" ? T("ذكاء اصطناعي", "AI") : T("تحليل مدمج", "Built-in")}</span>
            </div>
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
  const [key, setKey] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [testing, setTesting] = useState(false);
  const [config, setConfig] = useState<CoachConfig | null>(null);
  useEffect(() => { call<CoachConfig>("get_coach_config").then(setConfig, () => undefined); }, []);
  const set = (patch: Partial<Settings>) => { setSaved(false); setTest(null); setS({ ...s, ...patch }); };
  const source = AI_SOURCES.find((a) => a.id === s.ai_provider) ?? AI_SOURCES[0];
  const needsAi = s.ai_provider !== "template";
  const hasKey = view.ai_keys_set.includes(s.ai_provider);
  const saveAll = async () => {
    await act("save_settings", { settings: s });
    if (key.trim()) await act("set_ai_key", { provider: s.ai_provider, key });
    if (config) await call("save_coach_config", { config });
    setKey("");
    setSaved(true);
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
  return (
    <>
      <PageHead title={T("الإعدادات", "Settings")} />
      <section className="card">
        <h2>{T("الماوس", "Mouse")}</h2>
        <div className="grid2">
          <Num label="DPI" value={s.dpi} step={50} min={50} set={(dpi) => set({ dpi })} />
          <Num label={T("الحساسية", "Sensitivity")} value={s.sens} step={0.01} min={0.01} set={(sens) => set({ sens })} />
        </div>
        <p className="small muted">{T("الحساب الحالي", "Current")}: {view.cm360.toFixed(1)} cm/360. {T("غيرها بس إذا غيرتها باللعبة، لأن المقارنات تعتمد عليها.", "Change this only when you change it in the game; comparisons depend on it.")}</p>
      </section>
      <section className="card">
        <h2>{T("التطبيق", "App")}</h2>
        <div className="grid2"><LangSelect ctx={ctx} value={s.locale} set={(locale) => set({ locale })} /></div>
        <Toggle checked={s.windowed} set={(windowed) => set({ windowed })} label={T("شغّل اللعبة بنافذة", "Run the game in a window")} />
        <Toggle checked={s.test_mode} set={(test_mode) => set({ test_mode })} label={T("وضع التجربة", "Test mode")} help={T("تمارين 20 ثانية وبدون انتظار يوم الراحة.", "20-second drills and no rest-day wait.")} />
      </section>
      <section className="card">
        <h2>{T("مصدر الذكاء الاصطناعي", "AI source")}</h2>
        <div className="grid2">
          <label className="field">{T("المصدر", "Source")}
            <select value={s.ai_provider} onChange={(e) => { const a = AI_SOURCES.find((x) => x.id === e.target.value)!; set({ ai_provider: a.id, ai_model: a.model, ai_base_url: a.id === "custom" ? s.ai_base_url : "" }); setKey(""); }}>
              <option value="template">{T("التحليل المدمج (بدون AI)", "Built-in analysis (no AI)")}</option>
              <option value="claude">Claude</option>
              <option value="openai">OpenAI</option>
              <option value="gemini">Gemini</option>
              <option value="openrouter">OpenRouter</option>
              <option value="custom">{T("مخصص (Custom)", "Custom")}</option>
            </select>
          </label>
          {needsAi && <label className="field">{T("اسم الموديل", "Model name")}<input dir="ltr" value={s.ai_model} onChange={(e) => set({ ai_model: e.target.value })} /></label>}
          {s.ai_provider === "custom" && <label className="field">{T("رابط الخادم (Base URL)", "Server URL (Base URL)")}<input dir="ltr" placeholder="http://localhost:11434/v1" value={s.ai_base_url} onChange={(e) => set({ ai_base_url: e.target.value })} /></label>}
          {needsAi && <label className="field">API key{source.needsKey ? "" : T(" (اختياري)", " (optional)")}<input dir="ltr" type="password" autoComplete="off" placeholder={hasKey ? T("محفوظ. اكتب مفتاح جديد لتغييره", "Saved. Type a new key to replace it") : ""} value={key} onChange={(e) => { setKey(e.target.value); setSaved(false); }} /></label>}
        </div>
        {needsAi && (
          <div className="row">
            <button className="btn ghost" disabled={testing} onClick={runTest}>{testing ? T("يفحص...", "Testing...") : T("افحص الاتصال", "Test connection")}</button>
            {test && <span className={test.ok ? "up" : "down"}>{test.text}</span>}
          </div>
        )}
        {needsAi && <p className="small muted">{T("المفتاح ينحفظ على جهازك بس، ويروح للمصدر اللي اخترته فقط.", "The key is stored only on this computer and sent only to the source you chose.")}</p>}
      </section>
      {config && (
        <section className="card">
          <h2>{T("شنو يشوف المدرب من بياناتك", "What the coach can see")}</h2>
          <div className="toggle-grid">
            {DATA_TOGGLES.map(([k, ar, en]) => <Toggle key={k} checked={Boolean(config[k])} set={(v) => { setSaved(false); setConfig({ ...config, [k]: v }); }} label={T(ar, en)} />)}
          </div>
          <label className="field spaced">{T("تعليمات إضافية للمدرب", "Extra instructions for the coach")}
            <textarea rows={3} value={config.extra_instructions} onChange={(e) => { setSaved(false); setConfig({ ...config, extra_instructions: e.target.value }); }} />
          </label>
          <p className="small muted">{T("نفس الإعدادات بملف JSON وتكدر تعدله:", "The same settings live in a JSON file you can edit:")} <code dir="ltr">{view.coach_config_path}</code></p>
        </section>
      )}
      <div className="row savebar">
        <button className="btn" onClick={saveAll}>{T("احفظ", "Save")}</button>
        {saved && <span className="up">{T("انحفظ", "Saved")}</span>}
      </div>
    </>
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
