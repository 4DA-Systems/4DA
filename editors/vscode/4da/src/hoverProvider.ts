import * as vscode from 'vscode';
import type { MCPClient } from './mcpClient';

export class HoverProvider implements vscode.HoverProvider {
    constructor(private client: MCPClient) {}

    async provideHover(
        document: vscode.TextDocument,
        position: vscode.Position
    ): Promise<vscode.Hover | null> {
        const line = document.lineAt(position.line).text;
        const pkg = extractPackage(line, document.languageId);
        if (!pkg) return null;

        const info = await this.client.getDependencyInfo(pkg.name, pkg.ecosystem);
        if (!info) return null;

        const md = new vscode.MarkdownString();
        // Only allow the specific command URI we use in the hover
        md.isTrusted = { enabledCommands: ['4da.openApp'] };

        const ver = info.version ?? 'unknown';

        // Package name and version header
        md.appendMarkdown('### ');
        md.appendText(info.name);
        md.appendMarkdown('\n\n');

        // Version comparison
        if (info.latestVersion && info.latestVersion !== ver && ver !== 'unknown') {
            const behind = versionDiff(ver, info.latestVersion);
            md.appendMarkdown('$(versions) ');
            md.appendMarkdown('`');
            md.appendText(ver);
            md.appendMarkdown('`');
            md.appendMarkdown(' $(arrow-right) ');
            md.appendMarkdown('`');
            md.appendText(info.latestVersion);
            md.appendMarkdown('`');
            if (behind) {
                md.appendMarkdown(` — *${behind}*`);
            }
            md.appendMarkdown('\n\n');
        } else if (ver !== 'unknown') {
            md.appendMarkdown('$(check) ');
            md.appendMarkdown('`');
            md.appendText(ver);
            md.appendMarkdown('`');
            if (info.latestVersion && info.latestVersion === ver) {
                md.appendMarkdown(' — *latest*');
            }
            md.appendMarkdown('\n\n');
        }

        // Security alerts with severity indicators
        if (info.alerts.length > 0) {
            md.appendMarkdown('---\n\n');
            for (const alert of info.alerts) {
                const severityIcon = severityIndicator(alert.severity);
                md.appendMarkdown(`${severityIcon} `);
                md.appendText(alert.title);
                if (alert.cveId) {
                    md.appendMarkdown(' `');
                    md.appendText(alert.cveId);
                    md.appendMarkdown('`');
                }
                md.appendMarkdown(` **${alert.severity.toUpperCase()}**`);
                if (alert.affectedVersions) {
                    md.appendMarkdown(' (affects ');
                    md.appendText(alert.affectedVersions);
                    md.appendMarkdown(')');
                }
                md.appendMarkdown('\n\n');
            }
        } else {
            md.appendMarkdown('$(check) No known issues\n\n');
        }

        // Action links
        md.appendMarkdown('---\n\n');
        const registryUrl = getRegistryUrl(pkg.name, pkg.ecosystem);
        if (registryUrl) {
            md.appendMarkdown(`[$(link-external) View on ${ecosystemLabel(pkg.ecosystem)}](${registryUrl}) | `);
        }
        md.appendMarkdown('*[$(pulse) Open in 4DA](command:4da.openApp)*');

        return new vscode.Hover(md);
    }
}

/**
 * Get a visual severity indicator using codicons and emphasis.
 */
function severityIndicator(severity: string): string {
    switch (severity.toLowerCase()) {
        case 'critical': return '$(error) $(flame)';
        case 'high': return '$(warning)';
        case 'medium': return '$(info)';
        case 'low': return '$(circle-outline)';
        default: return '$(info)';
    }
}

/**
 * Compute a human-readable version diff description.
 * Returns null if versions can't be compared.
 */
function versionDiff(current: string, latest: string): string | null {
    const cur = current.replace(/^[~^>=<]/, '').split('.').map(Number);
    const lat = latest.replace(/^[~^>=<]/, '').split('.').map(Number);

    if (cur.length < 2 || lat.length < 2) return null;
    if (cur.some(isNaN) || lat.some(isNaN)) return null;

    if (lat[0] > cur[0]) {
        const diff = lat[0] - cur[0];
        return `${diff} major ${diff === 1 ? 'version' : 'versions'} behind`;
    }
    if (lat[1] > cur[1]) {
        const diff = lat[1] - cur[1];
        return `${diff} minor ${diff === 1 ? 'update' : 'updates'} behind`;
    }
    if (lat.length >= 3 && cur.length >= 3 && lat[2] > cur[2]) {
        const diff = lat[2] - cur[2];
        return `${diff} ${diff === 1 ? 'patch' : 'patches'} behind`;
    }
    return null;
}

/**
 * Get the registry URL for a package in a given ecosystem.
 */
function getRegistryUrl(name: string, ecosystem: string): string | null {
    switch (ecosystem) {
        case 'npm': return `https://www.npmjs.com/package/${encodeURIComponent(name)}`;
        case 'pip': return `https://pypi.org/project/${encodeURIComponent(name)}/`;
        case 'cargo': return `https://crates.io/crates/${encodeURIComponent(name)}`;
        case 'go': return `https://pkg.go.dev/${encodeURIComponent(name)}`;
        default: return null;
    }
}

/**
 * Human-readable ecosystem name.
 */
function ecosystemLabel(ecosystem: string): string {
    switch (ecosystem) {
        case 'npm': return 'npm';
        case 'pip': return 'PyPI';
        case 'cargo': return 'crates.io';
        case 'go': return 'pkg.go.dev';
        default: return ecosystem;
    }
}

function extractPackage(line: string, lang: string): { name: string; ecosystem: string } | null {
    // TypeScript/JavaScript
    if (['typescript', 'typescriptreact', 'javascript', 'javascriptreact'].includes(lang)) {
        const m = line.match(/(?:import|from)\s+['"]([^'"./][^'"]*)['"]/);
        if (m) {
            const name = m[1].startsWith('@')
                ? m[1].split('/').slice(0, 2).join('/')
                : m[1].split('/')[0];
            return { name, ecosystem: 'npm' };
        }
        const r = line.match(/require\s*\(\s*['"]([^'"./][^'"]*)['"]\s*\)/);
        if (r) {
            const name = r[1].startsWith('@')
                ? r[1].split('/').slice(0, 2).join('/')
                : r[1].split('/')[0];
            return { name, ecosystem: 'npm' };
        }
    }

    // Python
    if (lang === 'python') {
        const m = line.match(/(?:from|import)\s+([a-zA-Z_][a-zA-Z0-9_]*)/);
        if (m) return { name: m[1], ecosystem: 'pip' };
    }

    // Rust
    if (lang === 'rust') {
        const m = line.match(/use\s+([a-z_][a-z0-9_]*)::/);
        if (m) return { name: m[1], ecosystem: 'cargo' };
    }

    // Go
    if (lang === 'go') {
        const m = line.match(/["']([a-zA-Z0-9./\-_]+)["']/);
        if (m && m[1].includes('/')) return { name: m[1], ecosystem: 'go' };
    }

    return null;
}
