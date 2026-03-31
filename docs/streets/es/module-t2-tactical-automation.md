# Módulo T: Automatización Táctica

**Curso STREETS de Ingresos para Desarrolladores — Módulo de Pago**
*Semanas 12-13 | 6 Lecciones | Entregable: Un Pipeline Automatizado Generando Valor*

> "LLMs, agentes, MCP y cron jobs como multiplicadores de fuerza."

---

Tienes motores de ingresos funcionando. Tienes clientes. Tienes procesos que funcionan. Y estás gastando el 60-70% de tu tiempo haciendo las mismas cosas una y otra vez: procesando entradas, formateando salidas, revisando monitores, enviando actualizaciones, revisando colas.

Ese tiempo es tu recurso más caro, y lo estás quemando en tareas que un VPS de {= regional.currency_symbol | fallback("$") =}5/mes podría manejar.

{@ insight hardware_benchmark @}

Este módulo trata de eliminarte sistemáticamente del circuito — no completamente (esa es una trampa que cubriremos en la Lección 5), sino del 80% del trabajo que no requiere tu juicio. El resultado: tus flujos de ingresos producen dinero mientras duermes, mientras estás en tu trabajo diario, mientras construyes lo siguiente.

Al final de estas dos semanas, habrás logrado:

- Una comprensión clara de los cuatro niveles de automatización y dónde te encuentras hoy
- Cron jobs funcionales y automatizaciones programadas ejecutándose en tu infraestructura
- Al menos un pipeline potenciado por LLM procesando entradas sin tu intervención
- Una comprensión de los sistemas basados en agentes y cuándo tienen sentido económico
- Un marco de humano-en-el-circuito para que la automatización no destruya tu reputación
- Un pipeline completo y desplegado que genera valor sin tu participación activa

{? if stack.primary ?}
Tu stack principal es {= stack.primary | fallback("tu stack principal") =}, así que los ejemplos de automatización que vienen serán más directamente aplicables cuando se adapten a ese ecosistema. La mayoría de los ejemplos usan Python por portabilidad, pero los patrones se traducen a cualquier lenguaje.
{? endif ?}

Este es el módulo con más código del curso. Al menos la mitad de lo que sigue es código ejecutable. Cópialo, adáptalo, despliégalo.

Vamos a automatizar.

---

## Lección 1: La Pirámide de Automatización

*"La mayoría de los desarrolladores automatizan en el Nivel 1. El dinero está en el Nivel 3."*

### Los Cuatro Niveles

Cada automatización en tu stack de ingresos cae en algún lugar de esta pirámide:

```
┌───────────────────────────────┐
│  Nivel 4: Agentes Autónomos   │  ← Toma decisiones por ti
│  (IA decide Y actúa)         │
├───────────────────────────────┤
│  Nivel 3: Pipelines           │  ← Aquí está el dinero
│  Inteligentes (con LLM)      │
├───────────────────────────────┤
│  Nivel 2: Automatización      │  ← La mayoría se detiene aquí
│  Programada (cron + scripts) │
├───────────────────────────────┤
│  Nivel 1: Manual con          │  ← Donde está la mayoría
│  Plantillas (copiar-pegar)   │
└───────────────────────────────┘
```

Seamos específicos sobre cómo se ve cada nivel en la práctica.

### Nivel 1: Manual con Plantillas

Tú haces el trabajo, pero tienes listas de verificación, plantillas y snippets para acelerar las cosas.

**Ejemplos:**
- Escribes un post de blog usando una plantilla markdown con frontmatter prellenado
- Facturas a clientes duplicando la factura del mes pasado y cambiando los números
- Respondes a correos de soporte usando respuestas guardadas
- Publicas contenido ejecutando manualmente un comando de despliegue

**Costo en tiempo:** 100% de tu tiempo por unidad de producción.
**Tasa de error:** Moderada — eres humano, cometes errores cuando estás cansado.
**Techo de escala:** Tú. Tus horas. Eso es todo.

La mayoría de los desarrolladores viven aquí y ni siquiera se dan cuenta de que hay una pirámide por encima.

### Nivel 2: Automatización Programada

Los scripts se ejecutan en horarios. Escribiste la lógica una vez. Se ejecuta sin ti.

**Ejemplos:**
- Un cron job que verifica tu feed RSS y publica nuevos artículos en redes sociales
- Un GitHub Action que construye y despliega tu sitio cada mañana a las 6 AM
- Un script que se ejecuta cada hora para verificar precios de la competencia y registrar cambios
- Un respaldo diario de base de datos que se ejecuta a las 3 AM

**Costo en tiempo:** Cero continuo (después de la configuración inicial de 1-4 horas).
**Tasa de error:** Baja — determinista, la misma lógica cada vez.
**Techo de escala:** Tantas tareas como tu máquina pueda programar. Cientos.

Aquí es donde la mayoría de los desarrolladores técnicos aterrizan. Es cómodo. Pero tiene un límite duro: solo puede manejar tareas con lógica determinista. Si la tarea requiere juicio, estás atascado.

### Nivel 3: Pipelines Inteligentes

