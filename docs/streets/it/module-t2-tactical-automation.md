# Modulo T: Automazione Tattica

**Corso STREETS per il Reddito degli Sviluppatori — Modulo a Pagamento**
*Settimane 12-13 | 6 Lezioni | Deliverable: Una Pipeline Automatizzata che Genera Valore*

> "LLM, agenti, MCP e cron job come moltiplicatori di forza."

---

Hai motori di reddito in funzione. Hai clienti. Hai processi che funzionano. E stai spendendo il 60-70% del tuo tempo a fare le stesse cose più e più volte: elaborare input, formattare output, controllare monitor, inviare aggiornamenti, revisionare code.

Quel tempo è la tua risorsa più costosa, e lo stai bruciando su compiti che un VPS da {= regional.currency_symbol | fallback("$") =}5/mese potrebbe gestire.

{@ insight hardware_benchmark @}

Questo modulo riguarda il rimuoverti sistematicamente dal loop — non completamente (è una trappola che tratteremo nella Lezione 5), ma dall'80% del lavoro che non richiede il tuo giudizio. Il risultato: i tuoi flussi di reddito producono entrate mentre dormi, mentre sei al tuo lavoro diurno, mentre stai costruendo la prossima cosa.

Alla fine di queste due settimane, avrai:

- Una comprensione chiara dei quattro livelli di automazione e di dove ti trovi oggi
- Cron job e automazioni schedulate funzionanti sulla tua infrastruttura
- Almeno una pipeline alimentata da LLM che elabora input senza il tuo intervento
- Una comprensione dei sistemi basati su agenti e di quando hanno senso economico
- Un framework human-in-the-loop affinché l'automazione non distrugga la tua reputazione
- Una pipeline completa, deployata, che genera valore senza il tuo coinvolgimento attivo

{? if stack.primary ?}
Il tuo stack principale è {= stack.primary | fallback("il tuo stack principale") =}, quindi gli esempi di automazione che seguono saranno più direttamente applicabili se adattati a quell'ecosistema. La maggior parte degli esempi usa Python per portabilità, ma i pattern si traducono in qualsiasi linguaggio.
{? endif ?}

Questo è il modulo più ricco di codice del corso. Almeno la metà di ciò che segue è codice eseguibile. Copialo, adattalo, deployalo.

Automatizziamo.

---

## Lezione 1: La Piramide dell'Automazione

*"La maggior parte degli sviluppatori automatizza al Livello 1. I soldi sono al Livello 3."*

### I Quattro Livelli

Ogni automazione nel tuo stack di reddito si colloca da qualche parte in questa piramide:

```
┌───────────────────────────────┐
│  Livello 4: Agenti Autonomi   │  ← Prende decisioni per te
│  (L'IA decide E agisce)       │
├───────────────────────────────┤
│  Livello 3: Pipeline          │  ← I soldi sono qui
│  Intelligenti (LLM-powered)   │
├───────────────────────────────┤
│  Livello 2: Automazione       │  ← La maggior parte degli sviluppatori si ferma qui
│  Schedulata (cron + script)   │
├───────────────────────────────┤
│  Livello 1: Manuale con       │  ← Dove si trova la maggior parte degli sviluppatori
│  Template (copia-incolla)     │
└───────────────────────────────┘
```

Vediamo nello specifico come appare ciascun livello nella pratica.

### Livello 1: Manuale con Template

Fai il lavoro tu, ma hai checklist, template e snippet per velocizzarti.

**Esempi:**
- Scrivi un post per il blog usando un template markdown con frontmatter pre-compilato
- Fatturi ai clienti duplicando la fattura del mese scorso e cambiando i numeri
- Rispondi alle email di supporto usando risposte salvate
- Pubblichi contenuti eseguendo manualmente un comando di deploy

**Costo in tempo:** 100% del tuo tempo per unità di output.
**Tasso di errore:** Moderato — sei umano, fai errori quando sei stanco.
**Tetto di scala:** Tu. Le tue ore. Punto.

La maggior parte degli sviluppatori vive qui e non si rende nemmeno conto che c'è una piramide sopra di loro.

### Livello 2: Automazione Schedulata

Gli script girano su pianificazioni. Hai scritto la logica una volta. Si esegue senza di te.

**Esempi:**
- Un cron job che controlla il tuo feed RSS e pubblica nuovi articoli sui social media
- Una GitHub Action che compila e deploya il tuo sito ogni mattina alle 6
- Uno script che gira ogni ora per controllare i prezzi dei concorrenti e registrare i cambiamenti
- Un backup giornaliero del database che gira alle 3 di notte

**Costo in tempo:** Zero continuativo (dopo il setup iniziale di 1-4 ore).
**Tasso di errore:** Basso — deterministico, stessa logica ogni volta.
**Tetto di scala:** Quanti compiti la tua macchina riesce a schedulare. Centinaia.

Qui è dove si piazzano la maggior parte degli sviluppatori tecnici. È comodo. Ma ha un limite rigido: può gestire solo compiti con logica deterministica. Se il compito richiede giudizio, sei bloccato.

### Livello 3: Pipeline Intelligenti

Gli script girano su pianificazioni, ma includono un LLM che gestisce le decisioni di giudizio.

**Esempi:**
- I feed RSS vengono ingeriti, l'LLM riassume ogni articolo, bozza una newsletter, tu revisioni per 10 minuti e premi invio
- Le email di feedback dei clienti vengono classificate per sentimento e urgenza, le bozze di risposta vengono messe in coda per la tua approvazione
- Nuove offerte di lavoro nella tua nicchia vengono scrappate, l'LLM valuta la rilevanza, ricevi un digest giornaliero di 5 opportunità invece di scansionare 200 annunci
- I post dei blog concorrenti vengono monitorati, l'LLM estrae i cambiamenti chiave di prodotto, ricevi un report settimanale di intelligence competitiva

**Costo in tempo:** 10-20% del tempo manuale. Revisioni e approvi invece di creare.
**Tasso di errore:** Basso per compiti di classificazione, moderato per la generazione (ed è per questo che revisioni).
**Tetto di scala:** Migliaia di elementi al giorno. Il tuo collo di bottiglia è il costo delle API, non il tuo tempo.

**Qui sono i soldi.** Il Livello 3 permette a una persona di gestire flussi di reddito che normalmente richiederebbero un team di 3-5 persone.

### Livello 4: Agenti Autonomi

Sistemi IA che osservano, decidono e agiscono senza il tuo intervento.

**Esempi:**
- Un agente che monitora le metriche del tuo SaaS, rileva un calo nelle iscrizioni, testa A/B un cambio di prezzo e lo ripristina se non funziona
- Un agente di supporto che gestisce le domande dei clienti di Tier 1 in modo completamente autonomo, escalando a te solo per le questioni complesse
- Un agente di contenuti che identifica topic di tendenza, genera bozze, schedula la pubblicazione e monitora le performance

**Costo in tempo:** Vicino a zero per i casi gestiti. Revisioni le metriche, non le singole azioni.
**Tasso di errore:** Dipende interamente dai tuoi guardrail. Senza di essi: alto. Con buoni guardrail: sorprendentemente basso per domini ristretti.
**Tetto di scala:** Effettivamente illimitato per i compiti nell'ambito dell'agente.

Il Livello 4 è reale e raggiungibile, ma non è da dove si parte. E come vedremo nella Lezione 5, gli agenti completamente autonomi che interagiscono con i clienti sono pericolosi per la tua reputazione se implementati male.

> **Parliamoci chiaro:** Se sei al Livello 1 adesso, non provare a saltare al Livello 4. Passerai settimane a costruire un "agente autonomo" che si rompe in produzione e danneggia la fiducia dei clienti. Sali la piramide un livello alla volta. Il Livello 2 è un pomeriggio di lavoro. Il Livello 3 è un progetto da weekend. Il Livello 4 arriva dopo che il Livello 3 ha girato in modo affidabile per un mese.

### Autovalutazione: Dove Sei?

Per ciascuno dei tuoi flussi di reddito, valutati onestamente:

| Flusso di Reddito | Livello Attuale | Ore/Settimana Spese | Potrei Automatizzare A |
|---|---|---|---|
| [es., Newsletter] | [1-4] | [X] ore | [livello target] |
| [es., Elaborazione clienti] | [1-4] | [X] ore | [livello target] |
| [es., Social media] | [1-4] | [X] ore | [livello target] |
| [es., Supporto] | [1-4] | [X] ore | [livello target] |

La colonna che conta di più è "Ore/Settimana Spese." Il flusso con le ore più alte e il livello più basso è il tuo primo obiettivo di automazione. È quello con il ROI più grande.

### L'Economia di Ciascun Livello

Supponiamo che tu abbia un flusso di reddito che richiede 10 ore/settimana del tuo tempo e genera {= regional.currency_symbol | fallback("$") =}2.000/mese:

| Livello | Il Tuo Tempo | La Tua Tariffa Effettiva | Costo Automazione |
|---|---|---|---|
| Livello 1 | 10 ore/settimana | $50/ora | $0 |
| Livello 2 | 3 ore/settimana | $167/ora | $5/mese (VPS) |
| Livello 3 | 1 ora/settimana | $500/ora | $30-50/mese (API) |
| Livello 4 | 0,5 ore/settimana | $1.000/ora | $50-100/mese (API + calcolo) |

Passare dal Livello 1 al Livello 3 non cambia il tuo fatturato. Cambia la tua tariffa oraria effettiva da $50 a $500. E quelle 9 ore liberate? Vanno a costruire il prossimo flusso di reddito o a migliorare quello attuale.

> **Errore comune:** Automatizzare prima il flusso a più basso reddito perché è "più facile." No. Automatizza il flusso che consuma più ore rispetto al suo reddito. È lì che sta il ROI.

### Tocca a Te

1. Compila la tabella di autovalutazione sopra per ogni flusso di reddito (o flusso pianificato) che hai.
2. Identifica il tuo obiettivo di automazione con il ROI più alto: il flusso con più ore e il livello di automazione più basso.
3. Scrivi i 3 compiti che consumano più tempo in quel flusso. Automatizzerai il primo nella Lezione 2.

---

## Lezione 2: Dal Livello 1 al 2 — Automazione Schedulata

*"Cron è del 1975. Funziona ancora. Usalo."*

### Fondamenti dei Cron Job

{? if computed.os_family == "windows" ?}
Sei su Windows, quindi cron non è nativo nel tuo sistema. Hai due opzioni: usare WSL (Windows Subsystem for Linux) per avere il vero cron, o usare Windows Task Scheduler (trattato sotto). WSL è raccomandato se ti trovi a tuo agio — tutti gli esempi di cron in questa lezione funzionano direttamente in WSL. Se preferisci Windows nativo, salta alla sezione Task Scheduler dopo questa.
{? endif ?}

