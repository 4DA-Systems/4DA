/**
 * MCP Client — communicates with 4DA's MCP server via stdio JSON-RPC.
 *
 * Connection modes:
 * 1. Direct: Spawns 4DA MCP server as subprocess, communicates via stdin/stdout
 * 2. Degraded: If MCP server unavailable, returns empty data gracefully
 *
 * Protocol: JSON-RPC 2.0 over newline-delimited stdio (MCP spec)
 */

import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';

// ============================================================================
// Public Interfaces (consumed by statusBar, hoverProvider, diagnostics, signalPanel)
// ============================================================================

export interface Signal {
    title: string;
    url?: string;
    source: string;
    score: number;
    signalType: string;
    priority: string;
    summary?: string;
}

export interface DependencyInfo {
    name: string;
    version?: string;
    latestVersion?: string;
    ecosystem: string;
    alerts: DependencyAlert[];
}

export interface DependencyAlert {
    type: string;
    severity: string;
    title: string;
    cveId?: string;
    affectedVersions?: string;
}

// ============================================================================
// JSON-RPC Types
// ============================================================================

interface JsonRpcRequest {
    jsonrpc: '2.0';
    id: number;
    method: string;
    params?: Record<string, unknown>;
}

interface JsonRpcResponse {
    jsonrpc: '2.0';
    id: number;
    result?: unknown;
    error?: { code: number; message: string; data?: unknown };
}

interface JsonRpcNotification {
    jsonrpc: '2.0';
    method: string;
    params?: Record<string, unknown>;
}

// ============================================================================
// MCP Tool Response Types (from 4DA MCP server)
// ============================================================================

interface McpToolResult {
    content: Array<{ type: string; text: string }>;
    isError?: boolean;
}

interface McpSignal {
    id: number;
    title: string;
    url: string | null;
    source_type: string;
    relevance_score: number;
    signal_type: string;
    signal_priority: string;
    action: string;
    triggers: string[];
    confidence: number;
    discovered_ago: string;
}

interface McpSignalsResponse {
    signals: McpSignal[];
    total: number;
    summary: Record<string, number>;
}

interface McpRelevantItem {
    id: number;
    source_type: string;
    source_id: string;
    url: string | null;
    title: string;
    content: string;
    relevance_score: number;
    created_at: string;
    discovered_ago: string;
}

interface McpProjectHealthResponse {
    projects: Array<{
        project_path: string;
        project_name: string;
        dependency_count: number;
        dependencies: Array<{
            name: string;
            version: string;
            language: string;
        }>;
        health: {
            security_score: number;
            security_issues: number;
        };
    }>;
    total_projects: number;
    summary: string;
}

// ============================================================================
// Stdio Read Buffer (mirrors MCP SDK's ReadBuffer)
// ============================================================================

class ReadBuffer {
    private buffer: Buffer | undefined;

    append(chunk: Buffer): void {
        this.buffer = this.buffer ? Buffer.concat([this.buffer, chunk]) : chunk;
    }

    readMessage(): unknown | null {
        if (!this.buffer) return null;
        const index = this.buffer.indexOf('\n');
        if (index === -1) return null;

        const line = this.buffer.toString('utf8', 0, index).replace(/\r$/, '');
        this.buffer = this.buffer.subarray(index + 1);

        if (!line.trim()) return null;
        try {
            return JSON.parse(line);
        } catch {
            return null;
        }
    }

    clear(): void {
        this.buffer = undefined;
    }
}

// ============================================================================
// MCP Client
// ============================================================================

export class MCPClient {
    private process: cp.ChildProcess | null = null;
    private readBuffer = new ReadBuffer();
    private pendingRequests = new Map<number, {
        resolve: (value: unknown) => void;
        reject: (reason: Error) => void;
        timer: ReturnType<typeof setTimeout>;
    }>();
    private nextId = 1;
    private connected = false;
    private connecting = false;
    private serverPath: string | null = null;

    // Cache to avoid hammering the MCP server
    private signalCache: { data: Signal[]; timestamp: number } | null = null;
    private healthCache = new Map<string, { data: McpProjectHealthResponse; timestamp: number }>();
    private static CACHE_TTL_MS = 30_000; // 30 seconds

