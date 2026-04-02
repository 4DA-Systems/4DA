# Модуль R: Движки Дохода

**Курс STREETS по доходу для разработчиков — Платный модуль**
*Недели 5-8 | 8 Уроков | Результат: Твой Первый Движок Дохода + План для Движка #2*

> "Строй системы, которые генерируют доход, а не просто код, который поставляет функции."

---

У тебя есть инфраструктура (Модуль S). У тебя есть то, что конкуренты не могут легко скопировать (Модуль T). Теперь пора превратить всё это в деньги.

Это самый длинный модуль в курсе, потому что он самый важный. Восемь движков дохода. Восемь разных способов превратить твои навыки, оборудование и время в доход. Каждый — это полный плейбук с реальным кодом, реальными ценами, реальными платформами и реальной математикой.

{@ insight engine_ranking @}

Ты не будешь строить все восемь. Ты выберешь два.

**Стратегия 1+1:**
- **Движок 1:** Самый быстрый путь к первому доллару. Недели 5-6.
- **Движок 2:** Самый масштабируемый. Планирование на Неделях 7-8, строительство в Модуле E.

Почему два? Один поток дохода хрупок. Два движка, обслуживающих разные типы клиентов через разные каналы, дают устойчивость.

К концу модуля у тебя будет:

- Доход от Движка 1 (или инфраструктура для генерации в течение дней)
- Детальный план для Движка 2
- Чёткое понимание, какие движки соответствуют твоим навыкам, времени и готовности к риску
- Реальный, развёрнутый код

{? if progress.completed("T") ?}
Ты построил свои рвы в Модуле T. Теперь рвы становятся фундаментом движков дохода — чем сложнее копировать, тем устойчивее доход.
{? endif ?}

Никакой теории. Никакого «когда-нибудь». Давай строить.

---

## Урок 1: Цифровые Продукты

*"Самая близкая к печатанию денег вещь, которая при этом легальна."*

**Время до первого доллара:** 1-2 недели
**Постоянные затраты времени:** 2-4 часа/неделю (поддержка, обновления, маркетинг)
**Маржа:** 95%+ (после создания затраты близки к нулю)

### Почему Цифровые Продукты Первыми

{@ insight stack_fit @}

Цифровые продукты — движок с наивысшей маржой и наименьшим риском. Строишь один раз, продаёшь вечно.

Математика: 20-40 часов на создание, цена {= regional.currency_symbol | fallback("$") =}49, 10 копий в первый месяц = {= regional.currency_symbol | fallback("$") =}490, далее 5/месяц = {= regional.currency_symbol | fallback("$") =}245/месяц пассивно. Сложи три продукта — {= regional.currency_symbol | fallback("$") =}735/месяц пока спишь.

### Что Продаётся

{? if stack.primary ?}
Как разработчик на {= stack.primary | fallback("разработчик") =}, ты знаешь проблемы своего стека:
{? else ?}
Вот за что разработчики реально платят:
{? endif ?}

**Стартер-Киты:** Tauri 2.0 ($49-79), Next.js SaaS ($79-149), MCP-шаблоны ($29-49), ИИ-агент конфиги ($29-39), Rust CLI ($29-49).

**Библиотеки Компонентов:** Дашборд кит ($39-69), Email шаблоны ($29-49), Landing pages ($29-49).

**Конфигурация:** Docker Compose ($19-29), Nginx/Caddy ($19-29), GitHub Actions ($19-29).

> **Прямой разговор:** Лучше продаются продукты, решающие конкретную, немедленную проблему. «Экономь 40 часов» побеждает «изучи новый фреймворк» каждый раз.

### Где Продавать

