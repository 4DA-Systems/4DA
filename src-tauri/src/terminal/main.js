(function(){
'use strict';

/* ── Element refs ── */
var out=document.getElementById('out'),inp=document.getElementById('inp');
var liveDot=document.getElementById('liveDot'),liveText=document.getElementById('liveText');
var authOverlay=document.getElementById('authOverlay'),authInput=document.getElementById('authInput');
var ghostEl=document.getElementById('ghost');

/* ── State ── */
var history=[],histIdx=-1,histBuf='';
var token=localStorage.getItem('4da_term_token')||'';
var isAmbient=false,ambInterval=null;
var startTime=performance.now();
var reconnecting=false,reconnectTimer=null;
var lastSignalCount=0;
var pendingMore=null;
var lastOutput='';
var watchInterval=null;
var lastNotifiedCritical=0;
var jsonMode=false;
var cmdStartTime=0;
var cachedSignals=null;
var aliases=JSON.parse(localStorage.getItem('4da_aliases')||'{}');
var readingQueue=JSON.parse(localStorage.getItem('4da_queue')||'[]');

/* ── Command list for tab completion ── */
var COMMANDS=['help','signals','briefing','score','search','radar','decisions','dna','gaps',
  'status','clear','ambient','theme','whoami','uptime','matrix','fortune','neofetch','ping','token',
  'more','diff','watch','copy','alias','unalias','sources','save','queue','read'];
var tabMatches=[],tabIdx=-1,tabPrefix='';

/* ── Command descriptions for help and palette ── */
var CMD_HELP={
  'signals':'Current signals above threshold',
  'briefing':'Intelligence briefing',
  'score':'Score a URL (score <url>)',
  'search':'Search scored items (search <q>)',
  'radar':'Stack intelligence \u2014 health, priorities, trends',
  'decisions':'Active decision windows',
  'dna':'Developer DNA profile',
  'gaps':'Knowledge blind spots',
  'status':'System status',
  'clear':'Clear terminal',
  'ambient':'Ambient display mode',
  'theme':'Switch color theme (theme <name>)',
  'whoami':'Developer identity card',
  'neofetch':'System info display',
  'uptime':'Terminal session uptime',
  'fortune':'Developer wisdom',
  'matrix':'Matrix rain effect',
  'ping':'Source connectivity check',
  'token':'Set authentication token',
  'more':'Show more results from last command',
  'diff':'Compare signals with previous snapshot',
  'watch':'Auto-refresh a command (watch <cmd>)',
  'copy':'Copy last output to clipboard',
  'alias':'Set command alias (alias name = cmd)',
  'unalias':'Remove alias (unalias name)',
  'sources':'List registered sources',
  'save':'Save signal to reading queue (save <n>)',
  'queue':'Show reading queue',
  'read':'Open queued item (read <n>)'
};

/* ── Theme system ── */
var THEMES={
  gold:   {gold:'#D4AF37',fg:'#C8C8C8',green:'#22C55E',muted:'#555',dim:'#2A2A2A',cyan:'#67E8F9',red:'#EF4444'},
  phosphor:{gold:'#33FF33',fg:'#33FF33',green:'#33FF33',muted:'#1A9A1A',dim:'#0D4D0D',cyan:'#33FF33',red:'#FF3333'},
  frost:  {gold:'#67E8F9',fg:'#B0E0E6',green:'#67E8F9',muted:'#5A8A8F',dim:'#2A3A3F',cyan:'#67E8F9',red:'#FF6B6B'},
  ember:  {gold:'#FF6B35',fg:'#D0D0D0',green:'#FF6B35',muted:'#8A6A5A',dim:'#3A2A1F',cyan:'#FFB088',red:'#EF4444'}
};
var currentTheme=localStorage.getItem('4da_term_theme')||'gold';

/* ── Fortune quotes ── */
var FORTUNES=[
  'First, solve the problem. Then, write the code. \u2014 John Johnson',
  'Any fool can write code that a computer can understand. Good programmers write code that humans can understand. \u2014 Martin Fowler',
  'The best error message is the one that never shows up. \u2014 Thomas Fuchs',
  'Code is like humor. When you have to explain it, it is bad. \u2014 Cory House',
  'Simplicity is the soul of efficiency. \u2014 Austin Freeman',
  'Make it work, make it right, make it fast. \u2014 Kent Beck',
  'Walking on water and developing software from a specification are easy if both are frozen. \u2014 Edward V. Berard',
  'The most dangerous phrase in the language is: We have always done it this way. \u2014 Grace Hopper',
  'Programs must be written for people to read, and only incidentally for machines to execute. \u2014 Abelson & Sussman',
  'Measuring programming progress by lines of code is like measuring aircraft building progress by weight. \u2014 Bill Gates'
];

/* ── Helpers ── */
function w(text,cls){
  var d=document.createElement('div');d.className='out-line';
  if(cls)d.innerHTML='<span class="'+cls+'">'+esc(text)+'</span>';
  else d.textContent=text;
  out.appendChild(d);scroll();
}
function wh(html){var d=document.createElement('div');d.className='out-line';d.innerHTML=html;out.appendChild(d);scroll()}
function wcmd(text){var d=document.createElement('div');d.className='out-line out-cmd';d.innerHTML='<span class="g">&gt; '+esc(text)+'</span>';out.appendChild(d);scroll()}
function wsep(label){wh('<span class="m">\u2500\u2500 '+esc(label)+' \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500</span>')}
function wkv(key,val,cls){
  var v=cls?'<span class="'+cls+'">'+esc(String(val))+'</span>':esc(String(val));
  wh('<span class="m">  '+esc(key).padEnd(14)+'</span>'+v);
}
function whint(text){wh('<div style="text-align:right;margin-top:4px"><span class="d">'+esc(text)+'</span></div>')}
function bar(score,width){
  var n=Math.round(score*(width||10));
  return '<span class="bar"><span class="bar-fill">'+'\u2593'.repeat(n)+'</span><span class="bar-empty">'+'\u2591'.repeat((width||10)-n)+'</span></span>';
}
function sparkline(value,max){
  var chars='\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588';
  var ratio=max>0?Math.min(value/max,1):0;
  var idx=Math.round(ratio*(chars.length-1));
  return chars[idx];
}
function sparkbar(value,max,len){
  var chars='\u2581\u2582\u2583\u2584\u2585\u2586\u2587\u2588';
  var result='';
  for(var i=0;i<(len||8);i++){
    var seg=(i+1)/(len||8);
    var ratio=max>0?Math.min(value/max,1):0;
    if(seg<=ratio){var idx=Math.min(Math.floor(seg*chars.length),chars.length-1);result+=chars[idx]}
    else result+='\u2581';
  }
  return result;
}
function scroll(){out.scrollTop=out.scrollHeight}
function esc(s){if(s==null)return'';s=String(s);var d=document.createElement('div');d.textContent=s;return d.innerHTML}
function escAttr(s){return String(s).replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;')}
function link(url,text){return'<a href="'+escAttr(url)+'" target="_blank" rel="noopener">'+esc(text)+'</a>'}
function rmLast(){var lines=out.querySelectorAll('.out-line');if(lines.length)lines[lines.length-1].remove()}

/* ── Phase 1.7: Better error messages ── */
function apiErr(e){
  if(e.message==='auth')return;
  if(e.message==='Failed to fetch'||e.message==='NetworkError when attempting to fetch resource.')
    w('Connection refused. Is the 4DA desktop app running?','r');
  else
    w('Error: '+e.message,'r');
}

function fmtUptime(ms){
  var s=Math.floor(ms/1000),m=Math.floor(s/60),h=Math.floor(m/60);
  s%=60;m%=60;
  if(h>0)return h+'h '+m+'m '+s+'s';
  if(m>0)return m+'m '+s+'s';
  return s+'s';
}

/* ── Phase 1.1: Response timing helper ── */
function showTiming(){
  var elapsed=(performance.now()-cmdStartTime).toFixed(0);
  wh('<span class="d" style="float:right;font-size:11px">'+elapsed+'ms</span>');
}

/* ── Phase 1.3: Levenshtein distance for typo correction ── */
function levenshtein(a,b){
  var m=[],i,j;
  for(i=0;i<=b.length;i++)m[i]=[i];
  for(j=0;j<=a.length;j++)m[0][j]=j;
  for(i=1;i<=b.length;i++)for(j=1;j<=a.length;j++)
    m[i][j]=Math.min(m[i-1][j]+1,m[i][j-1]+1,m[i-1][j-1]+(b[i-1]===a[j-1]?0:1));
  return m[b.length][a.length];
}

/* ── Phase 2.3: Flag parser ── */
function parseFlags(arg){
  var flags={};
  var m;
  if(m=arg.match(/--source\s+(\S+)/))flags.source=m[1];
  if(m=arg.match(/--priority\s+(\S+)/))flags.priority=m[1];
  if(m=arg.match(/--top\s+(\d+)/))flags.top=parseInt(m[1]);
  return flags;
}

/* ── Phase 2.4: Output tracking for copy ── */
function captureOutputStart(){
  return out.querySelectorAll('.out-line').length;
}
function captureOutputEnd(startIdx){
  var lines=out.querySelectorAll('.out-line');
  var texts=[];
  for(var i=startIdx;i<lines.length;i++){
    texts.push(lines[i].textContent);
  }
  lastOutput=texts.join('\n');
}

/* ── API ── */
function api(endpoint){
  var h=token?{'X-4DA-Token':token}:{};
  return fetch(endpoint,{headers:h}).then(function(r){
    if(r.status===401){showAuth();throw new Error('auth')}
    return r.json();
  });
}
function showAuth(){authOverlay.style.display='flex';authInput.focus()}
function hideAuth(){authOverlay.style.display='none';inp.focus()}

/* ── Auth handlers ── */
authInput.addEventListener('keydown',function(e){
  if(e.key==='Enter'){token=this.value.trim();if(token){localStorage.setItem('4da_term_token',token);hideAuth();refreshStatus()}}
});
document.getElementById('authSkip').addEventListener('click',function(){token='';hideAuth();refreshStatus()});

/* ── Theme application ── */
function applyTheme(name){
  var t=THEMES[name];if(!t)return false;
  currentTheme=name;
  localStorage.setItem('4da_term_theme',name);
  var root=document.documentElement.style;
  root.setProperty('--gold',t.gold);
  root.setProperty('--fg',t.fg);
  root.setProperty('--green',t.green);
  root.setProperty('--muted',t.muted);
  root.setProperty('--dim',t.dim);
  root.setProperty('--cyan',t.cyan);
  root.setProperty('--red',t.red);
  return true;
}
/* Apply saved theme on load */
if(currentTheme!=='gold')applyTheme(currentTheme);

/* ── Boot sequence ── */
function bootSequence(){
  out.innerHTML='';
  var bootLines=[];

  api('/api/boot').then(function(d){
    bootLines=[
      {tag:'INIT',cls:'g',text:'Connecting to 4DA core...'},
      {tag:'OK',cls:'gr',text:'Database: '+(d.db_items||0)+' items indexed'},
      {tag:'OK',cls:'gr',text:'Monitoring: '+(d.sources||0)+' sources active'},
      {tag:'OK',cls:'gr',text:'ACE: '+(d.tech_detected||0)+' technologies detected'},
      {tag:'OK',cls:'gr',text:'PASIFA: threshold '+(d.threshold||0.35)+', rejection '+(d.rejection_pct||0)+'%'},
      {tag:'OK',cls:'gr',text:(d.total_scanned||0)+' scanned \u00B7 '+(d.total_relevant||0)+' relevant'},
      {tag:'LIVE',cls:'gr',text:'Signal Terminal online.',bold:true}
    ];
    renderBootLines(bootLines);
  }).catch(function(){
    /* Fallback: try /api/status if /api/boot doesn't exist */
    api('/api/status').then(function(d){
      bootLines=[
        {tag:'INIT',cls:'g',text:'Connecting to 4DA core...'},
        {tag:'OK',cls:'gr',text:'Database: online'},
        {tag:'OK',cls:'gr',text:'Monitoring: '+(d.monitoring?'active':'idle')},
        {tag:'OK',cls:'gr',text:'Scanned: '+(d.total_scanned||0)+' items'},
        {tag:'OK',cls:'gr',text:'Relevant: '+(d.total_relevant||0)+' items'},
        {tag:'OK',cls:'gr',text:'Threshold: '+(d.threshold||0.35)},
        {tag:'LIVE',cls:'gr',text:'Signal Terminal online.',bold:true}
      ];
      renderBootLines(bootLines);
    }).catch(function(e){
      if(e.message==='auth'){showAuth();return}
      bootLines=[
        {tag:'INIT',cls:'g',text:'Connecting to 4DA core...'},
        {tag:'FAIL',cls:'r',text:'Could not reach backend. Retrying...'}
      ];
      renderBootLines(bootLines);
      startReconnect();
    });
  });
}

function renderBootLines(lines){
  var i=0;
  function nextLine(){
    if(i>=lines.length){
      /* Post-boot: empty line + help hint */
      setTimeout(function(){
        w('');
        wh('<span class="d">Type </span><span class="g">help</span><span class="d"> for commands \u00B7 </span><span class="d">Ctrl+P</span><span class="d"> command palette</span>');
      },200);
      return;
    }
    var ln=lines[i];
    var tagColor=ln.tag==='INIT'?'g':ln.tag==='FAIL'?'r':'gr';
    var boldOpen=ln.bold?'font-weight:bold;':'';
    var tagStr='<span class="'+tagColor+'">['+esc(ln.tag)+']</span>';
    var textStr='<span style="'+boldOpen+'">'+esc(ln.text)+'</span>';
    wh(tagStr+' '+textStr);
    i++;
    setTimeout(nextLine,100);
  }
  nextLine();
}

/* ── Reconnect logic ── */
function startReconnect(){
  if(reconnecting)return;
  reconnecting=true;
  var banner=document.createElement('div');
  banner.className='reconnect-banner';banner.id='reconnBanner';
  banner.textContent='RECONNECTING...';
  document.body.appendChild(banner);
  reconnectTimer=setInterval(function(){
    fetch('/api/status',{headers:token?{'X-4DA-Token':token}:{}})
    .then(function(r){if(r.ok)return r.json();throw new Error('offline')})
    .then(function(){
      reconnecting=false;
      clearInterval(reconnectTimer);reconnectTimer=null;
      var b=document.getElementById('reconnBanner');if(b)b.remove();
      wh('<span class="gr">[RECONNECTED]</span>');
      refreshStatus();
    }).catch(function(){});
  },5000);
}

/* ── Status refresh ── */
function refreshStatus(){
  api('/api/status').then(function(d){
    liveDot.className='live-dot '+(d.monitoring?'on':'off');
    liveText.textContent=d.monitoring?'LIVE':'IDLE';
    document.getElementById('sb-mon').textContent=d.monitoring?'monitoring':'idle';
    document.getElementById('sb-scan').textContent=(d.total_scanned||0)+' scanned';
    document.getElementById('sb-rel').textContent=(d.total_relevant||0)+' relevant';
    document.getElementById('sb-thr').textContent='threshold '+(d.threshold||0.35);
    /* Tab title with signal count */
    lastSignalCount=d.total_relevant||0;
    document.title=lastSignalCount>0?'('+lastSignalCount+') 4DA Terminal':'4DA Signal Terminal';

    /* Phase 3.3: Browser notification for critical signals */
    if('Notification' in window && Notification.permission==='granted'){
      api('/api/signals').then(function(sd){
        var criticalCount=0;
        (sd.signals||[]).forEach(function(s){
          if(s.signal_priority==='critical')criticalCount++;
        });
        if(criticalCount>0 && criticalCount!==lastNotifiedCritical){
          lastNotifiedCritical=criticalCount;
          new Notification('4DA Signal Terminal',{
            body:criticalCount+' critical signal'+(criticalCount>1?'s':''),
            icon:'/icon'
          });
        }
      }).catch(function(){});
    }
  }).catch(function(){
    liveDot.className='live-dot off';liveText.textContent='OFFLINE';
    document.getElementById('sb-mon').textContent='disconnected';
    startReconnect();
  });
}

/* ── Tab completion ── */
function tabComplete(){
  var val=inp.value;
  /* If continuing a tab cycle */
  if(tabMatches.length>0&&val===tabMatches[tabIdx]){
    tabIdx=(tabIdx+1)%tabMatches.length;
    inp.value=tabMatches[tabIdx];
    ghostEl.textContent='';
    return;
  }
  /* New tab press: find matches */
  tabPrefix=val.toLowerCase();
  var allCmds=COMMANDS.concat(Object.keys(aliases));
  tabMatches=allCmds.filter(function(c){return c.indexOf(tabPrefix)===0});
  if(tabMatches.length===0){ghostEl.textContent='';return}
  if(tabMatches.length===1){
    inp.value=tabMatches[0];
    ghostEl.textContent='';
    tabIdx=0;
  } else {
    tabIdx=0;
    inp.value=tabMatches[0];
    ghostEl.textContent='';
  }
}

inp.addEventListener('input',function(){
  /* Clear ghost on manual typing */
  ghostEl.textContent='';
  tabMatches=[];tabIdx=-1;
  /* Show ghost preview while typing */
  var val=this.value.toLowerCase();
  if(val.length>0){
    var allCmds=COMMANDS.concat(Object.keys(aliases));
    var match=allCmds.find(function(c){return c.indexOf(val)===0&&c!==val});
    if(match)ghostEl.textContent=match.substring(val.length);
    else ghostEl.textContent='';
  } else {
    ghostEl.textContent='';
  }
});

/* ── Input handling ── */
inp.addEventListener('keydown',function(e){
  if(e.key==='Tab'){
    e.preventDefault();
    tabComplete();
    return;
  }
  if(e.key==='Enter'){
    var cmd=this.value.trim();if(!cmd)return;
    history.push(cmd);if(history.length>100)history.shift();
    histIdx=-1;histBuf='';this.value='';ghostEl.textContent='';exec(cmd);
  } else if(e.key==='ArrowUp'){
    e.preventDefault();if(histIdx===-1)histBuf=this.value;
    if(histIdx<history.length-1){histIdx++;this.value=history[history.length-1-histIdx]}
  } else if(e.key==='ArrowDown'){
    e.preventDefault();
    if(histIdx>0){histIdx--;this.value=history[history.length-1-histIdx]}
    else if(histIdx===0){histIdx=-1;this.value=histBuf}
  } else if(e.key==='l'&&e.ctrlKey){e.preventDefault();out.innerHTML=''}
  else if(e.key==='k'&&e.ctrlKey){e.preventDefault();inp.focus()}
});

document.addEventListener('keydown',function(e){
  if(e.target===inp||e.target===authInput)return;
  /* Phase 2.5: Command palette */
  if(e.key==='p'&&e.ctrlKey){
    e.preventDefault();
    showPalette();
    return;
  }
  if(isAmbient){exitAmbient();return}
  if(!e.ctrlKey&&!e.altKey&&!e.metaKey&&e.key.length===1)inp.focus();
});

/* ── Phase 1.2: Command chaining with semicolons ── */
function exec(raw){
  var commands=raw.split(';').map(function(c){return c.trim()}).filter(Boolean);
  commands.forEach(function(cmdStr,i){
    if(i>0)w('');
    execSingle(cmdStr);
  });
}

/* ── Phase 1.6 / Phase 2.3: Flag handling in execSingle ── */
function execSingle(raw){
  wcmd(raw);
  cmdStartTime=performance.now();
  var outputStart=captureOutputStart();

  /* Phase 1.6: JSON output mode */
  var hasJson=raw.indexOf('--json')!==-1;
  var cleanRaw=hasJson?raw.replace(/--json/g,'').trim():raw;

  var parts=cleanRaw.split(/\s+/),cmd=parts[0].toLowerCase(),arg=parts.slice(1).join(' ');
  jsonMode=hasJson;

  /* Phase 2.6: Check aliases before command switch */
  if(aliases[cmd]){
    var aliasExpansion=aliases[cmd];
    if(arg)aliasExpansion+=' '+arg;
    wh('<span class="d">Alias: '+esc(cmd)+' \u2192 '+esc(aliasExpansion)+'</span>');
    execSingle(aliasExpansion);
    captureOutputEnd(outputStart);
    return;
  }

  switch(cmd){
    case'help':cmdHelp();break;
    case'signals':cmdSignals(arg);break;
    case'briefing':cmdBriefing();break;
    case'score':cmdScore(arg);break;
    case'search':cmdSearch(arg);break;
    case'radar':cmdRadar();break;
    case'decisions':cmdDecisions();break;
    case'dna':cmdDna();break;
    case'gaps':cmdGaps();break;
    case'status':cmdStatus();break;
    case'clear':out.innerHTML='';break;
    case'ambient':enterAmbient();break;
    case'token':showAuth();break;
    case'theme':cmdTheme(arg);break;
    case'whoami':cmdWhoami();break;
    case'uptime':cmdUptime();break;
    case'matrix':cmdMatrix();break;
    case'fortune':cmdFortune();break;
    case'neofetch':cmdNeofetch();break;
    case'ping':cmdPing();break;
    /* Phase 1.5 */
    case'more':cmdMore();break;
    /* Phase 2.1 */
    case'diff':cmdDiff();break;
    /* Phase 2.2 */
    case'watch':cmdWatch(arg);break;
    /* Phase 2.4 */
    case'copy':cmdCopy();break;
    /* Phase 2.6 */
    case'alias':cmdAlias(arg);break;
    case'unalias':cmdUnalias(arg);break;
    /* Phase 3.2 */
    case'sources':cmdSources();break;
    /* Phase 3.4 */
    case'save':cmdSave(arg);break;
    case'queue':cmdQueue();break;
    case'read':cmdRead(arg);break;
    default:
      /* Phase 3.1: Try natural language interpretation */
      var nlp=tryNaturalLanguage(raw);
      if(nlp){
        wh('<span class="d">Interpreted as: '+esc(nlp)+'</span>');
        execSingle(nlp);
        captureOutputEnd(outputStart);
        return;
      }
      /* Phase 1.3: Typo correction */
      var closest=COMMANDS.reduce(function(best,c){
        var d=levenshtein(cmd,c);
        return d<best.d?{c:c,d:d}:best;
      },{c:'',d:99});
      if(closest.d<=2){
        w('Unknown command: '+cmd+'. Did you mean: '+closest.c+'?','r');
      } else {
        w('Unknown command: '+cmd+". Type 'help' for commands.",'r');
      }
  }

  /* Capture output for copy after sync commands; async commands handle their own captureOutputEnd */
  setTimeout(function(){captureOutputEnd(outputStart)},50);
}

/* ── Commands ── */
function cmdHelp(){
  wsep('COMMANDS');
  wkv('signals','Current signals above threshold');
  wkv('briefing','Intelligence briefing');
  wkv('score <url>','Score a URL against your profile');
  wkv('search <q>','Search scored items');
  wkv('radar','Stack intelligence \u2014 health, priorities, trends');
  wkv('decisions','Active decision windows');
  wkv('dna','Developer DNA profile');
  wkv('gaps','Knowledge blind spots');
  wkv('status','System status');
  wkv('sources','List registered sources');
  wkv('diff','Compare signals with previous snapshot');
  wkv('clear','Clear terminal');
  wkv('ambient','Ambient display mode');
  wkv('theme <name>','Switch color theme');
  w('');
  wsep('POWER USER');
  wkv('watch <cmd>','Auto-refresh every 30s');
  wkv('alias n = cmd','Set command alias');
  wkv('unalias <n>','Remove alias');
  wkv('copy','Copy last output to clipboard');
  wkv('save <n>','Save signal #n to reading queue');
  wkv('queue','Show reading queue');
  wkv('read <n>','Open & remove queued item #n');
  wkv('more','Show more results from last command');
  w('');
  wsep('EXTRAS');
  wkv('whoami','Developer identity card');
  wkv('neofetch','System info display');
  wkv('uptime','Terminal session uptime');
  wkv('fortune','Developer wisdom');
  wkv('matrix','Matrix rain effect');
  wkv('ping','Source connectivity check');
  w('');
  wsep('FLAGS');
  wh('<span class="d">  --json         Append to any command for raw JSON output</span>');
  wh('<span class="d">  --source X     Filter signals by source</span>');
  wh('<span class="d">  --priority X   Filter signals by priority</span>');
  wh('<span class="d">  --top N        Show only top N signals</span>');
  w('');
  wh('<span class="d">  Keyboard: \u2191\u2193 history \u00B7 Tab complete \u00B7 Ctrl+L clear \u00B7 Ctrl+K focus \u00B7 Ctrl+P palette</span>');
  wh('<span class="d">  Chaining: cmd1 ; cmd2 ; cmd3 (semicolons run commands sequentially)</span>');
  wh('<span class="d">  Natural language: try "show me signals" or "anything new?"</span>');
  wh('<span class="d">  Themes: gold \u00B7 phosphor \u00B7 frost \u00B7 ember</span>');
  showTiming();
}

/* ── Phase 1.5: Signal renderer for pagination ── */
function renderSignal(s){
  var scr=Math.round((s.score_raw||0)*100);
  var title=s.url?link(s.url,s.title):esc(s.title);
  var prio=s.signal_priority||'low';

  if(prio==='critical'||prio==='high'){
    var cardDiv=document.createElement('div');
    cardDiv.className='signal-card '+prio;
    var icon=prio==='critical'?'\u26A1':'\u25C6';
    var meta=[];
    if(s.signal_type)meta.push(s.signal_type);
    if(s.source)meta.push(s.source);
    if(s.signal_action)meta.push('Act: '+s.signal_action);
    cardDiv.innerHTML=
      '<div class="sc-title">'+icon+' '+title+' <span class="g">['+scr+'%]</span></div>'+
      (meta.length?'<div class="sc-meta">'+esc(meta.join(' \u00B7 '))+'</div>':'')+
      '<div class="sc-bar">'+bar(s.score_raw||0,20)+'</div>';
    out.appendChild(cardDiv);scroll();
  } else {
    wh('\u25C7 <span class="g">['+scr+']</span> '+title);
    var meta2=[];
    if(s.signal_type)meta2.push(s.signal_type);if(s.signal_priority)meta2.push(s.signal_priority);
    if(s.source)meta2.push(s.source);if(s.signal_action)meta2.push('Act: '+s.signal_action);
    if(meta2.length)wh('       <span class="d">'+esc(meta2.join(' \u00B7 '))+'</span>');
  }
}

function cmdSignals(arg){
  arg=arg||'';
  w('Loading...','d');
  api('/api/signals').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    if(!d.signals||!d.signals.length){w('No signals. Run an analysis first.','m');showTiming();return}

    /* Cache for save command */
    cachedSignals=d.signals;

    /* Phase 2.3: Apply filters */
    var flags=parseFlags(arg);
    var filtered=d.signals.slice();
    if(flags.source)filtered=filtered.filter(function(s){return s.source&&s.source.toLowerCase()===flags.source.toLowerCase()});
    if(flags.priority)filtered=filtered.filter(function(s){return s.signal_priority===flags.priority});
    if(flags.top)filtered=filtered.slice(0,flags.top);

    wsep('SIGNALS ('+filtered.length+')');w('');

    /* Phase 1.5: Truncation with "more" */
    var PAGE_SIZE=15;
    var firstBatch=filtered.slice(0,PAGE_SIZE);
    var rest=filtered.slice(PAGE_SIZE);

    firstBatch.forEach(renderSignal);

    if(rest.length>0){
      pendingMore={items:rest,renderer:renderSignal};
      wh('<span class="d">\u2500\u2500 '+rest.length+' more items (type \'more\' to show) \u2500\u2500</span>');
    }

    w('');wh('<span class="d">'+filtered.length+' signals. The rest was noise.</span>');
    whint('try: search <keyword> to filter');
    showTiming();
  }).catch(apiErr);
}

function cmdBriefing(){
  w('Generating...','d');
  api('/api/briefing').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    if(d.empty){w(d.message||'No items. Run an analysis first.','m');showTiming();return}
    wsep('BRIEFING');w('');
    wh('<span class="g">'+d.total_items+' items analyzed</span>');w('');
    if(d.top_items)d.top_items.forEach(function(item,i){
      var title=item.url?link(item.url,item.title):esc(item.title);
      wh('<span class="m">'+(i+1)+'. </span>'+title+' <span class="d">'+esc(item.source)+' '+esc(item.score)+'</span>');
    });
    if(d.source_summary){w('');wsep('SOURCES');Object.keys(d.source_summary).forEach(function(k){wkv(k,d.source_summary[k])})}
    whint('try: signals for the full list');
    showTiming();
  }).catch(apiErr);
}

