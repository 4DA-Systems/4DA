// Page wiring + rendering. All registry/OSV-sourced strings are rendered with
// textContent (never innerHTML) — advisory summaries are untrusted input.
import { parseManifest, detectManifest } from "./parsers.mjs";
import { scan } from "./sources.mjs";

const $ = (id) => document.getElementById(id);
const manifestEl = $("manifest");
const detectEl = $("detect");
const scanBtn = $("scanBtn");
const inputError = $("inputError");
const inputCard = $("inputCard");
const feedCard = $("feedCard");
const feedEl = $("feed");
const feedTitle = $("feedTitle");
const resultsEl = $("results");
const resultsFooter = $("resultsFooter");

const ECO_LABEL = { npm: "npm", "crates.io": "crates.io", PyPI: "PyPI", Go: "Go modules" };

// ---------------------------------------------------------------------------
// Sample stacks (the same reference stacks 4DA's public ledger monitors)
// ---------------------------------------------------------------------------
const SAMPLES = {
  rust: `[package]\nname = "rust-service"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\ntokio = { version = "1.38", features = ["full"] }\naxum = "0.7.5"\nserde = { version = "1.0.203", features = ["derive"] }\nserde_json = "1.0.117"\nreqwest = { version = "0.12.4", features = ["json"] }\nsqlx = { version = "0.7.4", features = ["postgres"] }\ntracing = "0.1.40"\ntower = "0.4.13"\nanyhow = "1.0.86"\nthiserror = "1.0.61"`,
  next: `{\n  "name": "nextjs-app",\n  "dependencies": {\n    "next": "14.2.3",\n    "react": "18.3.1",\n    "react-dom": "18.3.1",\n    "@prisma/client": "5.14.0",\n    "zod": "3.23.8",\n    "tailwindcss": "3.4.3",\n    "@tanstack/react-query": "5.40.0",\n    "next-auth": "4.24.7"\n  },\n  "devDependencies": {\n    "typescript": "5.4.5",\n    "eslint": "8.57.0",\n    "prisma": "5.14.0"\n  }\n}`,
  py: `torch==2.3.0\ntransformers==4.41.0\nnumpy==1.26.4\npandas==2.2.2\nfastapi==0.111.0\nuvicorn==0.29.0\nscikit-learn==1.4.2\npydantic==2.7.1\nhttpx==0.27.0\npillow==10.3.0`,
  go: `module example.com/go-service\n\ngo 1.22\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n\tgithub.com/jackc/pgx/v5 v5.5.5\n\tgithub.com/redis/go-redis/v9 v9.5.1\n\tgithub.com/spf13/viper v1.18.2\n\tgoogle.golang.org/grpc v1.63.2\n)`,
};

// ---------------------------------------------------------------------------
// Small DOM helpers — element creation only, no HTML strings.
// ---------------------------------------------------------------------------
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

function link(href, text) {
  const a = el("a", null, text);
  a.href = href;
  a.target = "_blank";
  a.rel = "noopener";
  return a;
}

