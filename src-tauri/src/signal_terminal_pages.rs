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
<div class="ep"><span class="method">GET</span><span class="path">/sw.js</span><div class="desc">Service worker for offline fallback</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/offline</span><div class="desc">Offline page shown when app is not running</div></div>

<h2>Data APIs (auth required)</h2>
<div class="ep"><span class="method">GET</span><span class="path">/api/boot</span><div class="desc">System boot data: status, signals, briefing in one request</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/status</span><div class="desc">System status: monitoring state, signal counts, threshold</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/signals</span><div class="desc">Top 50 relevant signals from latest analysis</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/briefing</span><div class="desc">Structured briefing: top 5 items with source diversity</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/score?url=...</span><div class="desc">Score a URL against current analysis results</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/radar</span><div class="desc">Tech radar: entries with ring, quadrant, movement</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/decisions</span><div class="desc">Active decision windows</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/dna</span><div class="desc">Developer DNA profile: stack, interests, stats</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/gaps</span><div class="desc">Knowledge gaps: stale dependencies, missed items</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/search?q=...</span><div class="desc">Full-text search across scored items</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/sources</span><div class="desc">Registered content sources and their status</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/stream</span><div class="desc">SSE live event stream (Server-Sent Events)</div></div>
<div class="ep"><span class="method">GET</span><span class="path">/api/simulate?add=X</span><div class="desc">Simulate adding/removing tech from your stack</div></div>
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
body{background:#030303;color:#D0D0D0;font:13px/1.6 'JetBrains Mono','SF Mono','Fira Code',monospace;padding:24px;display:flex;flex-direction:column;align-items:center;min-height:100vh}
.back{color:#D4AF37;text-decoration:none;font-size:12px;align-self:flex-start;margin-bottom:16px}
.card{border:1px solid #2A2A2A;border-radius:12px;background:linear-gradient(145deg,#0A0A0A 0%,#111 100%);padding:28px 32px;max-width:460px;width:100%;position:relative;overflow:hidden}
.card::before{content:'';position:absolute;top:0;left:0;right:0;height:3px;background:linear-gradient(90deg,#D4AF37,#B8962E,#D4AF37)}
.identity{margin-bottom:20px}
.identity h1{color:#fff;font-size:18px;font-weight:600;margin-bottom:6px;letter-spacing:0.5px}
.identity .summary{color:#777;font-size:11px;line-height:1.6}
.section{margin:16px 0}
.section-title{color:#D4AF37;font-size:10px;text-transform:uppercase;letter-spacing:2px;margin-bottom:8px;font-weight:600}
.badges{display:flex;flex-wrap:wrap;gap:6px}
.badge{background:#1A1A1A;border:1px solid #333;border-radius:20px;padding:4px 12px;font-size:11px;color:#E0E0E0;transition:border-color .2s}
.badge:hover{border-color:#D4AF37}
.stats-grid{display:grid;grid-template-columns:1fr 1fr;gap:8px}
.stat{background:#0D0D0D;border:1px solid #1A1A1A;border-radius:6px;padding:10px 12px}
.stat-val{color:#D4AF37;font-size:18px;font-weight:700;line-height:1}
.stat-label{color:#555;font-size:10px;text-transform:uppercase;letter-spacing:1px;margin-top:4px}
.actions{margin-top:20px;display:flex;gap:8px;justify-content:center}
.btn{border:none;padding:8px 18px;border-radius:6px;font:600 11px/1 'JetBrains Mono','SF Mono',monospace;cursor:pointer;transition:all .2s}
.btn-primary{background:#D4AF37;color:#050505}
.btn-primary:hover{background:#e8c44a;transform:translateY(-1px)}
.btn-secondary{background:#1A1A1A;color:#888;border:1px solid #333}
.btn-secondary:hover{border-color:#D4AF37;color:#D4AF37}
.brand{text-align:center;margin-top:20px;padding-top:16px;border-top:1px solid #1A1A1A}
.brand span{color:#333;font-size:9px;letter-spacing:2px;text-transform:uppercase}
.status{text-align:center;color:#888;padding:48px 0}
.err{color:#EF4444}
</style>
</head>
<body>
<a class="back" href="/">&larr; Terminal</a>
<div class="card" id="card"><div class="status">Loading DNA...</div></div>
<div class="actions" id="actions" style="display:none">
<button class="btn btn-primary" onclick="copyHtml()">Copy as HTML</button>
<button class="btn btn-secondary" onclick="copyText()">Copy as Text</button>
</div>
<script>
var dnaData=null;
(async()=>{
const C=document.getElementById('card');
const A=document.getElementById('actions');
try{
const tk=localStorage.getItem('4da-token')||'';
const r=await fetch('/api/dna',{headers:{'X-4DA-Token':tk}});
const d=await r.json();dnaData=d;
if(d.error){C.innerHTML='<div class="status err">'+d.error.replace(/</g,'&lt;')+'</div>';return}
var h='<div class="identity"><h1>'+(d.identity_summary||'Developer').replace(/</g,'&lt;')+'</h1></div>';
if(d.primary_stack&&d.primary_stack.length){h+='<div class="section"><div class="section-title">Primary Stack</div><div class="badges">';d.primary_stack.forEach(function(s){h+='<span class="badge">'+s.replace(/</g,'&lt;')+'</span>'});h+='</div></div>'}
if(d.stats){var s=d.stats;var rej=s.rejection_rate?Math.round(s.rejection_rate*100)+'%':'N/A';
h+='<div class="section"><div class="section-title">Intelligence Stats</div><div class="stats-grid">';
h+='<div class="stat"><div class="stat-val">'+(s.project_count||0)+'</div><div class="stat-label">Projects</div></div>';
h+='<div class="stat"><div class="stat-val">'+(s.dependency_count||0)+'</div><div class="stat-label">Dependencies</div></div>';
h+='<div class="stat"><div class="stat-val">'+rej+'</div><div class="stat-label">Rejection Rate</div></div>';
h+='<div class="stat"><div class="stat-val">'+(s.days_active||0)+'</div><div class="stat-label">Days Active</div></div>';
h+='</div></div>'}
if(d.interests&&d.interests.length){h+='<div class="section"><div class="section-title">Interests</div><div class="badges">';d.interests.forEach(function(s){h+='<span class="badge">'+s.replace(/</g,'&lt;')+'</span>'});h+='</div></div>'}
h+='<div class="brand"><span>Generated by 4DA Signal Terminal</span></div>';
C.innerHTML=h;A.style.display='flex';
}catch(e){C.innerHTML='<div class="status err">'+String(e).replace(/</g,'&lt;')+'</div>'}
})();
function copyHtml(){var el=document.getElementById('card');navigator.clipboard.writeText(el.outerHTML).then(function(){flash('btn-primary','Copied!')}).catch(function(){alert('Copy failed')})}
function copyText(){if(!dnaData)return;var lines=[];lines.push(dnaData.identity_summary||'Developer');lines.push('');
if(dnaData.primary_stack)lines.push('Stack: '+dnaData.primary_stack.join(', '));
if(dnaData.stats){var s=dnaData.stats;lines.push('Projects: '+(s.project_count||0)+' | Dependencies: '+(s.dependency_count||0));
lines.push('Rejection Rate: '+(s.rejection_rate?Math.round(s.rejection_rate*100)+'%':'N/A')+' | Days Active: '+(s.days_active||0))}
if(dnaData.interests)lines.push('Interests: '+dnaData.interests.join(', '));
lines.push('');lines.push('Generated by 4DA Signal Terminal');
navigator.clipboard.writeText(lines.join('\n')).then(function(){flash('btn-secondary','Copied!')}).catch(function(){alert('Copy failed')})}
function flash(cls,msg){var b=document.querySelector('.'+cls);var orig=b.textContent;b.textContent=msg;setTimeout(function(){b.textContent=orig},1500)}
</script>
</body>
</html>"#;

pub const PWA_MANIFEST: &str = r##"{"name":"4DA Signal Terminal","short_name":"4DA","start_url":"/","display":"standalone","background_color":"#050505","theme_color":"#D4AF37","icons":[{"src":"/icon","sizes":"192x192","type":"image/svg+xml"}]}"##;

pub const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 192 192"><rect width="192" height="192" rx="32" fill="#050505"/><text x="96" y="130" text-anchor="middle" fill="#D4AF37" font-family="monospace" font-size="120" font-weight="700">4</text></svg>"##;
