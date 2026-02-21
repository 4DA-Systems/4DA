import type { ToolDescriptor } from '../../types/toolkit';

interface ToolkitCardProps {
  tool: ToolDescriptor;
  pinned: boolean;
  onOpen: () => void;
  onTogglePin: () => void;
}

const ICON_PATHS: Record<string, string> = {
  braces: 'M4 6a2 2 0 0 1 2-2h1a1 1 0 0 1 0 2H6v3a2 2 0 0 1-1 1.73A2 2 0 0 1 6 12v3h1a1 1 0 0 1 0 2H6a2 2 0 0 1-2-2v-3a1 1 0 0 0-1-1 1 1 0 0 1 0-2 1 1 0 0 0 1-1V6Zm16 0a2 2 0 0 0-2-2h-1a1 1 0 0 0 0 2h1v3a2 2 0 0 0 1 1.73A2 2 0 0 0 18 12v3h-1a1 1 0 0 0 0 2h1a2 2 0 0 0 2-2v-3a1 1 0 0 1 1-1 1 1 0 0 0 0-2 1 1 0 0 1-1-1V6Z',
  regex: 'M6 4v4a2 2 0 0 0 4 0V4M14 4l3 4-3 4M17 4l-3 4 3 4M6 16l2-4M10 16l-2-4',
  palette: 'M12 2a10 10 0 0 0 0 20 2 2 0 0 0 2-2v-.7a2 2 0 0 1 2-2h1.1A2 2 0 0 0 19 15.3 8 8 0 0 0 12 2Zm-4.5 9a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Zm3-4a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Zm5 0a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Z',
  lock: 'M12 2a4 4 0 0 0-4 4v2H7a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V6a4 4 0 0 0-4-4Zm2 6V6a2 2 0 0 0-4 0v2h4Z',
  diff: 'M5 4h14M5 8h8M5 12h14M5 16h8M5 20h14',
  clock: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 4v6l4 2',
  database: 'M12 2C6.48 2 2 3.79 2 6v12c0 2.21 4.48 4 10 4s10-1.79 10-4V6c0-2.21-4.48-4-10-4Zm0 16c-4.42 0-8-1.34-8-3V6c0-1.66 3.58-3 8-3s8 1.34 8 3v9c0 1.66-3.58 3-8 3Z',
  network: 'M5 12h14M12 5v14M5 5l14 14M19 5L5 19M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Z',
  terminal: 'M4 17l6-5-6-5M12 19h8',
  globe: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10A15.3 15.3 0 0 1 12 2Z',
  // Intelligence tool icons
  heartPulse: 'M19.5 12.6l-7.5 7.4-7.5-7.4A5 5 0 0 1 12 4.5a5 5 0 0 1 7.5 8.1ZM5 12h3l2-4 3 8 2-4h4',
  shield: 'M12 2l8 4v6c0 5.5-3.8 10.7-8 12-4.2-1.3-8-6.5-8-12V6l8-4Zm0 6v4m0 4h.01',
  rss: 'M4 11a9 9 0 0 1 9 9M4 4a16 16 0 0 1 16 16M5 20a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z',
  target: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 4a6 6 0 1 0 0 12 6 6 0 0 0 0-12Zm0 4a2 2 0 1 0 0 4 2 2 0 0 0 0-4Z',
  scale: 'M12 2v20M2 7l10-5 10 5M4 7v5c0 2 3.5 4 8 4s8-2 8-4V7',
  package: 'M16.5 9.4l-9-5.2M21 16V8a2 2 0 0 0-1-1.7l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.7l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16ZM3.3 7L12 12l8.7-5M12 22V12',
};

export function ToolkitCard({ tool, pinned, onOpen, onTogglePin }: ToolkitCardProps) {
  const iconPath = ICON_PATHS[tool.icon];

  return (
    <button
      onClick={onOpen}
      className="group relative text-left p-4 bg-bg-secondary border border-border rounded-xl hover:border-white/20 hover:bg-bg-tertiary transition-all focus:outline-none focus:ring-1 focus:ring-white/30"
    >
      {/* Pin button */}
      <button
        onClick={(e) => { e.stopPropagation(); onTogglePin(); }}
        className={`absolute top-2 right-2 p-1 rounded transition-opacity ${
          pinned ? 'opacity-100 text-[#D4AF37]' : 'opacity-0 group-hover:opacity-60 text-gray-500 hover:text-white'
        }`}
        title={pinned ? 'Unpin' : 'Pin to top'}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill={pinned ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="2">
          <path d="M12 2l2.4 7.4H22l-6.2 4.5 2.4 7.4L12 16.8l-6.2 4.5 2.4-7.4L2 9.4h7.6z"/>
        </svg>
      </button>

      {/* Pro badge */}
      {tool.pro && (
        <span className="absolute top-2 left-2 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30 rounded">
          Pro
        </span>
      )}

      {/* Icon */}
      <div className="w-10 h-10 mb-3 flex items-center justify-center rounded-lg bg-bg-tertiary border border-border group-hover:border-white/20 transition-colors">
        {iconPath ? (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-gray-400 group-hover:text-white transition-colors">
            <path d={iconPath}/>
          </svg>
        ) : (
          <span className="text-sm text-gray-500">{tool.icon}</span>
        )}
      </div>

      {/* Name + description */}
      <h3 className="text-sm font-medium text-white mb-1 group-hover:text-white transition-colors">
        {tool.name}
      </h3>
      <p className="text-xs text-gray-500 leading-relaxed line-clamp-2">
        {tool.description}
      </p>
    </button>
  );
}