Los scripts se ejecutan en horarios, pero incluyen un LLM que maneja las decisiones de juicio.

**Ejemplos:**
- Los feeds RSS se ingieren, el LLM resume cada artículo, redacta un newsletter, tú revisas durante 10 minutos y le das enviar
- Los correos de feedback de clientes se clasifican por sentimiento y urgencia, las respuestas pre-redactadas se ponen en cola para tu aprobación
- Las nuevas ofertas de trabajo en tu nicho se rastrean, el LLM evalúa relevancia, recibes un resumen diario de 5 oportunidades en lugar de escanear 200 listados
- Los posts de blog de la competencia se monitorean, el LLM extrae cambios clave de producto, recibes un reporte semanal de inteligencia competitiva

**Costo en tiempo:** 10-20% del tiempo manual. Revisas y apruebas en lugar de crear.
**Tasa de error:** Baja para tareas de clasificación, moderada para generación (por eso revisas).
**Techo de escala:** Miles de items por día. Tu cuello de botella es el costo de API, no tu tiempo.

**Aquí es donde está el dinero.** El Nivel 3 permite que una persona opere flujos de ingresos que normalmente requerirían un equipo de 3-5 personas.

### Nivel 4: Agentes Autónomos

Sistemas de IA que observan, deciden y actúan sin tu participación.

**Ejemplos:**
- Un agente que monitorea las métricas de tu SaaS, detecta una caída en registros, prueba A/B un cambio de precio y lo revierte si no funciona
- Un agente de soporte que maneja preguntas de clientes de Tier 1 de forma completamente autónoma, solo escalando a ti para problemas complejos
- Un agente de contenido que identifica temas tendencia, genera borradores, programa publicaciones y monitorea el rendimiento

**Costo en tiempo:** Casi cero para los casos manejados. Revisas métricas, no acciones individuales.
**Tasa de error:** Depende completamente de tus barandillas. Sin ellas: alta. Con buenas barandillas: sorprendentemente baja para dominios estrechos.
**Techo de escala:** Efectivamente ilimitado para las tareas dentro del alcance del agente.

El Nivel 4 es real y alcanzable, pero no es donde empiezas. Y como cubriremos en la Lección 5, los agentes completamente autónomos orientados al cliente son peligrosos para tu reputación si están mal implementados.

> **Hablando en serio:** Si estás en el Nivel 1 ahora mismo, no intentes saltar al Nivel 4. Pasarás semanas construyendo un "agente autónomo" que se rompe en producción y daña la confianza del cliente. Sube la pirámide un nivel a la vez. El Nivel 2 es una tarde de trabajo. El Nivel 3 es un proyecto de fin de semana. El Nivel 4 llega después de que hayas tenido el Nivel 3 funcionando de forma confiable durante un mes.

### Autoevaluación: ¿Dónde Estás?

Para cada uno de tus flujos de ingresos, evalúate honestamente:

| Flujo de Ingresos | Nivel Actual | Horas/Semana | Podría Automatizar A |
|-------------------|-------------|-------------|---------------------|
| [ej., Newsletter] | [1-4] | [X] hrs | [nivel objetivo] |
| [ej., Procesamiento de clientes] | [1-4] | [X] hrs | [nivel objetivo] |
| [ej., Redes sociales] | [1-4] | [X] hrs | [nivel objetivo] |
| [ej., Soporte] | [1-4] | [X] hrs | [nivel objetivo] |

La columna que más importa es "Horas/Semana." El flujo con más horas y menor nivel es tu primer objetivo de automatización. Ese es el que tiene el mayor ROI.

### La Economía de Cada Nivel

Digamos que tienes un flujo de ingresos que toma 10 horas/semana de tu tiempo y genera {= regional.currency_symbol | fallback("$") =}2,000/mes:

| Nivel | Tu Tiempo | Tu Tarifa Efectiva | Costo de Automatización |
|-------|----------|-------------------|------------------------|
| Nivel 1 | 10 hrs/semana | $50/hr | $0 |
| Nivel 2 | 3 hrs/semana | $167/hr | $5/mes (VPS) |
| Nivel 3 | 1 hr/semana | $500/hr | $30-50/mes (API) |
| Nivel 4 | 0.5 hrs/semana | $1,000/hr | $50-100/mes (API + cómputo) |

Pasar del Nivel 1 al Nivel 3 no cambia tus ingresos. Cambia tu tarifa horaria efectiva de $50 a $500. ¿Y esas 9 horas liberadas? Van a construir el siguiente flujo de ingresos o mejorar el actual.

> **Error Común:** Automatizar tu flujo de menor ingreso primero porque es "más fácil." No. Automatiza el flujo que consume más horas relativo a su ingreso. Ahí es donde está el ROI.

### Tu Turno

1. Llena la tabla de autoevaluación de arriba para cada flujo de ingresos (o flujo planificado) que tengas.
2. Identifica tu objetivo de automatización de mayor ROI: el flujo con más horas y el menor nivel de automatización.
3. Escribe las 3 tareas que más tiempo consumen en ese flujo. Automatizarás la primera en la Lección 2.