**Gumroad** — Просто, 10% с продажи. **Lemon Squeezy** — Merchant of Record, 5% + $0.50, обрабатывает НДС/GST.
{? if regional.country ?}
*В {= regional.country | fallback("твоей стране") =} Merchant of Record берёт на себя налоговое соответствие.*
{? endif ?}
**Свой Сайт** — Stripe Checkout, 2.9% + $0.30. Максимальный контроль.
{? if regional.payment_processors ?}
*Доступные процессоры: {= regional.payment_processors | fallback("Stripe, PayPal") =}.*
{? endif ?}

### От Идеи до Размещения за 48 Часов

**Час 0-2:** Выбери продукт. **Час 2-16:** Построй. **Час 16-20:** Листинг на Lemon Squeezy. **Час 20-24:** Продающая страница (5 разделов). **Час 24-48:** Мягкий запуск.

### Лицензирование

Персональная ($49), Командная ($149), Расширенная ($299).

### Математика Дохода

{@ insight cost_projection @}

```
Доход на продажу (Lemon Squeezy): $46.05
$500/месяц = 11 продаж | $1,000/месяц = 22 | $2,000/месяц = 44
```

**Бенчмарки:** ShipFast — $528K за 4 месяца. Tailwind UI — $500K за 3 дня.

### Твоя очередь

{? if stack.primary ?}
1. Определи продукт (30 мин). 2. Построй MVP (8-16 ч). 3. Настрой Lemon Squeezy (30 мин). 4. Напиши продающую страницу (2 ч). 5. Мягкий запуск (1 ч).
{? else ?}
1. Определи продукт (30 мин). 2. Построй MVP (8-16 ч). 3. Настрой Lemon Squeezy (30 мин). 4. Напиши продающую страницу (2 ч). 5. Мягкий запуск (1 ч).
{? endif ?}

---

## Урок 2: Монетизация Контента

*"Ты уже знаешь вещи, за которые тысячи людей заплатили бы."*

**Время до первого доллара:** 2-4 недели | **Время:** 5-10 ч/неделю | **Маржа:** 70-95%

{@ insight stack_fit @}

Контент компаундится: месяц 1 = $0, месяц 6 = $500, месяц 12 = $3,000.

```
Доход = Трафик x Конверсия x Доход/Конверсия
Техблог: 50K посетителей x 2% x $5 = $5,000/месяц
Рассылка: 5K подписчиков x 10% премиум x $5/месяц = $2,500/месяц
YouTube: 10K подписчиков = $1,000-2,500/месяц
```

### Каналы

**Блог с партнёрками:** Vercel, DigitalOcean, Stripe, JetBrains, Hetzner.

**Рассылка:** Beehiiv (0% комиссии) или Ghost (полный контроль).

```python
#!/usr/bin/env python3
"""newsletter_pipeline.py — Полуавтоматическое производство рассылки."""
import requests, json
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
NICHE = "Rust ecosystem and systems programming"

def fetch_hn_stories(limit=30) -> list[dict]:
    story_ids = requests.get("https://hacker-news.firebaseio.com/v0/topstories.json").json()[:limit]
    return [requests.get(f"https://hacker-news.firebaseio.com/v0/item/{sid}.json").json()
            for sid in story_ids]

def classify_and_summarize(items: list[dict]) -> list[dict]:
    results = []
    for item in items:
        prompt = f"""Rate relevance to {NICHE} (1-10). If >= 7, summarize in 2 sentences.
Title: "{item.get('title','')}" URL: {item.get('url','')}
Output JSON: {{"relevance": N, "summary": "...", "category": "Tool|Tutorial|News|Research|Opinion"}}"""

        resp = requests.post(OLLAMA_URL, json={"model": "llama3.1:8b", "prompt": prompt,
            "stream": False, "format": "json", "options": {"temperature": 0.3}})
        try:
            data = json.loads(resp.json()["response"])
            if data.get("relevance", 0) >= 7:
                item.update(data)
                results.append(item)
        except (json.JSONDecodeError, KeyError):
            continue
    return sorted(results, key=lambda x: x.get("relevance", 0), reverse=True)

def generate_draft(items: list[dict]) -> str:
    items_text = "\n".join(f"- [{i.get('title','')}]({i.get('url','')}) — {i.get('summary','')}"
                          for i in items[:8])
    prompt = f"""Write a {NICHE} newsletter. Items:\n{items_text}\n
Include: intro (2-3 sentences), each item with analysis (WHY it matters, WHAT to do),
Quick Takes section, closing. Be opinionated. Markdown format."""

    resp = requests.post(OLLAMA_URL, json={"model": "llama3.1:8b", "prompt": prompt,
        "stream": False, "options": {"temperature": 0.5, "num_ctx": 4096}})
    return resp.json()["response"]

if __name__ == "__main__":
    stories = fetch_hn_stories()
    relevant = classify_and_summarize(stories)
    draft = generate_draft(relevant)
    filename = f"newsletter-draft-{datetime.now().strftime('%Y-%m-%d')}.md"
    open(filename, "w").write(draft)
    print(f"Черновик: {filename} — ТЕПЕРЬ добавь экспертизу, исправь ошибки, опубликуй.")
```

