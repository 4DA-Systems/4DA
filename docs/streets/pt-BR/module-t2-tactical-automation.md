# Modulo T: Automacao Tatica

**Curso STREETS de Renda para Desenvolvedores — Modulo Pago**
*Semanas 12-13 | 6 Licoes | Entregavel: Um Pipeline Automatizado Gerando Valor*

> "LLMs, agentes, MCP e cron jobs como multiplicadores de forca."

---

Voce tem motores de receita funcionando. Voce tem clientes. Voce tem processos que funcionam. E voce esta gastando 60-70% do seu tempo fazendo as mesmas coisas repetidamente: processando entradas, formatando saidas, verificando monitores, enviando atualizacoes, revisando filas.

Esse tempo e seu recurso mais caro, e voce esta queimando-o em tarefas que um VPS de {= regional.currency_symbol | fallback("$") =}5/mes poderia lidar.

{@ insight hardware_benchmark @}

Este modulo e sobre remover sistematicamente voce do loop — nao completamente (isso e uma armadilha que vamos cobrir na Licao 5), mas dos 80% do trabalho que nao exigem seu julgamento. O resultado: seus fluxos de renda produzem receita enquanto voce dorme, enquanto esta no seu emprego, enquanto esta construindo a proxima coisa.

Ao final dessas duas semanas, voce tera:

- Uma compreensao clara dos quatro niveis de automacao e onde voce esta hoje
- Cron jobs e automacoes agendadas funcionando na sua infraestrutura
- Pelo menos um pipeline alimentado por LLM processando entradas sem seu envolvimento
- Uma compreensao de sistemas baseados em agentes e quando eles fazem sentido economicamente
- Um framework de humano-no-loop para que a automacao nao destrua sua reputacao
- Um pipeline completo, implantado, que gera valor sem seu envolvimento ativo

{? if stack.primary ?}
Seu stack principal e {= stack.primary | fallback("seu stack principal") =}, entao os exemplos de automacao adiante serao mais diretamente aplicaveis quando adaptados a esse ecossistema. A maioria dos exemplos usa Python por portabilidade, mas os padroes se traduzem para qualquer linguagem.
{? endif ?}

Este e o modulo com mais codigo de todo o curso. Pelo menos metade do que segue e codigo executavel. Copie, adapte, implante.

Vamos automatizar.

---

## Licao 1: A Piramide da Automacao

*"A maioria dos desenvolvedores automatiza no Nivel 1. O dinheiro esta no Nivel 3."*

### Os Quatro Niveis

Toda automacao no seu stack de renda se encaixa em algum lugar desta piramide:

```
┌───────────────────────────────┐
│  Nivel 4: Agentes Autonomos   │  ← Toma decisoes por voce
│  (IA decide E age)            │
├───────────────────────────────┤
│  Nivel 3: Pipelines           │  ← O dinheiro esta aqui
│  Inteligentes (com LLM)      │
├───────────────────────────────┤
│  Nivel 2: Automacao           │  ← A maioria dos devs para aqui
│  Agendada (cron + scripts)   │
├───────────────────────────────┤
│  Nivel 1: Manual com          │  ← Onde a maioria dos devs esta
│  Templates (copiar-colar)    │
└───────────────────────────────┘
```

Vamos ser especificos sobre como cada nivel funciona na pratica.

### Nivel 1: Manual com Templates

Voce faz o trabalho, mas tem checklists, templates e snippets para acelerar as coisas.

**Exemplos:**
- Voce escreve um post de blog usando um template markdown com frontmatter pre-preenchido
- Voce fatura clientes duplicando a fatura do mes passado e mudando os numeros
- Voce responde emails de suporte usando respostas salvas
- Voce publica conteudo executando manualmente um comando de deploy

**Custo de tempo:** 100% do seu tempo por unidade de saida.
**Taxa de erro:** Moderada — voce e humano, comete erros quando esta cansado.
**Teto de escala:** Voce. Suas horas. So isso.

A maioria dos desenvolvedores vive aqui e nem percebe que existe uma piramide acima deles.

### Nivel 2: Automacao Agendada

Scripts rodam em agendamentos. Voce escreveu a logica uma vez. Ela executa sem voce.

**Exemplos:**
- Um cron job que verifica seu feed RSS e posta novos artigos nas redes sociais
- Uma GitHub Action que builda e deploya seu site todo dia de manha as 6h
- Um script que roda a cada hora para verificar precos de concorrentes e registrar mudancas
- Um backup diario do banco de dados que roda as 3h da manha

**Custo de tempo:** Zero continuo (apos a configuracao inicial de 1-4 horas).
**Taxa de erro:** Baixa — deterministico, mesma logica toda vez.
**Teto de escala:** Quantas tarefas sua maquina conseguir agendar. Centenas.

Aqui e onde a maioria dos desenvolvedores tecnicos se estabelece. E confortavel. Mas tem um limite rigido: so pode lidar com tarefas com logica deterministica. Se a tarefa exigir julgamento, voce fica preso.

### Nivel 3: Pipelines Inteligentes

Scripts rodam em agendamentos, mas incluem um LLM que lida com as decisoes de julgamento.

**Exemplos:**
- Feeds RSS sao ingeridos, LLM resume cada artigo, redige uma newsletter, voce revisa por 10 minutos e envia
- Emails de feedback de clientes sao classificados por sentimento e urgencia, respostas pre-redigidas sao enfileiradas para sua aprovacao
- Novas vagas de emprego no seu nicho sao coletadas, LLM avalia relevancia, voce recebe um digest diario de 5 oportunidades em vez de escanear 200 listagens
- Posts de blog de concorrentes sao monitorados, LLM extrai mudancas-chave de produto, voce recebe um relatorio semanal de inteligencia competitiva

**Custo de tempo:** 10-20% do tempo manual. Voce revisa e aprova em vez de criar.
**Taxa de erro:** Baixa para tarefas de classificacao, moderada para geracao (por isso voce revisa).
**Teto de escala:** Milhares de itens por dia. Seu gargalo e custo de API, nao seu tempo.

**E aqui que o dinheiro esta.** O Nivel 3 permite que uma pessoa opere fluxos de renda que normalmente exigiriam uma equipe de 3-5 pessoas.

### Nivel 4: Agentes Autonomos

Sistemas de IA que observam, decidem e agem sem seu envolvimento.

**Exemplos:**
- Um agente que monitora as metricas do seu SaaS, detecta uma queda em cadastros, testa A/B uma mudanca de preco e reverte se nao funcionar
- Um agente de suporte que lida com perguntas Tier 1 de clientes de forma totalmente autonoma, so escalando para voce em questoes complexas
- Um agente de conteudo que identifica topicos em alta, gera rascunhos, agenda publicacao e monitora desempenho

**Custo de tempo:** Proximo de zero para casos tratados. Voce revisa metricas, nao acoes individuais.
**Taxa de erro:** Depende inteiramente dos seus guardrails. Sem eles: alta. Com bons guardrails: surpreendentemente baixa para dominios estreitos.
**Teto de escala:** Efetivamente ilimitado para as tarefas dentro do escopo do agente.

O Nivel 4 e real e alcancavel, mas nao e onde voce comeca. E como vamos cobrir na Licao 5, agentes totalmente autonomos voltados para o cliente sao perigosos para sua reputacao se mal implementados.

> **Papo Reto:** Se voce esta no Nivel 1 agora, nao tente pular para o Nivel 4. Voce vai gastar semanas construindo um "agente autonomo" que quebra em producao e danifica a confianca do cliente. Suba a piramide um nivel de cada vez. O Nivel 2 e uma tarde de trabalho. O Nivel 3 e um projeto de fim de semana. O Nivel 4 vem depois que voce teve o Nivel 3 rodando de forma confiavel por um mes.

