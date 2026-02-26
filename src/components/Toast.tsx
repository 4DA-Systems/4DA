import { useTranslation } from 'react-i18next';
import type { Toast as ToastData, ToastType } from '../hooks/use-toasts';

const typeStyles: Record<ToastType, { border: string; icon: string; text: string }> = {
  success: { border: 'border-l-green-500', icon: '\u2713', text: 'text-green-400' },
  error: { border: 'border-l-red-500', icon: '\u2717', text: 'text-red-400' },
  warning: { border: 'border-l-yellow-500', icon: '!', text: 'text-yellow-400' },
  info: { border: 'border-l-blue-500', icon: 'i', text: 'text-blue-400' },
};

interface ToastContainerProps {
  toasts: ToastData[];
  onDismiss: (id: number) => void;
}

export function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  const { t } = useTranslation();
  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm" role="status" aria-live="polite">
      {toasts.map(toast => {
        const style = typeStyles[toast.type];
        return (
          <div
            key={toast.id}
            className={`bg-bg-secondary border border-border ${style.border} border-l-4 rounded-lg px-4 py-3 shadow-lg flex items-start gap-3 animate-slide-in`}
          >
            <span className={`${style.text} font-bold text-sm flex-shrink-0 mt-0.5`}>
              {style.icon}
            </span>
            <p className="text-sm text-gray-300 flex-1 min-w-0 break-words">{toast.message}</p>
            {toast.action && (
              <button
                onClick={() => { toast.action!.onClick(); onDismiss(toast.id); }}
                className="text-xs font-medium text-orange-400 hover:text-orange-300 transition-colors flex-shrink-0 ml-1 px-2 py-1 bg-orange-500/10 rounded"
              >
                {toast.action.label}
              </button>
            )}
            <button
              onClick={() => onDismiss(toast.id)}
              className="text-gray-600 hover:text-gray-300 transition-colors flex-shrink-0 ml-1"
              aria-label={t('action.dismiss')}
            >
              x
            </button>
          </div>
        );
      })}
    </div>
  );
}
