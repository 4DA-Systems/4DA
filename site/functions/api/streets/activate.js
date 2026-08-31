// LEGACY PATH SHIM — do not add logic here.
//
// The Signal licence endpoint lived at /api/streets/activate until 2026-08-20
// (the paid tier was once branded "STREETS"). The canonical route is now
// /api/license/activate; this shim serves the identical handler at the old URL.
//
// Who still calls this path:
//   * pre-rename desktop builds — the recovery URL in
//     src-tauri/src/settings_commands_license.rs is baked into the installed
//     binary, so every build shipped before the rename GETs this path;
//   * the pre-rename Stripe event destination, until its URL is flipped to
//     /api/license/activate in the dashboard.
//
// Deletable once BOTH are gone: the Stripe destination points at the new URL
// AND no pre-rename desktop build remains in use. Until then this file is three
// lines of insurance on the payment path — leave it alone.
export { onRequest } from '../license/activate.js';
