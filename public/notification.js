// 4DA Custom Notification Window
// Standalone vanilla JS — no React, no bundler dependency.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

const DISMISS_DURATIONS = {
  critical: 8000,
  alert: 6000,
  advisory: 5000,
  watch: 4000,
};

// ---------------------------------------------------------------------------
// DOM references
// ---------------------------------------------------------------------------

const card = document.getElementById('card');
const atmosphereLayer = document.getElementById('atmosphere-layer');
const priorityDot = document.getElementById('priority-dot');
const typeLabel = document.getElementById('type-label');
const sourceBadge = document.getElementById('source-badge');
const timeAgo = document.getElementById('time-ago');
const titleText = document.getElementById('title-text');
const actionText = document.getElementById('action-text');
const depsContainer = document.getElementById('deps-container');
const chainContainer = document.getElementById('chain-container');
const chainDots = document.getElementById('chain-dots');
const dismissBtn = document.getElementById('dismiss-btn');
const notificationEl = document.getElementById('notification');

let dismissTimer = null;
let isHovering = false;
let currentPriority = 'low';
let currentItemId = null;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Truncate a string to `max` characters, appending ellipsis if needed. */
function truncate(str, max) {
  if (!str) return '';
  return str.length > max ? str.slice(0, max) + '\u2026' : str;
}

/** Map variant / signal_type to a display label for the header. */
function getTypeLabel(variant, signalType) {
  if (variant === 'chain') return 'CHAIN ESCALATING';
  if (variant === 'multi') return 'SIGNALS';
  if (variant === 'digest') return 'NEW ITEMS';
  if (variant === 'briefing') return 'MORNING BRIEFING';

  const labels = {
    security_alert: 'SECURITY ALERT',
    breaking_change: 'BREAKING CHANGE',
    tool_discovery: 'TOOL DISCOVERY',
    tech_trend: 'TECH TREND',
    learning: 'LEARNING',
    competitive_intel: 'COMPETITIVE INTEL',
  };
  return labels[signalType] || 'SIGNAL';
}

/** Build dependency pill HTML fragments. Show at most 3, with overflow count. */
function buildDeps(deps) {
  if (!deps || deps.length === 0) return '';
  const maxShow = 3;
  const shown = deps.slice(0, maxShow);
  let html = shown
    .map(function (d) {
      return '<span class="dep-pill">' + escapeHtml(d) + '</span>';
    })
    .join('');
  if (deps.length > maxShow) {
    html += '<span class="dep-overflow">+' + (deps.length - maxShow) + '</span>';
  }
  return html;
}

/** Build chain dot + connector HTML for chain-type notifications. */
function buildChainDots(filled, total) {
  if (!total) return '';
  let html = '';
  for (let i = 0; i < total; i++) {
    if (i > 0) {
      var connClass = i < filled ? 'filled' : '';
      html += '<span class="chain-connector ' + connClass + '"></span>';
    }
    var dotClass = i < filled ? 'filled' : 'empty';
    html += '<span class="chain-dot ' + dotClass + '"></span>';
  }
  return html;
}

/** Minimal HTML entity escaping for user-supplied text. */
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ---------------------------------------------------------------------------
// GPU rendering (progressive enhancement over CSS)
// ---------------------------------------------------------------------------

/** Card components — full GPU-rendered card with occlude blend. */
var NOTIF_CARD_TAGS = {
  critical: 'fourda-notif-card-critical',
  high: 'fourda-notif-card-high',
  medium: 'fourda-notif-card-medium',
  low: 'fourda-notif-card-low',
};
var NOTIF_CARD_SCRIPTS = {
  critical: '/notif-card-critical.js',
  high: '/notif-card-high.js',
  medium: '/notif-card-medium.js',
  low: '/notif-card-low.js',
};
var loadedScripts = {};
var runtimeLoaded = false;
var currentGameEl = null;
var gpuAvailable = null; // null = untested, true/false after detection

/** Detect if GPU rendering is available (WebGL2 as minimum). */
function detectGPU() {
  if (gpuAvailable !== null) return gpuAvailable;
  try {
    var canvas = document.createElement('canvas');
    var gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
    gpuAvailable = !!gl;
    if (gl) {
      // Clean up the test context
      var ext = gl.getExtension('WEBGL_lose_context');
      if (ext) ext.loseContext();
    }
    canvas.remove();
  } catch (e) {
    gpuAvailable = false;
  }
  console.log('[4DA Notification] GPU available:', gpuAvailable);
  return gpuAvailable;
}

/** Load the shared 4DA component runtime (renderer classes) once. Returns a promise. */
function ensureGameRuntime() {
  if (runtimeLoaded) return Promise.resolve();
  return new Promise(function (resolve) {
    runtimeLoaded = true;
    var script = document.createElement('script');
    script.src = '/fourda-runtime.js';
    script.onload = resolve;
    script.onerror = function () {
      runtimeLoaded = false; // Allow retry
      resolve(); // Don't block — component will fail gracefully
    };
    document.head.appendChild(script);
  });
}

/** Lazy-load a card component script (only loaded once per priority).
 *  Loads the shared runtime first, then the lightweight component. */
