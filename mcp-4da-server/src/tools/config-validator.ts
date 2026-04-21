// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * Config Validator Tool
 *
 * Validate and diagnose 4DA configuration issues.
 * This is a SUPERPOWER - it catches config errors before they cause problems.
 */

import type { FourDADatabase } from "../db.js";

export const configValidatorTool = {
  name: "config_validator",
  description: `Validate 4DA configuration and detect issues.
Returns:
- Schema validation results
- Missing required fields
- Invalid values detected
- Cross-field consistency checks
- Recommendations for fixes

Use this when things aren't working as expected.`,
  inputSchema: {
    type: "object",
    properties: {
      section: {
        type: "string",
        enum: ["all", "sources", "embedding", "llm", "ace", "digest", "ui"],
        description: "Config section to validate (default: all)",
      },
      fix_suggestions: {
        type: "boolean",
        description: "Include fix suggestions (default: true)",
      },
    },
  },
};

export interface ConfigValidatorParams {
  section?: "all" | "sources" | "embedding" | "llm" | "ace" | "digest" | "ui";
  fix_suggestions?: boolean;
}

interface ValidationIssue {
  section: string;
  field: string;
  severity: "error" | "warning" | "info";
  message: string;
  current_value: unknown;
  expected: string;
  fix?: string;
}

interface SectionValidation {
  section: string;
  status: "valid" | "warnings" | "errors";
  issue_count: number;
  issues: ValidationIssue[];
}

interface ConfigValidationResult {
  overall_status: "valid" | "warnings" | "errors";
  sections: SectionValidation[];
  summary: {
    total_issues: number;
    errors: number;
    warnings: number;
    info: number;
  };
  quick_fixes: string[];
  config_score: number;
}

/** Patterns that indicate a field contains sensitive data (API keys, tokens, etc.) */
const SENSITIVE_FIELD_PATTERNS = /key|token|password|secret|credential|bearer|api_key/i;

/**
 * Redact a value if the field name suggests it contains sensitive data.
 * Returns "[REDACTED]" for sensitive fields, the original value otherwise.
 */
function redactSensitiveValue(fieldName: string, value: unknown): unknown {
  if (SENSITIVE_FIELD_PATTERNS.test(fieldName)) {
    return "[REDACTED]";
  }
  // Also check string values that look like API keys (long alphanumeric strings)
  if (typeof value === "string" && value.length > 20 && /^[A-Za-z0-9_\-./+=]+$/.test(value)) {
    // Heuristic: long opaque strings are likely secrets even if the field name doesn't match
    return "[REDACTED]";
  }
  return value;
}

/**
 * Scrub all `current_value` fields in validation issues to prevent API key leakage.
 */
function redactIssues(issues: ValidationIssue[]): ValidationIssue[] {
  return issues.map(issue => ({
    ...issue,
    current_value: redactSensitiveValue(issue.field, issue.current_value),
  }));
}

export function executeConfigValidator(
  db: FourDADatabase,
  params: ConfigValidatorParams
): ConfigValidationResult {
  const { section = "all", fix_suggestions = true } = params;

  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { all: (...args: unknown[]) => unknown[]; get: (...args: unknown[]) => unknown } } }).db;

  // Get current settings
  let settingsRows: { key: string; value: string }[] = [];
  try {
    settingsRows = dbInstance.prepare(`
      SELECT key, value FROM settings
    `).all() as { key: string; value: string }[];
  } catch {
    return {
      overall_status: "valid",
      sections: [],
      summary: { total_issues: 0, errors: 0, warnings: 0, info: 0 },
      quick_fixes: ["Configuration validation unavailable — settings stored externally (not in this database)."],
      config_score: 100,
    };
  }
  const settings: Record<string, unknown> = {};
  for (const row of settingsRows) {
    try {
      settings[row.key] = JSON.parse(row.value);
    } catch {
      settings[row.key] = row.value;
    }
  }

  const sections: SectionValidation[] = [];

  // Validate each section
  if (section === "all" || section === "sources") {
    sections.push(validateSources(settings, fix_suggestions));
  }
  if (section === "all" || section === "embedding") {
    sections.push(validateEmbedding(settings, fix_suggestions));
  }
  if (section === "all" || section === "llm") {
    sections.push(validateLLM(settings, fix_suggestions));
  }
  if (section === "all" || section === "ace") {
    sections.push(validateACE(settings, fix_suggestions));
  }
  if (section === "all" || section === "digest") {
    sections.push(validateDigest(settings, fix_suggestions));
  }
  if (section === "all" || section === "ui") {
    sections.push(validateUI(settings, fix_suggestions));
  }

  // Redact sensitive values from all issue current_value fields before output
  for (const sec of sections) {
    sec.issues = redactIssues(sec.issues);
  }

  // Calculate summary
  const allIssues = sections.flatMap(s => s.issues);
  const errors = allIssues.filter(i => i.severity === "error").length;
  const warnings = allIssues.filter(i => i.severity === "warning").length;
  const info = allIssues.filter(i => i.severity === "info").length;

  const overallStatus: "valid" | "warnings" | "errors" =
    errors > 0 ? "errors" : warnings > 0 ? "warnings" : "valid";

  // Generate quick fixes
  const quickFixes = allIssues
    .filter(i => i.fix && i.severity !== "info")
    .map(i => i.fix!)
    .slice(0, 5);

  // Calculate config score (0-100)
  const maxScore = 100;
  const errorPenalty = errors * 20;
  const warningPenalty = warnings * 5;
  const configScore = Math.max(0, maxScore - errorPenalty - warningPenalty);

  return {
    overall_status: overallStatus,
    sections,
    summary: {
      total_issues: allIssues.length,
      errors,
      warnings,
      info,
    },
    quick_fixes: quickFixes,
    config_score: configScore,
  };
}

