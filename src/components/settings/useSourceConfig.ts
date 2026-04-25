// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { RssFeedValidation, YouTubeChannelValidation } from '../../lib/commands';
import { reportError } from '../../lib/error-reporter';
import { translateError } from '../../utils/error-messages';
import { parseSourceInput } from '../../utils/source-input-parser';

export type ValidationResult = (RssFeedValidation & YouTubeChannelValidation) | null;

export function useSourceConfig(onStatusChange: (status: string) => void) {
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

  // Default source lists + disabled tracking
  const [defaultRssFeeds, setDefaultRssFeeds] = useState<string[]>([]);
  const [disabledDefaultRss, setDisabledDefaultRss] = useState<string[]>([]);
  const [defaultYoutubeChannels, setDefaultYoutubeChannels] = useState<string[]>([]);
  const [disabledDefaultYoutube, setDisabledDefaultYoutube] = useState<string[]>([]);
  const [defaultTwitterHandles, setDefaultTwitterHandles] = useState<string[]>([]);
  const [disabledDefaultTwitter, setDisabledDefaultTwitter] = useState<string[]>([]);

  const [rssPreviewOpen, setRssPreviewOpen] = useState(false);
  const [validating, setValidating] = useState(false);
  const [validationResult, setValidationResult] = useState<ValidationResult>(null);

  const loadSources = useCallback(async () => {
    try {
      const [rss, youtube, twitter, xKeyExists, github, defRss, defYt, defTw, disRss, disYt, disTw] = await Promise.all([
        cmd('get_rss_feeds'),
        cmd('get_youtube_channels'),
        cmd('get_twitter_handles'),
        cmd('has_x_api_key'),
        cmd('get_github_languages'),
        cmd('get_default_rss_feeds'),
        cmd('get_default_youtube_channels'),
        cmd('get_default_twitter_handles'),
        cmd('get_disabled_default_rss_feeds'),
        cmd('get_disabled_default_youtube_channels'),
        cmd('get_disabled_default_twitter_handles'),
      ]);
      setRssFeeds(rss.feeds);
      setYoutubeChannels(youtube.channels);
      setTwitterHandles(twitter.handles);
      setHasXApiKey(xKeyExists);
      setGithubLanguages(github.languages);
      setDefaultRssFeeds(defRss.feeds);
      setDefaultYoutubeChannels(defYt.channels);
      setDefaultTwitterHandles(defTw.handles);
      setDisabledDefaultRss(disRss.disabled);
      setDisabledDefaultYoutube(disYt.disabled);
      setDisabledDefaultTwitter(disTw.disabled);
    } catch (error) {
      reportError('SourceConfigPanel.load', error);
    }
  }, []);

  useEffect(() => {
    loadSources();
  }, [loadSources]);

  const rssParsed = useMemo(
    () => (newRssFeed.trim().length > 0 ? parseSourceInput(newRssFeed) : null),
    [newRssFeed],
  );

  const flash = useCallback((msg: string) => {
    onStatusChange(msg);
    setTimeout(() => onStatusChange(''), 2000);
  }, [onStatusChange]);

  const clearInput = () => { setNewRssFeed(''); setRssPreviewOpen(false); };
  const autoClearValidation = () => { setTimeout(() => setValidationResult(null), 5000); };

  const confirmRssAdd = async () => {
    if (!rssParsed) return;
    try {
      if (rssParsed.kind === 'rss') {
        setValidating(true); setValidationResult(null);
        const result = await cmd('validate_rss_feed', { url: rssParsed.value });
        if (!result.valid) { setValidationResult(result); setValidating(false); return; }
        await cmd('set_rss_feeds', { feeds: [...rssFeeds, rssParsed.value] });
        setRssFeeds((f) => [...f, rssParsed.value]);
        cmd('fetch_single_feed', { url: rssParsed.value }).catch(() => {});
        setValidationResult(result);
        flash(`Added ${result.feed_title || 'feed'} — ${result.item_count} items`);
        clearInput(); autoClearValidation();
      } else if (rssParsed.kind === 'youtube-channel-id' || rssParsed.kind === 'youtube-handle') {
        setValidating(true); setValidationResult(null);
        const result = await cmd('validate_youtube_channel', { channelId: rssParsed.value });
        if (!result.valid) { setValidationResult(result); setValidating(false); return; }
        await cmd('set_youtube_channels', { channels: [...youtubeChannels, rssParsed.value] });
        setYoutubeChannels((c) => [...c, rssParsed.value]);
        cmd('fetch_single_youtube_channel', { channelId: rssParsed.value }).catch(() => {});
        flash(`Added ${result.channel_name} — ${result.video_count} videos`);
        clearInput(); autoClearValidation();
      } else if (rssParsed.kind === 'twitter-handle') {
        await cmd('set_twitter_handles', { handles: [...twitterHandles, rssParsed.value] });
        setTwitterHandles((h) => [...h, rssParsed.value]);
        flash(t('sources.twitter.added')); clearInput();
      } else if (rssParsed.kind === 'github-languages') {
        const langs = rssParsed.value.split(',').map((l) => l.trim().toLowerCase()).filter(Boolean);
        const updated = Array.from(new Set([...githubLanguages, ...langs]));
        await cmd('set_github_languages', { languages: updated });
        setGithubLanguages(updated);
        flash(t('sources.github.added')); clearInput();
      }
      setValidating(false);
    } catch (error) {
      setValidating(false);
      onStatusChange(`Error: ${translateError(error)}`);
    }
  };

  const tryDiscoveredFeed = async (feedUrl: string) => {
    setNewRssFeed(feedUrl); setValidationResult(null); setValidating(true);
    try {
      const result = await cmd('validate_rss_feed', { url: feedUrl });
      if (result.valid) {
        await cmd('set_rss_feeds', { feeds: [...rssFeeds, feedUrl] });
        setRssFeeds((f) => [...f, feedUrl]);
        cmd('fetch_single_feed', { url: feedUrl }).catch(() => {});
        flash(`Added ${result.feed_title || 'feed'} — ${result.item_count} items`);
        clearInput(); autoClearValidation();
      } else { setValidationResult(result); }
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
    setValidating(false);
  };

  const addRssFeed = () => {
    if (!rssParsed || rssParsed.kind === 'unknown') {
      onStatusChange(t('sources.rss.unrecognized', 'Paste a URL, @handle, or comma-separated language list'));
      setTimeout(() => onStatusChange(''), 3000);
      return;
    }
    setRssPreviewOpen(true);
  };

  const removeRssFeed = async (url: string) => {
    const updated = rssFeeds.filter((f) => f !== url);
    try {
      await cmd('set_rss_feeds', { feeds: updated });
      setRssFeeds(updated);
      flash(t('sources.rss.removed'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const addYoutubeChannel = async () => {
    const id = newYoutubeChannel.trim();
    if (!id) return;
    try {
      await cmd('set_youtube_channels', { channels: [...youtubeChannels, id] });
      setYoutubeChannels((c) => [...c, id]);
      setNewYoutubeChannel('');
      flash(t('sources.youtube.added'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const removeYoutubeChannel = async (id: string) => {
    const updated = youtubeChannels.filter((c) => c !== id);
    try {
      await cmd('set_youtube_channels', { channels: updated });
      setYoutubeChannels(updated);
      flash(t('sources.youtube.removed'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const addTwitterHandle = async () => {
    const handle = newTwitterHandle.trim().replace(/^@/, '');
    if (!handle) return;
    try {
      await cmd('set_twitter_handles', { handles: [...twitterHandles, handle] });
      setTwitterHandles((h) => [...h, handle]);
      setNewTwitterHandle('');
      flash(t('sources.twitter.added'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const removeTwitterHandle = async (handle: string) => {
    const updated = twitterHandles.filter((h) => h !== handle);
    try {
      await cmd('set_twitter_handles', { handles: updated });
      setTwitterHandles(updated);
      flash(t('sources.twitter.removed'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const saveXApiKey = async () => {
    const key = xApiKey.trim();
    try {
      await cmd('set_x_api_key', { key });
      setHasXApiKey(key.length > 0);
      setXApiKey('');
      flash(key ? t('sources.twitter.keySaved') : t('sources.twitter.keyCleared'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const addGithubLanguage = async () => {
    const lang = newGithubLanguage.trim().toLowerCase();
    if (!lang || githubLanguages.includes(lang)) return;
    const updated = [...githubLanguages, lang];
    try {
      await cmd('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      setNewGithubLanguage('');
      flash(t('sources.github.added'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const removeGithubLanguage = async (lang: string) => {
    const updated = githubLanguages.filter((l) => l !== lang);
    try {
      await cmd('set_github_languages', { languages: updated });
      setGithubLanguages(updated);
      flash(t('sources.github.removed'));
    } catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  };

  const toggleDefaultRss = useCallback(async (url: string, enabled: boolean) => {
    const updated = enabled
      ? disabledDefaultRss.filter((f) => f !== url)
      : [...disabledDefaultRss, url];
    setDisabledDefaultRss(updated);
    try { await cmd('set_disabled_default_rss_feeds', { feeds: updated }); }
    catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  }, [disabledDefaultRss, onStatusChange]);

  const toggleDefaultYoutube = useCallback(async (ch: string, enabled: boolean) => {
    const updated = enabled
      ? disabledDefaultYoutube.filter((c) => c !== ch)
      : [...disabledDefaultYoutube, ch];
    setDisabledDefaultYoutube(updated);
    try { await cmd('set_disabled_default_youtube_channels', { channels: updated }); }
    catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  }, [disabledDefaultYoutube, onStatusChange]);

  const toggleDefaultTwitter = useCallback(async (handle: string, enabled: boolean) => {
    const updated = enabled
      ? disabledDefaultTwitter.filter((h) => h !== handle)
      : [...disabledDefaultTwitter, handle];
    setDisabledDefaultTwitter(updated);
    try { await cmd('set_disabled_default_twitter_handles', { handles: updated }); }
    catch (error) { onStatusChange(`Error: ${translateError(error)}`); }
  }, [disabledDefaultTwitter, onStatusChange]);

  const activeDefaultRss = defaultRssFeeds.length - disabledDefaultRss.length;
  const activeDefaultYoutube = defaultYoutubeChannels.length - disabledDefaultYoutube.length;
  const activeDefaultTwitter = defaultTwitterHandles.length - disabledDefaultTwitter.length;
  const totalSources = rssFeeds.length + youtubeChannels.length + twitterHandles.length
    + activeDefaultRss + activeDefaultYoutube + activeDefaultTwitter;

  return {
    // Custom sources
    rssFeeds, youtubeChannels, twitterHandles, githubLanguages, hasXApiKey,
    // Input state
    newRssFeed, setNewRssFeed, newYoutubeChannel, setNewYoutubeChannel,
    newTwitterHandle, setNewTwitterHandle, newGithubLanguage, setNewGithubLanguage,
    xApiKey, setXApiKey,
    // RSS smart parse + validation
    rssParsed, rssPreviewOpen, setRssPreviewOpen, confirmRssAdd, addRssFeed,
    validating, validationResult, setValidationResult, tryDiscoveredFeed,
    // CRUD handlers
    removeRssFeed, addYoutubeChannel, removeYoutubeChannel,
    addTwitterHandle, removeTwitterHandle, saveXApiKey,
    addGithubLanguage, removeGithubLanguage,
    // Defaults
    defaultRssFeeds, disabledDefaultRss, toggleDefaultRss,
    defaultYoutubeChannels, disabledDefaultYoutube, toggleDefaultYoutube,
    defaultTwitterHandles, disabledDefaultTwitter, toggleDefaultTwitter,
    // Totals
    totalSources,
  };
}
