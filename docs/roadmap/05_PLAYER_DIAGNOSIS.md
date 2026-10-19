# 🏥 الخطة 5: تشخيص مشاكل اللاعب وعلاجها

> **الهدف:** نظام ذكي يحدد نقاط الضعف الدقيقة في أداء اللاعب ويوصي بتمارين مخصصة لحلها.

---

## 🔬 مشاكل اللاعبين الشائعة في VALORANT

### خريطة المشاكل
```
                    مشاكل التصويب
                    ─────────────
                         │
        ┌────────────────┼────────────────┐
        │                │                │
   ⚡ السرعة        🎯 الدقة        🧠 القرار
        │                │                │
   ┌────┼────┐     ┌────┼────┐     ┌────┼────┐
   │    │    │     │    │    │     │    │    │
  بطء  عدم  عدم  over  under micro  peek  cross
  رد   ثبات  flick shoot shoot adjust timing place
  فعل  TTK   سريع                         ment
```

---

## 📋 وحدات التشخيص

### 5.1 Overshoot / Undershoot Detector
```javascript
class OverUnderDetector {
  // المشكلة: اللاعب يتجاوز الهدف (overshoot) أو لا يصل إليه (undershoot)
  
  analyze(shotEvents, telemetry) {
    const results = { overshoot: 0, undershoot: 0, perfect: 0 };
    
    for (const shot of shotEvents) {
      if (!shot.isHit) continue;
      
      // نحلل الـ 100ms الأخيرة قبل الطلقة
      const preShotMovement = this._getPreShotMovement(telemetry, shot.t, 100);
      
      if (preShotMovement.hasDirectionChange) {
        // الماوس غيّر اتجاهه = overshoot (تجاوز وعاد)
        results.overshoot++;
      } else if (preShotMovement.isDecelerating && shot.distToCenter > shot.targetRadius * 0.5) {
        // الماوس يتباطأ ولم يصل للمنتصف = undershoot
        results.undershoot++;
      } else {
        results.perfect++;
      }
    }
    
    const total = results.overshoot + results.undershoot + results.perfect;
    
    return {
      overshootRate: results.overshoot / total,
      undershootRate: results.undershoot / total,
      perfectRate: results.perfect / total,
      
      diagnosis: this._diagnose(results, total),
      prescription: this._prescribe(results, total),
    };
  }
  
  _diagnose(results, total) {
    const osRate = results.overshoot / total;
    const usRate = results.undershoot / total;
    
    if (osRate > 0.4) return {
      problem: 'OVERSHOOT_DOMINANT',
      severity: osRate > 0.6 ? 'severe' : 'moderate',
      explanation: 'أنت تتجاوز الهدف كثيراً ثم تعود. هذا يعني أن حساسيتك عالية أو أنك تفليك بقوة زائدة.',
    };
    
    if (usRate > 0.4) return {
      problem: 'UNDERSHOOT_DOMINANT', 
      severity: usRate > 0.6 ? 'severe' : 'moderate',
      explanation: 'أنت لا تصل للهدف وتطلق قبل الوصول. هذا يعني أن حساسيتك منخفضة أو أنك تستعجل.',
    };
    
    return {
      problem: 'BALANCED',
      severity: 'none',
      explanation: 'توازن جيد بين الدقة والسرعة.',
    };
  }
  
  _prescribe(results, total) {
    const osRate = results.overshoot / total;
    const usRate = results.undershoot / total;
    
    const prescriptions = [];
    
    if (osRate > 0.35) {
      prescriptions.push({
        action: 'REDUCE_SENSITIVITY',
        detail: `خفّض حساسيتك بنسبة ${Math.round(osRate * 30)}%`,
        scenario: 'Precision Micro-Adjust',
        settings: { targetRadius: 12, lifespan: 3, roundCount: 30 },
      });
    }
    
    if (usRate > 0.35) {
      prescriptions.push({
        action: 'INCREASE_CONFIDENCE',
        detail: 'تدرب على التصويب بثقة أكبر مع أهداف أكبر حجماً',
        scenario: 'Speed Flick Training',
        settings: { targetRadius: 32, lifespan: 1.5, roundCount: 50 },
      });
    }
    
    return prescriptions;
  }
}
```

