# 🔬 محرك التشخيص

> **المهمة:** تحويل تليمتري خام (أحداث ماوس + أهداف + نقرات) إلى **ملف مهارات** بمجالات ثقة، ثم إلى **تشخيص سببي** قابل للتفسير.

```
.aft  ──▶ 1. Segmentation ──▶ 2. Engagement Metrics ──▶ 3. Skill Scores ──▶ 4. Diagnosis
         (تقسيم لمواجهات       (مقاييس لكل مواجهة)       (13 مهارة + CI)       (أسباب + أدلة)
          وحركات فرعية)
```

---

## 1. نموذج المهارات (Skill Model v1)

| ID | المهارة | ماذا تقيس | المقياس الأساسي | أهميتها في VALORANT |
|----|--------|----------|----------------|:---:|
| `S01_REACTION` | زمن رد الفعل البصري | من ظهور الهدف حتى بدء الحركة | median onset latency (ms) | عالية |
| `S02_FLICK_SPEED` | سرعة الفليك | معدل نقل المعلومات الحركية | Fitts throughput (bits/s) | عالية |
| `S03_FLICK_PRECISION` | دقة نهاية الفليك | انتشار نقاط النهاية | effective width We (°) | عالية جداً |
| `S04_GAIN_CONTROL` | التحكم بالمدى | overshoot/undershoot في الحركة الأولى | primary gain ratio (median, IQR) | عالية |
| `S05_MICRO_CORRECTION` | التصحيح الدقيق | سرعة وكفاءة التصحيح بعد الحركة الأولى | correction time + submovement count | عالية جداً |
| `S06_TRACK_SMOOTH` | التتبّع السلس | متابعة هدف بسرعة ثابتة | time-on-target %، RMS error (°) | متوسطة |
| `S07_TRACK_REACTIVE` | التتبّع التفاعلي | الاستجابة لتغيّر اتجاه الهدف (strafe) | reversal lag (ms) | عالية (مبارزات strafe) |
| `S08_TARGET_SWITCH` | التبديل بين الأهداف | الانتقال بين عدة أهداف | time-between-kills | متوسطة |
| `S09_CLICK_TIMING` | توقيت النقر | النقر قبل الاستقرار أو بعد التردد | premature-click rate، dwell time | عالية جداً |
| `S10_HEAD_LEVEL` | الثبات العمودي | انحراف الـ pitch عن مستوى الرأس | vertical error SD (°) | عالية جداً |
| `S11_DIRECTIONAL_BALANCE` | التوازن الاتجاهي | الفرق بين الاتجاهات | max octant deficit (z) | متوسطة |
| `S12_COUNTER_STRAFE` | الإيقاف قبل الإطلاق | الإطلاق أثناء الحركة | moving-shot rate، stop-to-shot (ms) | عالية جداً |
| `S13_CONSISTENCY` | الثبات والتحمّل | التذبذب والتراجع عبر الجلسة | CV of TTK، fatigue slope | متوسطة |

كل مهارة تُخزَّن كالتالي:
```rust
pub struct SkillEstimate {
    pub skill: SkillId,
    pub raw: f64,              // بالوحدة الأصلية (ms, °, bits/s…)
    pub score: f64,            // 0–100 بعد التطبيع على المعايير (percentile-based)
    pub ci95: (f64, f64),      // مجال الثقة للـ score
    pub n_evidence: u32,       // عدد المواجهات التي بُني عليها
    pub reliability: Reliability, // Low / Medium / High حسب n و SEM
}
```

---

## 2. التقسيم (Segmentation)

### 2.1 تحويل الإدخال إلى سرعة زاوية
- الإدخال: أحداث `(t_us, dx, dy)` ← تراكم إلى زوايا `(yaw, pitch)` عبر `aim-core`.
- إعادة أخذ عيّنات بشكل موحّد بتردد **1000Hz** (interpolation خطي)، ثم تنعيم بفلتر **Savitzky–Golay** (نافذة 15ms، درجة 2) للحصول على السرعة الزاوية `ω(t)` بـ °/s.
- السبب: الأحداث الخام غير منتظمة زمنياً (polling rate يتراوح بين 125 و8000Hz). وكل المقاييس اللاحقة تحتاج شبكة زمنية ثابتة.

### 2.2 المواجهة (Engagement)
تبدأ بظهور الهدف وتنتهي بالإصابة أو بانتهاء الوقت. داخلها:

