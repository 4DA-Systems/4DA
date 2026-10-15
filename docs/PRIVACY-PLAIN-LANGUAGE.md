# How 4DA Handles Your Privacy

*Last updated: October 2026*

## Where does my data live?

On your computer. Only your computer.

4DA stores everything in a local database on your machine. There is no "4DA cloud." We don't run servers that hold your data. Your source code and file contents never leave your machine. If you delete the app, your data is gone -- we can't recover it because we never had it.

## What does the app connect to?

These, all documented:

- **Dependency checks** -- to find advisories and fixes, the app sends the names and versions of the packages in your lockfiles to OSV.dev and the package registries (npm, crates.io, PyPI, the Go module proxy). Go module paths are sent in full, so a private module path names that repository. Never your code.
- **Content sources** (Hacker News, GitHub, Reddit, arXiv, RSS feeds) -- your computer fetches public articles directly from these services. We are not in the middle.
- **AI providers** (only if you set one up) -- OpenAI, Anthropic, or Ollama (runs locally). You provide your own API key. It goes straight from your machine to the provider. We never see it.
- **License check** (paid tier only) -- if you buy a Signal subscription, the app verifies your license key with Keygen and, near expiry, renews it with our own server, `4da.ai` (the key only). If you ask the app to recover a lost key, it sends your email to `4da.ai`. No name, no device ID.
- **Update check** -- the app checks GitHub for new versions. No personal data is sent.
- **One-time download** -- the local search model, from Hugging Face, only when a feature first needs it.

On Windows, the Microsoft WebView2 runtime that draws the app's window also talks to Microsoft on its own; [NETWORK.md §5](../NETWORK.md#5-not-4das-code-the-windows-webview-runtime) explains what it sends and how to limit it.

## What do you collect about me?

Nothing, by default.

4DA Systems does not operate a server that collects your data, an analytics pipeline, or a user account system. Our only server the app talks to is the licence endpoint above. There is no 4DA-operated tracking, cookies, fingerprinting, or sign-up.

The app records what you click and save to build a preference profile you can inspect and delete; relevance comes from your local codebase. All of that stays on your machine in a local SQLite database.

**No telemetry, no exceptions**: 4DA sends no crash reports and no analytics to any server — there is no toggle that turns that on, because there is nothing to turn on. If something breaks, **Settings → Privacy → Export diagnostics** builds a scrubbed, local report (file paths, API keys, and PII redacted) that you review and choose whether to attach to a bug report. Nothing leaves your machine unless you send it yourself.

Your BYOK LLM calls go direct to the provider *you* chose (Anthropic, OpenAI, or localhost Ollama). Dependency checks go to OSV.dev and the package registries. Source adapters fetch public content (HN, GitHub, Reddit, CVE feeds, etc.). License validation and renewal reach Keygen and `4da.ai` if you're on a paid tier. Updates check GitHub Releases. The full list, including the opt-in flows (cloud translation, digest email, GitHub release notes), is in [NETWORK.md](../NETWORK.md) with source references.

## What about my API keys?

You bring your own keys (BYOK). They are stored in your operating system's secure vault -- Windows Credential Manager, macOS Keychain, or Linux Secret Service. Keys are sent only to the AI provider you chose. We never see, store, or touch them.

## Can I verify all of this?

Yes. Three ways:

1. **Read the code.** 4DA is source-available. Every line is public.
2. **Watch the network.** Use Wireshark or Little Snitch. Every connection the `fourda` process makes is one listed in NETWORK.md (on Windows, `msedgewebview2.exe` is Microsoft's runtime, described above).
3. **Build it yourself.** Clone the repo, compile it, run your own copy. See [BUILD-FROM-SOURCE.md](BUILD-FROM-SOURCE.md).

## What if 4DA shuts down?

The app keeps working. There is no server to turn off. Your data is on your machine. The source code is public. The license converts to fully open-source (Apache 2.0) after three years. You lose nothing.

## What about the free version?

The free tier makes zero calls to 4DA Systems. Not one. If you use the free tier with Ollama for local AI, the app can run offline after fetching content, working from the advisory and registry data it last downloaded.

## Where is the full legal version?

Read the complete Privacy Policy at [docs/legal/PRIVACY-POLICY.md](legal/PRIVACY-POLICY.md) or at [4da.ai/privacy](https://4da.ai/privacy).

## Questions?

- General: support@4da.ai
- Privacy: privacy@4da.ai
- Security: security@4da.ai

---

4DA Systems Pty Ltd | [4da.ai](https://4da.ai) | This is a plain-language summary, not a legal document. See [PRIVACY-POLICY.md](legal/PRIVACY-POLICY.md) for the full legal privacy policy.
