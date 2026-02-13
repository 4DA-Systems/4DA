import { useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { AudioBriefingStatus } from '../types';

export function AudioBriefing() {
  const [status, setStatus] = useState<AudioBriefingStatus | null>(null);
  const [generating, setGenerating] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // eslint-disable-next-line no-undef
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      const s = await invoke<AudioBriefingStatus>('get_audio_briefing_status');
      setStatus(s);
    } catch (e) {
      console.error('Failed to load audio status:', e);
    }
  }, []);

  const generate = async () => {
    setGenerating(true);
    setError(null);
    try {
      const path = await invoke<string>('generate_audio_briefing');
      await loadStatus();
      // Auto-play after generation
      playAudio(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setGenerating(false);
    }
  };

  const playAudio = (filePath: string) => {
    if (audioRef.current) {
      audioRef.current.pause();
    }
    // eslint-disable-next-line no-undef
    const audio = new Audio(convertFileSrc(filePath));
    audio.onplay = () => setPlaying(true);
    audio.onended = () => setPlaying(false);
    audio.onpause = () => setPlaying(false);
    audio.onerror = () => {
      setPlaying(false);
      setError('Failed to play audio');
    };
    audioRef.current = audio;
    audio.play().catch(() => setError('Audio playback blocked'));
  };

  const togglePlay = () => {
    if (!audioRef.current && status?.file_path) {
      playAudio(status.file_path);
      return;
    }
    if (audioRef.current) {
      if (playing) {
        audioRef.current.pause();
      } else {
        audioRef.current.play().catch(() => {});
      }
    }
  };

  // Load status on first render
  if (status === null) {
    loadStatus();
  }

  return (
    <div className="flex items-center gap-2">
      {status?.file_path && (
        <button
          onClick={togglePlay}
          className={`w-10 h-10 rounded-lg flex items-center justify-center transition-all ${
            playing
              ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
              : 'bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300'
          }`}
          title={playing ? 'Pause audio briefing' : 'Play audio briefing'}
        >
          {playing ? '⏸' : '🔊'}
        </button>
      )}
      <button
        onClick={generate}
        disabled={generating}
        className="w-10 h-10 rounded-lg flex items-center justify-center bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300 transition-all disabled:opacity-30"
        title={generating ? 'Generating audio...' : 'Generate audio briefing'}
      >
        {generating ? (
          <div className="w-4 h-4 border-2 border-gray-500 border-t-transparent rounded-full animate-spin" />
        ) : (
          '🎙'
        )}
      </button>
      {error && (
        <span className="text-[10px] text-red-400 max-w-[120px] truncate" title={error}>
          {error}
        </span>
      )}
    </div>
  );
}
