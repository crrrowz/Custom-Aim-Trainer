import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { call, type ChatAnswer, type CustomTargets, type Detail, type Flick, type Plan, type Settings, type View } from "./api";
import { AimPath, DirectionWheel, ReactionTimeline, ShotScatter, SkillLines, SkillRadar, Spark, SpeedChart, TrackingChart, type Series } from "./charts";
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
  ["coach", "المدرب", "Coach", "M4 5h16v11H9l-5 4z"],
  ["settings", "الإعدادات", "Settings", "M4 7h10m4 0h2M4 17h2m4 0h10M14 5v4m-4 6v4"],
];

interface Recording { key: string; path: string; title: string; when: number; group: string }

function recordingsOf(view: View, lang: Lang, T: Ctx["T"]): Recording[] {
  const out: Recording[] = [];
  view.history.forEach((a, n) => {
    for (const path of a.files ?? []) {
      const m = path.match(/assess\.v1\.([a-z_]+)\.aft$/);
      const scenario = m?.[1] ?? "";
      if (scenario === "warmup") continue;
      out.push({ key: path, path, title: pick(scenarioNames[scenario], lang, scenario), when: a.at_ms ?? 0, group: `${T("تقييم", "Assessment")} ${n + 1}` });
    }
  });
  for (const p of view.practice) {
    if (p.file) out.push({ key: p.file, path: p.file, title: pick(drillNames[p.name], lang, p.name), when: p.at_ms ?? 0, group: T("تدريب", "Training") });
  }
  return out.sort((a, b) => b.when - a.when);
}

