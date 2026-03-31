# Module T : Automatisation Tactique

**Cours STREETS de Revenus pour Developpeurs — Module Payant**
*Semaines 12-13 | 6 Lecons | Livrable : Un Pipeline Automatise Generant de la Valeur*

> "LLMs, agents, MCP et cron jobs comme multiplicateurs de force."

---

Tu as des moteurs de revenus qui tournent. Tu as des clients. Tu as des processus qui fonctionnent. Et tu passes 60-70% de ton temps a faire les memes choses encore et encore : traiter des entrees, formater des sorties, verifier des moniteurs, envoyer des mises a jour, examiner des files d'attente.

Ce temps est ta ressource la plus chere, et tu le brules sur des taches qu'un VPS a {= regional.currency_symbol | fallback("$") =}5/mois pourrait gerer.

{@ insight hardware_benchmark @}

Ce module traite de te retirer systematiquement de la boucle — pas completement (c'est un piege que nous couvrirons dans la Lecon 5), mais des 80% du travail qui ne necessite pas ton jugement. Le resultat : tes flux de revenus produisent de l'argent pendant que tu dors, pendant que tu es a ton travail, pendant que tu construis la prochaine chose.

A la fin de ces deux semaines, tu auras :

- Une comprehension claire des quatre niveaux d'automatisation et ou tu te situes aujourd'hui
- Des cron jobs fonctionnels et des automatisations planifiees qui tournent sur ton infrastructure
- Au moins un pipeline alimente par LLM qui traite des entrees sans ton intervention
- Une comprehension des systemes bases sur des agents et quand ils ont un sens economique
- Un cadre humain-dans-la-boucle pour que l'automatisation ne detruise pas ta reputation
- Un pipeline complet et deploye qui genere de la valeur sans ta participation active

{? if stack.primary ?}
Ton stack principal est {= stack.primary | fallback("ton stack principal") =}, donc les exemples d'automatisation qui suivent seront les plus directement applicables lorsqu'ils sont adaptes a cet ecosysteme. La plupart des exemples utilisent Python pour la portabilite, mais les patterns se traduisent dans n'importe quel langage.
{? endif ?}

C'est le module le plus charge en code du cours. Au moins la moitie de ce qui suit est du code executable. Copie-le, adapte-le, deploie-le.

Automatisons.

---

## Lecon 1 : La Pyramide d'Automatisation

*"La plupart des developpeurs automatisent au Niveau 1. L'argent est au Niveau 3."*

### Les Quatre Niveaux

Chaque automatisation dans ton stack de revenus se situe quelque part dans cette pyramide :

```
┌───────────────────────────────┐
│  Niveau 4 : Agents Autonomes  │  ← Prend des decisions pour toi
│  (IA decide ET agit)         │
├───────────────────────────────┤
│  Niveau 3 : Pipelines         │  ← L'argent est ici
│  Intelligents (avec LLM)     │
├───────────────────────────────┤
│  Niveau 2 : Automatisation    │  ← La plupart s'arretent ici
│  Planifiee (cron + scripts)  │
├───────────────────────────────┤
│  Niveau 1 : Manuel avec       │  ← Ou se trouve la majorite
│  Modeles (copier-coller)     │
└───────────────────────────────┘
```

Soyons specifiques sur ce a quoi ressemble chaque niveau en pratique.

### Niveau 1 : Manuel avec Modeles

Tu fais le travail, mais tu as des checklists, des modeles et des snippets pour accelerer les choses.

**Exemples :**
- Tu ecris un article de blog avec un modele markdown avec un frontmatter pre-rempli
- Tu factures tes clients en dupliquant la facture du mois dernier et en changeant les chiffres
- Tu reponds aux emails de support avec des reponses enregistrees
- Tu publies du contenu en executant manuellement une commande de deploiement

**Cout en temps :** 100% de ton temps par unite de production.
**Taux d'erreur :** Modere — tu es humain, tu fais des erreurs quand tu es fatigue.
**Plafond d'echelle :** Toi. Tes heures. C'est tout.

La plupart des developpeurs vivent ici et ne realisent meme pas qu'il y a une pyramide au-dessus d'eux.

### Niveau 2 : Automatisation Planifiee

Les scripts tournent selon un calendrier. Tu as ecrit la logique une fois. Elle s'execute sans toi.

**Exemples :**
- Un cron job qui verifie ton flux RSS et publie les nouveaux articles sur les reseaux sociaux
- Un GitHub Action qui construit et deploie ton site chaque matin a 6h
- Un script qui tourne toutes les heures pour verifier les prix des concurrents et enregistrer les changements
- Une sauvegarde quotidienne de base de donnees qui tourne a 3h du matin

**Cout en temps :** Zero en continu (apres la configuration initiale de 1-4 heures).
**Taux d'erreur :** Faible — deterministe, la meme logique a chaque fois.
**Plafond d'echelle :** Autant de taches que ta machine peut planifier. Des centaines.

C'est la ou la plupart des developpeurs techniques atterrissent. C'est confortable. Mais ca a une limite dure : ca ne peut gerer que des taches avec une logique deterministe. Si la tache necessite du jugement, tu es bloque.

### Niveau 3 : Pipelines Intelligents

Les scripts tournent selon un calendrier, mais ils incluent un LLM qui gere les decisions de jugement.

**Exemples :**
- Les flux RSS sont ingeres, le LLM resume chaque article, redige une newsletter, tu revois pendant 10 minutes et tu appuies sur envoyer
- Les emails de feedback des clients sont classes par sentiment et urgence, les reponses pre-redigees sont mises en file d'attente pour ton approbation
- Les nouvelles offres d'emploi dans ta niche sont scrapees, le LLM evalue la pertinence, tu recois un resume quotidien de 5 opportunites au lieu de parcourir 200 annonces
- Les articles de blog des concurrents sont surveilles, le LLM extrait les changements cles de produit, tu recois un rapport hebdomadaire de veille concurrentielle

**Cout en temps :** 10-20% du temps manuel. Tu revois et approuves au lieu de creer.
**Taux d'erreur :** Faible pour les taches de classification, modere pour la generation (c'est pourquoi tu revois).
**Plafond d'echelle :** Des milliers d'elements par jour. Ton goulot d'etranglement est le cout de l'API, pas ton temps.

