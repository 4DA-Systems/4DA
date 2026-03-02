import type { CoachMessage } from '../../types/coach';
import { renderSimpleMarkdown } from './coach-markdown';

// ---------------------------------------------------------------------------
// Suggested prompts for empty state
// ---------------------------------------------------------------------------

export const SUGGESTED_PROMPT_KEYS = [
  'coach.chat.suggestEngine',
  'coach.chat.suggest30DayPlan',
  'coach.chat.suggestMarketFit',
] as const;

// ---------------------------------------------------------------------------
// Loading dots animation
// ---------------------------------------------------------------------------

export function LoadingDots() {
  return (
    <div className="flex items-center gap-1 px-4 py-3">
      <span className="w-2 h-2 rounded-full bg-[#D4AF37] animate-pulse" style={{ animationDelay: '0ms' }} />
      <span className="w-2 h-2 rounded-full bg-[#D4AF37] animate-pulse" style={{ animationDelay: '200ms' }} />
      <span className="w-2 h-2 rounded-full bg-[#D4AF37] animate-pulse" style={{ animationDelay: '400ms' }} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Single message bubble
// ---------------------------------------------------------------------------

export function MessageBubble({ message }: { message: CoachMessage }) {
  const isUser = message.role === 'user';

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-3`}>
      <div
        className={`max-w-[80%] rounded-xl px-4 py-3 text-sm leading-relaxed ${
          isUser
            ? 'bg-bg-tertiary text-white rounded-br-sm'
            : 'bg-bg-secondary border border-border text-text-secondary rounded-bl-sm'
        }`}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className="space-y-1">{renderSimpleMarkdown(message.content)}</div>
        )}

        {/* Token cost metadata for assistant messages */}
        {!isUser && (message.token_count > 0 || message.cost_cents > 0) && (
          <div className="flex items-center gap-2 mt-2 pt-2 border-t border-border">
            {message.cost_cents > 0 && (
              <span className="text-[10px] text-[#666] font-mono">
                cost: {message.cost_cents.toFixed(2)}c
              </span>
            )}
            {message.token_count > 0 && (
              <span className="text-[10px] text-[#666] font-mono">
                {message.token_count} tokens
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
