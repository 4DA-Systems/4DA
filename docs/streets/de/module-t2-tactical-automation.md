# Modul T: Taktische Automatisierung

**STREETS Einkommenskurs für Entwickler — Bezahltes Modul**
*Wochen 12-13 | 6 Lektionen | Ergebnis: Eine automatisierte Pipeline, die Wert generiert*

> "LLMs, Agenten, MCP und Cron Jobs als Kraftmultiplikatoren."

---

Du hast Einkommensmaschinen am Laufen. Du hast Kunden. Du hast Prozesse, die funktionieren. Und du verbringst 60-70% deiner Zeit damit, immer wieder die gleichen Dinge zu tun: Eingaben verarbeiten, Ausgaben formatieren, Monitore prüfen, Updates senden, Warteschlangen überprüfen.

Diese Zeit ist deine teuerste Ressource, und du verbrennst sie für Aufgaben, die ein {= regional.currency_symbol | fallback("$") =}5/Monat VPS erledigen könnte.

{@ insight hardware_benchmark @}

In diesem Modul geht es darum, dich systematisch aus dem Kreislauf zu entfernen — nicht vollständig (das ist eine Falle, die wir in Lektion 5 behandeln), aber aus den 80% der Arbeit, die dein Urteilsvermögen nicht erfordern. Das Ergebnis: Deine Einkommensströme generieren Umsatz, während du schläfst, während du bei deinem Tagesjob bist, während du das Nächste aufbaust.

Am Ende dieser zwei Wochen wirst du Folgendes erreicht haben:

- Ein klares Verständnis der vier Automatisierungsstufen und wo du heute stehst
- Funktionierende Cron Jobs und geplante Automatisierungen, die auf deiner Infrastruktur laufen
- Mindestens eine LLM-gesteuerte Pipeline, die Eingaben ohne dein Zutun verarbeitet
- Ein Verständnis von agentenbasierten Systemen und wann sie wirtschaftlich sinnvoll sind
- Ein Human-in-the-Loop-Framework, damit Automatisierung deinen Ruf nicht zerstört
- Eine vollständige, bereitgestellte Pipeline, die Wert ohne deine aktive Beteiligung generiert

{? if stack.primary ?}
Dein primärer Stack ist {= stack.primary | fallback("dein primärer Stack") =}, daher sind die folgenden Automatisierungsbeispiele am direktesten anwendbar, wenn sie an dieses Ökosystem angepasst werden. Die meisten Beispiele verwenden Python für Portabilität, aber die Muster lassen sich auf jede Sprache übertragen.
{? endif ?}

Dies ist das code-lastigste Modul des Kurses. Mindestens die Hälfte des Folgenden ist ausführbarer Code. Kopiere ihn, passe ihn an, stelle ihn bereit.

Lass uns automatisieren.

---

## Lektion 1: Die Automatisierungspyramide

*"Die meisten Entwickler automatisieren auf Stufe 1. Das Geld liegt auf Stufe 3."*

### Die vier Stufen

Jede Automatisierung in deinem Einkommens-Stack fällt irgendwo in diese Pyramide:

```
┌───────────────────────────────┐
│  Stufe 4: Autonome Agenten    │  ← Trifft Entscheidungen für dich
│  (KI entscheidet UND handelt) │
├───────────────────────────────┤
│  Stufe 3: Intelligente        │  ← Hier liegt das Geld
│  Pipelines (LLM-gesteuert)   │
├───────────────────────────────┤
│  Stufe 2: Geplante            │  ← Die meisten hören hier auf
│  Automatisierung (cron+Skripte)│
├───────────────────────────────┤
│  Stufe 1: Manuell mit         │  ← Wo die meisten stehen
│  Vorlagen (kopieren-einfügen) │
└───────────────────────────────┘
```

Schauen wir uns genau an, wie jede Stufe in der Praxis aussieht.

### Stufe 1: Manuell mit Vorlagen

Du erledigst die Arbeit, aber du hast Checklisten, Vorlagen und Snippets, um die Dinge zu beschleunigen.

**Beispiele:**
- Du schreibst einen Blogpost mit einer Markdown-Vorlage mit vorgefülltem Frontmatter
- Du erstellst Rechnungen für Kunden, indem du die Rechnung des letzten Monats duplizierst und die Zahlen änderst
- Du beantwortest Support-E-Mails mit gespeicherten Antworten
- Du veröffentlichst Inhalte, indem du manuell einen Deploy-Befehl ausführst

**Zeitaufwand:** 100% deiner Zeit pro Ausgabeeinheit.
**Fehlerrate:** Mäßig — du bist ein Mensch, du machst Fehler, wenn du müde bist.
**Skalierungsgrenze:** Du. Deine Stunden. Das war's.

Die meisten Entwickler leben hier und merken nicht einmal, dass es eine Pyramide über ihnen gibt.

### Stufe 2: Geplante Automatisierung

Skripte laufen nach Zeitplan. Du hast die Logik einmal geschrieben. Sie wird ohne dich ausgeführt.

**Beispiele:**
- Ein Cron Job, der deinen RSS-Feed prüft und neue Artikel in sozialen Medien postet
- Ein GitHub Action, der deine Seite jeden Morgen um 6 Uhr baut und bereitstellt
- Ein Skript, das stündlich die Preise der Konkurrenz prüft und Änderungen protokolliert
- Ein tägliches Datenbank-Backup, das um 3 Uhr morgens läuft

**Zeitaufwand:** Null laufend (nach anfänglicher Einrichtung von 1-4 Stunden).
**Fehlerrate:** Niedrig — deterministisch, jedes Mal die gleiche Logik.
**Skalierungsgrenze:** So viele Aufgaben, wie deine Maschine planen kann. Hunderte.

Hier landen die meisten technisch versierten Entwickler. Es ist komfortabel. Aber es hat eine harte Grenze: Es kann nur Aufgaben mit deterministischer Logik bewältigen. Wenn die Aufgabe Urteilsvermögen erfordert, steckst du fest.

### Stufe 3: Intelligente Pipelines

Skripte laufen nach Zeitplan, aber sie beinhalten ein LLM, das die Urteilsentscheidungen übernimmt.

**Beispiele:**
- RSS-Feeds werden aufgenommen, LLM fasst jeden Artikel zusammen, entwirft einen Newsletter, du überprüfst 10 Minuten und drückst auf Senden
- Kundenfeedback-E-Mails werden nach Stimmung und Dringlichkeit klassifiziert, vorentworfene Antworten werden zur Genehmigung in die Warteschlange gestellt
- Neue Stellenangebote in deiner Nische werden gescrapt, LLM bewertet die Relevanz, du bekommst eine tägliche Zusammenfassung von 5 Möglichkeiten statt 200 Einträge zu durchsuchen
- Blog-Posts der Konkurrenz werden überwacht, LLM extrahiert wichtige Produktänderungen, du bekommst einen wöchentlichen Wettbewerbsintelligenz-Bericht

**Zeitaufwand:** 10-20% der manuellen Zeit. Du überprüfst und genehmigst statt zu erstellen.
**Fehlerrate:** Niedrig für Klassifikationsaufgaben, mäßig für Generierung (deshalb überprüfst du).
**Skalierungsgrenze:** Tausende von Items pro Tag. Dein Engpass sind die API-Kosten, nicht deine Zeit.

**Hier liegt das Geld.** Stufe 3 ermöglicht es einer Person, Einkommensströme zu betreiben, die normalerweise ein Team von 3-5 Personen erfordern würden.

### Stufe 4: Autonome Agenten

KI-Systeme, die ohne dein Zutun beobachten, entscheiden und handeln.

**Beispiele:**
- Ein Agent, der deine SaaS-Metriken überwacht, einen Rückgang bei Anmeldungen erkennt, eine Preisänderung per A/B-Test durchführt und sie zurücksetzt, wenn sie nicht funktioniert
- Ein Support-Agent, der Tier-1-Kundenfragen vollständig autonom bearbeitet und nur bei komplexen Problemen an dich eskaliert
- Ein Content-Agent, der Trendthemen identifiziert, Entwürfe generiert, Veröffentlichungen plant und die Performance überwacht

