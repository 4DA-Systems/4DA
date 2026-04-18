# How 4DA Handles Your Privacy

*Last updated: March 2026*

## Where does my data live?

On your computer. Only your computer.

4DA stores everything in a local database on your machine. There is no "4DA cloud." We don't run servers that hold your data. If you delete the app, your data is gone -- we can't recover it because we never had it.

## What does the app connect to?

Four things, all transparent:

- **Content sources** (Hacker News, GitHub, Reddit, arXiv, RSS feeds) -- your computer fetches public articles directly from these services. We are not in the middle.
- **AI providers** (only if you set one up) -- OpenAI, Anthropic, or Ollama (runs locally). You provide your own API key. It goes straight from your machine to the provider. We never see it.
- **License check** (paid tier only) -- if you buy a Signal subscription, the app verifies your license key with Keygen. That's it. No name, no email, no device ID.
- **Update check** -- the app checks GitHub for new versions. No personal data is sent.

## What do you collect about me?

Nothing, by default.

4DA Systems does not operate a server, an analytics pipeline, or a user account system. There is no 4DA-operated tracking, cookies, fingerprinting, or sign-up.

The app learns from what you click and save to improve your recommendations — all of that stays on your machine in a local SQLite database.

**One opt-in exception**: crash reporting (Sentry). It is **off by default**. If you turn it on in Settings → Privacy, anonymized crash traces get sent to Sentry with file paths, API keys, and PII scrubbed. You can verify the scrubbing in `src/lib/sentry-init.ts`. With the toggle off, no Sentry connection is ever attempted.

Your BYOK LLM calls go direct to the provider *you* chose (Anthropic, OpenAI, or localhost Ollama). Source adapters fetch public content (HN, GitHub, Reddit, CVE feeds, etc.). License validation pings Keygen if you're on a paid tier. Updates check GitHub Releases. That's the full list — documented in [NETWORK.md](../NETWORK.md) with source references.

## What about my API keys?

You bring your own keys (BYOK). They are stored in your operating system's secure vault -- Windows Credential Manager, macOS Keychain, or Linux Secret Service. Keys are sent only to the AI provider you chose. We never see, store, or touch them.

## Can I verify all of this?

Yes. Three ways:

1. **Read the code.** 4DA is source-available. Every line is public.
2. **Watch the network.** Use Wireshark or Little Snitch. You will see exactly what we described above and nothing else.
3. **Build it yourself.** Clone the repo, compile it, run your own copy. See [BUILD-FROM-SOURCE.md](BUILD-FROM-SOURCE.md).

## What if 4DA shuts down?

The app keeps working. There is no server to turn off. Your data is on your machine. The source code is public. The license converts to fully open-source (Apache 2.0) after two years. You lose nothing.

## What about the free version?

The free tier makes zero calls to 4DA Systems. Not one. If you use the free tier with Ollama for local AI, the app can run completely offline after fetching content.

## Where is the full legal version?

Read the complete Privacy Policy at [docs/legal/PRIVACY-POLICY.md](legal/PRIVACY-POLICY.md) or at [4da.ai/privacy](https://4da.ai/privacy).

## Questions?

- General: support@4da.ai
- Privacy: privacy@4da.ai
- Security: security@4da.ai

---

4DA Systems Pty Ltd | [4da.ai](https://4da.ai) | This is a plain-language summary, not a legal document. See [PRIVACY-POLICY.md](legal/PRIVACY-POLICY.md) for the full legal privacy policy.