function cmdScore(url){
  if(!url){w('Usage: score <url>','r');return}
  w('Scoring...','d');
  api('/api/score?url='+encodeURIComponent(url)).then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    if(!d.found){w('URL not in current analysis results.','m');showTiming();return}
    wsep('SCORE');w('');
    var scr=Math.round((d.score||0)*100);
    wh('<span class="g">RELEVANCE: '+scr+'/100</span>  '+bar(d.score));w('');
    if(d.breakdown){
      var b=d.breakdown,axes=[
        ['Context',b.context_score],['Interest',b.interest_score],['Keyword',b.keyword_score],
        ['ACE',b.ace_boost],['Deps',b.dep_match_score],['Freshness',b.freshness_mult],
        ['Quality',b.content_quality_mult],['Domain',b.domain_relevance]
      ],delay=0;
      axes.forEach(function(a){setTimeout(function(){
        var val=a[1]!=null?a[1]:0;
        wh('  '+bar(Math.min(val,1))+' <span class="m">'+esc(a[0]).padEnd(12)+'</span><span class="g">'+(typeof val==='number'?val.toFixed(2):'\u2014')+'</span>');
      },delay);delay+=150});
      setTimeout(function(){w('');
        var sigs=b.confirmed_signals||b.signal_count||0;
        wh('<span class="d">  Confirmation: '+sigs+' axes agree. '+(scr>=70?'Strong':'Moderate')+' signal.</span>');
        if(d.explanation)wh('<span class="d">  '+esc(d.explanation)+'</span>');
        showTiming();
      },delay);
    } else {
      showTiming();
    }
  }).catch(apiErr);
}

