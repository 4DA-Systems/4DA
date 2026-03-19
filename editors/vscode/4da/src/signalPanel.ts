import * as crypto from 'crypto';
import * as vscode from 'vscode';
import type { MCPClient, Signal } from './mcpClient';

export class SignalPanelProvider implements vscode.WebviewViewProvider {
    private view?: vscode.WebviewView;

    constructor(
        private readonly extensionUri: vscode.Uri,
        private readonly client: MCPClient
    ) {}

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

    private async updateContent() {
        if (!this.view) return;
        const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        const signals = await this.client.getSignals(workspace);
        this.view.webview.html = this.render(signals);
    }

    private render(signals: Signal[]): string {
        const nonce = crypto.randomBytes(16).toString('hex');

        const items = signals.length > 0
            ? signals.map(s => `
                <div class="signal ${esc(s.signalType)}" onclick="openUrl('${sanitizeUrl(s.url ?? '')}')">
                    <span class="badge">${badge(s.signalType)}</span>
                    <div class="content">
                        <div class="title">${esc(s.title)}</div>
                        <div class="meta">${esc(s.source)} · ${(s.score * 100).toFixed(0)}%</div>
                        ${s.summary ? `<div class="summary">${esc(s.summary)}</div>` : ''}
                    </div>
                </div>`).join('')
            : '<div class="empty">No signals yet. 4DA is watching.</div>';

        return `<!DOCTYPE html><html><head>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${nonce}';">
<style>
body{background:#0A0A0A;color:#FFF;font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;padding:0;margin:0;font-size:13px}
.header{padding:12px 16px;border-bottom:1px solid #2A2A2A}
.header h3{margin:0;font-size:13px;font-weight:600;color:#A0A0A0;text-transform:uppercase;letter-spacing:.5px}
.signal{padding:12px 16px;border-bottom:1px solid #1F1F1F;cursor:pointer;display:flex;gap:10px}
.signal:hover{background:#141414}
.badge{font-size:11px;padding:2px 6px;border-radius:3px;white-space:nowrap;align-self:flex-start;margin-top:2px}
.security_alert .badge{background:#3B1111;color:#EF4444}
.breaking_change .badge{background:#3B2E11;color:#F59E0B}
.tool_discovery .badge{background:#0F2B1B;color:#22C55E}
.tech_trend .badge{background:#0F1B2B;color:#3B82F6}
.learning .badge{background:#1B0F2B;color:#A855F7}
.content{flex:1;min-width:0}
.title{font-weight:500;line-height:1.4}
.meta{font-size:11px;color:#8A8A8A;margin-top:4px}
.summary{font-size:12px;color:#A0A0A0;margin-top:6px;line-height:1.4}
.empty{padding:32px 16px;text-align:center;color:#8A8A8A}
.footer{padding:12px 16px;border-top:1px solid #2A2A2A;text-align:center}
.footer a{color:#8A8A8A;text-decoration:none;font-size:11px}
.footer a:hover{color:#FFF}
</style></head><body>
<div class="header"><h3>Signals</h3></div>
${items}
<div class="footer"><a href="#" onclick="vscode.postMessage({type:'openApp'})">Open in 4DA</a></div>
<script nonce="${nonce}">const vscode=acquireVsCodeApi();function openUrl(u){if(u)vscode.postMessage({type:'openUrl',url:u})}</script>
</body></html>`;
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
