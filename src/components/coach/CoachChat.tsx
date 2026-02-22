import { useState, useRef, useEffect, useCallback } from 'react';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import type { CoachMessage } from '../../types/coach';

// ---------------------------------------------------------------------------
// Simple markdown renderer (no external library)
// ---------------------------------------------------------------------------

function renderSimpleMarkdown(text: string): React.ReactNode[] {
  const lines = text.split('\n');
  const elements: React.ReactNode[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Headings
    if (line.startsWith('### ')) {
      elements.push(
        <h4 key={i} className="text-sm font-semibold text-white mt-3 mb-1">
          {renderInline(line.slice(4))}
        </h4>,
      );
      continue;
    }
    if (line.startsWith('## ')) {
      elements.push(
        <h3 key={i} className="text-base font-semibold text-white mt-3 mb-1">
          {renderInline(line.slice(3))}
        </h3>,
      );
      continue;
    }
    if (line.startsWith('# ')) {
      elements.push(
        <h2 key={i} className="text-lg font-semibold text-white mt-3 mb-1">
          {renderInline(line.slice(2))}
        </h2>,
      );
      continue;
    }

    // Bullet lists
    if (line.startsWith('- ') || line.startsWith('* ')) {
      elements.push(
        <li key={i} className="ml-4 list-disc text-[#A0A0A0]">
          {renderInline(line.slice(2))}
        </li>,
      );
      continue;
    }

    // Numbered lists
    const numberedMatch = line.match(/^(\d+)\.\s/);
    if (numberedMatch) {
      elements.push(
        <li key={i} className="ml-4 list-decimal text-[#A0A0A0]">
          {renderInline(line.slice(numberedMatch[0].length))}
        </li>,
      );
      continue;
    }

    // Empty line -> spacer
    if (line.trim() === '') {
      elements.push(<div key={i} className="h-2" />);
      continue;
    }

    // Regular paragraph
    elements.push(
      <p key={i} className="text-[#A0A0A0]">
        {renderInline(line)}
      </p>,
    );
  }

  return elements;
}

