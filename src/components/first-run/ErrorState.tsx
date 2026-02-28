import { useTranslation } from 'react-i18next';

interface ErrorStateProps {
  status: string;
  onRetry: () => void;
  onContinue: () => void;
}

export function ErrorState({ status, onRetry, onContinue }: ErrorStateProps) {
  const { t } = useTranslation();

  const isEmbeddingError = status?.includes('Embedding');
  const isFetchError = status?.includes('fetch');

  return (
    <div className="text-center px-8 max-w-md">
      <div className={`w-20 h-20 mx-auto mb-6 rounded-2xl border flex items-center justify-center ${
        isEmbeddingError
          ? 'bg-amber-500/10 border-amber-500/30'
          : 'bg-red-500/10 border-red-500/30'
      }`}>
        <svg className={`w-8 h-8 ${isEmbeddingError ? 'text-amber-400' : 'text-red-400'}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {isEmbeddingError ? (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" />
          ) : (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z" />
          )}
        </svg>
      </div>
      <h2 className="text-xl font-medium text-white mb-2">{t('firstRun.errorTitle')}</h2>
      <p className="text-sm text-gray-400 mb-4">
        {isEmbeddingError
          ? t('firstRun.errorEmbedding')
          : isFetchError
          ? t('firstRun.errorFetch')
          : t('firstRun.errorGeneric')}
      </p>
      {isEmbeddingError && (
        <p className="text-xs text-gray-500 mb-6 px-4">
          {t('firstRun.basicModeExplainer')}
        </p>
      )}
      {!isEmbeddingError && <div className="mb-6" />}
      <div className="flex flex-col items-center gap-3">
        <div className="flex items-center gap-3">
          <button
            onClick={onRetry}
            aria-label="Retry analysis"
            className="px-6 py-3 bg-orange-500 text-white font-medium rounded-lg hover:bg-orange-600 transition-colors"
          >
            {t('firstRun.tryAgain')}
          </button>
          <button
            onClick={onContinue}
            className="px-6 py-3 text-gray-400 hover:text-white transition-colors text-sm"
          >
            {t('firstRun.continueAnyway')}
          </button>
        </div>
        <p className="text-xs text-gray-600">
          {t('firstRun.settingsHint')}
        </p>
      </div>
    </div>
  );
}