---

## Lección 2: Del Nivel 1 al 2 — Automatización Programada

*"Cron es de 1975. Todavía funciona. Úsalo."*

### Fundamentos de Cron Jobs

{? if computed.os_family == "windows" ?}
Estás en Windows, así que cron no es nativo de tu sistema. Tienes dos opciones: usar WSL (Windows Subsystem for Linux) para obtener cron real, o usar el Programador de Tareas de Windows (cubierto abajo). WSL es recomendado si te sientes cómodo con él — todos los ejemplos de cron en esta lección funcionan directamente en WSL. Si prefieres Windows nativo, salta a la sección del Programador de Tareas después de esto.
{? endif ?}

Sí, incluso en 2026, cron es el rey para tareas programadas. Es confiable, está en todas partes y no requiere una cuenta en la nube, una suscripción SaaS ni un esquema YAML que tengas que buscar en Google cada vez.

**La sintaxis de cron en 30 segundos:**

```
┌───────── minuto (0-59)
│ ┌───────── hora (0-23)
│ │ ┌───────── día del mes (1-31)
│ │ │ ┌───────── mes (1-12)
│ │ │ │ ┌───────── día de la semana (0-7, 0 y 7 = Domingo)
│ │ │ │ │
* * * * *  comando
```

**Horarios comunes:**

```bash
# Cada hora
0 * * * *  /path/to/script.sh

# Cada día a las 6 AM
0 6 * * *  /path/to/script.sh

# Cada lunes a las 9 AM
0 9 * * 1  /path/to/script.sh

# Cada 15 minutos
*/15 * * * *  /path/to/script.sh

# Primer día de cada mes a medianoche
0 0 1 * *  /path/to/script.sh
```

**Configurando un cron job:**

```bash
# Editar tu crontab
crontab -e

# Listar cron jobs existentes
crontab -l

# CRÍTICO: Siempre establece variables de entorno al inicio
# Cron se ejecuta con un entorno mínimo — PATH podría no incluir tus herramientas
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# Registra la salida para poder depurar fallos
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **Error Común:** Escribir un script que funciona perfectamente cuando lo ejecutas manualmente, y luego falla silenciosamente en cron porque cron no carga tu `.bashrc` o `.zshrc`. Siempre usa rutas absolutas en los scripts de cron. Siempre establece `PATH` al inicio de tu crontab. Siempre redirige la salida a un archivo de log.

### Programadores en la Nube para Cuando Cron No Es Suficiente

Si tu máquina no está encendida 24/7, o necesitas algo más robusto, usa un programador en la nube:

**GitHub Actions (gratis para repos públicos, 2,000 min/mes en privados):**

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

**Vercel Cron (gratis en plan Hobby, 1 por día; plan Pro: ilimitado):**

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

### Automatizaciones Reales para Construir Ahora Mismo

Aquí hay cinco automatizaciones que puedes implementar hoy. Cada una toma 30-60 minutos y elimina horas de trabajo manual semanal.

#### Automatización 1: Auto-Publicar Contenido en Horario

Escribes posts de blog por adelantado. Este script los publica en el horario programado.

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

**Tus posts en markdown se ven así:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Your post content here...
```

Escribe posts cuando llegue la inspiración. Establece la fecha. El script se encarga del resto.

#### Automatización 2: Auto-Publicar en Redes Sociales con Nuevo Contenido

Cuando tu blog publica algo nuevo, esto postea en Twitter/X y Bluesky automáticamente.

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

Costo: $0. Se ejecuta en tu máquina o en un GitHub Action gratuito.

#### Automatización 3: Monitor de Precios de la Competencia

Entérate al instante cuando un competidor cambia sus precios. No más revisar manualmente cada semana.

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

#### Automatización 4: Reporte Semanal de Ingresos

Cada lunes por la mañana, esto genera un reporte de tus datos de ingresos y te lo envía por correo.

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

#### Automatización 5: Auto-Respaldo de Datos de Clientes

Nunca pierdas entregables de clientes. Esto se ejecuta cada noche y mantiene 30 días de respaldos.

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

### Temporizadores Systemd para Más Control

Si necesitas más de lo que cron ofrece — como ordenamiento de dependencias, límites de recursos o reintento automático — usa temporizadores systemd:

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
### Alternativa del Programador de Tareas de Windows

Si no estás usando WSL, el Programador de Tareas de Windows hace el mismo trabajo. Usa `schtasks` desde la línea de comandos o la GUI del Programador de Tareas (`taskschd.msc`). La diferencia clave: cron usa una sola expresión, el Programador de Tareas usa campos separados para disparadores, acciones y condiciones. Cada ejemplo de cron en esta lección se traduce directamente — programa tus scripts de Python de la misma manera, solo a través de una interfaz diferente.
{? endif ?}

### Tu Turno

1. Elige la automatización más simple de esta lección que aplique a tu flujo de ingresos.
2. Impleméntala. No "planea implementarla." Escribe el código, pruébalo, prográmalo.
3. Configura el registro de logs para que puedas verificar que se está ejecutando. Revisa los logs cada mañana durante 3 días.
4. Una vez que sea estable, deja de revisar diariamente. Revisa semanalmente. Eso es automatización.

