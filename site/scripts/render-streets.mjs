// Renders the canonical STREETS modules (docs/streets/**) into web-safe
// Eleventy pages under site/src/streets/ — English plus all 12 translations.
//
// docs/streets/ is the single source of truth (EN at the root, translations
// in docs/streets/<lang>/). The module markdown carries in-app
// personalization directives from the retired desktop engine:
//   {@ mirror|insight|temporal NAME @}   -- data widgets        -> dropped
//   {? if EXPR ?} ... {? elif ?} ... {? else ?} ... {? endif ?} -- conditionals
//        every condition keys on local profile data a web reader does not
//        have, so the {? else ?} branch (the no-data branch) is emitted and
//        the personalized branches are dropped; no else -> block dropped
//   {= EXPR | fallback("X") =}           -- interpolations      -> fallback X
//
// Output URLs: /streets/<slug>/ (EN) and /streets/<lang>/<slug>/ (translations),
// with hreflang alternates emitted into each page's frontmatter for the layout.
//
// Run after editing docs/streets:  node scripts/render-streets.mjs
// The generated files are committed so the Vercel build (rooted at site/)
// never needs to reach outside its root.

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SITE = dirname(dirname(fileURLToPath(import.meta.url)));
const DOCS = join(SITE, "..", "docs", "streets");
const OUT = join(SITE, "src", "streets");

// EN first (x-default). label = native language name for the switcher.
const LOCALES = [
  { code: "en", label: "English", dir: "ltr", prev: "Previous", next: "Next" },
  { code: "ar", label: "العربية", dir: "rtl", prev: "السابق", next: "التالي" },
  { code: "de", label: "Deutsch", dir: "ltr", prev: "Zurück", next: "Weiter" },
  { code: "es", label: "Español", dir: "ltr", prev: "Anterior", next: "Siguiente" },
  { code: "fr", label: "Français", dir: "ltr", prev: "Précédent", next: "Suivant" },
  { code: "hi", label: "हिन्दी", dir: "ltr", prev: "पिछला", next: "अगला" },
  { code: "it", label: "Italiano", dir: "ltr", prev: "Precedente", next: "Successivo" },
  { code: "ja", label: "日本語", dir: "ltr", prev: "前へ", next: "次へ" },
  { code: "ko", label: "한국어", dir: "ltr", prev: "이전", next: "다음" },
  { code: "pt-BR", label: "Português (BR)", dir: "ltr", prev: "Anterior", next: "Próximo" },
  { code: "ru", label: "Русский", dir: "ltr", prev: "Назад", next: "Далее" },
  { code: "tr", label: "Türkçe", dir: "ltr", prev: "Önceki", next: "Sonraki" },
  { code: "zh", label: "中文", dir: "ltr", prev: "上一个", next: "下一个" },
];

// STREETS acronym order. Descriptions (EN pages) mirror the landing cards;
// non-EN pages derive their description from the translated lead paragraph.
const PAGES = [
  {
    src: "module-s-sovereign-setup.md",
    slug: "sovereign-setup",
    title: "Module S: Sovereign Setup",
    description:
      "Audit what you already own -- hardware, skills, tools, time -- and configure it as a foundation for generating income.",
    translated: true,
  },
  {
    src: "module-t-technical-moats.md",
    slug: "technical-moats",
    title: "Module T: Technical Moats",
    description:
      "Identify what makes your combination of skills hard to replicate, and how to signal that to the people who'd pay for it.",
    translated: true,
  },
  {
    src: "module-r-revenue-engines.md",
    slug: "revenue-engines",
    title: "Module R: Revenue Engines",
    description:
      "8 concrete ways developers earn independent income -- validation frameworks, pricing guidance, and 30-day launch plans.",
    translated: true,
  },
  {
    src: "module-e1-execution-playbook.md",
    slug: "execution-playbook",
    title: "Module E: Execution Playbook",
    description:
      "The operating system for shipping revenue-generating projects alongside a full-time job.",
    translated: true,
  },
  {
    src: "module-e2-evolving-edge.md",
    slug: "evolving-edge",
    title: "Module E: Evolving Edge",
    description:
      "Trend detection, pivot frameworks, and how to surface revenue-relevant signals before they become obvious.",
    translated: true,
  },
  {
    src: "module-t2-tactical-automation.md",
    slug: "tactical-automation",
    title: "Module T: Tactical Automation",
    description:
      "Delivery pipelines, self-serve onboarding, payment automation, and the monitoring stack behind passive income.",
    translated: true,
  },
  {
    src: "module-s2-stacking-streams.md",
    slug: "stacking-streams",
    title: "Module S: Stacking Streams",
    description:
      "Compound income architecture: how streams interact, when to add vs. scale, and the math behind $10K/month.",
    translated: true,
  },
  {
    src: "2026-developer-income-map.md",
    slug: "income-map",
    title: "The 2026 Developer Income Map",
    description:
      "The companion market map: where developer income is moving in 2026, and which engines ride each shift.",
    translated: false, // EN only
  },
];

const DIRECTIVE = /\{\?[\s\S]*?\?\}|\{@[\s\S]*?@\}|\{=[\s\S]*?=\}/g;

