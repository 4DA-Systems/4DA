// 4DA Intelligence Briefing — Desktop Widget
// Standalone vanilla JS — no React, no bundler dependency.
// Uses window.__TAURI__ globals (withGlobalTauri: true in tauri.conf.json).

var AUTO_DISMISS_MS = 300000;

function cleanSynthesis(text) {
  if (!text) return text;
  return text
    .replace(/\[\d[\d, ]*\]/g, '')   // Strip citation brackets
    .replace(/\*\*/g, '')             // Strip markdown bold
    .replace(/^#+\s+/gm, '')         // Strip markdown headers
    .replace(/^[-*]\s+/gm, '')       // Strip bullet markers
    .replace(/\s*\([\d]+ items scanned[^)]*\)/g, '')  // Strip diagnostic parentheticals
    .replace(/  +/g, ' ')            // Collapse double spaces
    .trim();
}

// ---------------------------------------------------------------------------
// DOM references
// ---------------------------------------------------------------------------

var card = document.getElementById('card');
var atmosphereLayer = document.getElementById('atmosphere-layer');
var briefingDate = document.getElementById('briefing-date');
var itemsList = document.getElementById('items-list');
var chainsSection = document.getElementById('chains-section');
var chainsList = document.getElementById('chains-list');
var personalizationSection = document.getElementById('personalization-section');
var personalizationText = document.getElementById('personalization-text');
var synthesisSection = document.getElementById('synthesis-section');
var synthesisText = document.getElementById('synthesis-text');
var synthesisHintSection = document.getElementById('synthesis-hint-section');
var synthesisHintText = document.getElementById('synthesis-hint-text');
var synthesisProvenance = document.getElementById('synthesis-provenance');
var preemptionSection = document.getElementById('preemption-section');
var preemptionList = document.getElementById('preemption-list');
var freshnessSection = document.getElementById('freshness-section');
var freshnessLine = document.getElementById('freshness-line');
var stalenessBar = document.getElementById('staleness-bar');
var stalenessText = document.getElementById('staleness-text');
var dismissBtn = document.getElementById('dismiss-btn');
var openAppBtn = document.getElementById('open-app-btn');

var dismissTimer = null;
var isHovering = false;
var corroborationAvailable = false;

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
  return str.length > max ? str.slice(0, max) + '…' : str;
}

function friendlyHint(raw) {
  if (!raw) return '';
  var lower = raw.toLowerCase();
  if (lower.indexOf('401') !== -1 || lower.indexOf('authentication') !== -1 || lower.indexOf('invalid') !== -1 && lower.indexOf('key') !== -1) {
    return 'API key invalid or expired — check your cloud API key in Settings.';
  }
  if (lower.indexOf('no synthesis-capable') !== -1 || lower.indexOf('no llm configured') !== -1 || lower.indexOf('no cloud api') !== -1) {
    return 'Configure a cloud AI provider (Anthropic or OpenAI) in Settings to enable briefing synthesis.';
  }
  if (lower.indexOf('rate limit') !== -1 || lower.indexOf('429') !== -1) {
    return 'API rate limit reached — synthesis will retry on next briefing.';
  }
  if (lower.indexOf('timeout') !== -1 || lower.indexOf('timed out') !== -1) {
    return 'Provider took too long to respond — synthesis will retry on next briefing.';
  }
  return 'Intelligence synthesis unavailable — check cloud AI settings.';
}

function isAbstention(text) {
  if (!text) return true;
  var lower = text.toLowerCase();
  return lower.indexOf('low signal') === 0 || lower.indexOf('no noteworthy') !== -1;
}

/** True if the brief has any signal/alert/chain worth rendering (vs. a pure quiet state). */
function hasRenderableContent(data) {
  return !!((data.items && data.items.length > 0)
    || (data.preemption_alerts && data.preemption_alerts.length > 0)
    || (data.escalating_chains && data.escalating_chains.length > 0));
}