**Mínimo:** Un cron job ejecutándose de forma confiable al final del día de hoy.

---

## Lección 3: Del Nivel 2 al 3 — Pipelines Potenciados por LLM

*"Agrega inteligencia a tus automatizaciones. Aquí es donde una persona empieza a parecer un equipo."*

### El Patrón

Cada pipeline potenciado por LLM sigue la misma forma:

```
Fuentes de Entrada → Ingerir → Procesar con LLM → Formatear Salida → Entregar (o Poner en Cola para Revisión)
```

La magia está en el paso "Procesar con LLM". En lugar de escribir reglas deterministas para cada caso posible, describes lo que quieres en lenguaje natural, y el LLM maneja las decisiones de juicio.

### Cuándo Usar Local vs API

{? if settings.has_llm ?}
Tienes {= settings.llm_provider | fallback("un proveedor de LLM") =} configurado con {= settings.llm_model | fallback("tu modelo de LLM") =}. Eso significa que puedes empezar a construir pipelines inteligentes inmediatamente. La decisión de abajo te ayuda a elegir cuándo usar tu configuración local versus una API para cada pipeline.
{? else ?}
No tienes un LLM configurado todavía. Los pipelines en esta lección funcionan tanto con modelos locales (Ollama) como con APIs en la nube. Configura al menos uno antes de construir tu primer pipeline — Ollama es gratis y toma 10 minutos instalarlo.
{? endif ?}

Esta decisión tiene un impacto directo en tus márgenes:

| Factor | Local (Ollama) | API (Claude, GPT) |
|--------|---------------|-------------------|
| **Costo por 1M tokens** | ~$0.003 (electricidad) | $0.15 - $15.00 |
| **Velocidad (tokens/seg)** | 20-60 (8B en GPU de gama media) | 50-100+ |
| **Calidad (8B local vs API)** | Buena para clasificación, extracción | Mejor para generación, razonamiento |
| **Privacidad** | Los datos nunca salen de tu máquina | Los datos van al proveedor |
| **Tiempo activo** | Depende de tu máquina | 99.9%+ |
| **Capacidad por lotes** | Limitada por memoria GPU | Limitada por límites de tasa y presupuesto |

{? if profile.gpu.exists ?}
Con {= profile.gpu.model | fallback("tu GPU") =} en tu máquina, la inferencia local es una opción fuerte. La velocidad y el tamaño del modelo que puedes ejecutar depende de tu VRAM — verifica qué cabe antes de comprometerte con un pipeline solo local.
{? if computed.has_nvidia ?}
Las GPUs NVIDIA obtienen el mejor rendimiento con Ollama gracias a la aceleración CUDA. Deberías poder ejecutar modelos de 7-8B parámetros cómodamente, y posiblemente más grandes dependiendo de tu {= profile.gpu.vram | fallback("VRAM disponible") =}.
{? endif ?}
{? else ?}
Sin una GPU dedicada, la inferencia local será más lenta (solo CPU). Aún funciona para trabajos por lotes pequeños y tareas de clasificación, pero para cualquier cosa sensible al tiempo o de alto volumen, un modelo API será más práctico.
{? endif ?}

**Reglas generales:**
- **Alto volumen, barra de calidad más baja** (clasificación, extracción, etiquetado) → Local
- **Bajo volumen, calidad crítica** (contenido orientado al cliente, análisis complejo) → API
- **Datos sensibles** (info de clientes, datos propietarios) → Local, siempre
- **Más de 10,000 items/mes** → Local ahorra dinero real

**Comparación de costos mensuales para un pipeline típico:**

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

Para pipelines de clasificación y extracción, la diferencia de calidad entre un modelo local de 8B bien configurado y un modelo API de frontera es a menudo insignificante. Prueba ambos. Usa el más barato que cumpla tu barra de calidad.

{@ insight cost_projection @}

### Pipeline 1: Generador de Contenido para Newsletter

Esta es la automatización LLM más común para desarrolladores con ingresos basados en contenido. Los feeds RSS entran, un borrador de newsletter sale.

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

**Lo que esto cuesta:**
- Procesar 50 artículos/día con un modelo local de 8B: ~$0/mes
- Tu tiempo: 10 minutos revisando el borrador vs 2 horas curando manualmente
- Tiempo ahorrado por semana: ~10 horas si ejecutas un newsletter semanal

### Pipeline 2: Reportes de Investigación y Análisis de Clientes

Este pipeline raspa datos públicos, los analiza con un LLM y produce un reporte que puedes vender.

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

**Modelo de negocio:** Cobra $200-500 por reporte de investigación personalizado. Tu costo: $0.05 en llamadas API y 15 minutos de revisión. Puedes producir 3-4 reportes por hora una vez que el pipeline sea estable.

### Pipeline 3: Monitor de Señales de Mercado

