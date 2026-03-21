import * as crypto from 'crypto';
import * as vscode from 'vscode';
import type { MCPClient, Signal } from './mcpClient';

export class SignalPanelProvider implements vscode.WebviewViewProvider {
    private view?: vscode.WebviewView;
    private lastSeenIds = new Set<string>();
    private disposables: vscode.Disposable[] = [];

    constructor(
        private readonly extensionUri: vscode.Uri,
        private readonly client: MCPClient
    ) {
        // Refresh when active editor changes — context-aware signals
        this.disposables.push(
            vscode.window.onDidChangeActiveTextEditor(() => {
                if (this.view?.visible) {
                    this.updateContent();
                }
            })
        );
    }

    resolveWebviewView(webviewView: vscode.WebviewView) {
        this.view = webviewView;
        webviewView.webview.options = { enableScripts: true };
        webviewView.webview.onDidReceiveMessage(msg => {
            if (msg.type === 'openUrl' && msg.url) {
                const uri = vscode.Uri.parse(msg.url);
                if (uri.scheme === 'https' || uri.scheme === 'http') {
                    vscode.env.openExternal(uri);
                }
            } else if (msg.type === 'openApp') {
                vscode.commands.executeCommand('4da.openApp');
            }
        });
        this.updateContent();
    }

    async refresh() {
        if (this.view) await this.updateContent();
    }

    dispose() {
        this.disposables.forEach(d => d.dispose());
    }

    private async updateContent() {
        if (!this.view) return;

        const editor = vscode.window.activeTextEditor;
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

        // Extract current file context for relevance filtering
        let fileContext: string[] = [];
        if (editor) {
            const text = editor.document.getText();
            const lines = text.split('\n').slice(0, 50);
            fileContext = this.extractImports(lines, editor.document.languageId);
        }

        if (!this.client.isConnected()) {
            this.view.webview.html = this.renderWelcome();
            return;
        }

        const signals = await this.client.getSignals(workspace);
        const { relevant, other } = this.rankByContext(signals, fileContext);
        this.view.webview.html = this.render(relevant, other, fileContext);
    }

