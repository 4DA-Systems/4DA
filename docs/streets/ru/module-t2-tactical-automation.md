# Модуль T: Тактическая Автоматизация

**Курс STREETS по доходу для разработчиков — Платный модуль**
*Недели 12-13 | 6 Уроков | Результат: Один Автоматизированный Пайплайн, Генерирующий Ценность*

> "LLM, агенты, MCP и cron-задачи как мультипликаторы силы."

---

У тебя работают движки дохода. У тебя есть клиенты. У тебя есть процессы, которые работают. И ты тратишь 60-70% своего времени, делая одно и то же снова и снова: обрабатывая входные данные, форматируя выходные, проверяя мониторы, отправляя обновления, просматривая очереди.

Это время — твой самый дорогой ресурс, и ты сжигаешь его на задачах, с которыми справился бы VPS за {= regional.currency_symbol | fallback("$") =}5/месяц.

{@ insight hardware_benchmark @}

Этот модуль — о том, чтобы систематически убирать себя из цикла — не полностью (это ловушка, которую мы разберём в Уроке 5), а из 80% работы, которая не требует твоего суждения. Результат: твои потоки дохода приносят выручку, пока ты спишь, пока ты на основной работе, пока строишь следующую штуку.

К концу этих двух недель у тебя будет:

- Чёткое понимание четырёх уровней автоматизации и где ты находишься сегодня
- Работающие cron-задачи и запланированные автоматизации на твоей инфраструктуре
- Как минимум один пайплайн на основе LLM, обрабатывающий входные данные без твоего участия
- Понимание агентных систем и когда они экономически оправданы
- Фреймворк «человек-в-цикле», чтобы автоматизация не уничтожила твою репутацию
- Один полный, развёрнутый пайплайн, генерирующий ценность без твоего активного участия

{? if stack.primary ?}
Твой основной стек — {= stack.primary | fallback("твой основной стек") =}, поэтому примеры автоматизации впереди будут наиболее применимы при адаптации к этой экосистеме. Большинство примеров используют Python для портативности, но паттерны переносятся на любой язык.
{? endif ?}

Это модуль с наибольшим количеством кода во всём курсе. Как минимум половина того, что следует дальше — это исполняемый код. Копируй, адаптируй, развёртывай.

Давай автоматизировать.

---

## Урок 1: Пирамида Автоматизации

*"Большинство разработчиков автоматизируют на Уровне 1. Деньги — на Уровне 3."*

### Четыре Уровня

Каждая автоматизация в твоём стеке дохода находится где-то на этой пирамиде:

```
┌───────────────────────────────┐
│  Уровень 4: Автономные Агенты │  ← Принимает решения за тебя
│  (ИИ решает И действует)      │
├───────────────────────────────┤
│  Уровень 3: Интеллектуальные  │  ← Деньги здесь
│  Пайплайны (на основе LLM)   │
├───────────────────────────────┤
│  Уровень 2: Запланированная   │  ← Большинство разработчиков
│  Автоматизация (cron+скрипты) │     останавливаются здесь
├───────────────────────────────┤
│  Уровень 1: Ручная работа     │  ← Где большинство
│  с Шаблонами (копировать-     │     разработчиков
│  вставить)                    │
└───────────────────────────────┘
```

Давай будем конкретны о том, как каждый уровень выглядит на практике.

### Уровень 1: Ручная работа с Шаблонами

Ты делаешь работу сам, но у тебя есть чек-листы, шаблоны и сниппеты, чтобы ускорить процесс.

**Примеры:**
- Ты пишешь пост в блог, используя markdown-шаблон с предзаполненным frontmatter
- Ты выставляешь счета клиентам, дублируя счёт прошлого месяца и меняя цифры
- Ты отвечаешь на письма поддержки, используя сохранённые ответы
- Ты публикуешь контент, вручную запуская команду деплоя

**Затраты времени:** 100% твоего времени на единицу продукции.
**Частота ошибок:** Умеренная — ты человек, ты делаешь ошибки, когда устал.
**Потолок масштабирования:** Ты. Твои часы. Вот и всё.

Большинство разработчиков живут здесь и даже не осознают, что над ними есть пирамида.

### Уровень 2: Запланированная Автоматизация

Скрипты запускаются по расписанию. Ты написал логику один раз. Она выполняется без тебя.

**Примеры:**
- Cron-задача, которая проверяет твой RSS-фид и публикует новые статьи в соцсети
- GitHub Action, которая собирает и деплоит твой сайт каждое утро в 6 часов
- Скрипт, который запускается каждый час, чтобы проверить цены конкурентов и записать изменения
- Ежедневный бэкап базы данных, который запускается в 3 часа ночи

**Затраты времени:** Ноль на постоянной основе (после начальной настройки за 1-4 часа).
**Частота ошибок:** Низкая — детерминированная, одна и та же логика каждый раз.
**Потолок масштабирования:** Столько задач, сколько твоя машина может запланировать. Сотни.

Здесь большинство технических разработчиков и закрепляются. Это комфортно. Но у этого есть жёсткий предел: можно обрабатывать только задачи с детерминированной логикой. Если задача требует суждения, ты в тупике.

### Уровень 3: Интеллектуальные Пайплайны

Скрипты запускаются по расписанию, но включают LLM, который обрабатывает решения, требующие суждения.

**Примеры:**
- RSS-фиды загружаются, LLM резюмирует каждую статью, составляет черновик рассылки, ты просматриваешь 10 минут и отправляешь
- Письма с отзывами клиентов классифицируются по тональности и срочности, заготовленные ответы ставятся в очередь на твоё одобрение
- Новые вакансии в твоей нише парсятся, LLM оценивает релевантность, ты получаешь ежедневный дайджест из 5 возможностей вместо просмотра 200 объявлений
- Посты в блогах конкурентов мониторятся, LLM извлекает ключевые изменения продукта, ты получаешь еженедельный отчёт конкурентной разведки

**Затраты времени:** 10-20% от ручного времени. Ты проверяешь и одобряешь вместо того, чтобы создавать.
**Частота ошибок:** Низкая для задач классификации, умеренная для генерации (поэтому ты и проверяешь).
**Потолок масштабирования:** Тысячи элементов в день. Твоё узкое место — стоимость API, а не твоё время.

**Вот где деньги.** Уровень 3 позволяет одному человеку управлять потоками дохода, которые обычно требовали бы команды из 3-5 человек.

### Уровень 4: Автономные Агенты

ИИ-системы, которые наблюдают, принимают решения и действуют без твоего участия.

**Примеры:**
- Агент, который мониторит метрики твоего SaaS, обнаруживает падение регистраций, A/B-тестирует изменение цен и откатывает, если не работает
- Агент поддержки, который полностью автономно обрабатывает вопросы клиентов Tier 1, эскалируя тебе только сложные случаи
- Контент-агент, который определяет трендовые темы, генерирует черновики, планирует публикацию и мониторит показатели

**Затраты времени:** Близки к нулю для обработанных случаев. Ты проверяешь метрики, а не отдельные действия.
**Частота ошибок:** Полностью зависит от твоих гарантий. Без них: высокая. С хорошими гарантиями: удивительно низкая для узких доменов.
**Потолок масштабирования:** Фактически неограничен для задач в рамках компетенции агента.

Уровень 4 реален и достижим, но это не то, с чего ты начинаешь. И как мы разберём в Уроке 5, полностью автономные агенты, работающие с клиентами, опасны для твоей репутации при плохой реализации.

> **Прямой разговор:** Если ты сейчас на Уровне 1, не пытайся перепрыгнуть на Уровень 4. Ты потратишь недели на создание «автономного агента», который сломается в продакшене и повредит доверие клиентов. Поднимайся по пирамиде на один уровень за раз. Уровень 2 — это один вечер работы. Уровень 3 — проект на выходные. Уровень 4 приходит после того, как Уровень 3 проработает надёжно месяц.