Este es el pipeline que te dice qué construir a continuación. Monitorea múltiples fuentes, clasifica señales y te alerta cuando una oportunidad cruza tu umbral.

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

**Lo que esto hace en la práctica:** Recibes una notificación de Slack 2-3 veces por semana diciendo algo como "OPORTUNIDAD: Nuevo framework lanzado sin starter kit — podrías construir uno este fin de semana." Esa señal, actuar sobre ella antes que otros, es cómo te mantienes adelante.

> **Hablando en serio:** La calidad de las salidas de estos pipelines depende completamente de tus prompts y tu definición de nicho. Si tu nicho es vago ("Soy un desarrollador web"), el LLM marcará todo. Si es específico ("Construyo apps de escritorio con Tauri para el mercado de desarrolladores enfocados en privacidad"), será quirúrgicamente preciso. Dedica 30 minutos a definir bien tu nicho. Es la entrada individual de mayor apalancamiento para cada pipeline que construyas.

### Tu Turno

{? if stack.contains("python") ?}
Buenas noticias: los ejemplos de pipeline anteriores ya están en tu lenguaje principal. Puedes copiarlos directamente y empezar a adaptarlos. Concéntrate en obtener la definición de nicho y los prompts correctos — de ahí viene el 90% de la calidad de la salida.
{? else ?}
Los ejemplos anteriores usan Python por portabilidad, pero los patrones funcionan en cualquier lenguaje. Si prefieres construir en {= stack.primary | fallback("tu stack principal") =}, las piezas clave a replicar son: cliente HTTP para obtener RSS/API, parseo de JSON para respuestas de LLM, e I/O de archivos para gestión de estado. La interacción con el LLM es solo un POST HTTP a Ollama o una API en la nube.
{? endif ?}

1. Elige uno de los tres pipelines anteriores (newsletter, investigación o monitor de señales).
2. Adáptalo a tu nicho. Cambia los feeds, la descripción de la audiencia, los criterios de clasificación.
3. Ejecútalo manualmente 3 veces para probar la calidad de la salida.
4. Ajusta los prompts hasta que la salida sea útil sin edición pesada.
5. Prográmalo con cron.

**Objetivo:** Un pipeline potenciado por LLM ejecutándose en horario dentro de las 48 horas de leer esta lección.

---

## Lección 4: Del Nivel 3 al 4 — Sistemas Basados en Agentes

*"Un agente es solo un bucle que observa, decide y actúa. Construye uno."*

### Qué Significa Realmente "Agente" en 2026

Elimina el hype. Un agente es un programa que:

1. **Observa** — lee alguna entrada o estado
2. **Decide** — usa un LLM para determinar qué hacer
3. **Actúa** — ejecuta la decisión
4. **Itera** — vuelve al paso 1

Eso es todo. La diferencia entre un pipeline (Nivel 3) y un agente (Nivel 4) es que el agente itera. Actúa sobre su propia salida. Maneja tareas de múltiples pasos donde el siguiente paso depende del resultado del anterior.

Un pipeline procesa items uno a la vez a través de una secuencia fija. Un agente navega una secuencia impredecible basada en lo que encuentra.

### Servidores MCP que Sirven a Clientes

Un servidor MCP es uno de los sistemas prácticos más cercanos a agentes que puedes construir. Expone herramientas que un agente de IA (Claude Code, Cursor, etc.) puede llamar en nombre de tus clientes.

{? if stack.contains("typescript") ?}
El ejemplo de servidor MCP a continuación usa TypeScript — justo en tu zona. Puedes extenderlo con tu tooling existente de TypeScript y desplegarlo junto a tus otros servicios Node.js.
{? endif ?}

Aquí hay un ejemplo real: un servidor MCP que responde preguntas de clientes desde la documentación de tu producto.

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

**Modelo de negocio:** Dale este servidor MCP a tus clientes como parte de tu producto. Obtienen respuestas instantáneas a sus preguntas sin crear tickets de soporte. Tú pasas menos tiempo en soporte. Todos ganan.

Para premium: cobra $9-29/mes por una versión hospedada con búsqueda vectorial, documentación versionada y analíticas sobre qué preguntan los clientes.

### Procesamiento Automatizado de Feedback de Clientes

Este agente lee el feedback de clientes (de correo electrónico, tickets de soporte o un formulario), lo clasifica y crea borradores de respuestas y tickets de funcionalidades.

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

**Cómo funciona esto en la práctica:**
1. Los clientes envían feedback (vía formulario, correo electrónico o sistema de soporte)
2. El feedback llega como archivos JSON en el directorio de bandeja de entrada
3. El agente procesa cada uno: clasifica, resume, redacta una respuesta
4. Tú abres la cola de revisión una o dos veces al día
5. Para items simples (elogios, preguntas básicas con buenos borradores de respuesta), apruebas el borrador
6. Para items complejos (bugs, clientes molestos), escribes una respuesta personal
7. Tiempo neto: 15 minutos por día en lugar de 2 horas

### El Patrón IA Redacta, Humano Aprueba

Este patrón es el núcleo de la automatización práctica de Nivel 4. El agente maneja el trabajo pesado. Tú manejas las decisiones de juicio.

