# 4DA Multilingual Guide

4DA supports 13 languages natively. Every part of the app — UI, notifications, briefings, feed content, coaching, and error messages — can be displayed in your language.

## Supported Languages

| Language | Code | Coverage | Script |
|----------|------|----------|--------|
| English | en | 100% (source) | Latin |
| Arabic | ar | 96% | Arabic (RTL) |
| German | de | 95% | Latin |
| Spanish | es | 95% | Latin |
| French | fr | 94% | Latin |
| Hindi | hi | 97% | Devanagari |
| Italian | it | 94% | Latin |
| Japanese | ja | 96% | CJK |
| Korean | ko | 96% | Hangul |
| Portuguese (Brazil) | pt-BR | 95% | Latin |
| Russian | ru | 96% | Cyrillic |
| Turkish | tr | 96% | Latin |
| Chinese (Simplified) | zh | 97% | CJK |

---

## Setting Your Language

### During Setup

When you first open 4DA, the setup wizard detects your system language automatically. If your system is set to Japanese, 4DA launches in Japanese. You can change the language at any time.

### In Settings

1. Open **Settings** (gear icon in the header)
2. Go to the **General** tab
3. Find **Locale** section
4. Select your **Language** from the dropdown
5. The entire app switches immediately — no restart needed

---

## Content Translation

4DA doesn't just translate the interface — it translates the actual content from your feeds (Hacker News titles, Reddit posts, RSS articles, GitHub repos).

### How It Works

When you set a non-English language:

1. **New content** is automatically translated when fetched (background, non-blocking)
2. **Translations are cached** in a local SQLite database — same content is never translated twice
3. **Technical terms stay in English** — React, Kubernetes, API, TypeScript, etc.
4. **Your briefing** is generated natively in your language (not translated from English)
5. **Search works cross-lingually** — search in Japanese, find relevant English content

### Translation Providers

4DA supports multiple translation backends. You choose what works best for you:

| Provider | Cost | Quality | Privacy | Speed |
|----------|------|---------|---------|-------|
| **Ollama** (default) | Free | Good | Full privacy | 1-5s per batch |
| **DeepL** | Free tier: 500k chars/mo | Excellent | API call | < 1s |
| **Azure Translator** | Free tier: 2M chars/mo | Very good | API call | < 1s |
| **Google Cloud** | Free tier: 500k chars/mo | Very good | API call | < 1s |

**Ollama** is used by default — your content never leaves your machine. For faster, higher-quality translations, configure a dedicated translation API.

---

## Setting Up a Dedicated Translation API

Dedicated translation APIs are 10-50x faster and often higher quality than LLM-based translation. All three major providers offer generous free tiers.

### Azure Translator (Recommended)

**Why:** 2 million characters per month, free forever. No credit card required. Best free tier.

**Setup:**

