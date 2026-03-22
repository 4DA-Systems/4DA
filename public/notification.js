// 4DA Custom Notification Window
// Standalone vanilla JS — no React, no bundler dependency.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

const DISMISS_DURATIONS = {
  critical: 8000,
  high: 6000,
  medium: 5000,
  low: 4000,
};

// ---------------------------------------------------------------------------
// DOM references
// ---------------------------------------------------------------------------

const card = document.getElementById('card');
const gameLayer = document.getElementById('game-layer');
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
// GAME atmosphere management
// ---------------------------------------------------------------------------

/** Map priority to GAME WebComponent tag name. */
var GAME_TAGS = {
  critical: 'game-notif-critical',
  high: 'game-notif-high',
  medium: 'game-notif-medium',
  low: 'game-notif-digest',
};

var currentGameEl = null;

/** Swap the GAME atmosphere WebComponent in the game-layer div. */
function swapGameComponent(priority) {
  var tagName = GAME_TAGS[priority] || GAME_TAGS.low;

  // Skip if already showing the correct component
  if (currentGameEl && currentGameEl.tagName.toLowerCase() === tagName) return;

  // Remove current GAME element
  if (currentGameEl) {
    currentGameEl.remove();
    currentGameEl = null;
  }

  // Check if the custom element is defined (components loaded in HTML head)
  if (!customElements.get(tagName)) {
    // GAME component not available — CSS glow fallback handles visual
    return;
  }

  // Create and insert the GAME WebComponent
  currentGameEl = document.createElement(tagName);
  currentGameEl.style.cssText = 'width:100%;height:100%;display:block;';
  gameLayer.appendChild(currentGameEl);
}

// ---------------------------------------------------------------------------
// Content update
// ---------------------------------------------------------------------------

/** Populate the notification card DOM from a data payload. */
function updateContent(data) {
  currentPriority = data.priority || 'low';

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

  // GAME atmosphere opacity scales with priority
  var gameOpacity = { critical: 0.15, high: 0.12, medium: 0.08, low: 0.05 };
  gameLayer.style.setProperty(
    '--game-opacity',
    String(gameOpacity[currentPriority] || 0.05)
  );

  // Swap GAME atmosphere WebComponent based on priority
  swapGameComponent(currentPriority);

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
// Show / Hide
// ---------------------------------------------------------------------------

/** Show the notification with enter animation. */
function showNotification(data) {
  updateContent(data);

  // Reset animation state
  card.classList.remove('visible', 'exiting', 'dismissing');

  // Trigger enter animation on next frame so CSS transition fires
  requestAnimationFrame(function () {
    card.classList.add('visible');
    // GAME atmosphere fades in slightly after card appears
    setTimeout(function () {
      gameLayer.classList.add('active');
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
  gameLayer.classList.remove('active');

  // After animation completes, signal Rust to hide the window
  var delay = reason === 'dismiss' ? 150 : 400;
  setTimeout(function () {
    card.classList.remove('visible', 'exiting', 'dismissing');
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
  emitTauri('notification-clicked');
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
    emitTauri('notification-clicked');
  }
});

// Start
init();
