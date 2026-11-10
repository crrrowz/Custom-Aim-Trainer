import { useMemo, useState, type ReactNode } from "react";
import type { Flick, Skill, TrackBin } from "./api";

export const COLORS = { hit: "#3ddc97", miss: "#ff6b6b", ink: "#e8edf3", muted: "#8a97a8", grid: "#232b36", line: "#5aa9ff", warn: "#e5ad45" };
export const SERIES = ["#3ddc97", "#5aa9ff", "#e5ad45", "#c58bff"];

/** Hover readout shared by the charts. Text stays in ink colors; marks carry the color. */
function useTip() {
  const [tip, setTip] = useState<{ x: number; y: number; body: ReactNode } | null>(null);
  const show = (e: React.MouseEvent, body: ReactNode) => {
    const box = (e.currentTarget as Element).closest(".chart")!.getBoundingClientRect();
    setTip({ x: e.clientX - box.left, y: e.clientY - box.top, body });
  };
  const node = tip && (
    <div className="tip" style={{ left: tip.x + 12, top: tip.y + 12 }}>
      {tip.body}
    </div>
  );
  return { show, hide: () => setTip(null), node };
}

const niceMax = (v: number) => {
  const p = 10 ** Math.floor(Math.log10(Math.max(v, 1e-6)));
  const n = v / p;
  return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * p;
};

export function AimPath({ flick, progressMs, labels }: {
  flick: Flick;
  progressMs: number;
  labels: { onset: string; primary: string; shot: string };
}) {
  const W = 620, H = 400, PAD = 26;
  const { scale, cx, cy } = useMemo(() => {
    const xs = flick.path.map((p) => p.x).concat(-flick.radius_deg, flick.radius_deg);
    const ys = flick.path.map((p) => p.y).concat(-flick.radius_deg, flick.radius_deg);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const scale = Math.min((W - 2 * PAD) / Math.max(x1 - x0, 1), (H - 2 * PAD) / Math.max(y1 - y0, 1));
    return { scale, cx: (x0 + x1) / 2, cy: (y0 + y1) / 2 };
  }, [flick]);
  const px = (x: number) => W / 2 + (x - cx) * scale;
  const py = (y: number) => H / 2 - (y - cy) * scale;

  const shown = flick.path.filter((p) => p.t_ms <= progressMs);
  const line = (pts: Flick["path"]) => pts.map((p) => `${px(p.x).toFixed(1)},${py(p.y).toFixed(1)}`).join(" ");
  const at = (ms: number | null) => {
    if (ms === null) return null;
    return flick.path.reduce((a, b) => (Math.abs(b.t_ms - ms) < Math.abs(a.t_ms - ms) ? b : a));
  };
  const markers: [number | null, string, string][] = [
    [flick.onset_ms, labels.onset, COLORS.line],
    [flick.primary_end_ms, labels.primary, COLORS.warn],
    [flick.shot_ms, labels.shot, flick.hit ? COLORS.hit : COLORS.miss],
  ];
  const head = shown[shown.length - 1] ?? flick.path[0];

  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label="aim path">
      <line x1={px(0)} x2={px(0)} y1={0} y2={H} stroke={COLORS.grid} />
      <line x1={0} x2={W} y1={py(0)} y2={py(0)} stroke={COLORS.grid} />
      <circle cx={px(0)} cy={py(0)} r={Math.max(flick.radius_deg * scale, 3)} fill={COLORS.hit} fillOpacity={0.14} stroke={COLORS.hit} strokeWidth={1.5} />
      <polyline points={line(flick.path)} fill="none" stroke={COLORS.grid} strokeWidth={2} strokeLinejoin="round" />
      <polyline points={line(shown)} fill="none" stroke={COLORS.ink} strokeWidth={2} strokeLinejoin="round" />
      {markers.map(([ms, name, color]) => {
        const p = at(ms);
        if (!p || (ms ?? 0) > progressMs) return null;
        return (
          <g key={name}>
            <circle cx={px(p.x)} cy={py(p.y)} r={5} fill={color} stroke="#12161b" strokeWidth={2} />
            <text x={px(p.x)} y={py(p.y) - 11} textAnchor="middle" fill={COLORS.muted} fontSize={12}>
              {name}
            </text>
          </g>
        );
      })}
      <g transform={`translate(${px(head.x)},${py(head.y)})`} stroke={COLORS.ink} strokeWidth={1.5}>
        <line x1={-8} x2={-3} /><line x1={3} x2={8} /><line y1={-8} y2={-3} /><line y1={3} y2={8} />
      </g>
    </svg>
  );
}

