# 🗄️ البيانات والتليمتري

## 1. صيغة `.aft` (AimForge Telemetry) — v1

**المبادئ:** مضغوطة، ومُرقّمة الإصدار، وتحتوي كل ما يلزم لإعادة التحليل بالكامل **دون أي مصدر خارجي**.

```
┌────────────────────────────────────────────┐
│ Magic "AFT\0" (4B) │ format_version u16    │
├────────────────────────────────────────────┤
│ Header (postcard-serialized, zstd)         │
│  session_id, created_at (UTC)              │
│  app_version, trainer_version              │
│  scenario_id, scenario_version, form, seed │
│  game_profile ("valorant.v1")              │
│  mouse: {dpi, sens, deg_per_count}         │
│  display: {w, h, hz, hfov, vfov}           │
│  input: {est_polling_hz}                   │
│  thresholds_version (للمرجعية فقط)          │
├────────────────────────────────────────────┤
│ Streams (columnar, delta-encoded, zstd):   │
│  input:   t_us[], dx[], dy[]               │  ← كل حدث raw
│  frames:  t_us[], present_t_us[]           │  ← لحساب زمن العرض
│  view:    t_us[], yaw[], pitch[]           │  ← مُشتق، يُخزَّن للتحقق
│  targets: id, spawn_t, despawn_t, kind,    │
│           params (size, path spline, …)    │
│  shots:   t_us[], hit_target_id?, hitzone  │
│  keys:    t_us[], key, down/up             │  ← WASD لـ counter-strafe
│  player:  t_us[], vel_x[], vel_z[]         │
├────────────────────────────────────────────┤
│ Footer: CRC32 + stream offsets             │
└────────────────────────────────────────────┘
```

- **الحجم المتوقّع:** جلسة 60 ثانية بماوس 1000Hz ≈ 60k حدث × ~4 بايت بعد الترميز التفاضلي والضغط ≈ **150–300KB**. بطارية كاملة ≈ 1–2MB.
- **الترقية:** `aim-telemetry` يقرأ كل الإصدارات السابقة ويرقّيها في الذاكرة (`upgrade_v1_to_v2`). **لا نحذف دعم إصدار قديم أبداً.**
- **مسار الأهداف:** يُخزَّن كـ spline/معادلة وليس كعيّنات، لأن السيناريو حتمي من الـ seed. الـ seed وحده يكفي نظرياً، لكن نخزّن المعاملات للأمان ضد تغيّر كود المولّد.

---

## 2. التخزين المحلي (SQLite عبر `aim-store`)

يعمل التطبيق **بالكامل دون اتصال (offline-first)**، والمزامنة لاحقة.

```sql
CREATE TABLE sessions (
  id              TEXT PRIMARY KEY,         -- uuid v7 (مرتّب زمنياً)
  kind            TEXT NOT NULL,            -- 'assessment' | 'drill' | 'minicheck' | 'free'
  scenario_id     TEXT NOT NULL,
  form            INTEGER,
  seed            INTEGER NOT NULL,
  started_at      TEXT NOT NULL,
  duration_ms     INTEGER NOT NULL,
  dpi             INTEGER NOT NULL,
  sens            REAL NOT NULL,
  display_hz      INTEGER,
  telemetry_path  TEXT NOT NULL,
  telemetry_sha256 TEXT NOT NULL,
  plan_id         TEXT,
  block_no        INTEGER,
  quality_flags   INTEGER NOT NULL DEFAULT 0,  -- bitmask: frame_drops, settings_changed, aborted
  synced_at       TEXT
);

CREATE TABLE skill_observations (
  session_id  TEXT NOT NULL REFERENCES sessions(id),
  skill       TEXT NOT NULL,             -- 'S04_GAIN_CONTROL'
  raw         REAL NOT NULL,
  score       REAL,
  ci_lo       REAL, ci_hi REAL,
  n_evidence  INTEGER NOT NULL,
  analysis_version TEXT NOT NULL,        -- لإعادة التحليل لاحقاً
  norms_version    TEXT,
  PRIMARY KEY (session_id, skill, analysis_version)
);

CREATE TABLE findings (
  id          TEXT PRIMARY KEY,
  assessment_session_id TEXT NOT NULL REFERENCES sessions(id),
  rule_id     TEXT NOT NULL,            -- 'D2'
  severity    REAL NOT NULL,
  priority    REAL NOT NULL,
  evidence    TEXT NOT NULL,            -- JSON
  rules_version TEXT NOT NULL
);

CREATE TABLE plans (
  id          TEXT PRIMARY KEY,
  created_at  TEXT NOT NULL,
  state       TEXT NOT NULL,            -- JSON of PlanState
  focus_skills TEXT NOT NULL,           -- JSON array
  baseline_session_id TEXT NOT NULL
);

CREATE TABLE plan_blocks (
  plan_id     TEXT NOT NULL REFERENCES plans(id),
  block_no    INTEGER NOT NULL,
  drills      TEXT NOT NULL,            -- JSON [{drill_id, minutes, start_d}]
  started_at  TEXT, ended_at TEXT,
  retest_session_id TEXT,
  outcome     TEXT,                     -- JSON: per-skill verdict, delta, mdc, did
  PRIMARY KEY (plan_id, block_no)
);

CREATE TABLE drill_state (
  drill_id    TEXT PRIMARY KEY,
  difficulty  REAL NOT NULL,            -- آخر d من الـ staircase
  updated_at  TEXT NOT NULL
);
```

