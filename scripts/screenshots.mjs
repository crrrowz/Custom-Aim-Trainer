// Captures every page of the desktop UI, on its built-in sample data, into docs/images/.
//
// Usage (Windows, from the repo root):
//   npm --prefix apps/desktop/ui run dev -- --port 1431      (in one terminal)
//   node scripts/screenshots.mjs [outDir] [url]              (in another)
//
// It starts a headless Microsoft Edge with a remote-debugging port, drives the UI through the
// DevTools protocol (no extra packages), and saves 1920x1230 PNGs. Pages are opened by clicking the
// side menu, so a renamed menu item must be renamed here too.
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const outDir = process.argv[2] ?? "docs/images";
const url = process.argv[3] ?? "http://localhost:1431/";
const port = 9333;
const edge = process.env.EDGE_PATH ?? "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";

/** [file name, steps]; a step is ["nav", menu label] | ["sel", css selector, text] | ["wait", ms]. */
const PAGES = [
  ["dashboard", [["nav", "Dashboard"]]],
  ["plan", [["nav", "Plan"]]],
  ["practice", [["nav", "Free practice"]]],
  ["analysis", [["nav", "Analysis"]]],
  ["session-replay", [["sel", ".session-card", ""], ["wait", 1500]]],
  ["session-shots", [["sel", "[role=tab]", "Where shots landed"]]],
  ["session-directions", [["sel", "[role=tab]", "Accuracy & speed"]]],
  ["progress", [["nav", "Progress"]]],
  ["coach", [["nav", "Coach reports"], ["sel", ".scope-cards button", ""], ["wait", 800]]],
  ["settings", [["nav", "Settings"]]],
  ["dashboard-ar", [["sel", ".lang-switch button", "العربية"], ["wait", 600], ["nav", "الرئيسية"]]],
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
mkdirSync(outDir, { recursive: true });
const profile = mkdtempSync(join(tmpdir(), "aimforge-shots-"));
const browser = spawn(edge, ["--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--hide-scrollbars", "about:blank"], { stdio: "ignore" });

try {
  let targets;
  for (let i = 0; i < 40 && !targets; i++) {
    await sleep(250);
    targets = await fetch(`http://localhost:${port}/json/list`).then((r) => r.json()).catch(() => undefined);
  }
  if (!targets) throw new Error("Edge did not start; set EDGE_PATH to msedge.exe");
  const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = (m) => { const msg = JSON.parse(m.data); pending.get(msg.id)?.(msg); pending.delete(msg.id); };
  const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
  const js = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description ?? expression);
    return r.result?.result?.value;
  };
  const click = (sel, text) => js(`(() => {
    const el = [...document.querySelectorAll(${JSON.stringify(sel)})].find((e) => e.textContent.includes(${JSON.stringify(text)}));
    if (!el) throw new Error("not found: ${sel} ${text}");
    el.click();
  })()`);

  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 820, deviceScaleFactor: 1.5, mobile: false });
  await send("Page.navigate", { url });
  await sleep(2500);
  await click(".lang-switch button", "English");
  await sleep(400);
  // The sample data starts in test mode; switch it off so pages look like normal use.
  await click(".nav button", "Settings");
  await sleep(1200);
  await js(`(() => { const i = document.querySelector('input[aria-label="Test mode"]'); if (i && i.checked) i.click(); })()`);
  await sleep(600);

  for (const [name, steps] of PAGES) {
    for (const [kind, a, b] of steps) {
      if (kind === "nav") await click(".nav button", a);
      else if (kind === "sel") await click(a, b);
      else if (kind === "wait") await sleep(a);
      await sleep(500);
    }
    await js("window.scrollTo(0, 0)");
    await sleep(700);
    const shot = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(outDir, `${name}.png`), Buffer.from(shot.result.data, "base64"));
    console.log("saved", name);
  }
  ws.close();
} finally {
  browser.kill();
  await sleep(500);
  rmSync(profile, { recursive: true, force: true });
}
