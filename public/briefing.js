// 4DA Intelligence Briefing — Desktop Widget
// Standalone vanilla JS — no React, no bundler dependency.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

var AUTO_DISMISS_MS = 60000;

// ---------------------------------------------------------------------------
// DOM references
// ---------------------------------------------------------------------------

var card = document.getElementById('card');
var gameLayer = document.getElementById('game-layer');
var briefingDate = document.getElementById('briefing-date');
var itemsList = document.getElementById('items-list');
var chainsSection = document.getElementById('chains-section');
var chainsList = document.getElementById('chains-list');
var wisdomSection = document.getElementById('wisdom-section');
var wisdomList = document.getElementById('wisdom-list');
var gapsSection = document.getElementById('gaps-section');
var gapsList = document.getElementById('gaps-list');
var ongoingSection = document.getElementById('ongoing-section');
var ongoingLine = document.getElementById('ongoing-line');
var totalCount = document.getElementById('total-count');
var dismissBtn = document.getElementById('dismiss-btn');
var openAppBtn = document.getElementById('open-app-btn');

var dismissTimer = null;
var isHovering = false;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function truncate(str, max) {
  if (!str) return '';
  return str.length > max ? str.slice(0, max) + '\u2026' : str;
}

function formatScore(score) {
  if (score == null) return '';
  return Math.round(score * 100) + '%';
}

/** Extract date string from title like "4DA Intelligence Briefing — 02 Apr 2026" */
function parseDateFromTitle(title) {
  if (!title) return '';
  var parts = title.split('\u2014'); // em dash
  if (parts.length < 2) parts = title.split('—');
  if (parts.length < 2) parts = title.split('-');
  return (parts[parts.length - 1] || '').trim();
}

// ---------------------------------------------------------------------------
// Tauri IPC
// ---------------------------------------------------------------------------

function emitTauri(eventName, payload) {
  try {
    if (window.__TAURI__ && window.__TAURI__.event) {
      window.__TAURI__.event.emit(eventName, payload || null);
    }
  } catch (e) {
    console.warn('[4DA Briefing] emit failed:', eventName, e);
  }
}

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

function buildItemHtml(item) {
  var priority = escapeHtml(item.signal_priority || 'watch');
  var title = escapeHtml(truncate(item.title, 80));
  var desc = escapeHtml(truncate(item.description, 200));
  var sourceType = escapeHtml(item.source_type || '');
  var score = formatScore(item.score);
  var itemId = item.item_id != null ? item.item_id : '';
  var url = item.url || '';

  var depsHtml = '';
  if (item.matched_deps && item.matched_deps.length > 0) {
    depsHtml = '<div class="item-deps">';
    for (var i = 0; i < Math.min(item.matched_deps.length, 3); i++) {
      depsHtml += '<span class="dep-pill">' + escapeHtml(item.matched_deps[i]) + '</span>';
    }
    depsHtml += '</div>';
  }

  return '<div class="briefing-item priority-' + priority + '" data-item-id="' + itemId + '" data-url="' + escapeHtml(url) + '">'
    + '<div class="item-header">'
    + '<span class="priority-dot ' + priority + '"></span>'
    + '<span class="item-title">' + title + '</span>'
    + '</div>'
    + (desc ? '<div class="item-description">' + desc + '</div>' : '')
    + '<div class="item-meta">'
    + '<span class="item-source">' + sourceType + '</span>'
    + (score ? '<span class="item-score">' + score + '</span>' : '')
    + depsHtml
    + '</div>'
    + '</div>';
}

function buildWisdomHtml(signals) {
  var html = '';
  for (var i = 0; i < signals.length; i++) {
    var signal = signals[i];
    var text = escapeHtml(signal.text || '');
    var confidence = signal.confidence != null ? Math.round(signal.confidence * 100) : 0;
    var isAntiPattern = signal.signal_type === 'anti-pattern';
    var typeClass = isAntiPattern ? 'wisdom-anti-pattern' : 'wisdom-principle';
    var typeLabel = isAntiPattern ? 'AVOID' : 'VALIDATED';

    html += '<div class="wisdom-row ' + typeClass + '">'
      + '<div class="wisdom-meta">'
      + '<span class="wisdom-type-badge">' + typeLabel + '</span>'
      + '<span class="wisdom-confidence">' + confidence + '%</span>'
      + '</div>'
      + '<div class="wisdom-text">' + text + '</div>'
      + '</div>';
  }
  return html;
}

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