/** Extract date string from title like "4DA Intelligence Briefing — 02 Apr 2026" */
function parseDateFromTitle(title) {
  if (!title) return '';
  var parts = title.split('—'); // em dash
  if (parts.length < 2) parts = title.split('—');
  if (parts.length < 2) parts = title.split('-');
  return (parts[parts.length - 1] || '').trim();
}

/** Human-readable content type badge — prefers content_type over source_type */
function contentBadge(contentType, sourceType) {
  if (contentType) {
    var ctMap = {
      'security_advisory': 'security',
      'breaking_change': 'breaking',
      'release_notes': 'release',
      'deep_dive': 'deep dive',
      'curated_digest': 'digest',
      'expert_analysis': 'analysis',
      'platform_update': 'update',
      'tutorial': 'tutorial',
      'show_and_tell': 'project',
      'question': 'question',
      'help_request': 'help',
      'deprecation_notice': 'deprecated',
      'migration_guide': 'migration',
      'vulnerability_report': 'security',
      'discussion': 'discussion'
    };
    if (ctMap[contentType]) return ctMap[contentType];
  }
  var srcMap = {
    'arxiv': 'paper',
    'papers_with_code': 'paper',
    'hackernews': 'discussion',
    'reddit': 'discussion',
    'lobsters': 'discussion',
    'github': 'repo',
    'devto': 'article',
    'rss': 'feed',
    'producthunt': 'launch',
    'youtube': 'video',
    'twitter': 'post',
    'x': 'post'
  };
  return srcMap[sourceType] || sourceType || '';
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
  var title = escapeHtml(truncate(item.title, 90));
  var source = escapeHtml(contentBadge(item.content_type, item.source_type));
  var itemId = item.item_id != null ? item.item_id : '';
  var url = item.url || '';

  var depsHtml = '';
  if (item.matched_deps && item.matched_deps.length > 0) {
    for (var i = 0; i < Math.min(item.matched_deps.length, 3); i++) {
      depsHtml += '<span class="dep-pill">' + escapeHtml(item.matched_deps[i]) + '</span>';
    }
  }

  var corrobHtml = '';
  if (corroborationAvailable && item.corroboration_count && item.corroboration_count > 1) {
    corrobHtml = '<span class="corrob-badge">' + item.corroboration_count + ' sources</span>';
  }

  return '<div class="briefing-item" data-item-id="' + itemId + '" data-url="' + escapeHtml(url) + '">'
    + '<div class="item-row">'
    + '<span class="item-source-badge">' + source + '</span>'
    + corrobHtml
    + '<span class="item-title">' + title + '</span>'
    + '</div>'
    + (depsHtml ? '<div class="item-deps">' + depsHtml + '</div>' : '')
    + '</div>';
}

/** Humanize a minute count into "just now" / "Nm ago" / "Nh ago" / "Nd ago". */
function formatAgo(minutes) {
  if (minutes == null) return '';
  var m = Math.max(0, Math.round(minutes));
  if (m < 1) return 'just now';
  if (m < 60) return m + 'm ago';
  var h = Math.floor(m / 60);
  if (h < 24) return h + 'h ago';
  return Math.floor(h / 24) + 'd ago';
}

/**
 * Build the freshness line from data_freshness. Prefers the engine-run
 * receipt ("Scanned 421 · 20/20 sources · 4m ago"); falls back to the raw item
 * watermark ("Freshest item 4m ago") when no receipt exists — so the line always
 * shows real freshness even with the headless engine / external verifier disabled.
 */
function buildFreshnessLine(df) {
  if (!df) return '';
  if (df.last_run_items_scored != null && df.last_run_age_minutes != null) {
    var scanned = df.last_run_items_scored;
    var ok = df.last_run_sources_succeeded != null ? df.last_run_sources_succeeded : 0;
    var failed = df.last_run_sources_failed != null ? df.last_run_sources_failed : 0;
    var total = ok + failed;
    var srcPart = total > 0 ? (ok + '/' + total + ' sources') : (ok + ' sources');
    return 'Scanned ' + scanned + ' · ' + srcPart + ' · ' + formatAgo(df.last_run_age_minutes);
  }
  if (df.newest_item_age_hours != null) {
    return 'Freshest item ' + formatAgo(df.newest_item_age_hours * 60);
  }
  return '';
}