function ensureGameScript(priority) {
  if (loadedScripts[priority]) return;
  loadedScripts[priority] = true;
  var src = NOTIF_CARD_SCRIPTS[priority];
  if (!src) return;
  ensureGameRuntime().then(function () {
    var script = document.createElement('script');
    script.type = 'module';
    script.src = src;
    document.head.appendChild(script);
  });
}

/** Upgrade from CSS card to GPU-rendered card.
 *  CSS card stays visible until GPU render is confirmed working.
 *  If GPU render fails, CSS card remains — user sees no difference. */
function upgradeToGameCard(priority) {
  if (!detectGPU()) return; // No GPU — CSS card stays

  var tagName = NOTIF_CARD_TAGS[priority] || NOTIF_CARD_TAGS.low;

  // Skip if already showing the correct component
  if (currentGameEl && currentGameEl.tagName.toLowerCase() === tagName) return;

  // Remove current atmosphere element
  destroyGameComponent();

  // Check if the custom element is registered yet
  if (!customElements.get(tagName)) return;

  // Only create if visible (non-zero size)
  var rect = atmosphereLayer.getBoundingClientRect();
  if (rect.width < 1 || rect.height < 1) return;

  // Create atmosphere element behind the CSS card
  currentGameEl = document.createElement(tagName);
  currentGameEl.style.cssText = 'position:absolute;inset:0;display:block;z-index:0;';
  atmosphereLayer.appendChild(currentGameEl);

  // Atmosphere renders the full card — hide CSS card background so text floats on it
  card.classList.add('atmosphere-active');
  atmosphereLayer.classList.add('active');
  atmosphereLayer.style.setProperty('--atmosphere-opacity', '1');
}

/** Destroy atmosphere component when notification hides (stops GPU rendering). */
function destroyGameComponent() {
  if (currentGameEl) {
    currentGameEl.remove();
    currentGameEl = null;
  }
  // Restore CSS card background
  card.classList.remove('atmosphere-active');
  atmosphereLayer.classList.remove('active');
}

// ---------------------------------------------------------------------------
// Content update
// ---------------------------------------------------------------------------

/** Populate the notification card DOM from a data payload. */
function updateContent(data) {
  currentPriority = data.priority || 'low';
  currentItemId = data.item_id || null;

  // Swap priority classes on card and dot
  card.classList.remove('critical', 'high', 'medium', 'low');
  priorityDot.classList.remove('critical', 'high', 'medium', 'low');
  card.classList.add(currentPriority);
  priorityDot.classList.add(currentPriority);

  // ARIA semantics — critical gets assertive alert role
  notificationEl.setAttribute(
    'role',
    currentPriority === 'critical' ? 'alert' : 'status'
  );
  notificationEl.setAttribute(
    'aria-live',
    currentPriority === 'critical' ? 'assertive' : 'polite'
  );

  // Atmosphere — subtle living texture, not the main event
  var gameOpacity = { critical: 0.12, high: 0.09, medium: 0.06, low: 0.04 };
  atmosphereLayer.style.setProperty(
    '--atmosphere-opacity',
    String(gameOpacity[currentPriority] || 0.05)
  );

  // Progressive enhancement: try to upgrade CSS card to GPU-rendered card
  if (detectGPU()) {
    ensureGameScript(currentPriority);
    // Defer upgrade to allow script to register the custom element
    setTimeout(function () { upgradeToGameCard(currentPriority); }, 150);
  }

  // -- Header --
  typeLabel.textContent = getTypeLabel(data.variant, data.signal_type);

  if (data.variant === 'multi' && data.count) {
    sourceBadge.textContent = '';
    typeLabel.textContent = data.count + ' ' + currentPriority.toUpperCase() + ' SIGNALS';
  } else if (data.variant === 'digest' && data.count) {
    sourceBadge.textContent = '';
    typeLabel.textContent = data.count + ' NEW ITEMS';
  } else if (data.variant === 'briefing' && data.count) {
    sourceBadge.textContent = data.count + ' items';
  } else if (data.source) {
    sourceBadge.textContent = data.source;
  } else {
    sourceBadge.textContent = '';
  }

  timeAgo.textContent = data.time_ago || 'just now';

  // -- Body --
  titleText.textContent = truncate(data.title, 60);
  actionText.textContent = data.action || '';

  // -- Footer: deps or chain --
  if (data.variant === 'chain' && data.chain_links_total) {
    depsContainer.style.display = 'none';
    chainContainer.style.display = 'flex';
    chainDots.innerHTML = buildChainDots(
      data.chain_links_filled || 0,
      data.chain_links_total || 4
    );
    // Append chain sources text if provided
    if (data.chain_sources && data.chain_sources.length > 0) {
      var existing = chainContainer.querySelector('.chain-sources');
      if (existing) existing.remove();
      var sourcesEl = document.createElement('span');
      sourcesEl.className = 'chain-sources';
      sourcesEl.textContent = data.chain_sources.join(' \u2192 ');
      chainContainer.appendChild(sourcesEl);
    }
  } else {
    chainContainer.style.display = 'none';
    depsContainer.style.display = 'flex';
    depsContainer.innerHTML = buildDeps(data.matched_deps);
  }
}

