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
        md.isTrusted = true;

        const ver = info.version ?? 'unknown';
        md.appendMarkdown(`**${info.name}**@${ver}`);
        if (info.latestVersion && info.latestVersion !== ver) {
            md.appendMarkdown(` (latest: ${info.latestVersion})`);
        }
        md.appendMarkdown('\n\n');

        if (info.alerts.length > 0) {
            for (const alert of info.alerts) {
                const icon = ['critical', 'high'].includes(alert.severity) ? '$(warning)' : '$(info)';
                md.appendMarkdown(`${icon} **${alert.title}**`);
                if (alert.cveId) md.appendMarkdown(` (${alert.cveId})`);
                md.appendMarkdown(` — ${alert.severity.toUpperCase()}\n\n`);
            }
        } else {
            md.appendMarkdown('$(check) No known issues\n\n');
        }

        md.appendMarkdown('---\n\n*[Open in 4DA](command:4da.openApp)*');
        return new vscode.Hover(md);
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