/** Resolve {? if ?}/{? elif ?}/{? else ?}/{? endif ?} blocks to their else branch. */
function resolveConditionals(text) {
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

  function parseBlock() {
    let branch = "";
    let elseText = null;
    let inElse = false;
    while (i < tokens.length) {
      const tok = tokens[i];
      if (tok.startsWith("{?")) {
        const k = kind(tok);
        i++;
        if (k === "if") {
          branch += parseBlock();
        } else if (k === "elif") {
          inElse = false;
          branch = "";
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
  text = text.replace(/\{=([\s\S]*?)=\}/g, (_, inner) => {
    const m = inner.match(/fallback\(\s*"((?:[^"\\]|\\.)*)"\s*\)/);
    return m ? m[1] : "";
  });
  text = text.replace(/^[ \t]*\{@[\s\S]*?@\}[ \t]*\r?\n/gm, "");
  text = text.replace(/\{@[\s\S]*?@\}/g, "");
  text = text.replace(/\n{4,}/g, "\n\n\n");
  return text;
}

function esc(s) {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

/** First markdown H1 of the document (translated module title). */
function extractH1(body) {
  const m = body.match(/^#\s+(.+)$/m);
  return m ? m[1].trim() : null;
}

/** First substantial plain paragraph, truncated for a meta description. */
function extractLead(body) {
  const lines = body.split(/\r?\n/);
  for (let j = 0; j < lines.length; j++) {
    const line = lines[j].trim();
    if (!line) continue;
    if (/^[#>*\-|`!\[]/.test(line)) continue; // headings, quotes, lists, tables, code, images
    if (line.length < 60) continue; // skip bylines/short labels
    const clean = line.replace(/\*\*|__|\*|`/g, "");
    return clean.length > 155 ? clean.slice(0, 152).trimEnd() + "..." : clean;
  }
  return null;
}

function pageUrl(locale, slug) {
  return locale === "en" ? `/streets/${slug}/` : `/streets/${locale}/${slug}/`;
}

function sourcePath(locale, src) {
  return locale === "en" ? join(DOCS, src) : join(DOCS, locale, src);
}

mkdirSync(OUT, { recursive: true });

let written = 0;
let failures = 0;

for (const locale of LOCALES) {
  const isEn = locale.code === "en";
  const localePages = PAGES.filter((p) => isEn || p.translated);

  if (!isEn) mkdirSync(join(OUT, locale.code), { recursive: true });

  localePages.forEach((page) => {
    const srcPath = sourcePath(locale.code, page.src);
    if (!existsSync(srcPath)) {
      console.error(`MISSING SOURCE: ${srcPath}`);
      failures++;
      return;
    }
    const raw = readFileSync(srcPath, "utf8");
    const body = stripDirectives(raw);

    const leaked = body.match(DIRECTIVE);
    if (leaked) {
      console.error(`LEAKED DIRECTIVES in ${locale.code}/${page.src}:`, leaked.slice(0, 5));
      failures++;
      return;
    }

    const h1 = extractH1(body);
    const title = isEn ? page.title : h1 ?? page.title;
    const description = isEn ? page.description : extractLead(body) ?? page.description;

    // prev/next within this locale's page order
    const idx = localePages.indexOf(page);
    const prev = idx > 0 ? localePages[idx - 1] : null;
    const next = idx < localePages.length - 1 ? localePages[idx + 1] : null;
    const titleFor = (p) => {
      if (isEn) return p.title;
      const b = stripDirectives(readFileSync(sourcePath(locale.code, p.src), "utf8"));
      return extractH1(b) ?? p.title;
    };

    // hreflang alternates: every locale that carries this page (+ x-default = en)
    const altLocales = page.translated ? LOCALES : LOCALES.filter((l) => l.code === "en");

    const fm = [
      "---",
      `title: "${esc(title)} -- STREETS by 4DA"`,
      `description: "${esc(description)}"`,
      "layout: streets-module.njk",
      `permalink: "${pageUrl(locale.code, page.slug)}"`,
      "templateEngineOverride: md",
      'tags: ["streetsModule"]',
      `locale: "${locale.code}"`,
      `localeDir: "${locale.dir}"`,
      `slug: "${page.slug}"`,
      `prevLabel: "${esc(locale.prev)}"`,
      `nextLabel: "${esc(locale.next)}"`,
      ...(prev
        ? [`prevUrl: "${pageUrl(locale.code, prev.slug)}"`, `prevTitle: "${esc(titleFor(prev))}"`]
        : []),
      ...(next
        ? [`nextUrl: "${pageUrl(locale.code, next.slug)}"`, `nextTitle: "${esc(titleFor(next))}"`]
        : []),
      "alternates:",
      ...altLocales.map(
        (l) =>
          `  - { code: "${l.code}", label: "${esc(l.label)}", url: "${pageUrl(l.code, page.slug)}" }`,
      ),
      "---",
      "",
    ].join("\n");

    const outPath = isEn
      ? join(OUT, `${page.slug}.md`)
      : join(OUT, locale.code, `${page.slug}.md`);
    writeFileSync(outPath, fm + body);
    written++;
  });
  console.log(`${locale.code}: ${localePages.length} pages`);
}

if (failures) {
  console.error(`\n${failures} page(s) FAILED.`);
  process.exit(1);
}
console.log(`\n${written} pages rendered clean across ${LOCALES.length} locales.`);
