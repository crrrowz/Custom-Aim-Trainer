# 🎮 الخطة 2: محاكاة فيزياء VALORANT بدقة 1:1

> **الهدف:** إعادة بناء كل جانب من جوانب فيزياء الحركة والتصويب في VALORANT داخل المتصفح.

---

## 🔬 تحليل فيزياء VALORANT الأصلية

### نموذج الحركة (Movement Model)
```
VALORANT Movement Constants (Source Engine derived):
─────────────────────────────────────────────────
Walk Speed:          3.53 m/s  (Jett, Reyna, etc.)
Run Speed:           5.73 m/s  (varies by agent)
Acceleration:        ~2000 units/s²
Counter-Strafe:      instant velocity = 0 (on opposite key)
Air Speed:           30% of ground speed
Friction:            ~4.0 (ground)
Stop Distance:       ~0.2s from full speed to zero
```

### نموذج الدقة (Accuracy Model)
```
VALORANT Accuracy States:
─────────────────────────────────────────────────
Standing Still:      100% accurate (after settle time)
Walking:             ~85% accurate (minor spread)
Running:             ~40% accurate (major spread)  
Counter-Strafing:    100% after ~0.1s (instant with perfect timing)
Jumping/Air:         ~15% accurate
Crouching:           110% accurate (tighter spread than standing)
```

---

## 📋 المهام التنفيذية

### المرحلة 1: Movement Engine v2

#### 1.1 نموذج Source Engine المبسّط
```javascript
class MovementEngineV2 {
  constructor() {
    // VALORANT-calibrated constants
    this.WALK_SPEED = 90;        // map units (scaled for canvas)
    this.RUN_SPEED = 150;
    this.ACCEL_GROUND = 2400;
    this.FRICTION_GROUND = 4.0;
    this.STOP_SPEED = 15;        // Below this → friction applies fully
    this.CS_WINDOW = 0.12;       // Counter-strafe timing window (seconds)
    
    // State
    this.velocity = { x: 0, y: 0 };
    this.wishDir = { x: 0, y: 0 };
    this.isGrounded = true;
    this.isCrouching = false;
    this.accuracyMultiplier = 1.0;
    this._csTimer = 0;
  }

  update(dt) {
    if (this.isGrounded) {
      this._groundMove(dt);
    } else {
      this._airMove(dt);
    }
    this._updateAccuracy(dt);
  }

  _groundMove(dt) {
    const speed = this._getSpeed();
    const wishSpeed = this.isCrouching ? speed * 0.65 : speed;
    
    // Apply friction FIRST (Source Engine order)
    this._applyFriction(dt);
    
    // Then accelerate
    this._accelerate(this.wishDir, wishSpeed, this.ACCEL_GROUND, dt);
  }

  _applyFriction(dt) {
    const speed = Math.sqrt(this.velocity.x ** 2 + this.velocity.y ** 2);
    if (speed < 1) { this.velocity.x = 0; this.velocity.y = 0; return; }
    
    const control = Math.max(speed, this.STOP_SPEED);
    const drop = control * this.FRICTION_GROUND * dt;
    const newSpeed = Math.max(0, speed - drop);
    
    if (newSpeed !== speed) {
      const factor = newSpeed / speed;
      this.velocity.x *= factor;
      this.velocity.y *= factor;
    }
  }

  _accelerate(wishDir, wishSpeed, accel, dt) {
    const currentSpeed = this.velocity.x * wishDir.x + this.velocity.y * wishDir.y;
    const addSpeed = wishSpeed - currentSpeed;
    if (addSpeed <= 0) return;
    
    const accelSpeed = Math.min(accel * dt * wishSpeed, addSpeed);
    this.velocity.x += accelSpeed * wishDir.x;
    this.velocity.y += accelSpeed * wishDir.y;
  }
}
```

#### 1.2 Counter-Strafe Precision
```javascript
_detectCounterStrafe(newDir) {
  // VALORANT counter-strafe: opposite key press → instant stop
  if (this._prevDir !== 0 && newDir !== 0 && 
      Math.sign(newDir) !== Math.sign(this._prevDir)) {
    
    const speed = Math.abs(this.velocity.x);
    const wasAtSpeed = speed > this.RUN_SPEED * 0.3;
    
    if (wasAtSpeed) {
      this.velocity.x = 0;  // Instant stop
      this._csTimer = this.CS_WINDOW;
      this._csTriggered = true;
      return true;
    }
  }
  return false;
}

// During CS window: accuracy recovers faster
_updateAccuracy(dt) {
  if (this._csTimer > 0) {
    this._csTimer -= dt;
    // Rapid accuracy recovery during CS window
    this.accuracyMultiplier = lerp(this.accuracyMultiplier, 1.0, 1 - Math.exp(-25 * dt));
  } else {
    // Normal accuracy based on speed
    const speed = Math.sqrt(this.velocity.x ** 2 + this.velocity.y ** 2);
    const speedRatio = speed / this.RUN_SPEED;
    
    if (speedRatio < 0.05) {
      this.accuracyMultiplier = 1.0;  // Standing still
    } else if (speedRatio < 0.3) {
      this.accuracyMultiplier = 0.85; // Walking
    } else {
      this.accuracyMultiplier = lerp(0.4, 0.1, (speedRatio - 0.3) / 0.7); // Running
    }
  }
}
```

