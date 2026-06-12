// Renders the canonical STREETS modules (docs/streets/*.md) into web-safe
// Eleventy pages under site/src/streets/.
//
// docs/streets/ is the single source of truth. The module markdown carries
// in-app personalization directives from the retired desktop engine:
//   {@ mirror|insight|temporal NAME @}   -- data widgets        -> dropped
//   {? if EXPR ?} ... {? elif ?} ... {? else ?} ... {? endif ?} -- conditionals
//        every condition keys on local profile data a web reader does not
//        have, so the {? else ?} branch (the no-data branch) is emitted and
//        the personalized branches are dropped; no else -> block dropped
//   {= EXPR | fallback("X") =}           -- interpolations      -> fallback X
//
// Run after editing docs/streets:  node scripts/render-streets.mjs
// The generated files are committed so the Vercel build (rooted at site/)
// never needs to reach outside its root.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const DOCS = join(SITE, "..", "docs", "streets");
const OUT = join(SITE, "src", "streets");

// STREETS acronym order. Descriptions mirror the landing-page module cards.
const PAGES = [
  {
    src: "module-s-sovereign-setup.md",
    slug: "sovereign-setup",
    title: "Module S: Sovereign Setup",
    description:
      "Audit what you already own -- hardware, skills, tools, time -- and configure it as a foundation for generating income.",
  },
  {
    src: "module-t-technical-moats.md",
    slug: "technical-moats",
    title: "Module T: Technical Moats",
    description:
      "Identify what makes your combination of skills hard to replicate, and how to signal that to the people who'd pay for it.",
  },
  {
    src: "module-r-revenue-engines.md",
    slug: "revenue-engines",
    title: "Module R: Revenue Engines",
    description:
      "8 concrete ways developers earn independent income -- validation frameworks, pricing guidance, and 30-day launch plans.",
  },
  {
    src: "module-e1-execution-playbook.md",
    slug: "execution-playbook",
    title: "Module E: Execution Playbook",
    description:
      "The operating system for shipping revenue-generating projects alongside a full-time job.",
  },
  {
    src: "module-e2-evolving-edge.md",
    slug: "evolving-edge",
    title: "Module E: Evolving Edge",
    description:
      "Trend detection, pivot frameworks, and how to surface revenue-relevant signals before they become obvious.",
  },
  {
    src: "module-t2-tactical-automation.md",
    slug: "tactical-automation",
    title: "Module T: Tactical Automation",
    description:
      "Delivery pipelines, self-serve onboarding, payment automation, and the monitoring stack behind passive income.",
  },
  {
    src: "module-s2-stacking-streams.md",
    slug: "stacking-streams",
    title: "Module S: Stacking Streams",
    description:
      "Compound income architecture: how streams interact, when to add vs. scale, and the math behind $10K/month.",
  },
  {
    src: "2026-developer-income-map.md",
    slug: "income-map",
    title: "The 2026 Developer Income Map",
    description:
      "The companion market map: where developer income is moving in 2026, and which engines ride each shift.",
  },
];

const DIRECTIVE = /\{\?[\s\S]*?\?\}|\{@[\s\S]*?@\}|\{=[\s\S]*?=\}/g;

/** Resolve {? if ?}/{? elif ?}/{? else ?}/{? endif ?} blocks to their else branch. */
function resolveConditionals(text) {
  // Tokenize into literal text and {? ... ?} control tokens.
  const tokens = text.split(/(\{\?[\s\S]*?\?\})/);
  let i = 0;

  function kind(tok) {
    const inner = tok.slice(2, -2).trim();
    if (inner.startsWith("if ") || inner === "if") return "if";
    if (inner.startsWith("elif")) return "elif";
    if (inner === "else") return "else";
    if (inner === "endif") return "endif";
    throw new Error(`Unknown conditional token: ${tok}`);
  }

  // Parses tokens after an "if" until its matching endif.
  // Returns the text this whole block contributes (the else branch, or "").
  function parseBlock() {
    let branch = ""; // accumulating current branch text
    let elseText = null; // captured {? else ?} branch
    let inElse = false;
    while (i < tokens.length) {
      const tok = tokens[i];
      if (tok.startsWith("{?")) {
        const k = kind(tok);
        i++;
        if (k === "if") {
          branch += parseBlock(); // nested block resolves recursively
        } else if (k === "elif") {
          inElse = false;
          branch = ""; // discard previous personalized branch
        } else if (k === "else") {
          inElse = true;
          branch = "";
        } else if (k === "endif") {
          if (inElse) elseText = branch;
          return elseText ?? "";
        }
      } else {
        branch += tok;
        i++;
      }
    }
    throw new Error("Unbalanced conditional: missing {? endif ?}");
  }

  let out = "";
  while (i < tokens.length) {
    const tok = tokens[i];
    if (tok.startsWith("{?")) {
      const k = kind(tok);
      if (k !== "if") throw new Error(`Top-level ${k} without if`);
      i++;
      out += parseBlock();
    } else {
      out += tok;
      i++;
    }
  }
  return out;
}

function stripDirectives(raw) {
  let text = resolveConditionals(raw);
  // {= expr | fallback("X") =} -> X ; no fallback -> empty
  text = text.replace(/\{=([\s\S]*?)=\}/g, (_, inner) => {
    const m = inner.match(/fallback\(\s*"((?:[^"\\]|\\.)*)"\s*\)/);
    return m ? m[1] : "";
  });
  // {@ widget @} lines vanish entirely (kill the line if the widget was alone on it)
  text = text.replace(/^[ \t]*\{@[\s\S]*?@\}[ \t]*\r?\n/gm, "");
  text = text.replace(/\{@[\s\S]*?@\}/g, "");
  // tidy: no 3+ consecutive blank lines
  text = text.replace(/\n{4,}/g, "\n\n\n");
  return text;
}

function esc(s) {
  return s.replace(/"/g, '\\"');
}

mkdirSync(OUT, { recursive: true });

let failures = 0;
PAGES.forEach((page, idx) => {
  const raw = readFileSync(join(DOCS, page.src), "utf8");
  const body = stripDirectives(raw);

  const leaked = body.match(DIRECTIVE);
  if (leaked) {
    console.error(`LEAKED DIRECTIVES in ${page.src}:`, leaked.slice(0, 5));
    failures++;
    return;
  }

  const prev = idx > 0 ? PAGES[idx - 1] : null;
  const next = idx < PAGES.length - 1 ? PAGES[idx + 1] : null;

  const fm = [
    "---",
    `title: "${esc(page.title)} -- STREETS by 4DA"`,
    `description: "${esc(page.description)}"`,
    "layout: streets-module.njk",
    `permalink: "/streets/${page.slug}/"`,
    "templateEngineOverride: md",
    ...(prev ? [`prevUrl: "/streets/${prev.slug}/"`, `prevTitle: "${esc(prev.title)}"`] : []),
    ...(next ? [`nextUrl: "/streets/${next.slug}/"`, `nextTitle: "${esc(next.title)}"`] : []),
    "---",
    "",
  ].join("\n");

  writeFileSync(join(OUT, `${page.slug}.md`), fm + body);
  console.log(`rendered ${page.src} -> src/streets/${page.slug}.md (${(body.length / 1024).toFixed(1)}KB)`);
});

if (failures) {
  console.error(`\n${failures} file(s) failed.`);
  process.exit(1);
}
console.log(`\n${PAGES.length} pages rendered clean.`);
