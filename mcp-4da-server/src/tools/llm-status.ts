/**
 * LLM Status Tool
 *
 * Check the status of the LLM configuration and Ollama availability.
 * Helps diagnose why AI synthesis might not be working.
 */

import type { FourDADatabase } from "../db.js";
import {
  getLLMConfig,
  canSynthesize,
  checkOllamaStatus,
  DEFAULT_OLLAMA_MODELS,
  RECOMMENDED_MODELS,
} from "../llm.js";

export const llmStatusTool = {
  name: "llm_status",
  description: `Check LLM configuration and availability.
Returns:
- Current LLM provider configuration
- Ollama status (if configured)
- Available models
- Recommended models for your hardware
- Configuration recommendations

Use this to diagnose AI synthesis issues.`,
  inputSchema: {
    type: "object",
    properties: {
      check_ollama: {
        type: "boolean",
        description: "Check Ollama connectivity (default: true)",
      },
    },
  },
};

export interface LLMStatusParams {
  check_ollama?: boolean;
}

interface LLMStatusResult {
  provider: string | null;
  can_synthesize: boolean;
  config: {
    model: string | null;
    model_light: string | null;
    model_heavy: string | null;
    ollama_url: string | null;
  };
  ollama?: {
    available: boolean;
    models: string[];
    has_recommended: boolean;
    missing_models: string[];
    error?: string;
  };
  recommendations: string[];
  setup_commands?: string[];
}

export async function executeLLMStatus(
  db: FourDADatabase,
  params: LLMStatusParams
): Promise<LLMStatusResult> {
  const { check_ollama = true } = params;

  const dbInstance = (db as unknown as { db: { prepare: (sql: string) => { get: (...args: unknown[]) => unknown } } }).db;
  const config = getLLMConfig(dbInstance);

  const result: LLMStatusResult = {
    provider: config.provider,
    can_synthesize: canSynthesize(config),
    config: {
      model: config.model || null,
      model_light: config.model_light || null,
      model_heavy: config.model_heavy || null,
      ollama_url: config.ollama_url || null,
    },
    recommendations: [],
  };

  // Check Ollama if configured
  if (config.provider === "ollama" && check_ollama) {
    const ollamaUrl = config.ollama_url || "http://localhost:11434";
    const status = await checkOllamaStatus(ollamaUrl);

    const recommendedModels = [
      DEFAULT_OLLAMA_MODELS.light,
      DEFAULT_OLLAMA_MODELS.medium,
      DEFAULT_OLLAMA_MODELS.heavy,
    ];

    const missingModels = recommendedModels.filter(
      m => !status.models.some(installed => installed.startsWith(m.split(":")[0]))
    );

    result.ollama = {
      available: status.available,
      models: status.models,
      has_recommended: missingModels.length === 0,
      missing_models: missingModels,
      error: status.error,
    };

    // Generate recommendations
    if (!status.available) {
      result.recommendations.push("Ollama is not running. Start it with: ollama serve");
      result.setup_commands = ["ollama serve"];
    } else if (status.models.length === 0) {
      result.recommendations.push("No models installed. Pull recommended models.");
      result.setup_commands = [
        RECOMMENDED_MODELS["16GB_VRAM"].install,
        `ollama pull ${DEFAULT_OLLAMA_MODELS.light}`,
      ];
    } else if (missingModels.length > 0) {
      result.recommendations.push(
        `Missing recommended models: ${missingModels.join(", ")}`
      );
      result.setup_commands = missingModels.map(m => `ollama pull ${m}`);
    } else {
      result.recommendations.push("Ollama is properly configured with recommended models.");
    }
  } else if (!config.provider) {
    result.recommendations.push(
      "No LLM provider configured. Set llm_provider to 'ollama', 'anthropic', or 'openai' in settings."
    );
  } else if (config.provider === "anthropic" && !config.anthropic_api_key) {
    result.recommendations.push("Anthropic selected but no API key configured.");
  } else if (config.provider === "openai" && !config.openai_api_key) {
    result.recommendations.push("OpenAI selected but no API key configured.");
  }

  return result;
}