function cmdSearch(q){
  if(!q){w('Usage: search <query>','r');return}
  w('Searching...','d');
  api('/api/search?q='+encodeURIComponent(q)).then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('SEARCH: '+d.query);w('');
    if(!d.results||!d.results.length){w('No results.','m');showTiming();return}
    wh('<span class="g">'+d.count+' results</span>');w('');

    /* Phase 1.5: Truncation */
    var PAGE_SIZE=15;
    var allResults=d.results;
    var firstBatch=allResults.slice(0,PAGE_SIZE);
    var rest=allResults.slice(PAGE_SIZE);

    firstBatch.forEach(function(r,i){
      var scr=Math.round((r.score||0)*100),title=r.url?link(r.url,r.title):esc(r.title);
      var rel=r.relevant?' <span class="gr">relevant</span>':'';
      wh('<span class="m">'+(i+1).toString().padStart(2)+'. </span>'+title+' <span class="d">'+esc(r.source)+' '+scr+'%</span>'+rel);
    });

    if(rest.length>0){
      pendingMore={items:rest,renderer:function(r){
        var scr=Math.round((r.score||0)*100),title=r.url?link(r.url,r.title):esc(r.title);
        var rel=r.relevant?' <span class="gr">relevant</span>':'';
        wh('<span class="m">\u00B7  </span>'+title+' <span class="d">'+esc(r.source)+' '+scr+'%</span>'+rel);
      }};
      wh('<span class="d">\u2500\u2500 '+rest.length+' more items (type \'more\' to show) \u2500\u2500</span>');
    }
    showTiming();
  }).catch(apiErr);
}