---

## 3. السحابة (Postgres في Supabase)

نفس الجداول تقريباً + `user_id`، ونضيف:

```sql
CREATE TABLE profiles (
  user_id       UUID PRIMARY KEY REFERENCES auth.users(id),
  display_name  TEXT,
  region        TEXT,
  declared_rank TEXT,                    -- يعلنه المستخدم
  riot_puuid    TEXT UNIQUE,             -- فقط بعد ربط RSO
  verified_rank TEXT,                    -- من Riot API
  locale        TEXT DEFAULT 'en',
  consent_research BOOLEAN NOT NULL DEFAULT false,  -- موافقة استخدام البيانات المجهولة للنماذج
  created_at    TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE entitlements (
  user_id     UUID PRIMARY KEY REFERENCES auth.users(id),
  tier        TEXT NOT NULL CHECK (tier IN ('free','pro','team')),
  source      TEXT NOT NULL,             -- 'paddle' | 'beta' | 'promo'
  external_id TEXT,                      -- Paddle subscription id
  valid_until TIMESTAMPTZ,
  updated_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE norms (
  version    TEXT NOT NULL,              -- 'norms.v3'
  skill      TEXT NOT NULL,
  cohort     TEXT NOT NULL,              -- 'all' | 'gold' | 'diamond' ...
  percentiles REAL[] NOT NULL,           -- p1..p99
  sem        REAL, mdc95 REAL,
  n          INTEGER NOT NULL,
  PRIMARY KEY (version, skill, cohort)
);

CREATE TABLE team_members (
  team_id UUID, user_id UUID, role TEXT CHECK (role IN ('owner','coach','player')),
  PRIMARY KEY (team_id, user_id)
);

ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;
CREATE POLICY own_sessions ON sessions
  USING (user_id = auth.uid()
         OR EXISTS (SELECT 1 FROM team_members c JOIN team_members p USING (team_id)
                    WHERE c.user_id = auth.uid() AND c.role IN ('owner','coach')
                      AND p.user_id = sessions.user_id));
```

### المزامنة
- **Push:** الجلسات والملاحظات والخطط (append-only تقريباً ← تعارضات قليلة؛ last-writer-wins للـ `plans.state` بختم زمني).
- **Upload:** ملفات `.aft` إلى `storage/telemetry/{user_id}/{session_id}.aft` (لمشتركي Pro، أو لمن وافق على `consent_research`).
- **Pull:** الـ norms، والـ entitlements، وتحديثات المحتوى (drills/rules) الموقّعة.

---

## 4. الخصوصية والامتثال

| البند | القرار |
|------|-------|
| البيانات الشخصية | البريد، الاسم المعروض، Riot ID (اختياري). لا شيء آخر |
| التليمتري | بيانات حركة ماوس ليست شخصية بذاتها، لكنها مرتبطة بالحساب ← نعاملها كبيانات شخصية |
| استخدامها للبحث/النماذج | **موافقة صريحة منفصلة** (`consent_research`)، مع إزالة الربط بالهوية في مجموعة التدريب |
| الحذف | زر "احذف حسابي وكل بياناتي" يحذف السجلات والملفات خلال 30 يوماً (متطلب GDPR) |
| التصدير | زر "حمّل بياناتي" (JSON + ملفات `.aft`) |
| القاصرون | كثير من لاعبي VALORANT دون 18 ← شروط استخدام مناسبة، وعمر أدنى 13 (أو 16 في بعض دول الاتحاد الأوروبي) |
| مفاتيح الطرف الثالث | على السيرفر فقط |
| سياسة الخصوصية والشروط | مطلوبة قبل الـ Beta العامة (قالب + مراجعة) |

---

## 5. بيانات Riot (اختياري، لقياس الانتقال للعبة)

- **الحالة:** يتطلّب تسجيل المنتج لدى Riot ومفتاح إنتاج، والوصول لبيانات مباريات اللاعب يتطلب موافقته عبر **Riot Sign-On**. نتحقق من المتطلبات الدقيقة عند التقديم.
- **ما نستخدمه:** HS%، ونسبة First Kill/First Death، والـ ADR، والرتبة، لكل مباراة. **وكلها مجمّعة وعلى مستوى المباراة فقط.**
- **التحليل:** ارتباط بين تغيّر المهارات في المنصة وتغيّر HS%/FK في اللعبة على مستوى المجموعة. هذا **دليل الانتقال للعبة** الذي نستخدمه في التسويق وفي معايرة `impact_weight`.
- **أثناء التطوير:** HenrikDev مسموح محلياً فقط، **ولا يُشحن في المنتج**.
- **إصلاح `check-player.html`:** إن بقي، يجب إزالة إدخال المفتاح من المتصفح، واستبدال `innerHTML` بـ `textContent`/عناصر DOM لمنع XSS.
