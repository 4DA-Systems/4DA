# Shopify Contact Page Fix — Instructions

Two files to install. Total time: ~10 minutes.

## Step 1: Replace the page content (above the form)

1. **Shopify Admin** → **Online Store** → **Pages** → click **Contact**.
2. In the content editor, switch to **HTML mode** (click the `<>` button in the editor toolbar).
3. **Select all** existing content → **delete it**.
4. **Paste** the contents of `shopify-contact-page-content.html`.
5. Click **Save**.

This adds:
- Context ("Have a question? We respond within one business day.")
- FAQ deflection box (license, SmartScreen, refund, bugs)
- Direct email + GitHub Issues buttons
- "Or use the form below" lead-in

## Step 2: Replace the contact form (the form itself)

This is the more involved step — you're editing the Shopify theme template.

1. **Shopify Admin** → **Online Store** → **Themes** → your active theme → **"..." → Edit code**.
2. Find the contact form template. Search for `contact` in the file browser. Typical locations:
   - **Dawn theme**: `sections/main-page.liquid` (the form is in the page template)
   - **Some themes**: `templates/page.contact.liquid`
   - **Others**: search for `{% form 'contact' %}` across all files — that's the form tag.
3. Find the block from `{% form 'contact' %}` to `{% endform %}` (inclusive).
4. Replace that entire block with the contents of `shopify-contact-form-template.liquid`.
5. Click **Save**.

### What changed in the form:

| Before | After |
|--------|-------|
| Name (placeholder only, no label) | Name (label + placeholder) |
| Email (placeholder only) | Email (label + placeholder) |
| Phone (required, broken regex validation) | **Removed** |
| Message (placeholder only) | Message (label + placeholder) |
| — | **Topic dropdown** (General / License / Install / Bug / Feature / Merch / Press / Other) |
| — | **Order/license number** (optional, for billing inquiries) |
| Generic "Submit" button | "Send message" + response time + GitHub link |
| No success message | **Success screen** with green check + "within one business day" |

### If you can't find the form template

Some Shopify themes use an "app block" for the contact form. In that case:

1. Go to **Online Store → Themes → Customize**.
2. Navigate to the **Contact page** in the page selector (top bar).
3. Find the "Contact form" section in the left panel.
4. If it's an app block, you may need to switch to a "Custom Liquid" section and paste the form template there.

## Step 3: Fix the footer email signup copy

The current footer says "Join our email list — Get exclusive deals and early access to new products." That's generic Shopify copy.

1. **Shopify Admin** → **Online Store** → **Themes** → **Customize**.
2. Scroll to the **Footer** section.
3. Find the "Newsletter" or "Email signup" block.
4. Change the heading to: **Stay in the loop**
5. Change the description to: **Release notes, intelligence updates, and the occasional insight. No spam, unsubscribe anytime.**
6. Click **Save**.

## Verify

After both steps:
1. Visit `shop.4da.ai/pages/contact` in an incognito window.
2. Confirm the FAQ box and buttons appear above the form.
3. Fill out the form — confirm no phone field, topic dropdown works, submit succeeds.
4. Check your email (support@4da.ai or wherever Shopify notifications route) — confirm the topic and order number appear in the notification.
5. Confirm the success message appears after submission.
