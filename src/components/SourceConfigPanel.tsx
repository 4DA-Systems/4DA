import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SourceConfigPanelProps {
  onStatusChange: (status: string) => void;
}

export function SourceConfigPanel({ onStatusChange }: SourceConfigPanelProps) {
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
      const [rss, youtube, twitter, xKey, github] = await Promise.all([
        invoke<{ feeds: string[]; count: number }>('get_rss_feeds'),
        invoke<{ channels: string[]; count: number }>('get_youtube_channels'),
        invoke<{ handles: string[]; count: number }>('get_twitter_handles'),
        invoke<string>('get_x_api_key'),
        invoke<{ languages: string[]; count: number }>('get_github_languages'),
      ]);
      setRssFeeds(rss.feeds);
      setYoutubeChannels(youtube.channels);
      setTwitterHandles(twitter.handles);
      setHasXApiKey(xKey.length > 0);
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
      await invoke('set_rss_feeds', { feeds: [...rssFeeds, url] });
      setRssFeeds((f) => [...f, url]);
      setNewRssFeed('');
      onStatusChange('RSS feed added');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const removeRssFeed = async (url: string) => {
    const updated = rssFeeds.filter((f) => f !== url);
    try {
      await invoke('set_rss_feeds', { feeds: updated });
      setRssFeeds(updated);
      onStatusChange('RSS feed removed');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const addYoutubeChannel = async () => {
    const id = newYoutubeChannel.trim();
    if (!id) return;
    try {
      await invoke('set_youtube_channels', { channels: [...youtubeChannels, id] });
      setYoutubeChannels((c) => [...c, id]);
      setNewYoutubeChannel('');
      onStatusChange('YouTube channel added');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const removeYoutubeChannel = async (id: string) => {
    const updated = youtubeChannels.filter((c) => c !== id);
    try {
      await invoke('set_youtube_channels', { channels: updated });
      setYoutubeChannels(updated);
      onStatusChange('YouTube channel removed');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const addTwitterHandle = async () => {
    const handle = newTwitterHandle.trim().replace(/^@/, '');
    if (!handle) return;
    try {
      await invoke('set_twitter_handles', { handles: [...twitterHandles, handle] });
      setTwitterHandles((h) => [...h, handle]);
      setNewTwitterHandle('');
      onStatusChange('Twitter handle added');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const removeTwitterHandle = async (handle: string) => {
    const updated = twitterHandles.filter((h) => h !== handle);
    try {
      await invoke('set_twitter_handles', { handles: updated });
      setTwitterHandles(updated);
      onStatusChange('Twitter handle removed');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const saveXApiKey = async () => {
    const key = xApiKey.trim();
    try {
      await invoke('set_x_api_key', { key });
      setHasXApiKey(key.length > 0);
      setXApiKey('');
      onStatusChange(key ? 'X API key saved' : 'X API key cleared');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const addGithubLanguage = async () => {
    const lang = newGithubLanguage.trim().toLowerCase();
    if (!lang || githubLanguages.includes(lang)) return;
    const updated = [...githubLanguages, lang];
    try {
      await invoke('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      setNewGithubLanguage('');
      onStatusChange('GitHub language added');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const removeGithubLanguage = async (lang: string) => {
    const updated = githubLanguages.filter((l) => l !== lang);
    try {
      await invoke('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      onStatusChange('GitHub language removed');
      setTimeout(() => onStatusChange(''), 2000);
    } catch (error) {
      onStatusChange(`Error: ${error}`);
    }
  };

  const totalSources = rssFeeds.length + youtubeChannels.length + twitterHandles.length;

  return (
    <div className="bg-[#1F1F1F] rounded-lg p-5 border border-[#2A2A2A]">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-3"
      >
        <div className="w-8 h-8 bg-cyan-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-cyan-400">📡</span>
        </div>
        <div className="flex-1 text-left">
          <div className="flex items-center gap-2">
            <h3 className="text-white font-medium">Source Configuration</h3>
            <span className="px-1.5 py-0.5 text-[10px] bg-cyan-500/20 text-cyan-400 rounded">
              {totalSources} custom
            </span>
          </div>
          <p className="text-gray-500 text-sm mt-0.5">
            RSS feeds, YouTube, GitHub languages, Twitter/X
          </p>
        </div>
        <span className="text-gray-500 text-xs">{expanded ? '−' : '+'}</span>
      </button>

      {expanded && (
        <div className="mt-4 space-y-5">
          {/* RSS Feeds */}
          <div>
            <label className="text-xs text-gray-400 block mb-2">RSS Feeds</label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newRssFeed}
                onChange={(e) => setNewRssFeed(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addRssFeed()}
                placeholder="https://example.com/feed.xml"
                className="flex-1 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={addRssFeed}
                className="px-3 py-2 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-cyan-500/30 transition-all"
              >
                Add
              </button>
            </div>
            {rssFeeds.length > 0 ? (
              <div className="space-y-1 max-h-24 overflow-y-auto">
                {rssFeeds.map((feed) => (
                  <div
                    key={feed}
                    className="flex items-center justify-between px-3 py-1.5 bg-[#141414] rounded border border-[#2A2A2A] group"
                  >
                    <span className="font-mono text-xs text-gray-300 truncate">
                      {feed}
                    </span>
                    <button
                      onClick={() => removeRssFeed(feed)}
                      className="text-gray-600 hover:text-red-400 ml-2 opacity-0 group-hover:opacity-100 transition-opacity text-xs"
                    >
                      x
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-xs text-gray-600">No custom RSS feeds</p>
            )}
          </div>

          {/* YouTube Channels */}
          <div>
            <label className="text-xs text-gray-400 block mb-2">
              YouTube Channels
              <span className="text-gray-600 ml-1">(no API key needed)</span>
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newYoutubeChannel}
                onChange={(e) => setNewYoutubeChannel(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addYoutubeChannel()}
                placeholder="Channel ID e.g. UCsBjURrPoezykLs9EqgamOA"
                className="flex-1 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={addYoutubeChannel}
                className="px-3 py-2 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-cyan-500/30 transition-all"
              >
                Add
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
                      className="text-red-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-gray-600">
                Using default tech channels (Fireship, ThePrimeagen, etc.)
              </p>
            )}
          </div>

          {/* GitHub Languages */}
          <div>
            <label className="text-xs text-gray-400 block mb-2">
              GitHub Languages
              <span className="text-gray-600 ml-1">(trending repos filter)</span>
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newGithubLanguage}
                onChange={(e) => setNewGithubLanguage(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addGithubLanguage()}
                placeholder="e.g. go, java, swift"
                className="flex-1 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={addGithubLanguage}
                className="px-3 py-2 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-cyan-500/30 transition-all"
              >
                Add
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
            <label className="text-xs text-gray-400 block mb-2">
              Twitter/X Accounts
              {hasXApiKey ? (
                <span className="text-green-400 ml-1">(API key set)</span>
              ) : (
                <span className="text-yellow-400 ml-1">(needs API key)</span>
              )}
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newTwitterHandle}
                onChange={(e) => setNewTwitterHandle(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && addTwitterHandle()}
                placeholder="@handle"
                className="flex-1 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none"
              />
              <button
                onClick={addTwitterHandle}
                className="px-3 py-2 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-cyan-500/30 transition-all"
              >
                Add
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
                      className="text-blue-400/40 hover:text-red-400 transition-colors"
                    >
                      x
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-gray-600 mb-3">
                Using default tech handles
              </p>
            )}

            {/* X API Key */}
            <div className="flex gap-2">
              <input
                type="password"
                value={xApiKey}
                onChange={(e) => setXApiKey(e.target.value)}
                placeholder={hasXApiKey ? '(key saved)' : 'X API Bearer Token'}
                className="flex-1 px-3 py-2 bg-[#141414] border border-[#2A2A2A] rounded-lg text-sm text-white placeholder:text-gray-600 focus:border-cyan-500/50 focus:outline-none font-mono"
              />
              <button
                onClick={saveXApiKey}
                className="px-3 py-2 text-sm bg-[#141414] border border-[#2A2A2A] rounded-lg text-gray-400 hover:text-white hover:border-cyan-500/30 transition-all"
              >
                {hasXApiKey ? 'Update' : 'Save'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