**YouTube:** Месяцы 1-3 = $0, 4-6 = $50-200, 7-12 = $500-1,500, Год 2 = $2,000-5,000.

**Бенчмарки:** TLDR Newsletter — $5-6.4M/год. Pragmatic Engineer — $1.5M+/год.

### Твоя очередь

1. Выбери канал (15 мин). 2. Определи нишу (30 мин). 3. Создай первый контент (4-8 ч). 4. Настрой монетизацию (1 ч). 5. Возьми расписание (5 мин).

---

## Урок 3: Микро-SaaS

*"Маленький инструмент, одна проблема, $9-29/месяц."*

**Время до первого доллара:** 4-8 недель | **Время:** 5-15 ч/неделю | **Маржа:** 80-90%

{@ insight stack_fit @}

Микро-SaaS — не стартап. Одна проблема, один человек, одна цена.

**Бенчмарки:** Pieter Levels ~$3M/год, 0 сотрудников. Bannerbear $50K+ MRR, 1 человек. 70% микро-SaaS < $1K/месяц.

{? if dna.top_engaged_topics ?}
Посмотри на свои темы: {= dna.top_engaged_topics | fallback("темы вовлечения") =}.
{? endif ?}

```
CAC < 3 мес. подписки | LTV/CAC > 3 | Расходы: $1-41/мес | Безубыточность: 1-5 клиентов
```

**Стек:** Hono + Turso + Lucia + Stripe + Vercel. Месячные расходы: $1/месяц до масштабирования.

```typescript
// Основной API для монитора аптайма
import { Hono } from "hono";
import { cors } from "hono/cors";
import { jwt } from "hono/jwt";
import Stripe from "stripe";

const app = new Hono();
const stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);
const PLAN_LIMITS = { free: 3, starter: 10, pro: 50 };

app.use("/api/*", cors());
app.use("/api/*", jwt({ secret: process.env.JWT_SECRET! }));

app.post("/api/monitors", async (c) => {
  const userId = c.get("jwtPayload").sub;
  const { url, interval } = await c.req.json();
  const plan = await db.getUserPlan(userId);
  const count = await db.getMonitorCount(userId);

  if (count >= (PLAN_LIMITS[plan] || 3)) {
    return c.json({ error: "Monitor limit reached", upgrade_url: "/pricing" }, 403);
  }

  const monitor = await db.createMonitor({
    userId, url,
    interval: Math.max(interval, plan === "free" ? 300 : 60),
    status: "unknown",
  });
  return c.json(monitor, 201);
});

app.get("/api/monitors", async (c) => {
  const userId = c.get("jwtPayload").sub;
  return c.json(await db.getMonitors(userId));
});

app.post("/webhooks/stripe", async (c) => {
  const sig = c.req.header("stripe-signature")!;
  const event = stripe.webhooks.constructEvent(
    await c.req.text(), sig, process.env.STRIPE_WEBHOOK_SECRET!
  );

  if (event.type.startsWith("customer.subscription.")) {
    const sub = event.data.object as Stripe.Subscription;
    const plan = event.type.includes("deleted")
      ? "free"
      : sub.items.data[0]?.price?.lookup_key || "free";
    await db.updateUserPlan(sub.metadata.userId!, plan);
  }
  return c.json({ received: true });
});

export default app;
```