export function SpeedChart({ flick, progressMs, labels, onScrub }: {
  flick: Flick;
  progressMs: number;
  labels: { y: string; onset: string; primary: string; shot: string };
  onScrub: (ms: number) => void;
}) {
  const W = 620, H = 170, L = 42, B = 22, T = 16;
  const maxT = flick.path[flick.path.length - 1].t_ms || 1;
  const maxV = niceMax(Math.max(...flick.path.map((p) => p.speed), 30));
  const x = (t: number) => L + (t / maxT) * (W - L - 8);
  const y = (v: number) => H - B - (v / maxV) * (H - B - T);
  const d = flick.path.map((p) => `${x(p.t_ms).toFixed(1)},${y(p.speed).toFixed(1)}`).join(" ");
  const marks: [number | null, string, string][] = [
    [flick.onset_ms, labels.onset, COLORS.line],
    [flick.primary_end_ms, labels.primary, COLORS.warn],
    [flick.shot_ms, labels.shot, flick.hit ? COLORS.hit : COLORS.miss],
  ];
  const scrub = (e: React.MouseEvent<SVGSVGElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    const px = ((e.clientX - box.left) / box.width) * W;
    onScrub(Math.max(0, Math.min(maxT, ((px - L) / (W - L - 8)) * maxT)));
  };
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" onMouseMove={(e) => e.buttons === 1 && scrub(e)} onMouseDown={scrub} role="img" aria-label={labels.y}>
      {[0, 0.5, 1].map((f) => (
        <g key={f}>
          <line x1={L} x2={W - 8} y1={y(maxV * f)} y2={y(maxV * f)} stroke={COLORS.grid} />
          <text x={L - 6} y={y(maxV * f) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(maxV * f)}</text>
        </g>
      ))}
      <text x={W - 8} y={H - 5} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(maxT)} ms</text>
      <text x={L} y={11} fill={COLORS.muted} fontSize={11}>{labels.y}</text>
      <polyline points={d} fill="none" stroke={COLORS.line} strokeWidth={2} strokeLinejoin="round" />
      {marks.map(([ms, name, color]) =>
        ms === null ? null : (
          <g key={name}>
            <line x1={x(ms)} x2={x(ms)} y1={T} y2={H - B} stroke={color} strokeDasharray="3 3" />
            <text x={x(ms) + 3} y={T + 9} fill={COLORS.muted} fontSize={11}>{name}</text>
          </g>
        ),
      )}
      <line x1={x(Math.min(progressMs, maxT))} x2={x(Math.min(progressMs, maxT))} y1={T} y2={H - B} stroke={COLORS.ink} />
    </svg>
  );
}