Sì, anche nel 2026, cron è il re dei compiti schedulati. È affidabile, è ovunque, e non richiede un account cloud, un abbonamento SaaS, o uno schema YAML che devi googlare ogni volta.

**La sintassi cron in 30 secondi:**

```
┌───────── minuto (0-59)
│ ┌───────── ora (0-23)
│ │ ┌───────── giorno del mese (1-31)
│ │ │ ┌───────── mese (1-12)
│ │ │ │ ┌───────── giorno della settimana (0-7, 0 e 7 = Domenica)
│ │ │ │ │
* * * * *  comando
```

**Pianificazioni comuni:**

```bash
# Ogni ora
0 * * * *  /path/to/script.sh

# Ogni giorno alle 6 di mattina
0 6 * * *  /path/to/script.sh

# Ogni lunedì alle 9 di mattina
0 9 * * 1  /path/to/script.sh

# Ogni 15 minuti
*/15 * * * *  /path/to/script.sh

# Primo giorno di ogni mese a mezzanotte
0 0 1 * *  /path/to/script.sh
```

**Impostare un cron job:**

```bash
# Modifica il tuo crontab
crontab -e

# Elenca i cron job esistenti
crontab -l

# CRITICO: Imposta sempre le variabili d'ambiente in cima
# Cron gira con un ambiente minimale — PATH potrebbe non includere i tuoi strumenti
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/tuoutente

# Registra l'output così puoi debuggare i fallimenti
0 6 * * * /home/tuoutente/scripts/daily-report.sh >> /home/tuoutente/logs/daily-report.log 2>&1
```

> **Errore comune:** Scrivere uno script che funziona perfettamente quando lo esegui manualmente, poi fallisce silenziosamente in cron perché cron non carica il tuo `.bashrc` o `.zshrc`. Usa sempre percorsi assoluti negli script cron. Imposta sempre `PATH` in cima al tuo crontab. Redireziona sempre l'output a un file di log.

### Scheduler Cloud per Quando Cron Non Basta

Se la tua macchina non è accesa 24/7, o hai bisogno di qualcosa di più robusto, usa uno scheduler cloud:

**GitHub Actions (gratis per repo pubblici, 2.000 min/mese per i privati):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # Ogni giorno alle 6 AM UTC
    - cron: '0 6 * * *'
  # Consenti trigger manuale per il testing
  workflow_dispatch:

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install dependencies
        run: npm ci

      - name: Run publisher
        env:
          CMS_API_KEY: ${{ secrets.CMS_API_KEY }}
          SOCIAL_TOKEN: ${{ secrets.SOCIAL_TOKEN }}
        run: node scripts/publish-scheduled-content.js
```

**Vercel Cron (gratis sul piano Hobby, 1 al giorno; piano Pro: illimitati):**

```typescript
// api/cron/daily-report.ts
// Endpoint cron di Vercel — configura la pianificazione in vercel.json

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Verifica che sia davvero Vercel a chiamare, non una richiesta HTTP casuale
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // La tua logica di automazione qui
  const report = await generateDailyReport();
  await sendToSlack(report);

  return new Response('OK', { status: 200 });
}
```

```json
// vercel.json
{
  "crons": [
    {
      "path": "/api/cron/daily-report",
      "schedule": "0 6 * * *"
    }
  ]
}
```

### Automazioni Reali da Costruire Subito

Ecco cinque automazioni che puoi implementare oggi. Ognuna richiede 30-60 minuti ed elimina ore di lavoro manuale settimanale.

#### Automazione 1: Pubblicazione Automatica di Contenuti su Pianificazione

Scrivi post per il blog in anticipo. Questo script li pubblica all'orario programmato.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — Pubblica post markdown alla data programmata.
Eseguilo giornalmente via cron: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# Il tuo endpoint API del CMS (Hashnode, Dev.to, Ghost, ecc.)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """Carica la lista dei nomi file dei post già pubblicati."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """Salva la lista dei nomi file dei post pubblicati."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """Estrai il frontmatter stile YAML da un file markdown."""
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    if not content.startswith("---"):
        return {}

    parts = content.split("---", 2)
    if len(parts) < 3:
        return {}

    metadata = {}
    for line in parts[1].strip().split("\n"):
        if ":" in line:
            key, value = line.split(":", 1)
            metadata[key.strip()] = value.strip().strip('"').strip("'")

    metadata["body"] = parts[2].strip()
    return metadata

def should_publish(metadata: dict) -> bool:
    """Controlla se un post deve essere pubblicato oggi."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """Pubblica un post alla tua API del CMS."""
    payload = {
        "title": metadata.get("title", "Untitled"),
        "content": metadata.get("body", ""),
        "tags": metadata.get("tags", "").split(","),
        "status": "published"
    }

    try:
        response = requests.post(
            CMS_API_URL,
            json=payload,
            headers={
                "Authorization": f"Bearer {CMS_API_KEY}",
                "Content-Type": "application/json"
            },
            timeout=30
        )
        response.raise_for_status()
        print(f"  Pubblicato: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  FALLITO: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"Controllo {len(posts)} post...")

    for filepath in sorted(posts):
        filename = os.path.basename(filepath)

        if filename in published:
            continue

        metadata = parse_frontmatter(filepath)
        if not metadata:
            continue

        if should_publish(metadata):
            if publish_post(metadata):
                published.add(filename)

    save_published(published)
    print(f"Totale pubblicati: {len(published)}")

if __name__ == "__main__":
    main()
```

**I tuoi post markdown sono così:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Il contenuto del tuo post qui...
```

Scrivi i post quando arriva l'ispirazione. Imposta la data. Lo script gestisce il resto.

#### Automazione 2: Pubblicazione Automatica sui Social Media per Nuovi Contenuti

Quando il tuo blog pubblica qualcosa di nuovo, questo posta automaticamente su Twitter/X e Bluesky.

```python
#!/usr/bin/env python3
"""
social_poster.py — Pubblica sulle piattaforme social quando vengono pubblicati nuovi contenuti.
Eseguilo ogni 30 minuti: */30 * * * * python3 /path/to/social_poster.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime

FEED_URL = os.environ.get("RSS_FEED_URL", "https://yourblog.com/rss.xml")
POSTED_LOG = os.path.expanduser("~/income/logs/social_posted.json")
BLUESKY_HANDLE = os.environ.get("BLUESKY_HANDLE", "")
BLUESKY_APP_PASSWORD = os.environ.get("BLUESKY_APP_PASSWORD", "")

def load_posted() -> set:
    try:
        with open(POSTED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_posted(posted: set):
    os.makedirs(os.path.dirname(POSTED_LOG), exist_ok=True)
    with open(POSTED_LOG, "w") as f:
        json.dump(sorted(posted), f, indent=2)

def get_rss_items(feed_url: str) -> list:
    """Parsa un feed RSS e restituisce una lista di elementi."""
    import xml.etree.ElementTree as ET

    response = requests.get(feed_url, timeout=30)
    response.raise_for_status()
    root = ET.fromstring(response.content)

    items = []
    for item in root.findall(".//item"):
        title = item.findtext("title", "")
        link = item.findtext("link", "")
        description = item.findtext("description", "")
        item_id = hashlib.md5(link.encode()).hexdigest()
        items.append({
            "id": item_id,
            "title": title,
            "link": link,
            "description": description[:200]
        })
    return items

def post_to_bluesky(text: str):
    """Pubblica su Bluesky tramite AT Protocol."""
    # Passo 1: Crea sessione
    session_resp = requests.post(
        "https://bsky.social/xrpc/com.atproto.server.createSession",
        json={
            "identifier": BLUESKY_HANDLE,
            "password": BLUESKY_APP_PASSWORD
        },
        timeout=30
    )
    session_resp.raise_for_status()
    session = session_resp.json()

    # Passo 2: Crea post
    post_resp = requests.post(
        "https://bsky.social/xrpc/com.atproto.repo.createRecord",
        headers={"Authorization": f"Bearer {session['accessJwt']}"},
        json={
            "repo": session["did"],
            "collection": "app.bsky.feed.post",
            "record": {
                "$type": "app.bsky.feed.post",
                "text": text,
                "createdAt": datetime.utcnow().isoformat() + "Z"
            }
        },
        timeout=30
    )
    post_resp.raise_for_status()
    print(f"  Pubblicato su Bluesky: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # Formatta il post social
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky ha un limite di 300 caratteri
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  Pubblicazione fallita: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

Costo: $0. Gira sulla tua macchina o su una GitHub Action gratuita.

#### Automazione 3: Monitor dei Prezzi dei Concorrenti

Sapere all'istante quando un concorrente cambia i prezzi. Niente più controlli manuali ogni settimana.

```python
#!/usr/bin/env python3
"""
price_monitor.py — Monitora le pagine dei prezzi dei concorrenti per rilevare cambiamenti.
Eseguilo ogni 6 ore: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # o Discord, email, ecc.

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # Per il monitoraggio dell'intera pagina; usa selector per elementi specifici
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """Scarica una pagina e restituisce il suo hash del contenuto e un estratto di testo."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # Prendi i primi 500 caratteri di testo visibile per contesto
    excerpt = content[:500]
    return content_hash, excerpt

def load_state(name: str) -> dict:
    state_file = os.path.join(MONITOR_DIR, f"{name}.json")
    try:
        with open(state_file, "r") as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return {}

def save_state(name: str, state: dict):
    os.makedirs(MONITOR_DIR, exist_ok=True)
    state_file = os.path.join(MONITOR_DIR, f"{name}.json")
    with open(state_file, "w") as f:
        json.dump(state, f, indent=2)

def send_alert(message: str):
    """Invia un alert tramite webhook Slack (sostituisci con Discord, email, ecc.)."""
    if not ALERT_WEBHOOK:
        print(f"ALERT (nessun webhook configurato): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  Impossibile scaricare {name}: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"CAMBIO DI PREZZO RILEVATO: {name}\n"
                f"URL: {url}\n"
                f"Cambiato alle: {datetime.utcnow().isoformat()}Z\n"
                f"Hash precedente: {previous_hash[:12]}...\n"
                f"Nuovo hash: {current_hash[:12]}...\n"
                f"Controlla manualmente."
            )
            send_alert(alert_msg)
            print(f"  CAMBIO: {name}")
        else:
            print(f"  Nessun cambio: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### Automazione 4: Report Settimanale dei Ricavi

Ogni lunedì mattina, questo genera un report dai tuoi dati di fatturato e te lo invia per email.

```python
#!/usr/bin/env python3
"""
weekly_report.py — Genera un report settimanale dei ricavi dal tuo foglio di calcolo/database.
Eseguilo i lunedì alle 7: 0 7 * * 1 python3 /path/to/weekly_report.py
"""

import os
import json
import sqlite3
import smtplib
from email.mime.text import MIMEText
from datetime import datetime, timedelta

DB_PATH = os.path.expanduser("~/income/data/revenue.db")
EMAIL_TO = os.environ.get("REPORT_EMAIL", "you@example.com")
SMTP_HOST = os.environ.get("SMTP_HOST", "smtp.gmail.com")
SMTP_PORT = int(os.environ.get("SMTP_PORT", "587"))
SMTP_USER = os.environ.get("SMTP_USER", "")
SMTP_PASS = os.environ.get("SMTP_PASS", "")

def init_db():
    """Crea la tabella dei ricavi se non esiste."""
    conn = sqlite3.connect(DB_PATH)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS transactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            stream TEXT NOT NULL,
            type TEXT NOT NULL CHECK(type IN ('income', 'expense')),
            description TEXT,
            amount REAL NOT NULL
        )
    """)
    conn.commit()
    return conn

