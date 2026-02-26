import { useTranslation } from 'react-i18next';

interface ConfirmGateProps {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmGate({ message, onConfirm, onCancel }: ConfirmGateProps) {
  const { t } = useTranslation();
  return (
    <div role="alertdialog" aria-label={message} className="flex items-center gap-3 px-3 py-2 bg-[#D4AF37]/10 border border-[#D4AF37]/30 rounded-lg animate-in fade-in">
      <span className="text-sm text-[#D4AF37] flex-1">{message}</span>
      <button
        onClick={onConfirm}
        className="px-3 py-1 text-xs font-medium text-black bg-[#D4AF37] rounded hover:bg-[#C4A030] transition-colors"
      >
        {t('commandDeck.confirm')}
      </button>
      <button
        onClick={onCancel}
        className="px-3 py-1 text-xs text-gray-400 hover:text-white transition-colors"
      >
        {t('action.cancel')}
      </button>
    </div>
  );
}
