import { useLicense } from '../hooks/use-license';

interface ProGateProps {
  children: React.ReactNode;
  feature?: string;
}

export function ProGate({ children, feature }: ProGateProps) {
  const { isPro } = useLicense();

  if (isPro) {
    return <>{children}</>;
  }

  return (
    <div className="relative">
      <div className="opacity-30 pointer-events-none select-none blur-[2px]">
        {children}
      </div>
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="bg-bg-secondary/95 backdrop-blur-sm border border-[#D4AF37]/30 rounded-xl px-6 py-5 text-center max-w-sm shadow-lg">
          <div className="flex items-center justify-center gap-2 mb-3">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="text-[#D4AF37]">
              <path d="M8 1L10 6H15L11 9.5L12.5 15L8 11.5L3.5 15L5 9.5L1 6H6L8 1Z" fill="currentColor"/>
            </svg>
            <span className="text-sm font-semibold text-[#D4AF37] tracking-wide uppercase">Pro</span>
          </div>
          <p className="text-sm text-gray-300 mb-1">
            {feature ? `${feature} is a Pro feature` : 'This is a Pro feature'}
          </p>
          <p className="text-xs text-gray-500 mb-4">
            Upgrade to unlock AI briefings, intelligence panels, and more.
          </p>
          <a
            href="https://4da.ai/pro"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block px-5 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors"
          >
            Upgrade to Pro
          </a>
        </div>
      </div>
    </div>
  );
}