def generate_report(conn: sqlite3.Connection) -> str:
    """Genera un report settimanale in testo semplice."""
    today = datetime.now()
    week_ago = today - timedelta(days=7)

    cursor = conn.execute("""
        SELECT stream, type, SUM(amount) as total
        FROM transactions
        WHERE date >= ? AND date <= ?
        GROUP BY stream, type
        ORDER BY stream, type
    """, (week_ago.strftime("%Y-%m-%d"), today.strftime("%Y-%m-%d")))

    rows = cursor.fetchall()

    total_income = 0
    total_expenses = 0
    streams = {}

    for stream, txn_type, amount in rows:
        if stream not in streams:
            streams[stream] = {"income": 0, "expense": 0}
        streams[stream][txn_type] = amount
        if txn_type == "income":
            total_income += amount
        else:
            total_expenses += amount

    report = []
    report.append(f"REPORT SETTIMANALE DEI RICAVI")
    report.append(f"Periodo: {week_ago.strftime('%Y-%m-%d')} - {today.strftime('%Y-%m-%d')}")
    report.append(f"Generato: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    Entrate:  ${data['income']:>10,.2f}")
        report.append(f"    Spese:    ${data['expense']:>10,.2f}")
        report.append(f"    Netto:    ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  TOTALE ENTRATE: ${total_income:>10,.2f}")
    report.append(f"  TOTALE SPESE:   ${total_expenses:>10,.2f}")
    report.append(f"  PROFITTO NETTO: ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  ROI:            {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """Invia il report via email."""
    msg = MIMEText(body, "plain")
    msg["Subject"] = subject
    msg["From"] = SMTP_USER
    msg["To"] = EMAIL_TO

    with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
        server.starttls()
        server.login(SMTP_USER, SMTP_PASS)
        server.sendmail(SMTP_USER, EMAIL_TO, msg.as_string())

def main():
    os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)
    conn = init_db()
    report = generate_report(conn)
    print(report)

    if SMTP_USER:
        send_email(
            f"Report Settimanale dei Ricavi — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nReport inviato per email.")
    conn.close()

if __name__ == "__main__":
    main()
```

#### Automazione 5: Backup Automatico dei Dati dei Clienti

Non perdere mai i deliverable dei clienti. Questo gira ogni notte e conserva 30 giorni di backup.

```bash
#!/bin/bash
# backup_client_data.sh — Backup notturno dei dati dei progetti clienti.
# Cron: 0 3 * * * /home/tuoutente/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# Crea backup compresso
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# Elimina i backup più vecchi del periodo di conservazione
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# Log
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup completato: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# Opzionale: sincronizza su una seconda posizione (disco esterno, altra macchina)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### Timer Systemd per Maggiore Controllo

Se hai bisogno di più di ciò che cron offre — come ordine di dipendenze, limiti sulle risorse, o retry automatici — usa i timer systemd:

```ini
# /etc/systemd/system/income-publisher.service
[Unit]
Description=Publish scheduled content
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=tuoutente
ExecStart=/usr/bin/python3 /home/tuoutente/scripts/scheduled_publisher.py
Environment="CMS_API_KEY=your-key-here"
Environment="CMS_API_URL=https://api.example.com/posts"
# Riavvia in caso di fallimento con backoff esponenziale
Restart=on-failure
RestartSec=60

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/income-publisher.timer
[Unit]
Description=Run content publisher daily at 6 AM

[Timer]
OnCalendar=*-*-* 06:00:00
Persistent=true
# Se la macchina era spenta alle 6, esegui quando torna online
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# Abilita e avvia il timer
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# Controlla lo stato
systemctl list-timers --all | grep income

# Visualizza i log
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Alternativa Windows Task Scheduler

Se non stai usando WSL, Windows Task Scheduler gestisce lo stesso compito. Usa `schtasks` dalla linea di comando o la GUI del Task Scheduler (`taskschd.msc`). La differenza chiave: cron usa una singola espressione, Task Scheduler usa campi separati per trigger, azioni e condizioni. Ogni esempio cron in questa lezione si traduce direttamente — schedula i tuoi script Python allo stesso modo, solo attraverso un'interfaccia diversa.
{? endif ?}

### Tocca a Te

1. Scegli l'automazione più semplice da questa lezione che si applica al tuo flusso di reddito.
2. Implementala. Non "pianifica di implementarla." Scrivi il codice, testalo, schedulalo.
3. Configura il logging così puoi verificare che stia girando. Controlla i log ogni mattina per 3 giorni.
4. Una volta stabile, smetti di controllare giornalmente. Controlla settimanalmente. Questa è automazione.

**Minimo:** Un cron job che gira in modo affidabile entro la fine della giornata.

---

## Lezione 3: Dal Livello 2 al 3 — Pipeline Alimentate da LLM

*"Aggiungi intelligenza alle tue automazioni. Qui è dove una persona inizia a sembrare un team."*

### Il Pattern

Ogni pipeline alimentata da LLM segue la stessa forma:

```
Sorgenti di Input → Ingestione → Elaborazione LLM → Formattazione Output → Consegna (o Coda per Revisione)
```

La magia sta nel passaggio "Elaborazione LLM." Invece di scrivere regole deterministiche per ogni caso possibile, descrivi ciò che vuoi in linguaggio naturale, e l'LLM gestisce le decisioni di giudizio.

### Quando Usare Locale vs API

{? if settings.has_llm ?}
Hai {= settings.llm_provider | fallback("un provider LLM") =} configurato con {= settings.llm_model | fallback("il tuo modello LLM") =}. Questo significa che puoi iniziare a costruire pipeline intelligenti immediatamente. La decisione sotto ti aiuta a scegliere quando usare il tuo setup locale rispetto a un'API per ogni pipeline.
{? else ?}
Non hai ancora un LLM configurato. Le pipeline in questa lezione funzionano sia con modelli locali (Ollama) che con API cloud. Configura almeno uno prima di costruire la tua prima pipeline — Ollama è gratuito e ci vogliono 10 minuti per installarlo.
{? endif ?}

Questa decisione ha un impatto diretto sui tuoi margini:

| Fattore | Locale (Ollama) | API (Claude, GPT) |
|---|---|---|
| **Costo per 1M token** | ~$0,003 (elettricità) | $0,15 - $15,00 |
| **Velocità (token/sec)** | 20-60 (8B su GPU di fascia media) | 50-100+ |
| **Qualità (8B locale vs API)** | Buona per classificazione, estrazione | Migliore per generazione, ragionamento |
| **Privacy** | I dati non lasciano mai la tua macchina | I dati vanno al provider |
| **Uptime** | Dipende dalla tua macchina | 99,9%+ |
| **Capacità batch** | Limitata dalla memoria GPU | Limitata dai rate limit e dal budget |

{? if profile.gpu.exists ?}
Con {= profile.gpu.model | fallback("la tua GPU") =} sulla tua macchina, l'inferenza locale è un'opzione forte. La velocità e la dimensione del modello che puoi eseguire dipendono dalla tua VRAM — controlla cosa ci sta prima di impegnarti in una pipeline solo locale.
{? if computed.has_nvidia ?}
Le GPU NVIDIA ottengono le migliori performance con Ollama grazie all'accelerazione CUDA. Dovresti essere in grado di eseguire modelli da 7-8B parametri comodamente, e possibilmente più grandi a seconda della tua {= profile.gpu.vram | fallback("VRAM disponibile") =}.
{? endif ?}
{? else ?}
Senza una GPU dedicata, l'inferenza locale sarà più lenta (solo CPU). Funziona comunque per piccoli job batch e compiti di classificazione, ma per qualsiasi cosa time-sensitive o ad alto volume, un modello API sarà più pratico.
{? endif ?}

**Regole pratiche:**
- **Alto volume, barra di qualità più bassa** (classificazione, estrazione, tagging) → Locale
- **Basso volume, qualità critica** (contenuti rivolti ai clienti, analisi complessa) → API
- **Dati sensibili** (info clienti, dati proprietari) → Locale, sempre
- **Più di 10.000 elementi/mese** → Locale fa risparmiare davvero

**Confronto costi mensili per una pipeline tipica:**

```
Elaborazione di 5.000 elementi/mese, ~500 token per elemento:

Locale (Ollama, llama3.1:8b):
  2.500.000 token × $0,003/1M = $0,0075/mese
  Praticamente gratis.

API (GPT-4o-mini):
  2.500.000 token input × $0,15/1M = $0,375
  2.500.000 token output × $0,60/1M = $1,50
  Totale: ~$1,88/mese
  Economico, ma 250x di più rispetto al locale.

API (Claude 3.5 Sonnet):
  2.500.000 token input × $3,00/1M = $7,50
  2.500.000 token output × $15,00/1M = $37,50
  Totale: ~$45/mese
  La qualità è eccellente, ma 6.000x di più rispetto al locale.
```

Per pipeline di classificazione ed estrazione, la differenza di qualità tra un modello locale 8B ben promptato e un modello API di frontiera è spesso trascurabile. Testa entrambi. Usa quello più economico che soddisfa la tua barra di qualità.

{@ insight cost_projection @}

### Pipeline 1: Generatore di Contenuti per Newsletter

Questa è l'automazione LLM più comune per gli sviluppatori con reddito basato sui contenuti. I feed RSS entrano, una bozza di newsletter esce.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — Ingerisci feed RSS, riassumi con LLM, genera bozza di newsletter.
Eseguilo giornalmente: 0 5 * * * python3 /path/to/newsletter_pipeline.py

Questa pipeline:
1. Scarica nuovi articoli da molteplici feed RSS
2. Invia ciascuno a un LLM locale per la sintesi
3. Li classifica per rilevanza rispetto al tuo pubblico
4. Genera una bozza di newsletter formattata
5. Salva la bozza per la tua revisione (spendi 10 min a revisionare, non 2 ore a curare)
"""

import os
import json
import hashlib
import requests
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

FEEDS = [
    "https://hnrss.org/frontpage",
    "https://blog.rust-lang.org/feed.xml",
    "https://this-week-in-rust.org/atom.xml",
    # Aggiungi i tuoi feed di nicchia qui
]

SEEN_FILE = os.path.expanduser("~/income/newsletter/seen.json")
DRAFTS_DIR = os.path.expanduser("~/income/newsletter/drafts")
AUDIENCE_DESCRIPTION = "Rust developers interested in systems programming, AI/ML, and developer tooling"

def load_seen() -> set:
    try:
        with open(SEEN_FILE, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_seen(seen: set):
    os.makedirs(os.path.dirname(SEEN_FILE), exist_ok=True)
    with open(SEEN_FILE, "w") as f:
        json.dump(sorted(seen), f)

def fetch_feed(url: str) -> list:
    """Parsa un feed RSS/Atom e restituisce gli articoli."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # Gestisce sia feed RSS che Atom
        for item in root.findall(".//{http://www.w3.org/2005/Atom}entry") or root.findall(".//item"):
            title = (item.findtext("{http://www.w3.org/2005/Atom}title")
                     or item.findtext("title") or "")
            link = (item.find("{http://www.w3.org/2005/Atom}link")
                    or item.find("link"))
            if link is not None:
                link_url = link.get("href", "") or link.text or ""
            else:
                link_url = ""

            description = (item.findtext("{http://www.w3.org/2005/Atom}summary")
                           or item.findtext("description") or "")

            article_id = hashlib.md5(f"{title}{link_url}".encode()).hexdigest()

            articles.append({
                "id": article_id,
                "title": title.strip(),
                "link": link_url.strip(),
                "description": description[:500].strip(),
                "source": url
            })
        return articles
    except Exception as e:
        print(f"  Impossibile scaricare {url}: {e}")
        return []

def llm_process(prompt: str) -> str:
    """Invia un prompt all'LLM locale e ottieni la risposta."""
    payload = {
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {
            "temperature": 0.3,
            "num_ctx": 4096
        }
    }

    try:
        resp = requests.post(OLLAMA_URL, json=payload, timeout=120)
        resp.raise_for_status()
        return resp.json().get("response", "").strip()
    except Exception as e:
        print(f"  Errore LLM: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """Usa l'LLM per valutare la rilevanza e generare un riassunto."""
    prompt = f"""You are a newsletter curator for an audience of: {AUDIENCE_DESCRIPTION}

Article title: {article['title']}
Article excerpt: {article['description']}

Respond in this exact JSON format (no other text):
{{
  "relevance": <1-10 integer, 10 = extremely relevant to the audience>,
  "summary": "<2-3 sentence summary focusing on why this matters to the audience>",
  "category": "<one of: tool, technique, news, opinion, tutorial>"
}}"""

    result_text = llm_process(prompt)

    try:
        # Prova a parsare il JSON dall'output dell'LLM
        # Gestisce i casi in cui l'LLM avvolge in blocchi markdown
        cleaned = result_text.strip()
        if cleaned.startswith("```"):
            cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
        result = json.loads(cleaned)
        article["relevance"] = result.get("relevance", 5)
        article["summary"] = result.get("summary", article["description"][:200])
        article["category"] = result.get("category", "news")
    except (json.JSONDecodeError, KeyError):
        article["relevance"] = 5
        article["summary"] = article["description"][:200]
        article["category"] = "news"

    return article

def generate_newsletter(articles: list) -> str:
    """Formatta gli articoli valutati in una bozza di newsletter."""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# La Tua Newsletter — {today}")
    newsletter.append("")
    newsletter.append("*[LA TUA INTRO QUI — Scrivi 2-3 frasi sul tema di questa settimana]*")
    newsletter.append("")

    section_titles = {
        "tool": "Strumenti e Release",
        "technique": "Tecniche e Pattern",
        "news": "Notizie dal Settore",
        "tutorial": "Tutorial e Guide",
        "opinion": "Prospettive"
    }

    for cat, title in section_titles.items():
        items = sections.get(cat, [])
        if not items:
            continue

        newsletter.append(f"## {title}")
        newsletter.append("")

        for item in items:
            newsletter.append(f"**[{item['title']}]({item['link']})**")
            newsletter.append(f"{item['summary']}")
            newsletter.append("")

    newsletter.append("---")
    newsletter.append("*[LA TUA CHIUSURA — Su cosa stai lavorando? Cosa dovrebbero tenere d'occhio i lettori?]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("Scaricamento feed...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} nuovi articoli")

    if not all_articles:
        print("Nessun nuovo articolo. Salto.")
        return

    print(f"\nValutazione di {len(all_articles)} articoli con LLM...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # Filtra solo gli articoli rilevanti e ordina per punteggio
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # Prendi i primi 10
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} articoli hanno superato la soglia di rilevanza (>= 6/10)")

    # Genera la bozza della newsletter
    draft = generate_newsletter(top_articles)

    # Salva la bozza
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nBozza salvata: {draft_path}")
    print("Revisionala, aggiungi la tua intro/chiusura, e invia.")

if __name__ == "__main__":
    main()
```

**Quanto costa:**
- Elaborare 50 articoli/giorno con un modello locale 8B: ~$0/mese
- Il tuo tempo: 10 minuti a revisionare la bozza contro 2 ore a curare manualmente
- Tempo risparmiato a settimana: ~10 ore se fai una newsletter settimanale

### Pipeline 2: Ricerca Clienti e Report di Intelligence

Questa pipeline scrappa dati pubblici, li analizza con un LLM, e produce un report che puoi vendere.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — Analizza dati pubblici su aziende/prodotti e genera report di intelligence.
Questo è un servizio che puoi vendere: $200-500 per report personalizzato.

Uso: python3 research_pipeline.py "Nome Azienda" "loro-sito.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# Usa un modello più grande per la qualità sui report a pagamento
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# Oppure usa API per qualità rivolta ai clienti:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """Instrada al modello locale o API in base alla configurazione."""
    if USE_API:
        return llm_query_api(prompt, max_tokens)
    return llm_query_local(prompt, max_tokens)

def llm_query_local(prompt: str, max_tokens: int = 2000) -> str:
    resp = requests.post(OLLAMA_URL, json={
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {"temperature": 0.4, "num_ctx": 8192}
    }, timeout=180)
    resp.raise_for_status()
    return resp.json().get("response", "")

def llm_query_api(prompt: str, max_tokens: int = 2000) -> str:
    resp = requests.post(
        "https://api.anthropic.com/v1/messages",
        headers={
            "x-api-key": ANTHROPIC_KEY,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json"
        },
        json={
            "model": "claude-sonnet-4-20250514",
            "max_tokens": max_tokens,
            "messages": [{"role": "user", "content": prompt}]
        },
        timeout=120
    )
    resp.raise_for_status()
    return resp.json()["content"][0]["text"]

def gather_public_data(company: str, domain: str) -> dict:
    """Raccoglie dati pubblicamente disponibili su un'azienda."""
    data = {"company": company, "domain": domain}

    # Controlla se il dominio è raggiungibile e ottieni info di base
    try:
        resp = requests.get(
            f"https://{domain}",
            timeout=15,
            headers={"User-Agent": "Mozilla/5.0 (ResearchBot/1.0)"},
            allow_redirects=True
        )
        data["website_status"] = resp.status_code
        data["website_title"] = ""
        if "<title>" in resp.text.lower():
            start = resp.text.lower().index("<title>") + 7
            end = resp.text.lower().index("</title>")
            data["website_title"] = resp.text[start:end].strip()
    except Exception as e:
        data["website_status"] = f"Error: {e}"

    # Controlla la presenza su GitHub
    try:
        gh_resp = requests.get(
            f"https://api.github.com/orgs/{company.lower().replace(' ', '-')}",
            timeout=10,
            headers={"Accept": "application/vnd.github.v3+json"}
        )
        if gh_resp.status_code == 200:
            gh_data = gh_resp.json()
            data["github_repos"] = gh_data.get("public_repos", 0)
            data["github_followers"] = gh_data.get("followers", 0)
    except Exception:
        pass

    return data

def generate_report(company: str, domain: str, data: dict) -> str:
    """Genera un report di analisi usando l'LLM."""
    context = json.dumps(data, indent=2)

    analysis_prompt = f"""You are a technology market analyst. Generate a concise research report about {company} ({domain}).

Available data:
{context}

Generate a report with these sections:
1. Company Overview (2-3 sentences based on available data)
2. Technical Stack Assessment (what can be inferred from their public presence)
3. Market Position (based on GitHub activity, web presence)
4. Opportunities (what services or products could someone offer TO this company)
5. Risks (any red flags for doing business with them)

Keep each section to 3-5 bullet points. Be specific and data-driven.
Format as clean markdown."""

    return llm_query(analysis_prompt, max_tokens=2000)

def main():
    if len(sys.argv) < 3:
        print("Uso: python3 research_pipeline.py 'Nome Azienda' 'dominio.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"Ricerca su: {company} ({domain})")
    print(f"Usando: {'API (Claude)' if USE_API else 'Locale (Ollama)'}")

    print("Raccolta dati pubblici...")
    data = gather_public_data(company, domain)

    print("Generazione analisi...")
    report = generate_report(company, domain, data)

    # Assembla il report finale
    final_report = f"""# Report di Ricerca: {company}

**Generato:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**Dominio:** {domain}
**Modello di analisi:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*Questo report è stato generato utilizzando esclusivamente dati pubblicamente disponibili.
Nessun dato proprietario o privato è stato accesso.*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nReport salvato: {filepath}")
    print(f"Costo API: ~${'0,02-0,05' if USE_API else '0,00'}")

if __name__ == "__main__":
    main()
```

**Modello di business:** Addebita $200-500 per report di ricerca personalizzato. Il tuo costo: $0,05 in chiamate API e 15 minuti di revisione. Puoi produrre 3-4 report all'ora una volta che la pipeline è stabile.

### Pipeline 3: Monitor dei Segnali di Mercato

Questa è la pipeline che ti dice cosa costruire dopo. Monitora molteplici sorgenti, classifica i segnali e ti avvisa quando un'opportunità supera la tua soglia.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — Monitora sorgenti pubbliche per opportunità di mercato.
Eseguilo ogni 2 ore: 0 */2 * * * python3 /path/to/signal_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

DATA_DIR = os.path.expanduser("~/income/signals")
ALERTS_FILE = os.path.join(DATA_DIR, "alerts.jsonl")
SEEN_FILE = os.path.join(DATA_DIR, "seen.json")

SLACK_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# La definizione della tua nicchia — l'LLM la usa per valutare la rilevanza
MY_NICHE = """
I build developer tools and local-first software. I know Rust, TypeScript, and Python.
I sell digital products (templates, starter kits), consulting, and a niche newsletter.
My audience is developers interested in privacy, local AI, and desktop apps.
"""

def load_seen() -> set:
    try:
        with open(SEEN_FILE, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_seen(seen: set):
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(SEEN_FILE, "w") as f:
        json.dump(sorted(seen), f)

def fetch_hn_top(limit: int = 30) -> list:
    """Scarica le storie top di Hacker News."""
    try:
        ids_resp = requests.get(
            "https://hacker-news.firebaseio.com/v0/topstories.json",
            timeout=15
        )
        ids = ids_resp.json()[:limit]

        items = []
        for item_id in ids:
            item_resp = requests.get(
                f"https://hacker-news.firebaseio.com/v0/item/{item_id}.json",
                timeout=10
            )
            data = item_resp.json()
            if data and data.get("type") == "story":
                items.append({
                    "id": f"hn-{item_id}",
                    "source": "hackernews",
                    "title": data.get("title", ""),
                    "url": data.get("url", f"https://news.ycombinator.com/item?id={item_id}"),
                    "score": data.get("score", 0),
                    "comments": data.get("descendants", 0)
                })
        return items
    except Exception as e:
        print(f"  Scaricamento HN fallito: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """Usa l'LLM per classificare un segnale per opportunità di mercato."""
    prompt = f"""You are a market analyst helping a developer find income opportunities.

Developer profile:
{MY_NICHE}

Signal:
- Source: {item['source']}
- Title: {item['title']}
- URL: {item.get('url', 'N/A')}
- Engagement: score={item.get('score', 'N/A')}, comments={item.get('comments', 'N/A')}

Classify this signal. Respond in this exact JSON format only:
{{
  "opportunity_score": <0-10, 10 = perfect opportunity for this developer>,
  "opportunity_type": "<one of: product_gap, education_gap, market_shift, tool_need, community_demand, not_relevant>",
  "reasoning": "<one sentence explaining why this is or isn't an opportunity>",
  "action": "<specific next step if score >= 7, or 'none'>"
}}"""

    try:
        resp = requests.post(OLLAMA_URL, json={
            "model": MODEL,
            "prompt": prompt,
            "stream": False,
            "options": {"temperature": 0.2, "num_ctx": 4096}
        }, timeout=120)
        resp.raise_for_status()

        result_text = resp.json().get("response", "").strip()
        if result_text.startswith("```"):
            result_text = result_text.split("\n", 1)[1].rsplit("```", 1)[0]

        classification = json.loads(result_text)
        item.update(classification)
    except (json.JSONDecodeError, Exception) as e:
        item["opportunity_score"] = 0
        item["opportunity_type"] = "not_relevant"
        item["reasoning"] = f"Classificazione fallita: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """Invia un alert per opportunità ad alto punteggio."""
    msg = (
        f"OPPORTUNITA' RILEVATA (punteggio: {item['opportunity_score']}/10)\n"
        f"Tipo: {item['opportunity_type']}\n"
        f"Titolo: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Perché: {item['reasoning']}\n"
        f"Azione: {item['action']}"
    )

    # Registra su file
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # Invia a Slack/Discord
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERT: {msg}")

def main():
    seen = load_seen()

    # Scarica dalle sorgenti
    print("Scaricamento segnali...")
    items = fetch_hn_top(30)
    # Aggiungi altre sorgenti qui: Reddit, feed RSS, GitHub trending, ecc.

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} nuovi segnali da classificare")

    # Classifica ogni segnale
    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("Fatto.")

if __name__ == "__main__":
    main()
```

**Cosa fa nella pratica:** Ricevi una notifica Slack 2-3 volte a settimana che dice qualcosa come "OPPORTUNITA': Nuovo framework rilasciato senza starter kit — potresti costruirne uno questo weekend." Quel segnale, agire su di esso prima degli altri, è come resti in vantaggio.

> **Parliamoci chiaro:** La qualità degli output di queste pipeline dipende interamente dai tuoi prompt e dalla definizione della tua nicchia. Se la tua nicchia è vaga ("Sono uno sviluppatore web"), l'LLM segnalerà tutto. Se è specifica ("Costruisco app desktop Tauri per il mercato degli sviluppatori privacy-first"), sarà chirurgicamente preciso. Dedica 30 minuti a definire bene la tua nicchia. È l'input con il più alto effetto leva per ogni pipeline che costruirai.

### Tocca a Te

{? if stack.contains("python") ?}
Buone notizie: gli esempi di pipeline sopra sono già nel tuo linguaggio principale. Puoi copiarli direttamente e iniziare ad adattarli. Concentrati sulla definizione della nicchia e sui prompt — è da lì che viene il 90% della qualità dell'output.
{? else ?}
Gli esempi sopra usano Python per portabilità, ma i pattern funzionano in qualsiasi linguaggio. Se preferisci costruire in {= stack.primary | fallback("il tuo stack principale") =}, i pezzi chiave da replicare sono: client HTTP per lo scaricamento di RSS/API, parsing JSON per le risposte dell'LLM, e I/O su file per la gestione dello stato. L'interazione con l'LLM è solo un HTTP POST a Ollama o a un'API cloud.
{? endif ?}

1. Scegli una delle tre pipeline sopra (newsletter, ricerca o monitor dei segnali).
2. Adattala alla tua nicchia. Cambia i feed, la descrizione del pubblico, i criteri di classificazione.
3. Eseguila manualmente 3 volte per testare la qualità dell'output.
4. Perfeziona i prompt finché l'output non è utile senza pesanti modifiche.
5. Schedulala con cron.

**Obiettivo:** Una pipeline alimentata da LLM in esecuzione schedulata entro 48 ore dalla lettura di questa lezione.

---

## Lezione 4: Dal Livello 3 al 4 — Sistemi Basati su Agenti

*"Un agente è semplicemente un loop che osserva, decide e agisce. Costruiscine uno."*

### Cosa Significa Davvero "Agente" nel 2026

Togli l'hype. Un agente è un programma che:

1. **Osserva** — legge qualche input o stato
2. **Decide** — usa un LLM per determinare cosa fare
3. **Agisce** — esegue la decisione
4. **Ripete** — torna al passo 1

Tutto qui. La differenza tra una pipeline (Livello 3) e un agente (Livello 4) è che l'agente cicla. Agisce sul proprio output. Gestisce compiti multi-step dove il passo successivo dipende dal risultato del precedente.

Una pipeline elabora gli elementi uno alla volta attraverso una sequenza fissa. Un agente naviga una sequenza imprevedibile in base a ciò che incontra.

### Server MCP che Servono i Clienti

Un server MCP è uno dei sistemi più pratici simili ad agenti che puoi costruire. Espone strumenti che un agente IA (Claude Code, Cursor, ecc.) può chiamare per conto dei tuoi clienti.

{? if stack.contains("typescript") ?}
L'esempio di server MCP sotto usa TypeScript — esattamente nel tuo terreno. Puoi estenderlo con il tuo tooling TypeScript esistente e deployarlo insieme ai tuoi altri servizi Node.js.
{? endif ?}

Ecco un esempio reale: un server MCP che risponde alle domande dei clienti dalla documentazione del tuo prodotto.

```typescript
// mcp-docs-server/src/index.ts
// Un server MCP che risponde alle domande dalla tua documentazione.
// I tuoi clienti puntano il loro Claude Code a questo server e ottengono risposte istantanee.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// Carica la tua documentazione in memoria all'avvio
const DOCS_DIR = process.env.DOCS_DIR || "./docs";

interface DocChunk {
  file: string;
  section: string;
  content: string;
}

function loadDocs(): DocChunk[] {
  const chunks: DocChunk[] = [];
  const files = fs.readdirSync(DOCS_DIR, { recursive: true }) as string[];

  for (const file of files) {
    if (!file.endsWith(".md")) continue;

    const fullPath = path.join(DOCS_DIR, file);
    const content = fs.readFileSync(fullPath, "utf-8");

    // Dividi per intestazioni per una ricerca migliore
    const sections = content.split(/^## /m);
    for (const section of sections) {
      if (section.trim().length < 20) continue;
      const firstLine = section.split("\n")[0].trim();
      chunks.push({
        file: file,
        section: firstLine,
        content: section.trim().slice(0, 2000),
      });
    }
  }

  return chunks;
}

function searchDocs(query: string, docs: DocChunk[], limit: number = 5): DocChunk[] {
  // Ricerca semplice per parole chiave — sostituisci con ricerca vettoriale per la produzione
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // Bonus per le corrispondenze nel titolo
      if (chunk.section.toLowerCase().includes(word)) score += 2;
    }
    return { chunk, score };
  });

  return scored
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((s) => s.chunk);
}

// Inizializzazione
const docs = loadDocs();
console.error(`Caricati ${docs.length} chunk di documentazione da ${DOCS_DIR}`);

const server = new McpServer({
  name: "product-docs",
  version: "1.0.0",
});

server.tool(
  "search_docs",
  "Search the product documentation for information about a topic",
  {
    query: z.string().describe("The question or topic to search for"),
    max_results: z.number().optional().default(5).describe("Maximum results to return"),
  },
  async ({ query, max_results }) => {
    const results = searchDocs(query, docs, max_results);

    if (results.length === 0) {
      return {
        content: [
          {
            type: "text" as const,
            text: `No documentation found for: "${query}". Try different keywords or check the docs at https://yourdomain.com/docs`,
          },
        ],
      };
    }

    const formatted = results
      .map(
        (r, i) =>
          `### Result ${i + 1}: ${r.section}\n**File:** ${r.file}\n\n${r.content}`
      )
      .join("\n\n---\n\n");

    return {
      content: [
        {
          type: "text" as const,
          text: `Found ${results.length} relevant sections:\n\n${formatted}`,
        },
      ],
    };
  }
);

server.tool(
  "list_topics",
  "List all available documentation topics",
  {},
  async () => {
    const topics = [...new Set(docs.map((d) => d.section))].sort();
    return {
      content: [
        {
          type: "text" as const,
          text: `Available documentation topics:\n\n${topics.map((t) => `- ${t}`).join("\n")}`,
        },
      ],
    };
  }
);

// Avvia il server
const transport = new StdioServerTransport();
server.connect(transport);
```

```json
// mcp-docs-server/package.json
{
  "name": "product-docs-mcp",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.0.0",
    "zod": "^3.22.0"
  },
  "devDependencies": {
    "typescript": "^5.3.0",
    "@types/node": "^20.0.0"
  }
}
```

**Modello di business:** Dai questo server MCP ai tuoi clienti come parte del tuo prodotto. Ottengono risposte istantanee alle loro domande senza aprire ticket di supporto. Tu spendi meno tempo sul supporto. Tutti vincono.

Per il premium: addebita $9-29/mese per una versione hosted con ricerca vettoriale, documentazione versionata e analytics su cosa chiedono i clienti.

### Elaborazione Automatizzata del Feedback Clienti

Questo agente legge il feedback dei clienti (da email, ticket di supporto o un form), lo classifica e crea bozze di risposte e ticket per le funzionalità.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — Elabora il feedback dei clienti in elementi classificati e azionabili.
Il pattern "bozza IA, approvazione umana".

Eseguilo ogni ora: 0 * * * * python3 /path/to/feedback_agent.py
"""

import os
import json
import requests
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

INBOX_DIR = os.path.expanduser("~/income/feedback/inbox")
PROCESSED_DIR = os.path.expanduser("~/income/feedback/processed")
REVIEW_DIR = os.path.expanduser("~/income/feedback/review")

def llm(prompt: str) -> str:
    resp = requests.post(OLLAMA_URL, json={
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "options": {"temperature": 0.3, "num_ctx": 4096}
    }, timeout=120)
    resp.raise_for_status()
    return resp.json().get("response", "").strip()

def process_feedback(feedback: dict) -> dict:
    """Classifica il feedback e genera una bozza di risposta."""

    classify_prompt = f"""Classify this customer feedback and draft a response.

Customer: {feedback.get('from', 'Unknown')}
Subject: {feedback.get('subject', 'No subject')}
Message: {feedback.get('body', '')}

Respond in this exact JSON format:
{{
  "category": "<bug_report | feature_request | support_question | praise | complaint | spam>",
  "urgency": "<low | medium | high | critical>",
  "sentiment": "<positive | neutral | negative | angry>",
  "summary": "<one sentence summary of the feedback>",
  "draft_response": "<professional, helpful draft response (2-4 sentences)>",
  "action_items": ["<list of specific actions to take>"],
  "needs_human": <true if this requires personal attention, false if draft response is sufficient>
}}"""

    result_text = llm(classify_prompt)

    try:
        cleaned = result_text.strip()
        if cleaned.startswith("```"):
            cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
        classification = json.loads(cleaned)
        feedback.update(classification)
    except (json.JSONDecodeError, Exception):
        feedback["category"] = "support_question"
        feedback["urgency"] = "medium"
        feedback["needs_human"] = True
        feedback["draft_response"] = "[Classificazione fallita — revisione manuale necessaria]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"Nessuna directory inbox: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("Nessun nuovo feedback.")
        return

    print(f"Elaborazione di {len(inbox_files)} elementi di feedback...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  Salto {filepath.name}: {e}")
            continue

        print(f"  Elaborazione: {feedback.get('subject', 'Nessun oggetto')[:50]}...")
        processed = process_feedback(feedback)

        # Salva la versione elaborata
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # Aggiungi alla coda di revisione
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Sconosciuto"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # Sposta l'originale fuori dall'inbox
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # Scrivi la coda di revisione
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # Riepilogo
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nElaborati: {len(review_queue)}")
    print(f"Critici: {critical}")
    print(f"Necessitano la tua attenzione: {needs_human}")
    print(f"Coda di revisione: {review_path}")

if __name__ == "__main__":
    main()
```

**Come funziona nella pratica:**
1. I clienti inviano feedback (tramite form, email o sistema di supporto)
2. Il feedback arriva come file JSON nella directory inbox
3. L'agente elabora ciascuno: classifica, riassume, bozza una risposta
4. Tu apri la coda di revisione una o due volte al giorno
5. Per gli elementi semplici (elogi, domande base con buone bozze di risposta), approvi la bozza
6. Per gli elementi complessi (bug, clienti arrabbiati), scrivi una risposta personale
7. Tempo netto: 15 minuti al giorno invece di 2 ore

### Il Pattern Bozza IA, Approvazione Umana

Questo pattern è il nucleo dell'automazione pratica di Livello 4. L'agente gestisce il lavoro pesante. Tu gestisci le decisioni di giudizio.

```
              ┌─────────────┐
              │ L'agente     │
              │ prepara la   │
              │ bozza        │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ Coda di      │
              │ Revisione    │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ Invio     │ │Modi-│ │Escalation│
    │ automatico│ │fica │ │(complesso)│
    │ (routine) │ │+invio│ │         │
    └───────────┘ └─────┘ └─────────┘
```

**Regole su cosa l'agente gestisce interamente vs cosa revisioni:**

| L'agente gestisce interamente (nessuna revisione) | Tu revisioni prima dell'invio |
|---|---|
| Ricevute di conferma ("Abbiamo ricevuto il tuo messaggio") | Risposte a clienti arrabbiati |
| Aggiornamenti di stato ("La tua richiesta è in elaborazione") | Prioritizzazione richieste funzionalità |
| Risposte FAQ (corrispondenza esatta) | Qualsiasi cosa che coinvolga denaro (rimborsi, prezzi) |
| Classificazione e cancellazione spam | Segnalazioni bug (devi verificare) |
| Logging e categorizzazione interna | Qualsiasi cosa che non hai mai visto prima |

> **Errore comune:** Lasciare che l'agente risponda ai clienti autonomamente dal primo giorno. Non farlo. Inizia con l'agente che prepara la bozza di tutto, tu che approvi tutto. Dopo una settimana, lascia che invii automaticamente le conferme di ricezione. Dopo un mese, lascia che invii automaticamente le risposte FAQ. Costruisci la fiducia in modo incrementale — con te stesso e con i tuoi clienti.

### Tocca a Te

1. Scegli uno: costruisci il server MCP per la documentazione OPPURE l'agente di elaborazione feedback.
2. Adattalo al tuo prodotto/servizio. Se non hai ancora clienti, usa il monitor dei segnali dalla Lezione 3 come tuo "cliente" — elabora il suo output attraverso il pattern dell'agente di feedback.
3. Eseguilo manualmente 10 volte con input diversi.
4. Misura: quale percentuale di output è utilizzabile senza modifiche? Questo è il tuo punteggio di qualità dell'automazione. Punta al 70%+ prima di schedulare.

---

## Lezione 5: Il Principio dello Human-in-the-Loop

*"L'automazione totale è una trappola. L'automazione parziale è un superpotere."*

### Perché l'80% di Automazione Batte il 100%

C'è una ragione specifica e misurabile per cui non dovresti mai automatizzare completamente i processi che interagiscono con i clienti: il costo di un output sbagliato è asimmetrico.

Un buon output automatizzato ti fa risparmiare 5 minuti.
Un output automatizzato sbagliato ti costa un cliente, un reclamo pubblico, un rimborso, o un danno reputazionale che richiede mesi per essere recuperato.

La matematica:

```
100% automazione:
  1.000 output/mese × 95% qualità = 950 buoni + 50 sbagliati
  50 output sbagliati × $50 costo medio (rimborso + supporto + reputazione) = $2.500/mese di danni

80% automazione + 20% revisione umana:
  800 output gestiti automaticamente, 200 revisionati da umano
  800 × 95% qualità = 760 buoni + 40 sbagliati auto
  200 × 99% qualità = 198 buoni + 2 sbagliati umani
  42 sbagliati totali × $50 = $2.100/mese di danni
  MA: ne intercetti 38 prima che raggiungano i clienti

  Output sbagliati effettivi che raggiungono i clienti: ~4
  Danno effettivo: ~$200/mese
```

Questa è una riduzione di 12x del costo dei danni. Il tuo tempo a revisionare 200 output (forse 2 ore) ti fa risparmiare $2.300/mese in danni.

### Non Automatizzare Mai Completamente Queste Cose

Alcune cose dovrebbero sempre avere un umano nel loop, indipendentemente da quanto l'IA diventi brava:

1. **Comunicazione rivolta ai clienti** — Un'email formulata male può far perdere un cliente per sempre. Una risposta generica, chiaramente generata dall'IA, può erodere la fiducia. Revisionala.

2. **Transazioni finanziarie** — Rimborsi, cambi di prezzo, fatturazione. Revisiona sempre. Il costo di un errore sono soldi veri.

3. **Contenuti pubblicati con il tuo nome** — La tua reputazione si costruisce negli anni e può essere distrutta con un singolo post sbagliato. Dieci minuti di revisione sono un'assicurazione a buon mercato.

4. **Output legale o di compliance** — Qualsiasi cosa che tocchi contratti, policy sulla privacy, termini di servizio. L'IA fa errori legali dal suono sicuro di sé.

5. **Decisioni su assunzioni o persone** — Se mai esternalizzi, non lasciare mai che un'IA prenda la decisione finale su chi lavorare.

### Debito di Automazione

{@ mirror automation_risk_profile @}

Il debito di automazione è peggio del debito tecnico perché è invisibile finché non esplode.

**Come appare il debito di automazione:**
- Un bot per i social media che pubblica all'orario sbagliato perché il fuso orario è cambiato
- Una pipeline per la newsletter che include un link rotto da 3 settimane perché nessuno controlla
- Un monitor dei prezzi che ha smesso di funzionare quando il concorrente ha ridisegnato la propria pagina
- Uno script di backup che fallisce silenziosamente perché il disco si è riempito

**Come prevenirlo:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — Monitora tutte le tue automazioni per fallimenti silenti.
Eseguilo ogni mattina: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# Definisci gli output attesi da ogni automazione
AUTOMATIONS = [
    {
        "name": "Pipeline Newsletter",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # Dovrebbe produrre almeno giornalmente
    },
    {
        "name": "Social Poster",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # Controlla il file direttamente
        "max_age_hours": 2,  # Dovrebbe aggiornarsi ogni 30 min
    },
    {
        "name": "Monitor Concorrenti",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # Dovrebbe girare ogni 6 ore
    },
    {
        "name": "Backup Clienti",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # Dovrebbe girare ogni notte
    },
    {
        "name": "Server Ollama",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """Controlla se l'automazione ha prodotto output recente."""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # Controlla i file recenti che corrispondono al pattern
        p = Path(path)
        if not p.exists():
            return False, f"Directory non trovata: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"Nessun file corrispondente a {automation['pattern']} in {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # Controlla il file direttamente
        if not os.path.exists(path):
            return False, f"File non trovato: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Ultimo output {age.total_seconds()/3600:.1f}h fa (max: {automation['max_age_hours']}h)"

    return True, f"OK (ultimo output {age.total_seconds()/3600:.1f}h fa)"

def check_http(automation: dict) -> tuple[bool, str]:
    """Controlla se un servizio sta rispondendo."""
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"OK (HTTP {resp.status_code})"
        return False, f"Stato inatteso: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"Connessione fallita: {e}"

def send_alert(message: str):
    if ALERT_WEBHOOK:
        import requests
        requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)
    print(message)

def main():
    failures = []

    for automation in AUTOMATIONS:
        check_type = automation["check_type"]

        if check_type == "file_freshness":
            ok, msg = check_file_freshness(automation)
        elif check_type == "http":
            ok, msg = check_http(automation)
        else:
            ok, msg = False, f"Tipo di check sconosciuto: {check_type}"

        status = "OK" if ok else "FAIL"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"CONTROLLO SALUTE AUTOMAZIONI — {len(failures)} FALLIMENTO/I\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nControlla i log e correggi prima che si accumulino."
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

Eseguilo ogni mattina. Quando un'automazione si rompe silenziosamente (e succederà), lo saprai entro 24 ore invece di 3 settimane.

### Costruire Code di Revisione

La chiave per rendere efficiente lo human-in-the-loop è raggruppare la revisione in batch. Non revisionare un elemento alla volta man mano che arrivano. Mettili in coda e revisiona a blocchi.

```python
#!/usr/bin/env python3
"""
review_queue.py — Una semplice coda di revisione per gli output generati dall'IA.
Revisiona una o due volte al giorno invece di controllare costantemente.
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """Aggiungi un elemento alla coda di revisione."""
    os.makedirs(QUEUE_DIR, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    filename = f"{timestamp}-{item_type}.json"
    filepath = os.path.join(QUEUE_DIR, filename)

    item = {
        "type": item_type,
        "created_at": datetime.utcnow().isoformat() + "Z",
        "status": "pending",
        "content": content
    }

    with open(filepath, "w") as f:
        json.dump(item, f, indent=2)

def review_queue():
    """Mostra tutti gli elementi in attesa di revisione."""
    if not os.path.isdir(QUEUE_DIR):
        print("La coda è vuota.")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("La coda è vuota.")
        return

    print(f"\n{'='*60}")
    print(f"CODA DI REVISIONE — {len(pending)} elementi in attesa")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    Articoli: {content.get('article_count', '?')}")
            print(f"    Bozza: {content.get('draft_path', 'sconosciuto')}")
        elif item["type"] == "customer_response":
            print(f"    A: {content.get('customer', 'sconosciuto')}")
            print(f"    Bozza: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    Testo: {content.get('text', '')[:100]}...")

        print(f"    Azioni: [a]pprova  [e]dita  [r]ifiuta  [s]alta")
        print()

    # In un'implementazione reale, aggiungeresti qui l'input interattivo
    # Per l'elaborazione batch, leggi le decisioni da un file o da una semplice CLI

if __name__ == "__main__":
    review_queue()
```

**L'abitudine della revisione:** Controlla la tua coda di revisione alle 8 e alle 16. Due sessioni, 10-15 minuti ciascuna. Tutto il resto gira autonomamente tra le revisioni.

> **Parliamoci chiaro:** Considera cosa succede quando salti la revisione umana: automatizzi completamente la tua newsletter, l'LLM inizia a inserire link allucinati a pagine che non esistono, e gli iscritti se ne accorgono prima di te. Perdi una fetta della tua lista e ci vogliono mesi per ricostruire la fiducia. Al contrario, lo sviluppatore che automatizza l'80% dello stesso processo — l'LLM cura e prepara la bozza, lui spende 10 minuti a revisionare — intercetta quelle allucinazioni prima che vengano inviate. La differenza non è l'automazione. È il passaggio di revisione.

### Tocca a Te

1. Configura lo script `automation_healthcheck.py` per qualsiasi automazione tu abbia costruito nelle Lezioni 2 e 3. Schedulalo per girare ogni mattina.
2. Implementa una coda di revisione per l'output della tua automazione a più alto rischio (qualsiasi cosa rivolta ai clienti).
3. Impegnati a controllare la coda di revisione due volte al giorno per una settimana. Registra quanti elementi approvi senza modifiche, quanti modifichi e quanti rifiuti. Questi dati ti dicono quanto è buona davvero la tua automazione.

---

## Lezione 6: Ottimizzazione dei Costi e la Tua Prima Pipeline

*"Se non riesci a generare $200 di fatturato da $200 di spesa in API, correggi il prodotto — non il budget."*

### L'Economia dell'Automazione Alimentata da LLM

Ogni chiamata LLM ha un costo. Anche i modelli locali costano elettricità e usura della GPU. La domanda è se l'output di quella chiamata genera più valore di quanto la chiamata costi.

{? if profile.gpu.exists ?}
Eseguire modelli locali su {= profile.gpu.model | fallback("la tua GPU") =} costa circa {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("qualche dollaro") =} di elettricità al mese per carichi di lavoro tipici delle pipeline. Questa è la baseline da battere con le alternative API.
{? endif ?}

**La regola del budget API da {= regional.currency_symbol | fallback("$") =}200/mese:**

Se stai spendendo {= regional.currency_symbol | fallback("$") =}200/mese in chiamate API per le tue automazioni, quelle automazioni dovrebbero generare almeno {= regional.currency_symbol | fallback("$") =}200/mese in valore — sia come ricavo diretto che come tempo risparmiato che converti in ricavo altrove.

Se non ci riescono: il problema non è il budget API. È il design della pipeline o il prodotto che supporta.

### Tracciamento del Costo-Per-Output

Aggiungi questo a ogni pipeline che costruisci:

```python
"""
cost_tracker.py — Traccia il costo di ogni chiamata LLM e il valore che genera.
Importa questo nelle tue pipeline per ottenere dati reali sui costi.
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# Prezzi per 1M token (aggiorna quando cambiano i prezzi)
PRICING = {
    # Modelli locali — stima costo elettricità
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # Modelli API
    "claude-sonnet-4-20250514": {"input": 3.00, "output": 15.00},
    "claude-3-5-haiku-20241022": {"input": 0.80, "output": 4.00},
    "gpt-4o-mini": {"input": 0.15, "output": 0.60},
    "gpt-4o": {"input": 2.50, "output": 10.00},
}

def log_cost(
    pipeline: str,
    model: str,
    input_tokens: int,
    output_tokens: int,
    revenue_generated: float = 0.0,
    item_id: str = ""
):
    """Registra il costo di una chiamata LLM."""
    prices = PRICING.get(model, {"input": 1.0, "output": 5.0})

    cost = (
        (input_tokens / 1_000_000 * prices["input"]) +
        (output_tokens / 1_000_000 * prices["output"])
    )

    entry = {
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "pipeline": pipeline,
        "model": model,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cost_usd": round(cost, 6),
        "revenue_usd": revenue_generated,
        "item_id": item_id,
    }

    os.makedirs(os.path.dirname(COST_LOG), exist_ok=True)
    with open(COST_LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")

    return cost

def monthly_report() -> dict:
    """Genera un riepilogo mensile costi/ricavi."""
    current_month = datetime.now().strftime("%Y-%m")
    pipelines = {}

    try:
        with open(COST_LOG, "r") as f:
            for line in f:
                entry = json.loads(line)
                if not entry["timestamp"].startswith(current_month):
                    continue

                pipeline = entry["pipeline"]
                if pipeline not in pipelines:
                    pipelines[pipeline] = {
                        "total_cost": 0,
                        "total_revenue": 0,
                        "call_count": 0,
                        "total_tokens": 0
                    }

                pipelines[pipeline]["total_cost"] += entry["cost_usd"]
                pipelines[pipeline]["total_revenue"] += entry.get("revenue_usd", 0)
                pipelines[pipeline]["call_count"] += 1
                pipelines[pipeline]["total_tokens"] += entry["input_tokens"] + entry["output_tokens"]
    except FileNotFoundError:
        pass

    # Stampa il report
    print(f"\nREPORT COSTI LLM — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    Chiamate: {data['call_count']}")
        print(f"    Token:    {data['total_tokens']:,}")
        print(f"    Costo:    ${data['total_cost']:.4f}")
        print(f"    Ricavi:   ${data['total_revenue']:.2f}")
        print(f"    ROI:      {roi:.1f}x")

        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  COSTO TOTALE:    ${grand_cost:.4f}")
    print(f"  RICAVI TOTALI:   ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  ROI COMPLESSIVO: {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### Batching per Efficienza API

Se stai usando modelli API, il batching fa risparmiare soldi veri:

```python
"""
batch_api.py — Raggruppa le chiamate API per efficienza.
Invece di fare 100 chiamate API separate, raggruppale.
"""

import os
import json
import time
import requests
from typing import Any

ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")

def batch_classify(
    items: list[dict],
    system_prompt: str,
    model: str = "claude-3-5-haiku-20241022",
    batch_size: int = 10,
    delay_between_batches: float = 1.0
) -> list[dict]:
    """
    Classifica molteplici elementi efficientemente raggruppandoli in singole chiamate API.

    Invece di 100 chiamate API (100 elementi × 1 chiamata ciascuno):
      - 100 chiamate × ~500 token input = 50.000 token input
      - 100 chiamate × ~200 token output = 20.000 token output
      - Costo con Haiku: ~$0,12

    Con batching (10 elementi per chiamata, 10 chiamate API):
      - 10 chiamate × ~2.500 token input = 25.000 token input
      - 10 chiamate × ~1.000 token output = 10.000 token output
      - Costo con Haiku: ~$0,06

    50% di risparmio solo dal batching.
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # Formatta il batch in un singolo prompt
        items_text = "\n".join(
            f"[ITEM {j+1}] {json.dumps(item)}"
            for j, item in enumerate(batch)
        )

        prompt = f"""Process each item below. For each item, provide a JSON object with your classification.

{items_text}

Respond with a JSON array containing one object per item, in the same order.
Each object should have: {{"item_index": <number>, "category": "<string>", "score": <1-10>}}"""

        try:
            resp = requests.post(
                "https://api.anthropic.com/v1/messages",
                headers={
                    "x-api-key": ANTHROPIC_KEY,
                    "anthropic-version": "2023-06-01",
                    "content-type": "application/json"
                },
                json={
                    "model": model,
                    "max_tokens": 2000,
                    "system": system_prompt,
                    "messages": [{"role": "user", "content": prompt}]
                },
                timeout=60
            )
            resp.raise_for_status()

            response_text = resp.json()["content"][0]["text"]
            # Parsa l'array JSON dalla risposta
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  Batch {i//batch_size + 1} fallito: {e}")
            # Fallback all'elaborazione individuale
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # Rate limiting per cortesia
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### Caching: Non Pagare Due Volte per la Stessa Risposta

```python
"""
llm_cache.py — Metti in cache le risposte LLM per evitare di pagare per elaborazioni duplicate.
"""

import os
import json
import hashlib
import sqlite3
from datetime import datetime, timedelta

CACHE_DB = os.path.expanduser("~/income/data/llm_cache.db")

def get_cache_db() -> sqlite3.Connection:
    os.makedirs(os.path.dirname(CACHE_DB), exist_ok=True)
    conn = sqlite3.connect(CACHE_DB)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            model TEXT NOT NULL,
            response TEXT NOT NULL,
            created_at TEXT NOT NULL,
            hit_count INTEGER DEFAULT 0
        )
    """)
    conn.commit()
    return conn

def cache_key(model: str, prompt: str) -> str:
    """Genera una chiave di cache deterministica da modello + prompt."""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """Ottieni una risposta in cache se disponibile e fresca."""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    row = conn.execute(
        "SELECT response, created_at FROM cache WHERE key = ?", (key,)
    ).fetchone()

    if row is None:
        conn.close()
        return None

    response, created_at = row
    age = datetime.utcnow() - datetime.fromisoformat(created_at)

    if age > timedelta(hours=max_age_hours):
        conn.execute("DELETE FROM cache WHERE key = ?", (key,))
        conn.commit()
        conn.close()
        return None

    # Aggiorna il contatore di hit
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """Metti in cache una risposta."""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """Mostra le statistiche della cache."""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Voci in cache: {total}")
    print(f"Hit totali della cache: {total_hits}")
    print(f"Risparmio stimato: ~${total_hits * 0.002:.2f} (media approssimativa per chiamata)")
```

**Usalo nelle tue pipeline:**

```python
# In qualsiasi pipeline che chiama un LLM:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # Gratis!

    response = call_llm(model, prompt)  # La tua funzione di chiamata LLM esistente
    set_cached(model, prompt, response)
    return response
```

Per pipeline che elaborano ripetutamente gli stessi tipi di contenuto (classificazione, estrazione), il caching può eliminare il 30-50% delle tue chiamate API. Questo è il 30-50% in meno sulla tua bolletta mensile.

### Costruire la Tua Prima Pipeline Completa: Passo dopo Passo

Ecco il processo completo da "ho un workflow manuale" a "gira mentre dormo."

**Passo 1: Mappa il tuo processo manuale attuale.**

Scrivi ogni passo che fai per un flusso di reddito specifico. Esempio per una newsletter:

```
1. Apri 15 feed RSS in schede del browser (10 min)
2. Scansiona i titoli, apri quelli interessanti (20 min)
3. Leggi 8-10 articoli in dettaglio (40 min)
4. Scrivi riassunti per i migliori 5 (30 min)
5. Scrivi il paragrafo introduttivo (10 min)
6. Formatta nello strumento email (15 min)
7. Invia alla lista (5 min)

Totale: ~2 ore e 10 minuti
```

**Passo 2: Identifica i tre passi che consumano più tempo.**

Dall'esempio: Leggere gli articoli (40 min), scrivere i riassunti (30 min), scansionare i titoli (20 min).

**Passo 3: Automatizza prima quello più facile.**

Scansionare i titoli è il più facile da automatizzare — è classificazione. Un LLM valuta la rilevanza, tu leggi solo quelli con il punteggio più alto.

**Passo 4: Misura il tempo risparmiato e la qualità.**

Dopo aver automatizzato la scansione dei titoli:
- Tempo risparmiato: 20 minuti
- Qualità: 90% di accordo con le tue scelte manuali
- Netto: 20 minuti risparmiati, perdita di qualità trascurabile

**Passo 5: Automatizza il passo successivo.**

Ora automatizza la scrittura dei riassunti. L'LLM prepara le bozze dei riassunti, tu li modifichi.

**Passo 6: Continua finché i rendimenti non sono decrescenti.**

```
Prima dell'automazione: 2h 10min per newsletter
Dopo Livello 2 (scaricamento schedulato): 1h 45min
Dopo Livello 3 (punteggio LLM + riassunti): 25min
Dopo Livello 3+ (LLM prepara l'intro): 10min solo revisione

Tempo risparmiato a settimana: ~2 ore
Tempo risparmiato al mese: ~8 ore
A $100/ora di tariffa effettiva: $800/mese in tempo liberato
Costo API: $0 (LLM locale) a $5/mese (API)
```

**Passo 7: La pipeline completa, collegata insieme.**

Ecco una GitHub Action che collega tutto per una pipeline di newsletter settimanale:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # Ogni domenica alle 5 AM UTC
    - cron: '0 5 * * 0'
  workflow_dispatch:

jobs:
  generate-newsletter:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.12'

      - name: Install dependencies
        run: pip install requests

      - name: Run newsletter pipeline
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
          NEWSLETTER_NICHE: "Rust developers, local AI, developer tooling"
        run: python scripts/newsletter_pipeline.py

      - name: Upload draft as artifact
        uses: actions/upload-artifact@v4
        with:
          name: newsletter-draft-${{ github.run_number }}
          path: drafts/

      - name: Notify via Slack
        if: success()
        run: |
          curl -X POST "${{ secrets.SLACK_WEBHOOK }}" \
            -H 'Content-Type: application/json' \
            -d '{"text":"Bozza della newsletter pronta per la revisione. Controlla gli artifact di GitHub Actions."}'
```

Questa gira ogni domenica alle 5 di mattina. Quando ti svegli, la bozza ti aspetta. Spendi 10 minuti a revisionarla con il caffè, premi invio, e la tua newsletter è pubblicata per la settimana.

### Tocca a Te: Costruisci la Tua Pipeline

Questo è il deliverable del modulo. Alla fine di questa lezione, dovresti avere una pipeline completa deployata e in esecuzione.

**Requisiti per la tua pipeline:**
1. Gira su una pianificazione senza il tuo intervento
2. Include almeno un passaggio di elaborazione LLM
3. Ha un passaggio di revisione umana per il controllo qualità
4. Ha un controllo di salute così sai se si rompe
5. È collegata a un flusso di reddito reale (o un flusso che stai costruendo)

**Checklist:**

- [ ] Scelto un flusso di reddito da automatizzare
- [ ] Mappato il processo manuale (tutti i passi, con stime di tempo)
- [ ] Identificati i 3 passi che consumano più tempo
- [ ] Automatizzato almeno il primo passo (classificazione/punteggio/filtro)
- [ ] Aggiunta l'elaborazione LLM per il secondo passo (sintesi/generazione/estrazione)
- [ ] Costruita una coda di revisione per la supervisione umana
- [ ] Impostato un controllo di salute per l'automazione
- [ ] Deployato su una pianificazione (cron, GitHub Actions, o timer systemd)
- [ ] Tracciato il costo e il tempo risparmiato per un ciclo completo
- [ ] Documentata la pipeline (cosa fa, come correggerla, cosa monitorare)

Se hai completato tutti e dieci i punti di questa checklist, hai un'automazione di Livello 3 in esecuzione. Hai appena liberato ore della tua settimana che puoi reinvestire nella costruzione di altri flussi o nel miglioramento di quelli esistenti.

---

## Modulo T: Completato

{@ temporal automation_progress @}

### Cosa Hai Costruito in Due Settimane

1. **Una comprensione della piramide dell'automazione** — sai dove sei e dove ciascuno dei tuoi flussi di reddito dovrebbe dirigersi.
2. **Automazioni schedulate** in esecuzione su cron o scheduler cloud — la fondazione poco glamour che rende tutto il resto possibile.
3. **Pipeline alimentate da LLM** che gestiscono le decisioni di giudizio che facevi manualmente — classificare, riassumere, generare, monitorare.
4. **Pattern basati su agenti** che puoi deployare per l'interazione con i clienti, l'elaborazione del feedback e prodotti basati su MCP.
5. **Un framework human-in-the-loop** che protegge la tua reputazione pur risparmiando l'80%+ del tuo tempo.
6. **Tracciamento e ottimizzazione dei costi** così le tue automazioni generano profitto, non solo attività.
7. **Una pipeline completa, deployata** che genera valore senza il tuo coinvolgimento attivo.

### L'Effetto Compounding

Ecco cosa succede nei prossimi 3 mesi se mantieni e estendi ciò che hai costruito in questo modulo:

```
Mese 1: Una pipeline, risparmio di 5-8 ore/settimana
Mese 2: Due pipeline, risparmio di 10-15 ore/settimana
Mese 3: Tre pipeline, risparmio di 15-20 ore/settimana

A $100/ora di tariffa effettiva, sono $1.500-2.000/mese
in tempo liberato — tempo che investi in nuovi flussi.

Il tempo liberato dal Mese 1 costruisce la pipeline per il Mese 2.
Il tempo liberato dal Mese 2 costruisce la pipeline per il Mese 3.
L'automazione fa compounding.
```

Ecco come un singolo sviluppatore opera come un team di cinque. Non lavorando di più. Costruendo sistemi che lavorano mentre tu non lo fai.

---

### Integrazione 4DA

{? if dna.identity_summary ?}
In base al tuo profilo da sviluppatore — {= dna.identity_summary | fallback("il tuo focus di sviluppo") =} — gli strumenti 4DA sotto si mappano direttamente ai pattern di automazione che hai appena imparato. Gli strumenti di classificazione dei segnali sono particolarmente rilevanti per gli sviluppatori nel tuo settore.
{? endif ?}

4DA è di per sé un'automazione di Livello 3. Ingerisce contenuti da decine di sorgenti, valuta ogni elemento con l'algoritmo PASIFA, e ti mostra solo ciò che è rilevante per il tuo lavoro — tutto senza che tu muova un dito. Non controlli manualmente Hacker News, Reddit e 50 feed RSS. 4DA lo fa e ti mostra ciò che conta.

Costruisci le tue pipeline di reddito allo stesso modo.

L'attention report di 4DA (`/attention_report` negli strumenti MCP) ti mostra dove va effettivamente il tuo tempo rispetto a dove dovrebbe andare. Eseguilo prima di decidere cosa automatizzare. Il divario tra "tempo speso" e "tempo che dovrebbe essere speso" è la tua roadmap di automazione.

Gli strumenti di classificazione dei segnali (`/get_actionable_signals`) possono alimentare direttamente la tua pipeline di monitoraggio del mercato — lasciando che il livello di intelligence di 4DA faccia il punteggio iniziale prima che la tua pipeline personalizzata faccia l'analisi specifica della nicchia.

Se stai costruendo pipeline che monitorano sorgenti per opportunità, non reinventare ciò che 4DA fa già. Usa il suo server MCP come blocco costruttivo nel tuo stack di automazione.

---

### Cosa Viene Dopo: Modulo S — Impilare i Flussi

Il Modulo T ti ha dato gli strumenti per far funzionare efficientemente ogni flusso di reddito. Il Modulo S (Impilare i Flussi) risponde alla domanda successiva: **quanti flussi dovresti gestire, e come si incastrano tra loro?**

Ecco cosa copre il Modulo S:

- **Teoria di portafoglio per i flussi di reddito** — perché 3 flussi battono 1 flusso, e perché 10 flussi battono zero
- **Correlazione tra flussi** — quali flussi si rinforzano a vicenda e quali competono per il tuo tempo
- **Il pavimento di reddito** — costruire una base di entrate ricorrenti che copra i tuoi costi prima di sperimentare
- **Ribilanciamento** — quando raddoppiare su un vincitore e quando eliminare un flusso sottoperformante
- **L'architettura da $10K/mese** — combinazioni specifiche di flussi che raggiungono le cinque cifre con 15-20 ore a settimana

Hai l'infrastruttura (Modulo S), i fossati (Modulo T), i motori (Modulo R), il playbook di lancio (Modulo E), il radar dei trend (Modulo E), e ora l'automazione (Modulo T). Il Modulo S lega tutto insieme in un portafoglio di reddito sostenibile e in crescita.

---

**La pipeline gira. La bozza è pronta. Spendi 10 minuti a revisionare.**

**Questa è automazione tattica. Così si scala.**