**Zeitaufwand:** Nahe null für bearbeitete Fälle. Du überprüfst Metriken, nicht einzelne Aktionen.
**Fehlerrate:** Hängt vollständig von deinen Leitplanken ab. Ohne sie: hoch. Mit guten Leitplanken: überraschend niedrig für enge Domänen.
**Skalierungsgrenze:** Effektiv unbegrenzt für die Aufgaben im Bereich des Agenten.

Stufe 4 ist real und erreichbar, aber es ist nicht der Startpunkt. Und wie wir in Lektion 5 behandeln werden, sind vollständig autonome, kundenorientierte Agenten gefährlich für deinen Ruf, wenn sie schlecht implementiert sind.

> **Klartext:** Wenn du gerade auf Stufe 1 bist, versuche nicht, auf Stufe 4 zu springen. Du wirst Wochen damit verbringen, einen "autonomen Agenten" zu bauen, der in der Produktion kaputtgeht und das Kundenvertrauen beschädigt. Steige die Pyramide eine Stufe nach der anderen auf. Stufe 2 ist ein Nachmittag Arbeit. Stufe 3 ist ein Wochenendprojekt. Stufe 4 kommt, nachdem du Stufe 3 einen Monat lang zuverlässig am Laufen hattest.

### Selbsteinschätzung: Wo stehst du?

Bewerte dich für jeden deiner Einkommensströme ehrlich:

| Einkommensstrom | Aktuelle Stufe | Stunden/Woche | Könnte automatisieren auf |
|-----------------|---------------|--------------|--------------------------|
| [z.B. Newsletter] | [1-4] | [X] Std. | [Zielstufe] |
| [z.B. Kundenverarbeitung] | [1-4] | [X] Std. | [Zielstufe] |
| [z.B. Social Media] | [1-4] | [X] Std. | [Zielstufe] |
| [z.B. Support] | [1-4] | [X] Std. | [Zielstufe] |

Die wichtigste Spalte ist "Stunden/Woche." Der Strom mit den meisten Stunden und der niedrigsten Stufe ist dein erstes Automatisierungsziel. Das ist der mit dem größten ROI.

### Die Wirtschaftlichkeit jeder Stufe

Nehmen wir an, du hast einen Einkommensstrom, der 10 Stunden/Woche deiner Zeit kostet und {= regional.currency_symbol | fallback("$") =}2.000/Monat generiert:

| Stufe | Deine Zeit | Dein effektiver Stundensatz | Automatisierungskosten |
|-------|-----------|---------------------------|----------------------|
| Stufe 1 | 10 Std./Woche | $50/Std. | $0 |
| Stufe 2 | 3 Std./Woche | $167/Std. | $5/Monat (VPS) |
| Stufe 3 | 1 Std./Woche | $500/Std. | $30-50/Monat (API) |
| Stufe 4 | 0,5 Std./Woche | $1.000/Std. | $50-100/Monat (API + Rechenleistung) |

Der Wechsel von Stufe 1 zu Stufe 3 ändert nicht deinen Umsatz. Er ändert deinen effektiven Stundensatz von $50 auf $500. Und diese 9 freigewordenen Stunden? Die investierst du in den Aufbau des nächsten Einkommensstroms oder die Verbesserung des aktuellen.

> **Häufiger Fehler:** Zuerst deinen umsatzschwächsten Strom automatisieren, weil es "einfacher" ist. Nein. Automatisiere den Strom, der die meisten Stunden im Verhältnis zu seinem Umsatz frisst. Da liegt der ROI.

### Du bist dran

1. Fülle die Selbsteinschätzungstabelle oben für jeden Einkommensstrom (oder geplanten Strom) aus, den du hast.
2. Identifiziere dein Automatisierungsziel mit dem höchsten ROI: den Strom mit den meisten Stunden und der niedrigsten Automatisierungsstufe.
3. Schreibe die 3 zeitaufwändigsten Aufgaben in diesem Strom auf. Du wirst die erste in Lektion 2 automatisieren.

---

## Lektion 2: Von Stufe 1 zu 2 — Geplante Automatisierung

*"Cron ist von 1975. Es funktioniert immer noch. Benutze es."*

### Cron-Job-Grundlagen

{? if computed.os_family == "windows" ?}
Du bist auf Windows, daher ist cron nicht nativ auf deinem System. Du hast zwei Optionen: WSL (Windows Subsystem for Linux) verwenden, um echtes cron zu bekommen, oder den Windows-Aufgabenplaner verwenden (unten behandelt). WSL wird empfohlen, wenn du dich damit wohlfühlst — alle Cron-Beispiele in dieser Lektion funktionieren direkt in WSL. Wenn du natives Windows bevorzugst, springe nach diesem Abschnitt zum Aufgabenplaner.
{? endif ?}

Ja, auch 2026 ist cron der König für geplante Aufgaben. Es ist zuverlässig, es ist überall, und es erfordert kein Cloud-Konto, kein SaaS-Abo und kein YAML-Schema, das du jedes Mal googeln musst.

**Die Cron-Syntax in 30 Sekunden:**

```
┌───────── Minute (0-59)
│ ┌───────── Stunde (0-23)
│ │ ┌───────── Tag des Monats (1-31)
│ │ │ ┌───────── Monat (1-12)
│ │ │ │ ┌───────── Wochentag (0-7, 0 und 7 = Sonntag)
│ │ │ │ │
* * * * *  Befehl
```

**Häufige Zeitpläne:**

```bash
# Every hour
0 * * * *  /path/to/script.sh

# Every day at 6 AM
0 6 * * *  /path/to/script.sh

# Every Monday at 9 AM
0 9 * * 1  /path/to/script.sh

# Every 15 minutes
*/15 * * * *  /path/to/script.sh

# First day of every month at midnight
0 0 1 * *  /path/to/script.sh
```

**Einen Cron Job einrichten:**

```bash
# Edit your crontab
crontab -e

# List existing cron jobs
crontab -l

# CRITICAL: Always set environment variables at the top
# Cron runs with a minimal environment — PATH might not include your tools
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# Log output so you can debug failures
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **Häufiger Fehler:** Ein Skript schreiben, das perfekt funktioniert, wenn du es manuell ausführst, und dann in cron still fehlschlägt, weil cron deine `.bashrc` oder `.zshrc` nicht lädt. Verwende immer absolute Pfade in Cron-Skripten. Setze immer `PATH` am Anfang deiner Crontab. Leite die Ausgabe immer in eine Logdatei um.

### Cloud-Scheduler, wenn Cron nicht reicht

Wenn deine Maschine nicht rund um die Uhr läuft, oder du etwas Robusteres brauchst, verwende einen Cloud-Scheduler:

**GitHub Actions (kostenlos für öffentliche Repos, 2.000 Min/Monat bei privaten):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # Every day at 6 AM UTC
    - cron: '0 6 * * *'
  # Allow manual trigger for testing
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

**Vercel Cron (kostenlos im Hobby-Plan, 1 pro Tag; Pro-Plan: unbegrenzt):**

```typescript
// api/cron/daily-report.ts
// Vercel cron endpoint — configure schedule in vercel.json

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Verify it's actually Vercel calling, not a random HTTP request
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // Your automation logic here
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

### Echte Automatisierungen zum sofort Bauen

Hier sind fünf Automatisierungen, die du heute implementieren kannst. Jede dauert 30-60 Minuten und eliminiert Stunden wöchentlicher manueller Arbeit.

#### Automatisierung 1: Inhalte automatisch nach Zeitplan veröffentlichen