```
 ω (°/s)
   │        ╭──╮  ← peak velocity
   │       ╱    ╲
   │      ╱      ╲      ╭╮   ╭╮   ← submovements (corrections)
   │     ╱        ╲____╱  ╲_╱  ╲___ click
   └────┴──────────┴──────────────┴──▶ t
   spawn onset   primary end    stable  shot
   │◀ S01 ▶│◀ ballistic ▶│◀── S05 ──▶│◀S09▶│
```

| الحدث | التعريف الدقيق |
|------|---------------|
| **Onset** | أول لحظة تتجاوز فيها `ω` قيمة **30°/s** وتبقى فوقها **20ms** متواصلة، وباتجاه يقلّ فيه البعد عن الهدف |
| **Peak** | أعلى `ω` بين onset ونهاية الحركة الأولى |
| **Primary end** | أول حدّ أدنى محلي للسرعة بعد الـ peak يكون فيه `ω < 0.2 × peak`، **أو** انعكاس في إشارة السرعة على محور الحركة |
| **Submovement** | كل نبضة سرعة لاحقة ذروتها > 15°/s مفصولة بحدود دنيا |
| **Stable** | أول لحظة بعدها يبقى المؤشر داخل الهدف وسرعته < 10°/s لمدة 30ms |
| **Shot** | لحظة النقر (من التليمتري) |

> كل العتبات في `content/analysis/thresholds.v1.ron` وليست في الكود. تُعاير في الـ Beta.

---

## 3. المعادلات

### 3.1 S01 — زمن رد الفعل
```
RT_i = t_onset − t_spawn_displayed
t_spawn_displayed = أول frame present بعد spawn (من frame timestamps)
S01.raw = median(RT_i)   // median وليس mean لمقاومة القيم الشاذة
```
نستبعد `RT < 100ms` (استباق/تخمين) ونعدّها كمؤشر منفصل اسمه `anticipation_rate`.

### 3.2 S02 — Fitts Throughput (المعيار العلمي لسرعة/دقة التصويب)
```
D   = المسافة الزاوية من نقطة البداية إلى مركز الهدف (°)
We  = 4.133 × SD(endpoint error projected on movement axis)   // effective width
IDe = log2(D / We + 1)                                         // bits
MT  = t_shot − t_onset                                         // seconds
TP  = mean over conditions( IDe / mean(MT) )                   // bits/s
```
- يُحسب لكل **شرط** (مسافة × حجم) ثم نأخذ المتوسط (ISO 9241-9).
- **لماذا هذا مهم:** TP يدمج السرعة والدقة في رقم واحد. اللاعب الذي "يسرّع" بلا دقة لن يرفعه، لأن We يكبر فيقلّ TP.

### 3.3 S03 — دقة نهاية الفليك
```
S03.raw = We (°) على اختبار الفليك الثابت
+ مقياس ثانوي: centroid bias = متوسط متجه الخطأ (هل يميل دائماً لجهة معينة؟)
```

### 3.4 S04 — Gain Ratio (التحكم بالمدى) ⭐
```
A_primary = المسافة الزاوية المقطوعة بين onset و primary end (مسقطة على اتجاه الهدف)
G_i       = A_primary / D
S04.raw   = median(G_i),  spread = IQR(G_i)

G ≈ 0.92–1.00  → مثالي (المهارة الطبيعية: الحركة الأولى تقف قليلاً قبل الهدف)
G > 1.05       → OVERSHOOT نظامي
G < 0.85       → UNDERSHOOT نظامي
IQR كبير       → عدم ثبات (مشكلة تحكم، وليست مشكلة حساسية)
```
**هذا أدقّ بكثير من المنطق الحالي** (`script.js` يعدّ أي تغيير اتجاه overshoot). Gain ratio يفرّق بين:
- overshoot **منتظم** يزداد مع المسافة ← قد يرتبط بالحساسية/الثقة في الحركة (انظر القاعدة D2).
- overshoot **عشوائي** ← مشكلة تحكم حركي، **ولا علاقة له بالحساسية**.

### 3.5 S05 — التصحيح الدقيق
```
T_corr  = t_stable − t_primary_end
N_sub   = عدد الـ submovements
S05.raw = median(T_corr), وثانوياً mean(N_sub)
```
تحليل إضافي: **oscillation index** = عدد انعكاسات الاتجاه داخل 2×r من المركز ÷ زمن التصحيح ← "رجفة" عند الهدف.

### 3.6 S06/S07 — التتبّع
```
e(t)       = angular_distance(crosshair(t), target(t))
TOT        = Σ[e(t) ≤ r_target] / N                     // time on target
RMS        = sqrt(mean(e(t)²))
lag        = argmax_τ xcorr(ω_target(t), ω_crosshair(t+τ))   // ms
reversal_RT= لكل انعكاس في اتجاه الهدف: الزمن حتى تنعكس سرعة المؤشر
S06.raw = TOT على تتبّع بسرعة ثابتة
S07.raw = median(reversal_RT) على تتبّع strafe عشوائي
```

