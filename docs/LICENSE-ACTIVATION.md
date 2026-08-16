# License Activation Guide

This guide covers how to activate a 4DA Signal or Team license. If you are using the free tier, no license key is required.

## Tiers Overview

| Tier | Key Required | Includes |
|------|-------------|----------|
| **Free** | No | All core features, relevance scoring, multi-source analysis, ACE context engine, the OSV security floor, Score Autopsy, signal chain analysis, channels, and — on your own API key or Ollama — AI briefings, natural language search, and Developer DNA |
| **Signal** | Yes | Everything in Free + Blind Spots (with AI assessment) and Knowledge Gaps — the analysis layer computed from your dependency graph and reading history |
| **Team** | Yes | Everything in Signal + team-scoped context sharing, centralized configuration |

The free tier is fully functional. Security baselines are never paywalled, and the BYOK
features above stay free because they run on your own key at no cost to us. Signal and Team
unlock the intelligence analysis layer — Blind Spots and Knowledge Gaps — for users who
want deeper analysis of their content.

## Getting Your License Key

1. Go to [4da.ai/signal](https://4da.ai/signal) and select a Signal or Team plan.
2. Complete the purchase. You will receive a license key via email.
3. Your key will be in one of two formats, depending on when it was issued:

   ```
   4DA-<payload>.<signature>
   BE1234-567890-ABCDEF-...
   ```

   Keys beginning with `4DA-` are self-signed: they carry an Ed25519 signature that is
   verified entirely on your device, with no network call. Keys beginning with `BE` are
   validated against the Keygen service. Both are accepted. Keep your key stored securely.

## Activating Your License

1. **Open Settings** -- press `,` (comma) or click the gear icon in the top navigation.
2. **Navigate to the License section** on the Intelligence tab.
3. **Paste your license key** into the input field. Copy-paste is recommended to avoid typos.
4. **Click Activate**. A `4DA-` key is verified locally via Ed25519; a `BE` key is validated against the Keygen API.
5. **Confirmation**: On success, the tier indicator changes to **Signal** (displayed in gold) with the message "All Signal features unlocked."

The key is persisted in your local settings. You will not need to re-enter it after restarting the app.

## Verifying Activation

After activation, confirm the following:

- **Settings > Intelligence**: The License section displays your active tier as "Signal" or "Team."
- **Feature availability**: Signal-only features (Blind Spots and Knowledge Gaps) are accessible without restriction.

  Note that Developer DNA (AD-026) and natural language search and AI briefings (AD-025) are **not** Signal-gated — they are free-tier features that run on your own API key or Ollama at no cost to us.
- **Status bar**: The app may display a tier badge in the UI confirming your active plan.

If any of these do not reflect your expected tier, see Troubleshooting below.

## Trial

4DA offers a **14-day free trial** of Signal features. No license key is needed to start the trial.

- The trial activates automatically when you first launch the app.
- All Signal features are available during the trial period.
- When the trial expires, the app reverts to the free tier. Your data and settings are preserved.
- You can upgrade to a paid license at any time during or after the trial.

## Troubleshooting

### "Invalid key"

- Verify the key was copied in full — for a `4DA-` key this includes the signature after the `.` separator.
- Remove any leading or trailing whitespace. (Line breaks pasted from an email are stripped automatically.)
- Ensure no characters were dropped or transposed. Re-copy from the original email.

### Network error during activation

- This applies only to `BE` keys, which are validated against the Keygen API. A `4DA-` key is verified locally and never needs a connection.
- Check that you are online.
- If you are behind a corporate proxy or firewall, ensure outbound HTTPS to `api.keygen.sh` is not blocked.
- Wait a moment and try again. Transient network failures resolve on retry, and a failed check never downgrades an existing tier.

### App shows Free tier after restart

- This should not happen. The license key is persisted in your local `settings.json`.
- If it does occur, re-enter your key in Settings > Intelligence > License and click Activate again.
- If the problem persists, check that the `data/settings.json` file is writable and not being reset by another process.

### Activation succeeds but Signal features are unavailable

- Restart the app to ensure all feature gates refresh.
- Verify the displayed tier in Settings matches what you expect.
- If the issue persists, contact support@4da.ai with your key (first 6 characters only) and a description of the problem.

## FAQ

### Does 4DA work offline after activation?

Yes. License validation is cached locally for 90 days, so you can use Signal features offline for that whole window. When you are next online, validation refreshes automatically in the background once the cache expires. Self-signed `4DA-` keys go further: they carry an Ed25519 signature that is verified entirely on your device and never require a network call at all.

### Can I move my license to a different machine?

Yes. Enter the same license key in **Settings > Intelligence > License** on the new machine. Validation is key-based only — 4DA does not register machines, send a hardware fingerprint, or cap the number of installs — so there is no deactivation step to perform on the old machine.

### What happens if I reinstall the app?

Reinstalling clears local settings. Re-enter your license key after installation. The key itself remains valid.

### Is the STREETS playbook included in the free tier?

Yes, and no key is needed — but it is no longer a tab inside the app. All 7 STREETS modules are published free on the open web at [4da.ai/streets](https://4da.ai/streets), for every user regardless of tier.

### How do I upgrade from Signal to Team?

Purchase a Team license at [4da.ai](https://4da.ai). Enter the new Team key in Settings. The previous Signal key will be replaced.

### How do I cancel or get a refund?

Contact support@4da.ai with your purchase details. Refer to the refund policy on [4da.ai](https://4da.ai).

### Where is my license key stored?

Locally in `data/settings.json` on your machine. 4DA Systems does not operate a license server: a `4DA-` key is verified on-device and transmitted nowhere, and a `BE` key is sent only to the third-party Keygen validation API. See the [Privacy Features](./FEATURES.md#privacy-features) documentation.