    /**
     * Extract import/dependency names from the first N lines of the active file.
     */
    private extractImports(lines: string[], languageId: string): string[] {
        const imports: string[] = [];

        for (const line of lines) {
            if (['typescript', 'typescriptreact', 'javascript', 'javascriptreact'].includes(languageId)) {
                // import ... from 'package' or require('package')
                const m = line.match(/(?:import|from)\s+['"]([^'"./][^'"]*)['"]/);
                if (m) {
                    const name = m[1].startsWith('@')
                        ? m[1].split('/').slice(0, 2).join('/')
                        : m[1].split('/')[0];
                    if (!imports.includes(name)) imports.push(name);
                }
                const r = line.match(/require\s*\(\s*['"]([^'"./][^'"]*)['"]\s*\)/);
                if (r) {
                    const name = r[1].startsWith('@')
                        ? r[1].split('/').slice(0, 2).join('/')
                        : r[1].split('/')[0];
                    if (!imports.includes(name)) imports.push(name);
                }
            }

            if (languageId === 'python') {
                const m = line.match(/(?:from|import)\s+([a-zA-Z_][a-zA-Z0-9_]*)/);
                if (m && !imports.includes(m[1])) imports.push(m[1]);
            }

            if (languageId === 'rust') {
                const m = line.match(/use\s+([a-z_][a-z0-9_]*)::/);
                if (m && !imports.includes(m[1])) imports.push(m[1]);
                // Also catch extern crate
                const ec = line.match(/extern\s+crate\s+([a-z_][a-z0-9_]*)/);
                if (ec && !imports.includes(ec[1])) imports.push(ec[1]);
            }

            if (languageId === 'go') {
                const m = line.match(/["']([a-zA-Z0-9./\-_]+)["']/);
                if (m && m[1].includes('/')) {
                    const pkg = m[1].split('/').pop() ?? m[1];
                    if (!imports.includes(pkg)) imports.push(pkg);
                }
            }
        }

        // Also extract language keyword for broader matching
        const langMap: Record<string, string> = {
            typescript: 'typescript', typescriptreact: 'react',
            javascript: 'javascript', javascriptreact: 'react',
            python: 'python', rust: 'rust', go: 'go', ruby: 'ruby',
        };
        const lang = langMap[languageId];
        if (lang && !imports.includes(lang)) imports.push(lang);

        return imports;
    }

    /**
     * Split signals into context-relevant and other, based on file imports.
     */
    private rankByContext(
        signals: Signal[],
        context: string[]
    ): { relevant: Signal[]; other: Signal[] } {
        if (context.length === 0) {
            return { relevant: signals, other: [] };
        }

        const contextLower = context.map(c => c.toLowerCase());
        const relevant: Signal[] = [];
        const other: Signal[] = [];

        for (const signal of signals) {
            const titleLower = signal.title.toLowerCase();
            const summaryLower = (signal.summary ?? '').toLowerCase();
            const sourceLower = signal.source.toLowerCase();

            const isRelevant = contextLower.some(ctx =>
                titleLower.includes(ctx) ||
                summaryLower.includes(ctx) ||
                sourceLower.includes(ctx)
            );

            if (isRelevant) {
                relevant.push(signal);
            } else {
                other.push(signal);
            }
        }

        return { relevant, other };
    }

    /**
     * Generate a signal ID for new-badge tracking.
     */
    private signalId(s: Signal): string {
        return `${s.title}::${s.source}::${s.signalType}`;
    }

    private renderWelcome(): string {
        const nonce = crypto.randomBytes(16).toString('hex');

        return `<!DOCTYPE html><html><head>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';">
<style>
body{background:#0A0A0A;color:#FFF;font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;padding:0;margin:0;font-size:13px}
.welcome{padding:32px 20px;text-align:center}
.welcome h3{margin:0 0 8px;font-size:15px;font-weight:600;color:#FFF}
.welcome p{color:#A0A0A0;margin:4px 0;line-height:1.6}
.setup-steps{margin:20px 0;padding:16px;background:#141414;border-radius:8px;border:1px solid #2A2A2A;text-align:left}
.setup-steps p{margin:8px 0;font-size:12px}
.setup-steps code{background:#1F1F1F;padding:2px 6px;border-radius:3px;font-family:'JetBrains Mono',monospace;font-size:11px;color:#D4AF37}
.setup-steps a{color:#D4AF37;text-decoration:none}
.setup-steps a:hover{text-decoration:underline}
.subtle{font-size:11px;color:#8A8A8A;margin-top:16px !important}
@keyframes breathe{0%,100%{opacity:.3}50%{opacity:.8}}
.pulse-dot{display:inline-block;width:6px;height:6px;background:#D4AF37;border-radius:50%;animation:breathe 2.5s ease-in-out infinite;margin-right:6px;vertical-align:middle}
</style></head><body>
<div class="welcome">
    <h3>Welcome to 4DA</h3>
    <p>Developer intelligence, right in your editor.</p>
    <div class="setup-steps">
        <p>1. Install <a href="https://4da.ai">4DA Desktop</a> for full intelligence</p>
        <p>2. Or run: <code>npx @4da/mcp</code> for quick setup</p>
    </div>
    <p class="subtle"><span class="pulse-dot"></span>Signals will appear here once connected.</p>
</div>
<script nonce="${nonce}">const vscode=acquireVsCodeApi();</script>
</body></html>`;
    }

    private render(relevant: Signal[], other: Signal[], context: string[]): string {
        const nonce = crypto.randomBytes(16).toString('hex');
        const allSignals = [...relevant, ...other];

        // Track which signals are "new" since last render
        const currentIds = new Set(allSignals.map(s => this.signalId(s)));
        const newIds = new Set<string>();
        if (this.lastSeenIds.size > 0) {
            for (const id of currentIds) {
                if (!this.lastSeenIds.has(id)) {
                    newIds.add(id);
                }
            }
        }
        this.lastSeenIds = currentIds;

        // Build context header
        const contextHeader = context.length > 0
            ? `<div class="context-bar">Relevant to: ${context.slice(0, 5).map(c => `<span class="ctx-tag">${esc(c)}</span>`).join(' ')}</div>`
            : '';

        // Build signal sections
        let items = '';

        if (allSignals.length === 0) {
            items = `<div class="empty"><div class="empty-dot"></div><div class="empty-text">No signals yet. 4DA is watching.</div></div>`;
        } else {
            if (relevant.length > 0 && other.length > 0) {
                items += `<div class="section-label">Relevant to this file</div>`;
                items += relevant.map((s, i) => this.renderSignal(s, i, newIds)).join('');
                items += `<div class="section-label other-label">Other signals</div>`;
                items += other.map((s, i) => this.renderSignal(s, relevant.length + i, newIds)).join('');
            } else {
                items += allSignals.map((s, i) => this.renderSignal(s, i, newIds)).join('');
            }
        }

        return `<!DOCTYPE html><html><head>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';">
<style>
body{background:#0A0A0A;color:#FFF;font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;padding:0;margin:0;font-size:13px}
.header{padding:12px 16px;border-bottom:1px solid #2A2A2A;background:linear-gradient(180deg,#141414 0%,#0A0A0A 100%)}
.header h3{margin:0;font-size:13px;font-weight:600;color:#A0A0A0;text-transform:uppercase;letter-spacing:.5px}
.context-bar{padding:6px 16px;background:#0F1B2B;border-bottom:1px solid #1A2A3A;font-size:11px;color:#3B82F6;display:flex;align-items:center;gap:4px;flex-wrap:wrap}
.ctx-tag{background:#1A2A3A;padding:1px 6px;border-radius:3px;font-size:10px;color:#60A5FA}
.section-label{padding:8px 16px 4px;font-size:10px;font-weight:600;text-transform:uppercase;letter-spacing:.8px;color:#D4AF37}
.other-label{color:#8A8A8A;border-top:1px solid #1F1F1F;margin-top:4px;padding-top:12px}
@keyframes fadeIn{from{opacity:0;transform:translateY(4px)}to{opacity:1;transform:translateY(0)}}
.signal{padding:12px 16px;border-bottom:1px solid #1F1F1F;cursor:pointer;display:flex;gap:10px;animation:fadeIn .3s ease forwards;opacity:0;transition:background .15s ease}
.signal:hover{background:#181818}
.signal:nth-child(1){animation-delay:0s}
.signal:nth-child(2){animation-delay:.04s}
.signal:nth-child(3){animation-delay:.08s}
.signal:nth-child(4){animation-delay:.12s}
.signal:nth-child(5){animation-delay:.16s}
.signal:nth-child(6){animation-delay:.2s}
.badge{font-size:11px;padding:2px 6px;border-radius:3px;white-space:nowrap;align-self:flex-start;margin-top:2px;font-weight:600;letter-spacing:.3px}
.security_alert .badge{background:#3B1111;color:#EF4444}
.breaking_change .badge{background:#3B2E11;color:#F59E0B}
.tool_discovery .badge{background:#0F2B1B;color:#22C55E}
.tech_trend .badge{background:#0F1B2B;color:#3B82F6}
.learning .badge{background:#1B0F2B;color:#A855F7}
.competitive_intel .badge{background:#2B1B0F;color:#F97316}
.new-badge{font-size:9px;padding:1px 5px;border-radius:3px;background:#D4AF37;color:#0A0A0A;font-weight:700;text-transform:uppercase;letter-spacing:.5px;margin-left:6px;vertical-align:middle}
.content{flex:1;min-width:0}
.title{font-weight:500;line-height:1.4;display:flex;align-items:center}
.meta{font-size:11px;color:#8A8A8A;margin-top:4px;display:flex;align-items:center;gap:6px}
.score-bar{width:40px;height:3px;background:#1F1F1F;border-radius:2px;overflow:hidden;display:inline-block}
.score-fill{height:100%;border-radius:2px;transition:width .3s ease}
.score-high{background:#22C55E}
.score-mid{background:#F59E0B}
.score-low{background:#8A8A8A}
.summary{font-size:12px;color:#A0A0A0;margin-top:6px;line-height:1.4}
.empty{padding:48px 16px;text-align:center;color:#8A8A8A}
@keyframes breathe{0%,100%{opacity:.3;transform:scale(.9)}50%{opacity:.8;transform:scale(1.1)}}
.empty-dot{width:8px;height:8px;background:#D4AF37;border-radius:50%;margin:0 auto 12px;animation:breathe 2.5s ease-in-out infinite}
.empty-text{font-size:12px}
.footer{padding:12px 16px;border-top:1px solid #2A2A2A;text-align:center}
.footer a{color:#8A8A8A;text-decoration:none;font-size:11px;transition:color .15s ease}
.footer a:hover{color:#FFF}
</style></head><body>
<div class="header"><h3>Signals</h3></div>
${contextHeader}
${items}
<div class="footer"><a href="#" onclick="vscode.postMessage({type:'openApp'})">Open in 4DA</a></div>
<script nonce="${nonce}">const vscode=acquireVsCodeApi();function openUrl(u){if(u)vscode.postMessage({type:'openUrl',url:u})}</script>
</body></html>`;
    }

    private renderSignal(s: Signal, index: number, newIds: Set<string>): string {
        const isNew = newIds.has(this.signalId(s));
        const scorePercent = (s.score * 100).toFixed(0);
        const scoreClass = s.score >= 0.7 ? 'score-high' : s.score >= 0.4 ? 'score-mid' : 'score-low';
        const delay = Math.min(index * 0.04, 0.3);

        return `
            <div class="signal ${esc(s.signalType)}" onclick="openUrl('${sanitizeUrl(s.url ?? '')}')" style="animation-delay:${delay}s">
                <span class="badge">${badge(s.signalType)}</span>
                <div class="content">
                    <div class="title">${esc(s.title)}${isNew ? '<span class="new-badge">new</span>' : ''}</div>
                    <div class="meta">
                        ${esc(s.source)}
                        <span class="score-bar"><span class="score-fill ${scoreClass}" style="width:${scorePercent}%"></span></span>
                        ${scorePercent}%
                    </div>
                    ${s.summary ? `<div class="summary">${esc(s.summary)}</div>` : ''}
                </div>
            </div>`;
    }
}

function badge(type: string): string {
    const map: Record<string, string> = {
        security_alert: 'SEC', breaking_change: 'BRK', tool_discovery: 'NEW',
        tech_trend: 'TRD', learning: 'LRN', competitive_intel: 'INT',
    };
    return map[type] ?? 'SIG';
}

function esc(s: string): string {
    return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
            .replace(/"/g,'&quot;').replace(/'/g,'&#x27;');
}

function sanitizeUrl(url: string): string {
    if (!url) return '';
    try {
        const parsed = new URL(url);
        if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
            return esc(url);
        }
        return '';
    } catch {
        return '';
    }
}