### 5.2 Crosshair Placement Analyzer
```javascript
class CrosshairPlacementAnalyzer {
  // المشكلة: اللاعب لا يضع الكروسهير في المكان الصحيح مسبقاً
  
  analyze(telemetry, targets) {
    const placements = [];
    
    for (const target of targets) {
      // عندما يظهر الهدف، أين كان الكروسهير؟
      const spawnMoment = telemetry.findFrame(target.spawnTime);
      const distFromTarget = dist(
        spawnMoment.cameraX, spawnMoment.cameraY,
        target.x, target.y
      );
      
      placements.push({
        distance: distFromTarget,
        angle: Math.atan2(target.y - spawnMoment.cameraY, target.x - spawnMoment.cameraX),
        reactionNeeded: distFromTarget > target.radius * 2,
      });
    }
    
    return {
      avgPreAimDistance: avg(placements.map(p => p.distance)),
      preAimedRate: placements.filter(p => !p.reactionNeeded).length / placements.length,
      
      // أي منطقة من الشاشة تحتاج تحسين؟
      weakZones: this._findWeakZones(placements),
      
      diagnosis: this._diagnose(placements),
    };
  }
  
  _findWeakZones(placements) {
    // قسّم الشاشة إلى 8 مناطق (مثل البوصلة)
    const zones = { N: [], NE: [], E: [], SE: [], S: [], SW: [], W: [], NW: [] };
    
    for (const p of placements) {
      const zoneName = this._angleToZone(p.angle);
      zones[zoneName].push(p.distance);
    }
    
    // أضعف منطقة = أعلى متوسط مسافة
    return Object.entries(zones)
      .map(([name, dists]) => ({ zone: name, avgDist: avg(dists) || 0, count: dists.length }))
      .sort((a, b) => b.avgDist - a.avgDist)
      .slice(0, 3); // أضعف 3 مناطق
  }
}
```

### 5.3 Counter-Strafe Quality Checker
```javascript
class CounterStrafeChecker {
  // المشكلة: اللاعب لا يتوقف قبل إطلاق النار (running accuracy)
  
  analyze(shotEvents) {
    const results = {
      shotWhileMoving: 0,
      shotWhileStopping: 0,
      shotWhileStopped: 0,
      perfectCS: 0,
    };
    
    for (const shot of shotEvents) {
      const speed = Math.abs(shot.playerVelocity);
      
      if (speed > 150) results.shotWhileMoving++;
      else if (speed > 50) results.shotWhileStopping++;
      else if (shot.wasCounterStrafed) results.perfectCS++;
      else results.shotWhileStopped++;
    }
    
    const total = shotEvents.length;
    const movingRate = results.shotWhileMoving / total;
    
    return {
      movingRate,
      stoppingRate: results.shotWhileStopping / total,
      stoppedRate: results.shotWhileStopped / total,
      perfectCSRate: results.perfectCS / total,
      
      grade: movingRate < 0.1 ? 'A' :
             movingRate < 0.2 ? 'B' :
             movingRate < 0.35 ? 'C' :
             movingRate < 0.5 ? 'D' : 'F',
      
      prescription: movingRate > 0.2 ? {
        problem: 'RUNNING_AND_GUNNING',
        explanation: `أنت تطلق أثناء الحركة في ${(movingRate * 100).toFixed(0)}% من الطلقات. في VALORANT هذا يعني عدم دقة شديدة.`,
        exercise: 'فعّل Counter-Strafe وتدرب على الضغط A→D→إطلاق',
        scenario: 'Counter-Strafe Drill',
        settings: { counterStrafeEnabled: true, targetLifespan: 2, roundCount: 20 },
      } : null,
    };
  }
}
```

