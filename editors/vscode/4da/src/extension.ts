import * as vscode from 'vscode';
import { StatusBarManager } from './statusBar';
import { SignalPanelProvider } from './signalPanel';
import { HoverProvider } from './hoverProvider';
import { DiagnosticsManager } from './diagnostics';
import { MCPClient } from './mcpClient';

let statusBar: StatusBarManager;
let diagnostics: DiagnosticsManager;
let panelProvider: SignalPanelProvider;
let mcpClient: MCPClient;
let refreshInterval: ReturnType<typeof setInterval> | undefined;
let lastSecurityCount = -1;

export async function activate(context: vscode.ExtensionContext) {
    mcpClient = new MCPClient();

    // Connect to MCP server in the background (non-blocking)
    mcpClient.connect().then(connected => {
        if (connected) {
            console.log('[4DA] MCP server connected');
        } else {
            console.log('[4DA] MCP server not available — running in degraded mode');
        }
    });

    // Status bar — signal count in bottom bar
    statusBar = new StatusBarManager(mcpClient);
    context.subscriptions.push(statusBar);

    // Signal panel — sidebar webview with signal feed
    panelProvider = new SignalPanelProvider(context.extensionUri, mcpClient);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('4da.signalPanel', panelProvider)
    );
    context.subscriptions.push(panelProvider);

    // Hover provider — dependency info on import statements
    const hoverProvider = new HoverProvider(mcpClient);
    const languages = [
        'typescript', 'typescriptreact', 'javascript', 'javascriptreact',
        'python', 'rust', 'go', 'ruby',
    ];
    for (const lang of languages) {
        context.subscriptions.push(
            vscode.languages.registerHoverProvider(lang, hoverProvider)
        );
    }

    // Diagnostics — inline vulnerability warnings
    diagnostics = new DiagnosticsManager(mcpClient, statusBar);
    context.subscriptions.push(diagnostics);

    // Commands
    context.subscriptions.push(
        vscode.commands.registerCommand('4da.showSignals', () => {
            vscode.commands.executeCommand('4da-signals.focus');
        }),
        vscode.commands.registerCommand('4da.refreshSignals', async () => {
            await statusBar.refresh();
            panelProvider.refresh();
            diagnostics.refresh();
        }),
        vscode.commands.registerCommand('4da.openApp', () => {
            vscode.env.openExternal(vscode.Uri.parse('4da://open'));
        })
    );

    // Periodic refresh with security alert detection
    const config = vscode.workspace.getConfiguration('4da');
    const interval = (config.get<number>('refreshInterval') ?? 300) * 1000;
    refreshInterval = setInterval(async () => {
        await statusBar.refresh();
        diagnostics.refresh();
        await checkSecurityAlerts();
    }, interval);

    // Initial load
    await statusBar.refresh();
    diagnostics.refresh();
    await checkSecurityAlerts();
}

/**
 * Check for security alerts and show toast notifications when new ones appear.
 * Only fires when the count changes to avoid notification spam.
 */
async function checkSecurityAlerts() {
    try {
        const signals = await mcpClient.getSignals();
        const securitySignals = signals.filter(
            s => s.signalType === 'security_alert'
        );
        const count = securitySignals.length;

        if (count > 0 && count !== lastSecurityCount && lastSecurityCount !== -1) {
            const msg = count === 1
                ? `4DA: Security alert — ${securitySignals[0].title}`
                : `4DA: ${count} security alerts detected`;

            vscode.window.showWarningMessage(msg, 'View Details').then(action => {
                if (action === 'View Details') {
                    vscode.commands.executeCommand('4da.showSignals');
                }
            });
        }

        lastSecurityCount = count;
    } catch {
        // Silently fail — security check is non-critical
    }
}

export function deactivate() {
    if (refreshInterval) {
        clearInterval(refreshInterval);
    }
    // Clean up MCP server subprocess
    if (mcpClient) {
        mcpClient.disconnect();
    }
}