### Autoavaliacao: Onde Voce Esta?

Para cada um dos seus fluxos de renda, avalie-se honestamente:

| Fluxo de Renda | Nivel Atual | Horas/Semana Gastas | Poderia Automatizar Para |
|----------------|------------|--------------------|-----------------------|
| [ex., Newsletter] | [1-4] | [X] hrs | [nivel alvo] |
| [ex., Processamento de clientes] | [1-4] | [X] hrs | [nivel alvo] |
| [ex., Redes sociais] | [1-4] | [X] hrs | [nivel alvo] |
| [ex., Suporte] | [1-4] | [X] hrs | [nivel alvo] |

A coluna mais importante e "Horas/Semana Gastas." O fluxo com mais horas e menor nivel e seu primeiro alvo de automacao. E o que tem o maior ROI.

### A Economia de Cada Nivel

Digamos que voce tenha um fluxo de renda que toma 10 horas/semana do seu tempo e gera {= regional.currency_symbol | fallback("$") =}2.000/mes:

| Nivel | Seu Tempo | Sua Taxa Efetiva | Custo da Automacao |
|-------|----------|-----------------|-------------------|
| Nivel 1 | 10 hrs/semana | $50/hr | $0 |
| Nivel 2 | 3 hrs/semana | $167/hr | $5/mes (VPS) |
| Nivel 3 | 1 hr/semana | $500/hr | $30-50/mes (API) |
| Nivel 4 | 0,5 hrs/semana | $1.000/hr | $50-100/mes (API + compute) |

Passar do Nivel 1 para o Nivel 3 nao muda sua receita. Muda sua taxa horaria efetiva de $50 para $500. E essas 9 horas liberadas? Elas vao para construir o proximo fluxo de renda ou melhorar o atual.

> **Erro Comum:** Automatizar seu fluxo de menor receita primeiro porque e "mais facil." Nao. Automatize o fluxo que consome mais horas em relacao a sua receita. E la que o ROI esta.

### Sua Vez

1. Preencha a tabela de autoavaliacao acima para cada fluxo de renda (ou fluxo planejado) que voce tem.
2. Identifique seu alvo de automacao com maior ROI: o fluxo com mais horas e o menor nivel de automacao.
3. Anote as 3 tarefas mais demoradas nesse fluxo. Voce vai automatizar a primeira na Licao 2.

---

## Licao 2: Nivel 1 para 2 — Automacao Agendada

*"Cron e de 1975. Ainda funciona. Use-o."*

### Fundamentos de Cron Jobs

{? if computed.os_family == "windows" ?}
Voce esta no Windows, entao cron nao e nativo do seu sistema. Voce tem duas opcoes: usar WSL (Windows Subsystem for Linux) para ter cron real, ou usar o Agendador de Tarefas do Windows (coberto abaixo). WSL e recomendado se voce esta confortavel com ele — todos os exemplos de cron nesta licao funcionam diretamente no WSL. Se voce prefere Windows nativo, pule para a secao do Agendador de Tarefas depois daqui.
{? endif ?}

Sim, mesmo em 2026, cron e rei para tarefas agendadas. E confiavel, esta em todo lugar, e nao requer uma conta em nuvem, uma assinatura SaaS, ou um schema YAML que voce tem que pesquisar no Google toda vez.

**A sintaxe do cron em 30 segundos:**

```
┌───────── minuto (0-59)
│ ┌───────── hora (0-23)
│ │ ┌───────── dia do mes (1-31)
│ │ │ ┌───────── mes (1-12)
│ │ │ │ ┌───────── dia da semana (0-7, 0 e 7 = Domingo)
│ │ │ │ │
* * * * *  comando
```

**Agendamentos comuns:**

```bash
# A cada hora
0 * * * *  /path/to/script.sh

# Todo dia as 6h
0 6 * * *  /path/to/script.sh

# Toda segunda-feira as 9h
0 9 * * 1  /path/to/script.sh

# A cada 15 minutos
*/15 * * * *  /path/to/script.sh

# Primeiro dia de cada mes a meia-noite
0 0 1 * *  /path/to/script.sh
```

**Configurando um cron job:**

```bash
# Edite seu crontab
crontab -e

# Liste cron jobs existentes
crontab -l

# CRITICO: Sempre defina variaveis de ambiente no topo
# Cron roda com um ambiente minimo — PATH pode nao incluir suas ferramentas
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/seuusuario

# Registre a saida para poder debugar falhas
0 6 * * * /home/seuusuario/scripts/daily-report.sh >> /home/seuusuario/logs/daily-report.log 2>&1
```

> **Erro Comum:** Escrever um script que funciona perfeitamente quando voce roda manualmente, e depois ele falha silenciosamente no cron porque o cron nao carrega seu `.bashrc` ou `.zshrc`. Sempre use caminhos absolutos em scripts de cron. Sempre defina `PATH` no topo do seu crontab. Sempre redirecione a saida para um arquivo de log.

### Agendadores em Nuvem Para Quando o Cron Nao Basta

Se sua maquina nao fica ligada 24/7, ou voce precisa de algo mais robusto, use um agendador em nuvem:

**GitHub Actions (gratis para repos publicos, 2.000 min/mes em privados):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # Todo dia as 6h UTC
    - cron: '0 6 * * *'
  # Permitir trigger manual para testes
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

**Vercel Cron (gratis no plano Hobby, 1 por dia; plano Pro: ilimitado):**

```typescript
// api/cron/daily-report.ts
// Endpoint cron do Vercel — configure o agendamento em vercel.json

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Verifique se e realmente o Vercel chamando, nao uma requisicao HTTP aleatoria
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // Sua logica de automacao aqui
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

### Automacoes Reais Para Construir Agora

Aqui estao cinco automacoes que voce pode implementar hoje. Cada uma leva 30-60 minutos e elimina horas de trabalho manual semanal.

#### Automacao 1: Auto-Publicar Conteudo em Agendamento

Voce escreve posts de blog com antecedencia. Este script publica-os no horario agendado.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — Publicar posts markdown na data agendada.
Rodar diariamente via cron: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# Seu endpoint de API do CMS (Hashnode, Dev.to, Ghost, etc.)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """Carregar a lista de nomes de arquivo de posts ja publicados."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """Salvar a lista de nomes de arquivo de posts publicados."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """Extrair frontmatter estilo YAML de um arquivo markdown."""
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
    """Verificar se um post deve ser publicado hoje."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """Publicar um post na API do seu CMS."""
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
        print(f"  Publicado: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  FALHOU: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"Verificando {len(posts)} posts...")

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
    print(f"Total publicados: {len(published)}")

if __name__ == "__main__":
    main()
```

**Seus posts markdown ficam assim:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Seu conteudo do post aqui...
```

Escreva posts quando a inspiracao bater. Defina a data. O script cuida do resto.

#### Automacao 2: Auto-Postar em Redes Sociais ao Publicar Novo Conteudo

Quando seu blog publica algo novo, isso posta no Twitter/X e Bluesky automaticamente.

```python
#!/usr/bin/env python3
"""
social_poster.py — Postar em plataformas sociais quando novo conteudo e publicado.
Rodar a cada 30 minutos: */30 * * * * python3 /path/to/social_poster.py
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
    """Parsear feed RSS e retornar lista de itens."""
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
    """Postar no Bluesky via AT Protocol."""
    # Passo 1: Criar sessao
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

    # Passo 2: Criar post
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
    print(f"  Postado no Bluesky: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # Formatar o post social
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky tem um limite de 300 caracteres
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  Falha ao postar: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

Custo: $0. Roda na sua maquina ou em uma GitHub Action gratuita.

#### Automacao 3: Monitor de Precos de Concorrentes

Saiba no instante em que um concorrente muda seus precos. Chega de verificar manualmente toda semana.

```python
#!/usr/bin/env python3
"""
price_monitor.py — Monitorar paginas de precos de concorrentes para mudancas.
Rodar a cada 6 horas: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # ou Discord, email, etc.