### 3.7 S09 — توقيت النقر ⭐
```
لكل نقرة:
  ω_click = السرعة الزاوية لحظة النقر
  inside  = هل المؤشر داخل الهدف؟
  dwell   = t_shot − t_stable  (إن وُجد stable)

premature = !inside && e_click < 1.5 r && ω_click > 60°/s   // نقر قبل الوصول/الاستقرار
hesitant  = inside && dwell > 150ms                         // تردّد
S09.raw   = premature_rate, وثانوياً median(dwell)
```

### 3.8 S10 — مستوى الرأس
في سيناريوهات الأهداف على ارتفاع الرأس (head-level):
```
pitch_error_i = pitch_crosshair(t_spawn) − pitch_target_head
S10.raw = SD(pitch_error) + |mean(pitch_error)|   // تشتت + انحياز
```

### 3.9 S11 — التوازن الاتجاهي
- نقسّم المواجهات إلى 8 اتجاهات (octants) حسب زاوية الحركة المطلوبة.
- لكل اتجاه: TP و G و T_corr.
- `deficit_k = (metric_k − metric_all) / SE_k` ← z-score.
- **لا نبلّغ عن اتجاه إلا إذا كان `n_k ≥ 12` و`|z| ≥ 2`** (تجنّب الإنذارات الكاذبة، وهي مشكلة موجودة في `PatternDetector` الحالي الذي يكتفي بـ 5 عيّنات).

### 3.10 S12 — Counter-strafe
```
v_player(t_shot)  من محاكاة الحركة
moving_shot       = v > v_accurate_threshold   // قيمة من ملف بيانات خاص بكل سلاح
stop_to_shot      = t_shot − t_velocity_below_threshold
S12.raw = moving_shot_rate، وثانوياً median(stop_to_shot)
```

### 3.11 S13 — الثبات والتحمّل
```
CV          = SD(TTK) / mean(TTK)
fatigue     = slope( انحدار خطي لـ TTK على رقم المواجهة ) / mean(TTK)   // % لكل دقيقة
warmup_gain = الأداء في آخر 30% − أول 30% (في بداية اليوم)
```

---

## 4. من الـ raw إلى الـ score (التطبيع)

### 4.1 المعايير (Norms)
- جدول percentiles لكل مهارة، **حسب الرتبة المعلنة** وعامّاً.
- **قبل وجود بيانات:** معايير مبدئية من Beta داخلية + قيم من الأدبيات، مع وسم `norms: provisional` ظاهر للمستخدم.
- **بعد الـ Beta:** تُحسب من البيانات الحقيقية وتُحدَّث شهرياً مع رقم إصدار (`norms.v3`).

```
score = 100 × percentile(raw | reference population)
// للمقاييس التي "الأقل أفضل" (RT, We, T_corr) نعكس الاتجاه
```

### 4.2 مجال الثقة لكل مهارة
- **Bootstrap** على مستوى المواجهات (1000 إعادة سحب) ← CI95 للـ raw ← تحويله إلى CI للـ score.
- `reliability = High` إذا كان عرض CI < 10 نقاط، و`Medium` إذا كان < 20، وإلا `Low`.
- **الواجهة تعرض CI دائماً** (شريط بهامش)، وتخفي التشخيص المبني على مهارة `Low`.

---

## 5. بطارية التقييم (Assessment Battery v1)

**المدة الإجمالية: ~9 دقائق** (اختبار حقيقي + راحة قصيرة بين الأقسام).

| # | الاختبار | المدة | المهارات المقاسة | التصميم |
|---|---------|------|-----------------|---------|
| A0 | Warm-up | 60s | — (لا يُحتسب) | أهداف كبيرة؛ يقلّل أثر "البرود" |
| A1 | Static Flick Grid | 75s | S01, S02, S03, S04, S05, S09, S11 | تصميم Fitts: 3 مسافات (8°, 20°, 40°) × 2 حجمين × 8 اتجاهات، **متوازن** |
| A2 | Micro Precision | 60s | S03, S05, S09 | أهداف صغيرة (0.4–0.7°) قريبة (2–6°) |
| A3 | Head-Level Peeks | 60s | S01, S10, S09 | أهداف تظهر من حواف "جدران" على ارتفاع رأس ثابت مع ضجيج |
| A4 | Smooth Track | 45s | S06 | هدف بسرعة زاوية ثابتة وتغيّر بطيء |
| A5 | Strafe Track | 45s | S07 | هدف يغيّر اتجاهه في أوقات عشوائية (A-D strafe) |
| A6 | Multi-Target Switch | 45s | S08, S13 | 3 أهداف في الوقت نفسه |
| A7 | Counter-Strafe Duel | 60s | S12, S09 | حركة WASD + أهداف؛ يجب الإيقاف قبل الإطلاق |
| — | S13 | — | — | يُستخرج من كامل الجلسة |