```
              ┌─────────────┐
              │ Agente redacta│
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ Cola de Revisión│
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │Auto-enviar│ │Editar│ │ Escalar │
    │ (rutina)  │ │+enviar│ │(complejo)│
    └───────────┘ └─────┘ └─────────┘
```

**Reglas para lo que el agente maneja completamente vs lo que tú revisas:**

| El agente maneja completamente (sin revisión) | Tú revisas antes de enviar |
|----------------------------------------------|---------------------------|
| Acuses de recibo ("Recibimos tu mensaje") | Respuestas a clientes molestos |
| Actualizaciones de estado ("Tu solicitud está siendo procesada") | Priorización de solicitudes de funcionalidades |
| Respuestas de FAQ (coincidencia exacta) | Cualquier cosa que involucre dinero (reembolsos, precios) |
| Clasificación y eliminación de spam | Reportes de bugs (necesitas verificar) |
| Registro interno y categorización | Cualquier cosa que nunca hayas visto antes |

> **Error Común:** Dejar que el agente responda a clientes de forma autónoma desde el primer día. No lo hagas. Empieza con el agente redactando todo, tú aprobando todo. Después de una semana, déjalo auto-enviar acuses de recibo. Después de un mes, déjalo auto-enviar respuestas de FAQ. Construye confianza incrementalmente — contigo mismo y con tus clientes.

### Tu Turno

1. Elige uno: construir el servidor MCP de documentación O el agente de procesamiento de feedback.
2. Adáptalo a tu producto/servicio. Si aún no tienes clientes, usa el monitor de señales de la Lección 3 como tu "cliente" — procesa su salida a través del patrón de agente de feedback.
3. Ejecútalo manualmente 10 veces con diferentes entradas.
4. Mide: ¿qué porcentaje de salidas son usables sin edición? Ese es tu puntaje de calidad de automatización. Apunta a 70%+ antes de programar.

---

## Lección 5: El Principio del Humano en el Circuito

*"La automatización completa es una trampa. La automatización parcial es un superpoder."*

### Por Qué el 80% de Automatización Supera al 100%

Hay una razón específica y medible por la que nunca deberías automatizar completamente los procesos orientados al cliente: el costo de una mala salida es asimétrico.

Una buena salida automatizada te ahorra 5 minutos.
Una mala salida automatizada te cuesta un cliente, una queja pública, un reembolso o un golpe a la reputación que toma meses recuperar.

Las matemáticas:

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

Eso es una reducción de 12x en el costo de daño. Tu tiempo revisando 200 salidas (quizás 2 horas) te ahorra $2,300/mes en daños.

### Nunca Automatices Completamente Esto

Algunas cosas siempre deberían tener un humano en el circuito, sin importar qué tan buena sea la IA:

1. **Comunicación orientada al cliente** — Un correo mal redactado puede perder un cliente para siempre. Una respuesta genérica, claramente de IA, puede erosionar la confianza. Revísalo.

2. **Transacciones financieras** — Reembolsos, cambios de precios, facturación. Siempre revisa. El costo de un error es dinero real.

3. **Contenido publicado con tu nombre** — Tu reputación se compone a lo largo de los años y puede destruirse en un mal post. Diez minutos de revisión son un seguro barato.

4. **Salida relacionada con lo legal o cumplimiento** — Cualquier cosa que toque contratos, políticas de privacidad, términos de servicio. La IA comete errores legales que suenan seguros.

5. **Decisiones de contratación o sobre personas** — Si alguna vez subcontratas, nunca dejes que una IA tome la decisión final sobre con quién trabajar.

### Deuda de Automatización

{@ mirror automation_risk_profile @}

La deuda de automatización es peor que la deuda técnica porque es invisible hasta que explota.

**Cómo se ve la deuda de automatización:**
- Un bot de redes sociales que publica a la hora equivocada porque cambió la zona horaria
- Un pipeline de newsletter que ha estado incluyendo un enlace roto durante 3 semanas porque nadie revisa
- Un monitor de precios que dejó de funcionar cuando el competidor rediseñó su página
- Un script de respaldo que falla silenciosamente porque el disco se llenó

**Cómo prevenirla:**

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

Ejecuta esto cada mañana. Cuando una automatización se rompa silenciosamente (y lo hará), lo sabrás en 24 horas en lugar de 3 semanas.

### Construyendo Colas de Revisión

La clave para hacer eficiente el humano-en-el-circuito es agrupar tu revisión. No revises un item a la vez a medida que llegan. Ponlos en cola y revisa por lotes.

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

**El hábito de revisión:** Revisa tu cola de revisión a las 8 AM y a las 4 PM. Dos sesiones, 10-15 minutos cada una. Todo lo demás se ejecuta autónomamente entre revisiones.

> **Hablando en serio:** Considera lo que pasa cuando te saltas la revisión humana: automatizas completamente tu newsletter, el LLM empieza a insertar enlaces alucinados a páginas que no existen, y los suscriptores lo notan antes que tú. Pierdes una parte de tu lista y toma meses reconstruir la confianza. En contraste, el desarrollador que automatiza el 80% del mismo proceso — el LLM cura y redacta, ellos pasan 10 minutos revisando — atrapa esas alucinaciones antes de que se envíen. La diferencia no es la automatización. Es el paso de revisión.

