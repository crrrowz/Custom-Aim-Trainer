// Browser-only stand-in for the Rust backend, so the screens can be worked on without the game.
import type { CoachConfig, CoachReport, Detail, Flick, SessionCard, Settings, Skill, View } from "./api";

const rows: [string, number, number, number, number, Skill["reliability"]][] = [
  ["S01Reaction", 244, 53, 47, 58, "High"],
  ["S02FlickSpeed", 4.1, 35, 28, 41, "Medium"],
  ["S03FlickPrecision", 2.3, 47, 40, 54, "Medium"],
  ["S04GainControl", 1.14, 28, 22, 37, "Medium"],
  ["S05MicroCorrection", 310, 55, 50, 60, "High"],
  ["S06TrackSmooth", 0.46, 37, 33, 41, "High"],
  ["S07TrackReactive", 260, 58, 49, 66, "Medium"],
  ["S08TargetSwitch", 780, 54, 44, 63, "Medium"],
  ["S09ClickTiming", 0.31, 22, 15, 30, "Medium"],
  ["S10HeadLevel", 1.1, 68, 60, 74, "Medium"],
  ["S13Consistency", 0.41, 53, 46, 60, "Low"],
];

const assessment = (shift: number, at_ms: number) => ({
  day: 20400,
  form: 0,
  at_ms,
  files: ["1_assess.v1.flick_static.aft", "4_assess.v1.smooth_track.aft"],
  profile: {
    norms_version: "norms.v0-provisional",
    skills: rows.map(([skill, raw, score, a, b, reliability]) => ({
      skill, raw, score: score + shift, ci95: [a + shift, b + shift] as [number, number], n_evidence: 96, reliability,
    })),
  },
});

const now = Date.now();
// The real backend writes findings and reports in the chosen language; the sample data does the same.
const en = () => view.settings.locale === "en";
const FINDINGS = {
  D6: { ar: ["نقر متسرّع", "تطلق قبل الوصول، فتضيع الرصاصة الأولى."], en: ["Clicking too early", "You fire before arriving, so the first bullet misses."] },
  D2: { ar: ["تجاوز منتظم للهدف", "حركتك الأولى تتجاوز الهدف بشكل متوقّع، وهذا يتصحّح بسرعة."], en: ["Steady overshoot", "Your first movement goes past the target by a predictable amount, which is quick to fix."] },
};
const localize = (v: View): View => {
  const lang = en() ? "en" : "ar";
  const tr = <F extends { id: string; headline: string; why: string }>(f: F): F => {
    const t = FINDINGS[f.id as keyof typeof FINDINGS]?.[lang];
    return t ? { ...f, headline: t[0], why: t[1] } : f;
  };
  return { ...v, findings: v.findings.map(tr), plan: v.plan && { ...v.plan, focus: v.plan.focus.map(tr) } };
};
let view: View = {
  settings: {
    onboarded: true, dpi: 800, sens: 0.35, locale: "ar", windowed: false, test_mode: true,
    ai_provider: "template", ai_base_url: "", ai_model: "", sound_volume: 60, crosshair: "00ff4c:0:4:0:1",
  },
  today: 20400, cm360: 46.6, trainer_found: true, claude_key_found: false,
  latest: assessment(6, now - 3600e3),
  findings: [
    { id: "D6", headline: "نقر متسرّع", why: "تطلق قبل الوصول، فتضيع الرصاصة الأولى.", severity: 0.62, evidence: [["S09.premature_rate", 0.31]], drills: ["confirm_click"] },
    { id: "D2", headline: "تجاوز منتظم للهدف", why: "حركتك الأولى تتجاوز الهدف بشكل متوقّع، وهذا يتصحّح بسرعة.", severity: 0.56, evidence: [["S04.median", 1.14], ["S04.iqr", 0.13]], drills: ["deceleration_ladder"] },
  ],
  history: [assessment(0, now - 8 * 86400e3), assessment(3, now - 4 * 86400e3), assessment(6, now - 3600e3)],
  plan: {
    state: { Active: { block: 1, day: 2, completed: 1 } }, block_days: 6, item_index: 1, retest_ready: false,
    levels: { confirm_click: 0.42, deceleration_ladder: 0.36 },
    today_done: false,
    focus: [{ id: "D6", headline: "نقر متسرّع", why: "تطلق قبل الوصول، فتضيع الرصاصة الأولى.", severity: 0.62, evidence: [["S09.premature_rate", 0.31]], drills: ["confirm_click"] }],
    today_items: [
      { kind: "WarmUp", drill: "warmup", minutes: 2, varied: false },
      { kind: "FocusA", drill: "confirm_click", minutes: 5, varied: false },
      { kind: "FocusB", drill: "deceleration_ladder", minutes: 4, varied: false },
      { kind: "Variation", drill: "confirm_click", minutes: 2, varied: true },
      { kind: "Maintenance", drill: "smooth_track", minutes: 2, varied: false },
      { kind: "MiniCheck", drill: "confirm_click", minutes: 1, varied: false },
    ],
  },
  block_result: null,
  sessions: [],
  reports: [],
  ai_keys_set: [],
  coach_config_path: "C:\\Users\\you\\AppData\\Roaming\\com.aimforge.desktop\\coach_config.json",
  notice: null,
  resume: null,
  estimate: null,
  practice: [
    { day: 20400, name: "warmup", hit_rate: 0.88, difficulty: 0.32, at_ms: now - 7200e3, file: "a.aft", mode: null },
    { day: 20400, name: "custom", hit_rate: 0.64, difficulty: null, at_ms: now - 5400e3, file: "b.aft", mode: "flick" },
    { day: 20400, name: "custom", hit_rate: 0.71, difficulty: null, at_ms: now - 1800e3, file: "c.aft", mode: "flick" },
  ],
};
// `?state=partial` shows a player whose only assessment stopped part-way.
if (typeof location !== "undefined" && new URLSearchParams(location.search).get("state") === "partial") {
  const est = assessment(0, now - 600e3);
  est.profile.skills = est.profile.skills.slice(0, 3);
  view = { ...view, latest: null, history: [], plan: null, estimate: est, resume: { done: 3, total: 8 } };
}
// `?state=new` shows the first-run screen.
if (typeof location !== "undefined" && new URLSearchParams(location.search).get("state") === "new") {
  view = { ...view, settings: { ...view.settings, onboarded: false } };
}

