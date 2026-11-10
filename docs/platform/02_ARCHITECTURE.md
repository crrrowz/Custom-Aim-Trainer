# 🏛️ المعمارية التقنية

## 1. المبادئ

1. **Hexagonal / Ports & Adapters:** منطق المجال (القياس، التحليل، التدريب) في مكتبات Rust نقية **بلا I/O**. كل ما هو خارجي (رسم، ملفات، شبكة، قاعدة بيانات، LLM، دفع) يمرّ عبر `trait`.
2. **Data-driven:** السيناريوهات والتمارين والعتبات ملفات بيانات (RON/JSON) مُرقّمة الإصدار، وليست كوداً. هذا يسمح بتعديلها دون إعادة بناء، وبتحديثها من السيرفر لاحقاً.
3. **تخزين الخام دائماً:** نحفظ التليمتري الخام مضغوطاً. أي خوارزمية تحليل جديدة يمكن إعادة تشغيلها على كل التاريخ (reprocessing).
4. **الحتمية (Determinism):** نفس الـ seed يعطي نفس السيناريو. ونفس التليمتري يعطي نفس التحليل بالضبط. هذا شرط لعدالة إعادة الاختبار ولإمكانية اختبار الكود.
5. **الوحدات الزاوية:** كل القياسات بالدرجات (°) وليس بالبكسل، لأن الدرجات هي ما يطابق اللعبة ويبقى ثابتاً بين الشاشات والدقّات.

---

## 2. الصورة الكبيرة

```
┌────────────────────────────── جهاز اللاعب ───────────────────────────────┐
│                                                                          │
│  ┌──────────── aimforge-desktop (Tauri 2) ────────────┐                  │
│  │  UI: React + TypeScript (Vite)                     │                  │
│  │   الداشبورد · التشخيص · الخطة · التقدّم · الحساب    │                  │
│  │            ▲ Tauri commands / events               │                  │
│  │  Rust backend:                                     │   stdin/stdout   │
│  │   aim-analysis · aim-coach · aim-store · aim-sync ─┼── JSON lines ──┐ │
│  └────────────────────────────────────────────────────┘                │ │
│                                                                        ▼ │
│  ┌──────────── aimforge-trainer (Bevy, sidecar) ────────────────────────┐│
│  │  Raw Input (winit DeviceEvent) · كاميرا 3D مطابقة لـ VALORANT          ││
│  │  aim-scenarios (seeded) · aim-telemetry (recorder) → session.aft     ││
│  └──────────────────────────────────────────────────────────────────────┘│
└───────────────────────────────────┬──────────────────────────────────────┘
                                    │ HTTPS (aim-sync)
                     ┌──────────────▼──────────────┐
                     │ المرحلة 1: Supabase           │
                     │  Auth · Postgres · Storage   │
                     │  Edge Functions (webhooks)   │
                     │ المرحلة 2+: خدمة Axum (Rust)  │
                     │  تستخدم نفس crates التحليل   │
                     └──────────────┬──────────────┘
                 ┌──────────────────┼──────────────────┐
             Paddle (MoR)   LLM API (Claude/أي مزوّد)  Riot API (رسمي)
```

### لماذا عمليتان (Tauri + Bevy) وليس واحدة؟
| البديل | الإيجابيات | السلبيات |
|-------|-----------|---------|
| **Tauri + Bevy sidecar (المختار)** | واجهة ويب جميلة وسريعة التطوير + محاكي native بلا قيود الـ WebView على الإدخال والـ vsync؛ كل جزء قابل للاستبدال وحده | عمليتان وIPC بسيط |
| Bevy فقط مع `bevy_egui` | تقنية واحدة | الواجهات الغنية (داشبورد، رسوم بيانية، دفع، حساب) أصعب بكثير |
| Tauri فقط مع Canvas/WebGL في WebView2 | الأسهل، ويعيد استخدام JS الحالي | الإدخال يمرّ عبر حلقة أحداث المتصفح، والتحكم بالـ vsync ومعدل الإطارات محدود ← دقة قياس أقل |

> **الاستراتيجية الانتقالية (اختيارية لتسريع المرحلة 1):** يمكن تشغيل محاكي الويب الحالي داخل Tauri مؤقتاً، بشرط أن يرسل التليمتري بنفس صيغة `aim-telemetry`، ثم يُستبدل بـ Bevy دون أن يتأثر أي شيء آخر. هذا هو أثر الـ Port.

---

## 3. هيكل المستودع