**C'est ici que se trouve l'argent.** Le Niveau 3 permet a une seule personne d'operer des flux de revenus qui necessiteraient normalement une equipe de 3-5 personnes.

### Niveau 4 : Agents Autonomes

Des systemes d'IA qui observent, decident et agissent sans ton intervention.

**Exemples :**
- Un agent qui surveille les metriques de ton SaaS, detecte une baisse d'inscriptions, effectue un test A/B sur un changement de prix et revient en arriere si ca ne fonctionne pas
- Un agent de support qui gere les questions clients de Tier 1 de facon entierement autonome, n'escaladant vers toi que pour les problemes complexes
- Un agent de contenu qui identifie les sujets tendance, genere des brouillons, planifie la publication et surveille la performance

**Cout en temps :** Quasi nul pour les cas traites. Tu examines les metriques, pas les actions individuelles.
**Taux d'erreur :** Depend entierement de tes garde-fous. Sans eux : eleve. Avec de bons garde-fous : etonnamment faible pour des domaines etroits.
**Plafond d'echelle :** Effectivement illimite pour les taches dans le perimetre de l'agent.

Le Niveau 4 est reel et atteignable, mais ce n'est pas le point de depart. Et comme nous le verrons dans la Lecon 5, les agents entierement autonomes orientes client sont dangereux pour ta reputation s'ils sont mal implementes.

> **Parlons franchement :** Si tu es au Niveau 1 en ce moment, n'essaie pas de sauter au Niveau 4. Tu passeras des semaines a construire un "agent autonome" qui casse en production et endommage la confiance des clients. Monte la pyramide un niveau a la fois. Le Niveau 2 est un apres-midi de travail. Le Niveau 3 est un projet de week-end. Le Niveau 4 arrive apres avoir fait tourner le Niveau 3 de facon fiable pendant un mois.

### Auto-evaluation : Ou en es-tu ?

Pour chacun de tes flux de revenus, evalue-toi honnetement :

| Flux de Revenus | Niveau Actuel | Heures/Semaine | Pourrait Automatiser A |
|-----------------|--------------|---------------|----------------------|
| [ex., Newsletter] | [1-4] | [X] hrs | [niveau cible] |
| [ex., Traitement clients] | [1-4] | [X] hrs | [niveau cible] |
| [ex., Reseaux sociaux] | [1-4] | [X] hrs | [niveau cible] |
| [ex., Support] | [1-4] | [X] hrs | [niveau cible] |

La colonne qui compte le plus est "Heures/Semaine." Le flux avec le plus d'heures et le niveau le plus bas est ta premiere cible d'automatisation. C'est celui avec le meilleur ROI.

### L'Economie de Chaque Niveau

Disons que tu as un flux de revenus qui prend 10 heures/semaine de ton temps et genere {= regional.currency_symbol | fallback("$") =}2 000/mois :

| Niveau | Ton Temps | Ton Taux Effectif | Cout d'Automatisation |
|--------|----------|------------------|----------------------|
| Niveau 1 | 10 hrs/semaine | $50/hr | $0 |
| Niveau 2 | 3 hrs/semaine | $167/hr | $5/mois (VPS) |
| Niveau 3 | 1 hr/semaine | $500/hr | $30-50/mois (API) |
| Niveau 4 | 0,5 hrs/semaine | $1 000/hr | $50-100/mois (API + calcul) |

Passer du Niveau 1 au Niveau 3 ne change pas tes revenus. Ca change ton taux horaire effectif de $50 a $500. Et ces 9 heures liberees ? Elles vont a la construction du prochain flux de revenus ou a l'amelioration de l'actuel.

> **Erreur Courante :** Automatiser ton flux a plus faible revenu en premier parce que c'est "plus facile." Non. Automatise le flux qui consomme le plus d'heures par rapport a son revenu. C'est la que se trouve le ROI.

### A Toi

