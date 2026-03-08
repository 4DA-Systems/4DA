import { Component, ErrorInfo, ReactNode } from 'react';

interface PanelErrorBoundaryProps {
  name: string;
  children: ReactNode;
}

interface PanelErrorBoundaryState {
  hasError: boolean;
}

/**
 * Lightweight error boundary for settings panels and collapsible sections.
 * Catches render errors in a single panel without crashing the parent modal.
 */
export class PanelErrorBoundary extends Component<PanelErrorBoundaryProps, PanelErrorBoundaryState> {
  constructor(props: PanelErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): PanelErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`PanelErrorBoundary [${this.props.name}]:`, error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div role="alert" className="bg-red-500/5 border border-red-500/20 rounded-lg p-4">
          <p className="text-sm text-red-400">
            {this.props.name} failed to load.
          </p>
          <button
            onClick={this.handleRetry}
            className="mt-2 px-3 py-1.5 text-xs font-medium text-gray-300 bg-bg-tertiary border border-border rounded-lg hover:text-white transition-colors"
          >
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