### Твоя очередь

1. Найди идею (2 ч). 2. Валидируй (1-2 дня). 3. Построй MVP (2-4 нед). 4. Разверни (1 день). 5. Отслеживай экономику (постоянно).

---

## Урок 4: Автоматизация-как-Услуга

*"Бизнесы заплатят тысячи за соединение инструментов."*

**Время:** 1-2 нед | **Маржа:** 80-95%

{@ insight stack_fit @}

Ты берёшь $500-$5,000. Экономишь 10-40 ч/неделю. ROI для клиента = 1 месяц.

{? if settings.has_llm ?}
Твой локальный LLM ({= settings.llm_model | fallback("модель") =}) — твоё оружие для приватной автоматизации.
{? endif ?}

{? if regional.country == "US" ?}
Питч: **"Приватная автоматизация. Данные не покидают инфраструктуру. HIPAA/SOC 2."**
{? else ?}
Питч: **"Приватная автоматизация. Данные не покидают инфраструктуру. GDPR-совместимо."**
{? endif ?}

**Примеры:** Квалификация лидов ($3K), обработка счетов ($2.5K), переработка контента ($1.5K).

```python
#!/usr/bin/env python3
"""invoice_processor.py — Автоматическое извлечение данных из счетов."""
import json, subprocess, requests
from dataclasses import dataclass, asdict
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"
WATCH_DIR, PROCESSED_DIR, REVIEW_DIR = (
    Path("./invoices/incoming"), Path("./invoices/processed"), Path("./invoices/review")
)

@dataclass
class InvoiceData:
    filename: str; vendor: str; invoice_number: str; date: str
    amount: float; currency: str; category: str; confidence: float
    needs_review: bool; line_items: list

def extract_text_from_pdf(pdf_path: Path) -> str:
    try:
        return subprocess.run(
            ["pdftotext", "-layout", str(pdf_path), "-"],
            capture_output=True, text=True, timeout=30
        ).stdout
    except FileNotFoundError:
        import PyPDF2
        return "\n".join(p.extract_text() for p in PyPDF2.PdfReader(str(pdf_path)).pages)

def extract_invoice_data(text: str, filename: str) -> InvoiceData:
    prompt = f"""Extract invoice data from this text. Output ONLY valid JSON.

Invoice text:
---
{text[:3000]}
---

Extract: {{"vendor": "...", "invoice_number": "...", "date": "YYYY-MM-DD",
"amount": 0.00, "currency": "USD",
"category": "Legal Services|Office Supplies|Software|Professional Services|Other",
"line_items": [{{"description": "...", "amount": 0.00}}],
"confidence": 0.0 to 1.0}}"""

    response = requests.post(OLLAMA_URL, json={
        "model": MODEL, "prompt": prompt, "stream": False,
        "format": "json", "options": {"temperature": 0.1}
    })
    try:
        d = json.loads(response.json()["response"])
        conf = float(d.get("confidence", 0))
        return InvoiceData(filename=filename, vendor=d.get("vendor", "UNKNOWN"),
            invoice_number=d.get("invoice_number", ""), date=d.get("date", ""),
            amount=float(d.get("amount", 0)), currency=d.get("currency", "USD"),
            category=d.get("category", "Other"), confidence=conf,
            needs_review=conf < 0.7, line_items=d.get("line_items", []))
    except (json.JSONDecodeError, KeyError, ValueError):
        return InvoiceData(filename=filename, vendor="EXTRACTION_FAILED",
            invoice_number="", date="", amount=0.0, currency="USD",
            category="Other", confidence=0.0, needs_review=True, line_items=[])

def process_invoices():
    for d in [WATCH_DIR, PROCESSED_DIR, REVIEW_DIR]: d.mkdir(parents=True, exist_ok=True)
    pdfs = list(WATCH_DIR.glob("*.pdf"))
    if not pdfs: return print("Нет счетов для обработки.")

    for pdf_path in pdfs:
        text = extract_text_from_pdf(pdf_path)
        if not text.strip():
            pdf_path.rename(REVIEW_DIR / pdf_path.name); continue

        invoice = extract_invoice_data(text, pdf_path.name)
        dest = REVIEW_DIR if invoice.needs_review else PROCESSED_DIR
        pdf_path.rename(dest / pdf_path.name)

        with open("./invoices/extracted.jsonl", "a") as f:
            f.write(json.dumps(asdict(invoice)) + "\n")
        print(f"  {'Ревью' if invoice.needs_review else 'OK'}: "
              f"{invoice.vendor} ${invoice.amount:.2f} ({invoice.confidence:.0%})")

if __name__ == "__main__":
    process_invoices()
```