1. Remplis le tableau d'auto-evaluation ci-dessus pour chaque flux de revenus (ou flux prevu) que tu as.
2. Identifie ta cible d'automatisation a plus haut ROI : le flux avec le plus d'heures et le plus bas niveau d'automatisation.
3. Note les 3 taches les plus chronophages dans ce flux. Tu automatiseras la premiere dans la Lecon 2.

---

## Lecon 2 : Du Niveau 1 au 2 — Automatisation Planifiee

*"Cron date de 1975. Ca marche encore. Utilise-le."*

### Les Fondamentaux des Cron Jobs

{? if computed.os_family == "windows" ?}
Tu es sur Windows, donc cron n'est pas natif sur ton systeme. Tu as deux options : utiliser WSL (Windows Subsystem for Linux) pour avoir un vrai cron, ou utiliser le Planificateur de taches Windows (couvert ci-dessous). WSL est recommande si tu es a l'aise avec — tous les exemples cron de cette lecon fonctionnent directement dans WSL. Si tu preferes Windows natif, passe a la section Planificateur de taches apres ceci.
{? endif ?}

Oui, meme en 2026, cron est le roi des taches planifiees. C'est fiable, c'est partout, et ca ne necessite pas de compte cloud, d'abonnement SaaS ou de schema YAML que tu dois chercher sur Google a chaque fois.

**La syntaxe cron en 30 secondes :**

```
┌───────── minute (0-59)
│ ┌───────── heure (0-23)
│ │ ┌───────── jour du mois (1-31)
│ │ │ ┌───────── mois (1-12)
│ │ │ │ ┌───────── jour de la semaine (0-7, 0 et 7 = Dimanche)
│ │ │ │ │
* * * * *  commande
```

**Calendriers courants :**

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

**Configurer un cron job :**

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

> **Erreur Courante :** Ecrire un script qui fonctionne parfaitement quand tu l'executes manuellement, puis il echoue silencieusement dans cron parce que cron ne charge pas ton `.bashrc` ou `.zshrc`. Utilise toujours des chemins absolus dans les scripts cron. Definis toujours `PATH` en haut de ton crontab. Redirige toujours la sortie vers un fichier de log.

### Planificateurs Cloud quand Cron ne Suffit Pas

Si ta machine n'est pas allumee 24/7, ou si tu as besoin de quelque chose de plus robuste, utilise un planificateur cloud :

**GitHub Actions (gratuit pour les repos publics, 2 000 min/mois sur les prives) :**

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

**Vercel Cron (gratuit sur le plan Hobby, 1 par jour ; plan Pro : illimite) :**

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

### Des Automatisations Reelles a Construire Maintenant

Voici cinq automatisations que tu peux implementer aujourd'hui. Chacune prend 30-60 minutes et elimine des heures de travail manuel hebdomadaire.

#### Automatisation 1 : Auto-Publier du Contenu selon un Calendrier

Tu ecris des articles de blog a l'avance. Ce script les publie a l'heure prevue.

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

