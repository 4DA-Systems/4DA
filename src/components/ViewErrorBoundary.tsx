import { Component, ErrorInfo, ReactNode } from 'react';
import i18n from '../i18n';

interface ViewErrorBoundaryProps {
  viewName: string;
  children: ReactNode;
  onReset?: () => void;
}

interface ViewErrorBoundaryState {
  hasError: boolean;
}

export class ViewErrorBoundary extends Component<ViewErrorBoundaryProps, ViewErrorBoundaryState> {
  constructor(props: ViewErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ViewErrorBoundaryState {
    return { hasError: true };
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
          <h2 className="text-lg font-semibold text-white mb-2">
            {i18n.t('error.viewFailed', {
              viewName: this.props.viewName,
            })}
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            {i18n.t('error.viewRecovery')}
          </p>
          <button
            onClick={this.handleRetry}
            className="px-4 py-2 text-sm font-medium bg-bg-tertiary text-white border border-border rounded-lg hover:bg-bg-secondary transition-colors"
          >
            {i18n.t('error.retry')}
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
