# 🧠 الخطة 3: الذكاء الاصطناعي والتحليل البيوميكانيكي

> **الهدف:** بناء نظام تحليل ذكي يفهم حركة يد اللاعب ويُقيّم أداءه بدقة علمية، ويُنتج تقارير تتفوق على Aim Lab Premium.

---

## 🔬 البيانات المُجمّعة (Raw Telemetry)

### ما نجمعه كل frame:
```javascript
class SessionTelemetry {
  constructor() {
    this.samples = [];        // كل frame
    this.shotEvents = [];     // كل طلقة
    this.killEvents = [];     // كل قتل
    this.peekEvents = [];     // كل peek
  }

  recordFrame(data) {
    this.samples.push({
      t: performance.now(),
      mouseX: data.rawMovementX,
      mouseY: data.rawMovementY,
      cameraX: data.cameraX,
      cameraY: data.cameraY,
      velocity: data.playerVelocity,
      isMoving: data.isMoving,
      accuracyState: data.accuracyState,
    });
  }

  recordShot(data) {
    this.shotEvents.push({
      t: performance.now(),
      worldX: data.crosshairWorldX,
      worldY: data.crosshairWorldY,
      targetId: data.targetId || null,
      isHit: data.isHit,
      isHeadshot: data.isHeadshot,
      distToCenter: data.distToCenter,   // Distance from target center
      playerVelocity: data.velocity,
      reactionTime: data.reactionTime,   // Time since target appeared
      flick: data.flickAngle,            // Angle of mouse flick
    });
  }
}
```

---

## 📊 التحليلات (Analytics Modules)

### 3.1 Path Efficiency (كفاءة المسار)
```
المفهوم:
═══════
المسافة المثالية (خط مستقيم) بين هدفين ÷ المسافة الفعلية التي تحركت بها يدك

Path Efficiency = idealDistance / actualDistance × 100%

┌─ Target A ─────────────────── Target B ─┐
│                                          │
│  Ideal: ─────────────── (100%)           │
│  Good:  ─────∽──────── (85%)             │
│  Bad:   ──╱╲──╱╲──╱╲── (40%)            │
└──────────────────────────────────────────┘

اللاعب المحترف: 85-95%
اللاعب المتوسط: 55-75%  
اللاعب المبتدئ: 30-50%
```

### 3.2 Flick Analysis (تحليل الفليك)
```javascript
class FlickAnalyzer {
  analyze(shotEvents) {
    const flicks = shotEvents.filter(s => s.flickAngle > 15); // > 15° = flick
    
    return {
      // سرعة الفليك
      avgFlickSpeed: avg(flicks.map(f => f.flickAngle / f.reactionTime)),
      
      // دقة الفليك
      flickAccuracy: flicks.filter(f => f.isHit).length / flicks.length,
      
      // Overshoot vs Undershoot
      overshootRate: this._calcOvershoot(flicks),
      undershootRate: this._calcUndershoot(flicks),
      
      // اتجاه الفليك الأقوى
      bestDirection: this._bestDirection(flicks), // 'left', 'right', 'up', 'down'
      worstDirection: this._worstDirection(flicks),
      
      // الزاوية المثالية (حيث تكون الدقة أعلى)
      sweetSpotAngle: this._sweetSpot(flicks),
    };
  }

  _calcOvershoot(flicks) {
    // اللاعب يتجاوز الهدف ثم يعود
    // نحلل آخر 50ms قبل الطلقة: هل كان الماوس يتراجع؟
    return flicks.filter(f => {
      const lastMovement = f.mouseDeltaBeforeShot;
      return Math.sign(lastMovement) !== Math.sign(f.totalFlickDelta);
    }).length / flicks.length;
  }
}
```

### 3.3 Reaction Time Breakdown (تحليل وقت الاستجابة)
```
وقت الاستجابة الكلي = وقت الرؤية + وقت القرار + وقت الحركة + وقت التصحيح

┌────────────────────────────────────────────────────┐
│  Target Appears                                     │
│       ↓                                             │
│  [Visual Processing] ─── 80-150ms (ثابت تقريباً)    │
│       ↓                                             │
│  [Decision] ─── 30-80ms (يتحسن بالتدريب)            │
│       ↓                                             │
│  [Ballistic Movement] ─── 100-300ms (الفليك)         │
│       ↓                                             │
│  [Correction Phase] ─── 0-200ms (ضبط دقيق)          │
│       ↓                                             │
│  Shot Fired!                                        │
└────────────────────────────────────────────────────┘
```

```javascript
class ReactionBreakdown {
  analyze(telemetry) {
    // نحدد متى بدأ اللاعب بتحريك الماوس بعد ظهور الهدف
    const visualReaction = this._timeToFirstMovement(telemetry);
    
    // نحدد متى وصل الماوس لمنطقة قريبة من الهدف (80% من المسافة)
    const ballisticPhase = this._timeToApproach(telemetry);
    
    // نحدد الوقت المستغرق في التصحيح الدقيق (آخر 20%)
    const correctionPhase = this._timeToPrecise(telemetry);
    
    return {
      total: visualReaction + ballisticPhase + correctionPhase,
      visual: visualReaction,
      ballistic: ballisticPhase,
      correction: correctionPhase,
      
      // تقييم
      bottleneck: this._identifyBottleneck(visualReaction, ballisticPhase, correctionPhase),
    };
  }
  
  _identifyBottleneck(visual, ballistic, correction) {
    if (visual > 200) return { phase: 'visual', tip: 'تدرب على Track & React scenarios' };
    if (ballistic > 250) return { phase: 'ballistic', tip: 'تدرب على Flick scenarios بزوايا كبيرة' };
    if (correction > 150) return { phase: 'correction', tip: 'اخفض الـ Sensitivity أو تدرب على Micro-adjust' };
    return { phase: 'none', tip: 'أداء ممتاز! حافظ على التناسق' };
  }
}
```