function mockFlick(i: number): Flick {
  const d = [8, 20, 40][i % 3];
  const angle = (i * 137 * Math.PI) / 180;
  const gain = 1.05 + 0.1 * Math.sin(i * 1.7);
  const hit = i % 4 !== 2;
  const [dx, dy] = [Math.cos(angle), Math.sin(angle)];
  const path = Array.from({ length: 150 }, (_, k) => {
    const t = k * 4;
    const u = Math.max(0, Math.min(1, (t - 210) / 180));
    const s = u * u * u * (10 - 15 * u + 6 * u * u);
    const c = Math.max(0, Math.min(1, (t - 420) / 110));
    const pos = d * (1 - gain * s + (gain - 1) * c * c * (3 - 2 * c) * (hit ? 1 : 0.4));
    const speed = 11 * d * 30 * u * u * (1 - u) * (1 - u) + (c > 0 && c < 1 ? 30 * d * 0.1 * c * (1 - c) * 4 : 0);
    return { t_ms: t, x: -dx * pos, y: -dy * pos, speed };
  });
  const last = path[path.length - 11];
  return {
    id: i + 1, t_s: i * 1.45, radius_deg: i % 2 ? 0.8 : 0.4, distance_deg: d, octant: i % 8,
    reaction_ms: 200 + 40 * Math.sin(i), anticipated: false, time_to_hit_ms: hit ? 520 + 60 * Math.sin(i * 0.6) + i * 1.5 : null,
    gain, corrections: 1 + (i % 3), hit, premature: !hit && i % 8 === 2, path,
    onset_ms: 214, primary_end_ms: 392, stable_ms: hit ? 540 : null, shot_ms: last.t_ms, endpoint: [last.x, last.y],
  };
}