```
aimforge/
├── Cargo.toml                    # [workspace]
├── crates/
│   ├── aim-core/                 # أنواع المجال والوحدات والرياضيات — no I/O, no_std-friendly
│   │   └── src/{units.rs, sensitivity.rs, camera.rs, geometry.rs, ids.rs}
│   ├── aim-telemetry/            # صيغة .aft (AimForge Telemetry) — كتابة وقراءة وترقية الإصدارات
│   ├── aim-scenarios/            # تعريفات السيناريوهات (RON) + مولّد مُبذَّر (ChaCha8Rng)
│   ├── aim-analysis/             # Engagement segmentation → metrics → skill scores → diagnosis
│   ├── aim-coach/                # prescription, plan, staircase, retest, evaluation
│   ├── aim-store/                # Port: SessionRepository + adapter SQLite محلي (rusqlite)
│   ├── aim-sync/                 # Port: SyncClient + adapter Supabase (reqwest)
│   ├── aim-llm/                  # Port: LlmProvider + adapters (Anthropic, OpenAI-compatible: سحابي أو محلي مثل Ollama)
│   └── aim-wasm/                 # (لاحقاً) bindings لتشغيل التحليل في المتصفح
├── apps/
│   ├── trainer/                  # Bevy binary (sidecar)
│   └── desktop/
│       ├── src-tauri/            # Tauri 2 Rust backend
│       └── ui/                   # React + TS + Vite
├── services/
│   ├── supabase/                 # migrations SQL + edge functions (المرحلة 1)
│   └── api/                      # Axum (المرحلة 2+)
├── content/
│   ├── scenarios/*.ron           # بيانات السيناريوهات
│   ├── drills/*.ron              # قوالب التمارين
│   └── assessment/v1/*.ron       # بطارية التقييم ونماذجها المتكافئة
├── tools/
│   ├── replay-viewer/            # أداة لعرض جلسة مسجّلة (تصحيح أخطاء التحليل)
│   └── synth/                    # مولّد لاعبين اصطناعيين لاختبار التحليل
├── legacy-web/                   # الكود القديم (نُقل لاحقاً إلى أرشيف خاص، مو جزء من المستودع)
└── docs/
```

### قواعد الاعتماد بين الـ crates (يُفرض في CI)
```
aim-core  ◀── aim-telemetry ◀── aim-analysis ◀── aim-coach
   ▲               ▲                                  
   └── aim-scenarios                                  
apps/* و aim-store/aim-sync/aim-llm هي فقط من يعتمد على I/O.
aim-analysis و aim-coach ممنوع أن تعتمد على bevy, tauri, reqwest, rusqlite.
```
(يُفرض عبر `cargo-deny` أو سكربت بسيط يفحص `cargo metadata`.)

---

## 4. `aim-core`: الرياضيات المطابقة لـ VALORANT

```rust
// crates/aim-core/src/sensitivity.rs

/// VALORANT: 0.07 degrees of yaw per mouse count at sensitivity 1.0.
pub const VALORANT_YAW_DEG_PER_COUNT: f64 = 0.07;
pub const CM_PER_INCH: f64 = 2.54;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseSetup {
    pub dpi: u32,
    pub in_game_sens: f64,
}

impl MouseSetup {
    pub fn deg_per_count(&self) -> f64 {
        VALORANT_YAW_DEG_PER_COUNT * self.in_game_sens
    }
    pub fn edpi(&self) -> f64 {
        self.dpi as f64 * self.in_game_sens
    }
    /// Physical mouse travel for a full 360° turn.
    pub fn cm_per_360(&self) -> f64 {
        let counts = 360.0 / self.deg_per_count();
        counts / self.dpi as f64 * CM_PER_INCH
    }
    /// Sensitivity giving the requested cm/360 at this DPI.
    pub fn sens_for_cm360(dpi: u32, cm360: f64) -> f64 {
        let counts = cm360 / CM_PER_INCH * dpi as f64;
        360.0 / counts / VALORANT_YAW_DEG_PER_COUNT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_values() {
        let s = MouseSetup { dpi: 800, in_game_sens: 0.35 };
        assert!((s.edpi() - 280.0).abs() < 1e-9);
        assert!((s.cm_per_360() - 46.65).abs() < 0.01);
        assert!((MouseSetup::sens_for_cm360(800, s.cm_per_360()) - 0.35).abs() < 1e-9);
    }
}
```