function buildChainsHtml(chains) {
  var html = '';
  for (var i = 0; i < chains.length; i++) {
    var chain = chains[i];
    var name = escapeHtml(truncate(chain.name, 60));
    var phase = escapeHtml(chain.phase || 'active');
    var links = chain.link_count || 0;
    var action = escapeHtml(truncate(chain.action, 120));
    var conf = chain.confidence != null ? Math.round(chain.confidence * 100) + '%' : '';
    html += '<div class="chain-row">'
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

function renderBriefing(data) {
  // Header date
  briefingDate.textContent = parseDateFromTitle(data.title);

  // Escalating chains
  if (data.escalating_chains && data.escalating_chains.length > 0) {
    chainsSection.style.display = '';
    chainsList.innerHTML = buildChainsHtml(data.escalating_chains);
  } else {
    chainsSection.style.display = 'none';
  }

  // AWE Wisdom signals
  if (data.wisdom_signals && data.wisdom_signals.length > 0) {
    wisdomSection.style.display = '';
    wisdomList.innerHTML = buildWisdomHtml(data.wisdom_signals);
  } else {
    wisdomSection.style.display = 'none';
  }

  // Signal items
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
    ongoingLine.innerHTML = '<span class="ongoing-label">Tracking:</span> ' + topics;
  } else {
    ongoingSection.style.display = 'none';
  }

  // Footer
  if (data.total_relevant != null) {
    totalCount.textContent = data.total_relevant + ' signal' + (data.total_relevant !== 1 ? 's' : '') + ' today';
  } else {
    totalCount.textContent = '';
  }
}

// ---------------------------------------------------------------------------
// Show / Hide
// ---------------------------------------------------------------------------

function showBriefing(data) {
  renderBriefing(data);
  card.classList.remove('visible', 'exiting');
  requestAnimationFrame(function () {
    card.classList.add('visible');
    gameLayer.classList.add('active');
  });
  startDismissTimer();
}

function startDismissTimer() {
  clearTimeout(dismissTimer);
  dismissTimer = setTimeout(function () {
    if (!isHovering) {
      hideBriefing();
    }
  }, AUTO_DISMISS_MS);
}

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

card.addEventListener('mouseenter', function () {
  isHovering = true;
  clearTimeout(dismissTimer);
});

card.addEventListener('mouseleave', function () {
  isHovering = false;
  startDismissTimer();
});

dismissBtn.addEventListener('click', function (e) {
  e.stopPropagation();
  hideBriefing();
});

// Open 4DA button
openAppBtn.addEventListener('click', function (e) {
  e.stopPropagation();
  invokeTauri('briefing_item_clicked', {});
});

// Click outside the card to dismiss
document.addEventListener('click', function (e) {
  if (!card.contains(e.target)) {
    hideBriefing();
  }
});

// Item click delegation
itemsList.addEventListener('click', function (e) {
  var item = e.target.closest('.briefing-item');
  if (!item) return;

  e.preventDefault();
  e.stopPropagation();

  // Try opening URL first
  var url = item.getAttribute('data-url');
  if (url) {
    invokeTauri('briefing_open_url', { url: url });
    return;
  }

  // Fall back to navigating in the main app
  var itemId = item.getAttribute('data-item-id');
  if (itemId) {
    var parsed = parseInt(itemId, 10);
    if (!isNaN(parsed)) {
      invokeTauri('briefing_item_clicked', { item_id: parsed });
      emitTauri('briefing-item-clicked', { item_id: parsed });
    }
  }
});

// Keyboard
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

    await listen('briefing-data', function (event) {
      showBriefing(event.payload);
    });

    emitTauri('briefing-ready');
    console.log('[4DA Briefing] Ready');
  } catch (e) {
    console.error('[4DA Briefing] Init failed:', e);
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
