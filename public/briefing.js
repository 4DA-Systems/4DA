// 4DA Intelligence Briefing Window
// Standalone vanilla JS — no React, no bundler dependency.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

var AUTO_DISMISS_MS = 60000;

// ---------------------------------------------------------------------------
// DOM references
// ---------------------------------------------------------------------------

var card = document.getElementById('card');
var gameLayer = document.getElementById('game-layer');
var briefingTitle = document.getElementById('briefing-title');
var itemsList = document.getElementById('items-list');
var chainsSection = document.getElementById('chains-section');
var chainsList = document.getElementById('chains-list');
var gapsSection = document.getElementById('gaps-section');
var gapsList = document.getElementById('gaps-list');
var ongoingSection = document.getElementById('ongoing-section');
var ongoingLine = document.getElementById('ongoing-line');
var totalCount = document.getElementById('total-count');
var dismissBtn = document.getElementById('dismiss-btn');

var dismissTimer = null;
var isHovering = false;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Minimal HTML entity escaping for user-supplied text. */
function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/** Truncate a string to `max` characters, appending ellipsis if needed. */
function truncate(str, max) {
  if (!str) return '';
  return str.length > max ? str.slice(0, max) + '\u2026' : str;
}

/** Format score as percentage string. */
function formatScore(score) {
  if (score == null) return '';
  return Math.round(score * 100) + '%';
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
    console.warn('[4DA Briefing] emit failed:', eventName, e);
  }
}

/** Invoke a Tauri command. */
function invokeTauri(command, args) {
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      return window.__TAURI__.core.invoke(command, args || {});
    }
  } catch (e) {
    console.warn('[4DA Briefing] invoke failed:', command, e);
  }
  return Promise.resolve();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/** Build the HTML for a single briefing item. */
function buildItemHtml(item) {
  var priority = escapeHtml(item.signal_priority || 'watch');
  var signalType = escapeHtml((item.signal_type || 'signal').replace(/_/g, ' '));
  var sourceType = escapeHtml(item.source_type || '');
  var score = formatScore(item.score);
  var desc = escapeHtml(truncate(item.description, 200));
  var title = escapeHtml(truncate(item.title, 80));
  var itemId = item.item_id != null ? item.item_id : '';
  var url = item.url || '';

  // Title: link if URL exists, plain text otherwise
  var titleHtml;
  if (url) {
    titleHtml = '<a class="item-title-link" href="#" data-url="' + escapeHtml(url) + '">' + title + '</a>';
  } else {
    titleHtml = '<span>' + title + '</span>';
  }

  // Matched deps pills
  var depsHtml = '';
  if (item.matched_deps && item.matched_deps.length > 0) {
    depsHtml = '<div class="item-deps">';
    for (var i = 0; i < item.matched_deps.length; i++) {
      depsHtml += '<span class="dep-pill">' + escapeHtml(item.matched_deps[i]) + '</span>';
    }
    depsHtml += '</div>';
  }

  // Description row
  var descHtml = desc ? '<div class="item-description">' + desc + '</div>' : '';

  return '<div class="briefing-item priority-' + priority + '" data-item-id="' + itemId + '">'
    + '<div class="item-top-row">'
    + '<span class="priority-dot ' + priority + '"></span>'
    + '<span class="signal-badge">' + signalType + '</span>'
    + '<span class="item-title">' + titleHtml + '</span>'
    + '<span class="source-badge">' + sourceType + '</span>'
    + (score ? '<span class="item-score">' + score + '</span>' : '')
    + '</div>'
    + descHtml
    + depsHtml
    + '</div>';
}

/** Build knowledge gaps HTML. */
function buildGapsHtml(gaps) {
  var html = '';
  for (var i = 0; i < gaps.length; i++) {
    var gap = gaps[i];
    var topic = escapeHtml(gap.topic || '');
    var days = gap.days_since_last != null ? gap.days_since_last : '?';
    html += '<div class="gap-row">'
      + '<span class="gap-topic">' + topic + '</span>'
      + '<span class="gap-days">' + days + 'd since last signal</span>'
      + '</div>';
  }
  return html;
}

/** Build escalating chain HTML. */
function buildChainsHtml(chains) {
  var html = '';
  for (var i = 0; i < chains.length; i++) {
    var chain = chains[i];
    var name = escapeHtml(truncate(chain.name, 60));
    var phase = escapeHtml(chain.phase || 'active');
    var links = chain.link_count || 0;
    var action = escapeHtml(truncate(chain.action, 120));
    var conf = chain.confidence != null ? Math.round(chain.confidence * 100) + '%' : '';
    html += '<div class="chain-row priority-' + phase + '">'
      + '<span class="priority-dot ' + (phase === 'peak' ? 'critical' : 'alert') + '"></span>'
      + '<span class="chain-name">' + name + '</span>'
      + '<span class="chain-phase">' + phase.toUpperCase() + '</span>'
      + '<span class="chain-links">' + links + ' signals</span>'
      + (conf ? '<span class="chain-confidence">' + conf + '</span>' : '')
      + '</div>'
      + (action ? '<div class="chain-action">' + action + '</div>' : '');
  }
  return html;
}

