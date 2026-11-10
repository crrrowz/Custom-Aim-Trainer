import { useEffect, useRef, useState } from "react";

/** Local calendar day as `YYYY-MM-DD`, the key every page groups sessions by. */
export const dayKey = (ms: number) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

type Props = {
  /** Days that have sessions. Every other day, and every day after today, cannot be picked. */
  available: Set<string>;
  selected: string[];
  onChange: (days: string[]) => void;
  /** Several days at once, for comparing them. */
  multi?: boolean;
  lang: "ar" | "en";
  label: string;
};

/** A button that opens a month calendar. */
export function DayPicker({ available, selected, onChange, multi, lang, label }: Props) {
  const [open, setOpen] = useState(false);
  const first = selected[0] ?? [...available].sort().pop() ?? dayKey(Date.now());
  const [month, setMonth] = useState(() => first.slice(0, 7));
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => box.current && !box.current.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  const locale = lang === "ar" ? "ar-IQ" : "en-GB";
  const today = dayKey(Date.now());
  const [y, m] = month.split("-").map(Number);
  const startDay = lang === "ar" ? 6 : 1; // Saturday in Iraq, Monday in English.
  const firstWeekday = new Date(y, m - 1, 1).getDay();
  const lead = (firstWeekday - startDay + 7) % 7;
  const count = new Date(y, m, 0).getDate();
  const cells: (string | null)[] = [
    ...Array.from({ length: lead }, () => null),
    ...Array.from({ length: count }, (_, i) => `${month}-${String(i + 1).padStart(2, "0")}`),
  ];
  const weekdays = Array.from({ length: 7 }, (_, i) =>
    new Date(2024, 0, 7 + ((startDay + i) % 7)).toLocaleDateString(locale, { weekday: "narrow" }),
  );
  const shift = (by: number) => {
    const d = new Date(y, m - 1 + by, 1);
    setMonth(`${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`);
  };
  const canNext = month < today.slice(0, 7);
  const pick = (d: string) => {
    if (!multi) {
      onChange([d]);
      setOpen(false);
      return;
    }
    onChange(selected.includes(d) ? selected.filter((x) => x !== d) : [...selected, d].sort());
  };
  const inMonth = [...available].filter((d) => d.startsWith(month)).length;

  return (
    <div className="daypicker" ref={box}>
      <button className={`pick-btn ${open ? "on" : ""}`} onClick={() => setOpen(!open)} aria-expanded={open}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
          <rect x="3" y="5" width="18" height="16" rx="2" /><path d="M3 10h18M8 3v4M16 3v4" />
        </svg>
        {label}
      </button>
      {open && (
        <div className="cal" role="dialog">
          <div className="cal-head">
            <button onClick={() => shift(-1)} aria-label="previous month">‹</button>
            <b>{new Date(y, m - 1, 1).toLocaleDateString(locale, { month: "long", year: "numeric" })}</b>
            <button onClick={() => shift(1)} disabled={!canNext} aria-label="next month">›</button>
          </div>
          <div className="cal-grid">
            {weekdays.map((w, i) => <span key={i} className="wd">{w}</span>)}
            {cells.map((d, i) => {
              if (!d) return <span key={i} />;
              const usable = available.has(d) && d <= today;
              const on = selected.includes(d);
              return (
                <button key={d} disabled={!usable} className={`${on ? "on" : ""} ${d === today ? "today" : ""} ${usable ? "has" : ""}`} onClick={() => pick(d)}>
                  {Number(d.slice(8))}
                </button>
              );
            })}
          </div>
          <small className="muted">
            {inMonth === 0
              ? lang === "ar" ? "ما لعبت بهذا الشهر." : "No sessions this month."
              : multi
                ? lang === "ar" ? "اختار أكثر من يوم حتى تقارنهم." : "Pick several days to compare them."
                : lang === "ar" ? "الأيام المضيئة بيها جلسات." : "Highlighted days have sessions."}
          </small>
        </div>
      )}
    </div>
  );
}