// ---------------------------------------------------------------------------
// Sound synthesis (Web Audio API — no audio files needed)
// ---------------------------------------------------------------------------

var audioCtx = null;

/** Play a synthesized notification chime based on priority. */
function playNotificationSound(priority) {
  // Only critical and high get sounds
  if (priority !== 'critical' && priority !== 'high') return;

  try {
    if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    var ctx = audioCtx;
    var now = ctx.currentTime;

    if (priority === 'critical') {
      // Two ascending tones: C5 → E5 (urgent)
      playTone(ctx, 523.25, now, 0.08, 0.15);       // C5
      playTone(ctx, 659.25, now + 0.1, 0.08, 0.15);  // E5
    } else {
      // Single soft chime: G4 (present but gentle)
      playTone(ctx, 392.0, now, 0.12, 0.1);  // G4
    }
  } catch {
    // Audio not available — silent fallback
  }
}

/** Play a single sine tone with attack/decay envelope. */
function playTone(ctx, freq, startTime, duration, volume) {
  var osc = ctx.createOscillator();
  var gain = ctx.createGain();
  osc.type = 'sine';
  osc.frequency.value = freq;
  gain.gain.setValueAtTime(0, startTime);
  gain.gain.linearRampToValueAtTime(volume, startTime + 0.01);  // 10ms attack
  gain.gain.exponentialRampToValueAtTime(0.001, startTime + duration);  // decay
  osc.connect(gain);
  gain.connect(ctx.destination);
  osc.start(startTime);
  osc.stop(startTime + duration + 0.05);
}

// ---------------------------------------------------------------------------
// Show / Hide
// ---------------------------------------------------------------------------

/** Show the notification with enter animation. */
function showNotification(data) {
  updateContent(data);

  // Play notification sound for critical/high priority
  playNotificationSound(currentPriority);

  // Reset animation state
  card.classList.remove('visible', 'exiting', 'dismissing');

  // Trigger enter animation on next frame so CSS transition fires
  requestAnimationFrame(function () {
    card.classList.add('visible');
    // Atmosphere fades in slightly after card appears
    setTimeout(function () {
      atmosphereLayer.classList.add('active');
    }, 200);
  });

  startDismissTimer();
}

/** Start or restart the auto-dismiss countdown. */
function startDismissTimer() {
  clearTimeout(dismissTimer);
  var duration = DISMISS_DURATIONS[currentPriority] || 4000;
  dismissTimer = setTimeout(function () {
    if (!isHovering) {
      hideNotification('auto');
    }
  }, duration);
}

/** Hide the notification with the appropriate exit animation. */
function hideNotification(reason) {
  clearTimeout(dismissTimer);

  if (reason === 'dismiss') {
    card.classList.add('dismissing');
  } else {
    card.classList.add('exiting');
  }
  atmosphereLayer.classList.remove('active');

  // After animation completes, clean up atmosphere + signal Rust to hide
  var delay = reason === 'dismiss' ? 150 : 400;
  setTimeout(function () {
    card.classList.remove('visible', 'exiting', 'dismissing');
    destroyGameComponent(); // Stop GPU rendering when hidden
    emitTauri('notification-hidden');
  }, delay);
}

// ---------------------------------------------------------------------------
// Tauri IPC helpers
// ---------------------------------------------------------------------------

/** Emit an event to the Rust backend via __TAURI__ globals. */
function emitTauri(eventName, payload) {
  try {
    if (window.__TAURI__ && window.__TAURI__.event) {
      window.__TAURI__.event.emit(eventName, payload || null);
    }
  } catch (e) {
    console.warn('[4DA Notification] emit failed:', eventName, e);
  }
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

async function init() {
  try {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
      console.error('[4DA Notification] __TAURI__ globals not available');
      return;
    }

    var listen = window.__TAURI__.event.listen;

    // Listen for notification data pushed from Rust
    await listen('notification-data', function (event) {
      showNotification(event.payload);
    });

    // Signal readiness to Rust — safe to emit data now
    emitTauri('notification-ready');
    console.log('[4DA Notification] Ready');
  } catch (e) {
    console.error('[4DA Notification] Init failed:', e);
  }
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

// Pause auto-dismiss on hover
card.addEventListener('mouseenter', function () {
  isHovering = true;
  clearTimeout(dismissTimer);
});

card.addEventListener('mouseleave', function () {
  isHovering = false;
  startDismissTimer();
});

// Click card to navigate (but not if clicking dismiss button)
card.addEventListener('click', function (e) {
  if (e.target === dismissBtn || dismissBtn.contains(e.target)) return;
  emitTauri('notification-clicked', { item_id: currentItemId });
});

// Dismiss button
dismissBtn.addEventListener('click', function (e) {
  e.stopPropagation();
  hideNotification('dismiss');
});

// Keyboard accessibility
document.addEventListener('keydown', function (e) {
  if (e.key === 'Escape') {
    hideNotification('dismiss');
  } else if (e.key === 'Enter') {
    emitTauri('notification-clicked', { item_id: currentItemId });
  }
});

// Start
init();