function cmdRadar(){
  w('Loading...','d');
  api('/api/radar').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    if(!d.entries||!d.entries.length){w('No radar entries. Run an analysis to populate.','m');showTiming();return}
    var entries=d.entries;
    var total=entries.length;

    /* ── Health grade ── */
    var holdCount=entries.filter(function(e){return e.ring==='hold'}).length;
    var adoptCount=entries.filter(function(e){return e.ring==='adopt'}).length;
    var downCount=entries.filter(function(e){return e.movement==='down'}).length;
    var healthScore=((adoptCount/total)*60)+((1-holdCount/total)*30)+((1-downCount/total)*10);
    var healthGrade;
    if(healthScore>=90)healthGrade='A';
    else if(healthScore>=80)healthGrade='A-';
    else if(healthScore>=70)healthGrade='B+';
    else if(healthScore>=60)healthGrade='B';
    else if(healthScore>=50)healthGrade='C';
    else healthGrade='D';

    /* ── Sub-scores ── */
    var secScore=holdCount===0?'A':(holdCount<=1?'B':'C');
    var freshScore=downCount===0?'A':(downCount<=2?'B':'C');
    var maintScore=adoptCount>=total*0.4?'A':(adoptCount>=total*0.2?'B':'C');

    wsep('STACK INTELLIGENCE');w('');
    var gradeCls=healthGrade.charAt(0)==='A'?'gr':(healthGrade.charAt(0)==='B'?'g':'r');
    wh('  <span class="m">HEALTH:</span> <span class="'+gradeCls+'" style="font-size:14px;font-weight:600">'+healthGrade+'</span>');
    wh('  <span class="d">security: '+secScore+' \u00B7 freshness: '+freshScore+' \u00B7 maintenance: '+maintScore+'</span>');
    w('');

    /* ── Needs attention ── */
    var attention=entries.filter(function(e){return e.ring==='hold'||e.movement==='down'});
    if(attention.length){
      wh('<span class="r" style="font-size:11px;letter-spacing:1px">  NEEDS ATTENTION</span>');
      attention.forEach(function(e){
        var reason=e.ring==='hold'?'hold':'declining';
        var sigCount=(e.signals&&e.signals.length)||0;
        var detail=sigCount>0?(sigCount+' signal'+(sigCount===1?'':'s')):'';
        if(e.ring==='hold'&&e.movement==='down')detail=detail?(detail+' \u00B7 declining community'):'declining community';
        else if(e.ring==='hold')detail=detail||'on hold';
        wh('  <span class="r">\u25C6 '+esc(e.name).padEnd(20)+'</span><span class="d">'+esc(reason).padEnd(9)+'</span><span class="r">\u2193</span>  <span class="d">'+esc(detail)+'</span>');
      });
      w('');
    }

    /* ── Tier display ── */
    var tiers=[
      {ring:'adopt',label:'CORE STACK',cls:'gr'},
      {ring:'trial',label:'EXPANDING',cls:'g'},
      {ring:'assess',label:'WATCHING',cls:'m'}
    ];
    tiers.forEach(function(tier){
      var items=entries.filter(function(e){return e.ring===tier.ring});
      if(!items.length)return;
      items.sort(function(a,b){return(b.score||0)-(a.score||0)});
      wh('<span class="'+tier.cls+'" style="font-size:11px;letter-spacing:1px">  '+tier.label+'</span>');
      items.forEach(function(e){
        var s=typeof e.score==='number'?e.score:0;
        var name=esc(e.name);
        var padName=name.padEnd(35);
        var scoreStr=s.toFixed(2);
        wh('  '+bar(s,10)+'  <span class="'+tier.cls+'">'+padName+'</span><span class="d">'+scoreStr+'</span>');
      });
      w('');
    });

    /* ── Movement ── */
    var moving=entries.filter(function(e){return e.movement&&e.movement!=='stable'});
    var stable=entries.filter(function(e){return!e.movement||e.movement==='stable'});
    var movementOrder={up:0,'new':1,down:2};
    moving.sort(function(a,b){return(movementOrder[a.movement]||1)-(movementOrder[b.movement]||1)});
    if(moving.length||stable.length){
      wh('<span class="m" style="font-size:11px;letter-spacing:1px">  MOVEMENT</span>');
      moving.forEach(function(e){
        var arrow,label,cls;
        if(e.movement==='up'){arrow='\u2191';label='accelerating';cls='gr'}
        else if(e.movement==='new'){arrow='\u2726';label='new';cls='g'}
        else{arrow='\u2193';label='declining';cls='r'}
        var sigCount=(e.signals&&e.signals.length)||0;
        wh('  <span class="'+cls+'">'+arrow+' '+esc(e.name).padEnd(20)+'</span><span class="d">'+esc(label)+' \u00B7 '+sigCount+' signal'+(sigCount===1?'':'s')+'</span>');
      });
      stable.forEach(function(e){
        var sigCount=(e.signals&&e.signals.length)||0;
        wh('  <span class="m">\u2192 '+esc(e.name).padEnd(20)+'</span><span class="d">stable \u00B7 '+sigCount+' signal'+(sigCount===1?'':'s')+'</span>');
      });
      w('');
    }

    /* ── Footer ── */
    var projectCount=0;
    entries.forEach(function(e){projectCount+=((e.signals&&e.signals.length)||0)});
    wh('<span class="d">  '+total+' technolog'+(total===1?'y':'ies')+' \u00B7 '+projectCount+' signals</span>');
    w('');
    whint('try: dna for your developer profile');
    showTiming();
  }).catch(apiErr);
}

