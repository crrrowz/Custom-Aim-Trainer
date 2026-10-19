# 🏆 الخطة 7: النظام التنافسي والتحديات

> **الهدف:** إضافة طبقة تنافسية تحفّز اللاعب على التحسن المستمر عبر تحديات يومية، سلالم ترتيب، وإنجازات.

---

## 📊 History & Progress Tracking

### 7.1 Session History Database
```javascript
class SessionHistory {
  constructor() {
    this.sessions = Storage.get('aimforge_history', []);
    this.MAX_SESSIONS = 500; // Keep last 500 sessions
  }

  record(sessionData) {
    this.sessions.push({
      id: uid(),
      date: new Date().toISOString(),
      scenario: sessionData.scenarioName,
      duration: sessionData.duration,
      
      // Core metrics
      hits: sessionData.hits,
      misses: sessionData.misses,
      headshots: sessionData.headshots,
      kills: sessionData.kills,
      accuracy: sessionData.accuracy,
      avgTTK: sessionData.avgTTK,
      pathEfficiency: sessionData.pathEff,
      score: sessionData.score,
      rank: sessionData.rank,
      
      // Advanced metrics
      overshootRate: sessionData.overshootRate,
      csGrade: sessionData.csGrade,
      consistency: sessionData.consistency,
      fatigueIndex: sessionData.fatigueIndex,
    });
    
    // Trim old sessions
    if (this.sessions.length > this.MAX_SESSIONS) {
      this.sessions = this.sessions.slice(-this.MAX_SESSIONS);
    }
    
    this.save();
  }

  // Get progress over time
  getProgressChart(metric, days = 30) {
    const cutoff = Date.now() - (days * 24 * 60 * 60 * 1000);
    const recent = this.sessions.filter(s => new Date(s.date).getTime() > cutoff);
    
    // Group by day
    const byDay = {};
    for (const s of recent) {
      const day = s.date.split('T')[0];
      if (!byDay[day]) byDay[day] = [];
      byDay[day].push(s[metric]);
    }
    
    return Object.entries(byDay).map(([day, values]) => ({
      date: day,
      avg: avg(values),
      best: Math.max(...values),
      sessions: values.length,
    }));
  }
}
```

### 7.2 Progress Visualization
```
┌─────────────────────────────────────────────────┐
│  📈 ACCURACY TREND (Last 30 Days)               │
│                                                  │
│  95% ┤                              ╭─╮         │
│  90% ┤                         ╭────╯ │         │
│  85% ┤                    ╭────╯      │         │
│  80% ┤               ╭────╯           │         │
│  75% ┤          ╭────╯                │         │
│  70% ┤     ╭────╯                     │         │
│  65% ┤─────╯                          │         │
│      └──────┬──────┬──────┬──────┬────┘         │
│           Week 1  Week 2  Week 3  Week 4        │
│                                                  │
│  📊 Summary:                                    │
│  • Started at 65% → Now at 92% (+27%)           │
│  • Best session: 96.2% (Jan 14)                 │
│  • Worst day: Tuesday (avg 78%)                 │
│  • Best time: 10pm-12am (avg 91%)               │
└─────────────────────────────────────────────────┘
```

---

## 🏅 Achievement System (الإنجازات)

```javascript
const ACHIEVEMENTS = {
  // Accuracy
  sharpshooter:    { name: 'القناص', icon: '🎯', desc: 'حقق 95%+ دقة في جلسة واحدة', condition: s => s.accuracy >= 95 },
  perfectSession:  { name: 'الكمال', icon: '💯', desc: 'أنهِ جلسة بدون أي خطأ', condition: s => s.misses === 0 && s.hits >= 10 },
  
  // Headshots
  headhunter:      { name: 'صياد الرؤوس', icon: '💀', desc: 'حقق 50%+ معدل هيدشوت', condition: s => s.headshots / s.hits >= 0.5 },
  
  // Speed
  flashReflexes:   { name: 'ردة فعل البرق', icon: '⚡', desc: 'متوسط TTK أقل من 200ms', condition: s => s.avgTTK < 200 },
  speedDemon:      { name: 'شيطان السرعة', icon: '🏎️', desc: 'اقتل 50 هدف في 60 ثانية', condition: s => s.kills >= 50 && s.duration <= 60 },
  
  // Consistency
  ironWill:        { name: 'الإرادة الحديدية', icon: '🗡️', desc: '10 جلسات متتالية بدقة 80%+', condition: (s, history) => last(history, 10).every(h => h.accuracy >= 80) },
  marathon:        { name: 'الماراثون', icon: '🏃', desc: 'تدرب 7 أيام متتالية', condition: (s, history) => consecutiveDays(history) >= 7 },
  
  // Improvement
  breakthrough:    { name: 'الاختراق', icon: '🚀', desc: 'تحسّن بنسبة 20% في أسبوع', condition: (s, history) => weeklyImprovement(history) >= 20 },
  
  // Rank
  goldRush:        { name: 'حمى الذهب', icon: '🏆', desc: 'وصل لرتبة Gold', condition: s => s.score >= 4000 },
  diamondHands:    { name: 'يد الماس', icon: '💎', desc: 'وصل لرتبة Diamond', condition: s => s.score >= 8000 },
  radiantAscension:{ name: 'صعود الراديانت', icon: '✨', desc: 'وصل لرتبة Radiant', condition: s => s.score >= 20000 },
  
  // Fun
  sprayNPray:      { name: 'رش وصلّي', icon: '🔫', desc: 'أطلق 100 طلقة في جلسة واحدة', condition: s => s.hits + s.misses >= 100 },
  noScope:         { name: 'بدون سكوب', icon: '🔭', desc: 'اقتل 10 أهداف بحساسية 0.15', condition: s => s.kills >= 10 && s.sensitivity <= 0.15 },
};
```

