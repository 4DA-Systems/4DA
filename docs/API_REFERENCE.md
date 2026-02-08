# 4DA API Reference

This document covers the Tauri commands exposed by 4DA for frontend and automation use.

## Analysis Commands

### `run_analysis`

Run analysis for a single source.

```typescript
invoke('run_analysis', { source: string }): Promise<AnalysisResult[]>
```

### `run_multi_source_analysis`

Run analysis across all enabled sources.

```typescript
invoke('run_multi_source_analysis'): Promise<AnalysisResult[]>
```

### `start_background_analysis`

Start a background analysis job.

```typescript
invoke('start_background_analysis', {
  source: string | null,
  intervalMinutes: number
}): Promise<string>  // Returns job ID
```

### `compute_relevance`

Compute relevance score for content.

```typescript
invoke('compute_relevance', {
  title: string,
  content: string,
  topics: string[]
}): Promise<number>
```

## Context Commands

### `index_context`

Index context directories.

```typescript
invoke('index_context'): Promise<void>
```

### `get_context_stats`

Get context engine statistics.

```typescript
invoke('get_context_stats'): Promise<ContextStats>
```

### `add_interest`

Add a topic interest.

```typescript
invoke('add_interest', {
  topic: string,
  weight?: number
}): Promise<void>
```

### `remove_interest`

Remove a topic interest.

```typescript
invoke('remove_interest', { topic: string }): Promise<void>
```

### `get_interests`

Get all interests.

```typescript
invoke('get_interests'): Promise<Interest[]>
```

### `add_exclusion`

Add an exclusion (anti-topic).

```typescript
invoke('add_exclusion', { topic: string }): Promise<void>
```

### `get_exclusions`

Get all exclusions.

```typescript
invoke('get_exclusions'): Promise<string[]>
```

### `set_context_dirs`

Set context directories to monitor.

```typescript
invoke('set_context_dirs', { dirs: string[] }): Promise<void>
```

### `get_context_dirs`

Get monitored directories.

```typescript
invoke('get_context_dirs'): Promise<string[]>
```

## ACE Commands

### `ace_detect_context`

Auto-detect context from directories.

```typescript
invoke('ace_detect_context', {
  dirs: string[]
}): Promise<ACEContext>
```

### `ace_get_active_topics`

Get currently active topics.

```typescript
invoke('ace_get_active_topics'): Promise<Topic[]>
```

### `ace_get_topic_affinities`

Get learned topic affinities.

```typescript
invoke('ace_get_topic_affinities'): Promise<TopicAffinity[]>
```

### `ace_get_anti_topics`

Get learned anti-topics.

```typescript
invoke('ace_get_anti_topics', {
  minRejections?: number
}): Promise<AntiTopic[]>
```

### `ace_auto_discover`

Run full auto-discovery.

```typescript
invoke('ace_auto_discover'): Promise<AutoDiscoverResult>
```

### `ace_get_health`

Get ACE health status.

```typescript
invoke('ace_get_health'): Promise<HealthSnapshot>
```

### `ace_start_watcher`

Start file watcher.

```typescript
invoke('ace_start_watcher'): Promise<void>
```

### `ace_stop_watcher`

Stop file watcher.

```typescript
invoke('ace_stop_watcher'): Promise<void>
```

## Settings Commands

### `get_settings`

Get current settings.

```typescript
invoke('get_settings'): Promise<Settings>
```

### `save_settings`

Save settings.

```typescript
invoke('save_settings', { settings: Settings }): Promise<void>
```

### `get_api_key`

Get a specific API key (masked).

```typescript
invoke('get_api_key', { provider: string }): Promise<string | null>
```

### `set_api_key`

Set an API key.

```typescript
invoke('set_api_key', {
  provider: string,
  key: string
}): Promise<void>
```

### `validate_api_key`

Validate an API key.

```typescript
invoke('validate_api_key', {
  provider: string,
  key: string
}): Promise<boolean>
```

## Feedback Commands

### `record_feedback`

