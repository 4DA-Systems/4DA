// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { ErrorInfo, ReactNode } from 'react';
import { Component } from 'react';
import i18n from '../i18n';

interface PanelErrorBoundaryProps {
  name: string;
  children: ReactNode;
}

interface PanelErrorBoundaryState {
  hasError: boolean;
  errorMessage: string | null;
}

/**
 * Lightweight error boundary for settings panels and collapsible sections.
 * Catches render errors in a single panel without crashing the parent modal.
 */
export class PanelErrorBoundary extends Component<PanelErrorBoundaryProps, PanelErrorBoundaryState> {
  constructor(props: PanelErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, errorMessage: null };
  }

  static getDerivedStateFromError(error: Error): PanelErrorBoundaryState {
    return { hasError: true, errorMessage: error.message || null };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`PanelErrorBoundary [${this.props.name}]:`, error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, errorMessage: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div role="alert" className="bg-red-500/5 border border-red-500/20 rounded-lg p-4">
          <p className="text-sm text-red-400">
            {i18n.t('panel.failedToLoad', { name: this.props.name })}
          </p>
          {this.state.errorMessage && (
            <p className="text-xs text-text-muted mt-1">{this.state.errorMessage}</p>
          )}
          {this.props.name === 'AI Provider' && (
            <p className="text-xs text-text-muted mt-1.5">
              Check that an API key is configured in Settings &gt; General &gt; AI Provider.
            </p>
          )}
          <button
            onClick={this.handleRetry}
            className="mt-2 px-3 py-1.5 text-xs font-medium text-text-secondary bg-bg-tertiary border border-border rounded-lg hover:text-white transition-colors"
          >
            {i18n.t('action.retry')}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