export function ShotScatter({ flicks, hitLabel, missLabel }: { flicks: Flick[]; hitLabel: string; missLabel: string }) {
  const tip = useTip();
  const S = 360, C = S / 2;
  const shots = flicks.filter((f) => f.endpoint);
  // Plotted in target radii so different target sizes share one map.
  const pts = shots.map((f) => ({ f, x: f.endpoint![0] / f.radius_deg, y: f.endpoint![1] / f.radius_deg }));
  const reach = Math.min(Math.max(2, ...pts.map((p) => Math.max(Math.abs(p.x), Math.abs(p.y)))) * 1.1, 8);
  const k = (C - 12) / reach;
  const mean = pts.length ? [pts.reduce((a, p) => a + p.x, 0) / pts.length, pts.reduce((a, p) => a + p.y, 0) / pts.length] : [0, 0];
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${S} ${S}`} className="plot square" direction="ltr" role="img" aria-label="shot map">
        <line x1={C} x2={C} y1={0} y2={S} stroke={COLORS.grid} />
        <line x1={0} x2={S} y1={C} y2={C} stroke={COLORS.grid} />
        {reach >= 2 && <circle cx={C} cy={C} r={2 * k} fill="none" stroke={COLORS.grid} strokeDasharray="4 4" />}
        <circle cx={C} cy={C} r={k} fill={COLORS.hit} fillOpacity={0.1} stroke={COLORS.hit} />
        {pts.map(({ f, x, y }) => {
          const off = Math.max(Math.abs(x), Math.abs(y)) > reach;
          if (off) return null;
          return (
            <circle key={f.id} cx={C + x * k} cy={C - y * k} r={4.5} fill={f.hit ? COLORS.hit : COLORS.miss} stroke="#12161b" strokeWidth={1.5}
              onMouseMove={(e) => tip.show(e, `${f.hit ? hitLabel : missLabel} · ${Math.hypot(f.endpoint![0], f.endpoint![1]).toFixed(2)}°`)} onMouseLeave={tip.hide} />
          );
        })}
        <g transform={`translate(${C + mean[0] * k},${C - mean[1] * k})`} stroke={COLORS.ink} strokeWidth={2}>
          <line x1={-7} x2={7} /><line y1={-7} y2={7} />
        </g>
      </svg>
      {tip.node}
    </div>
  );
}

export function ReactionTimeline({ flicks, yLabel, hitLabel, missLabel }: { flicks: Flick[]; yLabel: string; hitLabel: string; missLabel: string }) {
  const tip = useTip();
  const W = 920, H = 240, L = 46, B = 24, T = 16;
  const rows = flicks.filter((f) => f.time_to_hit_ms !== null || f.reaction_ms !== null);
  const value = (f: Flick) => f.time_to_hit_ms ?? f.shot_ms ?? f.reaction_ms ?? 0;
  const maxT = Math.max(...rows.map((f) => f.t_s), 1);
  const maxV = niceMax(Math.max(...rows.map(value), 100));
  const x = (t: number) => L + (t / maxT) * (W - L - 10);
  const y = (v: number) => H - B - (v / maxV) * (H - B - T);
  // Rolling median of 9 trials shows the trend without hiding single trials.
  const trend = rows.map((_, i) => {
    const w = rows.slice(Math.max(0, i - 4), i + 5).map(value).sort((a, b) => a - b);
    return w[Math.floor(w.length / 2)];
  });
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label={yLabel}>
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line x1={L} x2={W - 10} y1={y(maxV * f)} y2={y(maxV * f)} stroke={COLORS.grid} />
            <text x={L - 6} y={y(maxV * f) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(maxV * f)}</text>
          </g>
        ))}
        <text x={L} y={11} fill={COLORS.muted} fontSize={11}>{yLabel}</text>
        <text x={W - 10} y={H - 6} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(maxT)} s</text>
        {rows.map((f) => (
          <circle key={f.id} cx={x(f.t_s)} cy={y(value(f))} r={4} fill={f.hit ? COLORS.hit : COLORS.miss} fillOpacity={0.85}
            onMouseMove={(e) => tip.show(e, `${f.t_s.toFixed(1)} s · ${Math.round(value(f))} ms · ${f.hit ? hitLabel : missLabel}`)} onMouseLeave={tip.hide} />
        ))}
        <polyline points={rows.map((f, i) => `${x(f.t_s).toFixed(1)},${y(trend[i]).toFixed(1)}`).join(" ")} fill="none" stroke={COLORS.ink} strokeWidth={2} strokeLinejoin="round" />
      </svg>
      {tip.node}
    </div>
  );
}

export function TrackingChart({ bins, radius, yLabel }: { bins: TrackBin[]; radius: number; yLabel: string }) {
  const W = 920, H = 240, L = 46, B = 24, T = 16;
  const maxT = bins[bins.length - 1]?.t_s || 1;
  const maxV = niceMax(Math.max(...bins.map((b) => b.error_deg), radius));
  const x = (t: number) => L + (t / maxT) * (W - L - 10);
  const y = (v: number) => H - B - (v / maxV) * (H - B - T);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label={yLabel}>
      <rect x={L} y={y(radius)} width={W - L - 10} height={H - B - y(radius)} fill={COLORS.hit} fillOpacity={0.1} />
      {[0, 0.5, 1].map((f) => (
        <g key={f}>
          <line x1={L} x2={W - 10} y1={y(maxV * f)} y2={y(maxV * f)} stroke={COLORS.grid} />
          <text x={L - 6} y={y(maxV * f) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{(maxV * f).toFixed(1)}</text>
        </g>
      ))}
      <text x={L} y={11} fill={COLORS.muted} fontSize={11}>{yLabel}</text>
      <text x={W - 10} y={H - 6} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(maxT)} s</text>
      <polyline points={bins.map((b) => `${x(b.t_s).toFixed(1)},${y(b.error_deg).toFixed(1)}`).join(" ")} fill="none" stroke={COLORS.line} strokeWidth={2} strokeLinejoin="round" />
    </svg>
  );
}

export function SkillRadar({ skills, name }: { skills: Skill[]; name: (id: string) => string }) {
  const S = 400, C = S / 2, R = C - 92;
  if (skills.length < 3) return null;
  const pt = (i: number, r: number) => {
    const a = -Math.PI / 2 + (i * 2 * Math.PI) / skills.length;
    return [C + r * Math.cos(a), C + r * Math.sin(a)];
  };
  const ring = (f: number) => skills.map((_, i) => pt(i, R * f).map((n) => n.toFixed(1)).join(",")).join(" ");
  return (
    <svg viewBox={`0 0 ${S} ${S}`} className="plot square" role="img" aria-label="skills">
      {[0.33, 0.66, 1].map((f) => <polygon key={f} points={ring(f)} fill="none" stroke={COLORS.grid} />)}
      <polygon points={skills.map((s, i) => pt(i, (R * s.score) / 100).map((n) => n.toFixed(1)).join(",")).join(" ")} fill={COLORS.hit} fillOpacity={0.18} stroke={COLORS.hit} strokeWidth={2} strokeLinejoin="round" />
      {skills.map((s, i) => {
        const [x, y] = pt(i, R + 16);
        return (
          <text key={s.skill} x={x} y={y + 4} textAnchor={Math.abs(x - C) < 12 ? "middle" : x > C ? "start" : "end"} fill={COLORS.muted} fontSize={13}>
            {name(s.skill)}
          </text>
        );
      })}
    </svg>
  );
}

export interface Series { id: string; name: string; points: { label: string; v: number; lo: number; hi: number }[] }

export function SkillLines({ series }: { series: Series[] }) {
  const tip = useTip();
  const W = 920, H = 300, L = 40, B = 28, T = 14;
  const n = Math.max(...series.map((s) => s.points.length), 1);
  const x = (i: number) => (n === 1 ? (L + W) / 2 : L + (i / (n - 1)) * (W - L - 16));
  const y = (v: number) => H - B - (v / 100) * (H - B - T);
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label="progress">
        {[0, 25, 50, 75, 100].map((v) => (
          <g key={v}>
            <line x1={L} x2={W - 16} y1={y(v)} y2={y(v)} stroke={COLORS.grid} />
            <text x={L - 6} y={y(v) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{v}</text>
          </g>
        ))}
        {series[0]?.points.map((p, i) => (
          <text key={i} x={x(i)} y={H - 8} textAnchor="middle" fill={COLORS.muted} fontSize={11}>{p.label}</text>
        ))}
        {series.map((s, si) => {
          const color = SERIES[si % SERIES.length];
          const band = [...s.points.map((p, i) => `${x(i)},${y(p.hi)}`), ...[...s.points].reverse().map((p, i) => `${x(s.points.length - 1 - i)},${y(p.lo)}`)].join(" ");
          return (
            <g key={s.id}>
              {s.points.length > 1 && <polygon points={band} fill={color} fillOpacity={0.1} />}
              <polyline points={s.points.map((p, i) => `${x(i)},${y(p.v)}`).join(" ")} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" />
              {s.points.map((p, i) => (
                <circle key={i} cx={x(i)} cy={y(p.v)} r={5} fill={color} stroke="#12161b" strokeWidth={2}
                  onMouseMove={(e) => tip.show(e, `${s.name}: ${p.v.toFixed(0)} (${p.lo.toFixed(0)}–${p.hi.toFixed(0)})`)} onMouseLeave={tip.hide} />
              ))}
            </g>
          );
        })}
      </svg>
      {tip.node}
    </div>
  );
}

export function Spark({ values }: { values: number[] }) {
  if (values.length < 2) return null;
  const W = 96, H = 26;
  const [a, b] = [Math.min(...values), Math.max(...values)];
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * W},${H - 3 - ((v - a) / Math.max(b - a, 1e-6)) * (H - 6)}`).join(" ");
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H} direction="ltr" aria-hidden="true">
      <polyline points={pts} fill="none" stroke={COLORS.hit} strokeWidth={1.75} strokeLinejoin="round" />
    </svg>
  );
}