### 5.1 النماذج المتكافئة (Parallel Forms) — أساس عدالة إعادة الاختبار
- كل اختبار له **5 نماذج** (forms)، وكل نموذج = seed ثابت.
- النماذج **متوازنة بالتصميم:** نفس توزيع المسافات والأحجام والاتجاهات، والاختلاف فقط في الترتيب والمواقع الدقيقة. فلا يحفظ اللاعب المواقع، وتبقى الصعوبة متساوية.
- **تحقّق إحصائي:** في الـ Beta نقيس أن متوسط كل نموذج لا يختلف عن غيره (ANOVA، فرق < 0.2 SD). النموذج المنحرف يُستبعد.
- إعادة الاختبار تستخدم **نموذجاً لم يُستخدم في آخر اختبارين**.

### 5.2 ضبط الظروف
قبل كل تقييم نسجّل: DPI، الحساسية، Hz الشاشة، polling rate المقدَّر (من فترات الأحداث)، الوقت من اليوم، و"هل لعبت VALORANT في آخر ساعة؟" (سؤال سريع).
- **إذا تغيّرت الحساسية أو الـ DPI** بين اختبارين: المقارنة تُوسم بـ ⚠️ "ظروف مختلفة"، ولا تُستخدم في تقييم الخطة.
- إذا كانت `frame p99 > 2× p50`: نعرض تحذير أداء النظام، لأن الجلسة قد لا تمثل اللاعب.

---

## 6. التشخيص (Diagnosis Rules v1)

كل قاعدة = **شرط على المهارات/الأدلة** ← **Finding** يحتوي: السبب، الأدلة (أرقام من بيانات اللاعب)، الثقة، والتمارين المقترحة.

```ron
// content/diagnosis/rules.v1.ron
Rule(
  id: "D2_SYSTEMATIC_OVERSHOOT_SCALES_WITH_DISTANCE",
  when: All([
    Skill(S04_GAIN_CONTROL, median_gt: 1.05),
    Evidence(gain_vs_distance_slope, gt: 0.0, p_lt: 0.05),
    Skill(S04_GAIN_CONTROL, iqr_lt: 0.20),
  ]),
  finding: (
    cause: "OVERSHOOT_SYSTEMATIC",
    severity_from: "S04.median",
    explain_key: "diag.d2",          // نص مترجم (ar/en)
    evidence: ["S04.median", "gain_vs_distance_slope", "S04.iqr"],
    drills: ["deceleration_ladder", "flick_stop_short"],
    side_note_key: "diag.d2.sens_hint", // اقتراح حساسية: ملاحظة اختيارية وليست أمراً
  ),
)
```

### جدول القواعد الأساسية

| ID | الشرط (مختصر) | التشخيص | لماذا هذا مهم (للاعب) | التمارين |
|----|--------------|---------|------------------|----------|
| D1 | S01 منخفض جداً وباقي المهارات جيدة | **بطء البدء** | تخسر المواجهة قبل أن تبدأ الحركة | reaction_peek, reflex_dot |
| D2 | G > 1.05 ويزداد مع المسافة وIQR صغير | **Overshoot منتظم** | يدك تبالغ في الحركة الأولى بشكل متوقّع، وهذا قابل للتصحيح بسرعة | deceleration_ladder, flick_stop_short |
| D3 | IQR(G) > 0.30 | **عدم ثبات المدى** | المشكلة ليست الحساسية؛ حركتك غير متسقة | consistency_grid (ثابت المسافة) |
| D4 | G < 0.85 و N_sub مرتفع | **Undershoot + تصحيحات كثيرة** | تتوقّف مبكراً ثم "تزحف" للهدف | commit_flick (أهداف كبيرة، وقت محدود) |
| D5 | S05 منخفض و oscillation مرتفع | **رجفة عند الهدف** | المؤشر يهتز حول الرأس | micro_hold, slow_precision |
| D6 | S09 premature_rate > 25% | **نقر متسرّع** | تطلق قبل الوصول، فتضيع الرصاصة الأولى | confirm_click (أهداف لا تُحتسب إلا بعد 20ms ثبات) |
| D7 | S09 dwell > 150ms ودقة عالية | **تردّد** | دقيق لكن بطيء؛ في اللعبة سيسبقك الخصم | timed_kill (نافذة زمنية تتقلّص) |
| D8 | S10 انحياز عمودي > 1° | **Crosshair placement منخفض/مرتفع** | تحتاج تصحيحاً عمودياً في كل مواجهة | head_level_hold, peek_line |
| D9 | S11: اتجاه z ≤ −2 | **ضعف اتجاه معيّن** (مثلاً: يسار-أسفل) | | directional_flick(dir=X) |
| D10 | S07 reversal_RT مرتفع وS06 جيد | **تتبّع جيد لكن تفاعل ضعيف** | تخسر مبارزات الـ strafe | strafe_track_reactive |
| D11 | S12 moving_shot > 20% | **الإطلاق أثناء الحركة** | رصاصاتك غير دقيقة مهما كان تصويبك | cs_drill |
| D12 | S13 fatigue > 3%/min | **تعب/فقدان تركيز** | | جلسات أقصر + راحة |
| D13 | S02 جيد وS03 ضعيف | **مقايضة نحو السرعة** | تضحي بالدقة لتكون سريعاً | precision_first |
| D14 | S03 جيد وS02 ضعيف | **مقايضة نحو الحذر** | | speed_push |

