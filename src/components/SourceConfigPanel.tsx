import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { translateError } from '../utils/error-messages';

interface SourceConfigPanelProps {
  onStatusChange: (status: string) => void;
}

export function SourceConfigPanel({ onStatusChange }: SourceConfigPanelProps) {
  const { t } = useTranslation();
  const [rssFeeds, setRssFeeds] = useState<string[]>([]);
  const [youtubeChannels, setYoutubeChannels] = useState<string[]>([]);
  const [twitterHandles, setTwitterHandles] = useState<string[]>([]);
  const [githubLanguages, setGithubLanguages] = useState<string[]>([]);
  const [hasXApiKey, setHasXApiKey] = useState(false);

  const [newRssFeed, setNewRssFeed] = useState('');
  const [newYoutubeChannel, setNewYoutubeChannel] = useState('');
  const [newTwitterHandle, setNewTwitterHandle] = useState('');
  const [newGithubLanguage, setNewGithubLanguage] = useState('');
  const [xApiKey, setXApiKey] = useState('');

  const [expanded, setExpanded] = useState(false);

  const loadSources = useCallback(async () => {
    try {
      const [rss, youtube, twitter, xKeyExists, github] = await Promise.all([
        cmd('get_rss_feeds'),
        cmd('get_youtube_channels'),
        cmd('get_twitter_handles'),
        cmd('has_x_api_key'),
        cmd('get_github_languages'),
      ]);
      setRssFeeds(rss.feeds);
      setYoutubeChannels(youtube.channels);
      setTwitterHandles(twitter.handles);
      setHasXApiKey(xKeyExists);
      setGithubLanguages(github.languages);
    } catch (error) {
      console.error('Failed to load source config:', error);
    }
  }, []);

  useEffect(() => {
    loadSources();
  }, [loadSources]);

  const addRssFeed = async () => {
    const url = newRssFeed.trim();
    if (!url) return;
    try {
      await cmd('set_rss_feeds', { feeds: [...rssFeeds, url] });
      setRssFeeds((f) => [...f, url]);
      setNewRssFeed('');
      onStatusChange(t('sources.rss.added'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const removeRssFeed = async (url: string) => {
    const updated = rssFeeds.filter((f) => f !== url);
    try {
      await cmd('set_rss_feeds', { feeds: updated });
      setRssFeeds(updated);
      onStatusChange(t('sources.rss.removed'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const addYoutubeChannel = async () => {
    const id = newYoutubeChannel.trim();
    if (!id) return;
    try {
      await cmd('set_youtube_channels', { channels: [...youtubeChannels, id] });
      setYoutubeChannels((c) => [...c, id]);
      setNewYoutubeChannel('');
      onStatusChange(t('sources.youtube.added'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const removeYoutubeChannel = async (id: string) => {
    const updated = youtubeChannels.filter((c) => c !== id);
    try {
      await cmd('set_youtube_channels', { channels: updated });
      setYoutubeChannels(updated);
      onStatusChange(t('sources.youtube.removed'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const addTwitterHandle = async () => {
    const handle = newTwitterHandle.trim().replace(/^@/, '');
    if (!handle) return;
    try {
      await cmd('set_twitter_handles', { handles: [...twitterHandles, handle] });
      setTwitterHandles((h) => [...h, handle]);
      setNewTwitterHandle('');
      onStatusChange(t('sources.twitter.added'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const removeTwitterHandle = async (handle: string) => {
    const updated = twitterHandles.filter((h) => h !== handle);
    try {
      await cmd('set_twitter_handles', { handles: updated });
      setTwitterHandles(updated);
      onStatusChange(t('sources.twitter.removed'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const saveXApiKey = async () => {
    const key = xApiKey.trim();
    try {
      await cmd('set_x_api_key', { key });
      setHasXApiKey(key.length > 0);
      setXApiKey('');
      onStatusChange(key ? t('sources.twitter.keySaved') : t('sources.twitter.keyCleared'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const addGithubLanguage = async () => {
    const lang = newGithubLanguage.trim().toLowerCase();
    if (!lang || githubLanguages.includes(lang)) return;
    const updated = [...githubLanguages, lang];
    try {
      await cmd('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      setNewGithubLanguage('');
      onStatusChange(t('sources.github.added'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const removeGithubLanguage = async (lang: string) => {
    const updated = githubLanguages.filter((l) => l !== lang);
    try {
      await cmd('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      onStatusChange(t('sources.github.removed'));
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const totalSources = rssFeeds.length + youtubeChannels.length + twitterHandles.length;

  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <button
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        className="w-full flex items-center gap-3"
      >
        <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-cyan-400">📡</span>
        </div>
        <div className="flex-1 text-left">
          <div className="flex items-center gap-2">
            <h3 className="text-white font-medium">{t('sources.title')}</h3>
            <span className="px-1.5 py-0.5 text-[10px] bg-cyan-500/20 text-cyan-400 rounded">
              {t('sources.customCount', { count: totalSources })}
            </span>
          </div>
          <p className="text-text-muted text-sm mt-0.5">
            {t('sources.subtitle')}
          </p>
        </div>
        <span className="text-text-muted text-xs">{expanded ? '−' : '+'}</span>
      </button>

      {expanded && (
        <div className="mt-4 space-y-5">
          {/* RSS Feeds */}
          <div>
            <label className="text-xs text-text-secondary block mb-2">{t('sources.rss.label')}</label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newRssFeed}
                onChange={(e) => setNewRssFeed(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addRssFeed()}
                placeholder="https://example.com/feed.xml"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={addRssFeed}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            {rssFeeds.length > 0 ? (
              <div className="space-y-1 max-h-24 overflow-y-auto">
                {rssFeeds.map((feed) => (
                  <div
                    key={feed}
                    className="flex items-center justify-between px-3 py-1.5 bg-bg-secondary rounded border border-border group"
                  >
                    <span className="font-mono text-xs text-text-secondary truncate">
                      {feed}
                    </span>
                    <button
                      onClick={() => removeRssFeed(feed)}
                      aria-label={t('sources.rss.removeFeed', 'Remove feed')}
                      className="text-text-muted hover:text-red-400 ml-2 opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity text-xs"
                    >
                      x
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted">{t('sources.rss.empty')}</p>
            )}
          </div>

          {/* YouTube Channels */}
          <div>
            <label className="text-xs text-text-secondary block mb-2">
              {t('sources.youtube.label')}
              <span className="text-text-muted ml-1">{t('sources.youtube.noKeyNeeded')}</span>
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newYoutubeChannel}
                onChange={(e) => setNewYoutubeChannel(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addYoutubeChannel()}
                placeholder="Channel ID e.g. UCsBjURrPoezykLs9EqgamOA"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={addYoutubeChannel}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            {youtubeChannels.length > 0 ? (
              <div className="flex flex-wrap gap-1.5">
                {youtubeChannels.map((ch) => (
                  <span
                    key={ch}
                    className="inline-flex items-center gap-1 px-2 py-1 bg-red-500/10 text-red-400 text-xs rounded border border-red-500/20 group"
                  >
                    {ch.length > 20 ? ch.slice(0, 8) + '...' + ch.slice(-8) : ch}
                    <button
                      onClick={() => removeYoutubeChannel(ch)}
                      aria-label={t('sources.youtube.removeChannel', 'Remove channel')}
                      className="text-red-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted">
                {t('sources.youtube.defaultChannels')}
              </p>
            )}
          </div>

          {/* GitHub Languages */}
          <div>
            <label className="text-xs text-text-secondary block mb-2">
              {t('sources.github.label')}
              <span className="text-text-muted ml-1">{t('sources.github.trendingFilter')}</span>
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newGithubLanguage}
                onChange={(e) => setNewGithubLanguage(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addGithubLanguage()}
                placeholder="e.g. go, java, swift"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={addGithubLanguage}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {githubLanguages.map((lang) => (
                <span
                  key={lang}
                  className="inline-flex items-center gap-1 px-2 py-1 bg-purple-500/10 text-purple-400 text-xs rounded border border-purple-500/20 group"
                >
                  {lang}
                  <button
                    onClick={() => removeGithubLanguage(lang)}
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
            <label className="text-xs text-text-secondary block mb-2">
              {t('sources.twitter.label')}
              {hasXApiKey ? (
                <span className="text-green-400 ml-1">{t('sources.twitter.keySet')}</span>
              ) : (
                <span className="text-yellow-400 ml-1">{t('sources.twitter.needsKey')}</span>
              )}
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newTwitterHandle}
                onChange={(e) => setNewTwitterHandle(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addTwitterHandle()}
                placeholder="@handle"
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={addTwitterHandle}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {t('action.add')}
              </button>
            </div>
            {twitterHandles.length > 0 ? (
              <div className="flex flex-wrap gap-1.5 mb-3">
                {twitterHandles.map((h) => (
                  <span
                    key={h}
                    className="inline-flex items-center gap-1 px-2 py-1 bg-blue-500/10 text-blue-400 text-xs rounded border border-blue-500/20 group"
                  >
                    @{h}
                    <button
                      onClick={() => removeTwitterHandle(h)}
                      aria-label={t('sources.twitter.removeHandle', 'Remove handle')}
                      className="text-blue-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-text-muted mb-3">
                {t('sources.twitter.defaultHandles')}
              </p>
            )}

            {/* X API Key */}
            <div className="flex gap-2">
              <input
                type="password"
                value={xApiKey}
                onChange={(e) => setXApiKey(e.target.value)}
                placeholder={hasXApiKey ? '(key saved)' : 'X API Bearer Token'}
                className="flex-1 px-3 py-2 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={saveXApiKey}
                className="px-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {hasXApiKey ? t('sources.twitter.update') : t('action.save')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
