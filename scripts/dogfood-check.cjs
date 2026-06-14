#!/usr/bin/env node
/**
 * dogfood-check.cjs - rule-10 dogfood observation harness for the 2026-06-14
 * scoring changes (L1 direct-dep CVE full-evidence, L1b direct_dep_floor 0.65,
 * clickbait hard-ceiling, PIPELINE_VERSION 5->6 backlog re-score).
 *
 * READ-ONLY. Run any day of the dogfood window to watch:
 *   - backlog drain progress (v6 vs v5 items re-scored with new code)
 *   - the watched direct-dep CVEs lifting off the old 0.50 floor
 *   - precision tripwire (high-score count must not balloon)
 *   - clickbait sanity + engine freshness receipts
 *
 * Usage:  node scripts/dogfood-check.cjs
 * ASCII-only output (operator terminal renders unicode as '?').
 */
const path = require("path");
const fs = require("fs");
const Database = require("better-sqlite3");

const DB = path.join(__dirname, "..", "data", "4da.db");
const BASELINE = path.join(__dirname, "..", ".claude", "plans", "dogfood-baseline-2026-06-14.json");

function fmt(n) { return (n === null || n === undefined) ? "null" : Number(n).toFixed(4); }
function line() { console.log("-".repeat(66)); }

if (!fs.existsSync(DB)) { console.error("DB not found:", DB); process.exit(1); }
const base = fs.existsSync(BASELINE) ? JSON.parse(fs.readFileSync(BASELINE, "utf8")) : { items: {}, distribution: {}, high_score_count_gt065: null };
const db = new Database(DB, { readonly: true, fileMustExist: true });

console.log("=".repeat(66));
console.log(" 4DA SCORING DOGFOOD CHECK  (baseline: 2026-06-14 pre-bump, v5)");
console.log("=".repeat(66));

// 1. Backlog drain progress
line();
console.log("BACKLOG DRAIN (PIPELINE_VERSION 5 -> 6):");
const vers = db.prepare("SELECT scored_pipeline_version v, COUNT(*) n FROM source_items GROUP BY v ORDER BY n DESC").all();
let total = 0, v6 = 0, v5 = 0;
for (const r of vers) { total += r.n; if (r.v === 6) v6 = r.n; if (r.v === 5) v5 = r.n; }
for (const r of vers) console.log(`  v${r.v}: ${r.n}`);
const drainPct = total ? ((v6 / total) * 100).toFixed(1) : "0.0";
console.log(`  -> re-scored with NEW code (v6): ${v6} / ${total} (${drainPct}%); remaining v5 backlog: ${v5}`);

// 2. Watched CVE items. Two expected directions:
//    - title names a DIRECT non-dev dep  -> the 0.65 floor should LIFT it
//    - title does NOT name a user dep     -> it loses the old artificial 0.50 floor and should FALL
line();
const directDeps = new Set(db.prepare("SELECT DISTINCT lower(package_name) p FROM user_dependencies WHERE is_direct=1 AND is_dev=0 AND package_name IS NOT NULL").all().map(r => r.p));
function namesDirectDep(title) {
  const t = (title || "").toLowerCase();
  for (const d of directDeps) { if (d.length >= 4 && new RegExp("(^|[^a-z0-9._-])" + d.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "([^a-z0-9._-]|$)").test(t)) return d; }
  return null;
}
console.log("WATCHED CVE ITEMS  old -> now  (DEP=expect rise to 0.65 | non-dep=expect fall):");
const getScore = db.prepare("SELECT relevance_score s, scored_pipeline_version v, title t FROM source_items WHERE id=?");
let lifted = 0, fell = 0, pending = 0;
for (const id of Object.keys(base.items || {})) {
  const row = getScore.get(Number(id));
  if (!row) { console.log(`  id=${id} (gone)`); continue; }
  const old = base.items[id].old_score;
  const dep = namesDirectDep(row.t);
  const kind = dep ? `DEP(${dep})` : "non-dep";
  const changed = (row.s !== null && old !== null && Math.abs(row.s - old) > 1e-4);
  const arrow = changed ? "  <== CHANGED" : "";
  const rescored = row.v === 6 ? "[v6]" : `[v${row.v}]`;
  if (row.v === 6) { if (dep && row.s >= 0.65) lifted++; else if (!dep && row.s < old) fell++; } else pending++;
  const title = (row.t || "").replace(/[^\x20-\x7e]/g, "?").slice(0, 38);
  console.log(`  id=${id} ${rescored} ${kind.padEnd(14)} ${fmt(old)} -> ${fmt(row.s)}${arrow} | ${title}`);
}
console.log(`  -> direct-dep lifted >=0.65: ${lifted}; non-dep correctly fell: ${fell}; awaiting drain (v5): ${pending}`);