### Самооценка: Где ты?

Для каждого из своих потоков дохода оцени себя честно:

| Поток Дохода | Текущий Уровень | Часов/Неделю | Можно Автоматизировать До |
|--------------|----------------|-------------|--------------------------|
| [напр., Рассылка] | [1-4] | [X] ч | [целевой уровень] |
| [напр., Обработка клиентов] | [1-4] | [X] ч | [целевой уровень] |
| [напр., Соцсети] | [1-4] | [X] ч | [целевой уровень] |
| [напр., Поддержка] | [1-4] | [X] ч | [целевой уровень] |

Самый важный столбец — «Часов/Неделю». Поток с наибольшим количеством часов и наименьшим уровнем — твоя первая цель для автоматизации. Там наибольший ROI.

### Экономика Каждого Уровня

Допустим, у тебя есть поток дохода, который занимает 10 часов/неделю твоего времени и приносит {= regional.currency_symbol | fallback("$") =}2 000/месяц:

| Уровень | Твоё Время | Твоя Эффективная Ставка | Стоимость Автоматизации |
|---------|-----------|------------------------|------------------------|
| Уровень 1 | 10 ч/неделю | $50/ч | $0 |
| Уровень 2 | 3 ч/неделю | $167/ч | $5/месяц (VPS) |
| Уровень 3 | 1 ч/неделю | $500/ч | $30-50/месяц (API) |
| Уровень 4 | 0,5 ч/неделю | $1 000/ч | $50-100/месяц (API + вычисления) |

Переход с Уровня 1 на Уровень 3 не меняет твой доход. Он меняет твою эффективную почасовую ставку с $50 до $500. А эти 9 освободившихся часов? Они идут на построение следующего потока дохода или улучшение текущего.

> **Типичная ошибка:** Автоматизировать сначала поток с наименьшим доходом, потому что он «проще». Нет. Автоматизируй поток, который съедает больше всего часов относительно своего дохода. Там ROI.

### Твоя очередь

1. Заполни таблицу самооценки выше для каждого потока дохода (или планируемого потока), который у тебя есть.
2. Определи свою цель автоматизации с наибольшим ROI: поток с наибольшим количеством часов и наименьшим уровнем автоматизации.
3. Запиши 3 самые времязатратные задачи в этом потоке. Ты автоматизируешь первую в Уроке 2.

---

## Урок 2: С Уровня 1 на 2 — Запланированная Автоматизация

*"Cron — из 1975 года. Он всё ещё работает. Используй его."*

### Основы Cron-задач

{? if computed.os_family == "windows" ?}
Ты на Windows, поэтому cron не является родным для твоей системы. У тебя два варианта: использовать WSL (Windows Subsystem for Linux) для настоящего cron, или использовать Планировщик задач Windows (рассмотрен ниже). WSL рекомендуется, если тебе комфортно с ним — все примеры cron в этом уроке работают напрямую в WSL. Если предпочитаешь нативный Windows, переходи к разделу Планировщика задач после этого.
{? endif ?}

Да, даже в 2026 году cron — король для запланированных задач. Он надёжен, он везде, и не требует облачного аккаунта, SaaS-подписки или YAML-схемы, которую приходится гуглить каждый раз.

**Синтаксис cron за 30 секунд:**

```
┌───────── минута (0-59)
│ ┌───────── час (0-23)
│ │ ┌───────── день месяца (1-31)
│ │ │ ┌───────── месяц (1-12)
│ │ │ │ ┌───────── день недели (0-7, 0 и 7 = Воскресенье)
│ │ │ │ │
* * * * *  команда
```

**Типичные расписания:**

```bash
# Каждый час
0 * * * *  /path/to/script.sh

# Каждый день в 6 утра
0 6 * * *  /path/to/script.sh

# Каждый понедельник в 9 утра
0 9 * * 1  /path/to/script.sh

# Каждые 15 минут
*/15 * * * *  /path/to/script.sh

# Первый день каждого месяца в полночь
0 0 1 * *  /path/to/script.sh
```

**Настройка cron-задачи:**

```bash
# Редактировать crontab
crontab -e

# Список существующих cron-задач
crontab -l

# КРИТИЧНО: Всегда задавай переменные окружения в начале
# Cron запускается с минимальным окружением — PATH может не включать твои инструменты
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# Логируй вывод, чтобы можно было отладить сбои
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **Типичная ошибка:** Написать скрипт, который отлично работает при ручном запуске, а затем он тихо падает в cron, потому что cron не загружает твой `.bashrc` или `.zshrc`. Всегда используй абсолютные пути в cron-скриптах. Всегда устанавливай `PATH` в начале crontab. Всегда перенаправляй вывод в файл лога.

### Облачные Планировщики, Когда Cron Недостаточно

Если твоя машина не работает 24/7, или тебе нужно что-то более надёжное, используй облачный планировщик:

**GitHub Actions (бесплатно для публичных репо, 2 000 мин/месяц на приватных):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # Каждый день в 6 утра UTC
    - cron: '0 6 * * *'
  # Разрешить ручной запуск для тестирования
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

**Vercel Cron (бесплатно на Hobby-плане, 1 в день; Pro-план: безлимитно):**

```typescript
// api/cron/daily-report.ts
// Эндпоинт Vercel cron — настрой расписание в vercel.json

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Проверяем, что вызывает именно Vercel, а не случайный HTTP-запрос
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // Твоя логика автоматизации здесь
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

### Реальные Автоматизации для Создания Прямо Сейчас

Вот пять автоматизаций, которые ты можешь реализовать сегодня. Каждая занимает 30-60 минут и устраняет часы ручной работы еженедельно.

#### Автоматизация 1: Авто-Публикация Контента по Расписанию