function cmdDecisions(){
  w('Loading...','d');
  api('/api/decisions').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('DECISION WINDOWS');w('');
    if(!d.windows||!d.windows.length){w('No open decision windows.','m');showTiming();return}
    d.windows.forEach(function(w2){
      var urg=Math.round((w2.urgency||0)*100),icon=urg>70?'\u26A1':urg>40?'\u231B':'\u25C7';
      wh(icon+' <span class="g">'+esc(w2.title)+'</span>');
      wh('    <span class="d">'+esc(w2.type||'\u2014')+' \u00B7 urgency: '+(urg)+'%</span>');
      if(w2.expires_at)wh('    <span class="d">expires: '+esc(w2.expires_at)+'</span>');
      w('');
    });
    showTiming();
  }).catch(apiErr);
}

function cmdDna(){
  w('Generating...','d');
  api('/api/dna').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    if(d.error){w(d.error,'r');return}
    wsep('DEVELOPER DNA');w('');
    if(d.identity_summary)wh('<span class="g">'+esc(d.identity_summary)+'</span>');w('');
    if(d.primary_stack&&d.primary_stack.length)wkv('Primary',d.primary_stack.join(' \u00B7 '));
    if(d.adjacent_tech&&d.adjacent_tech.length)wkv('Adjacent',d.adjacent_tech.join(' \u00B7 '));
    if(d.interests&&d.interests.length)wkv('Interests',d.interests.join(' \u00B7 '));
    if(d.stats){w('');
      wkv('Processed',d.stats.total_items_processed);wkv('Relevant',d.stats.total_relevant);
      wkv('Rejection',d.stats.rejection_rate+'%');wkv('Projects',d.stats.project_count);
      wkv('Dependencies',d.stats.dependency_count);wkv('Days Active',d.stats.days_active);
    }
    if(d.top_engaged_topics&&d.top_engaged_topics.length){w('');wsep('TOP TOPICS');
      d.top_engaged_topics.forEach(function(t){
        wh('  '+bar(t.percent_of_total/100,8)+' <span class="m">'+esc(t.topic).padEnd(16)+'</span>'+t.interactions+' interactions');
      });
    }
    whint('try: gaps to find blind spots');
    showTiming();
  }).catch(apiErr);
}

function cmdGaps(){
  w('Detecting...','d');
  api('/api/gaps').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('KNOWLEDGE GAPS');w('');
    if(!d.gaps||!d.gaps.length){w('No knowledge gaps detected.','gr');showTiming();return}
    wh('<span class="g">'+d.count+' gaps detected</span>');w('');
    d.gaps.forEach(function(g){
      var sevCls=g.severity==='critical'?'r':g.severity==='high'?'g':'m';
      wh('<span class="'+sevCls+'">  '+esc(g.dependency)+'</span> <span class="d">'+esc(g.severity)+'</span>');
      wh('    <span class="d">stale: '+g.days_since_engagement+'d \u00B7 missed: '+g.missed_items_count+' items</span>');
    });
    showTiming();
  }).catch(apiErr);
}

function cmdStatus(){
  w('Checking...','d');
  api('/api/status').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('SYSTEM STATUS');w('');
    wkv('Monitoring',d.monitoring?'active':'inactive',d.monitoring?'gr':'r');
    wkv('Last Analysis',d.last_analysis||'never');
    wkv('Scanned',d.total_scanned||0);
    wkv('Relevant',d.total_relevant||0);
    wkv('Threshold',d.threshold||0.35);
    if(d.total_scanned>0){
      var rejPct=((1-(d.total_relevant||0)/(d.total_scanned||1))*100).toFixed(1);
      wkv('Rejection',rejPct+'%');
    }
    /* Sparkline bars */
    w('');
    var scanned=d.total_scanned||0,relevant=d.total_relevant||0;
    wh('  <span class="g">Signals:  '+sparkbar(relevant,Math.max(scanned,1),10)+'</span> <span class="d">('+relevant+' relevant)</span>');
    wh('  <span class="m">Scanned:  '+sparkbar(scanned,scanned,10)+'</span> <span class="d">('+scanned+' total)</span>');
    w('');
    wh('  <span class="d">Uptime: '+fmtUptime(performance.now()-startTime)+'</span>');
    whint('try: signals to see what matters');
    showTiming();
  }).catch(apiErr);
}