function renderFreshnessLine(df) {
  if (!freshnessSection || !freshnessLine) return;
  var text = buildFreshnessLine(df);
  if (text) {
    freshnessLine.textContent = text;
    freshnessSection.style.display = '';
  } else {
    freshnessSection.style.display = 'none';
  }
}

function buildChainsHtml(chains) {
  var html = '';
  for (var i = 0; i < chains.length; i++) {
    var chain = chains[i];
    var name = escapeHtml(truncate(chain.name, 60));
    var phase = escapeHtml(chain.phase || 'active');
    var links = chain.link_count || 0;
    var action = escapeHtml(truncate(chain.action, 120));
    html += '<div class="chain-row">'
      + '<span class="chain-phase">' + phase.toUpperCase() + '</span>'
      + '<span class="chain-name">' + name + '</span>'
      + '<span class="chain-links">' + links + ' signals</span>'
      + '</div>'
      + (action ? '<div class="chain-action">' + action + '</div>' : '');
  }
  return html;
}

function buildPreemptionHtml(alerts) {
  var html = '';
  for (var i = 0; i < alerts.length; i++) {
    var alert = alerts[i];
    var title = escapeHtml(truncate(alert.title, 70));
    var scope = alert.scope || 'external';
    var explanation = escapeHtml(truncate(alert.explanation, 160));

    // Show which project is affected for external/dev scope
    var context = '';
    if (scope !== 'primary' && alert.affected_projects && alert.affected_projects.length > 0) {
      var proj = alert.affected_projects[0];
      var projName = proj.split('/').pop() || proj.split('\\').pop() || proj;
      context = '<span class="preemption-scope">' + escapeHtml(projName) + '</span>';
    }

    html += '<div class="preemption-row scope-' + escapeHtml(scope) + '">'
      + '<div class="preemption-header">'
      + '<span class="preemption-title">' + title + '</span>'
      + context
      + '</div>'
      + (explanation ? '<div class="preemption-explanation">' + explanation + '</div>' : '')
      + '</div>';
  }
  return html;
}

function classifyItem(item) {
  var priority = (item.signal_priority || '').toLowerCase();
  var contentType = (item.content_type || '').toLowerCase();

  if (priority === 'critical' || priority === 'alert'
      || contentType === 'security_advisory' || contentType === 'vulnerability_report') {
    return 'review';
  }
  if (contentType === 'breaking_change' || contentType === 'deprecation_notice'
      || contentType === 'migration_guide') {
    return 'breaking';
  }
  return 'signals';
}

