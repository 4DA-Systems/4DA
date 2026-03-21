import * as vscode from 'vscode';
import type { MCPClient } from './mcpClient';
import type { StatusBarManager } from './statusBar';

export class DiagnosticsManager implements vscode.Disposable {
    private collection: vscode.DiagnosticCollection;
    private disposables: vscode.Disposable[] = [];
    private refreshTimer: ReturnType<typeof setTimeout> | undefined;
    private client: MCPClient;
    private statusBar: StatusBarManager | undefined;

    constructor(client: MCPClient, statusBar?: StatusBarManager) {
        this.client = client;
        this.statusBar = statusBar;
        this.collection = vscode.languages.createDiagnosticCollection('4da');

        const watcher = vscode.workspace.createFileSystemWatcher(
            '**/{package.json,Cargo.toml,requirements.txt,pyproject.toml,go.mod,Gemfile,pom.xml,*.gradle}'
        );
        watcher.onDidChange(() => this.refresh());
        watcher.onDidCreate(() => this.refresh());
        this.disposables.push(watcher);

        vscode.window.onDidChangeActiveTextEditor(() => this.refresh(), null, this.disposables);
    }

    async refresh() {
        clearTimeout(this.refreshTimer);
        this.refreshTimer = setTimeout(() => this.doRefresh(), 300);
    }

    private async doRefresh() {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            // No editor — clear file alert count
            this.statusBar?.setFileAlertCount(0);
            return;
        }

        const doc = editor.document;
        const diagnostics: vscode.Diagnostic[] = [];
        const limit = Math.min(doc.lineCount, 100);

        for (let i = 0; i < limit; i++) {
            const line = doc.lineAt(i).text;
            const pkg = extractImport(line, doc.languageId);
            if (!pkg) continue;

            const info = await this.client.getDependencyInfo(pkg, doc.languageId);
            if (!info?.alerts.length) continue;

            for (const alert of info.alerts) {
                const severity = mapSeverity(alert.severity);
                const range = new vscode.Range(i, 0, i, line.length);
                const diag = new vscode.Diagnostic(
                    range,
                    `4DA: ${alert.title} (${alert.severity.toUpperCase()})`,
                    severity
                );
                diag.source = '4DA';
                diag.code = alert.cveId ?? alert.type;
                diagnostics.push(diag);
            }
        }

        this.collection.set(doc.uri, diagnostics);

        // Update status bar with file-specific alert count
        const alertCount = diagnostics.filter(
            d => d.severity === vscode.DiagnosticSeverity.Error ||
                 d.severity === vscode.DiagnosticSeverity.Warning
        ).length;
        this.statusBar?.setFileAlertCount(alertCount);
    }

    dispose() {
        clearTimeout(this.refreshTimer);
        this.collection.dispose();
        this.disposables.forEach(d => d.dispose());
    }
}

function extractImport(line: string, lang: string): string | null {
    if (['typescript', 'typescriptreact', 'javascript', 'javascriptreact'].includes(lang)) {
        const m = line.match(/(?:import|from)\s+['"]([^'"./][^'"]*)['"]/);
        if (m) return m[1].startsWith('@') ? m[1].split('/').slice(0, 2).join('/') : m[1].split('/')[0];
    }
    if (lang === 'python') {
        const m = line.match(/(?:from|import)\s+([a-zA-Z_][a-zA-Z0-9_]*)/);
        if (m) return m[1];
    }
    if (lang === 'rust') {
        const m = line.match(/use\s+([a-z_][a-z0-9_]*)::/);
        if (m) return m[1];
    }
    return null;
}

function mapSeverity(s: string): vscode.DiagnosticSeverity {
    switch (s.toLowerCase()) {
        case 'critical': return vscode.DiagnosticSeverity.Error;
        case 'high': return vscode.DiagnosticSeverity.Warning;
        case 'medium': return vscode.DiagnosticSeverity.Warning;
        case 'low': return vscode.DiagnosticSeverity.Information;
        default: return vscode.DiagnosticSeverity.Hint;
    }
}
