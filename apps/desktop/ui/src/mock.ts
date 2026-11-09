// Browser-only stand-in for the Rust backend, so the screens can be worked on without the game.
import type { ChatAnswer, Settings, Skill, View } from "./api";

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

let view: View = {
  settings: {
    onboarded: true,
    dpi: 800,
    sens: 0.35,
    locale: "ar",
    windowed: false,
    test_mode: true,
    ai_provider: "template",
    ai_base_url: "http://localhost:11434/v1",
    ai_model: "llama3.2",
  },
  today: 20400,
  cm360: 46.6,
  trainer_found: true,
  claude_key_found: false,
  latest: {
    day: 20400,
    form: 0,
    profile: {
      norms_version: "norms.v0-provisional",
      skills: rows.map(([skill, raw, score, a, b, reliability]) => ({
        skill,
        raw,
        score,
        ci95: [a, b],
        n_evidence: 96,
        reliability,
      })),
    },
  },
  findings: [
    { id: "D6", headline: "نقر متسرّع", why: "تطلق قبل الوصول، فتضيع الرصاصة الأولى.", severity: 0.62, evidence: [["S09.premature_rate", 0.31]], drills: ["confirm_click"] },
    { id: "D2", headline: "تجاوز منتظم للهدف", why: "حركتك الأولى تتجاوز الهدف بشكل متوقّع، وهذا يتصحّح بسرعة.", severity: 0.56, evidence: [["S04.median", 1.14], ["S04.iqr", 0.13]], drills: ["deceleration_ladder"] },
  ],
  history: [],
  plan: {
    state: { Active: { block: 1, day: 2, completed: 1 } },
    block_days: 6,
    item_index: 1,
    retest_ready: false,
    levels: { confirm_click: 0.42, deceleration_ladder: 0.36 },
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
  practice: [{ day: 20400, name: "warmup", hit_rate: 0.88, difficulty: 0.32 }],
};

export async function mockCall<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (command === "save_settings") view = { ...view, settings: args!.settings as Settings };
  if (command === "ask_coach") {
    const answer: ChatAnswer = {
      reply: {
        headline: "نقر متسرّع",
        explanation: "تطلق قبل الوصول، فتضيع الرصاصة الأولى.",
        tips: ["confirm_click"],
        next_step: "ابدأ جلسة اليوم من صفحة الخطة.",
      },
      source: "template",
      note: null,
    };
    return answer as T;
  }
  return view as T;
}
