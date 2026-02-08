/**
 * LLM Client for 4DA MCP Server
 *
 * Provides intelligence synthesis over raw tool data.
 * Supports Anthropic Claude, OpenAI, and Ollama (local).
 *
 * Features:
 * - Tiered model selection (use smaller models for simpler tasks)
 * - Optimized prompts for local models
 * - Automatic fallback
 */

// =============================================================================
// Types
// =============================================================================

export interface LLMConfig {
  provider: "anthropic" | "openai" | "ollama" | null;
  anthropic_api_key?: string;
  openai_api_key?: string;
  ollama_url?: string;
  model?: string;
  // Tiered models for Ollama
  model_light?: string;  // For simple tasks (summarization)
  model_heavy?: string;  // For complex tasks (reasoning)
}

export type TaskComplexity = "light" | "medium" | "heavy";

export interface SynthesisRequest {
  system: string;
  prompt: string;
  max_tokens?: number;
  complexity?: TaskComplexity;  // Determines which model to use
}

export interface SynthesisResult {
  synthesis: string;
  model_used: string;
  tokens_used?: number;
  latency_ms?: number;
}

// =============================================================================
// Model Tiers - Optimized for 16GB VRAM (RTX 4080 Super)
// =============================================================================

/**
 * Default models for different task complexities
 * Optimized for 16GB VRAM
 */
export const DEFAULT_OLLAMA_MODELS = {
  // Simple tasks: summarization, formatting, basic extraction
  light: "llama3.1:8b-instruct-q8_0",

  // Medium tasks: analysis, recommendations
  medium: "qwen2.5:14b-instruct-q5_K_M",

  // Complex tasks: reasoning, predictions, pattern recognition
  heavy: "qwen2.5:14b-instruct-q5_K_M",
};

/**
 * Task complexity by tool
 */
export const TOOL_COMPLEXITY: Record<string, TaskComplexity> = {
  daily_briefing: "medium",      // Needs to synthesize multiple themes
  score_autopsy: "heavy",        // Needs to reason about score correctness
  trend_analysis: "heavy",       // Needs pattern recognition + prediction
  context_analysis: "medium",    // Structured advice
  topic_connections: "heavy",    // Graph reasoning
};

// =============================================================================
// Configuration
// =============================================================================

import { readFileSync, existsSync } from "fs";
import { join, dirname } from "path";

/**
 * Find settings.json by looking in common locations
 */
function findSettingsFile(): string | null {
  // Check environment variable first
  const envPath = process.env.FOURDA_SETTINGS_PATH;
  if (envPath && existsSync(envPath)) {
    return envPath;
  }

  // Check relative to database path
  const dbPath = process.env.FOURDA_DB_PATH;
  if (dbPath) {
    const settingsPath = join(dirname(dbPath), "settings.json");
    if (existsSync(settingsPath)) {
      return settingsPath;
    }
  }

  // Check common locations
  const commonPaths = [
    "/mnt/d/4da-v3/data/settings.json",
    join(process.cwd(), "data/settings.json"),
    join(process.cwd(), "../data/settings.json"),
  ];

  for (const path of commonPaths) {
    if (existsSync(path)) {
      return path;
    }
  }

  return null;
}

/**
 * Get LLM configuration from settings.json file
 */
