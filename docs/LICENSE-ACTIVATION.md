# License Activation Guide

This guide covers how to activate a 4DA Signal or Team license. If you are using the free tier, no license key is required.

## Tiers Overview

| Tier | Key Required | Includes |
|------|-------------|----------|
| **Free** | No | All core features, AI briefings, STREETS playbook (all 7 modules), relevance scoring, multi-source analysis, ACE context engine |
| **Signal** | Yes | Everything in Free + Developer DNA, Signal Chains, Knowledge Gaps, Semantic Shifts, Natural Language Search, Score Autopsy, Project Health, Standing Queries, Attention Report, Decision Signals |
| **Team** | Yes | Everything in Signal + team-scoped context sharing, centralized configuration |

The free tier is fully functional and includes AI briefings. Signal and Team unlock the intelligence analysis layer for users who want compound insights from their content.

## Getting Your License Key

1. Go to [4da.ai/signal](https://4da.ai/signal) and select a Signal or Team plan.
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
4. **Click Activate**. The app will validate your key against the Keygen API.
5. **Confirmation**: On success, the tier indicator changes to **Signal** (displayed in gold) with the message "All Signal features unlocked."

The key is persisted in your local settings. You will not need to re-enter it after restarting the app.

## Verifying Activation

After activation, confirm the following:

- **Settings > General**: The License section displays your active tier as "Signal" or "Team."
- **Feature availability**: Signal-only features (Developer DNA, Signal Chains, Knowledge Gaps, Score Autopsy, Natural Language Search) are accessible without restriction.
- **Status bar**: The app may display a tier badge in the UI confirming your active plan.

If any of these do not reflect your expected tier, see Troubleshooting below.

## Trial

4DA offers a **30-day free trial** of Pro features. No license key is needed to start the trial.

- The trial activates automatically when you first launch the app.
- All Pro features are available during the trial period.
- When the trial expires, the app reverts to the free tier. Your data and settings are preserved.
- You can upgrade to a paid license at any time during or after the trial.

## Troubleshooting

### "Invalid key"

- Verify the key was copied in full, including the `-V3` suffix.
- Remove any leading or trailing whitespace.
- Ensure no characters were dropped or transposed. Re-copy from the original email.

### Network error during activation

- License validation requires an internet connection. Check that you are online.
- If you are behind a corporate proxy or firewall, ensure outbound HTTPS requests to the Keygen API are not blocked.
- Wait a moment and try again. Transient network failures resolve on retry.

### "Key already activated" or device limit reached

- Each license key has a maximum number of machine activations.
- If you have reached the limit, deactivate an existing machine from your account at [4da.ai](https://4da.ai), or contact support.
- Email: support@4da.ai

### App shows Free tier after restart

- This should not happen. The license key is persisted in your local `settings.json`.
- If it does occur, re-enter your key in Settings > General > License and click Activate again.
- If the problem persists, check that the `data/settings.json` file is writable and not being reset by another process.

### Activation succeeds but Pro features are unavailable

- Restart the app to ensure all feature gates refresh.
- Verify the displayed tier in Settings matches what you expect.
- If the issue persists, contact support@4da.ai with your key (first 6 characters only) and a description of the problem.

## FAQ

### Does 4DA work offline after activation?

Yes. License validation is cached locally for 24 hours. You can use Pro features offline during that window. The next time you are online, validation refreshes automatically in the background. If you remain offline for more than 24 hours, the app may prompt for revalidation when connectivity returns.

### Can I move my license to a different machine?

Yes. Deactivate the license on your current machine (Settings > General > License > Deactivate), then activate on the new machine. Depending on your plan, you may have a limited number of concurrent activations.

### What happens if I reinstall the app?

Reinstalling clears local settings. Re-enter your license key after installation. The key itself remains valid.

### Is the STREETS playbook included in the free tier?

Yes. All 7 STREETS modules are available to every user, regardless of tier. No license key is needed for the playbook.

### How do I upgrade from Pro to Team?

Purchase a Team license at [4da.ai](https://4da.ai). Enter the new Team key in Settings. The previous Pro key will be replaced.

### How do I cancel or get a refund?

Contact support@4da.ai with your purchase details. Refer to the refund policy on [4da.ai](https://4da.ai).

### Where is my license key stored?

Locally in `data/settings.json` on your machine. It is never transmitted to 4DA servers beyond the initial Keygen validation call. See the [Privacy Features](./FEATURES.md#privacy-features) documentation.