```rust
// crates/aim-core/src/camera.rs
/// VALORANT uses a fixed 103° horizontal FOV (reference: 16:9).
pub const VALORANT_HFOV_DEG: f64 = 103.0;

pub fn vfov_from_hfov(hfov_deg: f64, aspect: f64) -> f64 {
    let h = hfov_deg.to_radians() / 2.0;
    (2.0 * (h.tan() / aspect).atan()).to_degrees()   // 16:9 → ≈70.53°
}

/// Angular radius (deg) of a sphere of radius r at distance d (same units).
pub fn angular_radius_deg(r: f64, d: f64) -> f64 { (r / d).atan().to_degrees() }

#[derive(Debug, Clone, Copy, Default)]
pub struct ViewAngles { pub yaw: f64, pub pitch: f64 } // degrees

impl ViewAngles {
    pub fn apply_counts(&mut self, dx: i32, dy: i32, deg_per_count: f64) {
        self.yaw = (self.yaw + dx as f64 * deg_per_count).rem_euclid(360.0);
        self.pitch = (self.pitch - dy as f64 * deg_per_count).clamp(-89.0, 89.0);
    }
}

/// Great-circle angle between two view directions (deg). Used for all distance metrics.
pub fn angular_distance(a: ViewAngles, b: ViewAngles) -> f64 { /* haversine on (yaw,pitch) */ todo!() }
```

> ⚠️ **للتحقق في المرحلة 1:** حدود الـ pitch الدقيقة في VALORANT، وسلوك FOV في نسب العرض غير 16:9. نتحقق بقياس عملي داخل اللعبة (عدد counts لدورة 360 كاملة) ونسجّل النتيجة في اختبار.

---

## 5. المحاكي (`apps/trainer`)

| المتطلب | التنفيذ |
|--------|---------|
| Raw Input | `DeviceEvent::MouseMotion` من winit (يستخدم `WM_INPUT` على Windows) — نسجّل **كل حدث** مع `Instant` وليس كل frame |
| بلا تسارع ماوس | Raw Input يتجاوز إعدادات تسارع Windows تلقائياً |
| معدل إطارات | `PresentMode::Immediate` اختياري، أو `AutoNoVsync`؛ هدف 240+ FPS؛ نسجّل frame times لاكتشاف الجلسات المتقطعة |
| الكاميرا | `ViewAngles` من `aim-core` و`Projection` بـ vFOV المحسوب |
| تزامن العرض والقياس | التحليل يستخدم **زوايا الكاميرا المحسوبة من الإدخال** وليس ما رُسم على الشاشة، مع تسجيل frame timestamps لتقدير زمن الاستجابة البصرية |
| الأهداف | كيانات Bevy مولّدة من `aim-scenarios` بـ seed مُمرّر من الـ desktop |
| الحركة (WASD) | فيزياء حركة مبسّطة لـ counter-strafe (المرحلة 3)؛ القيم في ملف بيانات قابل للمعايرة |
| الإخراج | ملف `session-<uuid>.aft` + رسالة `SessionFinished` عبر stdout |

### بروتوكول IPC (JSON lines)
```jsonc
// desktop → trainer (stdin)
{"type":"StartSession","session_id":"…","scenario":"assess.v1.flick_static","form":3,
 "seed":918273,"mouse":{"dpi":800,"sens":0.35},"display":{"hz":240},"duration_s":60}
{"type":"Abort"}

// trainer → desktop (stdout)
{"type":"Ready"}
{"type":"Progress","elapsed_s":31.2,"hits":41}
{"type":"SessionFinished","session_id":"…","telemetry_path":"…/session-….aft","frame_stats":{"p50_ms":4.1,"p99_ms":7.9}}
{"type":"Error","code":"GPU_INIT_FAILED","message":"…"}
```
البروتوكول مُعرَّف في crate صغير مشترك (`aim-ipc`) بأنواع `serde`، فيستحيل أن يختلف الطرفان.

---

## 6. الـ Ports (الواجهات)

```rust
// aim-store
pub trait SessionRepository {
    fn save_session(&self, meta: &SessionMeta, telemetry: &Path) -> Result<()>;
    fn save_analysis(&self, session: SessionId, a: &SessionAnalysis) -> Result<()>;
    fn skill_history(&self, player: PlayerId, skill: SkillId) -> Result<Vec<SkillObservation>>;
    fn active_plan(&self, player: PlayerId) -> Result<Option<TrainingPlan>>;
    fn save_plan(&self, plan: &TrainingPlan) -> Result<()>;
}

// aim-sync
#[async_trait]
pub trait SyncClient {
    async fn push(&self, batch: SyncBatch) -> Result<SyncAck>;
    async fn pull(&self, since: Cursor) -> Result<SyncBatch>;
    async fn upload_telemetry(&self, session: SessionId, bytes: Bytes) -> Result<()>;
    async fn entitlements(&self) -> Result<Entitlements>;   // Free / Pro / Team
    async fn norms(&self, version: NormsVersion) -> Result<NormTable>;
}

// aim-llm — any ready-made LLM behind one port (we never train our own model).
// Cloud providers are called via OUR server (no API key on the client);
// a local OpenAI-compatible server (Ollama, LM Studio, llama.cpp) is called on localhost.
#[async_trait]
pub trait LlmProvider {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse>;
    fn capabilities(&self) -> LlmCaps;
}
// NarrativeCoach<P: LlmProvider> builds prompts, validates numbers, falls back to templates.
// Details: 04_TRAINING_LOOP.md section 7.

// aim-coach — pure, injected
pub trait Clock { fn now(&self) -> DateTime<Utc>; }
```

