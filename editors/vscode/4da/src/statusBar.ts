import * as vscode from 'vscode';
import type { MCPClient } from './mcpClient';

export class StatusBarManager implements vscode.Disposable {
    private item: vscode.StatusBarItem;
    private client: MCPClient;

    constructor(client: MCPClient) {
        this.client = client;
        this.item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
        this.item.command = '4da.showSignals';
        this.item.show();
        this.setLoading();
    }

    private setLoading() {
        this.item.text = '$(sync~spin) 4DA';
        this.item.tooltip = 'Loading signals...';
    }

    async refresh() {
        try {
            const counts = await this.client.getSignalCount();
            if (counts.total === 0) {
                this.item.text = '$(check) 4DA';
                this.item.tooltip = 'No new signals';
                this.item.backgroundColor = undefined;
            } else if (counts.security > 0) {
                this.item.text = `$(shield) 4DA: ${counts.total}`;
                this.item.tooltip = `${counts.total} signals (${counts.security} security)`;
                this.item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
            } else {
                this.item.text = `$(pulse) 4DA: ${counts.total}`;
                this.item.tooltip = `${counts.total} signals`;
                this.item.backgroundColor = undefined;
            }
        } catch {
            this.item.text = '$(circle-slash) 4DA';
            this.item.tooltip = '4DA: Not connected';
            this.item.backgroundColor = undefined;
        }
    }

    dispose() {
        this.item.dispose();
    }
}
