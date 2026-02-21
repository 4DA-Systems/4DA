import sunLogo from '../assets/sun-logo.jpg';

export function AboutPanel() {
  return (
    <div className="space-y-8">
      {/* Logo + Identity */}
      <div className="flex flex-col items-center text-center">
        <img
          src={sunLogo}
          alt="4DA Sun Logo"
          className="w-28 h-28 rounded-2xl object-cover shadow-lg shadow-orange-500/20 mb-4"
        />
        <h3 className="text-xl font-semibold text-white">4DA</h3>
        <p className="text-sm text-gray-400 mt-1">4 Dimensional Autonomy</p>
        <p className="text-xs text-gray-500 mt-0.5">All signal. No feed.</p>
      </div>

      {/* Built With Section */}
      <div className="bg-bg-tertiary/50 border border-border rounded-xl p-5 space-y-4">
        <h4 className="text-sm font-medium text-white tracking-wide uppercase">
          Development Attribution
        </h4>

        <div className="space-y-3 text-sm text-gray-300 leading-relaxed">
          <p>
            4DA was conceived, designed, and directed by{' '}
            <span className="text-white font-medium">Antony Lawrence Kiddie Pasifa</span>{' '}
            &mdash; product vision, architecture decisions, design philosophy, quality
            standards, and every choice that defines what 4DA is.
          </p>

          <p>
            The codebase was primarily engineered through collaborative sessions with{' '}
            <span className="text-white font-medium">Claude</span>{' '}
            (Anthropic &mdash; Opus 4.5 and Opus 4.6 models), serving as the principal
            implementation engine across thousands of iterative development cycles.
          </p>

          <p className="text-gray-400 text-xs">
            This human-directed, AI-implemented development model is a legitimate and
            increasingly standard approach to software engineering. The complete git
            history serves as a verifiable, externally auditable record of this process.
          </p>
        </div>

        {/* Attribution Visual */}
        <div className="mt-6 flex items-center justify-center gap-6">
          {/* Human creator */}
          <div className="flex flex-col items-center gap-2">
            <div className="w-14 h-14 rounded-xl bg-bg-secondary border border-border flex items-center justify-center text-2xl">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-white">
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
            </div>
            <span className="text-[10px] text-gray-500 font-medium uppercase tracking-wider">Vision</span>
          </div>

          {/* Connection */}
          <div className="flex flex-col items-center gap-1">
            <div className="flex items-center gap-1">
              <div className="w-6 h-px bg-gray-600" />
              <div className="w-2 h-2 rounded-full bg-orange-500/60" />
              <div className="w-6 h-px bg-gray-600" />
            </div>
            <span className="text-[9px] text-gray-600">collaborative</span>
          </div>

          {/* Claude */}
          <div className="flex flex-col items-center gap-2">
            <div className="w-14 h-14 rounded-xl bg-bg-secondary border border-border flex items-center justify-center">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" className="text-[#D97706]">
                <path d="M13.5 2.5L12 4L10.5 2.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                <path d="M12 4v4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                <rect x="6" y="8" width="12" height="10" rx="3" stroke="currentColor" strokeWidth="1.5" />
                <circle cx="9.5" cy="13" r="1" fill="currentColor" />
                <circle cx="14.5" cy="13" r="1" fill="currentColor" />
                <path d="M10 16h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
            </div>
            <span className="text-[10px] text-gray-500 font-medium uppercase tracking-wider">Engine</span>
          </div>
        </div>
      </div>

      {/* Verification Notice */}
      <div className="bg-green-500/5 border border-green-500/20 rounded-lg p-4">
        <div className="flex items-start gap-3">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-green-400 mt-0.5 flex-shrink-0">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
            <polyline points="22 4 12 14.01 9 11.01" />
          </svg>
          <div>
            <p className="text-xs font-medium text-green-400">Externally Verifiable</p>
            <p className="text-xs text-gray-500 mt-1">
              Development process is auditable via the public git history. Every commit,
              co-authorship tag, and architectural decision is preserved in the repository.
            </p>
          </div>
        </div>
      </div>

      {/* Technical Details */}
      <div className="grid grid-cols-2 gap-3 text-xs">
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-gray-500 mb-1">Stack</p>
          <p className="text-gray-300">Rust + React + SQLite</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-gray-500 mb-1">Framework</p>
          <p className="text-gray-300">Tauri 2.0</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-gray-500 mb-1">License</p>
          <p className="text-gray-300">FSL-1.1-Apache-2.0</p>
        </div>
        <div className="bg-bg-tertiary/30 rounded-lg p-3 border border-border/50">
          <p className="text-gray-500 mb-1">Privacy Model</p>
          <p className="text-gray-300">100% Local / BYOK</p>
        </div>
      </div>

      {/* Keyboard Shortcuts */}
      <div className="bg-bg-tertiary/30 border border-border/50 rounded-xl p-5 space-y-3">
        <h4 className="text-sm font-medium text-white tracking-wide uppercase">
          Keyboard Shortcuts
        </h4>
        <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
          {[
            ['R', 'Run analysis'],
            ['F', 'Toggle filter panel'],
            ['B', 'Open briefing'],
            [',', 'Open settings'],
            ['?', 'Show help'],
            ['Esc', 'Close panel / modal'],
            ['Ctrl+`', 'Toggle Command Deck'],
            ['S', 'Save current item'],
            ['J / K', 'Navigate items'],
          ].map(([key, desc]) => (
            <div key={key} className="flex items-center gap-2">
              <kbd className="px-1.5 py-0.5 bg-bg-secondary rounded border border-border text-gray-400 font-mono text-[11px] min-w-[28px] text-center">
                {key}
              </kbd>
              <span className="text-gray-500">{desc}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Version + Copyright */}
      <div className="text-center pt-2 border-t border-border/50">
        <p className="text-xs text-gray-500">
          v1.0.0 &middot; &copy; 2025&ndash;2026 Antony Lawrence Kiddie Pasifa
        </p>
        <p className="text-[10px] text-gray-600 mt-1">
          All rights reserved. Built with vision, engineered with Claude.
        </p>
      </div>
    </div>
  );
}
