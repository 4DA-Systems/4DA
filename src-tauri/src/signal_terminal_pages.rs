// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

//! Static HTML pages for the Signal Terminal (Phase 2 & 3).
//! Kept separate from signal_terminal.rs to respect file size limits.

pub const SETUP_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>4DA Signal Terminal — Setup</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#050505;color:#D0D0D0;font:14px/1.6 'JetBrains Mono',monospace;padding:24px;max-width:640px;margin:0 auto}
h1{color:#D4AF37;font-size:18px;margin-bottom:24px;border-bottom:1px solid #333;padding-bottom:12px}
h2{color:#D4AF37;font-size:14px;margin:24px 0 12px;text-transform:uppercase;letter-spacing:1px}
p,.note{color:#888;font-size:12px;margin:8px 0}
.note{background:#0D0D0D;border:1px solid #333;border-radius:4px;padding:8px 12px}
.bm-link{display:inline-block;background:#D4AF37;color:#050505;padding:10px 20px;border-radius:4px;text-decoration:none;font-weight:700;font-size:14px;cursor:grab;margin:12px 0}
.bm-link:hover{background:#e8c44a}
code{background:#1A1A1A;padding:2px 6px;border-radius:3px;font-size:12px;color:#D4AF37;word-break:break-all}
.token-box{background:#0D0D0D;border:1px solid #D4AF37;border-radius:4px;padding:12px;margin:12px 0;font-size:13px;word-break:break-all}
.back{color:#D4AF37;text-decoration:none;font-size:12px;display:inline-block;margin-bottom:16px}
</style>
</head>
<body>
<a class="back" href="/">&larr; Terminal</a>
<h1>4DA Signal Terminal — Setup</h1>

<h2>Score This — Bookmarklet</h2>
<p>Drag this button to your bookmarks bar. Click it on any page to score it against your 4DA profile.</p>
<a class="bm-link" href="javascript:void(window.open('http://localhost:4445/score-popup?url='+encodeURIComponent(location.href)+'&title='+encodeURIComponent(document.title),'4da','width=420,height=500'))">Score This</a>
<p class="note">Default port is <code>4445</code> (dev) or <code>4444</code> (production). Edit the bookmarklet URL if your port differs.</p>

<h2>LAN Access</h2>
<div class="token-box" id="token-display">Token: loading...</div>
<p>To access from another device on your network, use your machine's IP address with the port and set the <code>X-4DA-Token</code> header for API calls.</p>

<h2>PWA Installation</h2>
<p>On mobile browsers, tap the share/menu icon and select <strong>"Add to Home Screen"</strong>. The terminal will launch as a standalone app.</p>
<p class="note">Requires the terminal to be accessible from the device (see LAN Access above).</p>

<script>
const p=new URLSearchParams(location.search);
const t=p.get('token');
if(t)document.getElementById('token-display').textContent='Token: '+t;
else document.getElementById('token-display').textContent='Token: (open this page from the terminal to see your token)';
</script>
</body>
</html>"#;

pub const SCORE_POPUP_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>4DA — Score</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#050505;color:#D0D0D0;font:13px/1.5 'JetBrains Mono',monospace;padding:16px}
h1{color:#D4AF37;font-size:15px;margin-bottom:12px}
.title{font-size:12px;color:#888;margin-bottom:16px;word-break:break-word}
.bar-row{display:flex;align-items:center;gap:8px;margin:6px 0}
.bar-label{width:90px;font-size:11px;color:#888;text-align:right;flex-shrink:0}
.bar-track{flex:1;height:14px;background:#1A1A1A;border-radius:3px;overflow:hidden}
.bar-fill{height:100%;background:#D4AF37;border-radius:3px;transition:width .6s ease}
.bar-val{width:36px;font-size:11px;color:#D4AF37;flex-shrink:0}
.score-big{font-size:32px;color:#D4AF37;font-weight:700;text-align:center;margin:16px 0}
.status{text-align:center;color:#888;font-size:12px;margin:16px 0}
.err{color:#EF4444}
.not-found{color:#888;text-align:center;margin:24px 0;font-size:13px}
</style>
</head>
<body>
<h1>4DA Score</h1>
<div class="title" id="pg-title"></div>
<div id="result"><div class="status">Scoring...</div></div>
<script>
const p=new URLSearchParams(location.search);
const url=p.get('url')||'';
const title=p.get('title')||url;
document.getElementById('pg-title').textContent=title;
const R=document.getElementById('result');
(async()=>{
try{
const tk=localStorage.getItem('4da-token')||'';
const r=await fetch('/api/score?url='+encodeURIComponent(url),{headers:{'X-4DA-Token':tk}});
const d=await r.json();
if(!d.found){R.innerHTML='<div class="not-found">URL not in current analysis.<br>Run an analysis first.</div>';return}
let h='<div class="score-big">'+Math.round((d.score||0)*100)+'%</div>';
const b=d.breakdown;
if(b){const bars=[['Context',b.context_score],['Interest',b.interest_score],['Keyword',b.keyword_score],['ACE Boost',b.ace_boost],['Freshness',b.freshness_mult],['Quality',b.content_quality_mult],['Novelty',b.novelty_mult],['Dep Match',b.dep_match_score]];
bars.forEach(([l,v])=>{const pct=Math.round((v||0)*100);h+='<div class="bar-row"><span class="bar-label">'+l+'</span><div class="bar-track"><div class="bar-fill" style="width:'+pct+'%"></div></div><span class="bar-val">'+pct+'%</span></div>';})}
if(d.explanation)h+='<div class="status">'+d.explanation.replace(/</g,'&lt;')+'</div>';
R.innerHTML=h;
}catch(e){R.innerHTML='<div class="status err">Error: '+String(e).replace(/</g,'&lt;')+'</div>'}
})();
</script>
</body>
</html>"#;

pub const API_DOCS_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>4DA Signal Terminal — API</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#050505;color:#D0D0D0;font:13px/1.6 'JetBrains Mono',monospace;padding:24px;max-width:720px;margin:0 auto}
h1{color:#D4AF37;font-size:18px;margin-bottom:8px}
.sub{color:#888;font-size:12px;margin-bottom:24px}
h2{color:#D4AF37;font-size:13px;margin:20px 0 8px;text-transform:uppercase;letter-spacing:1px}
.ep{background:#0D0D0D;border:1px solid #333;border-radius:4px;padding:10px 14px;margin:8px 0}
.method{color:#22C55E;font-weight:700;margin-right:8px}
.path{color:#D4AF37}
.desc{color:#888;font-size:12px;margin-top:4px}
.auth{color:#EF4444;font-size:11px}
.back{color:#D4AF37;text-decoration:none;font-size:12px;display:inline-block;margin-bottom:16px}
</style>
</head>
<body>
<a class="back" href="/">&larr; Terminal</a>
<h1>API Reference</h1>
<p class="sub">All <code>/api/*</code> routes require <code>X-4DA-Token</code> header. <span class="auth">401 if missing/invalid.</span></p>

<h2>Pages (no auth)</h2>
<div class="ep"><span class="method">GET</span><span class="path">/</span><div class="desc">Terminal UI</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/setup</span><div class="desc">Setup &amp; bookmarklet generator</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/score-popup</span><div class="desc">Score popup (used by bookmarklet). Params: <code>url</code>, <code>title</code></div></div>
<div class="ep"><span class="method">GET</span><span class="path">/card</span><div class="desc">Intelligence Card (shareable developer profile)</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/manifest.json</span><div class="desc">PWA manifest</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/icon</span><div class="desc">SVG icon (192x192)</div></div>

<h2>Data APIs (auth required)</h2>
<div class="ep"><span class="method">GET</span><span class="path">/api/status</span><div class="desc">System status: monitoring state, signal counts, threshold</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/signals</span><div class="desc">Top 50 relevant signals from latest analysis</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/briefing</span><div class="desc">Structured briefing: top 5 items with source diversity</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/score?url=...</span><div class="desc">Score a URL against current analysis results</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/radar</span><div class="desc">Tech radar: entries with ring, quadrant, movement</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/decisions</span><div class="desc">Active decision windows</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/dna</span><div class="desc">Developer DNA profile: stack, interests, stats</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/gaps</span><div class="desc">Knowledge gaps: stale dependencies, missed items</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/search?q=...</span><div class="desc">Full-text search across scored items</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/docs</span><div class="desc">This page</div></div>
</body>
</html>"#;

pub const CARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>4DA — Intelligence Card</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#050505;color:#D0D0D0;font:13px/1.6 'JetBrains Mono',monospace;padding:24px;display:flex;flex-direction:column;align-items:center}
.card{border:2px solid #D4AF37;border-radius:8px;background:#0A0A0A;padding:24px;max-width:420px;width:100%}
.card h1{color:#D4AF37;font-size:16px;margin-bottom:4px}
.card .summary{color:#888;font-size:12px;margin-bottom:16px}
.section{margin:12px 0}
.section-title{color:#D4AF37;font-size:11px;text-transform:uppercase;letter-spacing:1px;margin-bottom:6px}
.tag{display:inline-block;background:#1A1A1A;border:1px solid #333;border-radius:3px;padding:2px 8px;margin:2px;font-size:11px;color:#D0D0D0}
.stat-row{display:flex;justify-content:space-between;font-size:12px;padding:3px 0;border-bottom:1px solid #1A1A1A}
.stat-label{color:#888}
.stat-val{color:#D4AF37}
.actions{margin-top:16px;text-align:center}
.btn{background:#D4AF37;color:#050505;border:none;padding:8px 16px;border-radius:4px;font:700 12px/1 'JetBrains Mono',monospace;cursor:pointer}
.btn:hover{background:#e8c44a}
.status{text-align:center;color:#888;padding:48px 0}
.err{color:#EF4444}
.back{color:#D4AF37;text-decoration:none;font-size:12px;align-self:flex-start;margin-bottom:16px}
</style>
</head>
<body>
<a class="back" href="/">&larr; Terminal</a>
<div class="card" id="card"><div class="status">Loading DNA...</div></div>
<div class="actions" id="actions" style="display:none"><button class="btn" onclick="copyCard()">Copy as HTML</button></div>
<script>
(async()=>{
const C=document.getElementById('card');
const A=document.getElementById('actions');
try{
const tk=localStorage.getItem('4da-token')||'';
const r=await fetch('/api/dna',{headers:{'X-4DA-Token':tk}});
const d=await r.json();
if(d.error){C.innerHTML='<div class="status err">'+d.error.replace(/</g,'&lt;')+'</div>';return}
let h='<h1>Intelligence Card</h1>';
h+='<div class="summary">'+(d.identity_summary||'No profile yet').replace(/</g,'&lt;')+'</div>';
if(d.primary_stack&&d.primary_stack.length){h+='<div class="section"><div class="section-title">Stack</div>';d.primary_stack.forEach(s=>{h+='<span class="tag">'+s.replace(/</g,'&lt;')+'</span>'});h+='</div>'}
if(d.interests&&d.interests.length){h+='<div class="section"><div class="section-title">Interests</div>';d.interests.forEach(s=>{h+='<span class="tag">'+s.replace(/</g,'&lt;')+'</span>'});h+='</div>'}
if(d.stats){const s=d.stats;h+='<div class="section"><div class="section-title">Stats</div>';
h+='<div class="stat-row"><span class="stat-label">Items processed</span><span class="stat-val">'+(s.total_items_processed||0)+'</span></div>';
h+='<div class="stat-row"><span class="stat-label">Relevant</span><span class="stat-val">'+(s.total_relevant||0)+'</span></div>';
h+='<div class="stat-row"><span class="stat-label">Projects</span><span class="stat-val">'+(s.project_count||0)+'</span></div>';
h+='<div class="stat-row"><span class="stat-label">Dependencies</span><span class="stat-val">'+(s.dependency_count||0)+'</span></div>';
h+='<div class="stat-row"><span class="stat-label">Days active</span><span class="stat-val">'+(s.days_active||0)+'</span></div>';
h+='</div>'}
C.innerHTML=h;A.style.display='block';
}catch(e){C.innerHTML='<div class="status err">'+String(e).replace(/</g,'&lt;')+'</div>'}
})();
function copyCard(){const el=document.getElementById('card');const html=el.outerHTML;navigator.clipboard.writeText(html).then(()=>{const b=document.querySelector('.btn');b.textContent='Copied!';setTimeout(()=>b.textContent='Copy as HTML',1500)}).catch(()=>alert('Copy failed'))}
</script>
</body>
</html>"#;

pub const PWA_MANIFEST: &str = r##"{"name":"4DA Signal Terminal","short_name":"4DA","start_url":"/","display":"standalone","background_color":"#050505","theme_color":"#D4AF37","icons":[{"src":"/icon","sizes":"192x192","type":"image/svg+xml"}]}"##;

pub const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 192 192"><rect width="192" height="192" rx="32" fill="#050505"/><text x="96" y="130" text-anchor="middle" fill="#D4AF37" font-family="monospace" font-size="120" font-weight="700">4</text></svg>"##;