**Контракт:** 7 разделов, 50% аванс — не обсуждается.

{? if regional.business_entity_type ?}
В {= regional.country | fallback("твоей стране") =} используй свой {= regional.business_entity_type | fallback("тип юрлица") =}.
{? endif ?}

### Твоя очередь

1. Определи 3 проекта (1 ч). 2. Оцени один (30 мин). 3. Построй демо (4-8 ч). 4. Обратись к 5 клиентам (2 ч). 5. Подготовь контракт (30 мин).

---

## Урок 5: API Продукты

*"Преврати локальный LLM в генерирующий доход эндпоинт."*

**Время:** 2-4 нед | **Время:** 5-10 ч/неделю | **Маржа:** 70-90%

{@ insight stack_fit @}

{? if profile.gpu.exists ?}
С {= profile.gpu.model | fallback("GPU") =} — инференс локально для первых клиентов, затраты = 0.
{? endif ?}

```typescript
// src/api.ts — API классификации документов
import { Hono } from "hono";
import { cors } from "hono/cors";

const app = new Hono();

app.use("/v1/*", cors());

app.post("/v1/classify", async (c) => {
  const start = Date.now();
  const { text, domain = "auto" } = await c.req.json();

  if (!text) return c.json({ error: "Missing 'text' field." }, 400);
  if (text.length > 50000) return c.json({ error: "Text exceeds 50K char limit." }, 400);

  const prompt = `Classify this document. Domain: ${domain === "auto" ? "detect automatically" : domain}.
Document: ${text.slice(0, 5000)}
Respond with JSON: {"domain", "category", "confidence": 0-1, "subcategories": [],
"key_entities": [{"type", "value", "confidence"}], "summary": "one sentence"}`;

  try {
    let result;
    try {
      const resp = await fetch("http://127.0.0.1:11434/api/generate", {
        method: "POST",
        body: JSON.stringify({ model: "llama3.1:8b", prompt, stream: false, format: "json",
          options: { temperature: 0.1 } }),
        signal: AbortSignal.timeout(30000),
      });
      result = JSON.parse((await resp.json()).response);
    } catch {
      const resp = await fetch("https://api.anthropic.com/v1/messages", {
        method: "POST",
        headers: { "Content-Type": "application/json", "x-api-key": process.env.ANTHROPIC_API_KEY!,
          "anthropic-version": "2023-06-01" },
        body: JSON.stringify({ model: "claude-3-5-haiku-20241022", max_tokens: 1024,
          messages: [{ role: "user", content: prompt }] }),
      });
      result = JSON.parse((await resp.json()).content[0].text);
    }

    result.document_id = crypto.randomUUID();
    result.processing_time_ms = Date.now() - start;
    return c.json(result);
  } catch (error: any) {
    return c.json({ error: "Classification failed", message: error.message }, 500);
  }
});

export default app;
```

