import * as vscode from 'vscode';
import { StatusBarManager } from './statusBar';
import { SignalPanelProvider } from './signalPanel';
import { HoverProvider } from './hoverProvider';
import { DiagnosticsManager } from './diagnostics';
import { MCPClient } from './mcpClient';

let statusBar: StatusBarManager;
let diagnostics: DiagnosticsManager;
let mcpClient: MCPClient;
let refreshInterval: ReturnType<typeof setInterval> | undefined;

export async function activate(context: vscode.ExtensionContext) {
    mcpClient = new MCPClient();

    // Status bar — signal count in bottom bar
    statusBar = new StatusBarManager(mcpClient);
    context.subscriptions.push(statusBar);

    // Signal panel — sidebar webview with signal feed
    const panelProvider = new SignalPanelProvider(context.extensionUri, mcpClient);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('4da.signalPanel', panelProvider)
    );

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
    diagnostics = new DiagnosticsManager(mcpClient);
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

    // Periodic refresh
    const config = vscode.workspace.getConfiguration('4da');
    const interval = (config.get<number>('refreshInterval') ?? 300) * 1000;
    refreshInterval = setInterval(() => {
        statusBar.refresh();
        diagnostics.refresh();
    }, interval);

    // Initial load
    await statusBar.refresh();
    diagnostics.refresh();
}

export function deactivate() {
    if (refreshInterval) {
        clearInterval(refreshInterval);
    }
}