const whenText = (ms: number, lang: Lang) =>
  ms ? new Date(ms).toLocaleString(lang === "ar" ? "ar-IQ" : "en-GB", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "";

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
      setError(String(e));
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
        {tab === "home" && <Home ctx={ctx} />}
        {tab === "analysis" && <Analysis ctx={ctx} initial={recording} />}
        {tab === "progress" && <Progress ctx={ctx} />}
        {tab === "plan" && <PlanPage ctx={ctx} />}
        {tab === "practice" && <Practice ctx={ctx} />}
        {tab === "coach" && <Coach ctx={ctx} />}
        {tab === "settings" && <SettingsPage ctx={ctx} />}
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
  const recordings = recordingsOf(view, lang, T).slice(0, 5);
  const plan = view.plan;
  const active = plan && typeof plan.state === "object" && "Active" in plan.state ? plan.state.Active : null;
  const retest = plan && typeof plan.state === "object" && "AwaitingRetest" in plan.state;

  let hero: { title: string; sub: string; cta: string; run: () => void };
  if (!view.latest) {
    hero = { title: T("ابدأ بالتقييم", "Start with an assessment"), sub: T("7 اختبارات بحوالي 9 دقايق، وبعدها تعرف وين مشكلتك بالضبط.", "7 tests in about 9 minutes, then you will know exactly what to fix."), cta: T("ابدأ التقييم", "Start assessment"), run: () => act("run_assessment") };
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
          <button className="btn" onClick={hero.run}>{hero.cta}</button>
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
          {recordings.length === 0 && <Empty text={T("جلساتك تتسجل هنا.", "Your sessions appear here.")} />}
          {recordings.map((r) => (
            <button className="line" key={r.key} onClick={() => go("analysis", r.path)}>
              <span><b>{r.title}</b><small>{r.group} · {whenText(r.when, lang)}</small></span>
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

function Analysis({ ctx, initial }: { ctx: Ctx; initial?: string }) {
  const { view, T, lang, go } = ctx;
  const recordings = useMemo(() => recordingsOf(view, lang, T), [view, lang]);
  const [path, setPath] = useState(initial ?? recordings[0]?.path);
  const [detail, setDetail] = useState<Detail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view2, setView2] = useState<"replay" | "timeline" | "map" | "dirs">("replay");

  useEffect(() => {
    if (!path) return;
    setDetail(null);
    setError(null);
    call<Detail>("session_detail", { path }).then(setDetail, (e) => setError(String(e)));
  }, [path]);

  if (recordings.length === 0) {
    return (
      <>
        <PageHead title={T("التحليل", "Analysis")} />
        <Empty text={T("بعد أول جلسة تشوف هنا كل حركة سويتها: مسار المؤشر، السرعة، ووين نزلت طلقاتك.", "After your first session you will see every movement here: crosshair path, speed, and where your shots landed.")} action={<button className="btn" onClick={() => go("home")}>{T("ابدأ", "Get started")}</button>} />
      </>
    );
  }

  const isTracking = detail ? detail.tracking.length > 0 : false;
  const tabs: [typeof view2, string][] = [["replay", T("إعادة الحركة", "Aim replay")], ["timeline", T("عبر الجلسة", "Over the session")], ["map", T("خريطة الطلقات", "Shot map")], ["dirs", T("الاتجاهات", "Directions")]];

  return (
    <>
      <PageHead title={T("التحليل", "Analysis")}>
        <select aria-label={T("الجلسة", "Session")} value={path} onChange={(e) => setPath(e.target.value)}>
          {recordings.map((r) => <option key={r.key} value={r.path}>{r.group} · {r.title} · {whenText(r.when, lang)}</option>)}
        </select>
      </PageHead>
      {error && <div className="banner err">{error}</div>}
      {!detail && !error && <div className="skeleton" />}
      {detail && (
        <>
          <div className="stats">
            {isTracking ? (
              <>
                <Stat value={`${((detail.time_on_target ?? 0) * 100).toFixed(0)}%`} label={T("على الهدف", "On target")} />
                <Stat value={`${(detail.tracking.reduce((a, b) => a + b.error_deg, 0) / detail.tracking.length).toFixed(2)}°`} label={T("متوسط الابتعاد", "Average error")} />
                <Stat value={`${detail.duration_s.toFixed(0)} s`} label={T("المدة", "Length")} />
              </>
            ) : (
              <>
                <Stat value={detail.accuracy === null ? "—" : `${(detail.accuracy * 100).toFixed(0)}%`} label={T("الإصابة", "Accuracy")} hint={`${detail.hits} / ${detail.targets}`} />
                <Stat value={detail.median_reaction_ms === null ? "—" : `${detail.median_reaction_ms.toFixed(0)} ms`} label={T("رد الفعل", "Reaction")} />
                <Stat value={detail.median_time_to_hit_ms === null ? "—" : `${detail.median_time_to_hit_ms.toFixed(0)} ms`} label={T("وقت الإصابة", "Time to hit")} />
                <Stat value={detail.median_gain === null ? "—" : detail.median_gain.toFixed(2)} label={T("نسبة الحركة الأولى", "First movement")} hint={T("المثالي 0.92–1.00", "Ideal 0.92–1.00")} />
              </>
            )}
          </div>
          {isTracking ? (
            <section className="card">
              <h2>{T("بُعد المؤشر عن الهدف خلال الجلسة", "Crosshair distance from target over the session")}</h2>
              <p className="muted small">{T("المنطقة الخضراء تعني أنت على الهدف.", "The green band means you are on target.")}</p>
              <TrackingChart bins={detail.tracking} radius={detail.tracking_radius_deg ?? 0.6} yLabel="°" />
            </section>
          ) : detail.flicks.length === 0 ? (
            <Empty text={T("ما انسجلت أهداف بهذي الجلسة.", "No targets were recorded in this session.")} />
          ) : (
            <>
              <div className="tabs" role="tablist">
                {tabs.map(([id, name]) => <button key={id} role="tab" aria-selected={view2 === id} className={view2 === id ? "on" : ""} onClick={() => setView2(id)}>{name}</button>)}
              </div>
              {view2 === "replay" && <Replay key={path} detail={detail} ctx={ctx} />}
              {view2 === "timeline" && (
                <section className="card">
                  <h2>{T("وقت كل هدف عبر الجلسة", "Time per target across the session")}</h2>
                  <p className="muted small">{T("كل نقطة هدف. الخط الأبيض هو الاتجاه العام، إذا يصعد فأنت تتعب.", "Each dot is a target. The white line is the trend; rising means you are tiring.")}</p>
                  <ReactionTimeline flicks={detail.flicks} yLabel="ms" hitLabel={T("إصابة", "Hit")} missLabel={T("خطأ", "Miss")} />
                </section>
              )}
              {view2 === "map" && (
                <section className="card split">
                  <div>
                    <h2>{T("وين نزلت طلقاتك", "Where your shots landed")}</h2>
                    <p className="muted small">{T("الدائرة هي الهدف، وعلامة + هي مركز كل طلقاتك. إذا مزاحة فعندك انحياز ثابت لجهة.", "The circle is the target and + is the centre of all your shots. If it is off-centre you have a consistent bias.")}</p>
                    <Legend items={[["#3ddc97", T("إصابة", "Hit")], ["#ff6b6b", T("خطأ", "Miss")]]} />
                  </div>
                  <ShotScatter flicks={detail.flicks} hitLabel={T("إصابة", "Hit")} missLabel={T("خطأ", "Miss")} />
                </section>
              )}
              {view2 === "dirs" && (
                <section className="card split">
                  <div>
                    <h2>{T("سرعتك بكل اتجاه", "Your speed in each direction")}</h2>
                    <p className="muted small">{T("كل ما طال القطاع كنت أبطأ. البرتقالي هو الأبطأ. مرّر الماوس حتى تشوف الرقم.", "A longer wedge means slower. Orange is the slowest. Hover for the number.")}</p>
                  </div>
                  <DirectionWheel flicks={detail.flicks} unit="ms" />
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
  const list = useMemo(() => flickSort[filter](detail.flicks), [detail, filter]);
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
  if (view.history.length === 0) {
    return (<><PageHead title={T("التقدم", "Progress")} /><Empty text={T("منحنى كل مهارة يطلع هنا بعد أول تقييم.", "A curve for each skill appears here after your first assessment.")} /></>);
  }
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : p.length >= 4 ? p : [...p, id]));
  const series: Series[] = picked.map((id) => ({
    id,
    name: pick(skillNames[id], lang, id),
    points: view.history.map((a, i) => {
      const s = a.profile.skills.find((k) => k.skill === id);
      return { label: `#${i + 1}`, v: s?.score ?? 0, lo: s?.ci95[0] ?? 0, hi: s?.ci95[1] ?? 0 };
    }),
  }));
  const latest = view.latest!;
  return (
    <>
      <PageHead title={T("التقدم", "Progress")} sub={T("كل نقطة تقييم كامل، والشريط حول الخط هو نطاق الثقة.", "Each point is a full assessment; the band around the line is the confidence range.")} />
      <div className="chips">
        {ids.map((id) => <button key={id} className={picked.includes(id) ? "on" : ""} onClick={() => toggle(id)}>{pick(skillNames[id], lang, id)}</button>)}
      </div>
      <section className="card">
        {series.length > 1 && <Legend items={series.map((s, i) => [["#3ddc97", "#5aa9ff", "#e5ad45", "#c58bff"][i], s.name])} />}
        {view.history.length === 1 && <p className="muted small">{T("تحتاج تقييمين حتى يطلع منحنى.", "Two assessments are needed for a curve.")}</p>}
        <SkillLines series={series} />
      </section>
      <section className="card">
        <h2>{T("كل المهارات", "All skills")}</h2>
        {latest.profile.skills.map((s) => {
          const color = s.score >= 60 ? "var(--good)" : s.score >= 40 ? "var(--warn)" : "var(--bad)";
          return (
            <div className="skill" key={s.skill}>
              <div>{pick(skillNames[s.skill], lang, s.skill)}{s.reliability === "Low" && <small className="muted"> · {T("بيانات قليلة", "little data")}</small>}</div>
              <div className="track" title={`${s.ci95[0].toFixed(0)}–${s.ci95[1].toFixed(0)}`}>
                <div className="ci" style={{ left: `${s.ci95[0]}%`, width: `${Math.max(s.ci95[1] - s.ci95[0], 1)}%` }} />
                <div className="val" style={{ left: `${s.score}%`, background: color }} />
              </div>
              <div className="num">{s.score.toFixed(0)}</div>
            </div>
          );
        })}
        <p className="small muted">{T("الدرجات أولية لحد ما تتجمع بيانات لاعبين.", "Scores are provisional until player data exists.")}</p>
      </section>
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
      {view.findings.length > 0 && (
        <section className="card">
          <h2>{T("ليش هذي الخطة", "Why this plan")}</h2>
          {view.findings.map((f) => (
            <div className="finding" key={f.id}>
              <b>{f.headline}</b>
              <p className="muted">{f.why}</p>
              <div className="evidence">
                {f.evidence.map(([k, v]) => {
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
          {view.block_result.conditions_changed && <div className="banner">{T("تغيرت الحساسية أو الـ DPI، فالمقارنة غير دقيقة.", "Sensitivity or DPI changed, so this comparison is unreliable.")}</div>}
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
        <button className="btn" disabled={!view.trainer_found || plan.item_index >= plan.today_items.length} onClick={() => act("run_next_item")}>{T("شغّل التمرين الجاي", "Play next drill")}</button>
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
  const [c, setC] = useState<CustomTargets>({ mode: "flick", concurrent: 3, duration_s: 60, ...TIERS[1].p });
  const [tier, setTier] = useState<Tier | null>("medium");
  const [section, setSection] = useState<"build" | "library">("build");
  const set = (patch: Partial<CustomTargets>) => { setTier(null); setC({ ...c, ...patch }); };
  const usesDistance = c.mode === "flick" || c.mode === "multi";
  const usesSpeed = c.mode === "track" || c.mode === "strafe";
  const usesLimit = c.mode === "flick" || c.mode === "peek";
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
              {usesDistance && <Slider label={T("أقرب مسافة", "Nearest")} value={c.min_distance_deg} min={2} max={c.max_distance_deg} step={1} unit="°" set={(min_distance_deg) => set({ min_distance_deg })} />}
              {usesDistance && <Slider label={T("أبعد مسافة", "Farthest")} value={c.max_distance_deg} min={c.min_distance_deg} max={60} step={1} unit="°" set={(max_distance_deg) => set({ max_distance_deg })} />}
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

interface Turn { q: string | null; a: ChatAnswer | null; err?: string }

function Coach({ ctx }: { ctx: Ctx }) {
  const { T, lang, go, view } = ctx;
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const waiting = turns.some((t) => !t.a && !t.err);
  const ask = async (q: string | null) => {
    setQuestion("");
    setTurns((p) => [...p, { q, a: null }]);
    const update = (patch: Partial<Turn>) => setTurns((p) => p.map((t, i) => (i === p.length - 1 ? { ...t, ...patch } : t)));
    try { update({ a: await call<ChatAnswer>("ask_coach", { question: q }) }); } catch (e) { update({ err: String(e) }); }
  };
  const ideas = [T("ليش أخطي الطلقة الأولى؟", "Why do I miss my first shot?"), T("شنو أهم شي أصلحه؟", "What should I fix first?"), T("هل حساسيتي مشكلة؟", "Is my sensitivity the problem?")];
  const sourceName = view.settings.ai_provider === "template" ? T("نصوص ثابتة", "Fixed text") : view.settings.ai_provider === "local" ? T("موديل محلي", "Local model") : "Claude";
  return (
    <>
      <PageHead title={T("المدرب", "Coach")} sub={T("الأرقام تجي من القياس، والمدرب يشرحها بس.", "The numbers come from measurement; the coach only explains them.")}>
        <button className="tag link" onClick={() => go("settings")}>{sourceName}</button>
      </PageHead>
      <section className="card chat">
        {turns.length === 0 && (
          <div className="ideas">
            <button className="btn" onClick={() => ask(null)}>{T("اشرح نتائجي", "Explain my results")}</button>
            {ideas.map((q) => <button key={q} className="btn ghost" onClick={() => ask(q)}>{q}</button>)}
          </div>
        )}
        {turns.map((t, i) => (
          <div key={i}>
            <div className="bubble me">{t.q ?? T("اشرح نتائجي", "Explain my results")}</div>
            {!t.a && !t.err && <div className="bubble"><span className="muted">{T("يكتب...", "Writing...")}</span></div>}
            {t.err && <div className="banner err">{t.err}</div>}
            {t.a && (
              <div className="bubble">
                <b>{t.a.reply.headline}</b>
                <p>{t.a.reply.explanation}</p>
                {t.a.reply.tips.length > 0 && <ul>{t.a.reply.tips.map((tip) => <li key={tip}>{pick(drillNames[tip], lang, tip)}</li>)}</ul>}
                {t.a.reply.next_step && <p><b>{T("الخطوة الجاية", "Next step")}:</b> {t.a.reply.next_step}</p>}
                {t.a.note && <p className="small muted">{T("استخدمنا النص الثابت لأن:", "Showing fixed text because:")} {t.a.note}</p>}
              </div>
            )}
          </div>
        ))}
        <form className="row composer" onSubmit={(e) => { e.preventDefault(); if (question.trim()) ask(question.trim()); }}>
          <input className="grow" value={question} placeholder={T("اسأل عن نتائجك...", "Ask about your results...")} onChange={(e) => setQuestion(e.target.value)} />
          <button className="btn" disabled={waiting || !question.trim()}>{T("اسأل", "Ask")}</button>
        </form>
      </section>
    </>
  );
}

function SettingsPage({ ctx }: { ctx: Ctx }) {
  const { view, T, act } = ctx;
  const [s, setS] = useState<Settings>(view.settings);
  const [saved, setSaved] = useState(false);
  const set = (patch: Partial<Settings>) => { setSaved(false); setS({ ...s, ...patch }); };
  return (
    <>
      <PageHead title={T("الإعدادات", "Settings")} />
      <section className="card">
        <h2>{T("الماوس", "Mouse")}</h2>
        <div className="grid2">
          <Num label="DPI" value={s.dpi} step={50} min={50} set={(dpi) => set({ dpi })} />
          <Num label={T("الحساسية", "Sensitivity")} value={s.sens} step={0.01} min={0.01} set={(sens) => set({ sens })} />
        </div>
        <p className="small muted">{T("الحساب الحالي", "Current")}: {view.cm360.toFixed(1)} cm/360</p>
      </section>
      <section className="card">
        <h2>{T("التطبيق", "App")}</h2>
        <div className="grid2"><LangSelect ctx={ctx} value={s.locale} set={(locale) => set({ locale })} /></div>
        <Toggle checked={s.windowed} set={(windowed) => set({ windowed })} label={T("شغّل اللعبة بنافذة", "Run the game in a window")} />
        <Toggle checked={s.test_mode} set={(test_mode) => set({ test_mode })} label={T("وضع التجربة", "Test mode")} help={T("تمارين 20 ثانية وبدون انتظار يوم الراحة.", "20-second drills and no rest-day wait.")} />
      </section>
      <section className="card">
        <h2>{T("الذكاء الاصطناعي", "AI")}</h2>
        <div className="grid2">
          <label className="field">{T("المصدر", "Source")}
            <select value={s.ai_provider} onChange={(e) => set({ ai_provider: e.target.value })}>
              <option value="template">{T("نصوص ثابتة (بدون AI)", "Fixed text (no AI)")}</option>
              <option value="local">{T("محلي (Ollama / LM Studio)", "Local (Ollama / LM Studio)")}</option>
              <option value="claude">Claude</option>
            </select>
          </label>
          {s.ai_provider !== "template" && <label className="field">{T("الموديل", "Model")}<input dir="ltr" value={s.ai_model} onChange={(e) => set({ ai_model: e.target.value })} /></label>}
          {s.ai_provider === "local" && <label className="field">{T("رابط الخادم", "Server URL")}<input dir="ltr" value={s.ai_base_url} onChange={(e) => set({ ai_base_url: e.target.value })} /></label>}
        </div>
        {s.ai_provider === "claude" && <p className="small muted">{view.claude_key_found ? T("مفتاح ANTHROPIC_API_KEY موجود.", "ANTHROPIC_API_KEY found.") : T("لازم تضبط ANTHROPIC_API_KEY بالويندوز وتعيد فتح التطبيق.", "Set ANTHROPIC_API_KEY in Windows and reopen the app.")}</p>}
      </section>
      <div className="row savebar">
        <button className="btn" onClick={async () => { await act("save_settings", { settings: s }); setSaved(true); }}>{T("احفظ", "Save")}</button>
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
      <input type="number" dir="ltr" value={props.value} step={props.step} min={props.min} onChange={(e) => { const v = e.target.valueAsNumber; if (Number.isFinite(v)) props.set(v); }} />
    </label>
  );
}
