# 🚀 الخطة 1: محرك الأداء العالي (Performance Engine)

> **الهدف:** الوصول إلى 144+ FPS ثابت مع عشرات الأهداف والمؤثرات، بدون أي frame drop.

---

## 🔍 تحليل الوضع الحالي

### المشاكل
- Canvas 2D API يعتبر بطيء لـ rendering المكثف
- كل frame يُعاد رسم كل شيء (no dirty-rect optimization)
- Particle system يستخدم `Array.splice` مما يسبب GC pressure
- لا يوجد object pooling للأهداف أو الجسيمات
- `requestAnimationFrame` بدون frame budget monitoring

### القياسات المطلوبة (Benchmarks)
```
السيناريو                    | الهدف الحالي | الهدف المطلوب
─────────────────────────────┼──────────────┼──────────────
5 أهداف ثابتة               | ~60 FPS      | 240+ FPS
20 هدف متحرك + particles    | ~35 FPS      | 144+ FPS
50 هدف + effects + analytics | ~20 FPS      | 120+ FPS
```

---

## 📋 المهام التنفيذية

### المرحلة 1: التحسينات الفورية (بدون تغيير معماري)

#### 1.1 Object Pooling
```javascript
// بدلاً من إنشاء وحذف الجسيمات كل frame:
class ObjectPool {
  constructor(factory, reset, initialSize = 100) {
    this._factory = factory;
    this._reset = reset;
    this._pool = Array.from({ length: initialSize }, () => factory());
    this._active = [];
  }
  
  acquire() {
    const obj = this._pool.pop() || this._factory();
    this._active.push(obj);
    return obj;
  }
  
  release(obj) {
    const idx = this._active.indexOf(obj);
    if (idx !== -1) {
      this._active.splice(idx, 1);
      this._reset(obj);
      this._pool.push(obj);
    }
  }
}
```
- **التأثير المتوقع:** تقليل GC pauses بنسبة 80%+
- **الملفات:** `script.js` → `ParticleSystem`, `HitMarker`

#### 1.2 Spatial Hashing لـ Hit Detection
```javascript
// بدلاً من O(n) loop على كل الأهداف:
class SpatialHash {
  constructor(cellSize = 100) {
    this.cellSize = cellSize;
    this.grid = new Map();
  }
  
  _key(x, y) {
    return `${Math.floor(x / this.cellSize)},${Math.floor(y / this.cellSize)}`;
  }
  
  insert(target) {
    const key = this._key(target.x, target.y);
    if (!this.grid.has(key)) this.grid.set(key, []);
    this.grid.get(key).push(target);
  }
  
  query(x, y, radius) {
    const results = [];
    const minCX = Math.floor((x - radius) / this.cellSize);
    const maxCX = Math.floor((x + radius) / this.cellSize);
    const minCY = Math.floor((y - radius) / this.cellSize);
    const maxCY = Math.floor((y + radius) / this.cellSize);
    
    for (let cx = minCX; cx <= maxCX; cx++) {
      for (let cy = minCY; cy <= maxCY; cy++) {
        const cell = this.grid.get(`${cx},${cy}`);
        if (cell) results.push(...cell);
      }
    }
    return results;
  }
}
```
- **التأثير المتوقع:** hit detection من O(n) → O(1) في معظم الحالات

#### 1.3 Dirty Rect Rendering
- تتبع المناطق المتغيرة فقط (targets moved, particles active)
- إعادة رسم المنطقة المتغيرة + buffer حولها فقط
- Background يُرسم مرة واحدة في off-screen canvas

#### 1.4 Frame Budget Monitor
```javascript
_loop(timestamp) {
  const frameBudget = 1000 / 144; // ~6.9ms for 144 FPS
  const start = performance.now();
  
  this._update(dt);
  this._draw();
  
  const frameTime = performance.now() - start;
  this._frameTimeHistory.push(frameTime);
  
  // Auto-adjust quality if frame budget exceeded
  if (frameTime > frameBudget * 1.2) {
    this._reduceQuality(); // Disable particles, reduce glow, etc.
  }
}
```

---

### المرحلة 2: OffscreenCanvas + Web Worker

#### 2.1 نقل Physics إلى Worker Thread
```
Main Thread          Worker Thread
─────────────       ──────────────
Input Events    →   Physics Update
                    Target Movement
                    Collision Detection
                ←   Updated Positions
Render (Canvas)
```

- فصل الفيزياء عن الرسم = لا تأخير في الإدخال
- Input latency يبقى في الـ Main Thread
- الرسم يحصل على بيانات محدثة كل frame

#### 2.2 Double Buffering
- Off-screen canvas للخلفية الثابتة (grid, vignette, borders)
- Main canvas للعناصر المتحركة فقط
- تقليل draw calls بنسبة 40%+

---

### المرحلة 3: WebGL Migration (مستقبلي)

#### 3.1 لماذا WebGL؟
- Hardware-accelerated rendering
- GPU-based particle systems (thousands of particles)
- Shader-based effects (glow, bloom, motion blur)
- Instanced drawing لعشرات الأهداف

#### 3.2 المكتبة المقترحة
- **PixiJS v8** — lightweight 2D WebGL renderer
- يدعم Canvas fallback تلقائياً
- Particle container يرسم 10,000 particle بسهولة

---

## 📊 مؤشرات النجاح

| المؤشر | القيمة المطلوبة |
|--------|----------------|
| Frame Time (avg) | < 6.9ms (144 FPS) |
| Frame Time (P99) | < 11ms |
| GC Pauses | < 2ms per minute |
| Input Latency | < 1 frame |
| Memory Usage | < 50MB |
| First Paint | < 500ms |

---

## ⚠️ مخاطر وحلول

| المخاطر | الحل |
|---------|------|
| WebGL غير مدعوم | Canvas 2D fallback تلقائي |
| Worker communication overhead | SharedArrayBuffer + Atomics |
| Memory leaks من Object Pooling | WeakRef + periodic cleanup |
| Frame drops أثناء tab switch | `document.hidden` → pause loop |
