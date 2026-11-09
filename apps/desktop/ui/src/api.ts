import { invoke } from "@tauri-apps/api/core";
import { mockCall } from "./mock";

export interface Settings {
  onboarded: boolean;
  dpi: number;
  sens: number;
  locale: string;
  windowed: boolean;
  test_mode: boolean;
  ai_provider: string;
  ai_base_url: string;
  ai_model: string;
}
export interface Skill {
  skill: string;
  raw: number;
  score: number;
  ci95: [number, number];
  n_evidence: number;
  reliability: "Low" | "Medium" | "High";
}
export interface Assessment {
  day: number;
  form: number;
  profile: { norms_version: string; skills: Skill[] };
}
export interface Finding {
  id: string;
  headline: string;
  why: string;
  severity: number;
  evidence: [string, number][];
  drills: string[];
}
export interface PlanItem {
  kind: string;
  drill: string;
  minutes: number;
  varied: boolean;
}
export type PlanState =
  | "Draft"
  | "Completed"
  | { Active: { block: number; day: number; completed: number } }
  | { AwaitingRetest: { block: number; due_day: number } }
  | { Evaluated: { block: number } }
  | { Paused: { block: number; day: number; completed: number } };
export interface Plan {
  state: PlanState;
  block_days: number;
  today_items: PlanItem[];
  item_index: number;
  retest_ready: boolean;
  levels: Record<string, number>;
}
export interface SkillChange {
  skill: string;
  pre: number;
  post: number;
  delta: number;
  verdict: string;
  verdict_text: string;
  trained: boolean;
}
export interface BlockResult {
  rows: SkillChange[];
  training_effect: number | null;
  adaptation: string;
  conditions_changed: boolean;
}
export interface Practice {
  day: number;
  name: string;
  hit_rate: number;
  difficulty: number | null;
}
export interface View {
  settings: Settings;
  today: number;
  cm360: number;
  trainer_found: boolean;
  claude_key_found: boolean;
  latest: Assessment | null;
  findings: Finding[];
  history: Assessment[];
  plan: Plan | null;
  block_result: BlockResult | null;
  practice: Practice[];
}
export interface CustomTargets {
  mode: "flick" | "track" | "strafe" | "peek" | "multi";
  radius_deg: number;
  min_distance_deg: number;
  max_distance_deg: number;
  speed_deg_s: number;
  time_limit_s: number;
  concurrent: number;
  duration_s: number;
}
export interface ChatAnswer {
  reply: { headline: string; explanation: string; tips: string[]; next_step: string };
  source: "ai" | "template";
  note: string | null;
}

const inTauri = "__TAURI_INTERNALS__" in window;

export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(command, args) : mockCall<T>(command, args);
}