/** Render inline markdown: **bold**, `code` */
function renderInline(text: string): React.ReactNode {
  const parts: React.ReactNode[] = [];
  // Regex for bold (**text**) and inline code (`text`)
  const regex = /(\*\*(.+?)\*\*|`([^`]+)`)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(text)) !== null) {
    // Text before the match
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }

    if (match[2]) {
      // Bold
      parts.push(
        <strong key={match.index} className="font-semibold text-white">
          {match[2]}
        </strong>,
      );
    } else if (match[3]) {
      // Inline code
      parts.push(
        <code
          key={match.index}
          className="px-1 py-0.5 bg-[#1F1F1F] rounded text-[#D4AF37] text-xs font-mono"
        >
          {match[3]}
        </code>,
      );
    }

    lastIndex = match.index + match[0].length;
  }

  // Remaining text
  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>;
}

// ---------------------------------------------------------------------------
// Suggested prompts for empty state
// ---------------------------------------------------------------------------

const SUGGESTED_PROMPTS = [
  'What revenue engine fits my profile best?',
  'Help me create a 30-day launch plan',
  'Review my project idea for market fit',
];

// ---------------------------------------------------------------------------
// Loading dots animation
// ---------------------------------------------------------------------------

function LoadingDots() {
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

function MessageBubble({ message }: { message: CoachMessage }) {
  const isUser = message.role === 'user';

  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'} mb-3`}>
      <div
        className={`max-w-[80%] rounded-xl px-4 py-3 text-sm leading-relaxed ${
          isUser
            ? 'bg-[#1F1F1F] text-white rounded-br-sm'
            : 'bg-[#141414] border border-[#2A2A2A] text-[#A0A0A0] rounded-bl-sm'
        }`}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className="space-y-1">{renderSimpleMarkdown(message.content)}</div>
        )}

        {/* Token cost metadata for assistant messages */}
        {!isUser && (message.token_count > 0 || message.cost_cents > 0) && (
          <div className="flex items-center gap-2 mt-2 pt-2 border-t border-[#2A2A2A]">
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

// ===========================================================================
// CoachChat (main export)
// ===========================================================================

export function CoachChat() {
  const {
    messages,
    loading,
    activeSessionId,
  } = useAppStore(
    useShallow(s => ({
      messages: s.coachMessages,
      loading: s.coachLoading,
      activeSessionId: s.activeSessionId,
    })),
  );

  const sendMessage = useAppStore(s => s.sendCoachMessage);
  const createSession = useAppStore(s => s.createCoachSession);

  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-scroll to bottom when messages change or loading state changes
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, loading]);

  // Auto-resize textarea
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta) return;
    ta.style.height = 'auto';
    // Cap at ~4 lines (4 * 20px line-height + padding)
    ta.style.height = `${Math.min(ta.scrollHeight, 96)}px`;
  }, [input]);

  const handleSend = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || loading) return;

    // Auto-create a chat session if none is active
    let sessionId = activeSessionId;
    if (!sessionId) {
      sessionId = await createSession('chat');
      if (!sessionId) return;
    }

    setInput('');
    await sendMessage(trimmed);
  }, [input, loading, activeSessionId, createSession, sendMessage]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleSuggestedPrompt = useCallback(
    (prompt: string) => {
      setInput(prompt);
      // Focus textarea so user can review or immediately send
      textareaRef.current?.focus();
    },
    [],
  );

  const isEmpty = messages.length === 0 && !loading;

  return (
    <div className="flex flex-col flex-1 min-h-0">
      {/* Messages area */}
      <div className="flex-1 overflow-y-auto px-2 py-3 min-h-0">
        {isEmpty ? (
          // Empty / welcome state
          <div className="flex flex-col items-center justify-center h-full text-center px-4">
            <div className="w-12 h-12 bg-[#D4AF37]/10 rounded-xl flex items-center justify-center mb-4">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#D4AF37" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
              </svg>
            </div>
            <h3 className="text-base font-semibold text-white mb-1">STREETS Coach</h3>
            <p className="text-sm text-[#A0A0A0] max-w-sm mb-5">
              Ask anything about building independent developer income.
              Your sovereign profile and playbook progress inform every answer.
            </p>
            <div className="flex flex-col gap-2 w-full max-w-sm">
              {SUGGESTED_PROMPTS.map(prompt => (
                <button
                  key={prompt}
                  onClick={() => handleSuggestedPrompt(prompt)}
                  className="text-left px-4 py-2.5 bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg text-sm text-[#A0A0A0] hover:border-[#D4AF37]/40 hover:text-white transition-colors"
                >
                  {prompt}
                </button>
              ))}
            </div>
          </div>
        ) : (
          // Message list
          <>
            {messages.map(msg => (
              <MessageBubble key={msg.id} message={msg} />
            ))}
            {loading && (
              <div className="flex justify-start mb-3">
                <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl rounded-bl-sm">
                  <LoadingDots />
                </div>
              </div>
            )}
            <div ref={messagesEndRef} />
          </>
        )}
      </div>

      {/* Input area */}
      <div className="flex-shrink-0 border-t border-[#2A2A2A] pt-3 px-1">
        <div className="flex items-end gap-2">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask your coach..."
            rows={1}
            className="flex-1 resize-none bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg px-3 py-2 text-sm text-white placeholder-[#666] focus:outline-none focus:border-[#D4AF37]/50 leading-5"
          />
          <button
            onClick={handleSend}
            disabled={!input.trim() || loading}
            className="flex-shrink-0 px-4 py-2 bg-[#D4AF37] text-black text-sm font-medium rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Send
          </button>
        </div>
        <p className="text-[10px] text-[#666] mt-1.5 pl-1">
          Shift+Enter for newline. Responses use your configured AI provider.
        </p>
      </div>
    </div>
  );
}
