export type Lang = "ar" | "en";

export const skillNames: Record<string, [string, string]> = {
  S01Reaction: ["رد الفعل", "Reaction"],
  S02FlickSpeed: ["سرعة الفليك", "Flick speed"],
  S03FlickPrecision: ["دقة الفليك", "Flick precision"],
  S04GainControl: ["التحكم بالمدى", "Distance control"],
  S05MicroCorrection: ["التصحيح الدقيق", "Micro correction"],
  S06TrackSmooth: ["التتبع السلس", "Smooth tracking"],
  S07TrackReactive: ["التتبع التفاعلي", "Reactive tracking"],
  S08TargetSwitch: ["التبديل بين الأهداف", "Target switching"],
  S09ClickTiming: ["توقيت النقر", "Click timing"],
  S10HeadLevel: ["مستوى الرأس", "Head level"],
  S11DirectionalBalance: ["توازن الاتجاهات", "Directional balance"],
  S13Consistency: ["الثبات", "Consistency"],
  S12CounterStrafe: ["الإيقاف قبل الإطلاق", "Counter-strafe"],
};

export const scenarioNames: Record<string, [string, string]> = {
  warmup: ["إحماء", "Warm-up"],
  flick_static: ["الفليك الثابت", "Static flick"],
  micro_precision: ["الدقة الصغيرة", "Micro precision"],
  head_level_peeks: ["الظهور على مستوى الرأس", "Head-level peeks"],
  smooth_track: ["التتبع السلس", "Smooth tracking"],
  strafe_track: ["تتبع strafe", "Strafe tracking"],
  multi_switch: ["التبديل بين الأهداف", "Target switching"],
  counter_strafe: ["الإيقاف قبل الإطلاق", "Counter-strafe"],
};

export const drillNames: Record<string, [string, string]> = {
  warmup: ["إحماء", "Warm-up"],
  deceleration_ladder: ["سلّم التخفيف", "Deceleration ladder"],
  commit_flick: ["فليك كامل", "Commit flick"],
  consistency_grid: ["شبكة الثبات", "Consistency grid"],
  micro_hold: ["تثبيت دقيق", "Micro hold"],
  confirm_click: ["نقر بعد الوصول", "Confirm click"],
  timed_kill: ["قتل موقوت", "Timed kill"],
  head_level_hold: ["ثبات مستوى الرأس", "Head level hold"],
  peek_line: ["خط الظهور", "Peek line"],
  directional_flick: ["فليك الاتجاهات", "Directional flick"],
  strafe_track_reactive: ["تتبع strafe", "Reactive strafe"],
  smooth_track: ["تتبع سلس", "Smooth track"],
  reflex_dot: ["نقطة رد الفعل", "Reflex dot"],
  precision_first: ["الدقة أولاً", "Precision first"],
  speed_push: ["دفع السرعة", "Speed push"],
  cs_drill: ["تمرين الإيقاف", "Counter-strafe drill"],
  custom: ["جلسة مخصصة", "Custom session"],
};

export interface DrillInfo {
  group: "flick" | "precision" | "timing" | "tracking" | "placement";
  purpose: [string, string];
}