### 6.1 ترتيب الأولويات (Priority)
```
priority(finding) = severity × impact_weight(skill) × trainability(skill) × reliability_factor
```
- `impact_weight`: أهمية المهارة لـ VALORANT (من الجدول في القسم 1؛ قيمة قابلة للمعايرة لاحقاً بالارتباط مع إحصائيات Riot).
- `trainability`: ما مدى سرعة تحسّن هذه المهارة (تُقدَّر من البيانات؛ قيمة مبدئية: D6/D11/D8 سريعة، وD1 بطيئة).
- `reliability_factor`: High=1، Medium=0.7، Low=0 (لا يُعرض).

نعرض **أعلى 3 فقط** ونركّز الخطة على **أعلى 2**. التركيز على أشياء كثيرة في الوقت نفسه يقلّل التحسّن.

### 6.2 قاعدة الحساسية (بحذر)
لا نقول أبداً "حساسيتك خاطئة" بناءً على جلسة واحدة. التوصية بتغيير الحساسية تظهر **فقط** إذا:
1. D2 (overshoot منتظم) ثابت في **3 تقييمات متتالية**، **و**
2. لم يتحسّن بعد **كتلتي تدريب**، **و**
3. `cm/360` للاعب خارج المجال الشائع (يُحسب من المعايير).

عندها نقترح **تجربة A/B**: تقييم مصغّر بحساستين (الحالية و−10%) في يومين مختلفين، ونقارن TP وG. **القرار مبني على البيانات وليس على قاعدة ثابتة.**

---

## 7. نقل الكود الحالي

| الكود الحالي | يتحوّل إلى | التغيير |
|-------------|-----------|--------|
| `TargetEngagement.update` (`analyzer.js:98`) | `aim-analysis::segment` | زوايا بدل بكسل؛ 1000Hz بدل كل frame؛ تعريفات onset/primary-end الدقيقة |
| `_computeCorrectionSplit`، `_computeOscillation` | `metrics::correction`، `metrics::oscillation` | عتبات من ملف بيانات |
| `DeepFlickAnalyzer` | `metrics::fitts` + `metrics::gain` | Fitts TP + gain ratio بدل عدّ تغييرات الاتجاه |
| `PatternDetector._detectDirectionBias` | `skills::directional` | حدّ أدنى للعيّنات + z-score |
| `PatternDetector._detectTemporalPatterns` | `skills::consistency` | انحدار خطي بدل مقارنة أثلاث |
| `SessionComparator` | `aim-coach::evaluation` | MDC + DiD (انظر `04_TRAINING_LOOP.md`) |
| `FlickAnalyzer`/`PlayerDiagnosis` في `script.js` | ❌ يُحذف | مكرّر |
| `AimCoach` (rule-based + JSON export) | `RuleDiagnoser` + `aim-llm` | القواعد في ملفات بيانات؛ والشرح عبر LLM API (سحابي عبر السيرفر، أو محلي) |

### خطة التحقق من صحة النقل
1. نُسجّل 20 جلسة في الويب الحالي بصيغة JSON (تصدير موجود).
2. نحوّلها إلى `.aft` (أداة تحويل، مع ملاحظة أن البكسل يُحوَّل إلى زوايا تقريبياً).
3. نقارن مخرجات Rust بمخرجات JS على المقاييس المشتركة. أي اختلاف يجب أن يكون **مقصوداً ومُوثّقاً**.
