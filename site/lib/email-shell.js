// Shared chrome for every transactional email 4DA sends.
//
// WHY THIS IS A TABLE AND NOT A DIV
// ---------------------------------
// The first version styled `<body>` with `max-width: 560px; margin: 40px auto`.
// Outlook on Windows renders through Word, which ignores `max-width` and most
// margins, so that email arrived full-bleed and ragged for exactly the desktop
// business audience most likely to buy a developer tool. Nested presentation
// tables with explicit widths are the only layout every client honours, which is
// why every mature transactional email is built this way.
//
// WHY THE BRANDING IS TEXT, NOT AN IMAGE
// --------------------------------------
// Most clients block remote images by default, so an image-only header renders
// as an empty box on first open -- the worst possible first impression for a
// paid product. A text wordmark always renders, in every client, offline.
//
// The 4-sun mark is therefore ADDITIVE, never load-bearing: alt="" and fixed
// dimensions mean a blocked image leaves a clean 46px gap while the text
// wordmark still carries the brand. It is referenced by absolute URL because
// Gmail strips data: URIs; the asset ships with the site (email-sun.jpg — the
// white-tee sun-on-white cutout, so no black box sits on the white header).
//
// WHY THE PREHEADER MATTERS MORE THAN IT LOOKS
// --------------------------------------------
// Without one, the client fills the inbox preview line from the top of the body.
// For the licence email that meant the preview -- and the phone lock screen, and
// the desktop notification banner -- displayed the raw base64 LICENCE KEY.
// Unprofessional, and a small privacy leak to anyone glancing at the screen.
// The hidden preheader claims that line deliberately, and the zero-width padding
// after it stops the client pulling body text in behind it.

const BRAND = {
  black: '#0A0A0A',
  gold: '#D4AF37',
  ink: '#1A1A1A',
  muted: '#5A5A5A',
  faint: '#8A8A8A',
  hair: '#E5E5E5',
  panel: '#FAFAFA',
  page: '#F4F4F5',
  white: '#FFFFFF',
};

// Brand fonts first (render where installed), system stacks as the graceful
// fallback — email clients do not load webfonts.
const FONT_STACK =
  "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";
const MONO_STACK =
  "'JetBrains Mono', 'SF Mono', SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace";

const SUN_URL = 'https://4da.ai/email-sun.jpg';

/** Zero-width padding so body text cannot bleed into the preview line. */
const PREHEADER_PAD = '&#847;&zwnj;&nbsp;&#8203;'.repeat(30);

/**
 * How long a header badge may be.
 *
 * The header row is a fixed width budget, and email has no media queries worth
 * relying on -- Gmail strips <style> in several contexts, so a responsive rule
 * cannot be trusted to run. The three cells must therefore fit unaided at the
 * narrowest common phone width.
 *
 * On a 320px viewport the card's 32px side padding leaves ~256px. The sun is
 * 46px, the gap 12px and the wordmark ~45px, leaving roughly 150px. At 11px
 * uppercase with 0.16em tracking a character costs ~8.5px, so ~17 characters
 * is the ceiling; 16 keeps a margin. Beyond it the row wraps and the wordmark
 * gets squeezed, which is why `Subscription renewed` (20) became `Renewal`.
 *
 * Both header cells are additionally `white-space: nowrap`, so if a future
 * badge does overflow the layout pushes rather than fracturing mid-word.
 */
export const BADGE_MAX_CHARS = 16;

/**
 * A button that survives Outlook.
 *
 * A padded `<a>` collapses to bare underlined text in Word-rendered Outlook,
 * because it ignores padding on inline elements. Putting the colour and padding
 * on a `<td>` instead renders everywhere; Outlook simply shows square corners,
 * which is a far better failure than an invisible call to action.
 */
export function emailButton(href, label) {
  return `
              <table role="presentation" cellpadding="0" cellspacing="0" border="0" style="margin: 0 auto;">
                <tr>
                  <td align="center" bgcolor="${BRAND.gold}" style="border-radius: 6px;">
                    <a href="${href}" style="display: inline-block; padding: 13px 26px; font-family: ${FONT_STACK}; font-size: 15px; font-weight: 600; color: ${BRAND.black}; text-decoration: none; border-radius: 6px;">${label}</a>
                  </td>
                </tr>
              </table>`;
}