### 5.4 Timing & Patience Analyzer
```javascript
class TimingAnalyzer {
  // المشكلة: اللاعب يطلق مبكراً جداً أو متأخراً جداً
  
  analyze(shotEvents) {
    const earlyShots = [];   // أطلق قبل أن يكون دقيقاً
    const lateShots = [];    // انتظر طويلاً وفقد الفرصة
    const goodShots = [];    // توقيت مثالي
    
    for (const shot of shotEvents) {
      if (!shot.isHit && shot.distToCenter < shot.targetRadius * 1.5) {
        // كان قريباً لكن أخطأ → أطلق مبكراً
        earlyShots.push(shot);
      } else if (shot.reactionTime > 1500) {
        // انتظر أكثر من 1.5 ثانية
        lateShots.push(shot);
      } else if (shot.isHit) {
        goodShots.push(shot);
      }
    }
    
    return {
      earlyRate: earlyShots.length / shotEvents.length,
      lateRate: lateShots.length / shotEvents.length,
      goodRate: goodShots.length / shotEvents.length,
      
      avgGoodTiming: avg(goodShots.map(s => s.reactionTime)),
      
      diagnosis: this._diagnose(earlyShots, lateShots, shotEvents),
    };
  }
  
  _diagnose(early, late, all) {
    if (early.length / all.length > 0.25) {
      return {
        problem: 'TRIGGER_HAPPY',
        tip: 'أنت تستعجل بالإطلاق. خذ 0.1 ثانية إضافية للتأكد من الهدف.',
        exercise: 'تدرب مع أهداف صغيرة (radius 12) لإجبارك على الدقة',
      };
    }
    if (late.length / all.length > 0.2) {
      return {
        problem: 'TOO_PASSIVE',
        tip: 'أنت تنتظر طويلاً. في VALORANT الـ peek الطويل = موت مؤكد.',
        exercise: 'تدرب مع Target Lifespan = 1.5s لإجبارك على السرعة',
      };
    }
    return { problem: 'GOOD_TIMING', tip: 'توقيت إطلاقك ممتاز!' };
  }
}
```

---

## 🏥 التقرير التشخيصي الشامل

```
┌─────────────────────────────────────────────────┐
│           🏥 PLAYER DIAGNOSIS REPORT            │
│            Session: 2024-01-15 22:30            │
├─────────────────────────────────────────────────┤
│                                                  │
│  🎯 Aim Type: Flick-Dominant (72% flicks)       │
│  ⚡ Avg Reaction: 312ms (needs improvement)     │
│  📊 Consistency: 67% (moderate variance)        │
│                                                  │
│  ━━━ IDENTIFIED ISSUES ━━━                      │
│                                                  │
│  🔴 Overshoot Rate: 45% (SEVERE)                │
│     → خفّض الحساسية من 0.35 إلى 0.28            │
│     → تدرب على: Micro-Adjust Drill              │
│                                                  │
│  🟡 Counter-Strafe: Grade C (32% running shots) │
│     → فعّل Counter-Strafe في الإعدادات           │
│     → تدرب على: CS Drill (A→D→Click)            │
│                                                  │
│  🟢 Headshot Rate: 28% (Good)                   │
│     → استمر بالتحسن!                             │
│                                                  │
│  ━━━ RECOMMENDED TRAINING ━━━                   │
│                                                  │
│  1. [Micro-Adjust] 15 min/day                   │
│  2. [Counter-Strafe Drill] 10 min/day           │
│  3. [Speed Flick] 10 min/day                    │
│                                                  │
│  [Generate Custom Scenario] [Export PDF]          │
└─────────────────────────────────────────────────┘
```

---

## 🔄 Auto-Generated Scenarios

```javascript
class ScenarioGenerator {
  generateFromDiagnosis(diagnosis) {
    const scenarios = [];
    
    for (const issue of diagnosis.issues) {
      switch (issue.problem) {
        case 'OVERSHOOT_DOMINANT':
          scenarios.push({
            name: '🎯 Micro-Adjust Drill',
            targets: this._createSmallTargets(8, 12), // 8 targets, radius 12
            settings: {
              targetLifespan: 4,
              respawnDelay: 0.5,
              roundCount: 30,
              sensitivity: diagnosis.currentSens * 0.8, // اقتراح حساسية أقل
            },
          });
          break;
          
        case 'UNDERSHOOT_DOMINANT':
          scenarios.push({
            name: '⚡ Speed Training',
            targets: this._createLargeTargets(6, 28),
            settings: {
              targetLifespan: 1.5,
              respawnDelay: 0.3,
              roundCount: 50,
              randomRespawn: true,
            },
          });
          break;
          
        case 'RUNNING_AND_GUNNING':
          scenarios.push({
            name: '🛑 Counter-Strafe Drill',
            targets: this._createStaticTargets(3, 20),
            settings: {
              targetLifespan: 2,
              counterStrafeEnabled: true,
              roundCount: 20,
            },
          });
          break;
      }
    }
    
    return scenarios;
  }
}
```