### Tu Turno

1. Configura el script `automation_healthcheck.py` para las automatizaciones que construiste en las Lecciones 2 y 3. Prográmalo para que se ejecute cada mañana.
2. Implementa una cola de revisión para la salida de automatización de mayor riesgo (cualquier cosa orientada al cliente).
3. Comprométete a revisar la cola de revisión dos veces al día durante una semana. Registra cuántos items apruebas sin cambios, cuántos editas y cuántos rechazas. Estos datos te dicen qué tan buena es realmente tu automatización.

---

## Lección 6: Optimización de Costos y Tu Primer Pipeline

*"Si no puedes generar $200 en ingresos con $200 de gasto en API, arregla el producto — no el presupuesto."*

### La Economía de la Automatización Potenciada por LLM

Cada llamada a un LLM tiene un costo. Incluso los modelos locales cuestan electricidad y desgaste de GPU. La pregunta es si la salida de esa llamada genera más valor de lo que cuesta la llamada.

{? if profile.gpu.exists ?}
Ejecutar modelos locales en {= profile.gpu.model | fallback("tu GPU") =} cuesta aproximadamente {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("unos pocos dólares") =} en electricidad por mes para cargas de trabajo de pipeline típicas. Esa es la línea base a superar con alternativas de API.
{? endif ?}

**La regla de presupuesto de API de {= regional.currency_symbol | fallback("$") =}200/mes:**

Si estás gastando {= regional.currency_symbol | fallback("$") =}200/mes en llamadas API para tus automatizaciones, esas automatizaciones deberían estar generando al menos {= regional.currency_symbol | fallback("$") =}200/mes en valor — ya sea ingresos directos o tiempo ahorrado que conviertes en ingresos en otro lugar.

Si no lo hacen: el problema no es el presupuesto de API. Es el diseño del pipeline o el producto que lo soporta.

### Seguimiento de Costo Por Salida

Agrega esto a cada pipeline que construyas:

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

### Agrupación para Eficiencia de API

Si estás usando modelos API, la agrupación ahorra dinero real:

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

### Caché: No Pagues Dos Veces por la Misma Respuesta

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

**Úsalo en tus pipelines:**

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

Para pipelines que procesan los mismos tipos de contenido repetidamente (clasificación, extracción), el caché puede eliminar el 30-50% de tus llamadas API. Eso es un 30-50% menos en tu factura mensual.

### Construyendo Tu Primer Pipeline Completo: Paso a Paso

Aquí está el proceso completo desde "tengo un flujo de trabajo manual" hasta "se ejecuta mientras duermo."

**Paso 1: Mapea tu proceso manual actual.**

Escribe cada paso que tomas para un flujo de ingresos específico. Ejemplo para un newsletter:

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

**Paso 2: Identifica los tres pasos que más tiempo consumen.**

Del ejemplo: Leer artículos (40 min), escribir resúmenes (30 min), escanear titulares (20 min).

**Paso 3: Automatiza el más fácil primero.**

Escanear titulares es lo más fácil de automatizar — es clasificación. Un LLM puntúa relevancia, tú solo lees los mejor puntuados.

**Paso 4: Mide tiempo ahorrado y calidad.**

Después de automatizar el escaneo de titulares:
- Tiempo ahorrado: 20 minutos
- Calidad: 90% de acuerdo con tus elecciones manuales
- Neto: 20 minutos ahorrados, pérdida de calidad negligible

**Paso 5: Automatiza el siguiente paso.**

Ahora automatiza la escritura de resúmenes. El LLM redacta resúmenes, tú los editas.

**Paso 6: Sigue hasta que lleguen los rendimientos decrecientes.**

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

**Paso 7: El pipeline completo, conectado.**

Aquí hay un GitHub Action que conecta todo para un pipeline de newsletter semanal:

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

Esto se ejecuta cada domingo a las 5 AM. Para cuando despiertes, el borrador está esperando. Pasas 10 minutos revisándolo con el café, le das enviar, y tu newsletter está publicado para la semana.

### Tu Turno: Construye Tu Pipeline

Este es el entregable del módulo. Al final de esta lección, deberías tener un pipeline completo desplegado y ejecutándose.

**Requisitos para tu pipeline:**
1. Se ejecuta en un horario sin tu participación
2. Incluye al menos un paso de procesamiento con LLM
3. Tiene un paso de revisión humana para control de calidad
4. Tiene una verificación de salud para que sepas si se rompe
5. Está conectado a un flujo de ingresos real (o un flujo que estás construyendo)

**Lista de verificación:**