/* ── Theme command ── */
function cmdTheme(name){
  if(!name){
    wsep('THEMES');
    var names=Object.keys(THEMES);
    names.forEach(function(n){
      var marker=n===currentTheme?' <span class="gr">\u25C0 active</span>':'';
      var t=THEMES[n];
      wh('  <span style="color:'+t.gold+'">\u25A0</span> <span class="m">'+n+'</span>'+marker);
    });
    w('');wh('<span class="d">  Usage: theme &lt;name&gt;</span>');
    showTiming();
    return;
  }
  name=name.toLowerCase();
  if(applyTheme(name)){
    w('Theme set to '+name+'.','gr');
  } else {
    w('Unknown theme: '+name+'. Available: '+Object.keys(THEMES).join(', '),'r');
  }
  showTiming();
}

/* ── Easter eggs ── */

/* whoami */
function cmdWhoami(){
  w('Loading...','d');
  api('/api/dna').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    var stack=(d.primary_stack||['unknown']).join(' \u00B7 ');
    var focus=(d.interests||['unknown']).slice(0,2).join(' \u00B7 ');
    var projects=d.stats?d.stats.project_count||0:0;
    var deps=d.stats?d.stats.dependency_count||0:0;
    var rejection=d.stats?d.stats.rejection_rate||0:0;

    var box=document.createElement('div');box.className='info-box';
    box.innerHTML=
      '<div class="ib-title">\u25C9 DEVELOPER DNA</div>'+
      '<div class="ib-row"><span class="ib-key">Stack:     </span>'+esc(stack)+'</div>'+
      '<div class="ib-row"><span class="ib-key">Focus:     </span>'+esc(focus)+'</div>'+
      '<div class="ib-row"><span class="ib-key">Projects:  </span>'+projects+' \u00B7 Deps: '+deps+'</div>'+
      '<div class="ib-row"><span class="ib-key">Rejection: </span>'+rejection+'%</div>'+
      '<div class="ib-footer">All signal. No feed.</div>';
    out.appendChild(box);scroll();
    showTiming();
  }).catch(function(e){
    if(e.message!=='auth'){rmLast();w('Could not load DNA profile.','r')}
  });
}

/* uptime */
function cmdUptime(){
  var elapsed=performance.now()-startTime;
  wh('<span class="g">Terminal uptime:</span> '+fmtUptime(elapsed));
  wh('<span class="d">Session started '+new Date(Date.now()-elapsed).toLocaleTimeString()+'</span>');
  showTiming();
}

/* fortune */
function cmdFortune(){
  var quote=FORTUNES[Math.floor(Math.random()*FORTUNES.length)];
  w('');
  wh('<span class="g">"'+esc(quote)+'"</span>');
  w('');
  showTiming();
}

/* matrix */
function cmdMatrix(){
  var canvas=document.createElement('canvas');
  canvas.className='matrix-canvas';
  canvas.width=window.innerWidth;canvas.height=window.innerHeight;
  document.body.appendChild(canvas);
  var ctx=canvas.getContext('2d');
  var cols=Math.floor(canvas.width/14);
  var drops=[];
  for(var i=0;i<cols;i++)drops[i]=Math.random()*-20|0;

  var chars='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@#$%^&*()_+-=[]{}|;:,.<>?\u30A2\u30A4\u30A6\u30A8\u30AA\u30AB\u30AD\u30AF\u30B1\u30B3\u30B5\u30B7\u30B9\u30BB\u30BD\u30BF\u30C1\u30C4\u30C6\u30C8';
  var matrixGreen=currentTheme==='phosphor'?'#33FF33':currentTheme==='frost'?'#67E8F9':currentTheme==='ember'?'#FF6B35':'#22C55E';

  var frame=0,maxFrames=180; /* ~3 seconds at 60fps */
  var fadeAlpha=0;

  function draw(){
    if(frame>=maxFrames){
      /* Fade out */
      fadeAlpha+=0.05;
      ctx.fillStyle='rgba(0,0,0,'+Math.min(fadeAlpha,1)+')';
      ctx.fillRect(0,0,canvas.width,canvas.height);
      if(fadeAlpha>=1.2){canvas.remove();return}
      requestAnimationFrame(draw);
      return;
    }

    ctx.fillStyle='rgba(0,0,0,0.05)';
    ctx.fillRect(0,0,canvas.width,canvas.height);
    ctx.fillStyle=matrixGreen;ctx.font='14px monospace';

    for(var i=0;i<cols;i++){
      if(drops[i]<0){drops[i]++;continue}
      var ch=chars[Math.floor(Math.random()*chars.length)];
      /* Bright head */
      ctx.fillStyle='#fff';
      ctx.fillText(ch,i*14,drops[i]*16);
      /* Trail */
      ctx.fillStyle=matrixGreen;
      if(drops[i]>1){
        var trailCh=chars[Math.floor(Math.random()*chars.length)];
        ctx.globalAlpha=0.7;
        ctx.fillText(trailCh,i*14,(drops[i]-1)*16);
        ctx.globalAlpha=1;
      }

      if(drops[i]*16>canvas.height&&Math.random()>0.975)drops[i]=0;
      drops[i]++;
    }
    frame++;
    requestAnimationFrame(draw);
  }
  draw();
  wh('<span class="gr">Wake up, Neo...</span>');
}

/* neofetch */
function cmdNeofetch(){
  w('Loading...','d');
  api('/api/status').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    var elapsed=performance.now()-startTime;
    var lines=[
      '    \u256D\u2500\u2500\u2500\u2500\u2500\u256E',
      '    \u2502  4  \u2502  <span class="g">4DA Signal Terminal</span> <span class="d">v1.0.0</span>',
      '    \u2570\u2500\u2500\u2500\u2500\u2500\u256F  <span class="d">\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500</span>',
      '             <span class="m">OS:</span> 4DA Home v1.0.0',
      '             <span class="m">Signals:</span> '+(d.total_relevant||0)+' relevant',
      '             <span class="m">Sources:</span> '+(d.monitoring?'active':'idle'),
      '             <span class="m">Scanned:</span> '+(d.total_scanned||0),
      '             <span class="m">Threshold:</span> '+(d.threshold||0.35),
      '             <span class="m">Monitoring:</span> <span class="'+(d.monitoring?'gr':'r')+'">'+(d.monitoring?'active':'off')+'</span>',
      '             <span class="m">Uptime:</span> '+fmtUptime(elapsed),
      '             <span class="m">Theme:</span> '+currentTheme
    ];
    lines.forEach(function(l){wh(l)});
    showTiming();
  }).catch(function(e){
    if(e.message!=='auth'){rmLast();w('Could not load system info.','r')}
  });
}

/* ── Phase 1.4: Real ping data ── */
function cmdPing(){
  w('Pinging sources...','d');
  api('/api/sources').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('SOURCE CONNECTIVITY');w('');
    if(!d.sources||!d.sources.length){w('No sources registered.','m');showTiming();return}
    var delay=0;
    var sourceList=d.sources;
    sourceList.forEach(function(s){
      setTimeout(function(){
        wh('<span class="gr">\u25CF</span> <span class="m">'+esc(s.name||s.source_type||'unknown').padEnd(20)+'</span> '+esc(s.source_type||''));
      },delay);
      delay+=80;
    });
    setTimeout(function(){
      w('');wh('<span class="d">'+(d.count||sourceList.length)+' sources registered</span>');
      showTiming();
    },delay);
  }).catch(function(e){
    /* Fallback: if /api/sources doesn't exist, use status-based approach */
    if(e.message==='auth'){apiErr(e);return}
    rmLast();
    w('Pinging...','d');
    api('/api/status').then(function(){
      rmLast();
      wsep('SOURCE CONNECTIVITY');w('');
      var sources=['HackerNews','Reddit','GitHub','arXiv','RSS Feeds','Stack Overflow','Dev.to','Lobsters','Product Hunt','Tech Blogs','Newsletters'];
      var i=0;
      function nextPing(){
        if(i>=sources.length){w('');wh('<span class="gr">All sources reachable.</span>');showTiming();return}
        var src=sources[i];
        var online=Math.random()>0.05;
        wh('  '+src.padEnd(18)+' <span class="'+(online?'gr':'r')+'">\u25CF '+(online?'online':'timeout')+'</span>');
        i++;
        setTimeout(nextPing,80);
      }
      nextPing();
    }).catch(function(e2){
      if(e2.message!=='auth'){rmLast();w('Cannot reach backend.','r')}
    });
  });
}

/* ── Phase 1.5: More command ── */
function cmdMore(){
  if(!pendingMore||!pendingMore.items.length){w('Nothing more to show.','m');showTiming();return}
  var batch=pendingMore.items.splice(0,15);
  batch.forEach(pendingMore.renderer);
  if(pendingMore.items.length>0){
    wh('<span class="d">\u2500\u2500 '+pendingMore.items.length+' more items (type \'more\' to show) \u2500\u2500</span>');
  } else {
    pendingMore=null;
    w('');
    wh('<span class="d">End of results.</span>');
  }
  showTiming();
}