**Tes articles markdown ressemblent a ceci :**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Your post content here...
```

Ecris des articles quand l'inspiration vient. Fixe la date. Le script s'occupe du reste.

#### Automatisation 2 : Auto-Poster sur les Reseaux Sociaux pour du Nouveau Contenu

Quand ton blog publie quelque chose de nouveau, ceci poste automatiquement sur Twitter/X et Bluesky.

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

Cout : $0. Tourne sur ta machine ou un GitHub Action gratuit.

#### Automatisation 3 : Moniteur de Prix des Concurrents

Sois informe a l'instant ou un concurrent change ses prix. Plus besoin de verifier manuellement chaque semaine.

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

#### Automatisation 4 : Rapport Hebdomadaire de Revenus

Chaque lundi matin, ceci genere un rapport a partir de tes donnees de revenus et te l'envoie par email.

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

#### Automatisation 5 : Auto-Sauvegarde des Donnees Clients

Ne perds jamais de livrables clients. Ceci tourne chaque nuit et conserve 30 jours de sauvegardes.

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

### Timers Systemd pour Plus de Controle

Si tu as besoin de plus que ce que cron offre — comme l'ordonnancement des dependances, les limites de ressources ou le retry automatique — utilise les timers systemd :

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
### Alternative : Planificateur de Taches Windows

Si tu n'utilises pas WSL, le Planificateur de taches Windows fait le meme travail. Utilise `schtasks` depuis la ligne de commande ou l'interface graphique du Planificateur de taches (`taskschd.msc`). La difference cle : cron utilise une seule expression, le Planificateur de taches utilise des champs separes pour les declencheurs, actions et conditions. Chaque exemple cron de cette lecon se traduit directement — planifie tes scripts Python de la meme maniere, juste a travers une interface differente.
{? endif ?}

### A Toi

1. Choisis l'automatisation la plus simple de cette lecon qui s'applique a ton flux de revenus.
2. Implemente-la. Pas "prevois de l'implementer." Ecris le code, teste-le, planifie-le.
3. Configure la journalisation pour pouvoir verifier que ca tourne. Verifie les logs chaque matin pendant 3 jours.
4. Une fois que c'est stable, arrete de verifier quotidiennement. Verifie hebdomadairement. Ca, c'est l'automatisation.

**Minimum :** Un cron job tournant de facon fiable a la fin de la journee.

---

## Lecon 3 : Du Niveau 2 au 3 — Pipelines Alimentes par LLM

*"Ajoute de l'intelligence a tes automatisations. C'est la qu'une personne commence a ressembler a une equipe."*

### Le Pattern

Chaque pipeline alimente par LLM suit la meme forme :

```
Sources d'Entree → Ingerer → Traitement LLM → Formater la Sortie → Livrer (ou Mettre en File pour Revision)
```

La magie est dans l'etape "Traitement LLM". Au lieu d'ecrire des regles deterministes pour chaque cas possible, tu decris ce que tu veux en langage naturel, et le LLM gere les decisions de jugement.

### Quand Utiliser Local vs API

{? if settings.has_llm ?}
Tu as {= settings.llm_provider | fallback("un fournisseur LLM") =} configure avec {= settings.llm_model | fallback("ton modele LLM") =}. Ca veut dire que tu peux commencer a construire des pipelines intelligents immediatement. La decision ci-dessous t'aide a choisir quand utiliser ta configuration locale versus une API pour chaque pipeline.
{? else ?}
Tu n'as pas encore de LLM configure. Les pipelines de cette lecon fonctionnent a la fois avec des modeles locaux (Ollama) et des APIs cloud. Configure au moins un avant de construire ton premier pipeline — Ollama est gratuit et prend 10 minutes a installer.
{? endif ?}

Cette decision a un impact direct sur tes marges :

| Facteur | Local (Ollama) | API (Claude, GPT) |
|---------|---------------|-------------------|
| **Cout par 1M tokens** | ~$0,003 (electricite) | $0,15 - $15,00 |
| **Vitesse (tokens/sec)** | 20-60 (8B sur GPU milieu de gamme) | 50-100+ |
| **Qualite (8B local vs API)** | Bonne pour la classification, l'extraction | Meilleure pour la generation, le raisonnement |
| **Confidentialite** | Les donnees ne quittent jamais ta machine | Les donnees vont chez le fournisseur |
| **Disponibilite** | Depend de ta machine | 99,9%+ |
| **Capacite par lots** | Limitee par la memoire GPU | Limitee par les limites de debit et le budget |

{? if profile.gpu.exists ?}
Avec {= profile.gpu.model | fallback("ton GPU") =} sur ta machine, l'inference locale est une option solide. La vitesse et la taille du modele que tu peux executer dependent de ton VRAM — verifie ce qui rentre avant de t'engager sur un pipeline exclusivement local.
{? if computed.has_nvidia ?}
Les GPUs NVIDIA obtiennent les meilleures performances Ollama grace a l'acceleration CUDA. Tu devrais pouvoir executer des modeles de 7-8B parametres confortablement, et peut-etre plus grands selon ton {= profile.gpu.vram | fallback("VRAM disponible") =}.
{? endif ?}
{? else ?}
Sans GPU dedie, l'inference locale sera plus lente (CPU uniquement). Ca marche quand meme pour les petits jobs par lots et les taches de classification, mais pour tout ce qui est sensible au temps ou a haut volume, un modele API sera plus pratique.
{? endif ?}

**Regles generales :**
- **Haut volume, barre de qualite plus basse** (classification, extraction, etiquetage) → Local
- **Faible volume, qualite critique** (contenu oriente client, analyse complexe) → API
- **Donnees sensibles** (infos clients, donnees proprietaires) → Local, toujours
- **Plus de 10 000 elements/mois** → Local economise de l'argent reel

**Comparaison des couts mensuels pour un pipeline typique :**

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

Pour les pipelines de classification et d'extraction, la difference de qualite entre un modele local 8B bien configure et un modele API de pointe est souvent negligeable. Teste les deux. Utilise le moins cher qui atteint ta barre de qualite.

{@ insight cost_projection @}

### Pipeline 1 : Generateur de Contenu Newsletter

C'est l'automatisation LLM la plus courante pour les developpeurs avec des revenus bases sur le contenu. Les flux RSS entrent, un brouillon de newsletter sort.

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

**Ce que ca coute :**
- Traiter 50 articles/jour avec un modele local 8B : ~$0/mois
- Ton temps : 10 minutes de revision du brouillon vs 2 heures de curation manuelle
- Temps economise par semaine : ~10 heures si tu geres une newsletter hebdomadaire

### Pipeline 2 : Rapports de Recherche et d'Analyse Clients

Ce pipeline scrape des donnees publiques, les analyse avec un LLM et produit un rapport que tu peux vendre.

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

**Modele commercial :** Facture $200-500 par rapport de recherche personnalise. Ton cout : $0,05 en appels API et 15 minutes de revision. Tu peux produire 3-4 rapports par heure une fois que le pipeline est stable.

### Pipeline 3 : Moniteur de Signaux de Marche

C'est le pipeline qui te dit quoi construire ensuite. Il surveille plusieurs sources, classe les signaux et t'alerte quand une opportunite depasse ton seuil.

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

**Ce que ca fait en pratique :** Tu recois une notification Slack 2-3 fois par semaine disant quelque chose comme "OPPORTUNITE : Nouveau framework lance sans starter kit — tu pourrais en construire un ce week-end." Ce signal, agir dessus avant les autres, c'est comme ca que tu gardes une longueur d'avance.

> **Parlons franchement :** La qualite des sorties de ces pipelines depend entierement de tes prompts et de ta definition de niche. Si ta niche est vague ("Je suis developpeur web"), le LLM signalera tout. Si elle est specifique ("Je construis des apps desktop Tauri pour le marche des developpeurs soucieux de la confidentialite"), il sera chirurgicalement precis. Passe 30 minutes a bien definir ta niche. C'est l'entree individuelle avec le plus grand levier pour chaque pipeline que tu construis.

### A Toi

{? if stack.contains("python") ?}
Bonne nouvelle : les exemples de pipeline ci-dessus sont deja dans ton langage principal. Tu peux les copier directement et commencer a les adapter. Concentre-toi sur la definition de niche et les prompts — c'est de la que viennent 90% de la qualite de la sortie.
{? else ?}
Les exemples ci-dessus utilisent Python pour la portabilite, mais les patterns fonctionnent dans n'importe quel langage. Si tu preferes construire en {= stack.primary | fallback("ton stack principal") =}, les pieces cles a repliquer sont : client HTTP pour la recuperation RSS/API, parsing JSON pour les reponses LLM, et I/O fichier pour la gestion d'etat. L'interaction avec le LLM est juste un POST HTTP vers Ollama ou une API cloud.
{? endif ?}

1. Choisis l'un des trois pipelines ci-dessus (newsletter, recherche ou moniteur de signaux).
2. Adapte-le a ta niche. Change les feeds, la description de l'audience, les criteres de classification.
3. Execute-le manuellement 3 fois pour tester la qualite de la sortie.
4. Ajuste les prompts jusqu'a ce que la sortie soit utile sans edition lourde.
5. Planifie-le avec cron.

**Objectif :** Un pipeline alimente par LLM tournant selon un calendrier dans les 48 heures suivant la lecture de cette lecon.

---

## Lecon 4 : Du Niveau 3 au 4 — Systemes Bases sur des Agents

*"Un agent est juste une boucle qui observe, decide et agit. Construis-en un."*

### Ce que "Agent" Signifie Vraiment en 2026

Enleve le battage mediatique. Un agent est un programme qui :

1. **Observe** — lit une entree ou un etat
2. **Decide** — utilise un LLM pour determiner quoi faire
3. **Agit** — execute la decision
4. **Boucle** — retourne a l'etape 1

C'est tout. La difference entre un pipeline (Niveau 3) et un agent (Niveau 4) est que l'agent boucle. Il agit sur sa propre sortie. Il gere des taches multi-etapes ou la prochaine etape depend du resultat de la precedente.

Un pipeline traite des elements un par un a travers une sequence fixe. Un agent navigue une sequence imprevisible basee sur ce qu'il rencontre.

### Serveurs MCP qui Servent les Clients

Un serveur MCP est l'un des systemes pratiques les plus proches des agents que tu peux construire. Il expose des outils qu'un agent IA (Claude Code, Cursor, etc.) peut appeler au nom de tes clients.

{? if stack.contains("typescript") ?}
L'exemple de serveur MCP ci-dessous utilise TypeScript — pile dans ton domaine. Tu peux l'etendre avec ton outillage TypeScript existant et le deployer a cote de tes autres services Node.js.
{? endif ?}

Voici un exemple reel : un serveur MCP qui repond aux questions des clients a partir de la documentation de ton produit.

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

**Modele commercial :** Donne ce serveur MCP a tes clients comme partie de ton produit. Ils obtiennent des reponses instantanees a leurs questions sans creer de tickets de support. Tu passes moins de temps sur le support. Tout le monde y gagne.

Pour le premium : facture $9-29/mois pour une version hebergee avec recherche vectorielle, documentation versionnee et analytiques sur ce que demandent les clients.

### Traitement Automatise du Feedback Client

Cet agent lit le feedback des clients (par email, tickets de support ou formulaire), le classe et cree des brouillons de reponses et des tickets de fonctionnalites.

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

**Comment ca fonctionne en pratique :**
1. Les clients soumettent du feedback (via formulaire, email ou systeme de support)
2. Le feedback arrive sous forme de fichiers JSON dans le repertoire de boite de reception
3. L'agent traite chacun : classe, resume, redige une reponse
4. Tu ouvres la file de revision une ou deux fois par jour
5. Pour les elements simples (eloges, questions basiques avec de bons brouillons de reponse), tu approuves le brouillon
6. Pour les elements complexes (bugs, clients mecontents), tu ecris une reponse personnelle
7. Temps net : 15 minutes par jour au lieu de 2 heures

### Le Pattern IA Redige, Humain Approuve

Ce pattern est le coeur de l'automatisation pratique de Niveau 4. L'agent gere le travail fastidieux. Tu geres les decisions de jugement.

```
              ┌─────────────┐
              │ Agent redige │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ File de      │
              │  Revision    │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │Auto-envoi │ │Modi-│ │ Escala- │
    │ (routine) │ │fier │ │der      │
    └───────────┘ └─────┘ └─────────┘