export function getLLMConfig(
  _db?: { prepare: (sql: string) => { get: (...args: unknown[]) => unknown } }
): LLMConfig {
  const settingsPath = findSettingsFile();

  if (!settingsPath) {
    // Return environment-based config as fallback
    return {
      provider: (process.env.LLM_PROVIDER as LLMConfig["provider"]) || null,
      anthropic_api_key: process.env.ANTHROPIC_API_KEY,
      openai_api_key: process.env.OPENAI_API_KEY,
      ollama_url: process.env.OLLAMA_URL || "http://localhost:11434",
      model: process.env.LLM_MODEL,
      model_light: process.env.LLM_MODEL_LIGHT,
      model_heavy: process.env.LLM_MODEL_HEAVY,
    };
  }

  try {
    const settings = JSON.parse(readFileSync(settingsPath, "utf-8"));
    const llm = settings.llm || {};

    return {
      provider: llm.provider as LLMConfig["provider"],
      anthropic_api_key: llm.api_key || llm.anthropic_api_key,
      openai_api_key: llm.openai_api_key,
      ollama_url: llm.base_url || llm.ollama_url || "http://localhost:11434",
      model: llm.model,
      model_light: llm.model_light,
      model_heavy: llm.model_heavy,
    };
  } catch (error) {
    console.error("Failed to read settings.json:", error);
    return { provider: null };
  }
}

/**
 * Check if LLM synthesis is available
 */
export function canSynthesize(config: LLMConfig): boolean {
  if (!config.provider) return false;

  switch (config.provider) {
    case "anthropic":
      return !!config.anthropic_api_key;
    case "openai":
      return !!config.openai_api_key;
    case "ollama":
      return true; // Ollama doesn't need a key
    default:
      return false;
  }
}

/**
 * Get the appropriate model for task complexity
 */
function getModelForComplexity(config: LLMConfig, complexity: TaskComplexity): string {
  if (config.provider !== "ollama") {
    // Cloud providers use the configured model
    return config.model || (config.provider === "anthropic" ? "claude-3-5-sonnet-20241022" : "gpt-4o");
  }

  // Ollama: use tiered models
  switch (complexity) {
    case "light":
      return config.model_light || config.model || DEFAULT_OLLAMA_MODELS.light;
    case "medium":
      return config.model || DEFAULT_OLLAMA_MODELS.medium;
    case "heavy":
      return config.model_heavy || config.model || DEFAULT_OLLAMA_MODELS.heavy;
  }
}

// =============================================================================
// Synthesis
// =============================================================================

/**
 * Synthesize insights using LLM
 */
export async function synthesize(
  config: LLMConfig,
  request: SynthesisRequest
): Promise<SynthesisResult> {
  if (!canSynthesize(config)) {
    throw new Error("LLM not configured - cannot synthesize");
  }

  const startTime = Date.now();
  const complexity = request.complexity || "medium";

  let result: SynthesisResult;

  switch (config.provider) {
    case "anthropic":
      result = await synthesizeAnthropic(config, request);
      break;
    case "openai":
      result = await synthesizeOpenAI(config, request);
      break;
    case "ollama":
      result = await synthesizeOllama(config, request, complexity);
      break;
    default:
      throw new Error(`Unknown LLM provider: ${config.provider}`);
  }

  result.latency_ms = Date.now() - startTime;
  return result;
}

/**
 * Anthropic Claude synthesis
 */
async function synthesizeAnthropic(
  config: LLMConfig,
  request: SynthesisRequest
): Promise<SynthesisResult> {
  const model = config.model || "claude-3-5-sonnet-20241022";
  const maxTokens = request.max_tokens || 1024;

  const response = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": config.anthropic_api_key!,
      "anthropic-version": "2023-06-01",
    },
    body: JSON.stringify({
      model,
      max_tokens: maxTokens,
      system: request.system,
      messages: [{ role: "user", content: request.prompt }],
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Anthropic API error: ${error}`);
  }

  const data = (await response.json()) as {
    content: { type: string; text: string }[];
    usage?: { output_tokens: number };
  };

  return {
    synthesis: data.content[0]?.text || "",
    model_used: model,
    tokens_used: data.usage?.output_tokens,
  };
}

/**
 * OpenAI synthesis
 */
async function synthesizeOpenAI(
  config: LLMConfig,
  request: SynthesisRequest
): Promise<SynthesisResult> {
  const model = config.model || "gpt-4o";
  const maxTokens = request.max_tokens || 1024;

  const response = await fetch("https://api.openai.com/v1/chat/completions", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${config.openai_api_key}`,
    },
    body: JSON.stringify({
      model,
      max_tokens: maxTokens,
      messages: [
        { role: "system", content: request.system },
        { role: "user", content: request.prompt },
      ],
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`OpenAI API error: ${error}`);
  }

  const data = (await response.json()) as {
    choices: { message: { content: string } }[];
    usage?: { completion_tokens: number };
  };

  return {
    synthesis: data.choices[0]?.message?.content || "",
    model_used: model,
    tokens_used: data.usage?.completion_tokens,
  };
}