---

### المرحلة 2: Weapon Spray Patterns

#### 2.1 نظام الانتشار (Spread System)
```javascript
class WeaponSpread {
  constructor(weaponData) {
    this.baseSpread = weaponData.baseSpread;
    this.moveSpread = weaponData.moveSpread;
    this.fireSpread = weaponData.fireSpread;
    this.recoveryRate = weaponData.recoveryRate;
    
    this.currentSpread = 0;
    this.burstIndex = 0;
  }

  getSpreadAngle(accuracyMultiplier) {
    const total = this.baseSpread + this.currentSpread;
    // Apply movement penalty
    const withMovement = total / accuracyMultiplier;
    // Random angle within spread cone
    return (Math.random() - 0.5) * 2 * withMovement;
  }

  onFire() {
    this.currentSpread += this.fireSpread;
    this.burstIndex++;
  }

  update(dt) {
    // Recover spread over time
    this.currentSpread = Math.max(0, this.currentSpread - this.recoveryRate * dt);
    if (this.currentSpread === 0) this.burstIndex = 0;
  }
}

// VALORANT weapon presets
const WEAPONS = {
  vandal: { baseSpread: 0.25, moveSpread: 3.0, fireSpread: 0.6, recoveryRate: 8.0 },
  phantom: { baseSpread: 0.2, moveSpread: 2.8, fireSpread: 0.45, recoveryRate: 10.0 },
  sheriff: { baseSpread: 0.15, moveSpread: 4.0, fireSpread: 1.5, recoveryRate: 4.0 },
  guardian: { baseSpread: 0.1, moveSpread: 3.5, fireSpread: 0.8, recoveryRate: 6.0 },
};
```

#### 2.2 First Shot Accuracy (FSA)
```
VALORANT FSA Rules:
───────────────────────────────
1. Must be standing still (speed < threshold)
2. Must not have fired in last ~0.4s (weapon dependent)
3. Crouching slightly tightens FSA
4. ADS tightens FSA by ~20-40% (weapon dependent)
5. Running → stop → FSA available after "settle time" (~0.15s)
```

---

### المرحلة 3: نظام Peeking و Jiggle Peek

#### 3.1 تتبع Peek Timing
```javascript
class PeekAnalyzer {
  constructor() {
    this.peekHistory = [];
    this.currentPeek = null;
  }

  onMovementChange(dir, velocity, timestamp) {
    if (dir !== 0 && !this.currentPeek) {
      // Start of peek
      this.currentPeek = {
        startTime: timestamp,
        startDir: dir,
        maxExposure: 0,
        shotsFired: 0,
        shotsHit: 0,
      };
    }
    
    if (this.currentPeek && dir === -this.currentPeek.startDir) {
      // Counter-strafe detected → peek ending
      this.currentPeek.duration = timestamp - this.currentPeek.startTime;
      this.currentPeek.peekType = this._classifyPeek(this.currentPeek);
      this.peekHistory.push(this.currentPeek);
      this.currentPeek = null;
    }
  }

  _classifyPeek(peek) {
    if (peek.duration < 0.15) return 'jiggle';        // Jiggle peek
    if (peek.duration < 0.4) return 'quick';           // Quick peek
    if (peek.duration < 0.8) return 'normal';          // Normal peek  
    return 'wide';                                      // Wide/slow peek
  }

  getReport() {
    return {
      totalPeeks: this.peekHistory.length,
      avgPeekDuration: avg(this.peekHistory.map(p => p.duration)),
      peekTypes: this._countTypes(),
      bestPeek: this._findBest(),
      consistency: this._calcConsistency(),
    };
  }
}
```

---

## 📊 مقارنة الدقة

| الميزة | AimForge الحالي | AimForge المطلوب | VALORANT الأصلي |
|--------|----------------|-----------------|----------------|
| Counter-Strafe | ✅ فوري | ✅ مع timing window | ✅ |
| Accuracy States | ❌ (moving/not) | ✅ (4 states) | ✅ |
| Weapon Spread | ❌ | ✅ | ✅ |
| FSA Recovery | ❌ | ✅ | ✅ |
| Peek Tracking | ❌ | ✅ | N/A |
| Crouch Accuracy | ❌ | ✅ | ✅ |
| Run Inaccuracy | ❌ (binary) | ✅ (graduated) | ✅ |

---

## ⚠️ ملاحظات مهمة

> [!IMPORTANT]
> قيم الفيزياء في VALORANT ليست مُعلنة رسمياً من Riot. القيم المذكورة هنا مبنية على:
> 1. تحليل فيديوهات بـ frame-by-frame analysis
> 2. قياسات المجتمع (r/VALORANT, Blitz.gg data)
> 3. مقارنة مع Source Engine (الأساس الذي بُني عليه VALORANT)
> 
> يجب أن تبقى القيم قابلة للتعديل من الـ UI حتى يتمكن اللاعب من الضبط الدقيق.