function renderBriefing(data) {
  // Reset provenance from any previous render
  if (synthesisProvenance) {
    synthesisProvenance.textContent = '';
    synthesisProvenance.style.display = 'none';
  }

  // Header date
  briefingDate.textContent = parseDateFromTitle(data.title);

  // Track whether corroboration detection ran (embeddings available)
  corroborationAvailable = !!data.corroboration_available;

  // Data staleness & source health indicator
  if (data.data_freshness) {
    var df = data.data_freshness;
    var ageHours = df.newest_item_age_hours;
    var failingSources = df.failing_sources || 0;
    var totalSources = df.total_sources || 0;
    var checksLast24h = df.source_checks_last_24h || 0;
    var failPct = totalSources > 0 ? Math.round(failingSources * 100 / totalSources) : 0;
    var showBar = false;
    var parts = [];
    var critical = false;

    if (df.is_stale || (ageHours && ageHours > 48)) {
      var ageDays = Math.floor((ageHours || 72) / 24);
      parts.push('Data ' + ageDays + 'd old');
      critical = true;
      showBar = true;
    } else if (ageHours && ageHours > 24) {
      parts.push('Data ' + Math.floor(ageHours) + 'h old');
      showBar = true;
    }

    if (failPct >= 50) {
      parts.push(failingSources + '/' + totalSources + ' sources degraded');
      showBar = true;
    } else if (failingSources > 5) {
      parts.push(failingSources + ' sources failing');
      showBar = true;
    }

    if (checksLast24h === 0 && totalSources > 0 && !showBar) {
      parts.push('No source checks in 24h');
      showBar = true;
    }

    if (showBar) {
      stalenessBar.style.display = '';
      stalenessBar.className = critical ? 'staleness-bar staleness-critical' : 'staleness-bar';
      stalenessText.textContent = parts.join(' · ');
    } else {
      stalenessBar.style.display = 'none';
      stalenessBar.className = 'staleness-bar';
    }
  } else {
    stalenessBar.style.display = 'none';
    stalenessBar.className = 'staleness-bar';
  }

  // i18n: apply translated labels from payload
  if (data.labels) {
    var labelMap = {
      'label-header': data.labels.header,
      'label-preemption': data.labels.preemption,
      'label-escalating': data.labels.escalating,
      'label-signals': data.labels.signals
    };
    for (var key in labelMap) {
      var el = document.getElementById(key);
      if (el && labelMap[key]) el.textContent = labelMap[key];
    }
  }

  // First-briefing personalization context line
  if (data.personalization_context) {
    personalizationSection.style.display = '';
    personalizationText.textContent = data.personalization_context;
  } else {
    personalizationSection.style.display = 'none';
  }

  // LLM Synthesis — the hero section. Gets the most real estate.
  // Abstention messages ("Low signal — no new intelligence overnight") are shown
  // as a muted single-line message so the user knows the system ran but had nothing new.
  if (data.synthesis && !isAbstention(data.synthesis)) {
    synthesisSection.style.display = '';
    synthesisSection.classList.remove('abstention', 'synthesizing');
    synthesisText.textContent = cleanSynthesis(data.synthesis);
    if (synthesisHintSection) synthesisHintSection.style.display = 'none';
  } else if (data.synthesis && isAbstention(data.synthesis)) {
    // Honest quiet state. When there ARE signals/alerts to show, a "nothing
    // noteworthy" line would contradict the list directly below it — so suppress
    // it and let the signals speak, with the freshness line as the proof the
    // system looked. Only when there is genuinely nothing do we show a warm
    // collapse line.
    if (hasRenderableContent(data)) {
      synthesisSection.style.display = 'none';
      synthesisSection.classList.remove('abstention', 'synthesizing');
    } else {
      synthesisSection.style.display = '';
      synthesisSection.classList.remove('synthesizing');
      synthesisSection.classList.add('abstention');
      synthesisText.textContent = 'All quiet — nothing crosses your bar this morning.';
    }
    if (synthesisHintSection) synthesisHintSection.style.display = 'none';
  } else if (data.synthesis_hint) {
    // No cloud API configured — show honest empty state
    synthesisSection.style.display = 'none';
    if (synthesisHintSection && synthesisHintText) {
      synthesisHintSection.style.display = '';
      synthesisHintText.textContent = friendlyHint(data.synthesis_hint);
    }
  } else {
    // No synthesis in initial data — show loading indicator
    // (synthesis may arrive async from startup/manual trigger paths)
    synthesisSection.style.display = '';
    synthesisSection.classList.add('synthesizing');
    synthesisText.textContent = 'Synthesizing intelligence…';
  }

  // Preemption alerts
  if (data.preemption_alerts && data.preemption_alerts.length > 0) {
    preemptionSection.style.display = '';
    preemptionList.innerHTML = buildPreemptionHtml(data.preemption_alerts);
  } else {
    preemptionSection.style.display = 'none';
  }

  // Escalating chains
  if (data.escalating_chains && data.escalating_chains.length > 0) {
    chainsSection.style.display = '';
    chainsList.innerHTML = buildChainsHtml(data.escalating_chains);
  } else {
    chainsSection.style.display = 'none';
  }

  // Action-oriented item sections
  if (!data.items || data.items.length === 0) {
    itemsList.innerHTML = '';
    document.getElementById('items-section').style.display = 'none';
  } else {
    var review = [];
    var breaking = [];
    var signals = [];
    for (var i = 0; i < data.items.length; i++) {
      var cat = classifyItem(data.items[i]);
      if (cat === 'review') review.push(data.items[i]);
      else if (cat === 'breaking') breaking.push(data.items[i]);
      else signals.push(data.items[i]);
    }

    var html = '';
    if (review.length > 0) {
      html += '<div class="action-group"><div class="action-group-label urgency-critical">REVIEW</div>';
      for (var r = 0; r < review.length; r++) html += buildItemHtml(review[r]);
      html += '</div>';
    }
    if (breaking.length > 0) {
      html += '<div class="action-group"><div class="action-group-label urgency-high">BREAKING CHANGES</div>';
      for (var b = 0; b < breaking.length; b++) html += buildItemHtml(breaking[b]);
      html += '</div>';
    }
    if (signals.length > 0) {
      html += '<div class="action-group"><div class="action-group-label">SIGNALS</div>';
      for (var s = 0; s < signals.length; s++) html += buildItemHtml(signals[s]);
      html += '</div>';
    }

    document.getElementById('items-section').style.display = html ? '' : 'none';
    itemsList.innerHTML = html;
  }

  // Freshness line — always present, on every brief. The visible daily proof
  // that the data is live, and the trust anchor for a quiet brief.
  renderFreshnessLine(data.data_freshness);
}