/** The monospace panel the licence key sits in. The 3px gold spine is the
 *  accent that marks the key as THE artifact of the email. */
export function keyPanel(escapedKey) {
  return `
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
                <tr>
                  <td bgcolor="${BRAND.panel}" style="background-color: ${BRAND.panel}; border: 1px solid ${BRAND.hair}; border-left: 3px solid ${BRAND.gold}; border-radius: 0 8px 8px 0; padding: 18px 20px; font-family: ${MONO_STACK}; font-size: 13px; line-height: 1.7; color: ${BRAND.ink}; word-break: break-all;">${escapedKey}</td>
                </tr>
              </table>`;
}

/**
 * Wrap content in the shared shell.
 *
 * `color-scheme: light` is declared deliberately. Gmail and Apple Mail auto-invert
 * undeclared light emails in dark mode, and the inversion is not colour-aware --
 * it routinely turns a pale key panel into near-black behind dark text. The key
 * has to stay legible, so this email opts out of being reinterpreted rather than
 * gambling on each client's algorithm.
 */
export function renderShell({ title, preheader, badge, contentHtml, footerHtml }) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="color-scheme" content="light">
  <meta name="supported-color-schemes" content="light">
  <title>${title}</title>
</head>
<body style="margin: 0; padding: 0; background-color: ${BRAND.page};">
  <div style="display: none; max-height: 0; overflow: hidden; mso-hide: all;">${preheader}</div>
  <div style="display: none; max-height: 0; overflow: hidden; mso-hide: all;">${PREHEADER_PAD}</div>

  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" bgcolor="${BRAND.page}" style="background-color: ${BRAND.page};">
    <tr>
      <td align="center" style="padding: 32px 12px;">

        <table role="presentation" width="600" cellpadding="0" cellspacing="0" border="0" style="width: 100%; max-width: 600px; border-collapse: separate;">

          <tr>
            <td bgcolor="${BRAND.gold}" style="background-color: ${BRAND.gold}; height: 3px; line-height: 3px; font-size: 0; border-radius: 10px 10px 0 0;">&nbsp;</td>
          </tr>

          <tr>
            <td bgcolor="${BRAND.white}" style="background-color: ${BRAND.white}; padding: 18px 32px; border-left: 1px solid ${BRAND.hair}; border-right: 1px solid ${BRAND.hair}; border-bottom: 1px solid ${BRAND.hair};">
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
                <tr>
                  <td width="46" style="width: 46px;"><img src="${SUN_URL}" alt="" width="46" height="46" style="display: block; border: 0;"></td>
                  <td style="padding-left: 12px; white-space: nowrap; font-family: ${FONT_STACK}; font-size: 17px; font-weight: 700; letter-spacing: 0.14em; color: ${BRAND.black}; vertical-align: middle;">4DA</td>
                  <td align="right" style="white-space: nowrap; font-family: ${FONT_STACK}; font-size: 11px; font-weight: 600; letter-spacing: 0.16em; text-transform: uppercase; color: ${BRAND.faint}; vertical-align: middle;">${badge}</td>
                </tr>
              </table>
            </td>
          </tr>

          <tr>
            <td bgcolor="${BRAND.white}" style="background-color: ${BRAND.white}; padding: 30px 32px; border-left: 1px solid ${BRAND.hair}; border-right: 1px solid ${BRAND.hair};">
${contentHtml}
            </td>
          </tr>

          <tr>
            <td bgcolor="${BRAND.white}" style="background-color: ${BRAND.white}; padding: 0 32px 28px; border-left: 1px solid ${BRAND.hair}; border-right: 1px solid ${BRAND.hair}; border-bottom: 1px solid ${BRAND.hair}; border-radius: 0 0 10px 10px;">
              <div style="border-top: 1px solid ${BRAND.hair}; padding-top: 20px;">
${footerHtml}
              </div>
            </td>
          </tr>

        </table>

      </td>
    </tr>
  </table>
</body>
</html>`;
}

export { BRAND, FONT_STACK, MONO_STACK };