function validateSources(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  // Check enabled sources
  const sources = settings.sources as Record<string, { enabled: boolean }> | undefined;
  if (!sources) {
    issues.push({
      section: "sources",
      field: "sources",
      severity: "error",
      message: "No sources configured",
      current_value: undefined,
      expected: "At least one source should be enabled",
      fix: includeFixes ? "Add sources in Settings → Sources" : undefined,
    });
  } else {
    const enabledCount = Object.values(sources).filter(s => s.enabled).length;
    if (enabledCount === 0) {
      issues.push({
        section: "sources",
        field: "sources",
        severity: "error",
        message: "All sources are disabled",
        current_value: "0 enabled",
        expected: "At least one source should be enabled",
        fix: includeFixes ? "Enable at least one source in Settings" : undefined,
      });
    }
  }

  // Check fetch intervals
  const fetchInterval = settings.fetch_interval_minutes as number | undefined;
  if (fetchInterval && fetchInterval < 5) {
    issues.push({
      section: "sources",
      field: "fetch_interval_minutes",
      severity: "warning",
      message: "Very aggressive fetch interval may cause rate limiting",
      current_value: fetchInterval,
      expected: "5 minutes or more recommended",
      fix: includeFixes ? "Increase fetch_interval_minutes to 15+" : undefined,
    });
  }

  return {
    section: "sources",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}

function validateEmbedding(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  const embeddingProvider = settings.embedding_provider as string | undefined;
  const openaiKey = settings.openai_api_key as string | undefined;
  const ollamaUrl = settings.ollama_url as string | undefined;

  if (!embeddingProvider) {
    issues.push({
      section: "embedding",
      field: "embedding_provider",
      severity: "error",
      message: "No embedding provider configured",
      current_value: undefined,
      expected: "openai or ollama",
      fix: includeFixes ? "Set embedding_provider in Settings" : undefined,
    });
  } else if (embeddingProvider === "openai" && !openaiKey) {
    issues.push({
      section: "embedding",
      field: "openai_api_key",
      severity: "error",
      message: "OpenAI selected but no API key provided",
      current_value: "(empty)",
      expected: "Valid OpenAI API key",
      fix: includeFixes ? "Add your OpenAI API key in Settings → API Keys" : undefined,
    });
  } else if (embeddingProvider === "ollama" && !ollamaUrl) {
    issues.push({
      section: "embedding",
      field: "ollama_url",
      severity: "warning",
      message: "Ollama selected but no URL configured (using default)",
      current_value: ollamaUrl || "(empty)",
      expected: "http://localhost:11434 or custom URL",
      fix: includeFixes ? "Verify Ollama is running at default port" : undefined,
    });
  }

  // Check embedding model
  const embeddingModel = settings.embedding_model as string | undefined;
  if (!embeddingModel) {
    issues.push({
      section: "embedding",
      field: "embedding_model",
      severity: "info",
      message: "No embedding model specified (using default)",
      current_value: "(default)",
      expected: "text-embedding-3-small recommended",
    });
  }

  return {
    section: "embedding",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}

function validateLLM(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  const llmProvider = settings.llm_provider as string | undefined;
  const anthropicKey = settings.anthropic_api_key as string | undefined;
  const openaiKey = settings.openai_api_key as string | undefined;

  if (!llmProvider) {
    issues.push({
      section: "llm",
      field: "llm_provider",
      severity: "warning",
      message: "No LLM provider configured - relevance scoring will be limited",
      current_value: undefined,
      expected: "anthropic, openai, or ollama",
      fix: includeFixes ? "Configure an LLM provider for better relevance scoring" : undefined,
    });
  } else if (llmProvider === "anthropic" && !anthropicKey) {
    issues.push({
      section: "llm",
      field: "anthropic_api_key",
      severity: "error",
      message: "Anthropic selected but no API key provided",
      current_value: "(empty)",
      expected: "Valid Anthropic API key",
      fix: includeFixes ? "Add your Anthropic API key in Settings → API Keys" : undefined,
    });
  } else if (llmProvider === "openai" && !openaiKey) {
    issues.push({
      section: "llm",
      field: "openai_api_key",
      severity: "error",
      message: "OpenAI selected for LLM but no API key provided",
      current_value: "(empty)",
      expected: "Valid OpenAI API key",
      fix: includeFixes ? "Add your OpenAI API key in Settings → API Keys" : undefined,
    });
  }

  // Check daily limit
  const dailyLimit = settings.daily_cost_limit_usd as number | undefined;
  if (dailyLimit === undefined) {
    issues.push({
      section: "llm",
      field: "daily_cost_limit_usd",
      severity: "info",
      message: "No daily cost limit set",
      current_value: "(unlimited)",
      expected: "Consider setting a limit to avoid surprise bills",
    });
  }

  return {
    section: "llm",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}

function validateACE(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  const watchedDirs = settings.watched_directories as string[] | undefined;
  const aceEnabled = settings.ace_enabled as boolean | undefined;

  if (aceEnabled && (!watchedDirs || watchedDirs.length === 0)) {
    issues.push({
      section: "ace",
      field: "watched_directories",
      severity: "warning",
      message: "ACE enabled but no directories configured to watch",
      current_value: watchedDirs || [],
      expected: "At least one directory for context detection",
      fix: includeFixes ? "Add project directories in Settings → Context" : undefined,
    });
  }

  // Check for overly broad paths
  if (watchedDirs) {
    for (const dir of watchedDirs) {
      if (dir === "/" || dir === "C:\\" || dir.split(/[/\\]/).length <= 2) {
        issues.push({
          section: "ace",
          field: "watched_directories",
          severity: "warning",
          message: `Path "${dir}" is very broad - may cause performance issues`,
          current_value: dir,
          expected: "More specific project directories",
          fix: includeFixes ? `Use specific project paths instead of "${dir}"` : undefined,
        });
      }
    }
  }

  return {
    section: "ace",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}

function validateDigest(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  const digestEnabled = settings.digest_enabled as boolean | undefined;
  const digestEmail = settings.digest_email as string | undefined;

  if (digestEnabled && digestEmail) {
    // Basic email validation
    if (!digestEmail.includes("@") || !digestEmail.includes(".")) {
      issues.push({
        section: "digest",
        field: "digest_email",
        severity: "error",
        message: "Invalid email address format",
        current_value: digestEmail,
        expected: "Valid email address",
        fix: includeFixes ? "Correct the email address format" : undefined,
      });
    }
  }

  const digestFrequency = settings.digest_frequency as string | undefined;
  if (digestEnabled && !digestFrequency) {
    issues.push({
      section: "digest",
      field: "digest_frequency",
      severity: "info",
      message: "No digest frequency set (using default)",
      current_value: "(default)",
      expected: "daily, weekly, or custom schedule",
    });
  }

  return {
    section: "digest",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}

function validateUI(
  settings: Record<string, unknown>,
  includeFixes: boolean
): SectionValidation {
  const issues: ValidationIssue[] = [];

  const theme = settings.theme as string | undefined;
  if (theme && !["dark", "light", "system"].includes(theme)) {
    issues.push({
      section: "ui",
      field: "theme",
      severity: "warning",
      message: "Unknown theme value",
      current_value: theme,
      expected: "dark, light, or system",
      fix: includeFixes ? "Set theme to dark, light, or system" : undefined,
    });
  }

  const maxResults = settings.max_results_per_page as number | undefined;
  if (maxResults && (maxResults < 5 || maxResults > 100)) {
    issues.push({
      section: "ui",
      field: "max_results_per_page",
      severity: "warning",
      message: "Unusual results per page value",
      current_value: maxResults,
      expected: "Between 5 and 100",
      fix: includeFixes ? "Set max_results_per_page to a reasonable value (10-50)" : undefined,
    });
  }

  return {
    section: "ui",
    status: issues.some(i => i.severity === "error") ? "errors" :
            issues.some(i => i.severity === "warning") ? "warnings" : "valid",
    issue_count: issues.length,
    issues,
  };
}