- [ ] Elegí un flujo de ingresos para automatizar
- [ ] Mapeé el proceso manual (todos los pasos, con estimaciones de tiempo)
- [ ] Identifiqué los 3 pasos que más tiempo consumen
- [ ] Automaticé al menos el primer paso (clasificación/puntuación/filtrado)
- [ ] Agregué procesamiento LLM para el segundo paso (resumen/generación/extracción)
- [ ] Construí una cola de revisión para supervisión humana
- [ ] Configuré una verificación de salud para la automatización
- [ ] Desplegué en un horario (cron, GitHub Actions o temporizador systemd)
- [ ] Rastreé el costo y ahorro de tiempo para un ciclo completo
- [ ] Documenté el pipeline (qué hace, cómo arreglarlo, qué monitorear)

Si has completado los diez items de esta lista, tienes una automatización de Nivel 3 funcionando. Acabas de liberar horas de tu semana que puedes reinvertir en construir más flujos o mejorar los existentes.

---

## Módulo T: Completo

{@ temporal automation_progress @}

### Lo Que Has Construido en Dos Semanas

1. **Una comprensión de la pirámide de automatización** — sabes dónde estás y hacia dónde debería dirigirse cada uno de tus flujos de ingresos.
2. **Automatizaciones programadas** ejecutándose en cron o programadores en la nube — la base poco glamorosa que hace posible todo lo demás.
3. **Pipelines potenciados por LLM** que manejan las decisiones de juicio que solías tomar manualmente — clasificando, resumiendo, generando, monitoreando.
4. **Patrones basados en agentes** que puedes desplegar para interacción con clientes, procesamiento de feedback y productos potenciados por MCP.
5. **Un marco de humano-en-el-circuito** que protege tu reputación mientras todavía ahorra 80%+ de tu tiempo.
6. **Seguimiento y optimización de costos** para que tus automatizaciones generen ganancias, no solo actividad.
7. **Un pipeline completo y desplegado** generando valor sin tu participación activa.

### El Efecto Compuesto

Esto es lo que pasa en los próximos 3 meses si mantienes y extiendes lo que construiste en este módulo:

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

Así es como un desarrollador opera como un equipo de cinco. No trabajando más duro. Construyendo sistemas que trabajan mientras tú no.

---

### Integración con 4DA

{? if dna.identity_summary ?}
Basado en tu perfil de desarrollador — {= dna.identity_summary | fallback("tu enfoque de desarrollo") =} — las herramientas de 4DA a continuación se mapean directamente a los patrones de automatización que acabas de aprender. Las herramientas de clasificación de señales son particularmente relevantes para desarrolladores en tu espacio.
{? endif ?}

4DA es en sí mismo una automatización de Nivel 3. Ingiere contenido de docenas de fuentes, puntúa cada item con el algoritmo PASIFA y muestra solo lo que es relevante para tu trabajo — todo sin que muevas un dedo. No revisas manualmente Hacker News, Reddit y 50 feeds RSS. 4DA lo hace y te muestra lo que importa.

Construye tus pipelines de ingresos de la misma manera.

El reporte de atención de 4DA (`/attention_report` en las herramientas MCP) te muestra dónde va realmente tu tiempo versus dónde debería ir. Ejecútalo antes de decidir qué automatizar. La brecha entre "tiempo gastado" y "tiempo que debería gastarse" es tu hoja de ruta de automatización.

Las herramientas de clasificación de señales (`/get_actionable_signals`) pueden alimentar directamente tu pipeline de monitoreo de mercado — dejando que la capa de inteligencia de 4DA haga la puntuación inicial antes de que tu pipeline personalizado haga el análisis específico del nicho.

Si estás construyendo pipelines que monitorean fuentes en busca de oportunidades, no reinventes lo que 4DA ya hace. Usa su servidor MCP como un bloque de construcción en tu stack de automatización.

---

### Qué Viene Después: Módulo S — Apilando Flujos

El Módulo T te dio las herramientas para hacer que cada flujo de ingresos funcione eficientemente. El Módulo S (Apilando Flujos) responde la siguiente pregunta: **¿cuántos flujos deberías ejecutar, y cómo encajan juntos?**

Esto es lo que cubre el Módulo S:

- **Teoría de portafolio para flujos de ingresos** — por qué 3 flujos superan a 1 flujo, y por qué 10 flujos no superan a ninguno
- **Correlación de flujos** — qué flujos se refuerzan entre sí y cuáles compiten por tu tiempo
- **El piso de ingresos** — construir una base de ingresos recurrentes que cubra tus costos antes de experimentar
- **Rebalanceo** — cuándo duplicar en un ganador y cuándo matar un bajo rendimiento
- **La arquitectura de $10K/mes** — combinaciones específicas de flujos que alcanzan cinco cifras con 15-20 horas por semana

Tienes la infraestructura (Módulo S), los fosos (Módulo T), los motores (Módulo R), el playbook de lanzamiento (Módulo E), el radar de tendencias (Módulo E) y ahora la automatización (Módulo T). El Módulo S los une a todos en un portafolio de ingresos sostenible y creciente.

---

**El pipeline se ejecuta. El borrador está listo. Pasas 10 minutos revisando.**

**Eso es automatización táctica. Así es como escalas.**

*Tu equipo. Tus reglas. Tus ingresos.*