Record user feedback on an item.

```typescript
invoke('record_feedback', {
  itemId: number,
  action: 'click' | 'save' | 'dismiss' | 'ignore',
  metadata?: object
}): Promise<void>
```

### `get_feedback_stats`

Get feedback statistics.

```typescript
invoke('get_feedback_stats'): Promise<FeedbackStats>
```

## Health Commands

### `get_system_health`

Get overall system health.

```typescript
invoke('get_system_health'): Promise<SystemHealth>
```

### `get_usage_stats`

Get usage statistics.

```typescript
invoke('get_usage_stats'): Promise<UsageStats>
```

### `get_cost_tracking`

Get cost tracking data.

```typescript
invoke('get_cost_tracking'): Promise<CostData>
```

## Embedding Commands

### `generate_embedding`

Generate embedding for text.

```typescript
invoke('generate_embedding', {
  text: string
}): Promise<number[]>
```

### `find_similar`

Find similar items by embedding.

```typescript
invoke('find_similar', {
  embedding: number[],
  limit: number
}): Promise<SimilarItem[]>
```

## Source Commands

### `get_sources`

Get available sources.

```typescript
invoke('get_sources'): Promise<SourceInfo[]>
```

### `enable_source`

Enable a source.

```typescript
invoke('enable_source', { source: string }): Promise<void>
```

### `disable_source`

Disable a source.

```typescript
invoke('disable_source', { source: string }): Promise<void>
```

### `fetch_source`

Fetch items from a source.

```typescript
invoke('fetch_source', {
  source: string
}): Promise<SourceItem[]>
```

## Digest Commands

### `send_digest`

Send a digest email.

```typescript
invoke('send_digest', {
  email: string,
  items: AnalysisResult[]
}): Promise<void>
```

### `schedule_digest`

Schedule recurring digests.

```typescript
invoke('schedule_digest', {
  email: string,
  schedule: 'daily' | 'weekly',
  time: string
}): Promise<void>
```

## System Commands

### `open_in_browser`

Open URL in default browser.

```typescript
invoke('open_in_browser', { url: string }): Promise<void>
```

### `get_app_version`

Get application version.

```typescript
invoke('get_app_version'): Promise<string>
```

### `get_log_path`

Get path to log files.

```typescript
invoke('get_log_path'): Promise<string>
```

## Type Definitions

### AnalysisResult

```typescript
interface AnalysisResult {
  id: number;
  title: string;
  url: string | null;
  source: string;
  relevance_score: number;
  topics: string[];
  summary: string | null;
  metadata: object | null;
  created_at: string;
}
```

### Interest

```typescript
interface Interest {
  id: number;
  topic: string;
  weight: number;
  embedding: number[] | null;
  source: 'explicit' | 'github' | 'inferred';
}
```

### TopicAffinity

```typescript
interface TopicAffinity {
  topic: string;
  positive_signals: number;
  negative_signals: number;
  total_exposures: number;
  affinity_score: number;
  confidence: number;
}
```

### HealthSnapshot

```typescript
interface HealthSnapshot {
  status: 'healthy' | 'degraded' | 'unhealthy';
  components: ComponentStatus[];
  last_check: string;
  alerts: HealthAlert[];
}
```

### Settings

```typescript
interface Settings {
  anthropic_api_key: string | null;
  openai_api_key: string | null;
  ollama_host: string | null;
  context_dirs: string[];
  analysis: AnalysisSettings;
  sources: SourceSettings;
  ace: ACESettings;
  cost_limits: CostLimits;
  notifications: NotificationSettings;
}
```

## Error Handling

All commands return errors as strings. Handle them in your frontend:

```typescript
try {
  const result = await invoke('run_analysis', { source: 'hackernews' });
} catch (error) {
  console.error('Analysis failed:', error);
}
```

Common error patterns:
- `"No API key configured"` - Missing required API key
- `"Source disabled"` - Source is not enabled
- `"Rate limited"` - Too many requests
- `"Network error: ..."` - Connection issues