**قاعدة ذهبية:** مفاتيح مزوّدي الـ LLM السحابيين وRiot وPaddle **لا توضع أبداً على جهاز اللاعب**. كل الاستدعاءات تمرّ عبر Edge Functions أو خادم Axum.

---

## 7. الخلفية (Backend)

### المرحلة 1 — Supabase (مناسب لمطوّر واحد)
- **Auth:** Email + Discord OAuth. وRiot Sign-On لاحقاً عند الحصول على مفتاح إنتاج.
- **Postgres:** المخطط في `05_DATA_AND_TELEMETRY.md` مع Row Level Security.
- **Storage:** ملفات `.aft` في bucket خاص لكل مستخدم.
- **Edge Functions:** `paddle-webhook` (تحديث الاشتراك)، `coach-explain` (وكيل للـ LLM السحابي مع حد استخدام)، `riot-sync`.

### المرحلة 2 — خدمة Axum (عند توفر فريق/تمويل)
- تستورد `aim-analysis` و`aim-coach` كما هي ← إعادة تحليل على السيرفر، وحساب المعايير (norms)، ومنع التلاعب.
- Supabase يبقى قاعدة بيانات Postgres فقط، أو يُنقل إلى Postgres مُدار. **التغيير محصور في adapter `aim-sync`.**
- Job شهري يحسب المعايير (norms) وجدول فعالية التمارين ويعيد معايرة العتبات (SQL + Rust). لا يوجد تدريب نماذج ذكاء اصطناعي.

---

## 8. مسارات الانتقال المستقبلية (لماذا التصميم مرن)

| التغيير المستقبلي | ما يتغيّر | ما يبقى كما هو |
|------------------|----------|----------------|
| إضافة CS2 | ملف `GameProfile` جديد في `aim-core` (yaw 0.022°/count، FOV مختلف) + سيناريوهات | التحليل والتدريب والواجهة |
| نسخة ويب | `aim-wasm` + محاكي WebGPU | كل منطق المجال |
| استبدال Bevy بمحرك آخر | `apps/trainer` فقط، بشرط أن يحترم IPC و`.aft` | كل شيء آخر |
| استبدال Supabase | adapter جديد لـ `SyncClient` | كل شيء آخر |
| تغيير مزوّد الـ LLM (سحابي ↔ محلي) | إعداد `LlmProvider` فقط | كل شيء آخر |
| سياسة اختيار تمارين أذكى (إحصائية) | implementation جديد لـ `trait DrillPolicy` | الواجهات والتخزين |
| تطبيق جوال مرافق | يقرأ نفس API | — |

```rust
// aim-analysis: diagnosis behind a trait; rules live in versioned data files
pub trait Diagnoser {
    fn diagnose(&self, profile: &SkillProfile, evidence: &EvidenceSet) -> Vec<Finding>;
}
pub struct RuleDiagnoser { rules: RuleSet }          // rules.vN.ron, recalibrated from data
```

---

## 9. الجودة والأدوات

| المجال | الأداة |
|------|-------|
| CI | GitHub Actions: `cargo fmt --check`، `clippy -D warnings`، `cargo test`، `cargo deny`، بناء Windows |
| اختبارات التحليل | **Golden tests:** ملفات `.aft` ثابتة ← التحليل يجب أن يعطي JSON مطابقاً لملف محفوظ |
| لاعبون اصطناعيون | `tools/synth`: يولّد حركات بنموذج minimum-jerk + ضجيج + overshoot مُتحكَّم به ← نتأكد أن الكاشف يكتشف ما زرعناه |
| Property tests | `proptest` لتحويلات الحساسية والزوايا |
| Benchmarks | `criterion` — هدف: تحليل جلسة 60 ثانية في أقل من 50ms |
| توزيع | Tauri bundler (MSI/NSIS) + `tauri-plugin-updater` + توقيع الكود |
| مراقبة الأعطال | `sentry` (Rust + JS) مع إخفاء البيانات الشخصية |
| الإصدارات | SemVer للتطبيق، وإصدار منفصل لكل من: صيغة `.aft`، والبطارية، والقواعد، والمعايير |