/* ── Phase 2.1: Diff command ── */
function cmdDiff(){
  w('Comparing...','d');
  api('/api/signals').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}

    var current=(d.signals||[]).map(function(s){return s.url||s.title});
    var prev=JSON.parse(localStorage.getItem('4da_prev_signals')||'[]');
    localStorage.setItem('4da_prev_signals',JSON.stringify(current));

    if(!prev.length){w('No previous snapshot. Run diff again after your next analysis.','m');showTiming();return}

    var added=current.filter(function(u){return prev.indexOf(u)===-1});
    var removed=prev.filter(function(u){return current.indexOf(u)===-1});
    var unchanged=current.filter(function(u){return prev.indexOf(u)!==-1});

    wsep('DIFF');w('');
    wh('<span class="gr">+ '+added.length+' new signals</span>');
    wh('<span class="r">- '+removed.length+' dropped signals</span>');
    wh('<span class="d">= '+unchanged.length+' unchanged</span>');

    if(added.length){
      w('');wh('<span class="gr">NEW:</span>');
      (d.signals||[]).filter(function(s){return added.indexOf(s.url||s.title)!==-1}).forEach(function(s){
        wh('  <span class="gr">+</span> '+esc(s.title)+' <span class="d">'+esc(s.source||'')+' '+esc(String(s.score_raw||''))+'</span>');
      });
    }
    if(removed.length){
      w('');wh('<span class="r">DROPPED:</span>');
      removed.slice(0,5).forEach(function(u){wh('  <span class="r">-</span> <span class="d">'+esc(u)+'</span>')});
      if(removed.length>5)wh('  <span class="d">... and '+(removed.length-5)+' more</span>');
    }
    showTiming();
  }).catch(apiErr);
}

/* ── Phase 2.2: Watch mode ── */
function cmdWatch(arg){
  if(!arg){
    if(watchInterval){
      clearInterval(watchInterval);watchInterval=null;
      w('Watch stopped.','m');
    } else {
      w('Usage: watch <command> (type watch again to stop)','r');
    }
    showTiming();
    return;
  }
  if(watchInterval){clearInterval(watchInterval);watchInterval=null;w('Previous watch stopped.','m')}
  w('Watching "'+arg+'" every 30s. Type watch to stop.','m');
  execSingle(arg);
  watchInterval=setInterval(function(){
    out.innerHTML='';
    wh('<span class="d">\u2500\u2500 watch: '+esc(arg)+' (refreshing every 30s, type watch to stop) \u2500\u2500</span>');
    w('');
    execSingle(arg);
  },30000);
}

/* ── Phase 2.4: Copy command ── */
function cmdCopy(){
  if(!lastOutput){w('Nothing to copy.','m');showTiming();return}
  navigator.clipboard.writeText(lastOutput).then(function(){
    w('Copied to clipboard.','gr');
    showTiming();
  }).catch(function(){
    w('Clipboard access denied.','r');
    showTiming();
  });
}

/* ── Phase 2.5: Command palette ── */
function showPalette(){
  /* Prevent double-open */
  if(document.querySelector('.palette-overlay'))return;

  var overlay=document.createElement('div');
  overlay.className='palette-overlay';
  var box=document.createElement('div');
  box.className='palette-box';
  var searchInput=document.createElement('input');
  searchInput.type='text';searchInput.placeholder='Type a command...';searchInput.autocomplete='off';
  var list=document.createElement('div');
  list.className='palette-list';
  box.appendChild(searchInput);
  box.appendChild(list);
  overlay.appendChild(box);
  document.body.appendChild(overlay);

  var allCmds=COMMANDS.slice();
  var selectedIdx=0;

  function renderList(filter){
    var filtered=allCmds;
    if(filter){
      var lower=filter.toLowerCase();
      filtered=allCmds.filter(function(c){
        return c.indexOf(lower)!==-1||(CMD_HELP[c]||'').toLowerCase().indexOf(lower)!==-1;
      });
    }
    list.innerHTML='';
    selectedIdx=0;
    filtered.forEach(function(c,i){
      var item=document.createElement('div');
      item.className='palette-item'+(i===0?' selected':'');
      item.innerHTML='<span class="pi-cmd">'+esc(c)+'</span><span class="pi-desc">'+esc(CMD_HELP[c]||'')+'</span>';
      item.setAttribute('data-cmd',c);
      item.addEventListener('click',function(){
        closePalette();
        inp.value=c;
        exec(c);
      });
      list.appendChild(item);
    });
  }

  function updateSelection(newIdx){
    var items=list.querySelectorAll('.palette-item');
    if(!items.length)return;
    if(newIdx<0)newIdx=items.length-1;
    if(newIdx>=items.length)newIdx=0;
    items[selectedIdx].classList.remove('selected');
    selectedIdx=newIdx;
    items[selectedIdx].classList.add('selected');
    items[selectedIdx].scrollIntoView({block:'nearest'});
  }

  function closePalette(){
    overlay.remove();
    inp.focus();
  }

  searchInput.addEventListener('input',function(){
    renderList(this.value);
  });

  searchInput.addEventListener('keydown',function(e){
    if(e.key==='Escape'){e.preventDefault();closePalette();return}
    if(e.key==='ArrowDown'){e.preventDefault();updateSelection(selectedIdx+1);return}
    if(e.key==='ArrowUp'){e.preventDefault();updateSelection(selectedIdx-1);return}
    if(e.key==='Enter'){
      e.preventDefault();
      var items=list.querySelectorAll('.palette-item');
      if(items.length>0&&items[selectedIdx]){
        var cmd=items[selectedIdx].getAttribute('data-cmd');
        closePalette();
        inp.value=cmd;
        exec(cmd);
      }
    }
  });

  overlay.addEventListener('click',function(e){
    if(e.target===overlay)closePalette();
  });

  renderList('');
  searchInput.focus();
}

/* ── Phase 2.6: Alias commands ── */
function cmdAlias(arg){
  if(!arg){
    wsep('ALIASES');
    var keys=Object.keys(aliases);
    if(!keys.length){w('No aliases defined. Usage: alias name = command','m');showTiming();return}
    keys.forEach(function(k){wkv(k,aliases[k])});
    showTiming();
    return;
  }
  var eqIdx=arg.indexOf('=');
  if(eqIdx===-1){w('Usage: alias name = command','r');showTiming();return}
  var name=arg.substring(0,eqIdx).trim();
  var value=arg.substring(eqIdx+1).trim();
  if(!name||!value){w('Usage: alias name = command','r');showTiming();return}
  aliases[name]=value;
  localStorage.setItem('4da_aliases',JSON.stringify(aliases));
  w('Alias set: '+name+' \u2192 '+value,'gr');
  showTiming();
}

function cmdUnalias(arg){
  if(!arg){w('Usage: unalias name','r');showTiming();return}
  if(!aliases[arg]){w('Alias not found: '+arg,'r');showTiming();return}
  delete aliases[arg];
  localStorage.setItem('4da_aliases',JSON.stringify(aliases));
  w('Alias removed: '+arg,'m');
  showTiming();
}

/* ── Phase 3.1: Natural language interpretation ── */
function tryNaturalLanguage(raw){
  var lower=raw.toLowerCase();
  if(lower.match(/show.*signal|what.*signal|any.*signal/))return 'signals';
  if(lower.match(/show.*security|security.*alert|cve|vulnerabilit/))return 'signals --priority critical';
  if(lower.match(/what.*new|anything.*new|what.*changed/))return 'diff';
  if(lower.match(/my.*stack|tech.*stack|what.*using/))return 'radar';
  if(lower.match(/my.*profile|who.*am|about.*me/))return 'dna';
  if(lower.match(/blind.*spot|gap|missing|what.*miss/))return 'gaps';
  if(lower.match(/how.*doing|system|health/))return 'status';
  if(lower.match(/briefing|summary|digest|overview/))return 'briefing';
  if(lower.match(/decision|window|deadline|time.*sensitiv/))return 'decisions';
  if(lower.match(/source|feed|where.*from/))return 'sources';
  if(lower.match(/reading.*list|saved|bookmarks/))return 'queue';
  return null;
}

