import { useEffect, useState, type ReactNode } from "react";
import { call, type ChatAnswer, type CustomTargets, type Plan, type Settings, type View } from "./api";
import { dict, drillNames, skillNames, type Key, type Lang } from "./i18n";

type Tab = "home" | "results" | "plan" | "practice" | "coach" | "settings";
const TABS: Tab[] = ["home", "results", "plan", "practice", "coach", "settings"];
const RUNS_TRAINER = new Set(["run_assessment", "run_next_item", "run_retest", "run_custom", "run_scenario"]);

interface Ctx {
  view: View;
  lang: Lang;
  t: (k: Key) => string;
  act: (command: string, args?: Record<string, unknown>) => Promise<void>;
  go: (tab: Tab) => void;
}

const label = (names: Record<string, [string, string]>, id: string, lang: Lang) =>
  names[id]?.[lang === "ar" ? 0 : 1] ?? id;

export function App() {
  const [view, setView] = useState<View | null>(null);
  const [tab, setTab] = useState<Tab>("home");
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

  const t = (k: Key) => dict[lang][k];
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

  if (!view) {
    return error ? <div className="banner err">{error}</div> : null;
  }
  const ctx: Ctx = { view, lang, t, act, go: setTab };
  if (!view.settings.onboarded) return <Onboarding ctx={ctx} />;

  return (
    <div className="shell">
      <nav className="nav" aria-label="main">
        <div className="brand">
          Aim<span>Forge</span>
        </div>
        {TABS.map((id) => (
          <button key={id} className={tab === id ? "on" : ""} onClick={() => setTab(id)}>
            {t(id)}
          </button>
        ))}
      </nav>
      <main>
        {!view.trainer_found && <div className="banner">{t("trainerMissing")}</div>}
        {error && <div className="banner err">{t("error")}: {error}</div>}
        {tab === "home" && <Home ctx={ctx} />}
        {tab === "results" && <Results ctx={ctx} />}
        {tab === "plan" && <PlanPage ctx={ctx} />}
        {tab === "practice" && <PracticePage ctx={ctx} />}
        {tab === "coach" && <Coach ctx={ctx} />}
        {tab === "settings" && <SettingsPage ctx={ctx} />}
      </main>
      {busy && RUNS_TRAINER.has(busy) && <div className="overlay">{t("running")}</div>}
    </div>
  );
}

function Onboarding({ ctx }: { ctx: Ctx }) {
  const { view, t, act } = ctx;
  const [s, setS] = useState(view.settings);
  return (
    <main style={{ margin: "8vh auto", maxWidth: 480 }}>
      <h1>{t("welcome")}</h1>
      <p className="muted">{t("welcomeNote")}</p>
      <div className="card grid2">
        <Num label="DPI" value={s.dpi} step={50} set={(dpi) => setS({ ...s, dpi })} />
        <Num label="Sensitivity" value={s.sens} step={0.01} set={(sens) => setS({ ...s, sens })} />
        <LangSelect t={t} value={s.locale} set={(locale) => setS({ ...s, locale })} />
      </div>
      <p>
        <button className="btn" onClick={() => act("save_settings", { settings: { ...s, onboarded: true } })}>
          {t("continueBtn")}
        </button>
      </p>
    </main>
  );
}

function Home({ ctx }: { ctx: Ctx }) {
  const { view, t, act, go } = ctx;
  const plan = view.plan;
  const canTrain = plan && typeof plan.state === "object" && "Active" in plan.state;
  return (
    <>
      <h1>AimForge</h1>
      <p className="muted">{t("tagline")}</p>
      <div className="card">
        <h2>{view.latest ? t("retakeAssessment") : t("startAssessment")}</h2>
        <p className="muted">{view.latest ? t("assessmentNote") : `${t("noAssessment")} ${t("assessmentNote")}`}</p>
        <div className="row">
          <button className={view.latest ? "btn ghost" : "btn"} disabled={!view.trainer_found} onClick={() => act("run_assessment")}>
            {view.latest ? t("retakeAssessment") : t("startAssessment")}
          </button>
          {canTrain && (
            <button className="btn" onClick={() => go("plan")}>
              {t("today")}
            </button>
          )}
        </div>
      </div>
      {view.findings.length > 0 && (
        <div className="card">
          <h2>{t("topProblems")}</h2>
          <Findings ctx={ctx} compact />
        </div>
      )}
    </>
  );
}