export const drillInfo: Record<string, DrillInfo> = {
  warmup: { group: "flick", purpose: ["أهداف كبيرة حتى تسخن يدك.", "Large targets to get your hand moving."] },
  deceleration_ladder: { group: "flick", purpose: ["توقف الحركة الأولى على الهدف بدل ما تتجاوزه.", "Stop the first movement on the target instead of past it."] },
  commit_flick: { group: "flick", purpose: ["توصل بحركة وحدة كاملة بدون زحف.", "Arrive in one full movement, no crawling."] },
  consistency_grid: { group: "flick", purpose: ["نفس المسافة بكل اتجاه حتى تثبت حركتك.", "Same distance in every direction to steady your movement."] },
  directional_flick: { group: "flick", purpose: ["يكشف ويقوّي الاتجاه الضعيف.", "Finds and strengthens your weak direction."] },
  speed_push: { group: "flick", purpose: ["ترفع سرعتك فوق المريح.", "Pushes your speed past comfortable."] },
  micro_hold: { group: "precision", purpose: ["أهداف صغيرة قريبة، تثبت قبل النقر.", "Small close targets; settle before clicking."] },
  precision_first: { group: "precision", purpose: ["الدقة قبل السرعة.", "Accuracy before speed."] },
  confirm_click: { group: "timing", purpose: ["توصل أول وبعدين تنقر.", "Arrive first, then click."] },
  timed_kill: { group: "timing", purpose: ["تنهي الهدف قبل ما ينتهي الوقت.", "Finish the target before time runs out."] },
  reflex_dot: { group: "timing", purpose: ["تبدي الحركة أول ما تشوف الهدف.", "Start moving the moment you see it."] },
  smooth_track: { group: "tracking", purpose: ["تبقى على هدف يتحرك بسرعة ثابتة.", "Stay on a target moving at steady speed."] },
  strafe_track_reactive: { group: "tracking", purpose: ["ترد على تغيير الاتجاه مثل مبارزات الـ strafe.", "React to reversals, as in strafe duels."] },
  cs_drill: { group: "timing", purpose: ["تتوقف قبل النقر، مثل اللعبة.", "Stop before you click, as in the game."] },
  head_level_hold: { group: "placement", purpose: ["يبقى مؤشرك على مستوى الرأس.", "Keep your crosshair at head level."] },
  peek_line: { group: "placement", purpose: ["تنقر الهدف وهو يطلع من الجدار.", "Click targets as they emerge from walls."] },
};

export const groupNames: Record<DrillInfo["group"], [string, string]> = {
  flick: ["الفليك", "Flicks"],
  precision: ["الدقة", "Precision"],
  timing: ["التوقيت", "Timing"],
  tracking: ["التتبع", "Tracking"],
  placement: ["مكان المؤشر", "Crosshair placement"],
};

/** Plain-language names for the numbers the diagnosis cites. */
export const evidenceNames: Record<string, { name: [string, string]; pct?: boolean; unit?: string }> = {
  "S01.median_ms": { name: ["وسيط رد الفعل", "Median reaction"], unit: "ms" },
  "S02.tp_bits_s": { name: ["سرعة الفليك", "Flick throughput"], unit: "bits/s" },
  "S03.we_deg": { name: ["انتشار نقاط النهاية", "Endpoint spread"], unit: "°" },
  "S04.median": { name: ["نسبة الحركة الأولى للمسافة", "First movement vs distance"] },
  "S04.iqr": { name: ["تذبذب المدى", "Distance variation"] },
  gain_vs_distance_slope: { name: ["زيادة التجاوز مع المسافة", "Overshoot growth with distance"] },
  "S05.median_ms": { name: ["وقت التصحيح", "Correction time"], unit: "ms" },
  "S05.mean_submovements": { name: ["معدل التصحيحات", "Average corrections"] },
  "S07.median_ms": { name: ["تأخر رد الفعل على تغيير الاتجاه", "Reversal lag"], unit: "ms" },
  "S09.premature_rate": { name: ["نقرات قبل الوصول", "Clicks before arriving"], pct: true },
  "S09.median_dwell_ms": { name: ["تردد قبل النقر", "Hesitation before clicking"], unit: "ms" },
  "S12.moving_shot_rate": { name: ["طلقات أثناء الحركة", "Shots while moving"], pct: true },
  "S10.bias_deg": { name: ["انحراف عن مستوى الرأس", "Offset from head level"], unit: "°" },
  "S11.octant": { name: ["الاتجاه الأضعف (رقم)", "Weakest direction (index)"] },
  "S11.z": { name: ["شدة الضعف", "Weakness strength"] },
};
