# 4DA Configuration Reference

This document covers all configuration options available in 4DA.

## Settings File

Settings are stored in:
- Windows: `%APPDATA%\4da\data\settings.json`
- macOS: `~/Library/Application Support/4da/data/settings.json`
- Linux: `~/.config/4da/data/settings.json`

## API Keys

### Anthropic

```json
{
  "anthropic_api_key": "sk-ant-..."
}
```

- Required for Claude-based analysis
- Get from: https://console.anthropic.com
- Models used: claude-3-haiku, claude-3-sonnet

### OpenAI

```json
{
  "openai_api_key": "sk-..."
}
```

- Optional, for GPT-based analysis
- Get from: https://platform.openai.com
- Models used: gpt-4o-mini, text-embedding-3-small

### Ollama

```json
{
  "ollama_host": "http://localhost:11434",
  "ollama_model": "llama3.2"
}
```

- No API key required
- Fully local operation
- Requires Ollama installed and running

## Translation

Content translation can use dedicated APIs for faster, higher-quality results:

```json
{
  "translation": {
    "provider": "auto",
    "api_key": "",
    "auto_translate": true,
    "translate_descriptions": false
  }
}
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `provider` | string | `"auto"` | `"auto"`, `"deepl"`, `"google"`, `"azure"`, `"ollama"`, `"llm"` |
| `api_key` | string | `""` | API key for DeepL, Google, or Azure |
| `auto_translate` | bool | `true` | Translate feed titles at ingest time |
| `translate_descriptions` | bool | `false` | Also translate descriptions (higher API usage) |

**Free tier quotas:**
- Azure Translator: 2M chars/month (recommended)
- DeepL: 500k chars/month
- Google Cloud Translation: 500k chars/month

See **[Multilingual Guide](MULTILINGUAL.md)** for provider setup instructions.

## Locale

```json
{
  "locale": {
    "country": "US",
    "language": "en",
    "currency": "USD"
  }
}
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `country` | string | `"US"` | ISO 3166-1 country code |
| `language` | string | `"en"` | BCP-47 language tag (e.g., `"ja"`, `"de"`, `"pt-BR"`) |
| `currency` | string | `"USD"` | ISO 4217 currency code |

Supported languages: en, ar, de, es, fr, hi, it, ja, ko, pt-BR, ru, tr, zh.

## Analysis Settings

```json
{
  "analysis": {
    "max_items_per_source": 30,
    "relevance_threshold": 0.3,
    "include_content": true,
    "parallel_scraping": true
  }
}
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_items_per_source` | number | 30 | Maximum items to fetch per source |
| `relevance_threshold` | number | 0.3 | Minimum score for display |
| `include_content` | boolean | true | Scrape full content from URLs |
| `parallel_scraping` | boolean | true | Fetch content in parallel |

## Source Configuration

### Hacker News

```json
{
  "sources": {
    "hackernews": {
      "enabled": true,
      "max_items": 30,
      "fetch_interval_secs": 300
    }
  }
}
```

### arXiv

```json
{
  "sources": {
    "arxiv": {
      "enabled": true,
      "max_items": 50,
      "fetch_interval_secs": 3600,
      "categories": ["cs.AI", "cs.LG", "cs.PL"]
    }
  }
}
```

### Reddit

```json
{
  "sources": {
    "reddit": {
      "enabled": true,
      "max_items": 30,
      "fetch_interval_secs": 600,
      "subreddits": ["programming", "rust", "machinelearning"]
    }
  }
}
```

## Context Directories

```json
{
  "context_dirs": [
    "/home/user/projects",
    "/home/user/research"
  ]
}
```

Directories are scanned for:
- Project manifests (Cargo.toml, package.json, etc.)
- File changes (via file watcher)
- Git repositories

## ACE Configuration

```json
{
  "ace": {
    "scan_depth": 3,
    "file_watch_enabled": true,
    "git_analysis_enabled": true,
    "behavior_learning_enabled": true,
    "max_signals": 500
  }
}
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `scan_depth` | number | 3 | Directory depth to scan |
| `file_watch_enabled` | boolean | true | Enable real-time file watching |
| `git_analysis_enabled` | boolean | true | Analyze Git history |
| `behavior_learning_enabled` | boolean | true | Learn from interactions |
| `max_signals` | number | 500 | Maximum signals to track |

## Behavior Settings

```json
{
  "behavior": {
    "min_exposures": 3,
    "anti_topic_threshold": 5,
    "decay_half_life_days": 30
  }
}
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_exposures` | number | 3 | Minimum interactions before learning |
| `anti_topic_threshold` | number | 5 | Rejections to mark as anti-topic |
| `decay_half_life_days` | number | 30 | Days for preference decay |

## Cost Limits

```json
{
  "cost_limits": {
    "daily_limit_usd": 1.00,
    "warn_at_percent": 80,
    "per_provider": {
      "anthropic": 0.50,
      "openai": 0.50
    }
  }
}
```

## Notification Settings

```json
{
  "notifications": {
    "enabled": true,
    "high_relevance_threshold": 0.8,
    "sound_enabled": false
  }
}
```

## Digest Settings

```json
{
  "digest": {
    "enabled": false,
    "schedule": "daily",
    "time": "09:00",
    "max_items": 10,
    "email": "user@example.com"
  }
}
```

## Embedding Configuration

```json
{
  "embedding": {
    "provider": "fastembed",
    "model": "BAAI/bge-small-en-v1.5",
    "dimensions": 384
  }
}
```

| Provider | Model | Dimensions | Notes |
|----------|-------|------------|-------|
| fastembed | BAAI/bge-small-en-v1.5 | 384 | Local, fast |
| openai | text-embedding-3-small | 1536 | API calls |

## Background Jobs

```json
{
  "jobs": {
    "source_fetch_interval_mins": 5,
    "context_refresh_interval_mins": 15,
    "health_check_interval_mins": 10
  }
}
```

## Debug Settings

```json
{
  "debug": {
    "verbose_logging": false,
    "save_raw_responses": false,
    "timing_enabled": false
  }
}
```

## Environment Variables

Some settings can be overridden via environment:

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `OLLAMA_HOST` | Ollama server URL |
| `FDD_LOG_LEVEL` | Logging level (debug, info, warn, error) |

## Example Full Configuration

```json
{
  "anthropic_api_key": "sk-ant-...",
  "context_dirs": ["/home/user/projects"],
  "analysis": {
    "max_items_per_source": 30,
    "relevance_threshold": 0.3
  },
  "sources": {
    "hackernews": { "enabled": true },
    "arxiv": { "enabled": true, "categories": ["cs.AI"] },
    "reddit": { "enabled": false }
  },
  "ace": {
    "file_watch_enabled": true,
    "git_analysis_enabled": true
  },
  "cost_limits": {
    "daily_limit_usd": 1.00
  },
  "notifications": {
    "enabled": true,
    "high_relevance_threshold": 0.8
  }
}
```