**Ценообразование:** Бесплатный $0 / Стартовый $29 / Профессиональный $99 / Корпоративный — по запросу.

### Твоя очередь

1. Определи нишу (1 ч). 2. Построй прототип (8-16 ч). 3. Добавь авторизацию и биллинг (4-8 ч). 4. Документация (2-4 ч). 5. Запусти (1 ч).

---

## Урок 6: Консалтинг и Фракционный CTO

*"Самый быстрый движок для старта."*

**Время до первого доллара:** 1 неделя | **Маржа:** 95%+

{@ insight stack_fit @}

```
$200/ч x 5 ч/нед = $4,000/мес | $300/ч x 5 = $6,000 | $400/ч x 5 = $8,000
```

{? if stack.primary ?}
Ты продаёшь не «{= stack.primary | fallback("программирование") =}». Ты продаёшь экспертизу, снижение рисков, суждение, лидерство.
{? else ?}
Ты продаёшь экспертизу, снижение рисков, суждение, лидерство.
{? endif ?}

**Ставки:** Rust $78-143/ч, AI/ML $120-500/ч.

{? if stack.contains("rust") ?}
Rust-экспертиза = одна из самых высокооплачиваемых ниш. Предложение крайне ограничено.
{? endif ?}

**Горячие ниши:** Локальный ИИ ($200-400/ч), Privacy-first ($200-350/ч), Rust миграция ($250-400/ч), ИИ-инструменты ($150-300/ч).

{@ mirror feed_predicts_engine @}

> **Прямой разговор:** Консалтинг финансирует другие движки. Доход месяцев 1-3 → микро-SaaS или контент.

### Твоя очередь

1. LinkedIn (30 мин). 2. Пост (1 ч). 3. 5 сообщений (1 ч). 4. Платформа (30 мин). 5. Ставка (15 мин).

---

## Урок 7: Open Source + Премиум

*"Строй публично, завоёвывай доверие, монетизируй верх пирамиды."*

**Время:** 4-12 нед | **Маржа:** 80-95%

{@ insight stack_fit @}

Логика: инструмент → open source → разработчики используют → компании платят за SSO/SLA/хостинг.

**Лицензии:** FSL или AGPL для защиты дохода. MIT — только если нет плана монетизации.

**Бенчмарки:** Plausible = $3.1M ARR, AGPL, 0 VC. Ghost = $10.4M, 24K клиентов.

```typescript
// license.ts — Гейтинг функций для open core
type Plan = "free" | "pro" | "team" | "enterprise";

const PLAN_CONFIG: Record<Plan, { maxProjects: number; features: Set<string> }> = {
  free:       { maxProjects: 3,        features: new Set(["core", "cli", "local_storage", "export"]) },
  pro:        { maxProjects: 20,       features: new Set(["core", "cli", "local_storage", "export",
                "dashboard", "cloud_sync", "analytics", "api_access", "integrations"]) },
  team:       { maxProjects: 100,      features: new Set(["core", "cli", "local_storage", "export",
                "dashboard", "cloud_sync", "analytics", "api_access", "integrations",
                "sso", "rbac", "audit_logs", "team_management"]) },
  enterprise: { maxProjects: Infinity, features: new Set(["core", "cli", "local_storage", "export",
                "dashboard", "cloud_sync", "analytics", "api_access", "integrations",
                "sso", "rbac", "audit_logs", "team_management",
                "on_premise", "sla", "dedicated_support", "invoice_billing"]) },
};

class LicenseManager {
  constructor(private plan: Plan = "free") {}
  hasFeature(feature: string): boolean {
    return PLAN_CONFIG[this.plan].features.has(feature);
  }
  requireFeature(feature: string): void {
    if (!this.hasFeature(feature)) {
      const requiredPlan = (Object.entries(PLAN_CONFIG) as [Plan, any][])
        .find(([_, config]) => config.features.has(feature))?.[0] || "enterprise";
      throw new Error(`"${feature}" requires ${requiredPlan} plan. Upgrade at https://yourapp.com/pricing`);
    }
  }
}
```

### Твоя очередь

1. Определи проект (1 ч). 2. Выбери лицензию (15 мин). 3. Построй и выпусти (1-4 нед). 4. Тарифы (1 ч). 5. Запусти (1 день).

---

## Урок 8: Продукты Данных и Разведка

*"Информация ценна только когда обработана и доставлена в контексте."*

**Время:** 4-8 нед | **Маржа:** 85-95%

{@ insight stack_fit @}

{? if settings.has_llm ?}
С {= settings.llm_model | fallback("моделью") =} — пайплайн на нулевых предельных затратах.
{? endif ?}

```python
#!/usr/bin/env python3
"""intelligence_pipeline.py — Генерация еженедельного разведывательного отчёта."""
import requests, json, time, feedparser
from datetime import datetime, timedelta
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