    /**
     * Connect to the 4DA MCP server.
     * Spawns the server as a subprocess and performs the MCP initialize handshake.
     */
    async connect(): Promise<boolean> {
        if (this.connected) return true;
        if (this.connecting) return false;

        this.connecting = true;
        try {
            const serverPath = this.findMCPServer();
            if (!serverPath) {
                this.connecting = false;
                return false;
            }
            this.serverPath = serverPath;

            // Find node executable
            const nodePath = process.execPath || 'node';

            await new Promise<void>((resolve, reject) => {
                const env: Record<string, string> = {};

                // Inherit safe environment variables (mirrors MCP SDK)
                const inheritVars = process.platform === 'win32'
                    ? ['APPDATA', 'HOMEDRIVE', 'HOMEPATH', 'LOCALAPPDATA', 'PATH',
                       'PROCESSOR_ARCHITECTURE', 'SYSTEMDRIVE', 'SYSTEMROOT', 'TEMP',
                       'USERNAME', 'USERPROFILE', 'PROGRAMFILES']
                    : ['HOME', 'LOGNAME', 'PATH', 'SHELL', 'TERM', 'USER'];

                for (const key of inheritVars) {
                    const val = process.env[key];
                    if (val && !val.startsWith('()')) {
                        env[key] = val;
                    }
                }

                // Pass FOURDA_DB_PATH if set
                if (process.env['FOURDA_DB_PATH']) {
                    env['FOURDA_DB_PATH'] = process.env['FOURDA_DB_PATH'];
                }

                this.process = cp.spawn(nodePath, [serverPath], {
                    stdio: ['pipe', 'pipe', 'pipe'],
                    env,
                    windowsHide: true,
                });

                this.process.on('error', (err) => {
                    this.handleProcessExit();
                    reject(err);
                });

                this.process.on('spawn', () => {
                    resolve();
                });

                this.process.on('close', () => {
                    this.handleProcessExit();
                });

                // Handle stdout — parse JSON-RPC messages
                this.process.stdout?.on('data', (chunk: Buffer) => {
                    this.readBuffer.append(chunk);
                    this.processReadBuffer();
                });

                // Ignore stderr (MCP server logs there)
                this.process.stderr?.on('data', () => {});
            });

            // Perform MCP initialize handshake
            const initResult = await this.sendRequest('initialize', {
                protocolVersion: '2024-11-05',
                capabilities: {},
                clientInfo: {
                    name: '4da-vscode',
                    version: '0.1.0',
                },
            }) as { protocolVersion: string; capabilities: unknown; serverInfo: unknown } | null;

            if (!initResult) {
                this.disconnect();
                this.connecting = false;
                return false;
            }

            // Send initialized notification
            this.sendNotification('notifications/initialized', {});

            this.connected = true;
            this.connecting = false;
            return true;
        } catch {
            this.disconnect();
            this.connecting = false;
            return false;
        }
    }

    /**
     * Disconnect from the MCP server and clean up.
     */
    disconnect(): void {
        this.connected = false;
        this.connecting = false;

        // Reject all pending requests
        for (const [, pending] of this.pendingRequests) {
            clearTimeout(pending.timer);
            pending.reject(new Error('MCP client disconnected'));
        }
        this.pendingRequests.clear();

        if (this.process) {
            try { this.process.stdin?.end(); } catch {}
            try { this.process.kill(); } catch {}
            this.process = null;
        }

        this.readBuffer.clear();
        this.signalCache = null;
        this.healthCache.clear();
    }

    // ========================================================================
    // Public API (called by statusBar, hoverProvider, diagnostics, signalPanel)
    // ========================================================================

