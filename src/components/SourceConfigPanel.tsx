// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { DefaultSourceList } from './settings/DefaultSourceList';
import { FeedHealthDot } from './settings/FeedHealthDot';
import { SourcePreview } from './settings/SourcePreview';
import { ValidationFeedback } from './settings/ValidationFeedback';
import { useSourceConfig } from './settings/useSourceConfig';

interface SourceConfigPanelProps {
  onStatusChange: (status: string) => void;
}

export function SourceConfigPanel({ onStatusChange }: SourceConfigPanelProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);

  const s = useSourceConfig(onStatusChange);

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <button
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        className="w-full flex items-center gap-3"
      >
        <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-cyan-400">📡</span>
        </div>
        <div className="flex-1 text-start">
          <div className="flex items-center gap-2">
            <h3 className="text-white font-medium">{t('sources.title')}</h3>
            <span className="px-1.5 py-0.5 text-[10px] bg-cyan-500/20 text-cyan-400 rounded">
              {t('sources.customCount', { count: s.totalSources })}
            </span>
          </div>
          <p className="text-text-muted text-sm mt-0.5">
            {t('sources.subtitle')}
          </p>
        </div>
        <span className="text-text-muted text-xs">{expanded ? '−' : '+'}</span>
      </button>

      {expanded && (
        <div className="mt-3 space-y-3">
          {/* RSS Feeds (smart input -- also handles YouTube/Twitter/langs) */}
          <div>
            <label className="text-xs text-text-secondary block mb-1.5">
              {t('sources.rss.label')}
              <span className="text-text-muted ms-1 font-normal">
                {t('sources.rss.smartHint', '-- URL, @handle, or language list')}
              </span>
            </label>
            <div className="flex gap-2 mb-1.5">
              <input
                type="text"
                value={s.newRssFeed}
                onChange={(e) => {
                  s.setNewRssFeed(e.target.value);
                  if (s.rssPreviewOpen) s.setRssPreviewOpen(false);
                  if (s.validationResult) s.setValidationResult(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') { e.preventDefault(); s.addRssFeed(); }
                }}
                placeholder="blog.deno.com · @Fireship · rust, typescript"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={s.addRssFeed}
                disabled={s.validating || !s.newRssFeed.trim()}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
              >
                {t('action.add')}
              </button>
            </div>
            <ValidationFeedback
              validating={s.validating}
              result={s.validationResult}
              onTryFeed={s.tryDiscoveredFeed}
            />
            {s.rssPreviewOpen && s.rssParsed && (
              <SourcePreview
                parsed={s.rssParsed}
                onConfirm={() => { void s.confirmRssAdd(); }}
                onCancel={() => { s.setRssPreviewOpen(false); s.setNewRssFeed(''); }}
              />
            )}
            {s.rssFeeds.length > 0 ? (
              <div className="space-y-1 max-h-24 overflow-y-auto">
                {s.rssFeeds.map((feed) => (
                  <div
                    key={feed}
                    className="flex items-center justify-between px-3 py-1.5 bg-bg-secondary rounded border border-border group"
                  >
                    <span className="font-mono text-xs text-text-secondary truncate">
                      {feed}
                      <FeedHealthDot health={s.feedHealthMap[feed]} onReset={() => s.resetFeedHealth(feed, 'rss')} />
                    </span>
                    <button
                      onClick={() => s.removeRssFeed(feed)}
                      aria-label={t('sources.rss.removeFeed', 'Remove feed')}
                      className="text-text-muted hover:text-red-400 ms-2 opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity text-xs"
                    >
                      x
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted">{t('sources.rss.empty')}</p>
            )}
            <DefaultSourceList
              defaults={s.defaultRssFeeds}
              disabled={s.disabledDefaultRss}
              onToggle={s.toggleDefaultRss}
              label={t('sources.defaults.rss', 'Default RSS feeds')}
              healthMap={s.feedHealthMap}
              sourceType="rss"
              onResetHealth={s.resetFeedHealth}
            />
          </div>

          {/* YouTube Channels */}
          <div>
            <label className="text-xs text-text-secondary block mb-1.5">
              {t('sources.youtube.label')}
              <span className="text-text-muted ms-1">{t('sources.youtube.noKeyNeeded')}</span>
            </label>
            <div className="flex gap-2 mb-1.5">
              <input
                type="text"
                value={s.newYoutubeChannel}
                onChange={(e) => s.setNewYoutubeChannel(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && s.addYoutubeChannel()}
                placeholder="Channel ID e.g. UCsBjURrPoezykLs9EqgamOA"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={s.addYoutubeChannel}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            {s.youtubeChannels.length > 0 ? (
              <div className="flex flex-wrap gap-1.5">
                {s.youtubeChannels.map((ch) => (
                  <span
                    key={ch}
                    className="inline-flex items-center gap-1 px-2 py-1 bg-red-500/10 text-red-400 text-xs rounded border border-red-500/20 group"
                  >
                    {ch.length > 20 ? ch.slice(0, 8) + '...' + ch.slice(-8) : ch}
                    <FeedHealthDot health={s.feedHealthMap[ch]} onReset={() => s.resetFeedHealth(ch, 'youtube')} />
                    <button
                      onClick={() => s.removeYoutubeChannel(ch)}
                      aria-label={t('sources.youtube.removeChannel', 'Remove channel')}
                      className="text-red-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted">{t('sources.youtube.defaultChannels')}</p>
            )}
            <DefaultSourceList
              defaults={s.defaultYoutubeChannels}
              disabled={s.disabledDefaultYoutube}
              onToggle={s.toggleDefaultYoutube}
              label={t('sources.defaults.youtube', 'Default YouTube channels')}
              healthMap={s.feedHealthMap}
              sourceType="youtube"
              onResetHealth={s.resetFeedHealth}
            />
          </div>

          {/* GitHub Languages */}
          <div>
            <label className="text-xs text-text-secondary block mb-1.5">
              {t('sources.github.label')}
              <span className="text-text-muted ms-1">{t('sources.github.trendingFilter')}</span>
            </label>
            <div className="flex gap-2 mb-1.5">
              <input
                type="text"
                value={s.newGithubLanguage}
                onChange={(e) => s.setNewGithubLanguage(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && s.addGithubLanguage()}
                placeholder="e.g. go, java, swift"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={s.addGithubLanguage}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {s.githubLanguages.map((lang) => (
                <span
                  key={lang}
                  className="inline-flex items-center gap-1 px-2 py-1 bg-purple-500/10 text-purple-400 text-xs rounded border border-purple-500/20 group"
                >
                  {lang}
                  <button
                    onClick={() => s.removeGithubLanguage(lang)}
                    aria-label={t('sources.github.removeLanguage', 'Remove language')}
                    className="text-purple-400/40 hover:text-red-400 transition-colors"
                  >
                    x
                  </button>
                </span>
              ))}
            </div>
          </div>

          {/* Twitter/X */}
          <div>
            <label className="text-xs text-text-secondary block mb-1.5">
              {t('sources.twitter.label')}
              {s.hasXApiKey ? (
                <span className="text-green-400 ms-1">{t('sources.twitter.keySet')}</span>
              ) : (
                <span className="text-yellow-400 ms-1">{t('sources.twitter.needsKey')}</span>
              )}
            </label>
            <div className="flex gap-2 mb-1.5">
              <input
                type="text"
                value={s.newTwitterHandle}
                onChange={(e) => s.setNewTwitterHandle(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && s.addTwitterHandle()}
                placeholder="@handle"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={s.addTwitterHandle}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            {s.twitterHandles.length > 0 ? (
              <div className="flex flex-wrap gap-1.5">
                {s.twitterHandles.map((h) => (
                  <span
                    key={h}
                    className="inline-flex items-center gap-1 px-2 py-1 bg-blue-500/10 text-blue-400 text-xs rounded border border-blue-500/20 group"
                  >
                    @{h}
                    <FeedHealthDot health={s.feedHealthMap[h]} onReset={() => s.resetFeedHealth(h, 'twitter')} />
                    <button
                      onClick={() => s.removeTwitterHandle(h)}
                      aria-label={t('sources.twitter.removeHandle', 'Remove handle')}
                      className="text-blue-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted">{t('sources.twitter.defaultHandles')}</p>
            )}
            <DefaultSourceList
              defaults={s.defaultTwitterHandles}
              disabled={s.disabledDefaultTwitter}
              onToggle={s.toggleDefaultTwitter}
              label={t('sources.defaults.twitter', 'Default Twitter/X handles')}
              healthMap={s.feedHealthMap}
              sourceType="twitter"
              onResetHealth={s.resetFeedHealth}
            />

            {/* X API Key */}
            <div className="flex gap-2 mt-3">
              <input
                type="password"
                value={s.xApiKey}
                onChange={(e) => s.setXApiKey(e.target.value)}
                placeholder={s.hasXApiKey ? '(key saved)' : 'X API Bearer Token'}
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={s.saveXApiKey}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {s.hasXApiKey ? t('sources.twitter.update') : t('action.save')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