function Findings({ ctx, compact }: { ctx: Ctx; compact?: boolean }) {
  const { view, t, lang } = ctx;
  if (view.findings.length === 0) return <p className="muted">{t("noProblems")}</p>;
  return (
    <>
      {view.findings.map((f) => (
        <div className="finding" key={f.id}>
          <b>{f.headline}</b>
          <div className="muted">{f.why}</div>
          {!compact && (
            <div className="small muted">
              {t("evidence")}: {f.evidence.map(([k, v]) => `${k} = ${v.toFixed(2)}`).join(" · ")}
              <br />
              {t("suggestedDrills")}: {f.drills.map((d) => label(drillNames, d, lang)).join(" · ")}
            </div>
          )}
        </div>
      ))}
    </>
  );
}

function Results({ ctx }: { ctx: Ctx }) {
  const { view, t, lang } = ctx;
  if (!view.latest) {
    return (
      <>
        <h1>{t("results")}</h1>
        <div className="card muted">{t("noAssessment")}</div>
      </>
    );
  }
  return (
    <>
      <h1>{t("results")}</h1>
      <div className="card">
        <h2>{t("topProblems")}</h2>
        <Findings ctx={ctx} />
      </div>
      <div className="card">
        <h2>{t("skills")}</h2>
        {view.latest.profile.skills.map((s) => {
          const color = s.score >= 60 ? "var(--good)" : s.score >= 40 ? "var(--warn)" : "var(--bad)";
          return (
            <div className="skill" key={s.skill}>
              <div>
                {label(skillNames, s.skill, lang)}
                {s.reliability === "Low" && <div className="small muted">{t("lowReliability")}</div>}
              </div>
              <div className="track" title={`${t("confidence")}: ${s.ci95[0].toFixed(0)}-${s.ci95[1].toFixed(0)}`}>
                <div className="ci" style={{ left: `${s.ci95[0]}%`, width: `${s.ci95[1] - s.ci95[0]}%` }} />
                <div className="val" style={{ left: `${s.score}%`, background: color }} />
              </div>
              <div className="num">{s.score.toFixed(0)}</div>
            </div>
          );
        })}
        <p className="small muted">{t("provisional")}</p>
      </div>
    </>
  );
}

