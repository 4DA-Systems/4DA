import { Suspense, Component, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import i18next from 'i18next';

interface ToolkitShellProps {
  toolName: string;
  onBack: () => void;
  children: ReactNode;
}

interface EBState { hasError: boolean; error: Error | null }

class ToolErrorBoundary extends Component<{ toolName: string; onBack: () => void; children: ReactNode }, EBState> {
  constructor(props: { toolName: string; onBack: () => void; children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }
  static getDerivedStateFromError(error: Error): EBState {
    return { hasError: true, error };
  }
  componentDidCatch(error: Error) {
    console.error(`Toolkit tool "${this.props.toolName}" crashed:`, error);
  }
  render() {
    if (this.state.hasError) {
      return (
        <div role="alert" className="p-6 bg-bg-secondary border border-red-500/30 rounded-xl">
          <h3 className="text-sm font-medium text-red-400 mb-2">
            {i18next.t('toolkit.shell.errorEncountered', { toolName: this.props.toolName })}
          </h3>
          <pre className="text-xs text-text-muted font-mono bg-bg-tertiary rounded p-3 mb-4 overflow-auto max-h-32">
            {this.state.error?.message}
          </pre>
          <div className="flex gap-2">
            <button
              onClick={() => this.setState({ hasError: false, error: null })}
              className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary border border-border rounded-lg hover:bg-white/10 transition-colors"
            >
              {i18next.t('action.retry')}
            </button>
            <button
              onClick={this.props.onBack}
              className="px-3 py-1.5 text-xs text-text-muted hover:text-white transition-colors"
            >
              {i18next.t('toolkit.shell.backToToolkit')}
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

function LoadingFallback() {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-center py-20" role="status" aria-busy="true">
      <div className="flex items-center gap-3 text-text-muted">
        <div className="w-4 h-4 border-2 border-gray-600 border-t-white rounded-full animate-spin" />
        <span className="text-sm">{t('toolkit.shell.loadingTool')}</span>
      </div>
    </div>
  );
}

export function ToolkitShell({ toolName, onBack, children }: ToolkitShellProps) {
  const { t } = useTranslation();
  return (
    <div>
      {/* Header bar */}
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={onBack}
          aria-label={t('toolkit.shell.back')}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-text-secondary bg-bg-secondary border border-border rounded-lg hover:text-white hover:border-white/20 transition-all"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M19 12H5M12 19l-7-7 7-7"/>
          </svg>
          {t('toolkit.shell.back')}
        </button>
        <h2 className="text-sm font-medium text-white">{toolName}</h2>
        <span className="text-[10px] text-text-muted ms-auto">{t('toolkit.shell.escToClose')}</span>
      </div>

      {/* Tool content with error boundary */}
      <ToolErrorBoundary toolName={toolName} onBack={onBack}>
        <Suspense fallback={<LoadingFallback />}>
          {children}
        </Suspense>
      </ToolErrorBoundary>
    </div>
  );
}