COMPETITORS = [
    {
        "name": "ConcorrenteA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # Para monitoramento de pagina inteira; use seletor para elementos especificos
    },
    {
        "name": "ConcorrenteB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """Buscar uma pagina e retornar o hash do conteudo e trecho de texto."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # Pegar os primeiros 500 caracteres de texto visivel para contexto
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
    """Enviar alerta via webhook do Slack (troque por Discord, email, etc.)."""
    if not ALERT_WEBHOOK:
        print(f"ALERTA (nenhum webhook configurado): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  Falha ao buscar {name}: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"MUDANCA DE PRECO DETECTADA: {name}\n"
                f"URL: {url}\n"
                f"Alterado em: {datetime.utcnow().isoformat()}Z\n"
                f"Hash anterior: {previous_hash[:12]}...\n"
                f"Novo hash: {current_hash[:12]}...\n"
                f"Va verificar manualmente."
            )
            send_alert(alert_msg)
            print(f"  MUDANCA: {name}")
        else:
            print(f"  Sem mudanca: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### Automacao 4: Relatorio Semanal de Receita

Toda segunda-feira de manha, isso gera um relatorio dos seus dados de receita e envia por email para voce.

```python
#!/usr/bin/env python3
"""
weekly_report.py — Gerar relatorio semanal de receita da sua planilha/banco de dados.
Rodar as segundas as 7h: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """Criar a tabela de receita se nao existir."""
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
    """Gerar um relatorio semanal em texto puro."""
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
    report.append(f"RELATORIO SEMANAL DE RECEITA")
    report.append(f"Periodo: {week_ago.strftime('%Y-%m-%d')} a {today.strftime('%Y-%m-%d')}")
    report.append(f"Gerado em: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    Receita:   ${data['income']:>10,.2f}")
        report.append(f"    Despesas:  ${data['expense']:>10,.2f}")
        report.append(f"    Liquido:   ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  RECEITA TOTAL:   ${total_income:>10,.2f}")
    report.append(f"  DESPESAS TOTAIS: ${total_expenses:>10,.2f}")
    report.append(f"  LUCRO LIQUIDO:   ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  ROI:             {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """Enviar o relatorio via email."""
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
            f"Relatorio Semanal de Receita — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nRelatorio enviado por email.")
    conn.close()

if __name__ == "__main__":
    main()
```

#### Automacao 5: Auto-Backup de Dados de Clientes

Nunca perca entregas de clientes. Isso roda toda noite e mantem 30 dias de backups.

```bash
#!/bin/bash
# backup_client_data.sh — Backup noturno de dados de projetos de clientes.
# Cron: 0 3 * * * /home/seuusuario/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# Criar backup comprimido
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# Deletar backups mais antigos que o periodo de retencao
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# Log
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup completo: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# Opcional: sincronizar para um segundo local (drive externo, outra maquina)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### Timers do Systemd Para Mais Controle

Se voce precisa de mais do que o cron oferece — como ordenacao de dependencias, limites de recursos ou retry automatico — use timers do systemd:

```ini
# /etc/systemd/system/income-publisher.service
[Unit]
Description=Publicar conteudo agendado
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=seuusuario
ExecStart=/usr/bin/python3 /home/seuusuario/scripts/scheduled_publisher.py
Environment="CMS_API_KEY=sua-chave-aqui"
Environment="CMS_API_URL=https://api.example.com/posts"
# Reiniciar em caso de falha com backoff exponencial
Restart=on-failure
RestartSec=60

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/income-publisher.timer
[Unit]
Description=Executar publicador de conteudo diariamente as 6h

[Timer]
OnCalendar=*-*-* 06:00:00
Persistent=true
# Se a maquina estava desligada as 6h, executar quando voltar online
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# Habilitar e iniciar o timer
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# Verificar status
systemctl list-timers --all | grep income

# Ver logs
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Alternativa do Agendador de Tarefas do Windows

Se voce nao esta usando WSL, o Agendador de Tarefas do Windows faz o mesmo trabalho. Use `schtasks` na linha de comando ou a interface grafica do Agendador de Tarefas (`taskschd.msc`). A diferenca principal: cron usa uma unica expressao, o Agendador de Tarefas usa campos separados para triggers, acoes e condicoes. Cada exemplo de cron nesta licao se traduz diretamente — agende seus scripts Python da mesma forma, apenas atraves de uma interface diferente.
{? endif ?}

### Sua Vez

1. Escolha a automacao mais simples desta licao que se aplica ao seu fluxo de renda.
2. Implemente-a. Nao "planeje implementar." Escreva o codigo, teste, agende.
3. Configure logging para poder verificar que esta rodando. Verifique os logs toda manha por 3 dias.
4. Quando estiver estavel, pare de verificar diariamente. Verifique semanalmente. Isso e automacao.

**Minimo:** Um cron job rodando confiavelmente ate o final de hoje.

---

## Licao 3: Nivel 2 para 3 — Pipelines Alimentados por LLM

*"Adicione inteligencia as suas automacoes. E aqui que uma pessoa comeca a parecer uma equipe."*

### O Padrao

Todo pipeline alimentado por LLM segue a mesma forma:

```
Fontes de Entrada → Ingestao → Processamento LLM → Formatar Saida → Entregar (ou Enfileirar para Revisao)
```

A magica esta no passo "Processamento LLM". Em vez de escrever regras deterministicas para cada caso possivel, voce descreve o que quer em linguagem natural, e o LLM lida com as decisoes de julgamento.

### Quando Usar Local vs API

{? if settings.has_llm ?}
Voce tem {= settings.llm_provider | fallback("um provedor de LLM") =} configurado com {= settings.llm_model | fallback("seu modelo LLM") =}. Isso significa que voce pode comecar a construir pipelines inteligentes imediatamente. A decisao abaixo ajuda voce a escolher quando usar sua configuracao local versus uma API para cada pipeline.
{? else ?}
Voce ainda nao tem um LLM configurado. Os pipelines nesta licao funcionam tanto com modelos locais (Ollama) quanto com APIs em nuvem. Configure pelo menos um antes de construir seu primeiro pipeline — Ollama e gratuito e leva 10 minutos para instalar.
{? endif ?}

Esta decisao tem impacto direto nas suas margens:

| Fator | Local (Ollama) | API (Claude, GPT) |
|-------|---------------|-------------------|
| **Custo por 1M tokens** | ~$0,003 (eletricidade) | $0,15 - $15,00 |
| **Velocidade (tokens/seg)** | 20-60 (8B em GPU media) | 50-100+ |
| **Qualidade (8B local vs API)** | Boa para classificacao, extracao | Melhor para geracao, raciocinio |
| **Privacidade** | Dados nunca saem da sua maquina | Dados vao para o provedor |
| **Uptime** | Depende da sua maquina | 99,9%+ |
| **Capacidade de lote** | Limitada pela memoria da GPU | Limitada por rate limits e orcamento |

{? if profile.gpu.exists ?}
Com {= profile.gpu.model | fallback("sua GPU") =} na sua maquina, inferencia local e uma opcao forte. A velocidade e o tamanho do modelo que voce pode rodar dependem da sua VRAM — verifique o que cabe antes de se comprometer com um pipeline apenas local.
{? if computed.has_nvidia ?}
GPUs NVIDIA tem o melhor desempenho no Ollama gracas a aceleracao CUDA. Voce deve conseguir rodar modelos de 7-8B de parametros confortavelmente, e possivelmente maiores dependendo da sua {= profile.gpu.vram | fallback("VRAM disponivel") =}.
{? endif ?}
{? else ?}
Sem uma GPU dedicada, inferencia local sera mais lenta (apenas CPU). Ainda funciona para pequenos trabalhos em lote e tarefas de classificacao, mas para qualquer coisa sensivel ao tempo ou de alto volume, um modelo de API sera mais pratico.
{? endif ?}

**Regras praticas:**
- **Alto volume, barra de qualidade mais baixa** (classificacao, extracao, tagging) → Local
- **Baixo volume, qualidade critica** (conteudo voltado ao cliente, analise complexa) → API
- **Dados sensiveis** (informacoes de clientes, dados proprietarios) → Local, sempre
- **Mais de 10.000 itens/mes** → Local economiza dinheiro real

**Comparacao de custo mensal para um pipeline tipico:**

```
Processando 5.000 itens/mes, ~500 tokens por item:

Local (Ollama, llama3.1:8b):
  2.500.000 tokens × $0,003/1M = $0,0075/mes
  Basicamente gratis.

API (GPT-4o-mini):
  2.500.000 tokens de entrada × $0,15/1M = $0,375
  2.500.000 tokens de saida × $0,60/1M = $1,50
  Total: ~$1,88/mes
  Barato, mas 250x mais que local.

API (Claude 3.5 Sonnet):
  2.500.000 tokens de entrada × $3,00/1M = $7,50
  2.500.000 tokens de saida × $15,00/1M = $37,50
  Total: ~$45/mes
  A qualidade e excelente, mas 6.000x mais que local.
```

Para pipelines de classificacao e extracao, a diferenca de qualidade entre um modelo local de 8B bem configurado e um modelo de API de fronteira frequentemente e insignificante. Teste ambos. Use o mais barato que atende sua barra de qualidade.

{@ insight cost_projection @}

### Pipeline 1: Gerador de Conteudo para Newsletter

Esta e a automacao LLM mais comum para desenvolvedores com renda baseada em conteudo. Feeds RSS entram, um rascunho de newsletter sai.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — Ingerir feeds RSS, resumir com LLM, gerar rascunho de newsletter.
Rodar diariamente: 0 5 * * * python3 /path/to/newsletter_pipeline.py

Este pipeline:
1. Busca novos artigos de multiplos feeds RSS
2. Envia cada um para um LLM local para resumo
3. Classifica-os por relevancia para seu publico
4. Gera um rascunho formatado de newsletter
5. Salva o rascunho para sua revisao (voce gasta 10 min revisando, nao 2 horas curando)
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
    # Adicione seus feeds de nicho aqui
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
    """Parsear um feed RSS/Atom e retornar artigos."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # Lidar com feeds RSS e Atom
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
        print(f"  Falha ao buscar {url}: {e}")
        return []

def llm_process(prompt: str) -> str:
    """Enviar um prompt para o LLM local e obter a resposta."""
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
        print(f"  Erro do LLM: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """Usar LLM para pontuar relevancia e gerar um resumo."""
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
        # Tentar parsear o JSON da saida do LLM
        # Lidar com casos onde o LLM envolve em blocos de codigo markdown
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
    """Formatar artigos pontuados em um rascunho de newsletter."""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# Sua Newsletter — {today}")
    newsletter.append("")
    newsletter.append("*[SUA INTRO AQUI — Escreva 2-3 frases sobre o tema desta semana]*")
    newsletter.append("")

    section_titles = {
        "tool": "Ferramentas & Lancamentos",
        "technique": "Tecnicas & Padroes",
        "news": "Noticias da Industria",
        "tutorial": "Tutoriais & Guias",
        "opinion": "Perspectivas"
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
    newsletter.append("*[SEU ENCERRAMENTO — No que voce esta trabalhando? O que os leitores devem ficar de olho?]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("Buscando feeds...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} novos artigos")

    if not all_articles:
        print("Nenhum novo artigo. Pulando.")
        return

    print(f"\nPontuando {len(all_articles)} artigos com LLM...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # Filtrar apenas artigos relevantes e ordenar por pontuacao
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # Pegar os top 10
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} artigos passaram o limiar de relevancia (>= 6/10)")

    # Gerar o rascunho da newsletter
    draft = generate_newsletter(top_articles)

    # Salvar rascunho
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nRascunho salvo: {draft_path}")
    print("Revise, adicione sua intro/encerramento e envie.")

if __name__ == "__main__":
    main()
```

**Quanto isso custa:**
- Processar 50 artigos/dia com um modelo local 8B: ~$0/mes
- Seu tempo: 10 minutos revisando o rascunho vs 2 horas curando manualmente
- Tempo economizado por semana: ~10 horas se voce roda uma newsletter semanal

### Pipeline 2: Pesquisa de Clientes e Relatorios de Insights

Este pipeline coleta dados publicos, analisa-os com um LLM e produz um relatorio que voce pode vender.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — Analisar dados publicos de empresas/produtos e gerar relatorios de insights.
Este e um servico que voce pode vender: $200-500 por relatorio personalizado.

Uso: python3 research_pipeline.py "Nome da Empresa" "site-deles.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# Use um modelo maior para qualidade em relatorios pagos
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# Ou use API para qualidade voltada ao cliente:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """Rotear para modelo local ou API baseado na configuracao."""
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
    """Coletar dados publicamente disponiveis sobre uma empresa."""
    data = {"company": company, "domain": domain}

    # Verificar se o dominio esta acessivel e obter informacoes basicas
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
        data["website_status"] = f"Erro: {e}"

    # Verificar presenca no GitHub
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
    """Gerar um relatorio de analise usando LLM."""
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
        print("Uso: python3 research_pipeline.py 'Nome da Empresa' 'dominio.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"Pesquisando: {company} ({domain})")
    print(f"Usando: {'API (Claude)' if USE_API else 'Local (Ollama)'}")

    print("Coletando dados publicos...")
    data = gather_public_data(company, domain)

    print("Gerando analise...")
    report = generate_report(company, domain, data)

    # Montar relatorio final
    final_report = f"""# Relatorio de Pesquisa: {company}

**Gerado em:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**Dominio:** {domain}
**Modelo de analise:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*Este relatorio foi gerado usando apenas dados publicamente disponiveis.
Nenhum dado proprietario ou privado foi acessado.*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nRelatorio salvo: {filepath}")
    print(f"Custo de API: ~${'0.02-0.05' if USE_API else '0.00'}")

if __name__ == "__main__":
    main()
```

**Modelo de negocio:** Cobre $200-500 por relatorio personalizado de pesquisa. Seu custo: $0,05 em chamadas de API e 15 minutos de revisao. Voce pode produzir 3-4 relatorios por hora quando o pipeline estiver estavel.

### Pipeline 3: Monitor de Sinais de Mercado

Este e o pipeline que diz o que voce deve construir a seguir. Ele monitora multiplas fontes, classifica sinais e alerta voce quando uma oportunidade cruza seu limiar.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — Monitorar fontes publicas para oportunidades de mercado.
Rodar a cada 2 horas: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# Sua definicao de nicho — o LLM usa isso para pontuar relevancia
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
    """Buscar as top stories do Hacker News."""
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
        print(f"  Falha ao buscar HN: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """Usar LLM para classificar um sinal para oportunidade de mercado."""
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
        item["reasoning"] = f"Classificacao falhou: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """Enviar um alerta para oportunidades com alta pontuacao."""
    msg = (
        f"OPORTUNIDADE DETECTADA (pontuacao: {item['opportunity_score']}/10)\n"
        f"Tipo: {item['opportunity_type']}\n"
        f"Titulo: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Por que: {item['reasoning']}\n"
        f"Acao: {item['action']}"
    )

    # Registrar em arquivo
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # Enviar para Slack/Discord
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERTA: {msg}")

def main():
    seen = load_seen()

    # Buscar de fontes
    print("Buscando sinais...")
    items = fetch_hn_top(30)
    # Adicione mais fontes aqui: Reddit, feeds RSS, GitHub trending, etc.

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} novos sinais para classificar")

    # Classificar cada sinal
    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("Pronto.")

if __name__ == "__main__":
    main()
```

**O que isso faz na pratica:** Voce recebe uma notificacao no Slack 2-3 vezes por semana dizendo algo como "OPORTUNIDADE: Novo framework lancado sem starter kit — voce poderia construir um neste fim de semana." Esse sinal, agindo nele antes dos outros, e como voce se mantem a frente.

> **Papo Reto:** A qualidade das saidas desses pipelines depende inteiramente dos seus prompts e da sua definicao de nicho. Se seu nicho e vago ("Sou um desenvolvedor web"), o LLM vai sinalizar tudo. Se e especifico ("Eu construo apps desktop Tauri para o mercado de desenvolvedores focados em privacidade"), ele sera cirurgicamente preciso. Gaste 30 minutos acertando sua definicao de nicho. E a unica entrada de maior alavancagem para cada pipeline que voce construir.

### Sua Vez

{? if stack.contains("python") ?}
Boa noticia: os exemplos de pipeline acima ja estao na sua linguagem principal. Voce pode copia-los diretamente e comecar a adaptar. Foque em acertar a definicao de nicho e os prompts — e de la que 90% da qualidade da saida vem.
{? else ?}
Os exemplos acima usam Python por portabilidade, mas os padroes funcionam em qualquer linguagem. Se voce prefere construir em {= stack.primary | fallback("seu stack principal") =}, as pecas-chave para replicar sao: cliente HTTP para busca de RSS/API, parsing de JSON para respostas de LLM e I/O de arquivo para gerenciamento de estado. A interacao com o LLM e apenas um HTTP POST para Ollama ou uma API em nuvem.
{? endif ?}

1. Escolha um dos tres pipelines acima (newsletter, pesquisa ou monitor de sinais).
2. Adapte-o ao seu nicho. Mude os feeds, a descricao do publico, os criterios de classificacao.
3. Rode-o manualmente 3 vezes para testar a qualidade da saida.
4. Ajuste os prompts ate que a saida seja util sem edicao pesada.
5. Agende com cron.

**Meta:** Um pipeline alimentado por LLM rodando em agendamento dentro de 48 horas apos ler esta licao.

---

## Licao 4: Nivel 3 para 4 — Sistemas Baseados em Agentes

*"Um agente e apenas um loop que observa, decide e age. Construa um."*

### O Que "Agente" Realmente Significa em 2026

Tire todo o hype. Um agente e um programa que:

1. **Observa** — le alguma entrada ou estado
2. **Decide** — usa um LLM para determinar o que fazer
3. **Age** — executa a decisao
4. **Repete** — volta ao passo 1

So isso. A diferenca entre um pipeline (Nivel 3) e um agente (Nivel 4) e que o agente repete. Ele age sobre sua propria saida. Ele lida com tarefas de multiplas etapas onde o proximo passo depende do resultado do anterior.

Um pipeline processa itens um de cada vez atraves de uma sequencia fixa. Um agente navega uma sequencia imprevisivel baseada no que encontra.

### Servidores MCP Que Servem Clientes

Um servidor MCP e um dos sistemas mais praticos que voce pode construir, adjacente a agentes. Ele expoe ferramentas que um agente de IA (Claude Code, Cursor, etc.) pode chamar em nome dos seus clientes.

{? if stack.contains("typescript") ?}
O exemplo de servidor MCP abaixo usa TypeScript — bem na sua area. Voce pode estende-lo com suas ferramentas TypeScript existentes e deploya-lo junto com seus outros servicos Node.js.
{? endif ?}

Aqui esta um exemplo real: um servidor MCP que responde perguntas de clientes a partir da documentacao do seu produto.

```typescript
// mcp-docs-server/src/index.ts
// Um servidor MCP que responde perguntas da sua documentacao.
// Seus clientes apontam o Claude Code deles para este servidor e obtem respostas instantaneas.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// Carregar seus docs na memoria na inicializacao
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

    // Dividir por cabecalhos para melhor busca
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
  // Busca simples por palavras-chave — substitua por busca vetorial para producao
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // Bonus para correspondencias no titulo
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

// Inicializar
const docs = loadDocs();
console.error(`Carregados ${docs.length} chunks de docs de ${DOCS_DIR}`);

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

// Iniciar o servidor
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

**Modelo de negocio:** De este servidor MCP aos seus clientes como parte do seu produto. Eles obtem respostas instantaneas para suas perguntas sem abrir tickets de suporte. Voce gasta menos tempo com suporte. Todo mundo ganha.

Para premium: cobre $9-29/mes por uma versao hospedada com busca vetorial, docs versionados e analytics sobre o que os clientes estao perguntando.

### Processamento Automatizado de Feedback de Clientes

Este agente le feedback de clientes (de email, tickets de suporte ou um formulario), classifica-o e cria rascunhos de respostas e tickets de funcionalidades.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — Processar feedback de clientes em itens classificados e acionaveis.
O padrao "IA redige, humano aprova".

Rodar a cada hora: 0 * * * * python3 /path/to/feedback_agent.py
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
    """Classificar feedback e gerar rascunho de resposta."""

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
        feedback["draft_response"] = "[Classificacao falhou — necessita revisao manual]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"Nenhum diretorio de inbox: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("Nenhum feedback novo.")
        return

    print(f"Processando {len(inbox_files)} itens de feedback...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  Pulando {filepath.name}: {e}")
            continue

        print(f"  Processando: {feedback.get('subject', 'Sem assunto')[:50]}...")
        processed = process_feedback(feedback)

        # Salvar versao processada
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # Adicionar a fila de revisao
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Desconhecido"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # Mover original para fora do inbox
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # Escrever fila de revisao
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # Resumo
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nProcessados: {len(review_queue)}")
    print(f"Criticos: {critical}")
    print(f"Precisam da sua atencao: {needs_human}")
    print(f"Fila de revisao: {review_path}")

if __name__ == "__main__":
    main()
```

**Como isso funciona na pratica:**
1. Clientes enviam feedback (via formulario, email ou sistema de suporte)
2. Feedback chega como arquivos JSON no diretorio do inbox
3. Agente processa cada um: classifica, resume, redige uma resposta
4. Voce abre a fila de revisao uma ou duas vezes por dia
5. Para itens simples (elogios, perguntas basicas com bons rascunhos de resposta), voce aprova o rascunho
6. Para itens complexos (bugs, clientes irritados), voce escreve uma resposta pessoal
7. Tempo liquido: 15 minutos por dia em vez de 2 horas

### O Padrao IA Redige, Humano Aprova

Este padrao e o nucleo da automacao pratica de Nivel 4. O agente lida com o trabalho pesado. Voce lida com as decisoes de julgamento.

```
              ┌─────────────┐
              │ Agente redige│
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ Fila Revisao │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │Auto-enviar│ │Editar│ │ Escalar │
    │ (rotina)  │ │+envio│ │(complexo│
    └───────────┘ └─────┘ └─────────┘
```

**Regras para o que o agente lida completamente vs o que voce revisa:**

| Agente lida completamente (sem revisao) | Voce revisa antes de enviar |
|-------------------------------|--------------------------|
| Recibos de confirmacao ("Recebemos sua mensagem") | Respostas a clientes irritados |
| Atualizacoes de status ("Sua solicitacao esta sendo processada") | Priorizacao de pedidos de funcionalidades |
| Respostas de FAQ (correspondencia exata) | Qualquer coisa envolvendo dinheiro (reembolsos, precos) |
| Classificacao e exclusao de spam | Relatos de bugs (voce precisa verificar) |
| Logging e categorizacao interna | Qualquer coisa que voce nunca viu antes |

> **Erro Comum:** Deixar o agente responder a clientes de forma autonoma desde o primeiro dia. Nao faca isso. Comece com o agente redigindo tudo, voce aprovando tudo. Apos uma semana, deixe-o auto-enviar confirmacoes. Apos um mes, deixe-o auto-enviar respostas de FAQ. Construa confianca incrementalmente — consigo mesmo e com seus clientes.

### Sua Vez

1. Escolha um: construa o servidor MCP de docs OU o agente de processamento de feedback.
2. Adapte-o ao seu produto/servico. Se voce ainda nao tem clientes, use o monitor de sinais da Licao 3 como seu "cliente" — processe sua saida atraves do padrao de agente de feedback.
3. Rode-o manualmente 10 vezes com diferentes entradas.
4. Meca: qual porcentagem das saidas e utilizavel sem edicao? Essa e sua pontuacao de qualidade de automacao. Mire em 70%+ antes de agendar.

---

## Licao 5: O Principio do Humano-no-Loop

*"Automacao total e uma armadilha. Automacao parcial e um superpoder."*

### Por Que 80% de Automacao Supera 100%

Ha uma razao especifica e mensuravel pela qual voce nunca deve automatizar completamente processos voltados ao cliente: o custo de uma saida ruim e assimetrico.

Uma boa saida automatizada economiza 5 minutos.
Uma saida automatizada ruim custa um cliente, uma reclamacao publica, um reembolso ou um dano a reputacao que leva meses para recuperar.

A matematica:

```
100% de automacao:
  1.000 saidas/mes × 95% de qualidade = 950 boas + 50 ruins
  50 saidas ruins × $50 custo medio (reembolso + suporte + reputacao) = $2.500/mes em danos

80% de automacao + 20% de revisao humana:
  800 saidas auto-tratadas, 200 revisadas por humano
  800 × 95% de qualidade = 760 boas + 40 ruins auto
  200 × 99% de qualidade = 198 boas + 2 ruins humanas
  42 total ruins × $50 = $2.100/mes em danos
  MAS: voce pega 38 das ruins antes que cheguem aos clientes

  Saidas ruins reais chegando aos clientes: ~4
  Dano real: ~$200/mes
```

Isso e uma reducao de 12x no custo de danos. Seu tempo revisando 200 saidas (talvez 2 horas) economiza $2.300/mes em danos.

### Nunca Automatize Totalmente Estas

Algumas coisas devem sempre ter um humano no loop, independentemente de quao boa a IA fique:

1. **Comunicacao voltada ao cliente** — Um email mal redigido pode perder um cliente para sempre. Uma resposta generica, claramente de IA, pode erodir confianca. Revise.

2. **Transacoes financeiras** — Reembolsos, mudancas de preco, faturamento. Sempre revise. O custo de um erro e dinheiro real.

3. **Conteudo publicado com seu nome** — Sua reputacao se compoe ao longo de anos e pode ser destruida em um post ruim. Dez minutos de revisao sao um seguro barato.

4. **Saida relacionada a questoes legais ou de conformidade** — Qualquer coisa tocando contratos, politicas de privacidade, termos de servico. A IA comete erros legais que soam confiantes.

5. **Decisoes de contratacao ou sobre pessoas** — Se voce algum dia terceirizar, nunca deixe uma IA tomar a decisao final sobre com quem trabalhar.

### Divida de Automacao

{@ mirror automation_risk_profile @}

Divida de automacao e pior que divida tecnica porque e invisivel ate explodir.

**Como divida de automacao parece:**
- Um bot de redes sociais que posta no horario errado porque o fuso horario mudou
- Um pipeline de newsletter que inclui um link quebrado ha 3 semanas porque ninguem verifica
- Um monitor de precos que parou de funcionar quando o concorrente redesenhou sua pagina
- Um script de backup que falha silenciosamente porque o disco encheu

**Como prevenir:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — Monitorar todas as suas automacoes para falhas silenciosas.
Rodar toda manha: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# Definir saidas esperadas de cada automacao
AUTOMATIONS = [
    {
        "name": "Pipeline da Newsletter",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # Deve produzir pelo menos diariamente
    },
    {
        "name": "Postador Social",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # Verificar o arquivo diretamente
        "max_age_hours": 2,  # Deve atualizar a cada 30 min
    },
    {
        "name": "Monitor de Concorrentes",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # Deve rodar a cada 6 horas
    },
    {
        "name": "Backup de Clientes",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # Deve rodar toda noite
    },
    {
        "name": "Servidor Ollama",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """Verificar se a automacao produziu saida recente."""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # Verificar arquivos recentes correspondendo ao padrao
        p = Path(path)
        if not p.exists():
            return False, f"Diretorio nao encontrado: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"Nenhum arquivo correspondendo a {automation['pattern']} em {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # Verificar o arquivo diretamente
        if not os.path.exists(path):
            return False, f"Arquivo nao encontrado: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Ultima saida ha {age.total_seconds()/3600:.1f}h (max: {automation['max_age_hours']}h)"

    return True, f"OK (ultima saida ha {age.total_seconds()/3600:.1f}h)"

def check_http(automation: dict) -> tuple[bool, str]:
    """Verificar se um servico esta respondendo."""
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"OK (HTTP {resp.status_code})"
        return False, f"Status inesperado: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"Conexao falhou: {e}"

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
            ok, msg = False, f"Tipo de verificacao desconhecido: {check_type}"

        status = "OK" if ok else "FALHA"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"VERIFICACAO DE SAUDE DA AUTOMACAO — {len(failures)} FALHA(S)\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nVerifique os logs e corrija antes que se acumulem."
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

Rode isso toda manha. Quando uma automacao quebra silenciosamente (e vai acontecer), voce sabera em 24 horas em vez de 3 semanas.

### Construindo Filas de Revisao

A chave para tornar o humano-no-loop eficiente e agrupar sua revisao em lotes. Nao revise um item de cada vez conforme chegam. Enfileire-os e revise em lotes.

```python
#!/usr/bin/env python3
"""
review_queue.py — Uma fila de revisao simples para saidas geradas por IA.
Revise uma ou duas vezes por dia em vez de verificar constantemente.
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """Adicionar um item a fila de revisao."""
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
    """Mostrar todos os itens pendentes para revisao."""
    if not os.path.isdir(QUEUE_DIR):
        print("Fila esta vazia.")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("Fila esta vazia.")
        return

    print(f"\n{'='*60}")
    print(f"FILA DE REVISAO — {len(pending)} itens pendentes")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    Artigos: {content.get('article_count', '?')}")
            print(f"    Rascunho: {content.get('draft_path', 'desconhecido')}")
        elif item["type"] == "customer_response":
            print(f"    Para: {content.get('customer', 'desconhecido')}")
            print(f"    Rascunho: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    Texto: {content.get('text', '')[:100]}...")

        print(f"    Acoes: [a]provar  [e]ditar  [r]ejeitar  [p]ular")
        print()

    # Em uma implementacao real, voce adicionaria entrada interativa aqui
    # Para processamento em lote, leia decisoes de um arquivo ou CLI simples

if __name__ == "__main__":
    review_queue()
```

**O habito de revisao:** Verifique sua fila de revisao as 8h e as 16h. Duas sessoes, 10-15 minutos cada. Todo o resto roda de forma autonoma entre as revisoes.

> **Papo Reto:** Considere o que acontece quando voce pula a revisao humana: voce automatiza completamente sua newsletter, o LLM comeca a inserir links alucinados para paginas que nao existem, e os assinantes percebem antes de voce. Voce perde uma parte da sua lista e leva meses para reconstruir a confianca. Em contraste, o desenvolvedor que automatiza 80% do mesmo processo — LLM cura e redige, ele gasta 10 minutos revisando — pega essas alucinacoes antes que sejam enviadas. A diferenca nao e a automacao. E o passo de revisao.

### Sua Vez

1. Configure o script `automation_healthcheck.py` para qualquer automacao que voce construiu nas Licoes 2 e 3. Agende-o para rodar toda manha.
2. Implemente uma fila de revisao para sua saida de automacao de maior risco (qualquer coisa voltada ao cliente).
3. Comprometa-se a verificar a fila de revisao duas vezes por dia durante uma semana. Registre quantos itens voce aprova sem alteracao, quantos edita e quantos rejeita. Esses dados dizem quao boa sua automacao realmente e.

---

## Licao 6: Otimizacao de Custos e Seu Primeiro Pipeline

*"Se voce nao consegue gerar $200 em receita com $200 em gasto de API, corrija o produto — nao o orcamento."*

### A Economia da Automacao Alimentada por LLM

Toda chamada de LLM tem um custo. Ate modelos locais custam eletricidade e desgaste de GPU. A questao e se a saida dessa chamada gera mais valor do que a chamada custa.

{? if profile.gpu.exists ?}
Rodar modelos locais na {= profile.gpu.model | fallback("sua GPU") =} custa aproximadamente {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("alguns dolares") =} em eletricidade por mes para cargas de trabalho tipicas de pipeline. Esse e o baseline a ser superado com alternativas de API.
{? endif ?}

**A regra do orcamento de API de {= regional.currency_symbol | fallback("$") =}200/mes:**

Se voce esta gastando {= regional.currency_symbol | fallback("$") =}200/mes em chamadas de API para suas automacoes, essas automacoes devem estar gerando pelo menos {= regional.currency_symbol | fallback("$") =}200/mes em valor — seja receita direta ou tempo economizado que voce converte em receita em outro lugar.

Se nao estao: o problema nao e o orcamento de API. E o design do pipeline ou o produto que ele suporta.

### Rastreamento de Custo-Por-Saida

Adicione isso a cada pipeline que voce construir:

```python
"""
cost_tracker.py — Rastrear o custo de cada chamada de LLM e o valor que ela gera.
Importe isso nos seus pipelines para obter dados reais de custo.
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# Precos por 1M tokens (atualize conforme os precos mudam)
PRICING = {
    # Modelos locais — estimativa de custo de eletricidade
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # Modelos de API
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
    """Registrar o custo de uma chamada de LLM."""
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
    """Gerar um resumo mensal de custo/receita."""
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

    # Imprimir relatorio
    print(f"\nRELATORIO DE CUSTO LLM — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    Chamadas: {data['call_count']}")
        print(f"    Tokens:   {data['total_tokens']:,}")
        print(f"    Custo:    ${data['total_cost']:.4f}")
        print(f"    Receita:  ${data['total_revenue']:.2f}")
        print(f"    ROI:      {roi:.1f}x")

        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  CUSTO TOTAL:    ${grand_cost:.4f}")
    print(f"  RECEITA TOTAL:  ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  ROI GERAL:      {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### Agrupamento em Lotes para Eficiencia de API

Se voce esta usando modelos de API, agrupar em lotes economiza dinheiro real:

```python
"""
batch_api.py — Agrupar chamadas de API em lotes para eficiencia.
Em vez de fazer 100 chamadas de API separadas, agrupe-as.
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
    Classificar multiplos itens eficientemente agrupando-os em chamadas unicas de API.

    Em vez de 100 chamadas de API (100 itens × 1 chamada cada):
      - 100 chamadas × ~500 tokens de entrada = 50.000 tokens de entrada
      - 100 chamadas × ~200 tokens de saida = 20.000 tokens de saida
      - Custo com Haiku: ~$0,12

    Com agrupamento em lotes (10 itens por chamada, 10 chamadas de API):
      - 10 chamadas × ~2.500 tokens de entrada = 25.000 tokens de entrada
      - 10 chamadas × ~1.000 tokens de saida = 10.000 tokens de saida
      - Custo com Haiku: ~$0,06

    50% de economia so com agrupamento em lotes.
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # Formatar lote em um unico prompt
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
            # Parsear o array JSON da resposta
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  Lote {i//batch_size + 1} falhou: {e}")
            # Fallback para processamento individual
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # Cortesia de rate limiting
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### Cache: Nao Pague Duas Vezes Pela Mesma Resposta

```python
"""
llm_cache.py — Cache de respostas de LLM para evitar pagar por processamento duplicado.
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
    """Gerar uma chave de cache deterministica a partir de modelo + prompt."""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """Obter uma resposta em cache se disponivel e recente."""
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

    # Atualizar contagem de hits
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """Armazenar uma resposta em cache."""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """Mostrar estatisticas do cache."""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Entradas no cache: {total}")
    print(f"Total de cache hits: {total_hits}")
    print(f"Economia estimada: ~${total_hits * 0.002:.2f} (media aproximada por chamada)")
```

**Use nos seus pipelines:**

```python
# Em qualquer pipeline que chama um LLM:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # Gratis!

    response = call_llm(model, prompt)  # Sua funcao existente de chamada de LLM
    set_cached(model, prompt, response)
    return response
```

Para pipelines que processam os mesmos tipos de conteudo repetidamente (classificacao, extracao), o cache pode eliminar 30-50% das suas chamadas de API. Isso e 30-50% de desconto na sua conta mensal.

### Construindo Seu Primeiro Pipeline Completo: Passo a Passo

Aqui esta o processo completo de "eu tenho um workflow manual" ate "roda enquanto eu durmo."

**Passo 1: Mapeie seu processo manual atual.**

Anote cada passo que voce toma para um fluxo de renda especifico. Exemplo para uma newsletter:

```
1. Abrir 15 feeds RSS em abas do navegador (10 min)
2. Escanear manchetes, abrir as interessantes (20 min)
3. Ler 8-10 artigos em detalhe (40 min)
4. Escrever resumos para os top 5 (30 min)
5. Escrever paragrafo de introducao (10 min)
6. Formatar na ferramenta de email (15 min)
7. Enviar para a lista (5 min)

Total: ~2 horas e 10 minutos
```

**Passo 2: Identifique os tres passos mais demorados.**

Do exemplo: Ler artigos (40 min), escrever resumos (30 min), escanear manchetes (20 min).

**Passo 3: Automatize o mais facil primeiro.**

Escanear manchetes e o mais facil de automatizar — e classificacao. Um LLM pontua relevancia, voce so le os que ficaram no topo.

**Passo 4: Meca tempo economizado e qualidade.**

Apos automatizar a varredura de manchetes:
- Tempo economizado: 20 minutos
- Qualidade: 90% de concordancia com suas escolhas manuais
- Liquido: 20 minutos economizados, perda de qualidade negligenciavel

**Passo 5: Automatize o proximo passo.**

Agora automatize a escrita de resumos. O LLM redige resumos, voce edita.

**Passo 6: Continue ate retornos decrescentes.**

```
Antes da automacao: 2h10min por newsletter
Apos Nivel 2 (busca agendada): 1h45min
Apos Nivel 3 (pontuacao LLM + resumos): 25min
Apos Nivel 3+ (LLM redige intro): 10min apenas revisao

Tempo economizado por semana: ~2 horas
Tempo economizado por mes: ~8 horas
A $100/hr taxa efetiva: $800/mes em tempo liberado
Custo de API: $0 (LLM local) a $5/mes (API)
```

**Passo 7: O pipeline completo, conectado.**

Aqui esta uma GitHub Action que amarra tudo para um pipeline semanal de newsletter:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # Todo domingo as 5h UTC
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
            -d '{"text":"Rascunho da newsletter pronto para revisao. Verifique os artefatos do GitHub Actions."}'
```

Isso roda todo domingo as 5h. Quando voce acordar, o rascunho esta esperando. Voce gasta 10 minutos revisando-o tomando cafe, envia, e sua newsletter esta publicada para a semana.

### Sua Vez: Construa Seu Pipeline

Este e o entregavel do modulo. Ao final desta licao, voce deve ter um pipeline completo implantado e rodando.

**Requisitos para seu pipeline:**
1. Ele roda em agendamento sem seu envolvimento
2. Inclui pelo menos um passo de processamento LLM
3. Tem um passo de revisao humana para controle de qualidade
4. Tem uma verificacao de saude para que voce saiba se ele quebrar
5. Esta conectado a um fluxo de renda real (ou um fluxo que voce esta construindo)

**Checklist:**

- [ ] Escolheu um fluxo de renda para automatizar
- [ ] Mapeou o processo manual (todos os passos, com estimativas de tempo)
- [ ] Identificou os 3 passos mais demorados
- [ ] Automatizou pelo menos o primeiro passo (classificacao/pontuacao/filtragem)
- [ ] Adicionou processamento LLM para o segundo passo (resumo/geracao/extracao)
- [ ] Construiu uma fila de revisao para supervisao humana
- [ ] Configurou uma verificacao de saude para a automacao
- [ ] Implantou em agendamento (cron, GitHub Actions ou timer systemd)
- [ ] Rastreou o custo e a economia de tempo para um ciclo completo
- [ ] Documentou o pipeline (o que faz, como corrigir, o que monitorar)

Se voce fez todos os dez itens desta checklist, voce tem uma automacao de Nivel 3 rodando. Voce acabou de liberar horas da sua semana que pode reinvestir em construir mais fluxos ou melhorar os existentes.

---

## Modulo T: Completo

{@ temporal automation_progress @}

### O Que Voce Construiu em Duas Semanas

1. **Uma compreensao da piramide de automacao** — voce sabe onde esta e para onde cada um dos seus fluxos de renda deve se dirigir.
2. **Automacoes agendadas** rodando em cron ou agendadores em nuvem — a fundacao sem glamour que torna tudo mais possivel.
3. **Pipelines alimentados por LLM** que lidam com as decisoes de julgamento que voce costumava fazer manualmente — classificando, resumindo, gerando, monitorando.
4. **Padroes baseados em agentes** que voce pode implantar para interacao com clientes, processamento de feedback e produtos alimentados por MCP.
5. **Um framework de humano-no-loop** que protege sua reputacao enquanto ainda economiza 80%+ do seu tempo.
6. **Rastreamento e otimizacao de custos** para que suas automacoes gerem lucro, nao apenas atividade.
7. **Um pipeline completo, implantado** gerando valor sem seu envolvimento ativo.

### O Efeito Composto

Aqui esta o que acontece nos proximos 3 meses se voce mantiver e expandir o que construiu neste modulo:

```
Mes 1: Um pipeline, economizando 5-8 horas/semana
Mes 2: Dois pipelines, economizando 10-15 horas/semana
Mes 3: Tres pipelines, economizando 15-20 horas/semana

A $100/hr taxa efetiva, isso e $1.500-2.000/mes
em tempo liberado — tempo que voce investe em novos fluxos.

O tempo liberado do Mes 1 constroi o pipeline do Mes 2.
O tempo liberado do Mes 2 constroi o pipeline do Mes 3.
Automacao se compoe.
```

E assim que um desenvolvedor opera como uma equipe de cinco. Nao trabalhando mais. Construindo sistemas que trabalham enquanto voce nao trabalha.

---

### Integracao 4DA

{? if dna.identity_summary ?}
Baseado no seu perfil de desenvolvedor — {= dna.identity_summary | fallback("seu foco de desenvolvimento") =} — as ferramentas 4DA abaixo mapeiam diretamente para os padroes de automacao que voce acabou de aprender. As ferramentas de classificacao de sinais sao particularmente relevantes para desenvolvedores no seu espaco.
{? endif ?}

4DA e em si uma automacao de Nivel 3. Ele ingere conteudo de dezenas de fontes, pontua cada item com o algoritmo PASIFA e mostra apenas o que e relevante para seu trabalho — tudo sem voce levantar um dedo. Voce nao verifica manualmente Hacker News, Reddit e 50 feeds RSS. O 4DA faz isso e mostra o que importa.

Construa seus pipelines de renda da mesma forma.

O relatorio de atencao do 4DA (`/attention_report` nas ferramentas MCP) mostra onde seu tempo realmente vai versus onde deveria ir. Rode-o antes de decidir o que automatizar. A diferenca entre "tempo gasto" e "tempo que deveria ser gasto" e seu roadmap de automacao.

As ferramentas de classificacao de sinais (`/get_actionable_signals`) podem alimentar diretamente seu pipeline de monitoramento de mercado — deixando a camada de inteligencia do 4DA fazer a pontuacao inicial antes do seu pipeline personalizado fazer a analise especifica do nicho.

Se voce esta construindo pipelines que monitoram fontes para oportunidades, nao reinvente o que o 4DA ja faz. Use seu servidor MCP como um bloco de construcao no seu stack de automacao.

---

### O Que Vem a Seguir: Modulo S — Empilhando Fluxos

O Modulo T deu as ferramentas para fazer cada fluxo de renda funcionar eficientemente. O Modulo S (Empilhando Fluxos) responde a proxima pergunta: **quantos fluxos voce deve operar, e como eles se encaixam?**

Aqui esta o que o Modulo S cobre:

- **Teoria de portfolio para fluxos de renda** — por que 3 fluxos superam 1, e por que 10 fluxos superam nenhum
- **Correlacao de fluxos** — quais fluxos se reforcam mutuamente e quais competem pelo seu tempo
- **O piso de renda** — construindo uma base de receita recorrente que cobre seus custos antes de experimentar
- **Rebalanceamento** — quando dobrar a aposta em um vencedor e quando encerrar um perdedor
- **A arquitetura de $10K/mes** — combinacoes especificas de fluxos que alcancam cinco digitos com 15-20 horas por semana

Voce tem a infraestrutura (Modulo S), os fossos (Modulo T), os motores (Modulo R), o playbook de lancamento (Modulo E), o radar de tendencias (Modulo E), e agora a automacao (Modulo T). O Modulo S amarra tudo em um portfolio de renda sustentavel e crescente.

---

**O pipeline roda. O rascunho esta pronto. Voce gasta 10 minutos revisando.**

**Isso e automacao tatica. E assim que voce escala.**

*Seu equipamento. Suas regras. Sua receita.*