1. Go to [portal.azure.com](https://portal.azure.com)
2. Click **Create a resource**
3. Search for **Translator**
4. Click **Create**
   - Subscription: your Azure subscription (create free if needed)
   - Resource group: create new or use existing
   - Region: closest to you
   - Name: anything (e.g., "4da-translator")
   - **Pricing tier: Free (F0)** — 2M characters/month
5. Click **Review + create**, then **Create**
6. Once deployed, go to the resource
7. Click **Keys and Endpoint** in the left menu
8. Copy **Key 1**

**In 4DA:**

1. Open **Settings** → **General** → **Locale**
2. Under **Content Translation** (appears when language is non-English):
   - Set **Translation Provider** to **Azure Translator**
   - Paste your key in the **API Key** field
3. Done — translations now use Azure (instant, high quality)

### DeepL

**Why:** Best quality for European languages. 500k characters/month free.

**Setup:**

1. Go to [deepl.com/pro](https://www.deepl.com/pro#developer)
2. Click **Sign up for free**
3. Create an account (email + password, no credit card for free tier)
4. Go to your [account page](https://www.deepl.com/account/summary)
5. Find **Authentication Key for DeepL API** and copy it

**In 4DA:**

1. Open **Settings** → **General** → **Locale**
2. Under **Content Translation**:
   - Set **Translation Provider** to **DeepL**
   - Paste your key in the **API Key** field
3. Done

### Google Cloud Translation

**Why:** Broadest language support (100+ languages). 500k characters/month free.

**Setup:**

1. Go to [console.cloud.google.com](https://console.cloud.google.com)
2. Create a project (or use existing)
3. Enable the **Cloud Translation API**
4. Go to **APIs & Services** → **Credentials**
5. Click **Create Credentials** → **API key**
6. Copy the key
7. (Recommended) Restrict the key to "Cloud Translation API" only

**In 4DA:**

1. Open **Settings** → **General** → **Locale**
2. Under **Content Translation**:
   - Set **Translation Provider** to **Google Cloud**
   - Paste your key in the **API Key** field
3. Done

---

## Translation Settings

When your language is set to non-English, the **Content Translation** section appears in Settings > General > Locale:

| Setting | Default | Description |
|---------|---------|-------------|
| **Translation Provider** | Auto | Which API to use for translations |
| **API Key** | (empty) | Your translation API key |
| **Auto-translate feed titles** | On | Translate content titles when fetched |
| **Also translate descriptions** | Off | Translate descriptions too (uses more API quota) |

### Provider Options

- **Auto** — tries your dedicated API first, falls back to the main LLM
- **DeepL / Azure / Google Cloud** — uses the specified API exclusively
- **Local (Ollama)** — uses your local Ollama model (free, private, slower)
- **Main LLM** — uses your configured AI provider (Anthropic, OpenAI, etc.)

---

## Multilingual Embeddings

For the best experience in non-English languages, 4DA supports multilingual embedding models that understand content across languages.

### Default Model

`nomic-embed-text` — English-optimized, works well for English content scoring.

### Multilingual Model

`nomic-embed-text-v2-moe` — supports ~100 languages. Recommended if you work with content in multiple languages.

**To switch:**

1. Open **Settings** → **Intelligence** → **AI Provider**
2. Note the **Embedding Model** section at the bottom of **Locale** settings
3. Change the embedding model in your Ollama configuration
4. 4DA automatically detects the model change and re-embeds your content in the background

**What this means:** With the multilingual model, a Japanese developer can search for "React state management" in Japanese and find relevant English articles — the embedding model understands the semantic meaning across languages.

---

## Your Briefing in Your Language

The daily intelligence briefing is generated natively in your language — not translated from English. This means:

- Natural phrasing, not machine-translated stiffness
- Section headers in your language
- Technical terms preserved in English (React, Kubernetes, API)
- Same intelligence, same insights, just in your language

---

## Privacy

All translation follows 4DA's privacy-first architecture:

- **Ollama translations** — everything stays on your machine. Zero network calls.
- **Dedicated API translations** — only the text being translated is sent to the API. No metadata, no user data, no tracking.
- **API keys** — stored locally in your system's secure keychain. Never sent to 4DA servers.
- **Translation cache** — stored locally in your SQLite database. Never synced externally.

---

## Platform Notes

### Windows

- System language is auto-detected from your Windows locale settings
- CJK fonts (Japanese, Korean, Chinese) are built into Windows via YuGothic, SimSun
- Arabic text uses Segoe UI with proper RTL layout

### macOS

- System language detected from macOS language preferences
- CJK fonts via Hiragino, PingFang (built-in)
- Arabic text uses Geeza Pro (built-in)

### Linux

- System language detected from `LANG` environment variable
- CJK fonts may need to be installed: `sudo apt install fonts-noto-cjk`
- Arabic fonts may need: `sudo apt install fonts-noto-arabic`

---

## Troubleshooting

### "Content shows in English even though I set Japanese"

1. Check that your language is set in **Settings** → **General** → **Locale**
2. Run a new analysis — existing cached content may not be translated yet
3. If using Ollama: ensure Ollama is running (`ollama serve`)
4. If using a dedicated API: verify your API key is entered correctly

### "Translation seems slow"

- **Ollama:** Translation speed depends on your hardware. First translations are slower (model loading). Subsequent translations are faster.
- **Dedicated API:** Should be near-instant (< 1 second). If slow, check your network connection.
- **Cached translations:** After the first translation, all subsequent views are instant (served from local cache).

### "Some strings are still in English"

- Technical terms (React, API, TypeScript, etc.) are intentionally kept in English
- Brand terms (4DA, Signal, STREETS) stay in English
- If you see full sentences in English, the translation for that specific key may be missing — it will be translated in the next update

### "Daily token limit exceeded"

This message appears when the daily LLM budget is reached. Translation operations bypass this limit (they use a separate budget), but if you're also running heavy analysis, the analysis may be affected. Options:
- Wait until midnight for the daily reset
- Configure a dedicated translation API (DeepL/Azure) — translations won't use LLM tokens at all
- Adjust the limit in Settings → Intelligence