const detail = (path: string): Detail => {
  const tracking = path.includes("track");
  const flicks = tracking ? [] : Array.from({ length: 48 }, (_, i) => mockFlick(i));
  const hits = flicks.filter((f) => f.hit).length;
  return {
    scenario_id: tracking ? "assess.v1.smooth_track" : "assess.v1.flick_static",
    duration_s: tracking ? 45 : 70, targets: flicks.length, hits,
    accuracy: flicks.length ? hits / flicks.length : null,
    median_reaction_ms: tracking ? null : 214, median_time_to_hit_ms: tracking ? null : 561,
    median_gain: tracking ? null : 1.12, paused_s: 0, flicks,
    tracking: tracking ? Array.from({ length: 180 }, (_, i) => ({ t_s: i * 0.25, error_deg: 0.7 + 0.4 * Math.sin(i / 7) + 0.2 * Math.sin(i * 1.3), on_target: 0.5 })) : [],
    time_on_target: tracking ? 0.46 : null, tracking_radius_deg: tracking ? 0.6 : null, timing_suspect: false,
  };
};

const names = ["assess.v1.flick_static", "drill.confirm_click", "custom.practice", "assess.v1.smooth_track", "drill.deceleration_ladder"];
view.sessions = Array.from({ length: 22 }, (_, i): SessionCard => ({
  path: names[i % 5].includes("track") ? `${i}-track.aft` : `${i}.aft`,
  at_ms: now - i * 0.45 * 3600e3 - (i > 13 ? 86400e3 : 0),
  assessment: names[i % 5].startsWith("assess"),
  scenario: names[i % 5],
  targets: names[i % 5].includes("track") ? 0 : 48,
  hits: names[i % 5].includes("track") ? 0 : 40 - i,
  accuracy: names[i % 5].includes("track") ? null : (40 - i) / 48,
  median_reaction_ms: names[i % 5].includes("track") ? null : 230 + i * 3,
  median_time_to_hit_ms: names[i % 5].includes("track") ? null : 540 + i * 6,
  time_on_target: names[i % 5].includes("track") ? 0.46 : null,
  duration_s: 70,
  timing_suspect: false,
  partial: i === 3,
}));

let config: CoachConfig = { player_settings: true, skills: true, findings: true, recent_sessions: true, session_limit: 30, directions: true, plan: true, block_result: true, extra_instructions: "" };

export async function mockCall<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (command === "session_detail") return detail(args!.path as string) as T;
  if (command === "get_coach_config") return config as T;
  if (command === "save_coach_config") { config = args!.config as CoachConfig; return undefined as T; }
  if (command === "test_ai") return "ok" as T;
  if (command === "save_share_card") return `C:\\Users\\you\\Pictures\\AimForge\\${args!.name}.png` as T;
  if (command === "delete_sessions") view = { ...view, sessions: view.sessions.filter((c) => !(args!.paths as string[]).includes(c.path)) };
  if (command === "delete_reports") view = { ...view, reports: view.reports.filter((r) => !(args!.atMs as number[]).includes(r.at_ms)) };
  if (command === "save_settings") view = { ...view, settings: args!.settings as Settings };
  if (command === "generate_report") {
    const report: CoachReport = {
      at_ms: Date.now(), scope: args!.scope as string, sessions: 22, source: "built-in", note: null,
      reply: en() ? {
        summary: "I reviewed 22 sessions. Your accuracy averages 61% and rose by 8 points in the latest sessions.",
        strengths: ["Head level: score 74", "Reactive tracking: score 64"],
        weaknesses: [{ title: "Clicking too early (click timing)", detail: "You fire before arriving, so the first bullet misses.", fix: "Drill: confirm click" }],
        next_steps: ["Confirm click", "Deceleration ladder"],
      } : {
        summary: "راجعت 22 جلسة. دقتك بالمعدل 61%، وتحسنت بـ 8 نقطة بالجلسات الأخيرة.",
        strengths: ["مستوى الرأس: درجتك 74", "التتبع التفاعلي: درجتك 64"],
        weaknesses: [{ title: "نقر متسرّع (توقيت النقر)", detail: "تطلق قبل الوصول، فتضيع الرصاصة الأولى.", fix: "تمرين: نقر بعد الوصول" }],
        next_steps: ["نقر بعد الوصول", "سلّم التخفيف"],
      },
    };
    view = { ...view, reports: [report, ...view.reports] };
  }
  return localize(view) as T;
}
