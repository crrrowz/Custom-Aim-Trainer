# 🎯 الخطة 6: نظام السيناريوهات الذكي والتدريبات المتقدمة

> **الهدف:** مكتبة تدريبات متخصصة تغطي كل مهارة في VALORANT مع قابلية تخصيص لا محدودة.

---

## 📚 مكتبة السيناريوهات المدمجة (Built-in Scenarios)

### الفئة 1: Flick Training
```
┌─────────────────────────────────────────┐
│  📁 Flick Training                      │
│  ├── 🎯 Classic Flick (90°)            │
│  ├── ⚡ Speed Flick (small targets)     │
│  ├── 🔄 360° Flick (all directions)    │
│  ├── 📐 Angle Flick (45°/90°/135°)    │
│  └── 🎪 Random Distance Flick          │
└─────────────────────────────────────────┘
```

### الفئة 2: Tracking
```
┌─────────────────────────────────────────┐
│  📁 Tracking Training                   │
│  ├── ➡️ Horizontal Track (smooth)      │
│  ├── ↕️ Vertical Track                 │
│  ├── 🔀 Random Direction Track         │
│  ├── 🌀 Circular Track                 │
│  ├── ⚡ Speed Variation Track          │
│  └── 🎯 Precision Track (small target) │
└─────────────────────────────────────────┘
```

### الفئة 3: VALORANT-Specific
```
┌─────────────────────────────────────────┐
│  📁 VALORANT Drills                     │
│  ├── 🛑 Counter-Strafe Master          │
│  ├── 👀 Peek & Fire                    │
│  ├── 🏃 Jiggle Peek Drill             │
│  ├── 🎯 Headshot Line Practice         │
│  ├── 🔫 Spray Transfer                 │
│  └── 🏥 Post-Plant Crosshair           │
└─────────────────────────────────────────┘
```

### الفئة 4: Warmup Routines
```
┌─────────────────────────────────────────┐
│  📁 Warmup Routines                     │
│  ├── ☀️ Quick Warmup (5 min)           │
│  │   └── 3 scenarios × 100s each       │
│  ├── 🔥 Full Warmup (15 min)           │
│  │   └── 6 scenarios × 150s each       │
│  └── 💪 Intensive (30 min)             │
│      └── 10 scenarios × 180s each      │
└─────────────────────────────────────────┘
```

---

## 🔧 Scenario Definition Schema

```javascript
const SCENARIO_SCHEMA = {
  name: 'Counter-Strafe Master',
  description: 'تدرب على إيقاف الحركة والإطلاق بدقة',
  category: 'valorant',
  difficulty: 'intermediate', // beginner, intermediate, advanced, expert
  icon: '🛑',
  
  // إعدادات الجلسة
  session: {
    mode: 'rounds',        // 'rounds' | 'timer' | 'infinite'
    rounds: 30,
    timer: 0,
  },
  
  // إعدادات الأهداف
  targets: [
    {
      x: -200, y: 0,
      radius: 20,
      color: '#ff4655',
      health: 1,
      vx: 0, vy: 0,
      lifespan: 2.5,
    },
    {
      x: 200, y: 0,
      radius: 20,
      color: '#ff4655',
      health: 1,
      vx: 0, vy: 0,
      lifespan: 2.5,
    },
  ],
  
  // إعدادات اللعب
  gameplay: {
    respawnTargets: true,
    respawnDelay: 0.5,
    randomRespawn: true,
    randomArea: 800,
    targetLifespan: 2.5,
  },
  
  // إعدادات الحركة (تُطبّق مؤقتاً)
  movement: {
    counterStrafeEnabled: true,
    acceleration: 2400,
    maxSpeed: 600,
  },
  
  // معايير النجاح
  goals: {
    accuracy: 75,          // الحد الأدنى %
    headshitRate: 20,      // الحد الأدنى %
    avgTTK: 600,           // الحد الأقصى ms
  },
};
```

---

## 🏋️ Playlist System (قوائم التشغيل)

```javascript
class PlaylistManager {
  constructor() {
    this.playlists = Storage.get('aimforge_playlists', {});
  }
  
  // Warmup routine = playlist of scenarios played in sequence
  createPlaylist(name, scenarios) {
    this.playlists[name] = {
      name,
      scenarios: scenarios.map(s => ({
        scenarioName: s.name,
        duration: s.duration || 120, // seconds
        restBetween: s.rest || 10,   // seconds rest
      })),
      totalDuration: scenarios.reduce((sum, s) => sum + (s.duration || 120), 0),
    };
    this.save();
  }
  
  async runPlaylist(name, app) {
    const playlist = this.playlists[name];
    if (!playlist) return;
    
    for (let i = 0; i < playlist.scenarios.length; i++) {
      const scenario = playlist.scenarios[i];
      
      // Show progress: "Scenario 2/6: Counter-Strafe Master"
      app._showPlaylistProgress(i + 1, playlist.scenarios.length, scenario.scenarioName);
      
      // Load and run scenario
      app._loadScenario(scenario.scenarioName);
      app.settings.sessionTimer = scenario.duration;
      app._startTraining();
      
      // Wait for session to end
      await app._waitForSessionEnd();
      
      // Rest period (if not last)
      if (i < playlist.scenarios.length - 1) {
        await app._showRestTimer(scenario.restBetween);
      }
    }
    
    // Show combined results
    app._showPlaylistResults(playlist);
  }
}
```

---

## 🤖 AI Scenario Suggestions

```javascript
class ScenarioSuggester {
  suggest(playerProfile) {
    const suggestions = [];
    
    // Based on weaknesses
    if (playerProfile.flickAccuracy < 0.6) {
      suggestions.push({
        scenario: 'Classic Flick',
        reason: 'دقة الفليك منخفضة',
        priority: 'high',
      });
    }
    
    if (playerProfile.overshootRate > 0.4) {
      suggestions.push({
        scenario: 'Precision Micro-Adjust',
        reason: 'معدل تجاوز الهدف مرتفع',
        priority: 'critical',
      });
    }
    
    if (playerProfile.csGrade === 'D' || playerProfile.csGrade === 'F') {
      suggestions.push({
        scenario: 'Counter-Strafe Master',
        reason: 'Counter-Strafe ضعيف',
        priority: 'critical',
      });
    }
    
    // Daily variety (avoid staleness)
    const recentScenarios = playerProfile.last7Days.map(s => s.name);
    const underplayed = ALL_SCENARIOS.filter(s => !recentScenarios.includes(s.name));
    if (underplayed.length > 0) {
      suggestions.push({
        scenario: underplayed[0].name,
        reason: 'لم تلعب هذا السيناريو مؤخراً',
        priority: 'low',
      });
    }
    
    return suggestions.sort((a, b) => PRIORITY_ORDER[a.priority] - PRIORITY_ORDER[b.priority]);
  }
}
```
