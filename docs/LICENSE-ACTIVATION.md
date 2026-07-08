# License Activation Guide

This guide covers how to activate a 4DA Signal license. If you are using the free tier, no license key is required.

## Tiers Overview

| Tier | Key Required | Includes |
|------|-------------|----------|
| **Free** | No | All 20+ content sources, the full 5-axis scoring engine, AI daily briefings + weekly digest (BYOK), the multi-signal confirmation gate, ACE auto context discovery, behavior learning, Natural Language Search (BYOK), Developer DNA cards, the MCP server (14 tools), and the CLI. No expiration. |
| **Signal** | Yes | Everything in Free + the Signal tab (Key Signals + analytics), Score Autopsy, Signal Chains, Knowledge Gaps, Semantic Shifts, Attention Analytics, Standing Queries, and Project Health |

The free tier is a complete product — it is fully functional, never expires, and includes AI briefings. Signal unlocks the compound-intelligence analysis layer for developers who want the system to surface deeper patterns from their content.

## Getting Your License Key

1. Go to [4da.ai/signal](https://4da.ai/signal) and subscribe to the Signal plan ($12/month or $99/year).
2. Complete the purchase. You will receive a license key via email.
3. Your key is in the format:

   ```
   XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXXXX-V3
   ```

   Six groups of six characters separated by hyphens, ending with a version suffix. Keep this key stored securely.

## Activating Your License

1. **Open Settings** -- press `,` (comma) or click the gear icon in the top navigation.
2. **Navigate to the License section** on the General tab.
3. **Paste your license key** into the input field. Copy-paste is recommended to avoid typos.
4. **Click Activate**. The app validates your key online, then verifies it locally with Ed25519.
5. **Confirmation**: On success, the tier indicator changes to **Signal** (displayed in gold) with the message "All Signal features unlocked."

The key is persisted in your local settings. You will not need to re-enter it after restarting the app.

## Verifying Activation

After activation, confirm the following:

- **Settings > General**: The License section displays your active tier as "Signal."
- **Header bar**: A gold **SIGNAL** badge appears (it reads **SIGNAL TRIAL** during the trial and gray **FREE** otherwise).
- **Feature availability**: Signal-only features (Blind Spots, Knowledge Gaps, Semantic Shifts, Attention Analytics, Standing Queries, Project Health) are accessible without restriction.

If any of these do not reflect your expected tier, see Troubleshooting below.

## Trial

4DA includes a **14-day free trial** of Signal features. No license key is needed to start the trial.

- The trial starts automatically the first time you launch the app.
- All Signal features are available during the trial period.
- When the trial expires, the app reverts to the free tier. Your data and settings are preserved.
- You can subscribe to Signal at any time during or after the trial.

## Troubleshooting

### "Invalid key"

- Verify the key was copied in full, including the `-V3` suffix.
- Remove any leading or trailing whitespace.
- Ensure no characters were dropped or transposed. Re-copy from the original email.

### Network error during activation

- Online validation requires an internet connection. Check that you are online.
- If you are behind a corporate proxy or firewall, ensure outbound HTTPS requests to the license server are not blocked.
- Wait a moment and try again. Transient network failures resolve on retry.

### "Key already activated" or device limit reached

- Each license key has a maximum number of machine activations.
- If you have reached the limit, deactivate an existing machine from your account at [4da.ai](https://4da.ai), or contact support.
- Email: support@4da.ai

### App shows Free tier after restart

- This should not happen. The license key is persisted in your local `settings.json`.
- If it does occur, re-enter your key in Settings > General > License and click Activate again.
- If the problem persists, check that the `data/settings.json` file is writable and not being reset by another process.

### Activation succeeds but Signal features are unavailable

- Restart the app to ensure all feature gates refresh.
- Verify the displayed tier in Settings matches what you expect.
- If the issue persists, contact support@4da.ai with your key (first 6 characters only) and a description of the problem.

## FAQ

### Does 4DA work offline after activation?

Yes. Once validated, your license is cached locally so Signal features keep working offline. When you are online, the app quietly re-checks validity in the background. You would need to be offline for an extended period (well beyond normal use) before the app prompts for revalidation when connectivity returns.

### Can I move my license to a different machine?

Yes. Deactivate the license on your current machine (Settings > General > License > Deactivate), then activate on the new machine. Depending on your plan, you may have a limited number of concurrent activations.

### What happens if I reinstall the app?

Reinstalling clears local settings. Re-enter your license key after installation. The key itself remains valid.

### Is the STREETS playbook inside the app?

No. The STREETS playbook — 7 modules on turning developer skills into independent income — is published free on the open web at [4da.ai/streets](https://4da.ai/streets). It is not a tab inside the app; 4DA stays focused on intelligence (Brief, Preemption, Blind Spots, Signal). No license key is needed to read it.

### How do I cancel or get a refund?

Contact support@4da.ai with your purchase details. Refer to the refund policy on [4da.ai](https://4da.ai).

### Where is my license key stored?

Locally in `data/settings.json` on your machine. It is only sent to the license server for the initial online activation and periodic revalidation; it is never used to transmit your content. See the [Privacy Features](./FEATURES.md#privacy-features) documentation.
