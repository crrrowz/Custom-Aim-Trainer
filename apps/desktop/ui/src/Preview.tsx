import { useEffect, useRef } from "react";
import type { CustomTargets } from "./api";

const HALF_HFOV = (103 / 2) * (Math.PI / 180);
/** Screen position of an angle, using the same perspective projection as the game. */
const project = (deg: number, halfWidth: number) => (Math.tan((deg * Math.PI) / 180) / Math.tan(HALF_HFOV)) * halfWidth;

/** A small live picture of what the chosen settings will look like in the game. */
export function TargetPreview({ c }: { c: CustomTargets }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const settings = useRef(c);
  settings.current = c;

  useEffect(() => {
    const canvas = ref.current!;
    const g = canvas.getContext("2d")!;
    let raf = 0;
    const start = performance.now();
    const seedAngles = [0.4, 2.2, 3.7, 5.1, 1.3, 4.4];

    const draw = (now: number) => {
      const t = (now - start) / 1000;
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth, h = canvas.clientHeight;
      if (canvas.width !== Math.round(w * dpr)) {
        canvas.width = Math.round(w * dpr);
        canvas.height = Math.round(h * dpr);
      }
      g.setTransform(dpr, 0, 0, dpr, 0, 0);
      g.fillStyle = "#0b0e12";
      g.fillRect(0, 0, w, h);
      g.strokeStyle = "#1a212b";
      g.lineWidth = 1;
      for (let x = (w / 2) % 40; x < w; x += 40) { g.beginPath(); g.moveTo(x, 0); g.lineTo(x, h); g.stroke(); }
      for (let y = (h / 2) % 40; y < h; y += 40) { g.beginPath(); g.moveTo(0, y); g.lineTo(w, y); g.stroke(); }

      const s = settings.current;
      const r = Math.max(project(s.radius_deg, w / 2), 1.5);
      const cx = w / 2, cy = h / 2;
      const dot = (x: number, y: number, alpha = 1) => {
        g.globalAlpha = alpha;
        g.fillStyle = "#ff5a5f";
        g.beginPath();
        g.arc(x, y, r, 0, Math.PI * 2);
        g.fill();
        g.globalAlpha = 1;
      };

      if (s.mode === "track") {
        dot(cx + project(Math.sin(t * 0.7) * 22, w / 2), cy + project(Math.sin(t * 1.1) * 7, w / 2));
      } else if (s.mode === "strafe") {
        const period = Math.max(30 / s.speed_deg_s, 0.3);
        const phase = (t % (period * 2)) / period;
        const tri = phase < 1 ? phase : 2 - phase;
        dot(cx + project((tri - 0.5) * 30, w / 2), cy);
      } else if (s.mode === "peek") {
        const wall = project(25, w / 2);
        const out = Math.max(0, Math.sin(t * 1.6)) * r * 4;
        g.fillStyle = "#2a3442";
        for (const dir of [-1, 1]) g.fillRect(cx + dir * wall - 12, 0, 24, h);
        dot(cx + wall - 12 - out + r, cy - 8);
        g.fillStyle = "#2a3442";
        g.fillRect(cx + wall - 12, 0, 24, h);
      } else {
        const count = s.mode === "multi" ? Math.min(s.concurrent, seedAngles.length) : 1;
        const beat = s.mode === "flick" ? Math.floor(t / Math.max(s.time_limit_s, 0.4)) : 0;
        const lo = Math.min(s.min_distance_deg, s.max_distance_deg);
        const hi = Math.max(s.min_distance_deg, s.max_distance_deg);
        // Dashed rings show the nearest and farthest spawn distance.
        g.setLineDash([4, 5]);
        g.strokeStyle = "#3a4656";
        for (const deg of [lo, hi]) {
          g.beginPath();
          g.arc(cx, cy, project(deg, w / 2), 0, Math.PI * 2);
          g.stroke();
        }
        g.setLineDash([]);
        for (let i = 0; i < count; i++) {
          const a = seedAngles[(i + beat) % seedAngles.length] + beat * 1.3;
          const d = lo + ((hi - lo) * ((Math.sin(beat * 2.1 + i) + 1) / 2));
          dot(cx + Math.cos(a) * project(d, w / 2), cy + Math.sin(a) * project(d, w / 2) * 0.7);
        }
      }

      g.strokeStyle = "#e8edf3";
      g.lineWidth = 1.5;
      for (const [a, b, c1, d] of [[-9, 0, -3, 0], [3, 0, 9, 0], [0, -9, 0, -3], [0, 3, 0, 9]]) {
        g.beginPath(); g.moveTo(cx + a, cy + b); g.lineTo(cx + c1, cy + d); g.stroke();
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, []);

  return <canvas ref={ref} className="preview" aria-label="target preview" />;
}
