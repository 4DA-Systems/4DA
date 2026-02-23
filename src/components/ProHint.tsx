import { useLicense } from '../hooks/use-license';

interface ProHintProps {
  /** What Pro users see (the actual intelligence) */
  children: React.ReactNode;
  /** Short label for the feature, shown to free users */
  feature: string;
}

/**
 * Lightweight inline Pro teaser for feed items.
 * Unlike ProGate (which blurs + overlays), ProHint shows a subtle
 * clickable hint for free users and full content for Pro users.
 */
export function ProHint({ children, feature }: ProHintProps) {
  const { isPro } = useLicense();

  if (isPro) {
    return <>{children}</>;
  }

  return (
    <a
      href="https://4da.ai/streets"
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center gap-1 text-[10px] text-[#D4AF37]/60 hover:text-[#D4AF37] transition-colors group"
    >
      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" className="opacity-60 group-hover:opacity-100">
        <path d="M8 1L10 6H15L11 9.5L12.5 15L8 11.5L3.5 15L5 9.5L1 6H6L8 1Z" fill="currentColor"/>
      </svg>
      <span>Pro: {feature}</span>
    </a>
  );
}
