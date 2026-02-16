#!/usr/bin/env node
/**
 * 4DA MCP Setup — Zero-friction editor configuration
 *
 * Detects installed editors and writes MCP server configuration so
 * the user can start using 4DA tools immediately after install.
 *
 * Usage:
 *   npx @4da/mcp-server --setup
 *   4da-mcp-setup
 *   pnpm run setup
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { homedir } from "node:os";

/** MCP config snippet for editors that use the standard mcpServers format */
const MCP_CONFIG = {
  "4da": {
    command: "npx",
    args: ["@4da/mcp-server"],
  },
};

interface EditorConfig {
  name: string;
  /** Returns config file path if editor is detected, null otherwise */
  detect: () => string | null;
  /** Write MCP configuration to the detected config path */
  write: (configPath: string) => void;
}

/**
 * Safely read and parse a JSON file, returning empty object on failure.
 */
function readJsonSafe(filePath: string): Record<string, unknown> {
  if (!existsSync(filePath)) return {};
  try {
    return JSON.parse(readFileSync(filePath, "utf-8")) as Record<string, unknown>;
  } catch {
    return {};
  }
}

/**
 * Ensure directory exists and write JSON to file.
 */
function writeJsonFile(filePath: string, data: Record<string, unknown>): void {
  const dir = dirname(filePath);
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
  }
  writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n");
}

const editors: EditorConfig[] = [
  {
    name: "Claude Code",
    detect: () => {
      // Project-level config takes priority
      const projectClaudeDir = join(process.cwd(), ".claude");
      if (existsSync(projectClaudeDir)) {
        return join(projectClaudeDir, "settings.json");
      }
      // Fall back to global config
      return join(homedir(), ".claude.json");
    },
    write: (configPath: string) => {
      const config = readJsonSafe(configPath);
      const mcpServers = (config.mcpServers || {}) as Record<string, unknown>;
      mcpServers["4da"] = MCP_CONFIG["4da"];
      config.mcpServers = mcpServers;
      writeJsonFile(configPath, config);
      console.log(`  Written: ${configPath}`);
    },
  },
  {
    name: "Cursor",
    detect: () => {
      const cursorDir = join(homedir(), ".cursor");
      if (existsSync(cursorDir)) {
        return join(cursorDir, "mcp.json");
      }
      return null;
    },
    write: (configPath: string) => {
      const config = readJsonSafe(configPath);
      const mcpServers = (config.mcpServers || {}) as Record<string, unknown>;
      mcpServers["4da"] = MCP_CONFIG["4da"];
      config.mcpServers = mcpServers;
      writeJsonFile(configPath, config);
      console.log(`  Written: ${configPath}`);
    },
  },
  {
    name: "VS Code (Copilot)",
    detect: () => {
      const vscodePath = join(homedir(), ".vscode");
      if (existsSync(vscodePath)) {
        return join(vscodePath, "mcp.json");
      }
      return null;
    },
    write: (configPath: string) => {
      const config = readJsonSafe(configPath);
      // VS Code uses "servers" key instead of "mcpServers"
      const servers = (config.servers || {}) as Record<string, unknown>;
      servers["4da"] = {
        type: "stdio",
        command: "npx",
        args: ["@4da/mcp-server"],
      };
      config.servers = servers;
      writeJsonFile(configPath, config);
      console.log(`  Written: ${configPath}`);
    },
  },
];

/**
 * Run the setup wizard: detect editors and write MCP configurations.
 */
export function runSetup(): void {
  console.log("\n  4DA MCP Server — Setup\n");
  console.log("  Detecting editors...\n");

  let configured = 0;

  for (const editor of editors) {
    const configPath = editor.detect();
    if (configPath) {
      console.log(`  Found: ${editor.name}`);
      editor.write(configPath);
      configured++;
      console.log("");
    }
  }

  if (configured === 0) {
    console.log("  No supported editors detected.\n");
    console.log("  Manual setup — add to your editor's MCP config:\n");
    console.log(`  ${JSON.stringify(MCP_CONFIG, null, 2)}\n`);
  }

  console.log("  Done. Restart your editor to activate 4DA.\n");
}

// Run directly if invoked as a script (bin entry or pnpm run setup)
const isDirectRun =
  process.argv[1]?.endsWith("setup.js") || process.argv[1]?.endsWith("setup.ts");
if (isDirectRun) {
  runSetup();
}
