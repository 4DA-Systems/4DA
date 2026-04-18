// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Project Scanner for MCP Standalone Mode
 *
 * Lightweight project scanner that detects languages, frameworks,
 * dependencies, and topics from the current working directory.
 * Runs on startup when no existing 4DA database is found.
 *
 * Design goals:
 * - Fast (<2 seconds) — only checks immediate directory
 * - No recursion — reads manifest files at project root
 * - No dependencies — uses only Node.js built-ins
 */

import * as fs from "node:fs";
import * as path from "node:path";

// =============================================================================
// Types
// =============================================================================

export interface ProjectScanResult {
  languages: string[];
  frameworks: string[];
  dependencies: string[];
  devDependencies: string[];
  topics: string[];
  projectName: string;
  projectPath: string;
}

// =============================================================================
// Framework Detection Maps
// =============================================================================

/** npm package name -> framework label */
const NPM_FRAMEWORK_MAP: Record<string, string> = {
  react: "react",
  "react-dom": "react",
  vue: "vue",
  next: "next.js",
  nuxt: "nuxt",
  "@angular/core": "angular",
  svelte: "svelte",
  "@sveltejs/kit": "sveltekit",
  astro: "astro",
  remix: "remix",
  express: "express",
  fastify: "fastify",
  hono: "hono",
  elysia: "elysia",
  "react-native": "react-native",
  expo: "expo",
  electron: "electron",
  tauri: "tauri",
  "@tauri-apps/api": "tauri",
  "@tauri-apps/cli": "tauri",
};

/** npm devDependency name -> language/tool */
const NPM_DEVTOOL_MAP: Record<string, string> = {
  typescript: "typescript",
  vite: "vite",
  vitest: "vitest",
  jest: "jest",
  webpack: "webpack",
  esbuild: "esbuild",
  rollup: "rollup",
  tailwindcss: "tailwind",
  "@tailwindcss/vite": "tailwind",
  prisma: "prisma",
  drizzle: "drizzle",
};

/** Rust crate name -> framework label */
const RUST_FRAMEWORK_MAP: Record<string, string> = {
  tauri: "tauri",
  "tauri-build": "tauri",
  actix: "actix",
  "actix-web": "actix",
  axum: "axum",
  rocket: "rocket",
  warp: "warp",
  tokio: "tokio",
  serde: "serde",
  diesel: "diesel",
  sqlx: "sqlx",
  rusqlite: "sqlite",
  reqwest: "reqwest",
  tonic: "grpc",
  leptos: "leptos",
  yew: "yew",
  dioxus: "dioxus",
};

// =============================================================================
// Scanner
// =============================================================================

/**
 * Scan a directory for project context.
 * Only reads manifest files at the root level — no recursion.
 */