Du schreibst Blogposts im Voraus. Dieses Skript veröffentlicht sie zum geplanten Zeitpunkt.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — Publish markdown posts on their scheduled date.
Run daily via cron: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# Your CMS API endpoint (Hashnode, Dev.to, Ghost, etc.)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """Load the list of already-published post filenames."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """Save the list of published post filenames."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """Extract YAML-style frontmatter from a markdown file."""
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
    """Check if a post should be published today."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """Publish a post to your CMS API."""
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
        print(f"  Published: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  FAILED: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"Checking {len(posts)} posts...")

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
    print(f"Total published: {len(published)}")

if __name__ == "__main__":
    main()
```

**Deine Markdown-Posts sehen so aus:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Your post content here...
```

Schreibe Posts, wenn die Inspiration kommt. Setze das Datum. Das Skript erledigt den Rest.

#### Automatisierung 2: Automatisch in sozialen Medien posten bei neuem Inhalt

Wenn dein Blog etwas Neues veröffentlicht, postet dies automatisch auf Twitter/X und Bluesky.

```python
#!/usr/bin/env python3
"""
social_poster.py — Post to social platforms when new content is published.
Run every 30 minutes: */30 * * * * python3 /path/to/social_poster.py
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
    """Parse RSS feed and return list of items."""
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
    """Post to Bluesky via AT Protocol."""
    # Step 1: Create session
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

    # Step 2: Create post
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
    print(f"  Posted to Bluesky: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # Format the social post
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky has a 300 character limit
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  Failed to post: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

Kosten: $0. Läuft auf deiner Maschine oder einem kostenlosen GitHub Action.

#### Automatisierung 3: Wettbewerber-Preismonitor

Erfahre sofort, wenn ein Wettbewerber seine Preise ändert. Kein manuelles Prüfen mehr jede Woche.

```python
#!/usr/bin/env python3
"""
price_monitor.py — Monitor competitor pricing pages for changes.
Run every 6 hours: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # or Discord, email, etc.

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # For full-page monitoring; use selector for specific elements
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """Fetch a page and return its content hash and text excerpt."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # Grab first 500 chars of visible text for context
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
    """Send alert via Slack webhook (swap for Discord, email, etc.)."""
    if not ALERT_WEBHOOK:
        print(f"ALERT (no webhook configured): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  Failed to fetch {name}: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"PRICING CHANGE DETECTED: {name}\n"
                f"URL: {url}\n"
                f"Changed at: {datetime.utcnow().isoformat()}Z\n"
                f"Previous hash: {previous_hash[:12]}...\n"
                f"New hash: {current_hash[:12]}...\n"
                f"Go check it manually."
            )
            send_alert(alert_msg)
            print(f"  CHANGE: {name}")
        else:
            print(f"  No change: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### Automatisierung 4: Wöchentlicher Umsatzbericht

Jeden Montagmorgen generiert dies einen Bericht aus deinen Umsatzdaten und sendet ihn dir per E-Mail.

```python
#!/usr/bin/env python3
"""
weekly_report.py — Generate weekly revenue report from your tracking spreadsheet/database.
Run Mondays at 7 AM: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """Create the revenue table if it doesn't exist."""
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
    """Generate a plain-text weekly report."""
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
    report.append(f"WEEKLY REVENUE REPORT")
    report.append(f"Period: {week_ago.strftime('%Y-%m-%d')} to {today.strftime('%Y-%m-%d')}")
    report.append(f"Generated: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    Income:   ${data['income']:>10,.2f}")
        report.append(f"    Expenses: ${data['expense']:>10,.2f}")
        report.append(f"    Net:      ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  TOTAL INCOME:   ${total_income:>10,.2f}")
    report.append(f"  TOTAL EXPENSES: ${total_expenses:>10,.2f}")
    report.append(f"  NET PROFIT:     ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  ROI:            {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """Send the report via email."""
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
            f"Weekly Revenue Report — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nReport emailed.")
    conn.close()

if __name__ == "__main__":
    main()
```

#### Automatisierung 5: Automatisches Backup von Kundendaten

Verliere nie wieder Kundenlieferungen. Dies läuft nächtlich und bewahrt 30 Tage Backups auf.

```bash
#!/bin/bash
# backup_client_data.sh — Nightly backup of client project data.
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# Create compressed backup
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# Delete backups older than retention period
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# Log
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup complete: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# Optional: sync to a second location (external drive, another machine)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### Systemd-Timer für mehr Kontrolle

Wenn du mehr brauchst als cron bietet — wie Abhängigkeitsreihenfolge, Ressourcenlimits oder automatische Wiederholung — verwende Systemd-Timer:

```ini
# /etc/systemd/system/income-publisher.service
[Unit]
Description=Publish scheduled content
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=youruser
ExecStart=/usr/bin/python3 /home/youruser/scripts/scheduled_publisher.py
Environment="CMS_API_KEY=your-key-here"
Environment="CMS_API_URL=https://api.example.com/posts"
# Restart on failure with exponential backoff
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
# If the machine was off at 6 AM, run when it comes back online
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# Enable and start the timer
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# Check status
systemctl list-timers --all | grep income

# View logs
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Alternative: Windows-Aufgabenplaner

Wenn du WSL nicht verwendest, erledigt der Windows-Aufgabenplaner den gleichen Job. Verwende `schtasks` von der Kommandozeile oder die GUI des Aufgabenplaners (`taskschd.msc`). Der Hauptunterschied: Cron verwendet einen einzigen Ausdruck, der Aufgabenplaner verwendet separate Felder für Trigger, Aktionen und Bedingungen. Jedes Cron-Beispiel in dieser Lektion lässt sich direkt übertragen — plane deine Python-Skripte auf die gleiche Weise, nur über eine andere Oberfläche.
{? endif ?}

### Du bist dran

1. Wähle die einfachste Automatisierung aus dieser Lektion, die auf deinen Einkommensstrom zutrifft.
2. Implementiere sie. Nicht "plane, sie zu implementieren." Schreibe den Code, teste ihn, plane ihn ein.
3. Richte Logging ein, damit du überprüfen kannst, ob es läuft. Prüfe die Logs jeden Morgen 3 Tage lang.
4. Sobald es stabil ist, höre auf, täglich zu prüfen. Prüfe wöchentlich. Das ist Automatisierung.

**Minimum:** Ein Cron Job, der bis zum Ende des heutigen Tages zuverlässig läuft.

---

## Lektion 3: Von Stufe 2 zu 3 — LLM-gesteuerte Pipelines

*"Füge Intelligenz zu deinen Automatisierungen hinzu. Hier fängt eine Person an, wie ein Team auszusehen."*

### Das Muster

Jede LLM-gesteuerte Pipeline folgt der gleichen Form:

```
Eingabequellen → Aufnehmen → LLM-Verarbeitung → Ausgabe formatieren → Liefern (oder zur Überprüfung einreihen)
```

Die Magie steckt im Schritt "LLM-Verarbeitung". Statt deterministische Regeln für jeden möglichen Fall zu schreiben, beschreibst du, was du willst, in natürlicher Sprache, und das LLM übernimmt die Urteilsentscheidungen.

### Wann lokal vs. API verwenden

{? if settings.has_llm ?}
Du hast {= settings.llm_provider | fallback("einen LLM-Anbieter") =} mit {= settings.llm_model | fallback("deinem LLM-Modell") =} konfiguriert. Das bedeutet, du kannst sofort mit dem Aufbau intelligenter Pipelines beginnen. Die folgende Entscheidungshilfe zeigt dir, wann du dein lokales Setup versus eine API für jede Pipeline verwenden solltest.
{? else ?}
Du hast noch kein LLM konfiguriert. Die Pipelines in dieser Lektion funktionieren sowohl mit lokalen Modellen (Ollama) als auch mit Cloud-APIs. Richte mindestens eines ein, bevor du deine erste Pipeline baust — Ollama ist kostenlos und braucht 10 Minuten zur Installation.
{? endif ?}

Diese Entscheidung hat einen direkten Einfluss auf deine Margen:

| Faktor | Lokal (Ollama) | API (Claude, GPT) |
|--------|---------------|-------------------|
| **Kosten pro 1M Token** | ~$0,003 (Strom) | $0,15 - $15,00 |
| **Geschwindigkeit (Token/Sek.)** | 20-60 (8B auf Mittelklasse-GPU) | 50-100+ |
| **Qualität (8B lokal vs. API)** | Gut für Klassifikation, Extraktion | Besser für Generierung, Reasoning |
| **Datenschutz** | Daten verlassen nie deine Maschine | Daten gehen zum Anbieter |
| **Verfügbarkeit** | Abhängig von deiner Maschine | 99,9%+ |
| **Batch-Kapazität** | Begrenzt durch GPU-Speicher | Begrenzt durch Rate-Limits und Budget |

{? if profile.gpu.exists ?}
Mit {= profile.gpu.model | fallback("deiner GPU") =} auf deiner Maschine ist lokale Inferenz eine starke Option. Die Geschwindigkeit und Modellgröße, die du ausführen kannst, hängt von deinem VRAM ab — prüfe, was passt, bevor du dich auf eine rein lokale Pipeline festlegst.
{? if computed.has_nvidia ?}
NVIDIA-GPUs erzielen dank CUDA-Beschleunigung die beste Ollama-Performance. Du solltest 7-8B-Parameter-Modelle problemlos ausführen können, und möglicherweise größere, abhängig von deinem {= profile.gpu.vram | fallback("verfügbaren VRAM") =}.
{? endif ?}
{? else ?}
Ohne dedizierte GPU wird lokale Inferenz langsamer sein (nur CPU). Es funktioniert trotzdem für kleine Batch-Jobs und Klassifikationsaufgaben, aber für alles Zeitkritische oder mit hohem Volumen ist ein API-Modell praktischer.
{? endif ?}

**Faustregeln:**
- **Hohes Volumen, niedrigere Qualitätsanforderung** (Klassifikation, Extraktion, Tagging) → Lokal
- **Niedriges Volumen, qualitätskritisch** (kundenorientierter Inhalt, komplexe Analyse) → API
- **Sensible Daten** (Kundeninfos, proprietäre Daten) → Lokal, immer
- **Mehr als 10.000 Items/Monat** → Lokal spart echtes Geld

**Monatlicher Kostenvergleich für eine typische Pipeline:**

```
Processing 5,000 items/month, ~500 tokens per item:

Local (Ollama, llama3.1:8b):
  2,500,000 tokens × $0.003/1M = $0.0075/month
  Basically free.

API (GPT-4o-mini):
  2,500,000 input tokens × $0.15/1M = $0.375
  2,500,000 output tokens × $0.60/1M = $1.50
  Total: ~$1.88/month
  Cheap, but 250x more than local.

API (Claude 3.5 Sonnet):
  2,500,000 input tokens × $3.00/1M = $7.50
  2,500,000 output tokens × $15.00/1M = $37.50
  Total: ~$45/month
  Quality is excellent, but 6,000x more than local.
```

Für Klassifikations- und Extraktionspipelines ist der Qualitätsunterschied zwischen einem gut konfigurierten lokalen 8B-Modell und einem Frontier-API-Modell oft vernachlässigbar. Teste beide. Verwende das günstigere, das deine Qualitätsanforderung erfüllt.

{@ insight cost_projection @}

### Pipeline 1: Newsletter-Inhaltsgenerator

Dies ist die häufigste LLM-Automatisierung für Entwickler mit inhaltsbasierten Einkommen. RSS-Feeds gehen rein, ein Newsletter-Entwurf kommt raus.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — Ingest RSS feeds, summarize with LLM, generate newsletter draft.
Run daily: 0 5 * * * python3 /path/to/newsletter_pipeline.py

This pipeline:
1. Fetches new articles from multiple RSS feeds
2. Sends each to a local LLM for summarization
3. Ranks them by relevance to your audience
4. Generates a formatted newsletter draft
5. Saves the draft for your review (you spend 10 min reviewing, not 2 hours curating)
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
    # Add your niche feeds here
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
    """Parse an RSS/Atom feed and return articles."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # Handle both RSS and Atom feeds
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
        print(f"  Failed to fetch {url}: {e}")
        return []

def llm_process(prompt: str) -> str:
    """Send a prompt to the local LLM and get the response."""
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
        print(f"  LLM error: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """Use LLM to score relevance and generate a summary."""
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
        # Try to parse the JSON from the LLM output
        # Handle cases where LLM wraps in markdown code blocks
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
    """Format scored articles into a newsletter draft."""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# Your Newsletter — {today}")
    newsletter.append("")
    newsletter.append("*[YOUR INTRO HERE — Write 2-3 sentences about this week's theme]*")
    newsletter.append("")

    section_titles = {
        "tool": "Tools & Releases",
        "technique": "Techniques & Patterns",
        "news": "Industry News",
        "tutorial": "Tutorials & Guides",
        "opinion": "Perspectives"
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
    newsletter.append("*[YOUR CLOSING — What are you working on? What should readers look out for?]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("Fetching feeds...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} new articles")

    if not all_articles:
        print("No new articles. Skipping.")
        return

    print(f"\nScoring {len(all_articles)} articles with LLM...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # Filter to relevant articles only and sort by score
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # Take top 10
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} articles passed relevance threshold (>= 6/10)")

    # Generate the newsletter draft
    draft = generate_newsletter(top_articles)

    # Save draft
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nDraft saved: {draft_path}")
    print("Review it, add your intro/closing, and send.")

if __name__ == "__main__":
    main()
```

**Was das kostet:**
- 50 Artikel/Tag mit einem lokalen 8B-Modell verarbeiten: ~$0/Monat
- Deine Zeit: 10 Minuten den Entwurf überprüfen vs. 2 Stunden manuell kuratieren
- Zeitersparnis pro Woche: ~10 Stunden bei einem wöchentlichen Newsletter

### Pipeline 2: Kunden-Research und Insight-Berichte

Diese Pipeline scrapt öffentliche Daten, analysiert sie mit einem LLM und erstellt einen Bericht, den du verkaufen kannst.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — Analyze public company/product data and generate insight reports.
This is a service you can sell: $200-500 per custom report.

Usage: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# Use a larger model for quality on paid reports
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# Or use API for customer-facing quality:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """Route to local or API model based on configuration."""
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
    """Gather publicly available data about a company."""
    data = {"company": company, "domain": domain}

    # Check if domain is reachable and get basic info
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

    # Check GitHub presence
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
    """Generate an analysis report using LLM."""
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
        print("Usage: python3 research_pipeline.py 'Company Name' 'domain.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"Researching: {company} ({domain})")
    print(f"Using: {'API (Claude)' if USE_API else 'Local (Ollama)'}")

    print("Gathering public data...")
    data = gather_public_data(company, domain)

    print("Generating analysis...")
    report = generate_report(company, domain, data)

    # Assemble final report
    final_report = f"""# Research Report: {company}

**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**Domain:** {domain}
**Analysis model:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*This report was generated using publicly available data only.
No proprietary or private data was accessed.*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nReport saved: {filepath}")
    print(f"API cost: ~${'0.02-0.05' if USE_API else '0.00'}")

if __name__ == "__main__":
    main()
```

**Geschäftsmodell:** Berechne $200-500 pro individuellem Research-Bericht. Deine Kosten: $0,05 an API-Aufrufen und 15 Minuten Überprüfung. Du kannst 3-4 Berichte pro Stunde produzieren, sobald die Pipeline stabil läuft.

### Pipeline 3: Marktsignal-Monitor

Dies ist die Pipeline, die dir sagt, was du als Nächstes bauen sollst. Sie überwacht mehrere Quellen, klassifiziert Signale und alarmiert dich, wenn eine Gelegenheit deinen Schwellenwert überschreitet.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — Monitor public sources for market opportunities.
Run every 2 hours: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# Your niche definition — the LLM uses this to score relevance
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
    """Fetch top Hacker News stories."""
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
        print(f"  HN fetch failed: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """Use LLM to classify a signal for market opportunity."""
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
        item["reasoning"] = f"Classification failed: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """Send an alert for high-scoring opportunities."""
    msg = (
        f"OPPORTUNITY DETECTED (score: {item['opportunity_score']}/10)\n"
        f"Type: {item['opportunity_type']}\n"
        f"Title: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Why: {item['reasoning']}\n"
        f"Action: {item['action']}"
    )

    # Log to file
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # Send to Slack/Discord
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERT: {msg}")

def main():
    seen = load_seen()

    # Fetch from sources
    print("Fetching signals...")
    items = fetch_hn_top(30)
    # Add more sources here: Reddit, RSS feeds, GitHub trending, etc.

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} new signals to classify")

    # Classify each signal
    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("Done.")

if __name__ == "__main__":
    main()
```

**Was das in der Praxis tut:** Du bekommst 2-3 Mal pro Woche eine Slack-Benachrichtigung wie "GELEGENHEIT: Neues Framework ohne Starter-Kit veröffentlicht — du könntest dieses Wochenende eines bauen." Dieses Signal, darauf vor anderen zu reagieren, ist wie du vorne bleibst.

> **Klartext:** Die Qualität dieser Pipeline-Ausgaben hängt vollständig von deinen Prompts und deiner Nischendefinition ab. Wenn deine Nische vage ist ("Ich bin Webentwickler"), wird das LLM alles markieren. Wenn sie spezifisch ist ("Ich baue Tauri-Desktop-Apps für den datenschutzorientierten Entwicklermarkt"), wird es chirurgisch präzise sein. Investiere 30 Minuten, um deine Nischendefinition richtig hinzubekommen. Sie ist der einzelne Input mit dem größten Hebel für jede Pipeline, die du baust.

### Du bist dran

{? if stack.contains("python") ?}
Gute Nachricht: Die obigen Pipeline-Beispiele sind bereits in deiner Hauptsprache. Du kannst sie direkt kopieren und anpassen. Konzentriere dich darauf, die Nischendefinition und Prompts richtig hinzubekommen — daher kommen 90% der Ausgabequalität.
{? else ?}
Die obigen Beispiele verwenden Python für Portabilität, aber die Muster funktionieren in jeder Sprache. Wenn du lieber in {= stack.primary | fallback("deinem primären Stack") =} bauen möchtest, sind die Schlüsselteile zum Replizieren: HTTP-Client für RSS/API-Abruf, JSON-Parsing für LLM-Antworten und Datei-I/O für Zustandsverwaltung. Die LLM-Interaktion ist nur ein HTTP-POST an Ollama oder eine Cloud-API.
{? endif ?}

1. Wähle eine der drei obigen Pipelines (Newsletter, Research oder Signalmonitor).
2. Passe sie an deine Nische an. Ändere die Feeds, die Zielgruppenbeschreibung, die Klassifikationskriterien.
3. Führe sie 3 Mal manuell aus, um die Ausgabequalität zu testen.
4. Justiere die Prompts, bis die Ausgabe ohne starke Bearbeitung nützlich ist.
5. Plane sie mit cron ein.

**Ziel:** Eine LLM-gesteuerte Pipeline, die innerhalb von 48 Stunden nach dem Lesen dieser Lektion nach Zeitplan läuft.

---

## Lektion 4: Von Stufe 3 zu 4 — Agentenbasierte Systeme

*"Ein Agent ist nur eine Schleife, die beobachtet, entscheidet und handelt. Baue einen."*

### Was "Agent" 2026 wirklich bedeutet

Streiche den Hype. Ein Agent ist ein Programm, das:

1. **Beobachtet** — eine Eingabe oder einen Zustand liest
2. **Entscheidet** — ein LLM verwendet, um zu bestimmen, was zu tun ist
3. **Handelt** — die Entscheidung ausführt
4. **Wiederholt** — zurück zu Schritt 1 geht

Das war's. Der Unterschied zwischen einer Pipeline (Stufe 3) und einem Agenten (Stufe 4) ist, dass der Agent wiederholt. Er handelt auf Basis seiner eigenen Ausgabe. Er bewältigt mehrstufige Aufgaben, bei denen der nächste Schritt vom Ergebnis des vorherigen abhängt.

Eine Pipeline verarbeitet Items nacheinander durch eine feste Sequenz. Ein Agent navigiert eine unvorhersehbare Sequenz basierend auf dem, was er vorfindet.

### MCP-Server, die Kunden bedienen

Ein MCP-Server ist eines der praktischsten agentennahen Systeme, die du bauen kannst. Er stellt Werkzeuge bereit, die ein KI-Agent (Claude Code, Cursor usw.) im Auftrag deiner Kunden aufrufen kann.

{? if stack.contains("typescript") ?}
Das MCP-Server-Beispiel unten verwendet TypeScript — genau dein Gebiet. Du kannst es mit deinem bestehenden TypeScript-Tooling erweitern und neben deinen anderen Node.js-Services bereitstellen.
{? endif ?}

Hier ist ein reales Beispiel: ein MCP-Server, der Kundenfragen anhand der Dokumentation deines Produkts beantwortet.

```typescript
// mcp-docs-server/src/index.ts
// An MCP server that answers questions from your documentation.
// Your customers point their Claude Code at this server and get instant answers.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// Load your docs into memory at startup
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

    // Split by headings for better search
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
  // Simple keyword search — replace with vector search for production
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // Bonus for title matches
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

// Initialize
const docs = loadDocs();
console.error(`Loaded ${docs.length} doc chunks from ${DOCS_DIR}`);

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

// Start the server
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

**Geschäftsmodell:** Gib diesen MCP-Server deinen Kunden als Teil deines Produkts. Sie bekommen sofortige Antworten auf ihre Fragen, ohne Support-Tickets zu erstellen. Du verbringst weniger Zeit mit Support. Alle gewinnen.

Für Premium: Berechne $9-29/Monat für eine gehostete Version mit Vektorsuche, versionierter Dokumentation und Analysen darüber, was Kunden fragen.

### Automatisierte Verarbeitung von Kundenfeedback

Dieser Agent liest Kundenfeedback (per E-Mail, Support-Tickets oder Formular), klassifiziert es und erstellt Antwortentwürfe und Feature-Tickets.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — Process customer feedback into classified, actionable items.
The "AI draft, human approve" pattern.

Run every hour: 0 * * * * python3 /path/to/feedback_agent.py
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
    """Classify feedback and generate draft response."""

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
        feedback["draft_response"] = "[Classification failed — needs manual review]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"No inbox directory: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("No new feedback.")
        return

    print(f"Processing {len(inbox_files)} feedback items...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  Skipping {filepath.name}: {e}")
            continue

        print(f"  Processing: {feedback.get('subject', 'No subject')[:50]}...")
        processed = process_feedback(feedback)

        # Save processed version
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # Add to review queue
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # Move original out of inbox
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # Write review queue
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # Summary
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nProcessed: {len(review_queue)}")
    print(f"Critical: {critical}")
    print(f"Needs your attention: {needs_human}")
    print(f"Review queue: {review_path}")

if __name__ == "__main__":
    main()
```

**Wie das in der Praxis funktioniert:**
1. Kunden senden Feedback (per Formular, E-Mail oder Support-System)
2. Feedback landet als JSON-Dateien im Posteingangsverzeichnis
3. Der Agent verarbeitet jedes: klassifiziert, fasst zusammen, entwirft eine Antwort
4. Du öffnest die Überprüfungswarteschlange ein- bis zweimal am Tag
5. Für einfache Einträge (Lob, einfache Fragen mit guten Antwortentwürfen) genehmigst du den Entwurf
6. Für komplexe Einträge (Bugs, verärgerte Kunden) schreibst du eine persönliche Antwort
7. Nettozeit: 15 Minuten pro Tag statt 2 Stunden

### Das KI-entwirft-Mensch-genehmigt-Muster

Dieses Muster ist der Kern praktischer Stufe-4-Automatisierung. Der Agent erledigt die Schwerstarbeit. Du übernimmst die Urteilsentscheidungen.

```
              ┌─────────────┐
              │ Agent entwirft│
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │Überprüfungs- │
              │ warteschlange │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │Auto-senden│ │Bear-│ │Eskalieren│
    │ (Routine) │ │beiten│ │(komplex) │
    └───────────┘ └─────┘ └─────────┘
```

**Regeln dafür, was der Agent vollständig erledigt vs. was du überprüfst:**

| Agent erledigt vollständig (ohne Überprüfung) | Du überprüfst vor dem Senden |
|-----------------------------------------------|------------------------------|
| Empfangsbestätigungen ("Wir haben deine Nachricht erhalten") | Antworten an verärgerte Kunden |
| Statusupdates ("Deine Anfrage wird bearbeitet") | Priorisierung von Feature-Anfragen |
| FAQ-Antworten (exakte Übereinstimmung) | Alles, was Geld betrifft (Erstattungen, Preise) |
| Spam-Klassifikation und -Löschung | Bug-Reports (du musst verifizieren) |
| Interne Protokollierung und Kategorisierung | Alles, was du noch nie gesehen hast |

> **Häufiger Fehler:** Den Agenten vom ersten Tag an autonom auf Kunden antworten lassen. Tu das nicht. Fange damit an, dass der Agent alles entwirft und du alles genehmigst. Nach einer Woche lass ihn Empfangsbestätigungen automatisch senden. Nach einem Monat lass ihn FAQ-Antworten automatisch senden. Baue Vertrauen schrittweise auf — bei dir selbst und bei deinen Kunden.

### Du bist dran

1. Wähle eines: den MCP-Docs-Server ODER den Feedback-Verarbeitungsagenten bauen.
2. Passe es an dein Produkt/deinen Service an. Wenn du noch keine Kunden hast, verwende den Signalmonitor aus Lektion 3 als deinen "Kunden" — verarbeite dessen Ausgabe durch das Feedback-Agent-Muster.
3. Führe es 10 Mal manuell mit verschiedenen Eingaben aus.
4. Miss: Welcher Prozentsatz der Ausgaben ist ohne Bearbeitung verwendbar? Das ist dein Automatisierungsqualitäts-Score. Ziel: 70%+ bevor du es einplanst.

---

## Lektion 5: Das Human-in-the-Loop-Prinzip

*"Vollständige Automatisierung ist eine Falle. Teilweise Automatisierung ist eine Superkraft."*

### Warum 80% Automatisierung 100% schlägt

Es gibt einen spezifischen, messbaren Grund, warum du kundenorientierte Prozesse niemals vollständig automatisieren solltest: Die Kosten einer schlechten Ausgabe sind asymmetrisch.

Eine gute automatisierte Ausgabe spart dir 5 Minuten.
Eine schlechte automatisierte Ausgabe kostet dich einen Kunden, eine öffentliche Beschwerde, eine Erstattung oder einen Reputationsschaden, dessen Erholung Monate dauert.

Die Rechnung:

```
100% automation:
  1,000 outputs/month × 95% quality = 950 good + 50 bad
  50 bad outputs × $50 avg cost (refund + support + reputation) = $2,500/month in damage

80% automation + 20% human review:
  800 outputs auto-handled, 200 human-reviewed
  800 × 95% quality = 760 good + 40 bad auto
  200 × 99% quality = 198 good + 2 bad human
  42 total bad × $50 = $2,100/month in damage
  BUT: you catch 38 of the bad ones before they reach customers

  Actual bad outputs reaching customers: ~4
  Actual damage: ~$200/month
```

Das ist eine 12-fache Reduzierung der Schadenskosten. Deine Zeit für die Überprüfung von 200 Ausgaben (vielleicht 2 Stunden) spart dir $2.300/Monat an Schäden.

### Automatisiere niemals vollständig

Einige Dinge sollten immer einen Menschen im Kreislauf haben, egal wie gut die KI wird:

1. **Kundenorientierte Kommunikation** — Eine schlecht formulierte E-Mail kann einen Kunden für immer verlieren. Eine generische, offensichtlich KI-generierte Antwort kann Vertrauen erodieren. Überprüfe sie.

2. **Finanzielle Transaktionen** — Erstattungen, Preisänderungen, Rechnungsstellung. Immer überprüfen. Die Kosten eines Fehlers sind echtes Geld.

3. **Veröffentlichte Inhalte mit deinem Namen** — Dein Ruf baut sich über Jahre auf und kann in einem schlechten Post zerstört werden. Zehn Minuten Überprüfung sind eine billige Versicherung.

4. **Rechtliche oder Compliance-bezogene Ausgaben** — Alles, was Verträge, Datenschutzrichtlinien, AGB berührt. KI macht selbstsicher klingende juristische Fehler.

5. **Einstellungs- oder Personalentscheidungen** — Wenn du jemals outsourced, lass nie eine KI die endgültige Entscheidung treffen, mit wem du zusammenarbeitest.

### Automatisierungsschulden

{@ mirror automation_risk_profile @}

Automatisierungsschulden sind schlimmer als technische Schulden, weil sie unsichtbar sind, bis sie explodieren.

**Wie Automatisierungsschulden aussehen:**
- Ein Social-Media-Bot, der zur falschen Zeit postet, weil sich die Zeitzone geändert hat
- Eine Newsletter-Pipeline, die seit 3 Wochen einen kaputten Link enthält, weil niemand prüft
- Ein Preismonitor, der aufgehört hat zu funktionieren, als der Wettbewerber seine Seite neu gestaltet hat
- Ein Backup-Skript, das still fehlschlägt, weil die Festplatte voll ist

**Wie man sie verhindert:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — Monitor all your automations for silent failures.
Run every morning: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# Define expected outputs from each automation
AUTOMATIONS = [
    {
        "name": "Newsletter Pipeline",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # Should produce at least daily
    },
    {
        "name": "Social Poster",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # Check the file directly
        "max_age_hours": 2,  # Should update every 30 min
    },
    {
        "name": "Competitor Monitor",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # Should run every 6 hours
    },
    {
        "name": "Client Backup",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # Should run nightly
    },
    {
        "name": "Ollama Server",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """Check if automation has produced recent output."""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # Check for recent files matching pattern
        p = Path(path)
        if not p.exists():
            return False, f"Directory not found: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"No files matching {automation['pattern']} in {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # Check the file directly
        if not os.path.exists(path):
            return False, f"File not found: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Last output {age.total_seconds()/3600:.1f}h ago (max: {automation['max_age_hours']}h)"

    return True, f"OK (last output {age.total_seconds()/3600:.1f}h ago)"

def check_http(automation: dict) -> tuple[bool, str]:
    """Check if a service is responding."""
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"OK (HTTP {resp.status_code})"
        return False, f"Unexpected status: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"Connection failed: {e}"

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
            ok, msg = False, f"Unknown check type: {check_type}"

        status = "OK" if ok else "FAIL"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"AUTOMATION HEALTH CHECK — {len(failures)} FAILURE(S)\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nCheck logs and fix before these pile up."
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

Führe dies jeden Morgen aus. Wenn eine Automatisierung still kaputtgeht (und das wird sie), weißt du es innerhalb von 24 Stunden statt 3 Wochen.

### Überprüfungswarteschlangen aufbauen

Der Schlüssel, um Human-in-the-Loop effizient zu gestalten, ist das Bündeln deiner Überprüfung. Überprüfe nicht ein Element nach dem anderen, wenn sie eintreffen. Reihe sie ein und überprüfe sie in Batches.

```python
#!/usr/bin/env python3
"""
review_queue.py — A simple review queue for AI-generated outputs.
Review once or twice per day instead of constantly checking.
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """Add an item to the review queue."""
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
    """Show all pending items for review."""
    if not os.path.isdir(QUEUE_DIR):
        print("Queue is empty.")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("Queue is empty.")
        return

    print(f"\n{'='*60}")
    print(f"REVIEW QUEUE — {len(pending)} items pending")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    Articles: {content.get('article_count', '?')}")
            print(f"    Draft: {content.get('draft_path', 'unknown')}")
        elif item["type"] == "customer_response":
            print(f"    To: {content.get('customer', 'unknown')}")
            print(f"    Draft: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    Text: {content.get('text', '')[:100]}...")

        print(f"    Actions: [a]pprove  [e]dit  [r]eject  [s]kip")
        print()

    # In a real implementation, you'd add interactive input here
    # For batch processing, read decisions from a file or simple CLI

if __name__ == "__main__":
    review_queue()
```

**Die Überprüfungsgewohnheit:** Prüfe deine Überprüfungswarteschlange um 8 Uhr und um 16 Uhr. Zwei Sitzungen, 10-15 Minuten jeweils. Alles andere läuft autonom zwischen den Überprüfungen.

> **Klartext:** Überlege, was passiert, wenn du die menschliche Überprüfung weglässt: Du automatisierst deinen Newsletter vollständig, das LLM beginnt halluzinierte Links zu Seiten einzufügen, die nicht existieren, und Abonnenten bemerken es vor dir. Du verlierst einen Teil deiner Liste und es dauert Monate, das Vertrauen wieder aufzubauen. Im Gegensatz dazu fängt der Entwickler, der 80% des gleichen Prozesses automatisiert — LLM kuratiert und entwirft, er verbringt 10 Minuten mit der Überprüfung — diese Halluzinationen ab, bevor sie versendet werden. Der Unterschied ist nicht die Automatisierung. Es ist der Überprüfungsschritt.

### Du bist dran

1. Richte das `automation_healthcheck.py`-Skript für die Automatisierungen ein, die du in den Lektionen 2 und 3 gebaut hast. Plane es ein, jeden Morgen zu laufen.
2. Implementiere eine Überprüfungswarteschlange für deine riskanteste Automatisierungsausgabe (alles Kundenorientierte).
3. Verpflichte dich, die Überprüfungswarteschlange eine Woche lang zweimal täglich zu prüfen. Protokolliere, wie viele Einträge du unverändert genehmigst, wie viele du bearbeitest und wie viele du ablehnst. Diese Daten sagen dir, wie gut deine Automatisierung wirklich ist.

---

## Lektion 6: Kostenoptimierung und deine erste Pipeline

*"Wenn du aus $200 API-Ausgaben nicht $200 Umsatz generieren kannst, repariere das Produkt — nicht das Budget."*

### Die Wirtschaftlichkeit LLM-gesteuerter Automatisierung

Jeder LLM-Aufruf hat Kosten. Selbst lokale Modelle kosten Strom und GPU-Verschleiß. Die Frage ist, ob die Ausgabe dieses Aufrufs mehr Wert generiert als der Aufruf kostet.

{? if profile.gpu.exists ?}
Lokale Modelle auf {= profile.gpu.model | fallback("deiner GPU") =} zu betreiben kostet ungefähr {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("ein paar Dollar") =} Strom pro Monat für typische Pipeline-Arbeitslasten. Das ist die Baseline, die API-Alternativen schlagen müssen.
{? endif ?}

**Die {= regional.currency_symbol | fallback("$") =}200/Monat API-Budget-Regel:**

Wenn du {= regional.currency_symbol | fallback("$") =}200/Monat für API-Aufrufe für deine Automatisierungen ausgibst, sollten diese Automatisierungen mindestens {= regional.currency_symbol | fallback("$") =}200/Monat an Wert generieren — entweder direkte Einnahmen oder eingesparte Zeit, die du anderswo in Einnahmen umwandelst.

Wenn nicht: Das Problem ist nicht das API-Budget. Es ist das Pipeline-Design oder das Produkt, das sie unterstützt.

### Kosten-pro-Ausgabe-Tracking

Füge dies zu jeder Pipeline hinzu, die du baust:

```python
"""
cost_tracker.py — Track the cost of every LLM call and the value it generates.
Import this in your pipelines to get real cost data.
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# Pricing per 1M tokens (update as pricing changes)
PRICING = {
    # Local models — electricity cost estimate
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # API models
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
    """Log the cost of an LLM call."""
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
    """Generate a monthly cost/revenue summary."""
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

    # Print report
    print(f"\nLLM COST REPORT — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    Calls:    {data['call_count']}")
        print(f"    Tokens:   {data['total_tokens']:,}")
        print(f"    Cost:     ${data['total_cost']:.4f}")
        print(f"    Revenue:  ${data['total_revenue']:.2f}")
        print(f"    ROI:      {roi:.1f}x")

        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  TOTAL COST:    ${grand_cost:.4f}")
    print(f"  TOTAL REVENUE: ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  OVERALL ROI:   {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### Batching für API-Effizienz

Wenn du API-Modelle verwendest, spart Batching echtes Geld:

```python
"""
batch_api.py — Batch API calls for efficiency.
Instead of making 100 separate API calls, batch them.
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
    Classify multiple items efficiently by batching them into single API calls.

    Instead of 100 API calls (100 items × 1 call each):
      - 100 calls × ~500 input tokens = 50,000 tokens input
      - 100 calls × ~200 output tokens = 20,000 tokens output
      - Cost with Haiku: ~$0.12

    With batching (10 items per call, 10 API calls):
      - 10 calls × ~2,500 input tokens = 25,000 tokens input
      - 10 calls × ~1,000 output tokens = 10,000 tokens output
      - Cost with Haiku: ~$0.06

    50% savings from batching alone.
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # Format batch into a single prompt
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
            # Parse the JSON array from the response
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  Batch {i//batch_size + 1} failed: {e}")
            # Fall back to individual processing
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # Rate limiting courtesy
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### Caching: Zahle nicht zweimal für dieselbe Antwort

```python
"""
llm_cache.py — Cache LLM responses to avoid paying for duplicate processing.
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
    """Generate a deterministic cache key from model + prompt."""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """Get a cached response if available and fresh."""
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

    # Update hit count
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """Cache a response."""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """Show cache statistics."""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Cache entries: {total}")
    print(f"Total cache hits: {total_hits}")
    print(f"Estimated savings: ~${total_hits * 0.002:.2f} (rough avg per call)")
```

**Verwende es in deinen Pipelines:**

```python
# In any pipeline that calls an LLM:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # Free!

    response = call_llm(model, prompt)  # Your existing LLM call function
    set_cached(model, prompt, response)
    return response
```

Für Pipelines, die wiederholt die gleichen Inhaltstypen verarbeiten (Klassifikation, Extraktion), kann Caching 30-50% deiner API-Aufrufe eliminieren. Das sind 30-50% weniger auf deiner Monatsrechnung.

### Deine erste komplette Pipeline bauen: Schritt für Schritt

Hier ist der vollständige Prozess von "Ich habe einen manuellen Workflow" bis "Es läuft, während ich schlafe."

**Schritt 1: Bilde deinen aktuellen manuellen Prozess ab.**

Schreibe jeden Schritt auf, den du für einen bestimmten Einkommensstrom unternimmst. Beispiel für einen Newsletter:

```
1. Open 15 RSS feeds in browser tabs (10 min)
2. Scan headlines, open interesting ones (20 min)
3. Read 8-10 articles in detail (40 min)
4. Write summaries for top 5 (30 min)
5. Write intro paragraph (10 min)
6. Format in email tool (15 min)
7. Send to list (5 min)

Total: ~2 hours 10 minutes
```

**Schritt 2: Identifiziere die drei zeitaufwändigsten Schritte.**

Aus dem Beispiel: Artikel lesen (40 Min.), Zusammenfassungen schreiben (30 Min.), Überschriften scannen (20 Min.).

**Schritt 3: Automatisiere den einfachsten zuerst.**

Überschriften scannen ist am einfachsten zu automatisieren — es ist Klassifikation. Ein LLM bewertet die Relevanz, du liest nur die bestbewerteten.

**Schritt 4: Miss eingesparte Zeit und Qualität.**

Nach der Automatisierung des Überschriften-Scannens:
- Eingesparte Zeit: 20 Minuten
- Qualität: 90% Übereinstimmung mit deinen manuellen Entscheidungen
- Netto: 20 Minuten eingespart, vernachlässigbarer Qualitätsverlust

**Schritt 5: Automatisiere den nächsten Schritt.**

Jetzt automatisiere das Zusammenfassungsschreiben. Das LLM entwirft Zusammenfassungen, du bearbeitest sie.

**Schritt 6: Mache weiter bis zu abnehmenden Erträgen.**

```
Before automation: 2h 10min per newsletter
After Level 2 (scheduled fetching): 1h 45min
After Level 3 (LLM scoring + summaries): 25min
After Level 3+ (LLM drafts intro): 10min review only

Time saved per week: ~2 hours
Time saved per month: ~8 hours
At $100/hr effective rate: $800/month in freed time
API cost: $0 (local LLM) to $5/month (API)
```

**Schritt 7: Die komplette Pipeline, verbunden.**

Hier ist ein GitHub Action, der alles für eine wöchentliche Newsletter-Pipeline zusammenbindet:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # Every Sunday at 5 AM UTC
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
            -d '{"text":"Newsletter draft ready for review. Check GitHub Actions artifacts."}'
```

Dies läuft jeden Sonntag um 5 Uhr morgens. Bis du aufwachst, wartet der Entwurf. Du verbringst 10 Minuten mit der Überprüfung bei einem Kaffee, drückst auf Senden, und dein Newsletter ist für die Woche veröffentlicht.

### Du bist dran: Baue deine Pipeline

Dies ist das Modulergebnis. Am Ende dieser Lektion solltest du eine komplette Pipeline bereitgestellt und am Laufen haben.

**Anforderungen an deine Pipeline:**
1. Sie läuft nach Zeitplan ohne dein Zutun
2. Sie enthält mindestens einen LLM-Verarbeitungsschritt
3. Sie hat einen menschlichen Überprüfungsschritt zur Qualitätskontrolle
4. Sie hat einen Health-Check, damit du weißt, wenn sie kaputtgeht
5. Sie ist mit einem echten Einkommensstrom verbunden (oder einem, den du aufbaust)

**Checkliste:**

- [ ] Einen Einkommensstrom zur Automatisierung gewählt
- [ ] Den manuellen Prozess abgebildet (alle Schritte, mit Zeitschätzungen)
- [ ] Die 3 zeitaufwändigsten Schritte identifiziert
- [ ] Mindestens den ersten Schritt automatisiert (Klassifikation/Bewertung/Filterung)
- [ ] LLM-Verarbeitung für den zweiten Schritt hinzugefügt (Zusammenfassung/Generierung/Extraktion)
- [ ] Eine Überprüfungswarteschlange für menschliche Aufsicht gebaut
- [ ] Einen Health-Check für die Automatisierung eingerichtet
- [ ] Nach Zeitplan bereitgestellt (cron, GitHub Actions oder Systemd-Timer)
- [ ] Kosten und Zeitersparnis für einen vollständigen Zyklus verfolgt
- [ ] Die Pipeline dokumentiert (was sie tut, wie man sie repariert, was zu überwachen ist)

Wenn du alle zehn Punkte auf dieser Checkliste erledigt hast, hast du eine Stufe-3-Automatisierung am Laufen. Du hast gerade Stunden deiner Woche freigesetzt, die du in den Aufbau weiterer Ströme oder die Verbesserung bestehender investieren kannst.

---

## Modul T: Abgeschlossen

{@ temporal automation_progress @}

### Was du in zwei Wochen aufgebaut hast

1. **Ein Verständnis der Automatisierungspyramide** — du weißt, wo du stehst und wohin sich jeder deiner Einkommensströme bewegen sollte.
2. **Geplante Automatisierungen**, die auf cron oder Cloud-Schedulern laufen — das unscheinbare Fundament, das alles andere möglich macht.
3. **LLM-gesteuerte Pipelines**, die die Urteilsentscheidungen übernehmen, die du früher manuell getroffen hast — klassifizieren, zusammenfassen, generieren, überwachen.
4. **Agentenbasierte Muster**, die du für Kundeninteraktion, Feedback-Verarbeitung und MCP-basierte Produkte einsetzen kannst.
5. **Ein Human-in-the-Loop-Framework**, das deinen Ruf schützt und trotzdem 80%+ deiner Zeit spart.
6. **Kosten-Tracking und -Optimierung**, damit deine Automatisierungen Gewinn generieren, nicht nur Aktivität.
7. **Eine komplette, bereitgestellte Pipeline**, die Wert ohne deine aktive Beteiligung generiert.

### Der Zinseszinseffekt

Das passiert in den nächsten 3 Monaten, wenn du das, was du in diesem Modul gebaut hast, pflegst und erweiterst:

```
Month 1: One pipeline, saving 5-8 hours/week
Month 2: Two pipelines, saving 10-15 hours/week
Month 3: Three pipelines, saving 15-20 hours/week

At $100/hr effective rate, that's $1,500-2,000/month
in freed time — time you invest in new streams.

The freed time from Month 1 builds the pipeline for Month 2.
The freed time from Month 2 builds the pipeline for Month 3.
Automation compounds.
```

So arbeitet ein Entwickler wie ein Team von fünf. Nicht durch härteres Arbeiten. Durch den Aufbau von Systemen, die arbeiten, während du es nicht tust.

---

### 4DA-Integration

{? if dna.identity_summary ?}
Basierend auf deinem Entwicklerprofil — {= dna.identity_summary | fallback("dein Entwicklungsfokus") =} — passen die unten aufgeführten 4DA-Werkzeuge direkt zu den Automatisierungsmustern, die du gerade gelernt hast. Die Signalklassifikations-Werkzeuge sind besonders relevant für Entwickler in deinem Bereich.
{? endif ?}

4DA ist selbst eine Stufe-3-Automatisierung. Es nimmt Inhalte aus Dutzenden von Quellen auf, bewertet jedes Element mit dem PASIFA-Algorithmus und zeigt nur das an, was für deine Arbeit relevant ist — alles ohne dass du einen Finger rührst. Du prüfst nicht manuell Hacker News, Reddit und 50 RSS-Feeds. 4DA tut es und zeigt dir, was wichtig ist.

Baue deine Einkommenspipelines auf die gleiche Weise.

4DAs Aufmerksamkeitsbericht (`/attention_report` in den MCP-Werkzeugen) zeigt dir, wo deine Zeit tatsächlich hingeht versus wo sie hingehen sollte. Führe ihn aus, bevor du entscheidest, was du automatisieren willst. Die Lücke zwischen "investierte Zeit" und "sollte investierte Zeit" ist deine Automatisierungs-Roadmap.

Die Signalklassifikations-Werkzeuge (`/get_actionable_signals`) können direkt in deine Marktüberwachungspipeline einfließen — wobei 4DAs Intelligenzschicht die anfängliche Bewertung übernimmt, bevor deine individuelle Pipeline die nischenspezifische Analyse durchführt.

Wenn du Pipelines baust, die Quellen auf Gelegenheiten hin überwachen, erfinde nicht neu, was 4DA bereits tut. Verwende seinen MCP-Server als Baustein in deinem Automatisierungs-Stack.

---

### Was als Nächstes kommt: Modul S — Ströme stapeln

Modul T hat dir die Werkzeuge gegeben, um jeden Einkommensstrom effizient zum Laufen zu bringen. Modul S (Ströme stapeln) beantwortet die nächste Frage: **Wie viele Ströme solltest du betreiben, und wie passen sie zusammen?**

Das behandelt Modul S:

- **Portfoliotheorie für Einkommensströme** — warum 3 Ströme 1 Strom schlagen und warum 10 Ströme keinen schlagen
- **Stromkorrelation** — welche Ströme sich gegenseitig verstärken und welche um deine Zeit konkurrieren
- **Die Einkommensuntergrenze** — eine Basis wiederkehrender Einnahmen aufbauen, die deine Kosten deckt, bevor du experimentierst
- **Rebalancing** — wann du bei einem Gewinner verdoppelst und wann du einen Underperformer eliminierst
- **Die $10K/Monat-Architektur** — spezifische Stromkombinationen, die mit 15-20 Stunden pro Woche fünfstellig werden

Du hast die Infrastruktur (Modul S), die Gräben (Modul T), die Motoren (Modul R), das Launch-Playbook (Modul E), das Trendradar (Modul E) und jetzt die Automatisierung (Modul T). Modul S verbindet alles zu einem nachhaltigen, wachsenden Einkommensportfolio.

---

**Die Pipeline läuft. Der Entwurf ist fertig. Du verbringst 10 Minuten mit der Überprüfung.**

**Das ist taktische Automatisierung. So skalierst du.**

*Dein Rig. Deine Regeln. Deine Einnahmen.*
