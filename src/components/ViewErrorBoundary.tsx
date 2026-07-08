// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { ErrorInfo, ReactNode } from 'react';
import { Component } from 'react';
import i18n from '../i18n';

interface ViewErrorBoundaryProps {
  viewName: string;
  children: ReactNode;
  onReset?: () => void;
  /**
   * When this value changes, the boundary clears any captured error and
   * re-renders its children. Use it to recover automatically when the thing
   * being rendered changes (e.g. the Signal List/Graph toggle) without the
   * user having to hit Retry.
   */
  resetKey?: string | number;
}

interface ViewErrorBoundaryState {
  hasError: boolean;
  // Tracked so a change in the mounted view (or resetKey) auto-clears a prior
  // error. Without this, the single boundary instance that React reuses across
  // the tab-switch ternary stays stuck in its error state — the user clicks
  // another tab and still sees "failed to load", unable to navigate away.
  lastViewName: string;
  lastResetKey: string | number | undefined;
}

export class ViewErrorBoundary extends Component<ViewErrorBoundaryProps, ViewErrorBoundaryState> {
  constructor(props: ViewErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, lastViewName: props.viewName, lastResetKey: props.resetKey };
  }

  static getDerivedStateFromError(): Partial<ViewErrorBoundaryState> {
    return { hasError: true };
  }

  static getDerivedStateFromProps(
    props: ViewErrorBoundaryProps,
    state: ViewErrorBoundaryState,
  ): Partial<ViewErrorBoundaryState> | null {
    // A different view mounted here, or the caller bumped resetKey — drop the
    // stale error so navigation and view toggles always recover on their own.
    if (props.viewName !== state.lastViewName || props.resetKey !== state.lastResetKey) {
      return { hasError: false, lastViewName: props.viewName, lastResetKey: props.resetKey };
    }
    return null;
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error(`ViewErrorBoundary [${this.props.viewName}]:`, error, errorInfo);
  }

  handleRetry = () => {
    this.setState({ hasError: false });
    this.props.onReset?.();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div
          role="alert"
          className="bg-bg-secondary border border-red-500/20 rounded-xl p-6"
        >
          <h2 className="text-lg font-semibold text-text-primary mb-2">
            {i18n.t('error.viewFailed', {
              viewName: this.props.viewName,
            })}
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            {i18n.t('error.viewRecovery')}
          </p>
          <button
            onClick={this.handleRetry}
            className="px-4 py-2 text-sm font-medium bg-bg-tertiary text-text-primary border border-border rounded-lg hover:bg-bg-secondary transition-colors"
          >
            {i18n.t('error.retry')}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