### 3.4 Consistency Score (درجة التناسق)
```javascript
class ConsistencyAnalyzer {
  analyze(sessionHistory) {
    // Standard Deviation of TTK across all kills
    const ttkValues = sessionHistory.map(s => s.ttk);
    const ttkStdDev = standardDeviation(ttkValues);
    
    // Standard Deviation of accuracy across 10-kill windows
    const accWindows = this._windowedAccuracy(sessionHistory, 10);
    const accStdDev = standardDeviation(accWindows);
    
    // Consistency = inverse of variance (normalized 0-100)
    const ttkConsistency = Math.max(0, 100 - ttkStdDev * 0.5);
    const accConsistency = Math.max(0, 100 - accStdDev * 2);
    
    return {
      overall: (ttkConsistency + accConsistency) / 2,
      ttkConsistency,
      accConsistency,
      
      // Trend: improving or degrading over session?
      trend: this._calcTrend(ttkValues),
      
      // Fatigue detection
      fatigueIndex: this._detectFatigue(ttkValues),
    };
  }
  
  _detectFatigue(ttkValues) {
    // Compare first 30% vs last 30% of session
    const firstThird = ttkValues.slice(0, Math.floor(ttkValues.length * 0.3));
    const lastThird = ttkValues.slice(Math.floor(ttkValues.length * 0.7));
    
    const earlyAvg = avg(firstThird);
    const lateAvg = avg(lastThird);
    
    // If late TTK is 15%+ worse → fatigue detected
    const degradation = ((lateAvg - earlyAvg) / earlyAvg) * 100;
    return {
      detected: degradation > 15,
      degradationPercent: degradation,
      recommendation: degradation > 15 
        ? 'أداؤك تراجع في نهاية الجلسة. خذ استراحة 5 دقائق وعد.'
        : 'لا توجد علامات إرهاق. أداء ثابت!'
    };
  }
}
```

---

## 🏆 نظام الرتب المتقدم (Advanced Ranking)

### المعادلة النهائية
```
Final Score = Σ(weights × metrics)

Weights:
────────────────────────────────
Accuracy         × 30%
Headshot Rate    × 15%
Path Efficiency  × 20%
Avg TTK (inv)    × 15%
Consistency      × 10%
Peek Quality     × 10%
────────────────────────────────

Score → Rank Mapping:
────────────────────────────────
0-999      → Iron      🛡️  (#a19d94)
1000-1999  → Bronze    🥉  (#cd7f32)
2000-3999  → Silver    🥈  (#bdc3c7)
4000-5999  → Gold      🏆  (#f1c40f)
6000-7999  → Platinum  💠  (#4ebcc6)
8000-11999 → Diamond   💎  (#b489ff)
12000-15999→ Ascendant 🟢  (#24a070)
16000-19999→ Immortal  🔴  (#ff4655)
20000+     → Radiant   ✨  (#fffbe6)
```

### الرتب الفرعية
```
كل رتبة رئيسية تنقسم إلى 3 مستويات:
  Diamond 1 (8000-9300)
  Diamond 2 (9300-10600)
  Diamond 3 (10600-12000)
  
مع شريط تقدم (Progress Bar) يوضح موقعك داخل المستوى.
```

---

## 📈 تصور البيانات (Visualization)

### لوحة النتائج النهائية
```
┌─────────────────────────────────────────┐
│         SESSION COMPLETE                 │
│                                         │
│            ✨ RADIANT ✨                 │
│          Score: 24,350                   │
│                                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐  │
│  │ 47 Hits │ │ 12 HS   │ │ 3 Miss  │  │
│  │         │ │ (25.5%) │ │         │  │
│  └─────────┘ └─────────┘ └─────────┘  │
│                                         │
│  Accuracy  ██████████████░░ 94.0%      │
│  Path Eff  ████████████░░░░ 82.3%      │
│  Consist.  ███████████████░ 91.7%      │
│  TTK Avg   ██████████░░░░░ 245ms      │
│                                         │
│  ⚠️ Bottleneck: Correction Phase       │
│  💡 Try micro-adjust scenarios         │
│                                         │
│  [Continue]  [View Details]  [Share]    │
└─────────────────────────────────────────┘
```

### Mouse Path Heatmap (خريطة حرارية لمسار الماوس)
```javascript
class PathHeatmap {
  constructor(width, height, resolution = 4) {
    this.cellW = width / resolution;
    this.cellH = height / resolution;
    this.grid = new Float32Array(resolution * resolution);
  }

  addSample(x, y) {
    const cx = Math.floor(x / this.cellW);
    const cy = Math.floor(y / this.cellH);
    if (cx >= 0 && cy >= 0) {
      this.grid[cy * this.cols + cx]++;
    }
  }

  render(ctx) {
    const max = Math.max(...this.grid);
    for (let i = 0; i < this.grid.length; i++) {
      const intensity = this.grid[i] / max;
      const x = (i % this.cols) * this.cellW;
      const y = Math.floor(i / this.cols) * this.cellH;
      
      // Blue → Green → Yellow → Red
      ctx.fillStyle = `hsl(${(1 - intensity) * 240}, 100%, 50%)`;
      ctx.globalAlpha = intensity * 0.6;
      ctx.fillRect(x, y, this.cellW, this.cellH);
    }
  }
}
```