export function ValueBars({ rows, max, unit }: { rows: { label: string; value: number; tip?: string }[]; max?: number; unit: string }) {
  const tip = useTip();
  const W = 920, H = 200, L = 40, B = 24, T = 14;
  const top = max ?? niceMax(Math.max(...rows.map((r) => r.value), 1));
  const y = (v: number) => H - B - (v / top) * (H - B - T);
  const slot = (W - L - 10) / Math.max(rows.length, 1);
  const bar = Math.min(slot * 0.6, 54);
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label={unit}>
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line x1={L} x2={W - 10} y1={y(top * f)} y2={y(top * f)} stroke={COLORS.grid} />
            <text x={L - 6} y={y(top * f) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(top * f)}</text>
          </g>
        ))}
        {rows.map((r, i) => {
          const cx = L + slot * (i + 0.5);
          return (
            <g key={r.label}>
              <rect x={cx - bar / 2} y={y(r.value)} width={bar} height={Math.max(H - B - y(r.value), 2)} rx={4} fill={COLORS.hit}
                onMouseMove={(e) => tip.show(e, r.tip ?? `${r.value} ${unit}`)} onMouseLeave={tip.hide} />
              <text x={cx} y={H - 7} textAnchor="middle" fill={COLORS.muted} fontSize={11}>{r.label}</text>
            </g>
          );
        })}
      </svg>
      {tip.node}
    </div>
  );
}