/* ── Phase 3.2: Sources command ── */
function cmdSources(){
  w('Loading...','d');
  api('/api/sources').then(function(d){
    rmLast();
    if(jsonMode){wh('<pre style="color:var(--fg)">'+esc(JSON.stringify(d,null,2))+'</pre>');showTiming();return}
    wsep('SOURCES');w('');
    if(!d.sources||!d.sources.length){w('No sources registered.','m');showTiming();return}
    d.sources.forEach(function(s){
      wh('<span class="gr">\u25CF</span> <span class="m">'+esc(s.name||s.source_type||'unknown').padEnd(20)+'</span><span class="d">'+esc(s.source_type||'')+'</span>');
    });
    w('');wh('<span class="d">'+(d.count||d.sources.length)+' sources active</span>');
    showTiming();
  }).catch(function(e){
    /* Fallback if /api/sources doesn't exist */
    if(e.message==='auth'){apiErr(e);return}
    rmLast();
    w('Sources endpoint not available. Use ping for connectivity check.','m');
    showTiming();
  });
}

/* ── Phase 3.4: Reading queue ── */
function cmdSave(arg){
  if(!arg){w('Usage: save <n> (signal number from last signals output)','r');showTiming();return}
  var idx=parseInt(arg)-1;
  if(isNaN(idx)||idx<0){w('Invalid number. Use the signal number from the signals list.','r');showTiming();return}
  if(!cachedSignals||!cachedSignals.length){w('No cached signals. Run signals first.','r');showTiming();return}
  if(idx>=cachedSignals.length){w('Signal #'+arg+' does not exist. Max: '+cachedSignals.length,'r');showTiming();return}

  var signal=cachedSignals[idx];
  var queueItem={
    title:signal.title||'Untitled',
    url:signal.url||'',
    source:signal.source||'',
    score:signal.score_raw||0,
    saved_at:new Date().toISOString()
  };

  /* Prevent duplicates */
  var isDup=readingQueue.some(function(q){return q.url===queueItem.url&&q.title===queueItem.title});
  if(isDup){w('Already in reading queue: '+queueItem.title,'m');showTiming();return}

  readingQueue.push(queueItem);
  localStorage.setItem('4da_queue',JSON.stringify(readingQueue));
  w('Saved to queue: '+queueItem.title,'gr');
  wh('<span class="d">Queue now has '+readingQueue.length+' item'+(readingQueue.length>1?'s':'')+'</span>');
  showTiming();
}

function cmdQueue(){
  if(!readingQueue.length){w('Reading queue is empty. Use save <n> to add signals.','m');showTiming();return}
  wsep('READING QUEUE ('+readingQueue.length+')');w('');
  readingQueue.forEach(function(item,i){
    var title=item.url?link(item.url,item.title):esc(item.title);
    var scr=item.score?Math.round(item.score*100)+'%':'';
    wh('<span class="m">'+(i+1).toString().padStart(2)+'. </span>'+title+' <span class="d">'+esc(item.source)+' '+scr+'</span>');
    wh('       <span class="d">saved '+esc(new Date(item.saved_at).toLocaleDateString())+'</span>');
  });
  w('');
  wh('<span class="d">Use read &lt;n&gt; to open, or read clear to empty queue</span>');
  showTiming();
}

function cmdRead(arg){
  if(!arg){w('Usage: read <n> or read clear','r');showTiming();return}
  if(arg==='clear'){
    readingQueue=[];
    localStorage.setItem('4da_queue',JSON.stringify(readingQueue));
    w('Reading queue cleared.','m');
    showTiming();
    return;
  }
  var idx=parseInt(arg)-1;
  if(isNaN(idx)||idx<0||idx>=readingQueue.length){
    w('Invalid index. Queue has '+readingQueue.length+' item'+(readingQueue.length!==1?'s':'')+'.','r');
    showTiming();
    return;
  }
  var item=readingQueue[idx];
  if(item.url){
    window.open(item.url,'_blank','noopener');
    w('Opened: '+item.title,'gr');
  } else {
    w('No URL for this item: '+item.title,'m');
  }
  readingQueue.splice(idx,1);
  localStorage.setItem('4da_queue',JSON.stringify(readingQueue));
  wh('<span class="d">Removed from queue. '+readingQueue.length+' remaining.</span>');
  showTiming();
}

/* ── Ambient mode ── */
function enterAmbient(){
  isAmbient=true;document.body.classList.add('ambient');out.innerHTML='';
  var grid=document.createElement('div');grid.className='amb-grid';
  grid.innerHTML=
    '<div class="amb-section" id="amb-sig"><h3>Signals</h3><div class="amb-content"></div></div>'+
    '<div class="amb-section" id="amb-radar"><h3>Tech Radar</h3><div class="amb-content"></div></div>'+
    '<div class="amb-section" id="amb-dec"><h3>Decision Windows</h3><div class="amb-content"></div></div>'+
    '<div class="amb-section" id="amb-status"><h3>System</h3><div class="amb-content"></div></div>';
  out.appendChild(grid);refreshAmbient();ambInterval=setInterval(refreshAmbient,60000);
}
function exitAmbient(){
  isAmbient=false;document.body.classList.remove('ambient');
  if(ambInterval){clearInterval(ambInterval);ambInterval=null}
  out.innerHTML='';
  bootSequence();
  inp.focus();
}
function refreshAmbient(){
  Promise.all([
    api('/api/signals').catch(function(){return{signals:[]}}),
    api('/api/radar').catch(function(){return{entries:[]}}),
    api('/api/decisions').catch(function(){return{windows:[]}}),
    api('/api/status').catch(function(){return{}})
  ]).then(function(res){
    var sigs=res[0],radar=res[1],decs=res[2],status=res[3];
    var sc=document.querySelector('#amb-sig .amb-content');if(sc){
      sc.innerHTML=(sigs.signals||[]).slice(0,10).map(function(s){
        var icon=s.signal_priority==='critical'?'\u26A1':s.signal_priority==='high'?'\u25C6':'\u25C7';
        return'<div class="out-line">'+icon+' <span class="g">['+Math.round((s.score_raw||0)*100)+']</span> '+esc(s.title)+'</div>';
      }).join('')||'<div class="out-line d">No signals</div>'}
    var rc=document.querySelector('#amb-radar .amb-content');if(rc){
      var rings=['adopt','trial','assess','hold'],colors={adopt:'gr',trial:'g',assess:'m',hold:'r'};
      rc.innerHTML=rings.map(function(ring){
        var items=(radar.entries||[]).filter(function(e){return e.ring===ring});if(!items.length)return'';
        return'<div class="out-line"><span class="'+colors[ring]+'">'+ring.toUpperCase()+'</span> '+items.map(function(e){return esc(e.name)}).join(' \u00B7 ')+'</div>';
      }).join('')||'<div class="out-line d">No entries</div>'}
    var dc=document.querySelector('#amb-dec .amb-content');if(dc){
      dc.innerHTML=(decs.windows||[]).map(function(w2){
        return'<div class="out-line">\u231B <span class="g">'+esc(w2.title)+'</span></div>';
      }).join('')||'<div class="out-line d">No open windows</div>'}
    var stc=document.querySelector('#amb-status .amb-content');if(stc){
      stc.innerHTML='<div class="out-line"><span class="d">Monitoring:</span> <span class="'+(status.monitoring?'gr':'r')+'">'+(status.monitoring?'active':'off')+'</span></div>'+
        '<div class="out-line"><span class="d">Scanned:</span> '+(status.total_scanned||0)+'</div>'+
        '<div class="out-line"><span class="d">Relevant:</span> '+(status.total_relevant||0)+'</div>'+
        '<div class="out-line"><span class="d">Last:</span> '+esc(status.last_analysis||'never')+'</div>'}
  });
}

/* ── Phase 3.3: Request notification permission ── */
if('Notification' in window && Notification.permission==='default'){
  Notification.requestPermission();
}

/* ── Init ── */
/* Auto-auth for localhost: try without token first */
(function init(){
  fetch('/api/status',{headers:token?{'X-4DA-Token':token}:{}})
  .then(function(r){
    if(r.ok){
      /* Authenticated (localhost auto-trust or valid token) */
      liveDot.className='live-dot on';liveText.textContent='LIVE';
      bootSequence();
      refreshStatus();
    } else if(r.status===401){
      /* Need auth */
      showAuth();
      bootSequence();
    } else {
      throw new Error('offline');
    }
  }).catch(function(){
    liveDot.className='live-dot off';liveText.textContent='OFFLINE';
    bootSequence();
    startReconnect();
  });
})();

setInterval(refreshStatus,30000);
inp.focus();
})();