    /**
     * Get actionable signals from 4DA's intelligence feed.
     * Maps MCP `get_actionable_signals` tool results to the Signal interface.
     */
    async getSignals(_workspacePath?: string): Promise<Signal[]> {
        try {
            // Check cache
            if (this.signalCache && Date.now() - this.signalCache.timestamp < MCPClient.CACHE_TTL_MS) {
                return this.signalCache.data;
            }

            if (!await this.ensureConnected()) return [];

            const result = await this.callTool('get_actionable_signals', {
                limit: 50,
                since_hours: 48,
            });

            if (!result) return [];

            const response = result as McpSignalsResponse;
            if (!response.signals) return [];

            const signals: Signal[] = response.signals.map(s => ({
                title: s.title,
                url: s.url ?? undefined,
                source: s.source_type,
                score: s.relevance_score,
                signalType: s.signal_type,
                priority: s.signal_priority,
                summary: s.action,
            }));

            this.signalCache = { data: signals, timestamp: Date.now() };
            return signals;
        } catch {
            return [];
        }
    }

    /**
     * Get dependency info for a specific package.
     * Uses MCP `project_health` tool and cross-references with the package name.
     */
    async getDependencyInfo(
        packageName: string,
        ecosystem: string
    ): Promise<DependencyInfo | null> {
        try {
            if (!await this.ensureConnected()) {
                return { name: packageName, ecosystem, alerts: [] };
            }

            // Check health cache
            const cacheKey = '_all_';
            const cached = this.healthCache.get(cacheKey);
            let healthData: McpProjectHealthResponse;

            if (cached && Date.now() - cached.timestamp < MCPClient.CACHE_TTL_MS) {
                healthData = cached.data;
            } else {
                const result = await this.callTool('project_health', {});
                if (!result) {
                    return { name: packageName, ecosystem, alerts: [] };
                }
                healthData = result as McpProjectHealthResponse;
                this.healthCache.set(cacheKey, { data: healthData, timestamp: Date.now() });
            }

            // Find the package in project dependencies
            let foundVersion: string | undefined;
            const alerts: DependencyAlert[] = [];

            for (const project of healthData.projects || []) {
                for (const dep of project.dependencies || []) {
                    if (dep.name === packageName) {
                        foundVersion = dep.version;
                        // If this project has security issues, flag the package
                        if (project.health.security_issues > 0) {
                            alerts.push({
                                type: 'security',
                                severity: 'medium',
                                title: `Potential security concern in ${project.project_name}`,
                            });
                        }
                    }
                }
            }

            // Also check signals for security mentions of this package
            const signals = await this.getSignals();
            for (const signal of signals) {
                if (signal.signalType === 'security_alert' &&
                    signal.title.toLowerCase().includes(packageName.toLowerCase())) {
                    alerts.push({
                        type: 'security_alert',
                        severity: signal.priority === 'critical' ? 'critical' : 'high',
                        title: signal.title,
                    });
                }
                if (signal.signalType === 'breaking_change' &&
                    signal.title.toLowerCase().includes(packageName.toLowerCase())) {
                    alerts.push({
                        type: 'breaking_change',
                        severity: 'medium',
                        title: signal.title,
                    });
                }
            }

            return {
                name: packageName,
                version: foundVersion,
                ecosystem,
                alerts,
            };
        } catch {
            return { name: packageName, ecosystem, alerts: [] };
        }
    }

    /**
     * Get signal counts for the status bar.
     */
    async getSignalCount(): Promise<{ total: number; security: number; trends: number }> {
        try {
            const signals = await this.getSignals();
            const security = signals.filter(
                s => s.signalType === 'security_alert' || s.signalType === 'breaking_change'
            ).length;
            return { total: signals.length, security, trends: signals.length - security };
        } catch {
            return { total: 0, security: 0, trends: 0 };
        }
    }

    /**
     * Whether the client is currently connected to the MCP server.
     */
    isConnected(): boolean {
        return this.connected;
    }

    // ========================================================================
    // MCP Protocol Layer
    // ========================================================================

    /**
     * Call an MCP tool by name with arguments.
     * Returns the parsed JSON result, or null on failure.
     */
    private async callTool(name: string, args: Record<string, unknown>): Promise<unknown> {
        const result = await this.sendRequest('tools/call', {
            name,
            arguments: args,
        }) as McpToolResult | null;

        if (!result || result.isError) return null;

        // MCP tools return { content: [{ type: "text", text: "..." }] }
        // Parse the JSON text from the first content block
        const textContent = result.content?.find(c => c.type === 'text');
        if (!textContent?.text) return null;

        try {
            return JSON.parse(textContent.text);
        } catch {
            return null;
        }
    }