// ---------------------------------------------------------------------------
// Show / Hide
// ---------------------------------------------------------------------------

function showBriefing(data) {
  var hasItems = data.items && data.items.length > 0;
  var hasPreemption = data.preemption_alerts && data.preemption_alerts.length > 0;
  var hasChains = data.escalating_chains && data.escalating_chains.length > 0;
  if (!hasItems && !hasPreemption && !hasChains && isAbstention(data.synthesis)) {
    return;
  }
  renderBriefing(data);
  card.classList.remove('visible', 'exiting');
  requestAnimationFrame(function () {
    card.classList.add('visible');
    atmosphereLayer.classList.add('active');
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
  atmosphereLayer.classList.remove('active');
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

// Dismiss only via the X button — clicking outside should not dismiss,
// because tools like Lightshot require clicking the desktop first.

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

    // Async synthesis arrives after initial briefing
    await listen('briefing-synthesis', function (event) {
      if (!event.payload || !synthesisSection || !synthesisText) return;
      synthesisSection.classList.remove('synthesizing');
      if (isAbstention(event.payload)) {
        // Quiet day. The initial briefing-data already rendered any signals; a
        // late "nothing noteworthy" line would only contradict them — so hide it
        // and let the signals + freshness line stand as the brief.
        synthesisSection.classList.remove('abstention');
        synthesisSection.style.display = 'none';
      } else {
        synthesisSection.classList.remove('abstention');
        synthesisSection.style.display = '';
        synthesisText.textContent = cleanSynthesis(event.payload);
      }
      // Hide hint since synthesis arrived
      if (synthesisHintSection) synthesisHintSection.style.display = 'none';
    });

    // Synthesis provenance metadata — consumed for logging only, never shown
    await listen('briefing-synthesis-meta', function () {});

    // Synthesis unavailable hint
    await listen('briefing-synthesis-hint', function (event) {
      if (synthesisSection) {
        synthesisSection.classList.remove('synthesizing');
        synthesisSection.style.display = 'none';
      }
      if (event.payload && synthesisHintSection && synthesisHintText) {
        synthesisHintSection.style.display = '';
        synthesisHintText.textContent = friendlyHint(event.payload);
      }
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