type TrendPoint = { label: string; value: number; tip?: string; tick?: boolean };

/** Axis labels: the points marked as ticks, or about eight evenly spaced ones. */
const showTick = (points: TrendPoint[], i: number) => {
  if (points.some((p) => p.tick !== undefined)) return !!points[i].tick;
  const every = Math.ceil(points.length / 8);
  return i % every === 0 || i === points.length - 1;
};

export function TrendLine({ points, unit, better }: { points: TrendPoint[]; unit: string; better: "up" | "down" }) {
  const tip = useTip();
  const W = 920, H = 240, L = 46, B = 26, T = 16;
  if (points.length === 0) return null;
  const values = points.map((p) => p.value);
  const pad = Math.max((Math.max(...values) - Math.min(...values)) * 0.2, unit === "%" ? 5 : 15);
  const lo = Math.max(0, Math.min(...values) - pad);
  const hi = (unit === "%" ? Math.min(100, Math.max(...values) + pad) : Math.max(...values) + pad);
  const x = (i: number) => (points.length === 1 ? (L + W) / 2 : L + (i / (points.length - 1)) * (W - L - 20));
  const y = (v: number) => H - B - ((v - lo) / Math.max(hi - lo, 1e-6)) * (H - B - T);
  const improved = points.length > 1 && (better === "up" ? values[values.length - 1] >= values[0] : values[values.length - 1] <= values[0]);
  const color = points.length > 1 && !improved ? COLORS.warn : COLORS.hit;
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} className="plot" direction="ltr" role="img" aria-label={unit}>
        {[lo, (lo + hi) / 2, hi].map((v) => (
          <g key={v}>
            <line x1={L} x2={W - 10} y1={y(v)} y2={y(v)} stroke={COLORS.grid} />
            <text x={L - 6} y={y(v) + 4} textAnchor="end" fill={COLORS.muted} fontSize={11}>{Math.round(v)}</text>
          </g>
        ))}
        <polyline points={points.map((p, i) => `${x(i)},${y(p.value)}`).join(" ")} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" />
        {points.map((p, i) => (
          <g key={i}>
            <circle cx={x(i)} cy={y(p.value)} r={points.length > 40 ? 3 : 5} fill={color} stroke="#12161b" strokeWidth={2}
              onMouseMove={(e) => tip.show(e, p.tip ?? `${p.label}: ${Math.round(p.value)} ${unit}`)} onMouseLeave={tip.hide} />
            {showTick(points, i) && <text x={x(i)} y={H - 8} textAnchor="middle" fill={COLORS.muted} fontSize={11}>{p.label}</text>}
          </g>
        ))}
      </svg>
      {tip.node}
    </div>
  );
}
