import * as vscode from 'vscode';
import type { MCPClient } from './mcpClient';

export class StatusBarManager implements vscode.Disposable {
    private item: vscode.StatusBarItem;
    private client: MCPClient;
    private previousCount = -1;
    private pulseTimer: ReturnType<typeof setTimeout> | undefined;
    private fileAlertCount = 0;

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

    /**
     * Update file-specific alert count shown in the status bar.
     * Called by DiagnosticsManager when diagnostics change for the active file.
     */
    setFileAlertCount(count: number) {
        this.fileAlertCount = count;
        // Re-render with current data — don't re-fetch
        this.updateDisplay();
    }

    private updateDisplay() {
        // Only augment with file context if we have a valid previous count
        if (this.previousCount < 0) return;

        const total = this.previousCount;
        if (total === 0 && this.fileAlertCount === 0) {
            this.item.text = '$(check) 4DA';
            this.item.tooltip = 'No new signals';
        } else if (this.fileAlertCount > 0) {
            const alertWord = this.fileAlertCount === 1 ? 'alert' : 'alerts';
            this.item.text = `$(pulse) 4DA: ${total} · ${this.fileAlertCount} ${alertWord} here`;
            this.item.tooltip = `${total} signals · ${this.fileAlertCount} ${alertWord} in this file`;
        }
    }

    async refresh() {
        try {
            const counts = await this.client.getSignalCount();

            // Pulse animation: when new signals arrive, flash the background
            if (counts.total > this.previousCount && this.previousCount >= 0) {
                this.pulse(counts.security > 0);
            }

            this.previousCount = counts.total;

            if (counts.total === 0 && this.fileAlertCount === 0) {
                this.item.text = '$(check) 4DA';
                this.item.tooltip = 'No new signals';
                // Only clear background if we're not mid-pulse
                if (!this.pulseTimer) {
                    this.item.backgroundColor = undefined;
                }
            } else if (counts.security > 0) {
                this.item.text = `$(shield) 4DA: ${counts.total}`;
                this.item.tooltip = `${counts.total} signals (${counts.security} security)`;
                this.item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
            } else if (this.fileAlertCount > 0) {
                const alertWord = this.fileAlertCount === 1 ? 'alert' : 'alerts';
                this.item.text = `$(pulse) 4DA: ${counts.total} · ${this.fileAlertCount} ${alertWord} here`;
                this.item.tooltip = `${counts.total} signals · ${this.fileAlertCount} ${alertWord} in this file`;
                if (!this.pulseTimer) {
                    this.item.backgroundColor = undefined;
                }
            } else {
                this.item.text = `$(pulse) 4DA: ${counts.total}`;
                this.item.tooltip = `${counts.total} signals`;
                if (!this.pulseTimer) {
                    this.item.backgroundColor = undefined;
                }
            }
        } catch {
            this.item.text = '$(circle-slash) 4DA';
            this.item.tooltip = '4DA: Not connected';
            this.item.backgroundColor = undefined;
        }
    }

    /**
     * Briefly highlight the status bar item when new signals arrive.
     * Uses warningBackground for security signals, errorBackground for emphasis on new content.
     */
    private pulse(isSecurityRelated: boolean) {
        // Clear any existing pulse
        if (this.pulseTimer) {
            clearTimeout(this.pulseTimer);
        }

        this.item.backgroundColor = new vscode.ThemeColor(
            isSecurityRelated
                ? 'statusBarItem.errorBackground'
                : 'statusBarItem.warningBackground'
        );

        this.pulseTimer = setTimeout(() => {
            this.item.backgroundColor = undefined;
            this.pulseTimer = undefined;
        }, 3000);
    }

    dispose() {
        if (this.pulseTimer) {
            clearTimeout(this.pulseTimer);
        }
        this.item.dispose();
    }
}