    /**
     * Send a JSON-RPC request and wait for the response.
     */
    private sendRequest(method: string, params?: Record<string, unknown>): Promise<unknown> {
        return new Promise((resolve, reject) => {
            if (!this.process?.stdin) {
                resolve(null);
                return;
            }

            const id = this.nextId++;
            const request: JsonRpcRequest = {
                jsonrpc: '2.0',
                id,
                method,
                params,
            };

            // Timeout after 10 seconds
            const timer = setTimeout(() => {
                this.pendingRequests.delete(id);
                resolve(null);
            }, 10_000);

            this.pendingRequests.set(id, { resolve, reject, timer });

            const json = JSON.stringify(request) + '\n';
            try {
                this.process.stdin.write(json);
            } catch {
                clearTimeout(timer);
                this.pendingRequests.delete(id);
                resolve(null);
            }
        });
    }

    /**
     * Send a JSON-RPC notification (no response expected).
     */
    private sendNotification(method: string, params?: Record<string, unknown>): void {
        if (!this.process?.stdin) return;

        const notification: JsonRpcNotification = {
            jsonrpc: '2.0',
            method,
            params,
        };

        const json = JSON.stringify(notification) + '\n';
        try {
            this.process.stdin.write(json);
        } catch {
            // Ignore write errors on notifications
        }
    }

    /**
     * Process buffered stdout data into JSON-RPC messages.
     */
    private processReadBuffer(): void {
        while (true) {
            const message = this.readBuffer.readMessage();
            if (message === null) break;

            const msg = message as Record<string, unknown>;

            // JSON-RPC response (has id)
            if (typeof msg.id === 'number') {
                const pending = this.pendingRequests.get(msg.id);
                if (pending) {
                    clearTimeout(pending.timer);
                    this.pendingRequests.delete(msg.id);

                    if (msg.error) {
                        pending.resolve(null);
                    } else {
                        pending.resolve(msg.result ?? null);
                    }
                }
            }
            // Notifications from server (no id) — ignore for now
        }
    }

    /**
     * Handle process exit — clean up state.
     */
    private handleProcessExit(): void {
        this.connected = false;
        this.process = null;

        // Resolve all pending requests with null
        for (const [, pending] of this.pendingRequests) {
            clearTimeout(pending.timer);
            pending.resolve(null);
        }
        this.pendingRequests.clear();
    }

    /**
     * Ensure the client is connected, attempting reconnection if needed.
     */
    private async ensureConnected(): Promise<boolean> {
        if (this.connected && this.process) return true;
        return this.connect();
    }

    // ========================================================================
    // Server Discovery
    // ========================================================================

    /**
     * Find the 4DA MCP server entry point.
     * Checks multiple locations in priority order.
     */
    private findMCPServer(): string | null {
        const candidates: string[] = [];

        // 1. Environment variable override
        if (process.env['FOURDA_MCP_SERVER']) {
            candidates.push(process.env['FOURDA_MCP_SERVER']);
        }

        // 2. Global npm installation
        candidates.push(
            // npx-resolved path (global)
            path.join(os.homedir(), 'node_modules', '@4da', 'mcp-server', 'dist', 'index.js'),
            // npm global on Windows
            path.join(os.homedir(), 'AppData', 'Roaming', 'npm', 'node_modules', '@4da', 'mcp-server', 'dist', 'index.js'),
            // npm global on Linux/macOS
            '/usr/local/lib/node_modules/@4da/mcp-server/dist/index.js',
        );

        // 3. Relative to the 4DA app data directory
        const appData = process.env['APPDATA'] || path.join(os.homedir(), '.local', 'share');
        candidates.push(
            path.join(appData, '4da', 'mcp-server', 'dist', 'index.js'),
        );

        // 4. Development path (relative to extension)
        candidates.push(
            path.resolve(__dirname, '..', '..', '..', '..', 'mcp-4da-server', 'dist', 'index.js'),
        );

        for (const candidate of candidates) {
            try {
                if (fs.existsSync(candidate)) {
                    return candidate;
                }
            } catch {
                continue;
            }
        }

        return null;
    }
}