---

## 📅 Daily Challenges (التحديات اليومية)

```javascript
class DailyChallenge {
  constructor() {
    this.today = new Date().toISOString().split('T')[0];
    this.challenges = this._generateForDate(this.today);
  }
  
  _generateForDate(dateStr) {
    // Deterministic based on date (same for all users)
    const seed = hashCode(dateStr);
    const rng = seededRandom(seed);
    
    return [
      {
        name: 'تحدي الدقة',
        icon: '🎯',
        description: `حقق ${70 + Math.floor(rng() * 25)}% دقة في 30 طلقة`,
        reward: 500,
        type: 'accuracy',
        target: 70 + Math.floor(rng() * 25),
        roundCount: 30,
      },
      {
        name: 'تحدي السرعة',
        icon: '⚡',
        description: `اقتل ${10 + Math.floor(rng() * 15)} هدف في ${30 + Math.floor(rng() * 30)} ثانية`,
        reward: 750,
        type: 'speed',
        kills: 10 + Math.floor(rng() * 15),
        timeLimit: 30 + Math.floor(rng() * 30),
      },
      {
        name: 'تحدي الهيدشوت',
        icon: '💀',
        description: `سجّل ${3 + Math.floor(rng() * 7)} هيدشوت متتالية`,
        reward: 1000,
        type: 'headshots',
        streak: 3 + Math.floor(rng() * 7),
      },
    ];
  }
}
```

---

## 📊 Personal Best Tracking

```javascript
class PersonalBests {
  constructor() {
    this.records = Storage.get('aimforge_records', {});
  }

  check(scenario, metric, value) {
    const key = `${scenario}:${metric}`;
    const prev = this.records[key];
    
    if (!prev || this._isBetter(metric, value, prev.value)) {
      this.records[key] = {
        value,
        date: new Date().toISOString(),
        previous: prev ? prev.value : null,
      };
      this.save();
      
      return {
        isNewRecord: true,
        improvement: prev ? this._calcImprovement(metric, value, prev.value) : null,
      };
    }
    
    return { isNewRecord: false };
  }

  _isBetter(metric, newVal, oldVal) {
    // Lower is better for TTK, higher is better for accuracy
    if (metric === 'avgTTK') return newVal < oldVal;
    return newVal > oldVal;
  }
}

// Usage:
// const result = personalBests.check('Classic Flick', 'accuracy', 94.5);
// if (result.isNewRecord) Toast.show('🏅 NEW PERSONAL BEST! 94.5% Accuracy');
```

---

## 🎮 Skill Rating (SR) System

```
SR is a persistent rating that updates after every session.

SR Calculation:
───────────────
newSR = currentSR + (K × (performance - expected))

K = 32 (learning phase, first 20 sessions)
K = 16 (calibrated phase)

performance = normalized score (0-1)
expected = predicted performance based on current SR

SR Ranges:
───────────
0-500:    Iron
500-1000: Bronze
1000-1500: Silver
1500-2000: Gold
2000-2500: Platinum
2500-3000: Diamond
3000-3500: Ascendant
3500-4000: Immortal
4000+:    Radiant
```

---

## 🗂️ Data Export

```javascript
class DataExporter {
  exportCSV(sessions) {
    const headers = ['Date', 'Scenario', 'Duration', 'Hits', 'Misses', 
                     'Headshots', 'Accuracy', 'AvgTTK', 'PathEff', 'Score', 'Rank'];
    const rows = sessions.map(s => [
      s.date, s.scenario, s.duration, s.hits, s.misses,
      s.headshots, s.accuracy, s.avgTTK, s.pathEfficiency, s.score, s.rank
    ]);
    
    const csv = [headers, ...rows].map(r => r.join(',')).join('\n');
    this._download(csv, 'aimforge_history.csv', 'text/csv');
  }

  exportJSON(sessions) {
    const json = JSON.stringify({ 
      exportDate: new Date().toISOString(),
      version: '1.0',
      sessions 
    }, null, 2);
    this._download(json, 'aimforge_history.json', 'application/json');
  }
}
```