function PlanPage({ ctx }: { ctx: Ctx }) {
  const { view, t, act } = ctx;
  const plan = view.plan;
  return (
    <>
      <h1>{t("plan")}</h1>
      {!plan && <div className="card muted">{t("noPlan")}</div>}
      {plan && <PlanBody ctx={ctx} plan={plan} />}
      {view.block_result && (
        <div className="card">
          <h2>{t("blockResult")}</h2>
          {view.block_result.conditions_changed && <div className="banner">{t("conditionsChanged")}</div>}
          {view.block_result.training_effect !== null && (
            <p>
              {t("trainingEffect")}: <b>{view.block_result.training_effect.toFixed(1)}</b>
            </p>
          )}
          <table>
            <thead>
              <tr>
                <th>{t("skills")}</th>
                <th>{t("before")}</th>
                <th>{t("after")}</th>
                <th>{t("change")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {view.block_result.rows.map((r) => (
                <tr key={r.skill}>
                  <td>
                    {label(skillNames, r.skill, ctx.lang)} {r.trained && <span className="tag accent">{t("trainedTag")}</span>}
                  </td>
                  <td>{r.pre.toFixed(0)}</td>
                  <td>{r.post.toFixed(0)}</td>
                  <td>{r.delta > 0 ? `+${r.delta.toFixed(0)}` : r.delta.toFixed(0)}</td>
                  <td className="small">{r.verdict_text}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {typeof plan?.state === "object" && "Evaluated" in plan.state && (
            <p>
              <button className="btn" onClick={() => act("next_block")}>
                {t("nextBlock")}
              </button>
            </p>
          )}
        </div>
      )}
    </>
  );
}

function PlanBody({ ctx, plan }: { ctx: Ctx; plan: Plan }) {
  const { t, act, lang, view } = ctx;
  const state = plan.state;
  if (state === "Draft") {
    return (
      <div className="card">
        <h2>{t("planDraft")}</h2>
        <Findings ctx={ctx} compact />
        <p>
          <button className="btn" onClick={() => act("accept_plan")}>
            {t("acceptPlan")}
          </button>
        </p>
      </div>
    );
  }
  if (state === "Completed") return <div className="card">{t("planDone")}</div>;
  if ("Paused" in state) {
    return (
      <div className="card">
        <p>{t("planPaused")}</p>
        <button className="btn" onClick={() => act("accept_plan")}>
          {t("resume")}
        </button>
      </div>
    );
  }
  if ("AwaitingRetest" in state) {
    return (
      <div className="card">
        <h2>{t("retestTitle")}</h2>
        <p className="muted">{plan.retest_ready ? t("retestNote") : t("retestWait")}</p>
        <button className="btn" disabled={!plan.retest_ready || !view.trainer_found} onClick={() => act("run_retest")}>
          {t("startRetest")}
        </button>
      </div>
    );
  }
  if (!("Active" in state)) return null;
  const { block, day } = state.Active;
  return (
    <div className="card">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <h2 style={{ margin: 0 }}>
          {t("block")} {block} · {t("day")} {day} {t("of")} {plan.block_days}
        </h2>
        <button className="btn" disabled={!view.trainer_found} onClick={() => act("run_next_item")}>
          {t("startNext")}
        </button>
      </div>
      <div className="days">
        {Array.from({ length: plan.block_days }, (_, i) => (
          <i key={i} className={i < day ? "on" : ""} />
        ))}
      </div>
      <h2 style={{ marginTop: 14 }}>{t("today")}</h2>
      {plan.today_items.map((item, i) => (
        <div key={i} className={`item ${i < plan.item_index ? "done" : i === plan.item_index ? "now" : ""}`}>
          <span>{label(drillNames, item.drill, lang)}</span>
          <span className="muted small">
            {plan.levels[item.drill] !== undefined && `${t("level")} ${(plan.levels[item.drill] * 100).toFixed(0)} · `}
            {item.minutes} {t("minutes")}
          </span>
        </div>
      ))}
    </div>
  );
}

const DRILLS = Object.keys(drillNames).filter((d) => d !== "custom");

function PracticePage({ ctx }: { ctx: Ctx }) {
  const { view, t, act, lang } = ctx;
  const [c, setC] = useState<CustomTargets>({
    mode: "flick", radius_deg: 0.8, min_distance_deg: 10, max_distance_deg: 30,
    speed_deg_s: 20, time_limit_s: 2, concurrent: 3, duration_s: 60,
  });
  const set = (patch: Partial<CustomTargets>) => setC({ ...c, ...patch });
  const usesDistance = c.mode === "flick" || c.mode === "multi";
  const usesSpeed = c.mode === "track" || c.mode === "strafe";
  const usesLimit = c.mode === "flick" || c.mode === "peek";
  return (
    <>
      <h1>{t("practice")}</h1>
      <div className="card">
        <h2>{t("customTitle")}</h2>
        <div className="grid2">
          <label className="field">
            {t("mode")}
            <select value={c.mode} onChange={(e) => set({ mode: e.target.value as CustomTargets["mode"] })}>
              {(["flick", "track", "strafe", "peek", "multi"] as const).map((m) => (
                <option key={m} value={m}>
                  {t(m)}
                </option>
              ))}
            </select>
          </label>
          <Num label={t("radius")} value={c.radius_deg} step={0.05} min={0.1} set={(radius_deg) => set({ radius_deg })} />
          {usesDistance && <Num label={t("minDist")} value={c.min_distance_deg} step={1} min={1} set={(min_distance_deg) => set({ min_distance_deg })} />}
          {usesDistance && <Num label={t("maxDist")} value={c.max_distance_deg} step={1} min={1} set={(max_distance_deg) => set({ max_distance_deg })} />}
          {usesSpeed && <Num label={t("speed")} value={c.speed_deg_s} step={1} min={1} set={(speed_deg_s) => set({ speed_deg_s })} />}
          {usesLimit && <Num label={t("timeLimit")} value={c.time_limit_s} step={0.1} min={0.2} set={(time_limit_s) => set({ time_limit_s })} />}
          {c.mode === "multi" && <Num label={t("concurrent")} value={c.concurrent} step={1} min={1} set={(concurrent) => set({ concurrent })} />}
          <Num label={t("duration")} value={c.duration_s} step={5} min={5} set={(duration_s) => set({ duration_s })} />
        </div>
        <p>
          <button className="btn" disabled={!view.trainer_found} onClick={() => act("run_custom", { targets: c })}>
            {t("play")}
          </button>
        </p>
      </div>
      <div className="card">
        <h2>{t("stockDrills")}</h2>
        <div className="grid3">
          {DRILLS.map((d) => (
            <button key={d} className="btn ghost" disabled={!view.trainer_found} onClick={() => act("run_scenario", { scenario: d })}>
              {label(drillNames, d, lang)}
            </button>
          ))}
        </div>
      </div>
      {view.practice.length > 0 && (
        <div className="card">
          <h2>{t("recent")}</h2>
          {view.practice.slice(-6).reverse().map((p, i) => (
            <div className="item" key={i}>
              <span>{label(drillNames, p.name, lang)}</span>
              <span className="muted small">
                {t("hitRate")} {(p.hit_rate * 100).toFixed(0)}%
              </span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

function Coach({ ctx }: { ctx: Ctx }) {
  const { t } = ctx;
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<ChatAnswer | null>(null);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ask = async (q: string | null) => {
    setWaiting(true);
    setError(null);
    try {
      setAnswer(await call<ChatAnswer>("ask_coach", { question: q }));
    } catch (e) {
      setError(String(e));
    } finally {
      setWaiting(false);
    }
  };
  return (
    <>
      <h1>{t("coach")}</h1>
      <p className="muted">{t("coachIntro")}</p>
      <div className="card">
        <form
          className="row"
          onSubmit={(e) => {
            e.preventDefault();
            if (question.trim()) ask(question.trim());
          }}
        >
          <input className="grow" value={question} placeholder={t("askPlaceholder")} onChange={(e) => setQuestion(e.target.value)} />
          <button className="btn" disabled={waiting || !question.trim()}>
            {t("ask")}
          </button>
          <button type="button" className="btn ghost" disabled={waiting} onClick={() => ask(null)}>
            {t("explain")}
          </button>
        </form>
        {waiting && <p className="muted">{t("thinking")}</p>}
        {error && <div className="banner err">{error}</div>}
        {answer && !waiting && (
          <div className="answer">
            <span className="tag">{answer.source === "ai" ? t("aiAnswer") : t("fixedAnswer")}</span>
            <h2 style={{ marginTop: 8 }}>{answer.reply.headline}</h2>
            <div>{answer.reply.explanation}</div>
            {answer.reply.tips.length > 0 && (
              <ul>
                {answer.reply.tips.map((tip) => (
                  <li key={tip}>{label(drillNames, tip, ctx.lang)}</li>
                ))}
              </ul>
            )}
            {answer.reply.next_step && (
              <p>
                <b>{t("nextStep")}:</b> {answer.reply.next_step}
              </p>
            )}
            {answer.note && (
              <p className="small muted">
                {t("fallbackNote")} {answer.note}
              </p>
            )}
          </div>
        )}
      </div>
    </>
  );
}

function SettingsPage({ ctx }: { ctx: Ctx }) {
  const { view, t, act } = ctx;
  const [s, setS] = useState<Settings>(view.settings);
  const [saved, setSaved] = useState(false);
  const set = (patch: Partial<Settings>) => {
    setSaved(false);
    setS({ ...s, ...patch });
  };
  return (
    <>
      <h1>{t("settings")}</h1>
      <Section title={t("mouse")}>
        <div className="grid2">
          <Num label="DPI" value={s.dpi} step={50} min={50} set={(dpi) => set({ dpi })} />
          <Num label="Sensitivity" value={s.sens} step={0.01} min={0.01} set={(sens) => set({ sens })} />
          <LangSelect t={t} value={s.locale} set={(locale) => set({ locale })} />
        </div>
        <p className="small muted">cm/360: {view.cm360.toFixed(1)}</p>
        <label className="check">
          <input type="checkbox" checked={s.windowed} onChange={(e) => set({ windowed: e.target.checked })} />
          {t("windowedLabel")}
        </label>
        <label className="check" style={{ marginTop: 8 }}>
          <input type="checkbox" checked={s.test_mode} onChange={(e) => set({ test_mode: e.target.checked })} />
          {t("testModeLabel")}
        </label>
        <p className="small muted">{t("testModeHelp")}</p>
      </Section>
      <Section title={t("aiTitle")}>
        <div className="grid2">
          <label className="field">
            {t("aiProvider")}
            <select value={s.ai_provider} onChange={(e) => set({ ai_provider: e.target.value })}>
              <option value="template">{t("aiTemplate")}</option>
              <option value="local">{t("aiLocal")}</option>
              <option value="claude">{t("aiClaude")}</option>
            </select>
          </label>
          {s.ai_provider !== "template" && (
            <label className="field">
              {t("aiModel")}
              <input value={s.ai_model} onChange={(e) => set({ ai_model: e.target.value })} />
            </label>
          )}
          {s.ai_provider === "local" && (
            <label className="field">
              {t("aiUrl")}
              <input dir="ltr" value={s.ai_base_url} onChange={(e) => set({ ai_base_url: e.target.value })} />
            </label>
          )}
        </div>
        {s.ai_provider === "claude" && (
          <p className="small muted">{view.claude_key_found ? t("keyFound") : t("keyMissing")}</p>
        )}
      </Section>
      <p className="row">
        <button
          className="btn"
          onClick={async () => {
            await act("save_settings", { settings: s });
            setSaved(true);
          }}
        >
          {t("save")}
        </button>
        {saved && <span className="muted">{t("saved")}</span>}
      </p>
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="card">
      <h2>{title}</h2>
      {children}
    </div>
  );
}

function LangSelect({ t, value, set }: { t: (k: Key) => string; value: string; set: (v: string) => void }) {
  return (
    <label className="field">
      {t("language")}
      <select value={value} onChange={(e) => set(e.target.value)}>
        <option value="ar">العربية</option>
        <option value="en">English</option>
      </select>
    </label>
  );
}

function Num(props: { label: string; value: number; step: number; min?: number; set: (v: number) => void }) {
  return (
    <label className="field">
      {props.label}
      <input
        type="number"
        dir="ltr"
        value={props.value}
        step={props.step}
        min={props.min}
        onChange={(e) => {
          const v = e.target.valueAsNumber;
          if (Number.isFinite(v)) props.set(v);
        }}
      />
    </label>
  );
}
