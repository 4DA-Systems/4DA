/**
 * MCP Client — communicates with 4DA's MCP server for intelligence data.
 *
 * Connection modes:
 * 1. Direct: If 4DA desktop is running, connect to MCP server
 * 2. Degraded: If 4DA not installed, return empty data gracefully
 */

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

export class MCPClient {
    async getSignals(_workspacePath?: string): Promise<Signal[]> {
        try {
            // In production: call 4DA MCP server via stdio/HTTP
            // For now: return empty (degraded mode)
            return [];
        } catch {
            return [];
        }
    }

    async getDependencyInfo(
        packageName: string,
        ecosystem: string
    ): Promise<DependencyInfo | null> {
        try {
            // In production: call 4DA MCP `project_health` tool
            return {
                name: packageName,
                ecosystem,
                alerts: [],
            };
        } catch {
            return null;
        }
    }

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
}