function timeAgo(iso) {
  const d = (Date.now() - new Date(iso).getTime()) / 86400000;
  if (d < 1) return "today";
  if (d < 2) return "yesterday";
  return `${Math.floor(d)}d ago`;
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------
let detected = null;

function refreshDetect() {
  const text = manifestEl.value;
  inputError.hidden = true;
  detected = text.trim() ? detectManifest(text) : null;
  detectEl.replaceChildren();
  if (detected) {
    detectEl.append(el("span", "eco", ECO_LABEL[detected]), el("span", null, "manifest detected"));
    scanBtn.disabled = false;
  } else {
    if (text.trim().length > 40) detectEl.append(el("span", null, "format not recognized yet"));
    scanBtn.disabled = true;
  }
}

manifestEl.addEventListener("input", refreshDetect);

for (const btn of document.querySelectorAll("[data-sample]")) {
  btn.addEventListener("click", () => {
    manifestEl.value = SAMPLES[btn.dataset.sample];
    refreshDetect();
    runScan();
  });
}

inputCard.addEventListener("dragover", (e) => { e.preventDefault(); inputCard.classList.add("dragover"); });
inputCard.addEventListener("dragleave", () => inputCard.classList.remove("dragover"));
inputCard.addEventListener("drop", async (e) => {
  e.preventDefault();
  inputCard.classList.remove("dragover");
  const file = e.dataTransfer?.files?.[0];
  if (!file) return;
  manifestEl.value = await file.text();
  refreshDetect();
  if (detected) runScan();
});

scanBtn.addEventListener("click", runScan);

// Shareable demo links: ?sample=rust|next|py|go auto-fills and scans on load.
const auto = new URLSearchParams(location.search).get("sample");
if (auto && SAMPLES[auto]) {
  manifestEl.value = SAMPLES[auto];
  refreshDetect();
  queueMicrotask(runScan);
}

// ---------------------------------------------------------------------------
// Scan flow
// ---------------------------------------------------------------------------
let scanning = false;

async function runScan() {
  if (scanning || !detected) return;
  const parsed = parseManifest(manifestEl.value);
  if (!parsed || parsed.deps.length === 0) {
    inputError.textContent = "No dependencies found in this manifest.";
    inputError.hidden = false;
    return;
  }
  scanning = true;
  scanBtn.disabled = true;
  resultsEl.hidden = true;
  resultsEl.replaceChildren();
  resultsFooter.hidden = true;
  feedEl.textContent = "";
  feedTitle.textContent = `scanning ${parsed.label}`;
  feedCard.hidden = false;
  feedCard.classList.add("live");
  const t0 = performance.now();

  const progress = (line) => {
    feedEl.append(line + "\n");
    feedEl.scrollTop = feedEl.scrollHeight;
  };

  try {
    const report = await scan(parsed, progress);
    const secs = ((performance.now() - t0) / 1000).toFixed(1);
    feedTitle.textContent = `scan complete in ${secs}s`;
    render(parsed, report);
  } catch (e) {
    feedTitle.textContent = "scan failed";
    progress(`error: ${e.message}`);
  } finally {
    feedCard.classList.remove("live");
    scanning = false;
    scanBtn.disabled = false;
  }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------
function statCell(n, label, tone) {
  const cell = el("div", "stat");
  const num = el("div", "n " + (n === 0 ? "zero" : tone), String(n));
  cell.append(num, el("div", "l", label));
  return cell;
}

function panel(title, count, delay) {
  const p = el("section", "panel rise");
  p.style.animationDelay = `${delay}ms`;
  const head = el("div", "panel-head");
  head.append(el("h2", null, title), el("span", "count", String(count)));
  const rows = el("div", "rows");
  p.append(head, rows);
  return [p, rows];
}

function pkgCell(name, version) {
  const c = el("div", "pkg", name);
  if (version) c.append(el("span", "ver", ` ${version}`));
  return c;
}

function render(parsed, r) {
  const frag = document.createDocumentFragment();

  // Summary strip
  const stats = el("div", "stats rise");
  stats.append(
    statCell(r.stats.advisories, "advisories", "alert"),
    statCell(r.stats.releases, `releases · ${r.windowDays}d`, "gold"),
    statCell(r.stats.behind, "majors behind", "gold"),
    statCell(r.stats.deprecated, "deprecated", "alert"),
  );
  frag.append(stats);

  let delay = 60;

  // Advisories — actionable severities up front, the long tail behind a fold.
  if (r.advisories.length) {
    const [p, rows] = panel("Security advisories", r.advisories.length, delay += 60);
    const headline = r.advisories.filter((a) => !["low", "unknown"].includes(a.severity));
    const tail = r.advisories.filter((a) => ["low", "unknown"].includes(a.severity));
    const advisoryRow = (a) => {
      const row = el("div", "row");
      row.append(pkgCell(a.package, a.version));
      const what = el("div", "what");
      what.append(el("span", `sev ${a.severity}`, a.severity));
      what.append(link(a.url, a.cve ? `${a.cve} — ${a.summary}` : a.summary));
      if (a.fixed) what.append(el("span", "fix", `  fixed in ${a.fixed}`));
      if (a.dev) what.append(el("span", "devtag", "  dev"));
      row.append(what, el("div", "meta", a.published ? timeAgo(a.published) : ""));
      return row;
    };
    for (const a of headline) rows.append(advisoryRow(a));
    if (tail.length) {
      const fold = el("button", "fold", `show ${tail.length} low-severity advisor${tail.length > 1 ? "ies" : "y"}`);
      fold.addEventListener("click", () => {
        fold.remove();
        for (const a of tail) rows.append(advisoryRow(a));
      }, { once: true });
      rows.append(fold);
    }
    frag.append(p);
  }

  // Releases — grouped per package, newest first
  if (r.releases.length) {
    const byPkg = new Map();
    for (const rel of r.releases) {
      if (!byPkg.has(rel.package)) byPkg.set(rel.package, { ...rel, count: 0 });
      byPkg.get(rel.package).count++;
    }
    const groups = [...byPkg.values()].sort((a, b) => new Date(b.at) - new Date(a.at));
    const [p, rows] = panel(`Shipped in the last ${r.windowDays} days`, r.releases.length, delay += 60);
    for (const g of groups) {
      const row = el("div", "row");
      row.append(pkgCell(g.package, g.pinned ? `${g.pinned} pinned` : null));
      const what = el("div", "what");
      what.append(link(g.url, `${g.version} is out`));
      if (g.count > 1) what.append(el("span", "devtag", `  +${g.count - 1} more`));
      row.append(what, el("div", "meta", timeAgo(g.at)));
      rows.append(row);
    }
    frag.append(p);
  }

  // Falling behind
  if (r.behind.length) {
    const [p, rows] = panel("Falling behind", r.behind.length, delay += 60);
    for (const b of r.behind) {
      const row = el("div", "row");
      row.append(pkgCell(b.package, b.pinned));
      const what = el("div", "what");
      what.append(link(b.url, `latest is ${b.latest}`));
      what.append(el("span", "majors", `  +${b.majors} major${b.majors > 1 ? "s" : ""}`));
      if (b.dev) what.append(el("span", "devtag", "  dev"));
      row.append(what, el("div", "meta", ""));
      rows.append(row);
    }
    frag.append(p);
  }

  // Deprecated
  if (r.deprecated.length) {
    const [p, rows] = panel("Deprecated", r.deprecated.length, delay += 60);
    for (const d of r.deprecated) {
      const row = el("div", "row");
      row.append(pkgCell(d.package, null));
      const what = el("div", "what");
      what.append(link(d.url, d.message));
      row.append(what, el("div", "meta", ""));
      rows.append(row);
    }
    frag.append(p);
  }

  // Quiet stack
  if (!r.advisories.length && !r.releases.length && !r.behind.length && !r.deprecated.length) {
    const q = el("div", "quiet rise");
    q.append(
      el("div", "q-title", "Quiet month."),
      el("div", null, `${r.stats.deps} dependencies checked — no advisories, no releases in the window, nothing deprecated.`),
    );
    frag.append(q);
  }

  // Notes (caps, skips) — disclosed, never silent
  if (r.notes.length) {
    const n = el("div", "notes rise");
    for (const note of r.notes) n.append(el("p", null, note));
    frag.append(n);
  }

  resultsEl.replaceChildren(frag);
  resultsEl.hidden = false;
  resultsFooter.hidden = false;
}