/**
 * Ollama synthesis (local) - optimized for local models
 */
async function synthesizeOllama(
  config: LLMConfig,
  request: SynthesisRequest,
  complexity: TaskComplexity
): Promise<SynthesisResult> {
  const model = getModelForComplexity(config, complexity);
  const baseUrl = config.ollama_url || "http://localhost:11434";

  // Use chat endpoint for better instruction following
  const response = await fetch(`${baseUrl}/api/chat`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      model,
      messages: [
        { role: "system", content: request.system },
        { role: "user", content: request.prompt },
      ],
      stream: false,
      options: {
        num_predict: request.max_tokens || 512,
        temperature: 0.7,
        top_p: 0.9,
      },
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`Ollama API error: ${error}`);
  }

  const data = (await response.json()) as {
    message?: { content: string };
    eval_count?: number;
  };

  return {
    synthesis: data.message?.content || "",
    model_used: model,
    tokens_used: data.eval_count,
  };
}

// =============================================================================
// Synthesis Prompts - Optimized for Local Models
// =============================================================================

/**
 * Prompts are optimized for:
 * 1. Clear structure (local models need explicit formatting)
 * 2. Concise output (fewer tokens = faster inference)
 * 3. Specific instructions (reduce ambiguity)
 */
export const SYNTHESIS_PROMPTS = {
  /**
   * Daily Briefing - Executive synthesis
   * Complexity: MEDIUM
   */
  dailyBriefing: {
    complexity: "medium" as TaskComplexity,

    system: `You are an intelligence analyst. Synthesize data into actionable briefings.

OUTPUT FORMAT:
1. One-sentence summary of the day
2. The single most important thing to review (with reason)
3. One recommended action

Be direct. No filler. Every word must add value.`,

    buildPrompt: (data: unknown, context: unknown): string => `
DATA:
${JSON.stringify(data, null, 2)}

USER CONTEXT:
${JSON.stringify(context, null, 2)}

Generate briefing with:
1. ONE-LINE SUMMARY
2. TOP PRIORITY (title + why it matters to this user)
3. RECOMMENDED ACTION`,
  },

  /**
   * Score Autopsy - Explain the reasoning
   * Complexity: HEAVY (needs to reason about correctness)
   */
  scoreAutopsy: {
    complexity: "heavy" as TaskComplexity,

    system: `You are a scoring forensics expert. Analyze if relevance scores are correct.

OUTPUT FORMAT:
1. VERDICT: [ACCURATE / TOO HIGH / TOO LOW]
2. REASONING: Why you reached this verdict (cite specific data)
3. KEY FACTOR: The main thing driving this score
4. SUGGESTION: How to get more/fewer items like this

Be analytical. Cite numbers from the data.`,

    buildPrompt: (data: unknown, context: unknown): string => `
SCORE DATA:
${JSON.stringify(data, null, 2)}

USER INTERESTS:
${JSON.stringify(context, null, 2)}

Analyze: Is this score correct given the user's interests?`,
  },

  /**
   * Trend Analysis - Predict and explain
   * Complexity: HEAVY (needs pattern recognition + prediction)
   */
  trendAnalysis: {
    complexity: "heavy" as TaskComplexity,

    system: `You are a tech trend analyst. Interpret patterns and predict trajectories.

OUTPUT FORMAT:
1. KEY SIGNAL: The most important trend (one sentence)
2. WHY IT'S HAPPENING: Brief explanation
3. PREDICTION: What to expect in the next 1-2 weeks
4. ACTION: Should the user adjust their focus?

Ground predictions in the data. Don't speculate wildly.`,

    buildPrompt: (data: unknown, context: unknown): string => `
TREND DATA:
${JSON.stringify(data, null, 2)}

USER CONTEXT:
${JSON.stringify(context, null, 2)}

Interpret: What do these trends mean for this specific user?`,
  },

  /**
   * Context Analysis - Personalized optimization advice
   * Complexity: MEDIUM (structured recommendations)
   */
  contextAnalysis: {
    complexity: "medium" as TaskComplexity,

    system: `You are a personalization expert. Help users optimize their content filtering.

OUTPUT FORMAT:
1. BIGGEST GAP: What's missing from their context?
2. HIGHEST IMPACT CHANGE: One specific thing to add/remove
3. QUICK WIN: Something they can do in 30 seconds

Be specific. Give exact examples they can copy-paste.`,

    buildPrompt: (data: unknown): string => `
CONTEXT DATA:
${JSON.stringify(data, null, 2)}

Provide optimization recommendations. Be specific and actionable.`,
  },

  /**
   * Topic Connections - Explain the knowledge graph
   * Complexity: HEAVY (needs graph reasoning)
   */
  topicConnections: {
    complexity: "heavy" as TaskComplexity,

    system: `You are a knowledge graph analyst. Find hidden patterns in topic relationships.

OUTPUT FORMAT:
1. SURPRISING CONNECTION: A non-obvious relationship and its significance
2. MISSING LINK: Topics that should connect but don't
3. EXPLORATION PATH: A topic worth investigating based on the graph
4. PATTERN: What the graph structure reveals about their interests

Look for insights the user wouldn't notice themselves.`,

    buildPrompt: (data: unknown, context: unknown): string => `
GRAPH DATA:
${JSON.stringify(data, null, 2)}

USER CONTEXT:
${JSON.stringify(context, null, 2)}

Interpret: What hidden patterns does this graph reveal?`,
  },
};