```

**Regles pour ce que l'agent gere entierement vs ce que tu revois :**

| L'agent gere entierement (sans revision) | Tu revois avant d'envoyer |
|-----------------------------------------|--------------------------|
| Accuses de reception ("Nous avons recu ton message") | Reponses aux clients mecontents |
| Mises a jour de statut ("Ta demande est en cours de traitement") | Priorisation des demandes de fonctionnalites |
| Reponses FAQ (correspondance exacte) | Tout ce qui implique de l'argent (remboursements, tarifs) |
| Classification et suppression de spam | Rapports de bugs (tu dois verifier) |
| Journalisation et categorisation internes | Tout ce que tu n'as jamais vu auparavant |

> **Erreur Courante :** Laisser l'agent repondre aux clients de facon autonome des le premier jour. Ne fais pas ca. Commence avec l'agent qui redige tout, toi qui approuves tout. Apres une semaine, laisse-le envoyer automatiquement les accuses de reception. Apres un mois, laisse-le envoyer automatiquement les reponses FAQ. Construis la confiance progressivement — avec toi-meme et avec tes clients.

### A Toi

1. Choisis un : construire le serveur MCP de documentation OU l'agent de traitement du feedback.
2. Adapte-le a ton produit/service. Si tu n'as pas encore de clients, utilise le moniteur de signaux de la Lecon 3 comme ton "client" — traite sa sortie a travers le pattern d'agent de feedback.
3. Execute-le manuellement 10 fois avec differentes entrees.
4. Mesure : quel pourcentage des sorties sont utilisables sans edition ? C'est ton score de qualite d'automatisation. Vise 70%+ avant de planifier.

---

## Lecon 5 : Le Principe du Humain dans la Boucle

*"L'automatisation complete est un piege. L'automatisation partielle est un superpouvoir."*

### Pourquoi 80% d'Automatisation Bat 100%

Il y a une raison specifique et mesurable pour laquelle tu ne devrais jamais automatiser completement les processus orientes client : le cout d'une mauvaise sortie est asymetrique.

Une bonne sortie automatisee te fait gagner 5 minutes.
Une mauvaise sortie automatisee te coute un client, une plainte publique, un remboursement ou un coup a la reputation qui prend des mois a recuperer.

Le calcul :

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

C'est une reduction de 12x du cout des dommages. Ton temps a revoir 200 sorties (peut-etre 2 heures) t'economise $2 300/mois en dommages.

### Ne Jamais Automatiser Completement Ceci

Certaines choses devraient toujours avoir un humain dans la boucle, peu importe la qualite de l'IA :

1. **Communication orientee client** — Un email mal formule peut perdre un client pour toujours. Une reponse generique, visiblement generee par IA, peut eroder la confiance. Revois-le.

2. **Transactions financieres** — Remboursements, changements de prix, facturation. Toujours revoir. Le cout d'une erreur est de l'argent reel.

3. **Contenu publie avec ton nom** — Ta reputation se compose sur des annees et peut etre detruite en un mauvais post. Dix minutes de revision sont une assurance bon marche.

4. **Sortie liee au juridique ou a la conformite** — Tout ce qui touche aux contrats, politiques de confidentialite, conditions de service. L'IA fait des erreurs juridiques qui sonnent confiantes.

5. **Decisions de recrutement ou concernant des personnes** — Si tu externalises un jour, ne laisse jamais une IA prendre la decision finale sur avec qui travailler.

### Dette d'Automatisation

{@ mirror automation_risk_profile @}

La dette d'automatisation est pire que la dette technique parce qu'elle est invisible jusqu'a ce qu'elle explose.

**A quoi ressemble la dette d'automatisation :**
- Un bot de reseaux sociaux qui publie a la mauvaise heure parce que le fuseau horaire a change
- Un pipeline de newsletter qui inclut un lien casse depuis 3 semaines parce que personne ne verifie
- Un moniteur de prix qui a cesse de fonctionner quand le concurrent a redesigne sa page
- Un script de sauvegarde qui echoue silencieusement parce que le disque est plein

**Comment la prevenir :**

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

Execute ceci chaque matin. Quand une automatisation casse silencieusement (et ca arrivera), tu le sauras en 24 heures au lieu de 3 semaines.

### Construire des Files de Revision

La cle pour rendre le humain-dans-la-boucle efficace est de regrouper ta revision. Ne revois pas un element a la fois quand ils arrivent. Mets-les en file et revois-les par lots.

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

**L'habitude de revision :** Verifie ta file de revision a 8h et a 16h. Deux sessions, 10-15 minutes chacune. Tout le reste tourne de facon autonome entre les revisions.

> **Parlons franchement :** Considere ce qui se passe quand tu sautes la revision humaine : tu automatises completement ta newsletter, le LLM commence a inserer des liens hallucines vers des pages qui n'existent pas, et les abonnes le remarquent avant toi. Tu perds une partie de ta liste et ca prend des mois pour reconstruire la confiance. En revanche, le developpeur qui automatise 80% du meme processus — le LLM curate et redige, il passe 10 minutes a revoir — attrape ces hallucinations avant qu'elles ne soient envoyees. La difference n'est pas l'automatisation. C'est l'etape de revision.

### A Toi

1. Configure le script `automation_healthcheck.py` pour les automatisations que tu as construites dans les Lecons 2 et 3. Planifie-le pour tourner chaque matin.
2. Implemente une file de revision pour ta sortie d'automatisation la plus risquee (tout ce qui est oriente client).
3. Engage-toi a verifier la file de revision deux fois par jour pendant une semaine. Note combien d'elements tu approuves sans changement, combien tu modifies et combien tu rejettes. Ces donnees te disent a quel point ton automatisation est vraiment bonne.

---

## Lecon 6 : Optimisation des Couts et Ton Premier Pipeline

*"Si tu ne peux pas generer $200 de revenus avec $200 de depenses API, repare le produit — pas le budget."*

### L'Economie de l'Automatisation Alimentee par LLM

Chaque appel LLM a un cout. Meme les modeles locaux coutent de l'electricite et de l'usure GPU. La question est de savoir si la sortie de cet appel genere plus de valeur que ce que l'appel coute.

{? if profile.gpu.exists ?}
Faire tourner des modeles locaux sur {= profile.gpu.model | fallback("ton GPU") =} coute environ {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("quelques dollars") =} en electricite par mois pour des charges de travail de pipeline typiques. C'est la base a battre avec les alternatives API.
{? endif ?}

**La regle du budget API de {= regional.currency_symbol | fallback("$") =}200/mois :**

Si tu depenses {= regional.currency_symbol | fallback("$") =}200/mois en appels API pour tes automatisations, ces automatisations devraient generer au moins {= regional.currency_symbol | fallback("$") =}200/mois en valeur — soit des revenus directs, soit du temps economise que tu convertis en revenus ailleurs.

Si ce n'est pas le cas : le probleme n'est pas le budget API. C'est la conception du pipeline ou le produit qu'il soutient.

### Suivi du Cout par Sortie

Ajoute ceci a chaque pipeline que tu construis :

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

### Regroupement pour l'Efficacite API

Si tu utilises des modeles API, le regroupement economise de l'argent reel :

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

### Cache : Ne Paie Pas Deux Fois pour la Meme Reponse

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

**Utilise-le dans tes pipelines :**

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

Pour les pipelines qui traitent les memes types de contenu de facon repetee (classification, extraction), le cache peut eliminer 30-50% de tes appels API. C'est 30-50% de moins sur ta facture mensuelle.

### Construire Ton Premier Pipeline Complet : Etape par Etape

Voici le processus complet de "j'ai un workflow manuel" a "ca tourne pendant que je dors."

**Etape 1 : Cartographie ton processus manuel actuel.**

Ecris chaque etape que tu suis pour un flux de revenus specifique. Exemple pour une newsletter :

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

**Etape 2 : Identifie les trois etapes les plus chronophages.**

De l'exemple : Lire les articles (40 min), ecrire les resumes (30 min), parcourir les titres (20 min).

**Etape 3 : Automatise la plus facile en premier.**

Parcourir les titres est la plus facile a automatiser — c'est de la classification. Un LLM note la pertinence, tu ne lis que les mieux notes.

**Etape 4 : Mesure le temps economise et la qualite.**

Apres avoir automatise le parcours des titres :
- Temps economise : 20 minutes
- Qualite : 90% d'accord avec tes choix manuels
- Net : 20 minutes economisees, perte de qualite negligeable

**Etape 5 : Automatise l'etape suivante.**

Maintenant automatise l'ecriture des resumes. Le LLM redige les resumes, tu les edites.

**Etape 6 : Continue jusqu'aux rendements decroissants.**

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

**Etape 7 : Le pipeline complet, assemble.**

Voici un GitHub Action qui relie tout ensemble pour un pipeline de newsletter hebdomadaire :

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

Ceci tourne chaque dimanche a 5h du matin. Au moment ou tu te reveilles, le brouillon t'attend. Tu passes 10 minutes a le revoir avec ton cafe, tu appuies sur envoyer, et ta newsletter est publiee pour la semaine.

### A Toi : Construis Ton Pipeline

C'est le livrable du module. A la fin de cette lecon, tu devrais avoir un pipeline complet deploye et en fonctionnement.

**Exigences pour ton pipeline :**
1. Il tourne selon un calendrier sans ton intervention
2. Il inclut au moins une etape de traitement LLM
3. Il a une etape de revision humaine pour le controle qualite
4. Il a une verification de sante pour que tu saches s'il casse
5. Il est connecte a un flux de revenus reel (ou un flux que tu construis)

**Checklist :**

- [ ] Choisi un flux de revenus a automatiser
- [ ] Cartographie le processus manuel (toutes les etapes, avec estimations de temps)
- [ ] Identifie les 3 etapes les plus chronophages
- [ ] Automatise au moins la premiere etape (classification/notation/filtrage)
- [ ] Ajoute le traitement LLM pour la deuxieme etape (resume/generation/extraction)
- [ ] Construit une file de revision pour la supervision humaine
- [ ] Configure une verification de sante pour l'automatisation
- [ ] Deploye selon un calendrier (cron, GitHub Actions ou timer systemd)
- [ ] Suivi du cout et des economies de temps pour un cycle complet
- [ ] Documente le pipeline (ce qu'il fait, comment le reparer, quoi surveiller)

Si tu as complete les dix elements de cette checklist, tu as une automatisation de Niveau 3 qui tourne. Tu viens de liberer des heures de ta semaine que tu peux reinvestir dans la construction de plus de flux ou l'amelioration des existants.

---

## Module T : Termine

{@ temporal automation_progress @}

### Ce Que Tu As Construit en Deux Semaines

1. **Une comprehension de la pyramide d'automatisation** — tu sais ou tu en es et vers ou chacun de tes flux de revenus devrait se diriger.
2. **Des automatisations planifiees** qui tournent sur cron ou des planificateurs cloud — la fondation sans glamour qui rend tout le reste possible.
3. **Des pipelines alimentes par LLM** qui gerent les decisions de jugement que tu prenais manuellement — classer, resumer, generer, surveiller.
4. **Des patterns bases sur des agents** que tu peux deployer pour l'interaction client, le traitement du feedback et les produits alimentes par MCP.
5. **Un cadre humain-dans-la-boucle** qui protege ta reputation tout en economisant 80%+ de ton temps.
6. **Un suivi et une optimisation des couts** pour que tes automatisations generent du profit, pas juste de l'activite.
7. **Un pipeline complet et deploye** qui genere de la valeur sans ta participation active.

### L'Effet Compose

Voici ce qui se passe dans les 3 prochains mois si tu maintiens et etends ce que tu as construit dans ce module :

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

C'est comme ca qu'un developpeur opere comme une equipe de cinq. Pas en travaillant plus dur. En construisant des systemes qui travaillent pendant que tu ne le fais pas.

---

### Integration 4DA

{? if dna.identity_summary ?}
Base sur ton profil de developpeur — {= dna.identity_summary | fallback("ton focus de developpement") =} — les outils 4DA ci-dessous correspondent directement aux patterns d'automatisation que tu viens d'apprendre. Les outils de classification de signaux sont particulierement pertinents pour les developpeurs dans ton domaine.
{? endif ?}

4DA est lui-meme une automatisation de Niveau 3. Il ingere du contenu provenant de dizaines de sources, note chaque element avec l'algorithme PASIFA et fait remonter uniquement ce qui est pertinent pour ton travail — le tout sans que tu leves le petit doigt. Tu ne verifies pas manuellement Hacker News, Reddit et 50 flux RSS. 4DA le fait et te montre ce qui compte.

Construis tes pipelines de revenus de la meme facon.

Le rapport d'attention de 4DA (`/attention_report` dans les outils MCP) te montre ou va reellement ton temps versus ou il devrait aller. Execute-le avant de decider quoi automatiser. L'ecart entre "temps depense" et "temps qui devrait etre depense" est ta feuille de route d'automatisation.

Les outils de classification de signaux (`/get_actionable_signals`) peuvent alimenter directement ton pipeline de surveillance du marche — laissant la couche d'intelligence de 4DA faire la notation initiale avant que ton pipeline personnalise fasse l'analyse specifique a ta niche.

Si tu construis des pipelines qui surveillent des sources pour des opportunites, ne reinvente pas ce que 4DA fait deja. Utilise son serveur MCP comme un bloc de construction dans ton stack d'automatisation.

---

### Ce Qui Vient Ensuite : Module S — Empiler les Flux

Le Module T t'a donne les outils pour faire tourner chaque flux de revenus efficacement. Le Module S (Empiler les Flux) repond a la question suivante : **combien de flux devrais-tu gerer, et comment s'assemblent-ils ?**

Voici ce que couvre le Module S :

- **Theorie du portefeuille pour les flux de revenus** — pourquoi 3 flux battent 1 flux, et pourquoi 10 flux ne battent aucun
- **Correlation des flux** — quels flux se renforcent mutuellement et lesquels sont en competition pour ton temps
- **Le plancher de revenus** — construire une base de revenus recurrents qui couvre tes couts avant d'experimenter
- **Reequilibrage** — quand doubler la mise sur un gagnant et quand eliminer un sous-performeur
- **L'architecture a $10K/mois** — des combinaisons specifiques de flux qui atteignent cinq chiffres avec 15-20 heures par semaine

Tu as l'infrastructure (Module S), les douves (Module T), les moteurs (Module R), le playbook de lancement (Module E), le radar de tendances (Module E) et maintenant l'automatisation (Module T). Le Module S les reunit tous en un portefeuille de revenus durable et en croissance.

---

**Le pipeline tourne. Le brouillon est pret. Tu passes 10 minutes a revoir.**

**C'est l'automatisation tactique. C'est comme ca que tu scales.**

*Ton matos. Tes regles. Tes revenus.*