// 3. Direct-dep CVE band (approx: CVE items whose title names a distinctive direct non-dev dep)
line();
console.log("DIRECT-DEP CVE BAND (approx title-match, distinctive deps >=5 chars):");
const STOP = new Set(["path","http","https","core","image","react","next","redis","async","serde","tokio","query","router","build","cache","model","table","value","event","state","style","color","frame","point","stack"]);
const deps = db.prepare("SELECT DISTINCT lower(package_name) p FROM user_dependencies WHERE is_direct=1 AND is_dev=0 AND package_name IS NOT NULL AND length(package_name)>=6").all()
  .map(r => r.p).filter(p => !STOP.has(p));
const depSet = new Set(deps);
const cveItems = db.prepare("SELECT id, lower(title) t, relevance_score s, scored_pipeline_version v FROM source_items WHERE cve_ids IS NOT NULL AND cve_ids!='' AND cve_ids!='[]' AND relevance_score IS NOT NULL").all();
let band = { matched: 0, v6: 0, ge065: 0, ge065_v6: 0 };
for (const it of cveItems) {
  let hit = false;
  for (const d of depSet) { if (new RegExp("(^|[^a-z0-9._-])" + d.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "([^a-z0-9._-]|$)").test(it.t)) { hit = true; break; } }
  if (!hit) continue;
  band.matched++;
  if (it.v === 6) band.v6++;
  if (it.s >= 0.65) { band.ge065++; if (it.v === 6) band.ge065_v6++; }
}
console.log(`  matched CVE items: ${band.matched}; re-scored v6: ${band.v6}; now >=0.65: ${band.ge065} (of which v6: ${band.ge065_v6})`);
console.log(`  (distinctive direct deps tracked: ${depSet.size})`);

// 4. Precision tripwire: high-score count must not balloon vs baseline
line();
console.log("PRECISION TRIPWIRE (high-score count >0.65):");
const hi = db.prepare("SELECT COUNT(*) n FROM source_items WHERE relevance_score>0.65").get().n;
const baseHi = base.high_score_count_gt065;
const delta = baseHi != null ? hi - baseHi : null;
console.log(`  now: ${hi}  baseline: ${baseHi}  delta: ${delta === null ? "n/a" : (delta >= 0 ? "+" + delta : delta)}`);
if (baseHi != null && hi > baseHi * 1.5) console.log("  WARNING: high-score count grew >50% -- possible precision regression, inspect.");
else console.log("  ok (no runaway high-score inflation).");

// 5. Clickbait sanity (titles the new is_strong_clickbait cap targets; security exempt)
line();
console.log("CLICKBAIT SANITY (non-security items >=0.45 with strong-clickbait markers):");
const cbRows = db.prepare("SELECT id, title, relevance_score s FROM source_items WHERE relevance_score>=0.45 AND title IS NOT NULL AND (cve_ids IS NULL OR cve_ids='' OR cve_ids='[]') AND lower(title) NOT LIKE '%vulnerab%' AND lower(title) NOT LIKE '%cve-%' AND lower(title) NOT LIKE '%advisory%'").all();
const MARK = [/\bthis one\b/, /\byou won'?t believe\b/, /\bone (weird|simple) (trick|tip)\b/, /\bwill (shock|blow your mind)\b/, /\bnobody (is talking|tells you)\b/];
const cb = cbRows.filter(r => { const t = (r.title || "").toLowerCase(); return MARK.some(m => m.test(t)); });
console.log(`  flagged (heuristic, manual confirm): ${cb.length}`);
for (const r of cb.slice(0, 6)) console.log(`   id=${r.id} ${fmt(r.s)} | ${(r.title || "").replace(/[^\x20-\x7e]/g, "?").slice(0, 56)}`);
if (cb.length === 0) console.log("   none above 0.45.");

// 6. Engine freshness receipts
line();
console.log("ENGINE FRESHNESS (recent engine_runs):");
try {
  const runs = db.prepare("SELECT * FROM engine_runs ORDER BY rowid DESC LIMIT 5").all();
  if (!runs.length) console.log("  (no engine_runs rows)");
  for (const r of runs) {
    const ts = r.completed_at || r.started_at || r.created_at || "?";
    const scored = r.items_scored != null ? r.items_scored : "?";
    const rel = r.relevant_count != null ? r.relevant_count : "?";
    console.log(`  ${ts}  scored=${scored} relevant=${rel}`);
  }
} catch (e) { console.log("  engine_runs read error:", e.message); }

line();
console.log("Done. Re-run daily through the 7-day window. Backlog fully re-scores as v6 -> ~100%.");
db.close();
