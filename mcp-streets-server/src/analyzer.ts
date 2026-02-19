/**
 * Project analyzer for STREETS MCP Server
 *
 * Scans a project directory for manifest files (Cargo.toml, package.json,
 * go.mod, pyproject.toml, etc.), detects the tech stack, and maps it to
 * STREETS revenue engine recommendations.
 */

import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

import type { DetectedStack, EngineRecommendation, ReadinessResult, ReadinessCategory, ReadinessItem } from "./types.js";

// =============================================================================
// Engine definitions
// =============================================================================

const ENGINE_NAMES: Record<number, string> = {
  1: "Digital Products",
  2: "Content Monetization",
  3: "Micro-SaaS",
  4: "Automation-as-a-Service",
  5: "API Products",
  6: "Consulting and Fractional CTO",
  7: "Open Source + Premium",
  8: "Data Products and Intelligence",
};

// =============================================================================
// Stack detection
// =============================================================================

/**
 * Detect the tech stack from manifest files in a directory
 */
export function detectStack(projectPath: string): DetectedStack {
  const stack: DetectedStack = {
    languages: [],
    frameworks: [],
    categories: [],
    dependencies: [],
  };

  // --- Cargo.toml (Rust) ---
  const cargoPath = join(projectPath, "Cargo.toml");
  if (existsSync(cargoPath)) {
    stack.languages.push("Rust");
    const cargo = readFileSync(cargoPath, "utf-8");

    if (cargo.includes("tauri")) stack.frameworks.push("Tauri");
    if (cargo.includes("actix") || cargo.includes("axum") || cargo.includes("warp") || cargo.includes("rocket")) {
      stack.frameworks.push("Rust Web Framework");
      stack.categories.push("web-framework");
    }
    if (cargo.includes("clap") || cargo.includes("structopt")) {
      stack.categories.push("cli-tool");
    }
    if (cargo.includes("tokio") || cargo.includes("async-std")) {
      stack.categories.push("async-runtime");
    }
    if (cargo.includes("serde")) {
      stack.categories.push("serialization");
    }

    // Extract dependency names
    const depSection = cargo.match(/\[dependencies\]([\s\S]*?)(?:\n\[|\n$)/);
    if (depSection) {
      const deps = depSection[1].match(/^(\w[\w-]*)/gm);
      if (deps) stack.dependencies.push(...deps);
    }
  }

  // --- package.json (JavaScript/TypeScript) ---
  const packagePath = join(projectPath, "package.json");
  if (existsSync(packagePath)) {
    try {
      const pkg = JSON.parse(readFileSync(packagePath, "utf-8")) as PackageJson;

      if (pkg.devDependencies?.typescript || pkg.dependencies?.typescript) {
        stack.languages.push("TypeScript");
      } else {
        stack.languages.push("JavaScript");
      }

      const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
      const depNames = Object.keys(allDeps);
      stack.dependencies.push(...depNames);

      // Detect frameworks
      if (allDeps["next"]) { stack.frameworks.push("Next.js"); stack.categories.push("web-framework"); }
      if (allDeps["react"]) stack.frameworks.push("React");
      if (allDeps["vue"]) { stack.frameworks.push("Vue"); stack.categories.push("web-framework"); }
      if (allDeps["svelte"] || allDeps["@sveltejs/kit"]) { stack.frameworks.push("Svelte"); stack.categories.push("web-framework"); }
      if (allDeps["express"] || allDeps["fastify"] || allDeps["hono"]) {
        stack.frameworks.push("Node.js Server");
        stack.categories.push("web-framework");
      }
      if (allDeps["@modelcontextprotocol/sdk"]) {
        stack.frameworks.push("MCP");
        stack.categories.push("mcp-server");
      }
      if (allDeps["electron"] || allDeps["@tauri-apps/api"]) {
        stack.categories.push("desktop-app");
      }

      // AI/ML related
      if (allDeps["openai"] || allDeps["@anthropic-ai/sdk"] || allDeps["langchain"] || allDeps["@langchain/core"]) {
        stack.categories.push("ai-ml");
      }

      // CLI tool indicators
      if (pkg.bin) {
        stack.categories.push("cli-tool");
      }
    } catch {
      // Invalid JSON, skip
    }
  }

  // --- go.mod (Go) ---
  const goModPath = join(projectPath, "go.mod");
  if (existsSync(goModPath)) {
    stack.languages.push("Go");
    const goMod = readFileSync(goModPath, "utf-8");

    if (goMod.includes("gin-gonic") || goMod.includes("gorilla/mux") || goMod.includes("labstack/echo") || goMod.includes("go-chi")) {
      stack.frameworks.push("Go Web Framework");
      stack.categories.push("web-framework");
    }
    if (goMod.includes("cobra") || goMod.includes("urfave/cli")) {
      stack.categories.push("cli-tool");
    }

    // Extract module dependencies
    const requireBlock = goMod.match(/require\s*\(([\s\S]*?)\)/);
    if (requireBlock) {
      const deps = requireBlock[1].match(/\S+/g);
      if (deps) stack.dependencies.push(...deps.filter((d) => d.includes("/")));
    }
  }

  // --- pyproject.toml / requirements.txt (Python) ---
  const pyprojectPath = join(projectPath, "pyproject.toml");
  const requirementsPath = join(projectPath, "requirements.txt");

  if (existsSync(pyprojectPath) || existsSync(requirementsPath)) {
    stack.languages.push("Python");

    let content = "";
    if (existsSync(pyprojectPath)) {
      content = readFileSync(pyprojectPath, "utf-8");
    }
    if (existsSync(requirementsPath)) {
      content += "\n" + readFileSync(requirementsPath, "utf-8");
    }

    const contentLower = content.toLowerCase();

    if (contentLower.includes("django") || contentLower.includes("flask") || contentLower.includes("fastapi")) {
      stack.frameworks.push("Python Web Framework");
      stack.categories.push("web-framework");
    }
    if (contentLower.includes("torch") || contentLower.includes("tensorflow") || contentLower.includes("transformers") || contentLower.includes("scikit")) {
      stack.categories.push("ai-ml");
    }
    if (contentLower.includes("pandas") || contentLower.includes("numpy") || contentLower.includes("polars")) {
      stack.categories.push("data-processing");
    }
    if (contentLower.includes("click") || contentLower.includes("typer") || contentLower.includes("argparse")) {
      stack.categories.push("cli-tool");
    }
    if (contentLower.includes("openai") || contentLower.includes("anthropic") || contentLower.includes("langchain")) {
      stack.categories.push("ai-ml");
    }
  }

  // Deduplicate
  stack.languages = [...new Set(stack.languages)];
  stack.frameworks = [...new Set(stack.frameworks)];
  stack.categories = [...new Set(stack.categories)];

  return stack;
}

// =============================================================================
// Engine recommendation
// =============================================================================

/**
 * Map a detected stack to ranked revenue engine recommendations
 */
export function recommendEngines(stack: DetectedStack): EngineRecommendation[] {
  const scores: Array<{ engine: number; score: number; rationale: string }> = [];

  // Engine 1: Digital Products — always viable, bonus for frameworks with boilerplate potential
  {
    let score = 0.4;
    let rationale = "Every developer can create digital products (templates, starter kits, guides).";
    if (stack.frameworks.length > 0) {
      score += 0.2;
      rationale += ` Your ${stack.frameworks.join(", ")} experience is ideal for starter kits and boilerplates.`;
    }
    if (stack.categories.includes("mcp-server")) {
      score += 0.15;
      rationale += " MCP server templates are a hot, low-competition market.";
    }
    scores.push({ engine: 1, score, rationale });
  }

  // Engine 2: Content Monetization — good for niche domains
  {
    let score = 0.3;
    let rationale = "Content monetization works for any domain knowledge.";
    if (stack.categories.includes("ai-ml")) {
      score += 0.2;
      rationale += " AI/ML expertise is highly valuable for technical content.";
    }
    if (stack.languages.length >= 2) {
      score += 0.1;
      rationale += " Multi-language experience gives you broad content appeal.";
    }
    scores.push({ engine: 2, score, rationale });
  }

  // Engine 3: Micro-SaaS — strong for web frameworks
  {
    let score = 0.2;
    let rationale = "Micro-SaaS requires a web stack and hosting.";
    if (stack.categories.includes("web-framework")) {
      score += 0.4;
      rationale = `Your ${stack.frameworks.filter((f) => f.includes("Web") || f === "Next.js" || f === "Vue" || f === "Svelte").join(", ") || "web framework"} stack is a direct fit for Micro-SaaS.`;
    }
    if (stack.categories.includes("desktop-app")) {
      score += 0.1;
      rationale += " Desktop app experience can translate to niche SaaS tools.";
    }
    scores.push({ engine: 3, score, rationale });
  }

  // Engine 4: Automation-as-a-Service — good for CLI tools, scripting
  {
    let score = 0.3;
    let rationale = "Automation services are accessible to all developers.";
    if (stack.categories.includes("cli-tool")) {
      score += 0.3;
      rationale = "Your CLI tool experience maps directly to automation-as-a-service offerings.";
    }
    if (stack.categories.includes("data-processing")) {
      score += 0.15;
      rationale += " Data processing skills are perfect for batch automation workflows.";
    }
    if (stack.languages.includes("Python")) {
      score += 0.1;
      rationale += " Python is the dominant language for automation scripts.";
    }
    scores.push({ engine: 4, score, rationale });
  }

  // Engine 5: API Products — strong for web frameworks and AI/ML
  {
    let score = 0.15;
    let rationale = "API products require backend infrastructure knowledge.";
    if (stack.categories.includes("web-framework")) {
      score += 0.25;
      rationale = "Your server-side experience is a strong foundation for API products.";
    }
    if (stack.categories.includes("ai-ml")) {
      score += 0.35;
      rationale = "AI/ML dependencies detected — wrapping models as API products is a high-margin play.";
    }
    if (stack.categories.includes("mcp-server")) {
      score += 0.15;
      rationale += " MCP server experience means you understand tool-serving architecture.";
    }
    scores.push({ engine: 5, score, rationale });
  }

  // Engine 6: Consulting and Fractional CTO — niche domains, multi-language
  {
    let score = 0.2;
    let rationale = "Consulting requires deep domain expertise.";
    if (stack.languages.length >= 2) {
      score += 0.15;
      rationale += ` Multi-language expertise (${stack.languages.join(", ")}) broadens consulting appeal.`;
    }
    if (stack.frameworks.length >= 2) {
      score += 0.15;
      rationale += " Broad framework knowledge supports architectural consulting.";
    }
    if (stack.categories.includes("ai-ml")) {
      score += 0.2;
      rationale += " AI/ML consulting is in extremely high demand.";
    }
    scores.push({ engine: 6, score, rationale });
  }

  // Engine 7: Open Source + Premium — strong for CLI tools, libraries
  {
    let score = 0.15;
    let rationale = "Open Source + Premium requires an existing project or idea for one.";
    if (stack.categories.includes("cli-tool")) {
      score += 0.3;
      rationale = "CLI tools are ideal for the Open Source + Premium model (free core, paid features/support).";
    }
    if (stack.categories.includes("mcp-server")) {
      score += 0.25;
      rationale += " MCP servers can use open-core: free basic tools, premium advanced capabilities.";
    }
    if (stack.languages.includes("Rust") || stack.languages.includes("Go")) {
      score += 0.1;
      rationale += ` ${stack.languages.includes("Rust") ? "Rust" : "Go"} projects have strong open-source communities.`;
    }
    scores.push({ engine: 7, score, rationale });
  }

  // Engine 8: Data Products and Intelligence — strong for AI/ML, data processing
  {
    let score = 0.1;
    let rationale = "Data products require data collection and processing capabilities.";
    if (stack.categories.includes("ai-ml")) {
      score += 0.35;
      rationale = "AI/ML stack detected — data products and intelligence services are a natural fit.";
    }
    if (stack.categories.includes("data-processing")) {
      score += 0.3;
      rationale = "Data processing tools detected — you already have the pipeline skills for data products.";
    }
    if (stack.categories.includes("web-framework") && stack.categories.includes("ai-ml")) {
      score += 0.1;
      rationale += " Web + AI combination enables full-stack intelligence dashboards.";
    }
    scores.push({ engine: 8, score, rationale });
  }

  // Sort by score descending and build results
  scores.sort((a, b) => b.score - a.score);

  return scores.map((s) => ({
    engine_number: s.engine,
    name: ENGINE_NAMES[s.engine],
    match_score: Math.round(s.score * 100) / 100,
    rationale: s.rationale,
    detected_stack: stack,
  }));
}

// =============================================================================
// Readiness assessment (Sovereign Setup checklist)
// =============================================================================

/**
 * Assess readiness against the Sovereign Setup checklist from Module S
 */
export function assessReadiness(projectPath: string): ReadinessResult {
  const categories: ReadinessCategory[] = [];

  // Category 1: Local LLM
  {
    const items: ReadinessItem[] = [];

    // Check for Ollama (look for common config/model indicators)
    const ollamaRunning = checkCommandExists("ollama");
    items.push({
      name: "Ollama installed",
      met: ollamaRunning,
      detail: ollamaRunning
        ? "Ollama binary found in PATH"
        : "Ollama not found in PATH. Install from https://ollama.ai",
    });

    // Check for .env or config files with LLM keys
    const hasLLMConfig = existsSync(join(projectPath, ".env")) ||
      existsSync(join(projectPath, "data", "settings.json"));
    items.push({
      name: "LLM configuration present",
      met: hasLLMConfig,
      detail: hasLLMConfig
        ? "Found .env or settings file with potential LLM configuration"
        : "No .env or settings.json found. Configure API keys or local LLM endpoints.",
    });

    const score = items.filter((i) => i.met).length / items.length;
    categories.push({ name: "local_llm", score: Math.round(score * 100), items });
  }

  // Category 2: Legal Foundation
  {
    const items: ReadinessItem[] = [];

    const hasLicense = existsSync(join(projectPath, "LICENSE")) ||
      existsSync(join(projectPath, "LICENSE.md")) ||
      existsSync(join(projectPath, "LICENSE.txt"));
    items.push({
      name: "License file present",
      met: hasLicense,
      detail: hasLicense
        ? "License file found in project root"
        : "No LICENSE file found. Add one to protect your work and define terms.",
    });

    const hasTerms = existsSync(join(projectPath, "TERMS.md")) ||
      existsSync(join(projectPath, "terms-of-service.md"));
    items.push({
      name: "Terms of service",
      met: hasTerms,
      detail: hasTerms
        ? "Terms of service document found"
        : "No terms of service found. Not required for all projects, but recommended for commercial products.",
    });

    const hasPrivacy = existsSync(join(projectPath, "PRIVACY.md")) ||
      existsSync(join(projectPath, "privacy-policy.md"));
    items.push({
      name: "Privacy policy",
      met: hasPrivacy,
      detail: hasPrivacy
        ? "Privacy policy document found"
        : "No privacy policy found. Required if collecting any user data.",
    });

    const score = items.filter((i) => i.met).length / items.length;
    categories.push({ name: "legal_foundation", score: Math.round(score * 100), items });
  }

  // Category 3: Development Environment
  {
    const items: ReadinessItem[] = [];

    const hasGit = existsSync(join(projectPath, ".git"));
    items.push({
      name: "Git repository initialized",
      met: hasGit,
      detail: hasGit
        ? "Git repository found"
        : "No .git directory. Initialize with: git init",
    });

    const hasCI = existsSync(join(projectPath, ".github", "workflows")) ||
      existsSync(join(projectPath, ".gitlab-ci.yml")) ||
      existsSync(join(projectPath, ".circleci"));
    items.push({
      name: "CI/CD pipeline configured",
      met: hasCI,
      detail: hasCI
        ? "CI/CD configuration found"
        : "No CI/CD pipeline detected. Set up GitHub Actions, GitLab CI, or similar.",
    });

    const hasReadme = existsSync(join(projectPath, "README.md")) ||
      existsSync(join(projectPath, "readme.md"));
    items.push({
      name: "README documentation",
      met: hasReadme,
      detail: hasReadme
        ? "README.md found"
        : "No README.md found. Essential for any project, especially commercial ones.",
    });

    const hasEditorConfig = existsSync(join(projectPath, ".editorconfig")) ||
      existsSync(join(projectPath, ".prettierrc")) ||
      existsSync(join(projectPath, ".prettierrc.json")) ||
      existsSync(join(projectPath, "biome.json"));
    items.push({
      name: "Code formatting configured",
      met: hasEditorConfig,
      detail: hasEditorConfig
        ? "Code formatting configuration found"
        : "No .editorconfig, .prettierrc, or biome.json found. Consistent formatting matters for products.",
    });

    const score = items.filter((i) => i.met).length / items.length;
    categories.push({ name: "development_env", score: Math.round(score * 100), items });
  }

  // Category 4: Revenue Infrastructure
  {
    const items: ReadinessItem[] = [];

    // Check for payment-related dependencies
    const pkgPath = join(projectPath, "package.json");
    let hasPaymentDep = false;
    if (existsSync(pkgPath)) {
      try {
        const pkg = JSON.parse(readFileSync(pkgPath, "utf-8")) as PackageJson;
        const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
        hasPaymentDep = !!(allDeps["stripe"] || allDeps["@stripe/stripe-js"] || allDeps["lemonsqueezy"]);
      } catch {
        // Invalid JSON
      }
    }
    items.push({
      name: "Payment integration",
      met: hasPaymentDep,
      detail: hasPaymentDep
        ? "Payment SDK dependency found (Stripe or Lemon Squeezy)"
        : "No payment SDK detected in dependencies. Add Stripe or Lemon Squeezy when ready to monetize.",
    });

    // Check for landing page or marketing assets
    const hasLandingPage = existsSync(join(projectPath, "landing")) ||
      existsSync(join(projectPath, "site")) ||
      existsSync(join(projectPath, "www")) ||
      existsSync(join(projectPath, "public", "index.html"));
    items.push({
      name: "Landing page / marketing site",
      met: hasLandingPage,
      detail: hasLandingPage
        ? "Landing page or marketing site directory found"
        : "No dedicated landing page detected. You will need one to sell products.",
    });

    // Check for analytics
    let hasAnalytics = false;
    if (existsSync(pkgPath)) {
      try {
        const pkg = JSON.parse(readFileSync(pkgPath, "utf-8")) as PackageJson;
        const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
        hasAnalytics = !!(allDeps["plausible-tracker"] || allDeps["@vercel/analytics"] || allDeps["posthog-js"]);
      } catch {
        // Invalid JSON
      }
    }
    items.push({
      name: "Analytics integration",
      met: hasAnalytics,
      detail: hasAnalytics
        ? "Analytics SDK found in dependencies"
        : "No analytics SDK detected. Add Plausible, PostHog, or Vercel Analytics to track conversions.",
    });

    const score = items.filter((i) => i.met).length / items.length;
    categories.push({ name: "revenue_infrastructure", score: Math.round(score * 100), items });
  }

  // Calculate overall score
  const overall = categories.length > 0
    ? Math.round(categories.reduce((sum, c) => sum + c.score, 0) / categories.length)
    : 0;

  return {
    overall_score: overall,
    categories,
  };
}

// =============================================================================
// Helpers
// =============================================================================

interface PackageJson {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
  bin?: Record<string, string> | string;
}

/**
 * Simple check if a command exists (non-blocking heuristic)
 * On Windows, check common installation paths
 */
function checkCommandExists(command: string): boolean {
  const { execSync } = require("child_process") as typeof import("child_process");
  try {
    const cmd = process.platform === "win32" ? `where ${command}` : `which ${command}`;
    execSync(cmd, { stdio: "pipe" });
    return true;
  } catch {
    return false;
  }
}