/** Render the full briefing from the data payload. */
function renderBriefing(data) {
  // Header title
  briefingTitle.textContent = data.title || '4DA Intelligence Briefing';

  // Escalating chains (top-level, before items)
  if (data.escalating_chains && data.escalating_chains.length > 0) {
    chainsSection.style.display = '';
    chainsList.innerHTML = buildChainsHtml(data.escalating_chains);
  } else {
    chainsSection.style.display = 'none';
  }

  // Items
  if (!data.items || data.items.length === 0) {
    itemsList.innerHTML = '<div class="empty-state">Your stack is quiet. Nothing new.</div>';
  } else {
    var html = '';
    for (var i = 0; i < data.items.length; i++) {
      html += buildItemHtml(data.items[i]);
    }
    itemsList.innerHTML = html;
  }

  // Knowledge gaps
  if (data.knowledge_gaps && data.knowledge_gaps.length > 0) {
    gapsSection.style.display = '';
    gapsList.innerHTML = buildGapsHtml(data.knowledge_gaps);
  } else {
    gapsSection.style.display = 'none';
  }

  // Ongoing topics
  if (data.ongoing_topics && data.ongoing_topics.length > 0) {
    ongoingSection.style.display = '';
    var topics = data.ongoing_topics.map(function (t) { return escapeHtml(t); }).join(', ');
    ongoingLine.innerHTML = '<span class="ongoing-label">Ongoing:</span> ' + topics;
  } else {
    ongoingSection.style.display = 'none';
  }

  // Footer total count
  if (data.total_relevant != null) {
    totalCount.textContent = data.total_relevant + ' relevant item' + (data.total_relevant !== 1 ? 's' : '') + ' today';
  } else {
    totalCount.textContent = '';
  }
}

// ---------------------------------------------------------------------------
// Show / Hide
// ---------------------------------------------------------------------------

/** Show the briefing card with enter animation. */
function showBriefing(data) {
  renderBriefing(data);

  // Reset animation state
  card.classList.remove('visible', 'exiting');

  // Trigger enter animation on next frame
  requestAnimationFrame(function () {
    card.classList.add('visible');
    gameLayer.classList.add('active');
  });

  startDismissTimer();
}

/** Start or restart the auto-dismiss countdown. */
function startDismissTimer() {
  clearTimeout(dismissTimer);
  dismissTimer = setTimeout(function () {
    if (!isHovering) {
      hideBriefing();
    }
  }, AUTO_DISMISS_MS);
}

/** Hide the briefing with exit animation. */
function hideBriefing() {
  clearTimeout(dismissTimer);
  card.classList.add('exiting');
  gameLayer.classList.remove('active');

  setTimeout(function () {
    card.classList.remove('visible', 'exiting');
    emitTauri('briefing-hidden');
  }, 250);
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

// Dismiss button
dismissBtn.addEventListener('click', function (e) {
  e.stopPropagation();
  hideBriefing();
});

// Click outside the card to dismiss
document.addEventListener('click', function (e) {
  if (!card.contains(e.target)) {
    hideBriefing();
  }
});

// Delegate click handlers within the items list
itemsList.addEventListener('click', function (e) {
  // Check if clicking a title link
  var link = e.target.closest('.item-title-link');
  if (link) {
    e.preventDefault();
    e.stopPropagation();
    var url = link.getAttribute('data-url');
    if (url) {
      invokeTauri('briefing_open_url', { url: url });
    }
    return;
  }

  // Otherwise, clicking the item body area
  var item = e.target.closest('.briefing-item');
  if (item) {
    var itemId = item.getAttribute('data-item-id');
    if (itemId) {
      var parsed = parseInt(itemId, 10);
      if (!isNaN(parsed)) {
        invokeTauri('briefing_item_clicked', { item_id: parsed });
        emitTauri('briefing-item-clicked', { item_id: parsed });
      }
    }
  }
});

// Keyboard accessibility
document.addEventListener('keydown', function (e) {
  if (e.key === 'Escape') {
    hideBriefing();
  }
});

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

async function init() {
  try {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
      console.error('[4DA Briefing] __TAURI__ globals not available');
      return;
    }

    var listen = window.__TAURI__.event.listen;

    // Listen for briefing data pushed from Rust
    await listen('briefing-data', function (event) {
      showBriefing(event.payload);
    });

    // Signal readiness to Rust
    emitTauri('briefing-ready');
    console.log('[4DA Briefing] Ready');
  } catch (e) {
    console.error('[4DA Briefing] Init failed:', e);
  }
}

// Start on DOMContentLoaded (script is type="module", so deferred by default)
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