Ты пишешь посты в блог заранее. Этот скрипт публикует их в запланированное время.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — Публикация markdown-постов в назначенную дату.
Запускать ежедневно через cron: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# Эндпоинт API твоей CMS (Hashnode, Dev.to, Ghost и т.д.)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """Загрузить список уже опубликованных файлов."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """Сохранить список опубликованных файлов."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """Извлечь YAML-стиль frontmatter из markdown-файла."""
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
    """Проверить, нужно ли публиковать пост сегодня."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """Опубликовать пост через API CMS."""
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
        print(f"  Опубликовано: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  ОШИБКА: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"Проверяю {len(posts)} постов...")

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
    print(f"Всего опубликовано: {len(published)}")

if __name__ == "__main__":
    main()
```

**Твои markdown-посты выглядят так:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Содержимое поста здесь...
```

Пиши посты, когда приходит вдохновение. Устанавливай дату. Скрипт делает остальное.

#### Автоматизация 2: Авто-Публикация в Соцсетях при Новом Контенте

Когда твой блог публикует что-то новое, это автоматически постится в Twitter/X и Bluesky.

```python
#!/usr/bin/env python3
"""
social_poster.py — Публикация в соцсети при появлении нового контента.
Запускать каждые 30 минут: */30 * * * * python3 /path/to/social_poster.py
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
    """Распарсить RSS-фид и вернуть список элементов."""
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
    """Опубликовать в Bluesky через AT Protocol."""
    # Шаг 1: Создать сессию
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

    # Шаг 2: Создать пост
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
    print(f"  Опубликовано в Bluesky: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # Форматируем пост
        text = f"{item['title']}\n\n{item['link']}"

        # У Bluesky лимит 300 символов
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  Ошибка публикации: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

Стоимость: $0. Работает на твоей машине или бесплатном GitHub Action.

#### Автоматизация 3: Мониторинг Цен Конкурентов

Узнай мгновенно, когда конкурент меняет цены. Больше никакой ручной проверки каждую неделю.

```python
#!/usr/bin/env python3
"""
price_monitor.py — Мониторинг страниц цен конкурентов на изменения.
Запускать каждые 6 часов: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # или Discord, email и т.д.

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # Для мониторинга всей страницы; используй селектор для конкретных элементов
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """Получить страницу и вернуть хеш содержимого и фрагмент текста."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # Взять первые 500 символов видимого текста для контекста
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
    """Отправить оповещение через Slack webhook (замени на Discord, email и т.д.)."""
    if not ALERT_WEBHOOK:
        print(f"ОПОВЕЩЕНИЕ (webhook не настроен): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  Ошибка загрузки {name}: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"ОБНАРУЖЕНО ИЗМЕНЕНИЕ ЦЕН: {name}\n"
                f"URL: {url}\n"
                f"Изменено: {datetime.utcnow().isoformat()}Z\n"
                f"Предыдущий хеш: {previous_hash[:12]}...\n"
                f"Новый хеш: {current_hash[:12]}...\n"
                f"Проверь вручную."
            )
            send_alert(alert_msg)
            print(f"  ИЗМЕНЕНИЕ: {name}")
        else:
            print(f"  Без изменений: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### Автоматизация 4: Еженедельный Отчёт о Доходах

Каждый понедельник утром генерируется отчёт из данных о доходах и отправляется тебе по email.

```python
#!/usr/bin/env python3
"""
weekly_report.py — Генерация еженедельного отчёта о доходах из таблицы/базы данных.
Запускать по понедельникам в 7 утра: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """Создать таблицу доходов, если не существует."""
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
    """Генерация текстового еженедельного отчёта."""
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
    report.append(f"ЕЖЕНЕДЕЛЬНЫЙ ОТЧЁТ О ДОХОДАХ")
    report.append(f"Период: {week_ago.strftime('%Y-%m-%d')} — {today.strftime('%Y-%m-%d')}")
    report.append(f"Сгенерирован: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    Доход:    ${data['income']:>10,.2f}")
        report.append(f"    Расходы:  ${data['expense']:>10,.2f}")
        report.append(f"    Чистый:   ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  ОБЩИЙ ДОХОД:    ${total_income:>10,.2f}")
    report.append(f"  ОБЩИЕ РАСХОДЫ:  ${total_expenses:>10,.2f}")
    report.append(f"  ЧИСТАЯ ПРИБЫЛЬ: ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  ROI:            {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """Отправить отчёт по email."""
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
            f"Еженедельный отчёт о доходах — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nОтчёт отправлен на email.")
    conn.close()

if __name__ == "__main__":
    main()
```

#### Автоматизация 5: Авто-Бэкап Данных Клиентов

Никогда не теряй клиентские данные. Запускается каждую ночь и хранит 30 дней бэкапов.

```bash
#!/bin/bash
# backup_client_data.sh — Ночной бэкап данных клиентских проектов.
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# Создать сжатый бэкап
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# Удалить бэкапы старше периода хранения
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# Лог
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Бэкап завершён: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# Опционально: синхронизировать на второе хранилище (внешний диск, другая машина)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### Таймеры Systemd для Большего Контроля

Если тебе нужно больше, чем предлагает cron — например, упорядочивание зависимостей, лимиты ресурсов или автоматический повтор — используй таймеры systemd:

```ini
# /etc/systemd/system/income-publisher.service
[Unit]
Description=Публикация запланированного контента
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=youruser
ExecStart=/usr/bin/python3 /home/youruser/scripts/scheduled_publisher.py
Environment="CMS_API_KEY=your-key-here"
Environment="CMS_API_URL=https://api.example.com/posts"
# Перезапуск при сбое с экспоненциальной задержкой
Restart=on-failure
RestartSec=60

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/income-publisher.timer
[Unit]
Description=Запуск публикатора контента ежедневно в 6 утра

[Timer]
OnCalendar=*-*-* 06:00:00
Persistent=true
# Если машина была выключена в 6 утра, запустить при включении
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# Включить и запустить таймер
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# Проверить статус
systemctl list-timers --all | grep income

# Посмотреть логи
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Альтернатива — Планировщик Задач Windows

Если ты не используешь WSL, Планировщик задач Windows выполняет ту же функцию. Используй `schtasks` из командной строки или GUI Планировщика задач (`taskschd.msc`). Ключевое отличие: cron использует одно выражение, Планировщик задач использует отдельные поля для триггеров, действий и условий. Каждый пример cron в этом уроке напрямую переносится — планируй свои Python-скрипты точно так же, только через другой интерфейс.
{? endif ?}

### Твоя очередь

1. Выбери самую простую автоматизацию из этого урока, которая подходит к твоему потоку дохода.
2. Реализуй её. Не «планируй реализовать». Напиши код, протестируй, запланируй.
3. Настрой логирование, чтобы убедиться, что всё работает. Проверяй логи каждое утро 3 дня.
4. Когда стабилизируется, прекрати проверять ежедневно. Проверяй еженедельно. Это и есть автоматизация.

**Минимум:** Одна cron-задача, работающая надёжно, к концу сегодняшнего дня.

---

## Урок 3: С Уровня 2 на 3 — Пайплайны на Основе LLM

*"Добавь интеллект к своим автоматизациям. Здесь один человек начинает выглядеть как команда."*

### Паттерн

Каждый пайплайн на основе LLM следует одной и той же форме:

```
Источники Входных Данных → Загрузка → Обработка LLM → Форматирование → Доставка (или Очередь на Ревью)
```

Магия — в шаге «Обработка LLM». Вместо написания детерминированных правил на каждый возможный случай ты описываешь, что хочешь, на естественном языке, и LLM берёт на себя решения, требующие суждения.

### Когда Использовать Локальную Модель vs API

{? if settings.has_llm ?}
У тебя настроен {= settings.llm_provider | fallback("провайдер LLM") =} с {= settings.llm_model | fallback("твоей моделью LLM") =}. Это означает, что ты можешь начать строить интеллектуальные пайплайны прямо сейчас. Решение ниже поможет выбрать, когда использовать локальную настройку, а когда API для каждого пайплайна.
{? else ?}
У тебя ещё не настроен LLM. Пайплайны в этом уроке работают как с локальными моделями (Ollama), так и с облачными API. Настрой хотя бы одну перед построением первого пайплайна — Ollama бесплатна и устанавливается за 10 минут.
{? endif ?}

Это решение напрямую влияет на твою маржу:

| Фактор | Локально (Ollama) | API (Claude, GPT) |
|--------|------------------|-------------------|
| **Стоимость за 1М токенов** | ~$0,003 (электричество) | $0,15 - $15,00 |
| **Скорость (токенов/сек)** | 20-60 (8B на средней GPU) | 50-100+ |
| **Качество (8B локально vs API)** | Хорошо для классификации, извлечения | Лучше для генерации, рассуждений |
| **Приватность** | Данные не покидают твою машину | Данные идут к провайдеру |
| **Аптайм** | Зависит от твоей машины | 99,9%+ |
| **Пропускная способность пакетов** | Ограничена памятью GPU | Ограничена rate limits и бюджетом |

{? if profile.gpu.exists ?}
С {= profile.gpu.model | fallback("твоей GPU") =} на твоей машине, локальный инференс — сильный вариант. Скорость и размер модели, которую можешь запустить, зависят от VRAM — проверь, что помещается, прежде чем привязываться к пайплайну только на локальных моделях.
{? if computed.has_nvidia ?}
Видеокарты NVIDIA дают лучшую производительность Ollama благодаря ускорению CUDA. Ты сможешь комфортно запускать модели на 7-8B параметров, и возможно крупнее в зависимости от {= profile.gpu.vram | fallback("доступной VRAM") =}.
{? endif ?}
{? else ?}
Без выделенной GPU локальный инференс будет медленнее (только CPU). Он всё равно работает для небольших пакетных задач и классификации, но для всего, что чувствительно ко времени или высокообъёмно, API-модель будет практичнее.
{? endif ?}

**Практические правила:**
- **Большой объём, более низкая планка качества** (классификация, извлечение, тегирование) → Локально
- **Малый объём, критичное качество** (контент для клиентов, сложный анализ) → API
- **Чувствительные данные** (информация клиентов, проприетарные данные) → Локально, всегда
- **Более 10 000 элементов/месяц** → Локально экономит реальные деньги

**Сравнение ежемесячных затрат для типичного пайплайна:**

```
Обработка 5 000 элементов/месяц, ~500 токенов на элемент:

Локально (Ollama, llama3.1:8b):
  2 500 000 токенов × $0,003/1M = $0,0075/месяц
  Практически бесплатно.

API (GPT-4o-mini):
  2 500 000 входных токенов × $0,15/1M = $0,375
  2 500 000 выходных токенов × $0,60/1M = $1,50
  Итого: ~$1,88/месяц
  Дёшево, но в 250 раз дороже локального.

API (Claude 3.5 Sonnet):
  2 500 000 входных токенов × $3,00/1M = $7,50
  2 500 000 выходных токенов × $15,00/1M = $37,50
  Итого: ~$45/месяц
  Качество отличное, но в 6 000 раз дороже локального.
```

Для пайплайнов классификации и извлечения разница в качестве между хорошо настроенной локальной 8B моделью и фронтирной API-моделью часто незначительна. Протестируй обе. Используй более дешёвую, которая соответствует твоей планке качества.

{@ insight cost_projection @}

### Пайплайн 1: Генератор Контента для Рассылки

Это самая распространённая LLM-автоматизация для разработчиков с контент-ориентированным доходом. RSS-фиды входят, черновик рассылки выходит.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — Загрузка RSS-фидов, резюмирование через LLM, генерация черновика рассылки.
Запускать ежедневно: 0 5 * * * python3 /path/to/newsletter_pipeline.py

Этот пайплайн:
1. Загружает новые статьи из множества RSS-фидов
2. Отправляет каждую в локальный LLM для резюмирования
3. Ранжирует по релевантности для твоей аудитории
4. Генерирует отформатированный черновик рассылки
5. Сохраняет черновик для твоего ревью (ты тратишь 10 мин на проверку, а не 2 часа на курирование)
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
    # Добавь свои нишевые фиды здесь
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
    """Распарсить RSS/Atom-фид и вернуть статьи."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # Обработка RSS и Atom фидов
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
        print(f"  Ошибка загрузки {url}: {e}")
        return []

def llm_process(prompt: str) -> str:
    """Отправить промпт в локальный LLM и получить ответ."""
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
        print(f"  Ошибка LLM: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """Использовать LLM для оценки релевантности и генерации резюме."""
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
    """Сформатировать оценённые статьи в черновик рассылки."""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# Твоя Рассылка — {today}")
    newsletter.append("")
    newsletter.append("*[ТВОЁ ВСТУПЛЕНИЕ — Напиши 2-3 предложения о теме этой недели]*")
    newsletter.append("")

    section_titles = {
        "tool": "Инструменты и Релизы",
        "technique": "Техники и Паттерны",
        "news": "Новости Индустрии",
        "tutorial": "Туториалы и Руководства",
        "opinion": "Мнения"
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
    newsletter.append("*[ТВОЁ ЗАКЛЮЧЕНИЕ — Над чем ты работаешь? На что читателям стоит обратить внимание?]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("Загружаю фиды...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} новых статей")

    if not all_articles:
        print("Нет новых статей. Пропускаю.")
        return

    print(f"\nОцениваю {len(all_articles)} статей через LLM...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # Фильтруем только релевантные статьи и сортируем по оценке
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # Берём топ-10
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} статей прошли порог релевантности (>= 6/10)")

    # Генерируем черновик рассылки
    draft = generate_newsletter(top_articles)

    # Сохраняем черновик
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nЧерновик сохранён: {draft_path}")
    print("Проверь, добавь вступление/заключение и отправляй.")

if __name__ == "__main__":
    main()
```

**Сколько это стоит:**
- Обработка 50 статей/день с локальной 8B моделью: ~$0/месяц
- Твоё время: 10 минут на проверку черновика vs 2 часа ручного курирования
- Экономия времени в неделю: ~10 часов, если ведёшь еженедельную рассылку

### Пайплайн 2: Исследование Клиентов и Отчёты с Инсайтами

Этот пайплайн собирает публичные данные, анализирует их через LLM и создаёт отчёт, который можно продавать.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — Анализ публичных данных о компании/продукте и генерация отчётов.
Это услуга, которую ты можешь продавать: $200-500 за отчёт.

Использование: python3 research_pipeline.py "Название Компании" "их-сайт.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# Используй модель побольше для качества в платных отчётах
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# Или используй API для качества, пригодного для клиентов:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """Маршрутизация на локальную или API модель в зависимости от конфигурации."""
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
    """Собрать публично доступные данные о компании."""
    data = {"company": company, "domain": domain}

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
        data["website_status"] = f"Ошибка: {e}"

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
    """Генерация аналитического отчёта с помощью LLM."""
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
        print("Использование: python3 research_pipeline.py 'Название Компании' 'домен.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"Исследую: {company} ({domain})")
    print(f"Используя: {'API (Claude)' if USE_API else 'Локально (Ollama)'}")

    print("Собираю публичные данные...")
    data = gather_public_data(company, domain)

    print("Генерирую анализ...")
    report = generate_report(company, domain, data)

    final_report = f"""# Исследовательский Отчёт: {company}

**Сгенерирован:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**Домен:** {domain}
**Модель анализа:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*Этот отчёт сгенерирован с использованием только публично доступных данных.
Проприетарные или приватные данные не были задействованы.*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nОтчёт сохранён: {filepath}")
    print(f"Стоимость API: ~${'0.02-0.05' if USE_API else '0.00'}")

if __name__ == "__main__":
    main()
```

**Бизнес-модель:** Бери $200-500 за персональный исследовательский отчёт. Твои затраты: $0,05 на API-вызовы и 15 минут проверки. Ты можешь производить 3-4 отчёта в час, когда пайплайн стабилен.

### Пайплайн 3: Мониторинг Рыночных Сигналов

Это пайплайн, который подсказывает, что строить дальше. Он мониторит множество источников, классифицирует сигналы и оповещает тебя, когда возможность пересекает твой порог.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — Мониторинг публичных источников на рыночные возможности.
Запускать каждые 2 часа: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# Определение твоей ниши — LLM использует это для оценки релевантности
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
    """Загрузить топ-истории Hacker News."""
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
        print(f"  Ошибка загрузки HN: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """Использовать LLM для классификации сигнала на рыночную возможность."""
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
        item["reasoning"] = f"Классификация не удалась: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """Отправить оповещение о возможности с высокой оценкой."""
    msg = (
        f"ОБНАРУЖЕНА ВОЗМОЖНОСТЬ (оценка: {item['opportunity_score']}/10)\n"
        f"Тип: {item['opportunity_type']}\n"
        f"Заголовок: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Почему: {item['reasoning']}\n"
        f"Действие: {item['action']}"
    )

    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ОПОВЕЩЕНИЕ: {msg}")

def main():
    seen = load_seen()

    print("Загружаю сигналы...")
    items = fetch_hn_top(30)

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} новых сигналов для классификации")

    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("Готово.")

if __name__ == "__main__":
    main()
```

**Что это делает на практике:** Ты получаешь уведомление в Slack 2-3 раза в неделю с чем-то вроде «ВОЗМОЖНОСТЬ: Вышел новый фреймворк без стартер-кита — ты мог бы собрать один в эти выходные.» Этот сигнал, действие по нему раньше других — так ты остаёшься впереди.

> **Прямой разговор:** Качество выходных данных этих пайплайнов полностью зависит от твоих промптов и определения ниши. Если твоя ниша размыта («Я веб-разработчик»), LLM будет помечать всё подряд. Если она конкретна («Я строю десктопные приложения на Tauri для рынка разработчиков, ориентированных на приватность»), он будет хирургически точен. Потрать 30 минут на правильное определение ниши. Это единственный вход с наибольшим рычагом для каждого пайплайна, который ты построишь.

### Твоя очередь

{? if stack.contains("python") ?}
Хорошая новость: примеры пайплайнов выше уже на твоём основном языке. Ты можешь скопировать их напрямую и начать адаптировать. Сосредоточься на правильном определении ниши и промптах — оттуда идёт 90% качества выходных данных.
{? else ?}
Примеры выше используют Python для портативности, но паттерны работают на любом языке. Если ты предпочитаешь строить на {= stack.primary | fallback("своём основном стеке") =}, ключевые компоненты для репликации: HTTP-клиент для загрузки RSS/API, парсинг JSON для ответов LLM и файловый I/O для управления состоянием. Взаимодействие с LLM — это просто HTTP POST в Ollama или облачный API.
{? endif ?}

1. Выбери один из трёх пайплайнов выше (рассылка, исследование или мониторинг сигналов).
2. Адаптируй его под свою нишу. Смени фиды, описание аудитории, критерии классификации.
3. Запусти вручную 3 раза, чтобы проверить качество выходных данных.
4. Настраивай промпты, пока выходные данные не станут полезными без тяжёлого редактирования.
5. Запланируй через cron.

**Цель:** Один пайплайн на основе LLM, работающий по расписанию, в течение 48 часов после прочтения этого урока.

---

## Урок 4: С Уровня 3 на 4 — Агентные Системы

*"Агент — это просто цикл, который наблюдает, решает и действует. Построй один."*

### Что «Агент» На Самом Деле Означает в 2026

Убери весь хайп. Агент — это программа, которая:

1. **Наблюдает** — читает какие-то входные данные или состояние
2. **Решает** — использует LLM для определения, что делать
3. **Действует** — выполняет решение
4. **Повторяет** — возвращается к шагу 1

Вот и всё. Разница между пайплайном (Уровень 3) и агентом (Уровень 4) в том, что агент повторяет. Он действует на основе своих собственных выходных данных. Он обрабатывает многоэтапные задачи, где следующий шаг зависит от результата предыдущего.

Пайплайн обрабатывает элементы по одному через фиксированную последовательность. Агент навигирует непредсказуемую последовательность на основе того, что встречает.

### MCP-Серверы, Которые Обслуживают Клиентов

MCP-сервер — одна из самых практичных систем, смежных с агентами, которые ты можешь построить. Он предоставляет инструменты, которые ИИ-агент (Claude Code, Cursor и т.д.) может вызывать от имени твоих клиентов.

{? if stack.contains("typescript") ?}
Пример MCP-сервера ниже использует TypeScript — как раз в твоей зоне. Ты можешь расширить его с помощью существующих TypeScript-инструментов и развернуть вместе с другими Node.js-сервисами.
{? endif ?}

Вот реальный пример: MCP-сервер, который отвечает на вопросы клиентов из документации твоего продукта.

```typescript
// mcp-docs-server/src/index.ts
// MCP-сервер, который отвечает на вопросы из документации.
// Твои клиенты указывают свой Claude Code на этот сервер и получают мгновенные ответы.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// Загружаем документацию в память при запуске
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

    // Разделяем по заголовкам для лучшего поиска
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
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
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

const docs = loadDocs();
console.error(`Загружено ${docs.length} фрагментов документации из ${DOCS_DIR}`);

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

**Бизнес-модель:** Дай этот MCP-сервер своим клиентам как часть продукта. Они получают мгновенные ответы без тикетов в поддержку. Ты тратишь меньше времени на поддержку. Все в выигрыше.

Для премиума: бери $9-29/месяц за хостируемую версию с векторным поиском, версионированной документацией и аналитикой по тому, что клиенты спрашивают.

### Автоматическая Обработка Обратной Связи от Клиентов

Этот агент читает обратную связь клиентов (из email, тикетов или формы), классифицирует её и создаёт черновики ответов и тикеты по функциям.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — Обработка обратной связи клиентов в классифицированные, действенные элементы.
Паттерн «ИИ пишет черновик, человек одобряет».

Запускать каждый час: 0 * * * * python3 /path/to/feedback_agent.py
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
    """Классифицировать обратную связь и сгенерировать черновик ответа."""

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
        feedback["draft_response"] = "[Классификация не удалась — требуется ручная проверка]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"Нет директории входящих: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("Новой обратной связи нет.")
        return

    print(f"Обрабатываю {len(inbox_files)} элементов обратной связи...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  Пропускаю {filepath.name}: {e}")
            continue

        print(f"  Обрабатываю: {feedback.get('subject', 'Без темы')[:50]}...")
        processed = process_feedback(feedback)

        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Неизвестно"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nОбработано: {len(review_queue)}")
    print(f"Критических: {critical}")
    print(f"Требуют твоего внимания: {needs_human}")
    print(f"Очередь ревью: {review_path}")

if __name__ == "__main__":
    main()
```

**Как это работает на практике:**
1. Клиенты отправляют обратную связь (через форму, email или систему поддержки)
2. Обратная связь приходит как JSON-файлы в директорию входящих
3. Агент обрабатывает каждый: классифицирует, резюмирует, пишет черновик ответа
4. Ты открываешь очередь ревью раз или два в день
5. Для простых элементов (благодарности, базовые вопросы с хорошими черновиками ответов) ты одобряешь черновик
6. Для сложных элементов (баги, сердитые клиенты) ты пишешь личный ответ
7. Чистое время: 15 минут в день вместо 2 часов

### Паттерн «ИИ Пишет, Человек Одобряет»

Этот паттерн — ядро практической автоматизации Уровня 4. Агент делает тяжёлую работу. Ты принимаешь решения, требующие суждения.

```
              ┌──────────────┐
              │Агент пишет   │
              │  черновик    │
              └──────┬───────┘
                     │
              ┌──────▼───────┐
              │Очередь Ревью │
              └──────┬───────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼───┐ ┌───▼─────┐
    │Авто-отправ│ │Редакт│ │Эскалация│
    │ка(рутина) │ │+отпр.│ │(сложное)│
    └───────────┘ └──────┘ └─────────┘
```

**Правила: что агент делает полностью vs что ты проверяешь:**

| Агент делает полностью (без ревью) | Ты проверяешь перед отправкой |
|-------------------------------|--------------------------|
| Подтверждения получения («Мы получили ваше сообщение») | Ответы рассерженным клиентам |
| Обновления статуса («Ваш запрос обрабатывается») | Приоритизация запросов на функции |
| Ответы по FAQ (точное совпадение) | Всё, связанное с деньгами (возвраты, цены) |
| Классификация и удаление спама | Баг-репорты (нужно проверить) |
| Внутреннее логирование и категоризация | Всё, что ты не видел раньше |

> **Типичная ошибка:** Позволить агенту отвечать клиентам автономно с первого дня. Не делай этого. Начни с того, что агент пишет черновик всего, а ты одобряешь всё. Через неделю позволь ему авто-отправлять подтверждения. Через месяц — авто-отправлять ответы по FAQ. Выстраивай доверие постепенно — к самому себе и к своим клиентам.

### Твоя очередь

1. Выбери одно: построить MCP-сервер документации ИЛИ агент обработки обратной связи.
2. Адаптируй под свой продукт/сервис. Если у тебя ещё нет клиентов, используй мониторинг сигналов из Урока 3 как своего «клиента» — обработай его выходные данные через паттерн агента обратной связи.
3. Запусти вручную 10 раз с разными входными данными.
4. Измерь: какой процент выходных данных пригоден без редактирования? Это твоя оценка качества автоматизации. Цель — 70%+ перед планированием.

---

## Урок 5: Принцип «Человек-в-Цикле»

*"Полная автоматизация — ловушка. Частичная автоматизация — суперсила."*

### Почему 80% Автоматизации Лучше 100%

Есть конкретная, измеримая причина, почему нельзя полностью автоматизировать процессы, связанные с клиентами: стоимость плохого выхода асимметрична.

Хороший автоматический выход экономит тебе 5 минут.
Плохой автоматический выход стоит тебе клиента, публичную жалобу, возврат или удар по репутации, на восстановление которого уходят месяцы.

Математика:

```
100% автоматизации:
  1 000 выходов/месяц × 95% качества = 950 хороших + 50 плохих
  50 плохих × $50 средняя стоимость (возврат + поддержка + репутация) = $2 500/месяц ущерба

80% автоматизации + 20% проверки человеком:
  800 авто-обработанных, 200 проверенных человеком
  800 × 95% качества = 760 хороших + 40 плохих авто
  200 × 99% качества = 198 хороших + 2 плохих от человека
  42 всего плохих × $50 = $2 100/месяц ущерба
  НО: ты ловишь 38 плохих до того, как они дойдут до клиентов

  Реальных плохих выходов, дошедших до клиентов: ~4
  Реальный ущерб: ~$200/месяц
```

Это сокращение ущерба в 12 раз. Твоё время на проверку 200 выходов (может быть 2 часа) экономит $2 300/месяц в ущербе.

### Никогда Не Автоматизируй Полностью Это

Некоторые вещи всегда должны иметь человека в цикле, независимо от того, насколько хорош ИИ:

1. **Коммуникации с клиентами** — Плохо написанное письмо может потерять клиента навсегда. Шаблонный, явно ИИ-сгенерированный ответ может подорвать доверие. Проверяй.

2. **Финансовые операции** — Возвраты, изменения цен, выставление счетов. Всегда проверяй. Цена ошибки — реальные деньги.

3. **Опубликованный контент с твоим именем** — Твоя репутация строится годами и может быть разрушена одним плохим постом. Десять минут проверки — дешёвая страховка.

4. **Юридический контент или контент, связанный с соответствием** — Всё, что касается контрактов, политик конфиденциальности, условий обслуживания. ИИ делает уверенно звучащие юридические ошибки.

5. **Решения о найме или о людях** — Если когда-нибудь будешь аутсорсить, никогда не позволяй ИИ принимать окончательное решение о том, с кем работать.

### Долг Автоматизации

{@ mirror automation_risk_profile @}

Долг автоматизации хуже технического долга, потому что он невидим, пока не взрывается.

**Как выглядит долг автоматизации:**
- Бот в соцсетях, который постит не в то время, потому что сменился часовой пояс
- Пайплайн рассылки, который уже 3 недели включает битую ссылку, потому что никто не проверяет
- Монитор цен, который перестал работать, когда конкурент переделал свою страницу
- Скрипт бэкапа, который тихо падает, потому что диск заполнился

**Как предотвратить:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — Мониторинг всех автоматизаций на тихие сбои.
Запускать каждое утро: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

AUTOMATIONS = [
    {
        "name": "Пайплайн Рассылки",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,
    },
    {
        "name": "Постер в Соцсети",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,
        "max_age_hours": 2,
    },
    {
        "name": "Монитор Конкурентов",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,
    },
    {
        "name": "Бэкап Клиентов",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,
    },
    {
        "name": "Сервер Ollama",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        p = Path(path)
        if not p.exists():
            return False, f"Директория не найдена: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"Нет файлов по паттерну {automation['pattern']} в {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        if not os.path.exists(path):
            return False, f"Файл не найден: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Последний выход {age.total_seconds()/3600:.1f}ч назад (макс: {automation['max_age_hours']}ч)"

    return True, f"OK (последний выход {age.total_seconds()/3600:.1f}ч назад)"

def check_http(automation: dict) -> tuple[bool, str]:
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"OK (HTTP {resp.status_code})"
        return False, f"Неожиданный статус: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"Ошибка соединения: {e}"

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
            ok, msg = False, f"Неизвестный тип проверки: {check_type}"

        status = "OK" if ok else "СБОЙ"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"ПРОВЕРКА ЗДОРОВЬЯ АВТОМАТИЗАЦИЙ — {len(failures)} СБОЙ(ЕВ)\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nПроверь логи и исправь, пока не накопилось."
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

Запускай это каждое утро. Когда автоматизация тихо сломается (а она сломается), ты узнаешь в течение 24 часов, а не через 3 недели.

### Построение Очередей Ревью

Ключ к тому, чтобы «человек-в-цикле» работал эффективно — пакетная проверка. Не проверяй элементы по одному, по мере поступления. Ставь в очередь и проверяй пакетами.

```python
#!/usr/bin/env python3
"""
review_queue.py — Простая очередь ревью для ИИ-сгенерированных выходов.
Проверяй раз или два в день вместо постоянного мониторинга.
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
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
    if not os.path.isdir(QUEUE_DIR):
        print("Очередь пуста.")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("Очередь пуста.")
        return

    print(f"\n{'='*60}")
    print(f"ОЧЕРЕДЬ РЕВЬЮ — {len(pending)} элементов ожидают")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    Статей: {content.get('article_count', '?')}")
            print(f"    Черновик: {content.get('draft_path', 'неизвестно')}")
        elif item["type"] == "customer_response":
            print(f"    Кому: {content.get('customer', 'неизвестно')}")
            print(f"    Черновик: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    Текст: {content.get('text', '')[:100]}...")

        print(f"    Действия: [о]добрить  [р]едактировать  [у]далить  [п]ропустить")
        print()

if __name__ == "__main__":
    review_queue()
```

**Привычка ревью:** Проверяй очередь ревью в 8 утра и в 4 дня. Две сессии, 10-15 минут каждая. Всё остальное работает автономно между проверками.

> **Прямой разговор:** Представь, что происходит, когда ты пропускаешь проверку человеком: ты полностью автоматизируешь свою рассылку, LLM начинает вставлять галлюцинированные ссылки на несуществующие страницы, и подписчики замечают раньше тебя. Ты теряешь часть списка, и на восстановление доверия уходят месяцы. Напротив, разработчик, который автоматизирует 80% того же процесса — LLM курирует и пишет черновик, он тратит 10 минут на проверку — ловит эти галлюцинации до отправки. Разница не в автоматизации. Она в шаге проверки.

### Твоя очередь

1. Настрой скрипт `automation_healthcheck.py` для автоматизаций, которые ты построил в Уроках 2 и 3. Запланируй запуск каждое утро.
2. Реализуй очередь ревью для выходных данных автоматизации с наибольшим риском (всё, что связано с клиентами).
3. Взяв обязательство, проверяй очередь ревью дважды в день в течение одной недели. Записывай, сколько элементов одобряешь без изменений, сколько редактируешь и сколько отклоняешь. Эти данные покажут, насколько хороша твоя автоматизация на самом деле.

---

## Урок 6: Оптимизация Затрат и Твой Первый Пайплайн

*"Если ты не можешь сгенерировать $200 дохода из $200 расходов на API, исправь продукт — не бюджет."*

### Экономика Автоматизации на Основе LLM

Каждый вызов LLM имеет стоимость. Даже локальные модели стоят электричества и износа GPU. Вопрос в том, генерирует ли выход этого вызова больше ценности, чем стоит сам вызов.

{? if profile.gpu.exists ?}
Запуск локальных моделей на {= profile.gpu.model | fallback("твоей GPU") =} стоит примерно {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("несколько долларов") =} электричества в месяц для типичных нагрузок пайплайнов. Это базовый уровень, который нужно побить альтернативами API.
{? endif ?}

**Правило бюджета API в {= regional.currency_symbol | fallback("$") =}200/месяц:**

Если ты тратишь {= regional.currency_symbol | fallback("$") =}200/месяц на API-вызовы для своих автоматизаций, эти автоматизации должны генерировать как минимум {= regional.currency_symbol | fallback("$") =}200/месяц ценности — будь то прямой доход или сэкономленное время, которое ты конвертируешь в доход в другом месте.

Если не генерируют: проблема не в бюджете API. Она в дизайне пайплайна или в продукте, который он поддерживает.

### Отслеживание Стоимости на Выход

Добавь это в каждый пайплайн, который строишь:

```python
"""
cost_tracker.py — Отслеживание стоимости каждого вызова LLM и генерируемой ценности.
Импортируй в свои пайплайны для получения реальных данных о затратах.
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

PRICING = {
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    "claude-sonnet-4-20250514": {"input": 3.00, "output": 15.00},
    "claude-3-5-haiku-20241022": {"input": 0.80, "output": 4.00},
    "gpt-4o-mini": {"input": 0.15, "output": 0.60},
    "gpt-4o": {"input": 2.50, "output": 10.00},
}

def log_cost(pipeline, model, input_tokens, output_tokens, revenue_generated=0.0, item_id=""):
    prices = PRICING.get(model, {"input": 1.0, "output": 5.0})
    cost = (input_tokens / 1_000_000 * prices["input"]) + (output_tokens / 1_000_000 * prices["output"])

    entry = {
        "timestamp": datetime.utcnow().isoformat() + "Z",
        "pipeline": pipeline, "model": model,
        "input_tokens": input_tokens, "output_tokens": output_tokens,
        "cost_usd": round(cost, 6), "revenue_usd": revenue_generated, "item_id": item_id,
    }

    os.makedirs(os.path.dirname(COST_LOG), exist_ok=True)
    with open(COST_LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")
    return cost

def monthly_report() -> dict:
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
                    pipelines[pipeline] = {"total_cost": 0, "total_revenue": 0, "call_count": 0, "total_tokens": 0}
                pipelines[pipeline]["total_cost"] += entry["cost_usd"]
                pipelines[pipeline]["total_revenue"] += entry.get("revenue_usd", 0)
                pipelines[pipeline]["call_count"] += 1
                pipelines[pipeline]["total_tokens"] += entry["input_tokens"] + entry["output_tokens"]
    except FileNotFoundError:
        pass

    print(f"\nОТЧЁТ О ЗАТРАТАХ LLM — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    Вызовов: {data['call_count']}")
        print(f"    Токенов: {data['total_tokens']:,}")
        print(f"    Затраты: ${data['total_cost']:.4f}")
        print(f"    Доход:   ${data['total_revenue']:.2f}")
        print(f"    ROI:     {roi:.1f}x")
        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  ОБЩИЕ ЗАТРАТЫ: ${grand_cost:.4f}")
    print(f"  ОБЩИЙ ДОХОД:   ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  ОБЩИЙ ROI:     {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### Пакетная Обработка для Эффективности API

Если ты используешь API-модели, пакетная обработка экономит реальные деньги:

```python
"""
batch_api.py — Пакетные API-вызовы для эффективности.
Вместо 100 отдельных вызовов — пакетируй их.
"""

import os
import json
import time
import requests
from typing import Any

ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")

def batch_classify(items, system_prompt, model="claude-3-5-haiku-20241022", batch_size=10, delay_between_batches=1.0):
    """
    Классификация множества элементов эффективно через пакетирование в одиночные API-вызовы.

    Вместо 100 API-вызовов (100 элементов × 1 вызов):
      - 100 вызовов × ~500 входных токенов = 50 000 входных токенов
      - 100 вызовов × ~200 выходных токенов = 20 000 выходных токенов
      - Стоимость с Haiku: ~$0,12

    С пакетированием (10 элементов на вызов, 10 API-вызовов):
      - 10 вызовов × ~2 500 входных токенов = 25 000 входных токенов
      - 10 вызовов × ~1 000 выходных токенов = 10 000 выходных токенов
      - Стоимость с Haiku: ~$0,06

    50% экономии только от пакетирования.
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]
        items_text = "\n".join(f"[ITEM {j+1}] {json.dumps(item)}" for j, item in enumerate(batch))

        prompt = f"""Process each item below. For each item, provide a JSON object with your classification.

{items_text}

Respond with a JSON array containing one object per item, in the same order.
Each object should have: {{"item_index": <number>, "category": "<string>", "score": <1-10>}}"""

        try:
            resp = requests.post("https://api.anthropic.com/v1/messages", headers={
                "x-api-key": ANTHROPIC_KEY, "anthropic-version": "2023-06-01",
                "content-type": "application/json"
            }, json={"model": model, "max_tokens": 2000, "system": system_prompt,
                "messages": [{"role": "user", "content": prompt}]}, timeout=60)
            resp.raise_for_status()

            response_text = resp.json()["content"][0]["text"]
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]
            batch_results = json.loads(cleaned)
            results.extend(batch_results)
        except Exception as e:
            print(f"  Пакет {i//batch_size + 1} не удался: {e}")
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### Кэширование: Не Плати Дважды за Один Ответ

```python
"""
llm_cache.py — Кэширование ответов LLM для избежания повторной оплаты.
"""

import os, json, hashlib, sqlite3
from datetime import datetime, timedelta

CACHE_DB = os.path.expanduser("~/income/data/llm_cache.db")

def get_cache_db():
    os.makedirs(os.path.dirname(CACHE_DB), exist_ok=True)
    conn = sqlite3.connect(CACHE_DB)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY, model TEXT NOT NULL, response TEXT NOT NULL,
            created_at TEXT NOT NULL, hit_count INTEGER DEFAULT 0)""")
    conn.commit()
    return conn

def cache_key(model, prompt):
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model, prompt, max_age_hours=168):
    conn = get_cache_db()
    key = cache_key(model, prompt)
    row = conn.execute("SELECT response, created_at FROM cache WHERE key = ?", (key,)).fetchone()
    if row is None:
        conn.close()
        return None
    response, created_at = row
    age = datetime.utcnow() - datetime.fromisoformat(created_at)
    if age > timedelta(hours=max_age_hours):
        conn.execute("DELETE FROM cache WHERE key = ?", (key,))
        conn.commit(); conn.close()
        return None
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit(); conn.close()
    return response

def set_cached(model, prompt, response):
    conn = get_cache_db()
    key = cache_key(model, prompt)
    conn.execute("INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count) VALUES (?, ?, ?, ?, 0)",
        (key, model, response, datetime.utcnow().isoformat()))
    conn.commit(); conn.close()

def cache_stats():
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Записей в кэше: {total}")
    print(f"Всего попаданий кэша: {total_hits}")
    print(f"Примерная экономия: ~${total_hits * 0.002:.2f} (грубое среднее за вызов)")
```

**Используй в своих пайплайнах:**

```python
# В любом пайплайне, который вызывает LLM:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # Бесплатно!

    response = call_llm(model, prompt)  # Твоя существующая функция вызова LLM
    set_cached(model, prompt, response)
    return response
```

Для пайплайнов, которые повторно обрабатывают одни и те же типы контента (классификация, извлечение), кэширование может устранить 30-50% API-вызовов. Это 30-50% скидки с ежемесячного счёта.

### Построение Первого Полного Пайплайна: Шаг за Шагом

Вот полный процесс от «у меня ручной рабочий процесс» до «он работает, пока я сплю».

**Шаг 1: Опиши свой текущий ручной процесс.**

Запиши каждый шаг для одного конкретного потока дохода. Пример для рассылки:

```
1. Открыть 15 RSS-фидов в вкладках браузера (10 мин)
2. Просканировать заголовки, открыть интересные (20 мин)
3. Прочитать 8-10 статей детально (40 мин)
4. Написать резюме для топ-5 (30 мин)
5. Написать вступительный абзац (10 мин)
6. Отформатировать в email-инструменте (15 мин)
7. Отправить списку (5 мин)

Итого: ~2 часа 10 минут
```

**Шаг 2: Определи три самых времязатратных шага.**

Из примера: Чтение статей (40 мин), написание резюме (30 мин), сканирование заголовков (20 мин).

**Шаг 3: Автоматизируй сначала самый простой.**

Сканирование заголовков проще всего автоматизировать — это классификация. LLM оценивает релевантность, ты читаешь только те, что набрали высший балл.

**Шаг 4: Измерь сэкономленное время и качество.**

После автоматизации сканирования заголовков:
- Сэкономлено времени: 20 минут
- Качество: 90% совпадения с твоим ручным выбором
- Итог: 20 минут сэкономлено, потеря качества пренебрежимо мала

**Шаг 5: Автоматизируй следующий шаг.**

Теперь автоматизируй написание резюме. LLM пишет черновики резюме, ты редактируешь.

**Шаг 6: Продолжай до убывающей отдачи.**

```
До автоматизации: 2ч10мин на рассылку
После Уровня 2 (запланированная загрузка): 1ч45мин
После Уровня 3 (оценка LLM + резюме): 25мин
После Уровня 3+ (LLM пишет вступление): 10мин только проверка

Экономия времени в неделю: ~2 часа
Экономия времени в месяц: ~8 часов
При $100/ч эффективной ставке: $800/месяц освобождённого времени
Стоимость API: $0 (локальный LLM) до $5/месяц (API)
```

**Шаг 7: Полный пайплайн, собранный вместе.**

Вот GitHub Action, который связывает всё для еженедельного пайплайна рассылки:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # Каждое воскресенье в 5 утра UTC
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
            -d '{"text":"Черновик рассылки готов для проверки. Проверь артефакты GitHub Actions."}'
```

Это запускается каждое воскресенье в 5 утра. К тому времени, как ты проснёшься, черновик ждёт. Ты тратишь 10 минут на проверку за кофе, нажимаешь «отправить», и рассылка опубликована на неделю.

### Твоя Очередь: Построй Свой Пайплайн

Это результат модуля. К концу этого урока у тебя должен быть один полный развёрнутый и работающий пайплайн.

**Требования к пайплайну:**
1. Он работает по расписанию без твоего участия
2. Включает как минимум один шаг обработки LLM
3. Имеет шаг проверки человеком для контроля качества
4. Имеет проверку здоровья, чтобы ты знал, если он сломается
5. Подключён к реальному потоку дохода (или потоку, который ты строишь)

**Чек-лист:**

- [ ] Выбран поток дохода для автоматизации
- [ ] Описан ручной процесс (все шаги, с оценками времени)
- [ ] Определены 3 самых времязатратных шага
- [ ] Автоматизирован как минимум первый шаг (классификация/оценка/фильтрация)
- [ ] Добавлена обработка LLM для второго шага (резюмирование/генерация/извлечение)
- [ ] Построена очередь ревью для проверки человеком
- [ ] Настроена проверка здоровья автоматизации
- [ ] Развёрнуто по расписанию (cron, GitHub Actions или systemd timer)
- [ ] Отслежены затраты и экономия времени за один полный цикл
- [ ] Документирован пайплайн (что делает, как исправлять, что мониторить)

Если ты выполнил все десять пунктов этого чек-листа, у тебя работает автоматизация Уровня 3. Ты только что освободил часы своей недели, которые можно реинвестировать в построение новых потоков или улучшение существующих.

---

## Модуль T: Завершён

{@ temporal automation_progress @}

### Что Ты Построил за Две Недели

1. **Понимание пирамиды автоматизации** — ты знаешь, где находишься и куда каждый из твоих потоков дохода должен двигаться.
2. **Запланированные автоматизации**, работающие на cron или облачных планировщиках — негламурный фундамент, который делает всё остальное возможным.
3. **Пайплайны на основе LLM**, которые берут на себя решения, требующие суждения, которые ты раньше принимал вручную — классификация, резюмирование, генерация, мониторинг.
4. **Агентные паттерны**, которые ты можешь развернуть для взаимодействия с клиентами, обработки обратной связи и продуктов на основе MCP.
5. **Фреймворк «человек-в-цикле»**, который защищает твою репутацию, при этом экономя 80%+ твоего времени.
6. **Отслеживание и оптимизация затрат**, чтобы твои автоматизации генерировали прибыль, а не просто активность.
7. **Один полный, развёрнутый пайплайн**, генерирующий ценность без твоего активного участия.

### Эффект Сложного Процента

Вот что произойдёт за следующие 3 месяца, если ты будешь поддерживать и расширять то, что построил в этом модуле:

```
Месяц 1: Один пайплайн, экономия 5-8 часов/неделю
Месяц 2: Два пайплайна, экономия 10-15 часов/неделю
Месяц 3: Три пайплайна, экономия 15-20 часов/неделю

При $100/ч эффективной ставке, это $1 500-2 000/месяц
освобождённого времени — времени, которое ты инвестируешь в новые потоки.

Освобождённое время Месяца 1 строит пайплайн для Месяца 2.
Освобождённое время Месяца 2 строит пайплайн для Месяца 3.
Автоматизация накапливается.
```

Вот как один разработчик работает как команда из пяти. Не работая больше. Строя системы, которые работают, пока ты нет.

---

### Интеграция 4DA

{? if dna.identity_summary ?}
На основе твоего профиля разработчика — {= dna.identity_summary | fallback("твоего фокуса разработки") =} — инструменты 4DA ниже напрямую соответствуют паттернам автоматизации, которые ты только что изучил. Инструменты классификации сигналов особенно релевантны для разработчиков в твоей области.
{? endif ?}

4DA сам является автоматизацией Уровня 3. Он загружает контент из десятков источников, оценивает каждый элемент алгоритмом PASIFA и показывает только то, что релевантно твоей работе — всё без твоего участия. Ты не проверяешь вручную Hacker News, Reddit и 50 RSS-фидов. 4DA делает это и показывает, что важно.

Строй свои пайплайны дохода так же.

Отчёт о внимании 4DA (`/attention_report` в MCP-инструментах) показывает, куда реально уходит твоё время vs куда оно должно уходить. Запусти его перед тем, как решить, что автоматизировать. Разрыв между «потраченным временем» и «временем, которое должно быть потрачено» — это твоя дорожная карта автоматизации.

Инструменты классификации сигналов (`/get_actionable_signals`) могут напрямую питать твой пайплайн мониторинга рынка — позволяя интеллектуальному слою 4DA делать первоначальную оценку, прежде чем твой пользовательский пайплайн делает нишевый анализ.

Если ты строишь пайплайны, которые мониторят источники на возможности, не изобретай то, что 4DA уже делает. Используй его MCP-сервер как строительный блок в своём стеке автоматизации.

---

### Что Дальше: Модуль S — Суммирование Потоков

Модуль T дал тебе инструменты для эффективной работы каждого потока дохода. Модуль S (Суммирование Потоков) отвечает на следующий вопрос: **сколько потоков нужно вести и как они сочетаются?**

Вот что покрывает Модуль S:

- **Теория портфеля для потоков дохода** — почему 3 потока лучше 1, а 10 потоков лучше ни одного
- **Корреляция потоков** — какие потоки усиливают друг друга, а какие конкурируют за твоё время
- **Пол дохода** — построение базы повторяющегося дохода, которая покрывает расходы, прежде чем экспериментировать
- **Ребалансировка** — когда удваивать ставку на победителя и когда закрывать проигравшего
- **Архитектура $10K/месяц** — конкретные комбинации потоков, которые достигают пяти цифр при 15-20 часах в неделю

У тебя есть инфраструктура (Модуль S), рвы (Модуль T), движки (Модуль R), плейбук запуска (Модуль E), радар трендов (Модуль E) и теперь автоматизация (Модуль T). Модуль S связывает всё в устойчивый, растущий портфель дохода.

---

**Пайплайн работает. Черновик готов. Ты тратишь 10 минут на проверку.**

**Это тактическая автоматизация. Вот как ты масштабируешься.**

*Твоя машина. Твои правила. Твой доход.*
