# Getting Started with 4DA

This guide will walk you through setting up 4DA for the first time.

## Installation

### From a pre-built release (recommended)

Download the latest installer for your platform from the [Releases page](https://github.com/runyourempire/4DA/releases/latest).

- **Windows** — `.exe` installer. On first run, Windows SmartScreen will prompt for confirmation because 4DA is a newly released app; click **More info → Run anyway**. See the full [Windows install guide](launch/WINDOWS-INSTALL.md) for SHA-256 verification, signature validation, and auto-update details.
- **macOS** — `.dmg` disk image. Drag 4DA to Applications. Builds are signed and notarised by Apple.
- **Linux** — `.AppImage` (portable), `.deb` (Debian/Ubuntu), or `.rpm` (Fedora/RHEL).

Every release publishes SHA-256 checksums and a minisign signature. Verify before running if you want stronger assurance than code signing alone provides.

### From Source

1. **Install Prerequisites**
   ```bash
   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install Node.js 18+ (via nvm recommended)
   nvm install 18

   # Install pnpm
   npm install -g pnpm
   ```

2. **Clone and Build**
   ```bash
   git clone <repository-url>
   cd 4DA
   pnpm install
   pnpm tauri build
   ```

3. **Run the Installer**
   - Windows: `src-tauri/target/release/bundle/msi/4DA_*.msi`
   - macOS: `src-tauri/target/release/bundle/dmg/4DA_*.dmg`
   - Linux: `src-tauri/target/release/bundle/appimage/4DA_*.AppImage`

## First Run Setup

### 1. Configure API Keys

4DA requires at least one LLM provider for analysis. Open Settings and configure:

**Option A: Anthropic (Recommended)**
- Get an API key from [console.anthropic.com](https://console.anthropic.com)
- Enter it in Settings > API Keys > Anthropic

**Option B: OpenAI**
- Get an API key from [platform.openai.com](https://platform.openai.com)
- Enter it in Settings > API Keys > OpenAI

**Option C: Ollama (Free, Local)**
- Install Ollama from [ollama.ai](https://ollama.ai)
- Run `ollama pull llama3.2` to download a model
- No API key needed - 4DA will detect Ollama automatically

### 2. Choose Your Language

4DA auto-detects your system language. To change it or set up content translation:

- **Change language:** Settings > General > Locale > Language (13 languages available)
- **Content translation:** For automatic translation of feed content, configure a free translation API — see the **[Multilingual Guide](MULTILINGUAL.md)**
- **Quick setup:** Azure Translator gives 2M free characters/month — [portal.azure.com](https://portal.azure.com) > Create "Translator" resource > copy API key > paste in Settings

### 3. Add Context Directories

Tell 4DA where your projects and work files are:

1. Go to Settings > Context Directories
2. Click "Add Directory"
3. Select folders containing your projects

**Good candidates:**
- `~/projects/` or `~/code/`
- `~/Documents/research/`
- Any folder with project files (Cargo.toml, package.json, etc.)

4DA will scan these for:
- Programming languages and frameworks
- Active topics from file contents
- Git commit history
- Recent file modifications

### 3. Set Your Interests

Help 4DA understand what you care about:

1. Go to Settings > Interests
2. Add topics you want to see more of (e.g., "Rust", "machine learning", "distributed systems")
3. Add exclusions for topics you never want (e.g., "crypto", "web3")

### 4. Configure Sources

Choose which external sources to monitor:

| Source | Content | Update Frequency |
|--------|---------|------------------|
| Hacker News | Tech news, discussions | Every 5 minutes |
| arXiv | Academic papers | Every hour |
| Reddit | Community discussions | Every 10 minutes |

Enable/disable sources in Settings > Sources.

## Running Your First Analysis

1. Click "Run Analysis" in the main window
2. 4DA will:
   - Fetch items from enabled sources
   - Score each item against your context
   - Filter out low-relevance items
3. Review the results sorted by relevance

## Understanding Relevance Scores

Items are scored 0.0 to 1.0 based on:

- **Semantic Similarity**: How closely the content matches your interests
- **Topic Affinity**: Learned preferences from your interactions
- **Anti-Topic Penalty**: Reduces score for topics you've rejected

The formula:
```
score = base_score * affinity_multiplier * (1.0 - anti_penalty)
```

## Next Steps

- [Features Guide](./FEATURES.md) - Explore all capabilities
- [Configuration Reference](./CONFIGURATION.md) - Detailed settings
- [API Reference](./API_REFERENCE.md) - For developers

## Troubleshooting

### "No API key configured"

You need at least one LLM provider. See step 1 above.

### "No context directories"

Add at least one directory for 4DA to scan. See step 2 above.

### Analysis returns no results

- Check that your interests are set
- Ensure at least one source is enabled
- Try broadening your interests or reducing exclusions

### Ollama not detected

1. Ensure Ollama is running: `ollama serve`
2. Check the Ollama port (default: 11434)
3. Verify a model is available: `ollama list`