// =============================================================================
// Utility: Check Ollama availability
// =============================================================================

/**
 * Check if Ollama is running and has required models
 */
export async function checkOllamaStatus(
  baseUrl: string = "http://localhost:11434"
): Promise<{ available: boolean; models: string[]; error?: string }> {
  try {
    const response = await fetch(`${baseUrl}/api/tags`, {
      method: "GET",
      signal: AbortSignal.timeout(5000),
    });

    if (!response.ok) {
      return { available: false, models: [], error: "Ollama not responding" };
    }

    const data = (await response.json()) as { models?: { name: string }[] };
    const models = data.models?.map(m => m.name) || [];

    return { available: true, models };
  } catch (error) {
    return {
      available: false,
      models: [],
      error: error instanceof Error ? error.message : "Connection failed",
    };
  }
}

/**
 * Recommended models for 4DA synthesis tasks
 */
export const RECOMMENDED_MODELS = {
  "16GB_VRAM": {
    primary: "qwen2.5:14b-instruct-q5_K_M",
    fallback: "llama3.1:8b-instruct-q8_0",
    install: "ollama pull qwen2.5:14b-instruct-q5_K_M",
  },
  "12GB_VRAM": {
    primary: "mistral-nemo:12b-instruct-q5_K_M",
    fallback: "llama3.1:8b-instruct-q6_K",
    install: "ollama pull mistral-nemo:12b-instruct-q5_K_M",
  },
  "8GB_VRAM": {
    primary: "llama3.1:8b-instruct-q6_K",
    fallback: "llama3.2:3b-instruct-q8_0",
    install: "ollama pull llama3.1:8b-instruct-q6_K",
  },
};