def fetch_items(feeds, hn_min_score=50):
    items = []
    cutoff = datetime.now() - timedelta(days=7)
    for feed_cfg in feeds:
        try:
            for entry in feedparser.parse(feed_cfg["url"]).entries[:20]:
                items.append({"title": entry.get("title", ""), "url": entry.get("link", ""),
                    "source": feed_cfg["name"], "content": entry.get("summary", "")[:2000]})
        except Exception as e:
            print(f"  Предупреждение: {feed_cfg['name']}: {e}")

    week_ago = int(cutoff.timestamp())
    resp = requests.get(f"https://hn.algolia.com/api/v1/search?tags=story"
        f"&numericFilters=points>{hn_min_score},created_at_i>{week_ago}&hitsPerPage=30")
    for hit in resp.json().get("hits", []):
        items.append({"title": hit.get("title", ""), "source": "Hacker News",
            "url": hit.get("url", f"https://news.ycombinator.com/item?id={hit['objectID']}"),
            "content": hit.get("title", "")})

    seen = set()
    return [i for i in items if i["title"][:50].lower() not in seen and not seen.add(i["title"][:50].lower())]

def score_items(items, niche, criteria):
    scored = []
    for item in items:
        prompt = f"""Score this item for a {niche} newsletter. Criteria: {criteria}
Title: {item['title']} | Source: {item['source']} | Content: {item['content'][:1500]}
Output JSON: {{"relevance_score": 0-10, "category": "Breaking|Tool|Research|Tutorial|Industry|Security",
"summary": "2-3 sentences", "actionable_insight": "what to DO", "key_takeaway": "one sentence"}}"""

        try:
            resp = requests.post(OLLAMA_URL, json={"model": MODEL, "prompt": prompt,
                "stream": False, "format": "json", "options": {"temperature": 0.2}}, timeout=60)
            data = json.loads(resp.json()["response"])
            if data.get("relevance_score", 0) >= 5.0:
                item.update(data)
                scored.append(item)
        except Exception:
            continue
        time.sleep(0.5)
    return sorted(scored, key=lambda x: x.get("relevance_score", 0), reverse=True)

def generate_report(items, niche, issue):
    date_str = datetime.now().strftime('%B %d, %Y')
    report = f"# {niche} Intelligence — Issue #{issue}\n**Week of {date_str}**\n\n---\n\n"
    if items:
        top = items[0]
        report += f"## Топ-Сигнал: {top['title']}\n\n{top.get('summary','')}\n\n"
        report += f"**Почему важно:** {top.get('key_takeaway','')}\n\n"
        report += f"**Действие:** {top.get('actionable_insight','')}\n\n[Подробнее]({top['url']})\n\n---\n\n"
    for item in items[1:12]:
        report += f"### [{item['title']}]({item['url']})\n"
        report += f"*{item['source']} | {item.get('category','')} | Оценка: {item.get('relevance_score',0)}/10*\n\n"
        report += f"{item.get('summary','')}\n\n> **Действие:** {item.get('actionable_insight','')}\n\n"
    report += f"\n---\n*{len(items)} элементов проанализировано. Сгенерировано локально {date_str}.*\n"
    return report

