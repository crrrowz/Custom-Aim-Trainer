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
  sound_volume: number;
  crosshair: string;
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
  at_ms?: number;
  files?: string[];
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
  today_done: boolean;
  focus: Finding[];
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
  settings_before: [number, number];
  settings_after: [number, number];
}
export interface Practice {
  day: number;
  name: string;
  hit_rate: number;
  difficulty: number | null;
  level_before?: number | null;
  at_ms?: number;
  file?: string | null;
  mode?: string | null;
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
  sessions: SessionCard[];
  reports: CoachReport[];
  ai_keys_set: string[];
  coach_config_path: string;
  notice: string | null;
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
  area_h_deg: number;
  area_v_deg: number;
  reversal_s: number;
}
export interface PathPoint {
  t_ms: number;
  x: number;
  y: number;
  speed: number;
}
export interface Flick {
  id: number;
  t_s: number;
  radius_deg: number;
  distance_deg: number | null;
  octant: number | null;
  reaction_ms: number | null;
  anticipated: boolean;
  time_to_hit_ms: number | null;
  gain: number | null;
  corrections: number;
  hit: boolean;
  premature: boolean;
  path: PathPoint[];
  onset_ms: number | null;
  primary_end_ms: number | null;
  stable_ms: number | null;
  shot_ms: number | null;
  endpoint: [number, number] | null;
}
export interface TrackBin {
  t_s: number;
  error_deg: number;
  on_target: number;
}
export interface Detail {
  scenario_id: string;
  duration_s: number;
  targets: number;
  hits: number;
  accuracy: number | null;
  median_reaction_ms: number | null;
  median_time_to_hit_ms: number | null;
  median_gain: number | null;
  paused_s: number;
  flicks: Flick[];
  tracking: TrackBin[];
  time_on_target: number | null;
  tracking_radius_deg: number | null;
  timing_suspect: boolean;
}
export interface SessionCard {
  path: string;
  at_ms: number;
  assessment: boolean;
  scenario: string;
  targets: number;
  hits: number;
  accuracy: number | null;
  median_reaction_ms: number | null;
  median_time_to_hit_ms: number | null;
  time_on_target: number | null;
  duration_s: number;
  timing_suspect: boolean;
  partial: boolean;
}
export interface CoachReport {
  at_ms: number;
  scope: string;
  sessions: number;
  source: string;
  locale?: string;
  note: string | null;
  reply: {
    summary: string;
    strengths: string[];
    weaknesses: { title: string; detail: string; fix: string }[];
    next_steps: string[];
  };
}
export interface CoachConfig {
  player_settings: boolean;
  skills: boolean;
  findings: boolean;
  recent_sessions: boolean;
  session_limit: number;
  directions: boolean;
  plan: boolean;
  block_result: boolean;
  extra_instructions: string;
}

const inTauri = "__TAURI_INTERNALS__" in window;

export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(command, args) : mockCall<T>(command, args);
}