export function scanCurrentProject(cwd: string): ProjectScanResult {
  const result: ProjectScanResult = {
    languages: [],
    frameworks: [],
    dependencies: [],
    devDependencies: [],
    topics: [],
    projectName: path.basename(cwd),
    projectPath: cwd,
  };

  const langSet = new Set<string>();
  const fwSet = new Set<string>();

  // -------------------------------------------------------------------------
  // 1. package.json (JavaScript / TypeScript ecosystem)
  // -------------------------------------------------------------------------
  const pkgPath = path.join(cwd, "package.json");
  if (fs.existsSync(pkgPath)) {
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf-8"));
      langSet.add("javascript");

      if (pkg.name) {
        result.projectName = pkg.name;
      }

      // Production dependencies
      if (pkg.dependencies && typeof pkg.dependencies === "object") {
        const depNames = Object.keys(pkg.dependencies);
        result.dependencies.push(...depNames);

        for (const dep of depNames) {
          if (dep in NPM_FRAMEWORK_MAP) {
            fwSet.add(NPM_FRAMEWORK_MAP[dep]);
          }
        }
      }

      // Dev dependencies
      if (pkg.devDependencies && typeof pkg.devDependencies === "object") {
        const devDepNames = Object.keys(pkg.devDependencies);
        result.devDependencies.push(...devDepNames);

        for (const dep of devDepNames) {
          if (dep in NPM_DEVTOOL_MAP) {
            const tool = NPM_DEVTOOL_MAP[dep];
            if (tool === "typescript") {
              langSet.add("typescript");
            } else {
              fwSet.add(tool);
            }
          }
          // Also check frameworks in devDependencies (common for meta-frameworks)
          if (dep in NPM_FRAMEWORK_MAP) {
            fwSet.add(NPM_FRAMEWORK_MAP[dep]);
          }
        }
      }
    } catch {
      // Malformed package.json — skip
    }
  }

  // -------------------------------------------------------------------------
  // 2. Cargo.toml (Rust ecosystem)
  // -------------------------------------------------------------------------
  const cargoPath = path.join(cwd, "Cargo.toml");
  if (fs.existsSync(cargoPath)) {
    try {
      const content = fs.readFileSync(cargoPath, "utf-8");
      langSet.add("rust");

      // Extract project name from [package] section
      const nameMatch = content.match(/^\[package\]\s*\n(?:.*\n)*?name\s*=\s*"([^"]+)"/m);
      if (nameMatch) {
        result.projectName = nameMatch[1];
      }

      // Parse [dependencies] and [dev-dependencies] sections
      parseTOMLDependencies(content, "[dependencies]", result.dependencies, fwSet, RUST_FRAMEWORK_MAP);
      parseTOMLDependencies(content, "[dev-dependencies]", result.devDependencies, fwSet, RUST_FRAMEWORK_MAP);
      parseTOMLDependencies(content, "[build-dependencies]", result.devDependencies, fwSet, RUST_FRAMEWORK_MAP);
    } catch {
      // Malformed Cargo.toml — skip
    }
  }

  // -------------------------------------------------------------------------
  // 3. pyproject.toml / requirements.txt (Python ecosystem)
  // -------------------------------------------------------------------------
  const pyprojectPath = path.join(cwd, "pyproject.toml");
  const requirementsPath = path.join(cwd, "requirements.txt");
  const setupPyPath = path.join(cwd, "setup.py");

  if (fs.existsSync(pyprojectPath)) {
    langSet.add("python");
    try {
      const content = fs.readFileSync(pyprojectPath, "utf-8");
      // Extract dependencies from pyproject.toml
      const depsMatch = content.match(/dependencies\s*=\s*\[([\s\S]*?)\]/);
      if (depsMatch) {
        const deps = depsMatch[1].match(/"([^">=<!\s]+)/g);
        if (deps) {
          for (const dep of deps) {
            const name = dep.replace(/^"/, "");
            result.dependencies.push(name);
            detectPythonFramework(name, fwSet);
          }
        }
      }
    } catch {
      // Skip
    }
  } else if (fs.existsSync(requirementsPath)) {
    langSet.add("python");
    try {
      const content = fs.readFileSync(requirementsPath, "utf-8");
      for (const line of content.split("\n")) {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith("#") && !trimmed.startsWith("-")) {
          const name = trimmed.split(/[>=<!\[]/)[0].trim();
          if (name) {
            result.dependencies.push(name);
            detectPythonFramework(name, fwSet);
          }
        }
      }
    } catch {
      // Skip
    }
  } else if (fs.existsSync(setupPyPath)) {
    langSet.add("python");
  }

  // -------------------------------------------------------------------------
  // 4. go.mod (Go ecosystem)
  // -------------------------------------------------------------------------
  const goModPath = path.join(cwd, "go.mod");
  if (fs.existsSync(goModPath)) {
    langSet.add("go");
    try {
      const content = fs.readFileSync(goModPath, "utf-8");
      // Extract module name
      const moduleMatch = content.match(/^module\s+(\S+)/m);
      if (moduleMatch) {
        const parts = moduleMatch[1].split("/");
        result.projectName = parts[parts.length - 1];
      }
      // Extract require block dependencies
      const requireMatch = content.match(/require\s*\(([\s\S]*?)\)/);
      if (requireMatch) {
        for (const line of requireMatch[1].split("\n")) {
          const depMatch = line.trim().match(/^(\S+)\s+/);
          if (depMatch && !depMatch[1].startsWith("//")) {
            const dep = depMatch[1];
            result.dependencies.push(dep);
            detectGoFramework(dep, fwSet);
          }
        }
      }
    } catch {
      // Skip
    }
  }

  // -------------------------------------------------------------------------
  // 5. tsconfig.json presence (confirms TypeScript even without package.json)
  // -------------------------------------------------------------------------
  if (fs.existsSync(path.join(cwd, "tsconfig.json"))) {
    langSet.add("typescript");
  }

  // -------------------------------------------------------------------------
  // 6. Dockerfile / docker-compose.yml (container detection)
  // -------------------------------------------------------------------------
  if (
    fs.existsSync(path.join(cwd, "Dockerfile")) ||
    fs.existsSync(path.join(cwd, "docker-compose.yml")) ||
    fs.existsSync(path.join(cwd, "docker-compose.yaml"))
  ) {
    fwSet.add("docker");
  }

  // -------------------------------------------------------------------------
  // Finalize
  // -------------------------------------------------------------------------
  result.languages = [...langSet];
  result.frameworks = [...fwSet];

  // Deduplicate dependencies
  result.dependencies = [...new Set(result.dependencies)];
  result.devDependencies = [...new Set(result.devDependencies)];

  // Generate topics from detected stack
  result.topics = [
    ...new Set([
      ...result.languages,
      ...result.frameworks,
      // Add high-signal dependency names as topics (only well-known ones)
      ...result.dependencies.filter(isNotableTopic),
    ]),
  ];

  return result;
}