if __name__ == "__main__":
    NICHE = "Rust Ecosystem"
    CRITERIA = "High: new releases, critical crate updates, security vulns. Medium: blog posts, new crates."
    FEEDS = [
        {"name": "This Week in Rust", "url": "https://this-week-in-rust.org/rss.xml"},
        {"name": "Rust Blog", "url": "https://blog.rust-lang.org/feed.xml"},
    ]
    items = fetch_items(FEEDS)
    scored = score_items(items, NICHE, CRITERIA)
    report = generate_report(scored, NICHE, issue=1)
    output = Path(f"./reports/report-{datetime.now().strftime('%Y-%m-%d')}.md")
    output.parent.mkdir(exist_ok=True)
    output.write_text(report)
    print(f"Отчёт: {output}")
```

**Прогноз:** Месяц 1 = $150, Месяц 3 = $750, Месяц 6 = $2,250, Месяц 12 = $6,000.

{@ temporal revenue_benchmarks @}

**Бенчмарки:** Fireship ~$550K+/год, Wes Bos $10M+, Josh Comeau $550K за неделю.

### Твоя очередь

1. Ниша (30 мин). 2. Источники (1 ч). 3. Запусти пайплайн (2 ч). 4. Первый отчёт (2-4 ч). 5. Отправь 10 людям (30 мин).

---

## Выбор Движка: Выбери Два

{@ insight engine_ranking @}

**Матрица:** Навыки (1-5), Время (1-5), Скорость (1-5), Масштаб (1-5).

{? if dna.identity_summary ?}
На основе профиля — {= dna.identity_summary | fallback("навыки") =}.
{? endif ?}

{? if computed.experience_years < 3 ?}
> Начни с Цифровых Продуктов или Контента.
{? elif computed.experience_years < 8 ?}
> Консалтинг + Микро-SaaS или API Продукты.
{? else ?}
> Open Source + Премиум, Продукты Данных или Премиум Консалтинг.
{? endif ?}

{? if stack.contains("react") ?}
> React: UI библиотеки, Next.js шаблоны.
{? endif ?}
{? if stack.contains("python") ?}
> Python: Пайплайны данных, ML утилиты, автоматизация.
{? endif ?}
{? if stack.contains("rust") ?}
> Rust: Премиум ставки, CLI инструменты, WebAssembly.
{? endif ?}
{? if stack.contains("typescript") ?}
> TypeScript: npm пакеты, VS Code расширения, full-stack SaaS.
{? endif ?}

**Правило 40%:** Не > 40% дохода от одной платформы.

### Анти-Паттерны

1. Не 3+ движков. 2. Не два медленных. 3. Не два в одной категории. 4. Не пропускай математику.

---

## Интеграция 4DA

{@ mirror feed_predicts_engine @}

> 4DA обнаруживает возможность → STREETS даёт плейбук → Движок превращает сигнал в доход.

---

## Модуль R: Завершён

1. Работающий Движок 1. 2. План для Движка 2. 3. Развёрнутый код. 4. Матрица решений. 5. Математика дохода.

{? if progress.completed_modules ?}
Завершено {= progress.completed_count | fallback("0") =}/{= progress.total_count | fallback("7") =} ({= progress.completed_modules | fallback("ещё нет") =}). Модуль R — поворотный момент.
{? endif ?}

### Что Дальше: Модуль E — Плейбук Исполнения

Запуск, ценообразование, первые 10 клиентов, метрики, когда поворачивать.

---

*Твоя машина. Твои правила. Твой доход.*