// =============================================================================
// Helpers
// =============================================================================

/**
 * Parse a TOML section for dependency names.
 * Simple line-by-line parsing — handles `name = "version"` and `name = { ... }` forms.
 */
function parseTOMLDependencies(
  content: string,
  sectionHeader: string,
  target: string[],
  frameworkSet: Set<string>,
  frameworkMap: Record<string, string>,
): void {
  const headerIndex = content.indexOf(sectionHeader);
  if (headerIndex === -1) return;

  const afterHeader = content.substring(headerIndex + sectionHeader.length);
  for (const line of afterHeader.split("\n")) {
    const trimmed = line.trim();
    // Stop at next section
    if (trimmed.startsWith("[") && trimmed !== sectionHeader) break;
    // Skip empty lines and comments
    if (!trimmed || trimmed.startsWith("#")) continue;
    // Match dependency line: `name = ...`
    const depMatch = trimmed.match(/^([a-zA-Z_][\w-]*)\s*=/);
    if (depMatch) {
      const name = depMatch[1];
      target.push(name);
      if (name in frameworkMap) {
        frameworkSet.add(frameworkMap[name]);
      }
    }
  }
}

/** Detect Python framework from package name */
function detectPythonFramework(name: string, fwSet: Set<string>): void {
  const pythonFrameworks: Record<string, string> = {
    django: "django",
    flask: "flask",
    fastapi: "fastapi",
    starlette: "starlette",
    uvicorn: "uvicorn",
    celery: "celery",
    sqlalchemy: "sqlalchemy",
    pytorch: "pytorch",
    torch: "pytorch",
    tensorflow: "tensorflow",
    numpy: "numpy",
    pandas: "pandas",
    streamlit: "streamlit",
  };
  if (name in pythonFrameworks) {
    fwSet.add(pythonFrameworks[name]);
  }
}

/** Detect Go framework from module path */
function detectGoFramework(dep: string, fwSet: Set<string>): void {
  if (dep.includes("gin-gonic")) fwSet.add("gin");
  if (dep.includes("gorilla/mux")) fwSet.add("gorilla");
  if (dep.includes("labstack/echo")) fwSet.add("echo");
  if (dep.includes("gofiber/fiber")) fwSet.add("fiber");
  if (dep.includes("grpc")) fwSet.add("grpc");
}

/**
 * Filter for well-known packages worth adding as topics.
 * Avoids noise from utility packages.
 */
function isNotableTopic(name: string): boolean {
  const notable = new Set([
    // Databases
    "sqlite3", "better-sqlite3", "pg", "mysql2", "mongodb", "mongoose", "redis",
    "prisma", "drizzle-orm", "typeorm", "sequelize", "knex",
    // Auth
    "passport", "next-auth", "lucia", "clerk",
    // APIs
    "graphql", "trpc", "@trpc/server", "openapi",
    // AI/ML
    "openai", "@anthropic-ai/sdk", "langchain", "llamaindex",
    // Testing
    "vitest", "jest", "mocha", "playwright", "cypress",
    // Infrastructure
    "docker", "kubernetes",
    // Notable Rust crates
    "serde", "tokio", "axum", "actix-web", "sqlx", "diesel",
    "reqwest", "rusqlite", "clap", "tracing",
  ]);
  return notable.has(name);
}
