# الوحدة T: الأتمتة التكتيكية

**دورة STREETS لدخل المطورين — وحدة مدفوعة**
*الأسبوعان 12-13 | 6 دروس | المُخرج النهائي: خط أنابيب مؤتمت واحد يولّد قيمة*

> "نماذج اللغة الكبيرة، الوكلاء، MCP، ومهام cron كمضاعفات قوة."

---

لديك محركات إيرادات تعمل. لديك عملاء. لديك عمليات ناجحة. وأنت تقضي 60-70% من وقتك في تكرار نفس المهام مراراً: معالجة المدخلات، تنسيق المخرجات، مراقبة الأنظمة، إرسال التحديثات، مراجعة قوائم الانتظار.

هذا الوقت هو أغلى مواردك، وأنت تحرقه في مهام يمكن لخادم VPS بتكلفة {= regional.currency_symbol | fallback("$") =}5/شهر أن يتولاها.

{@ insight hardware_benchmark @}

هذه الوحدة تدور حول إزالتك بشكل منهجي من الحلقة — ليس بالكامل (هذا فخ سنغطيه في الدرس 5)، ولكن من الـ 80% من العمل الذي لا يتطلب حكمك. النتيجة: تيارات دخلك تنتج إيرادات أثناء نومك، أثناء عملك النهاري، أثناء بنائك للشيء التالي.

بنهاية هذين الأسبوعين، ستكون قد حققت:

- فهم واضح للمستويات الأربعة من الأتمتة وأين تقف اليوم
- مهام cron وأتمتة مجدولة تعمل على بنيتك التحتية
- خط أنابيب واحد على الأقل مدعوم بنموذج لغوي كبير يعالج المدخلات دون تدخلك
- فهم للأنظمة القائمة على الوكلاء ومتى تكون مجدية اقتصادياً
- إطار عمل بإشراف بشري حتى لا تدمر الأتمتة سمعتك
- خط أنابيب واحد مكتمل ومنشور يولّد قيمة دون تدخلك الفعلي

{? if stack.primary ?}
لغتك البرمجية الأساسية هي {= stack.primary | fallback("لغتك الأساسية") =}، لذا فإن أمثلة الأتمتة القادمة ستكون أكثر قابلية للتطبيق المباشر عند تكييفها مع هذا النظام البيئي. معظم الأمثلة تستخدم Python لقابلية النقل، لكن الأنماط تنطبق على أي لغة.
{? endif ?}

هذه هي الوحدة الأكثر كثافة في الشيفرة البرمجية في الدورة بأكملها. ما لا يقل عن نصف ما يلي هو كود قابل للتشغيل. انسخه، عدّله، انشره.

لنبدأ بالأتمتة.

---

## الدرس 1: هرم الأتمتة

*"معظم المطورين يؤتمتون عند المستوى 1. المال يكمن في المستوى 3."*

### المستويات الأربعة

كل أتمتة في مجموعة دخلك تقع في مكان ما على هذا الهرم:

```
┌───────────────────────────────┐
│  المستوى 4: الوكلاء المستقلون │  ← يتخذ القرارات عنك
│  (الذكاء الاصطناعي يقرر ويتصرف)│
├───────────────────────────────┤
│  المستوى 3: الأنابيب الذكية   │  ← المال هنا
│  (مدعومة بنماذج لغوية كبيرة) │
├───────────────────────────────┤
│  المستوى 2: الأتمتة المجدولة  │  ← معظم المطورين يتوقفون هنا
│  (cron + سكربتات)            │
├───────────────────────────────┤
│  المستوى 1: يدوي مع قوالب    │  ← حيث يقف معظم المطورين
│  (نسخ ولصق)                  │
└───────────────────────────────┘
```

لنكن محددين حول ما يبدو عليه كل مستوى عملياً.

### المستوى 1: يدوي مع قوالب

أنت تقوم بالعمل، لكن لديك قوائم مراجعة وقوالب ومقتطفات لتسريع الأمور.

**أمثلة:**
- تكتب مقال مدونة باستخدام قالب markdown مع بيانات وصفية معبأة مسبقاً
- تصدر فواتير للعملاء بنسخ فاتورة الشهر الماضي وتغيير الأرقام
- ترد على رسائل الدعم الإلكترونية باستخدام ردود محفوظة
- تنشر المحتوى بتشغيل أمر نشر يدوياً

**تكلفة الوقت:** 100% من وقتك لكل وحدة إنتاج.
**معدل الأخطاء:** متوسط — أنت بشر، ترتكب أخطاء عندما تكون متعباً.
**سقف التوسع:** أنت. ساعاتك. هذا كل ما في الأمر.

معظم المطورين يعيشون هنا ولا يدركون حتى أن هناك هرماً فوقهم.

### المستوى 2: الأتمتة المجدولة

السكربتات تعمل وفق جداول زمنية. كتبت المنطق مرة واحدة. ينفذ بدونك.

**أمثلة:**
- مهمة cron تتحقق من موجز RSS الخاص بك وتنشر مقالات جديدة على وسائل التواصل الاجتماعي
- GitHub Action يبني وينشر موقعك كل صباح في الساعة 6
- سكربت يعمل كل ساعة للتحقق من أسعار المنافسين وتسجيل التغييرات
- نسخة احتياطية يومية لقاعدة البيانات تعمل في الساعة 3 صباحاً

**تكلفة الوقت:** صفر مستمر (بعد الإعداد الأولي من 1-4 ساعات).
**معدل الأخطاء:** منخفض — حتمي، نفس المنطق في كل مرة.
**سقف التوسع:** بقدر ما تستطيع آلتك جدولته. المئات.

هنا يصل معظم المطورين التقنيين. إنه مريح. لكن له حد صارم: يمكنه فقط التعامل مع المهام ذات المنطق الحتمي. إذا تطلبت المهمة حكماً بشرياً، فأنت عالق.

### المستوى 3: الأنابيب الذكية

السكربتات تعمل وفق جداول زمنية، لكنها تتضمن نموذجاً لغوياً كبيراً يتعامل مع قرارات الحكم.

**أمثلة:**
- يتم استيعاب موجزات RSS، النموذج اللغوي يلخص كل مقال، يصيغ مسودة نشرة بريدية، تراجعها لمدة 10 دقائق وتضغط إرسال
- رسائل ملاحظات العملاء تُصنف حسب المشاعر والإلحاح، مسودات الردود تُوضع في قائمة انتظار لموافقتك
- إعلانات وظائف جديدة في تخصصك تُجمع، النموذج اللغوي يقيّم الصلة، تحصل على ملخص يومي لـ 5 فرص بدلاً من تصفح 200 إعلان
- مقالات مدونات المنافسين تُراقب، النموذج اللغوي يستخرج تغييرات المنتجات الرئيسية، تحصل على تقرير استخبارات تنافسية أسبوعي

**تكلفة الوقت:** 10-20% من الوقت اليدوي. تراجع وتوافق بدلاً من أن تُنشئ.
**معدل الأخطاء:** منخفض لمهام التصنيف، متوسط للتوليد (لهذا تراجع).
**سقف التوسع:** آلاف العناصر يومياً. عنق الزجاجة هو تكلفة الواجهة البرمجية، لا وقتك.

**هنا يكمن المال.** المستوى 3 يتيح لشخص واحد تشغيل تيارات دخل تتطلب عادةً فريقاً من 3-5 أشخاص.

### المستوى 4: الوكلاء المستقلون

أنظمة ذكاء اصطناعي تراقب وتقرر وتتصرف دون تدخلك.

**أمثلة:**
- وكيل يراقب مقاييس SaaS الخاص بك، يكتشف انخفاضاً في التسجيلات، يختبر تغيير تسعير A/B، ويتراجع إذا لم ينجح
- وكيل دعم يتعامل مع أسئلة العملاء من المستوى الأول بالكامل بشكل مستقل، ولا يصعّد إليك إلا للمسائل المعقدة
- وكيل محتوى يحدد المواضيع الرائجة، يولّد المسودات، يجدول النشر، ويراقب الأداء

**تكلفة الوقت:** قريبة من الصفر للحالات المُعالجة. تراجع المقاييس، لا الإجراءات الفردية.
**معدل الأخطاء:** يعتمد كلياً على حواجز الأمان. بدونها: مرتفع. مع حواجز جيدة: منخفض بشكل مفاجئ للنطاقات الضيقة.
**سقف التوسع:** غير محدود فعلياً للمهام ضمن نطاق الوكيل.

المستوى 4 حقيقي وقابل للتحقيق، لكنه ليس نقطة البداية. وكما سنغطي في الدرس 5، الوكلاء المستقلون الذين يتعاملون مع العملاء مباشرة خطرون على سمعتك إذا كانوا مُنفذين بشكل سيء.

> **حديث صريح:** إذا كنت في المستوى 1 الآن، لا تحاول القفز إلى المستوى 4. ستقضي أسابيع في بناء "وكيل مستقل" ينهار في الإنتاج ويضر بثقة العملاء. اصعد الهرم مستوى واحداً في كل مرة. المستوى 2 يحتاج ظهيرة واحدة من العمل. المستوى 3 مشروع عطلة نهاية أسبوع. المستوى 4 يأتي بعد أن يكون المستوى 3 يعمل بشكل موثوق لمدة شهر.

### تقييم ذاتي: أين أنت؟

لكل تيار من تيارات دخلك، قيّم نفسك بصدق:

| تيار الدخل | المستوى الحالي | الساعات/الأسبوع المُنفقة | يمكن أتمتته إلى |
|------------|---------------|------------------------|----------------|
| [مثلاً، النشرة البريدية] | [1-4] | [X] ساعة | [المستوى المستهدف] |
| [مثلاً، معالجة العملاء] | [1-4] | [X] ساعة | [المستوى المستهدف] |
| [مثلاً، وسائل التواصل الاجتماعي] | [1-4] | [X] ساعة | [المستوى المستهدف] |
| [مثلاً، الدعم] | [1-4] | [X] ساعة | [المستوى المستهدف] |

العمود الأهم هو "الساعات/الأسبوع المُنفقة." التيار ذو أعلى الساعات وأدنى مستوى هو هدف الأتمتة الأول. هذا هو الذي يحقق أعلى عائد على الاستثمار.

### اقتصاديات كل مستوى

لنفترض أن لديك تيار دخل يأخذ 10 ساعات/أسبوع من وقتك ويولّد {= regional.currency_symbol | fallback("$") =}2,000/شهر:

| المستوى | وقتك | معدلك الفعلي | تكلفة الأتمتة |
|---------|------|-------------|--------------|
| المستوى 1 | 10 ساعات/أسبوع | $50/ساعة | $0 |
| المستوى 2 | 3 ساعات/أسبوع | $167/ساعة | $5/شهر (VPS) |
| المستوى 3 | 1 ساعة/أسبوع | $500/ساعة | $30-50/شهر (API) |
| المستوى 4 | 0.5 ساعة/أسبوع | $1,000/ساعة | $50-100/شهر (API + حوسبة) |

الانتقال من المستوى 1 إلى المستوى 3 لا يغير إيراداتك. بل يغير معدلك الفعلي بالساعة من $50 إلى $500. وتلك الـ 9 ساعات المُحررة؟ تذهب لبناء تيار الدخل التالي أو تحسين الحالي.

> **خطأ شائع:** أتمتة تيار الدخل الأقل إيراداً أولاً لأنه "أسهل." لا. أتمِت التيار الذي يستهلك أكبر عدد من الساعات نسبة إلى إيراداته. هنا يكمن العائد على الاستثمار.

### دورك

1. املأ جدول التقييم الذاتي أعلاه لكل تيار دخل (أو تيار مخطط) لديك.
2. حدد هدف الأتمتة الأعلى عائداً على الاستثمار: التيار ذو أكبر عدد ساعات وأدنى مستوى أتمتة.
3. اكتب أكثر 3 مهام استهلاكاً للوقت في ذلك التيار. ستؤتمت الأولى في الدرس 2.

---

## الدرس 2: من المستوى 1 إلى 2 — الأتمتة المجدولة

*"cron من عام 1975. لا يزال يعمل. استخدمه."*

### أساسيات مهام cron

{? if computed.os_family == "windows" ?}
أنت على Windows، لذا cron ليس أصلياً في نظامك. لديك خياران: استخدام WSL (نظام Windows الفرعي لـ Linux) للحصول على cron حقيقي، أو استخدام Windows Task Scheduler (مُغطى أدناه). يُوصى بـ WSL إذا كنت مرتاحاً معه — جميع أمثلة cron في هذا الدرس تعمل مباشرة في WSL. إذا كنت تفضل Windows الأصلي، انتقل إلى قسم Task Scheduler بعد هذا.
{? endif ?}

نعم، حتى في 2026، cron هو الملك للمهام المجدولة. إنه موثوق، موجود في كل مكان، ولا يتطلب حساب سحابي، أو اشتراك SaaS، أو مخطط YAML عليك البحث عنه في Google كل مرة.

**صياغة cron في 30 ثانية:**

```
┌───────── الدقيقة (0-59)
│ ┌───────── الساعة (0-23)
│ │ ┌───────── يوم الشهر (1-31)
│ │ │ ┌───────── الشهر (1-12)
│ │ │ │ ┌───────── يوم الأسبوع (0-7، 0 و 7 = الأحد)
│ │ │ │ │
* * * * *  أمر
```

**جداول شائعة:**

```bash
# كل ساعة
0 * * * *  /path/to/script.sh

# كل يوم في الساعة 6 صباحاً
0 6 * * *  /path/to/script.sh

# كل يوم اثنين في الساعة 9 صباحاً
0 9 * * 1  /path/to/script.sh

# كل 15 دقيقة
*/15 * * * *  /path/to/script.sh

# أول يوم من كل شهر عند منتصف الليل
0 0 1 * *  /path/to/script.sh
```

**إعداد مهمة cron:**

```bash
# تحرير crontab الخاص بك
crontab -e

# عرض مهام cron الحالية
crontab -l

# حاسم: دائماً اضبط متغيرات البيئة في الأعلى
# cron يعمل ببيئة مُصغرة — PATH قد لا يتضمن أدواتك
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# سجّل المخرجات حتى تتمكن من تصحيح الأخطاء
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **خطأ شائع:** كتابة سكربت يعمل بشكل مثالي عند تشغيله يدوياً، ثم يفشل بصمت في cron لأن cron لا يُحمّل `.bashrc` أو `.zshrc` الخاص بك. استخدم دائماً المسارات المطلقة في سكربتات cron. اضبط دائماً `PATH` في أعلى crontab. أعد توجيه المخرجات دائماً إلى ملف سجل.

### المُجدولات السحابية عندما لا يكفي cron

إذا لم يكن جهازك يعمل على مدار الساعة، أو كنت بحاجة إلى شيء أكثر متانة، استخدم مُجدولاً سحابياً:

**GitHub Actions (مجاني للمستودعات العامة، 2,000 دقيقة/شهر على الخاصة):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # كل يوم في الساعة 6 صباحاً UTC
    - cron: '0 6 * * *'
  # السماح بالتشغيل اليدوي للاختبار
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

**Vercel Cron (مجاني على خطة Hobby، مرة واحدة يومياً؛ خطة Pro: غير محدود):**

```typescript
// api/cron/daily-report.ts
// نقطة نهاية Vercel cron — اضبط الجدول في vercel.json

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // تحقق من أن Vercel هي من تستدعي، وليس طلب HTTP عشوائي
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // منطق الأتمتة الخاص بك هنا
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

### أتمتة حقيقية يمكنك بناؤها الآن

إليك خمس أتمتة يمكنك تنفيذها اليوم. كل واحدة تستغرق 30-60 دقيقة وتزيل ساعات من العمل اليدوي الأسبوعي.

#### الأتمتة 1: نشر المحتوى تلقائياً وفق جدول

تكتب مقالات المدونة مسبقاً. هذا السكربت ينشرها في الوقت المحدد.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — نشر مقالات markdown في تاريخها المحدد.
شغّل يومياً عبر cron: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# نقطة نهاية واجهة CMS البرمجية الخاصة بك (Hashnode، Dev.to، Ghost، إلخ.)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """تحميل قائمة أسماء الملفات المنشورة مسبقاً."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """حفظ قائمة أسماء الملفات المنشورة."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """استخراج البيانات الوصفية بأسلوب YAML من ملف markdown."""
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
    """التحقق مما إذا كان يجب نشر المقال اليوم."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """نشر مقال على واجهة CMS البرمجية الخاصة بك."""
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
        print(f"  تم النشر: {metadata.get('title')}")
        return True
    except requests.RequestException as e:
        print(f"  فشل: {metadata.get('title')} — {e}")
        return False

def main():
    published = load_published()
    posts = glob.glob(os.path.join(CONTENT_DIR, "*.md"))

    print(f"التحقق من {len(posts)} مقال...")

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
    print(f"إجمالي المنشورات: {len(published)}")

if __name__ == "__main__":
    main()
```

**مقالات markdown الخاصة بك تبدو هكذا:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

محتوى مقالك هنا...
```

اكتب المقالات عندما يأتيك الإلهام. حدد التاريخ. السكربت يتولى الباقي.

#### الأتمتة 2: النشر التلقائي على وسائل التواصل الاجتماعي عند نشر محتوى جديد

عندما تنشر مدونتك شيئاً جديداً، هذا ينشر على Twitter/X و Bluesky تلقائياً.

```python
#!/usr/bin/env python3
"""
social_poster.py — النشر على منصات التواصل الاجتماعي عند نشر محتوى جديد.
شغّل كل 30 دقيقة: */30 * * * * python3 /path/to/social_poster.py
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
    """تحليل موجز RSS وإرجاع قائمة العناصر."""
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
    """النشر على Bluesky عبر بروتوكول AT."""
    # الخطوة 1: إنشاء جلسة
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

    # الخطوة 2: إنشاء منشور
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
    print(f"  تم النشر على Bluesky: {text[:60]}...")

def main():
    posted = load_posted()
    items = get_rss_items(FEED_URL)

    for item in items:
        if item["id"] in posted:
            continue

        # تنسيق المنشور الاجتماعي
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky لديه حد 300 حرف
        if len(text) > 300:
            text = f"{item['title'][:240]}...\n\n{item['link']}"

        try:
            post_to_bluesky(text)
            posted.add(item["id"])
        except Exception as e:
            print(f"  فشل النشر: {e}")

    save_posted(posted)

if __name__ == "__main__":
    main()
```

التكلفة: $0. يعمل على جهازك أو GitHub Action مجاني.

#### الأتمتة 3: مراقب أسعار المنافسين

اعرف فوراً عندما يغير منافس أسعاره. لا مزيد من التحقق اليدوي كل أسبوع.

```python
#!/usr/bin/env python3
"""
price_monitor.py — مراقبة صفحات تسعير المنافسين للتغييرات.
شغّل كل 6 ساعات: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # أو Discord، بريد إلكتروني، إلخ.

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # لمراقبة الصفحة كاملة؛ استخدم المحدد لعناصر محددة
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """جلب صفحة وإرجاع تجزئة محتواها ومقتطف نصي."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # أخذ أول 500 حرف من النص المرئي للسياق
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
    """إرسال تنبيه عبر webhook لـ Slack (بدّل إلى Discord أو بريد إلكتروني، إلخ.)."""
    if not ALERT_WEBHOOK:
        print(f"تنبيه (لا يوجد webhook مُعد): {message}")
        return

    requests.post(ALERT_WEBHOOK, json={"text": message}, timeout=10)

def main():
    for competitor in COMPETITORS:
        name = competitor["name"]
        url = competitor["url"]

        try:
            current_hash, excerpt = get_page_hash(url)
        except Exception as e:
            print(f"  فشل جلب {name}: {e}")
            continue

        state = load_state(name)
        previous_hash = state.get("hash", "")

        if previous_hash and current_hash != previous_hash:
            alert_msg = (
                f"تم اكتشاف تغيير في الأسعار: {name}\n"
                f"الرابط: {url}\n"
                f"تغيّر في: {datetime.utcnow().isoformat()}Z\n"
                f"التجزئة السابقة: {previous_hash[:12]}...\n"
                f"التجزئة الجديدة: {current_hash[:12]}...\n"
                f"تحقق منه يدوياً."
            )
            send_alert(alert_msg)
            print(f"  تغيير: {name}")
        else:
            print(f"  لا تغيير: {name}")

        save_state(name, {
            "hash": current_hash,
            "last_checked": datetime.utcnow().isoformat() + "Z",
            "url": url,
            "excerpt": excerpt[:200]
        })

if __name__ == "__main__":
    main()
```

#### الأتمتة 4: تقرير الإيرادات الأسبوعي

كل صباح اثنين، يُنشئ هذا تقريراً من بيانات إيراداتك ويرسله إليك بالبريد الإلكتروني.

```python
#!/usr/bin/env python3
"""
weekly_report.py — إنشاء تقرير إيرادات أسبوعي من جدول/قاعدة بيانات التتبع.
شغّل أيام الاثنين في الساعة 7 صباحاً: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """إنشاء جدول الإيرادات إذا لم يكن موجوداً."""
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
    """إنشاء تقرير أسبوعي بنص عادي."""
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
    report.append(f"تقرير الإيرادات الأسبوعي")
    report.append(f"الفترة: {week_ago.strftime('%Y-%m-%d')} إلى {today.strftime('%Y-%m-%d')}")
    report.append(f"تم الإنشاء: {today.strftime('%Y-%m-%d %H:%M')}")
    report.append("=" * 50)
    report.append("")

    for stream, data in sorted(streams.items()):
        net = data["income"] - data["expense"]
        report.append(f"  {stream}")
        report.append(f"    الدخل:     ${data['income']:>10,.2f}")
        report.append(f"    المصاريف:  ${data['expense']:>10,.2f}")
        report.append(f"    الصافي:    ${net:>10,.2f}")
        report.append("")

    report.append("=" * 50)
    report.append(f"  إجمالي الدخل:     ${total_income:>10,.2f}")
    report.append(f"  إجمالي المصاريف:  ${total_expenses:>10,.2f}")
    report.append(f"  صافي الربح:       ${total_income - total_expenses:>10,.2f}")

    if total_expenses > 0:
        roi = (total_income - total_expenses) / total_expenses
        report.append(f"  العائد على الاستثمار: {roi:>10.1f}x")

    return "\n".join(report)

def send_email(subject: str, body: str):
    """إرسال التقرير عبر البريد الإلكتروني."""
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
            f"تقرير الإيرادات الأسبوعي — {datetime.now().strftime('%Y-%m-%d')}",
            report
        )
        print("\nتم إرسال التقرير بالبريد الإلكتروني.")
    conn.close()

if __name__ == "__main__":
    main()
```

#### الأتمتة 5: النسخ الاحتياطي التلقائي لبيانات العملاء

لا تفقد مخرجات العملاء أبداً. يعمل هذا ليلاً ويحتفظ بنسخ احتياطية لمدة 30 يوماً.

```bash
#!/bin/bash
# backup_client_data.sh — نسخة احتياطية ليلية لبيانات مشاريع العملاء.
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# إنشاء نسخة احتياطية مضغوطة
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# حذف النسخ الاحتياطية الأقدم من فترة الاحتفاظ
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# تسجيل
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) اكتملت النسخة الاحتياطية: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# اختياري: مزامنة إلى موقع ثانٍ (قرص خارجي، جهاز آخر)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### مؤقتات systemd لمزيد من التحكم

إذا كنت بحاجة إلى أكثر مما يوفره cron — مثل ترتيب التبعيات، حدود الموارد، أو إعادة المحاولة التلقائية — استخدم مؤقتات systemd:

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
# إعادة التشغيل عند الفشل مع تأخير تصاعدي
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
# إذا كان الجهاز مطفأ في الساعة 6 صباحاً، شغّل عند عودته
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# تفعيل وبدء المؤقت
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# التحقق من الحالة
systemctl list-timers --all | grep income

# عرض السجلات
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### بديل Windows Task Scheduler

إذا كنت لا تستخدم WSL، فإن Windows Task Scheduler يتولى نفس المهمة. استخدم `schtasks` من سطر الأوامر أو واجهة Task Scheduler الرسومية (`taskschd.msc`). الفرق الرئيسي: cron يستخدم تعبيراً واحداً، Task Scheduler يستخدم حقولاً منفصلة للمشغلات والإجراءات والشروط. كل مثال cron في هذا الدرس يُترجم مباشرة — جدول سكربتات Python الخاصة بك بنفس الطريقة، فقط من خلال واجهة مختلفة.
{? endif ?}

### دورك

1. اختر أبسط أتمتة من هذا الدرس تنطبق على تيار دخلك.
2. نفّذها. ليس "خطط لتنفيذها." اكتب الكود، اختبره، جدوِله.
3. أعد تسجيل المخرجات حتى تتمكن من التحقق من أنها تعمل. تحقق من السجلات كل صباح لمدة 3 أيام.
4. بمجرد أن تستقر، توقف عن التحقق اليومي. تحقق أسبوعياً. هذه هي الأتمتة.

**الحد الأدنى:** مهمة cron واحدة تعمل بشكل موثوق بحلول نهاية اليوم.

---

## الدرس 3: من المستوى 2 إلى 3 — أنابيب مدعومة بنماذج لغوية كبيرة

*"أضف الذكاء إلى أتمتتك. هنا يبدأ شخص واحد في الظهور كفريق."*

### النمط

كل خط أنابيب مدعوم بنموذج لغوي كبير يتبع نفس الشكل:

```
مصادر الإدخال → الاستيعاب → المعالجة بالنموذج اللغوي → تنسيق المخرجات → التسليم (أو وضعها في قائمة المراجعة)
```

السحر يكمن في خطوة "المعالجة بالنموذج اللغوي." بدلاً من كتابة قواعد حتمية لكل حالة ممكنة، تصف ما تريده بلغة طبيعية، والنموذج اللغوي يتعامل مع قرارات الحكم.

### متى تستخدم النموذج المحلي مقابل الواجهة البرمجية

{? if settings.has_llm ?}
لديك {= settings.llm_provider | fallback("مزود نموذج لغوي") =} مُعد مع {= settings.llm_model | fallback("نموذجك اللغوي") =}. هذا يعني أنه يمكنك البدء في بناء أنابيب ذكية فوراً. القرار أدناه يساعدك في اختيار متى تستخدم إعدادك المحلي مقابل واجهة برمجية لكل خط أنابيب.
{? else ?}
لم تقم بإعداد نموذج لغوي بعد. الأنابيب في هذا الدرس تعمل مع النماذج المحلية (Ollama) والواجهات البرمجية السحابية. أعد واحدة على الأقل قبل بناء خط أنابيبك الأول — Ollama مجاني ويستغرق 10 دقائق للتثبيت.
{? endif ?}

هذا القرار له تأثير مباشر على هوامش ربحك:

| العامل | محلي (Ollama) | واجهة برمجية (Claude، GPT) |
|--------|--------------|--------------------------|
| **التكلفة لكل 1 مليون رمز** | ~$0.003 (كهرباء) | $0.15 - $15.00 |
| **السرعة (رمز/ثانية)** | 20-60 (8B على GPU متوسط) | 50-100+ |
| **الجودة (8B محلي مقابل API)** | جيد للتصنيف والاستخراج | أفضل للتوليد والاستدلال |
| **الخصوصية** | البيانات لا تغادر جهازك أبداً | البيانات تذهب للمزود |
| **وقت التشغيل** | يعتمد على جهازك | 99.9%+ |
| **قدرة الدُفعات** | محدودة بذاكرة GPU | محدودة بحدود المعدل والميزانية |

{? if profile.gpu.exists ?}
مع {= profile.gpu.model | fallback("GPU الخاص بك") =} على جهازك، الاستدلال المحلي خيار قوي. السرعة وحجم النموذج الذي يمكنك تشغيله يعتمدان على VRAM — تحقق مما يناسب قبل الالتزام بخط أنابيب محلي فقط.
{? if computed.has_nvidia ?}
بطاقات NVIDIA تحصل على أفضل أداء مع Ollama بفضل تسريع CUDA. يجب أن تتمكن من تشغيل نماذج بحجم 7-8B بارامتر بشكل مريح، وربما أكبر حسب {= profile.gpu.vram | fallback("VRAM المتاحة") =}.
{? endif ?}
{? else ?}
بدون GPU مخصص، سيكون الاستدلال المحلي أبطأ (CPU فقط). لا يزال يعمل لمهام الدُفعات الصغيرة ومهام التصنيف، لكن لأي شيء حساس للوقت أو كبير الحجم، نموذج الواجهة البرمجية سيكون أكثر عملية.
{? endif ?}

**قواعد عامة:**
- **حجم كبير، معيار جودة أقل** (تصنيف، استخراج، وسم) → محلي
- **حجم منخفض، حرج الجودة** (محتوى موجه للعملاء، تحليل معقد) → واجهة برمجية
- **بيانات حساسة** (معلومات العملاء، بيانات ملكية) → محلي، دائماً
- **أكثر من 10,000 عنصر/شهر** → المحلي يوفر مالاً حقيقياً

**مقارنة التكلفة الشهرية لخط أنابيب نموذجي:**

```
معالجة 5,000 عنصر/شهر، ~500 رمز لكل عنصر:

محلي (Ollama، llama3.1:8b):
  2,500,000 رمز × $0.003/1M = $0.0075/شهر
  مجاني عملياً.

واجهة برمجية (GPT-4o-mini):
  2,500,000 رمز إدخال × $0.15/1M = $0.375
  2,500,000 رمز إخراج × $0.60/1M = $1.50
  المجموع: ~$1.88/شهر
  رخيص، لكن 250 ضعف المحلي.

واجهة برمجية (Claude 3.5 Sonnet):
  2,500,000 رمز إدخال × $3.00/1M = $7.50
  2,500,000 رمز إخراج × $15.00/1M = $37.50
  المجموع: ~$45/شهر
  الجودة ممتازة، لكن 6,000 ضعف المحلي.
```

لأنابيب التصنيف والاستخراج، فرق الجودة بين نموذج محلي 8B مع موجه جيد ونموذج واجهة برمجية متقدم غالباً ما يكون ضئيلاً. اختبر كليهما. استخدم الأرخص الذي يلبي معيار جودتك.

{@ insight cost_projection @}

### خط الأنابيب 1: مولّد محتوى النشرة البريدية

هذه هي أتمتة النموذج اللغوي الأكثر شيوعاً للمطورين ذوي الدخل القائم على المحتوى. موجزات RSS تدخل، مسودة نشرة بريدية تخرج.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — استيعاب موجزات RSS، التلخيص بالنموذج اللغوي، إنشاء مسودة النشرة البريدية.
شغّل يومياً: 0 5 * * * python3 /path/to/newsletter_pipeline.py

هذا الخط:
1. يجلب مقالات جديدة من موجزات RSS متعددة
2. يرسل كل واحدة إلى نموذج لغوي محلي للتلخيص
3. يرتبها حسب الصلة بجمهورك
4. يولّد مسودة نشرة بريدية مُنسقة
5. يحفظ المسودة لمراجعتك (تقضي 10 دقائق في المراجعة، لا ساعتين في التنسيق)
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
    # أضف موجزات تخصصك هنا
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
    """تحليل موجز RSS/Atom وإرجاع المقالات."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # التعامل مع موجزات RSS و Atom
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
        print(f"  فشل جلب {url}: {e}")
        return []

def llm_process(prompt: str) -> str:
    """إرسال موجه إلى النموذج اللغوي المحلي والحصول على الاستجابة."""
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
        print(f"  خطأ النموذج اللغوي: {e}")
        return ""

def score_and_summarize(article: dict) -> dict:
    """استخدام النموذج اللغوي لتسجيل الصلة وإنشاء ملخص."""
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
        # محاولة تحليل JSON من مخرجات النموذج اللغوي
        # التعامل مع حالات تغليف النموذج بكتل كود markdown
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
    """تنسيق المقالات المُسجلة إلى مسودة نشرة بريدية."""
    today = datetime.now().strftime("%Y-%m-%d")

    sections = {"tool": [], "technique": [], "news": [], "opinion": [], "tutorial": []}
    for article in articles:
        cat = article.get("category", "news")
        if cat in sections:
            sections[cat].append(article)

    newsletter = []
    newsletter.append(f"# نشرتك البريدية — {today}")
    newsletter.append("")
    newsletter.append("*[مقدمتك هنا — اكتب 2-3 جمل عن موضوع هذا الأسبوع]*")
    newsletter.append("")

    section_titles = {
        "tool": "أدوات وإصدارات",
        "technique": "تقنيات وأنماط",
        "news": "أخبار الصناعة",
        "tutorial": "دروس وأدلة",
        "opinion": "وجهات نظر"
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
    newsletter.append("*[خاتمتك — ماذا تعمل عليه؟ ما الذي يجب على القراء الانتباه له؟]*")

    return "\n".join(newsletter)

def main():
    seen = load_seen()
    all_articles = []

    print("جلب الموجزات...")
    for feed_url in FEEDS:
        articles = fetch_feed(feed_url)
        new_articles = [a for a in articles if a["id"] not in seen]
        all_articles.extend(new_articles)
        print(f"  {feed_url}: {len(new_articles)} مقال جديد")

    if not all_articles:
        print("لا مقالات جديدة. تخطي.")
        return

    print(f"\nتسجيل {len(all_articles)} مقال بالنموذج اللغوي...")
    scored = []
    for i, article in enumerate(all_articles):
        print(f"  [{i+1}/{len(all_articles)}] {article['title'][:60]}...")
        scored_article = score_and_summarize(article)
        scored.append(scored_article)
        seen.add(article["id"])

    # تصفية المقالات ذات الصلة فقط وترتيبها حسب الدرجة
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # أخذ أفضل 10
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} مقال تجاوزت عتبة الصلة (>= 6/10)")

    # إنشاء مسودة النشرة البريدية
    draft = generate_newsletter(top_articles)

    # حفظ المسودة
    os.makedirs(DRAFTS_DIR, exist_ok=True)
    draft_path = os.path.join(DRAFTS_DIR, f"draft-{datetime.now().strftime('%Y-%m-%d')}.md")
    with open(draft_path, "w", encoding="utf-8") as f:
        f.write(draft)

    save_seen(seen)
    print(f"\nتم حفظ المسودة: {draft_path}")
    print("راجعها، أضف مقدمتك/خاتمتك، وأرسلها.")

if __name__ == "__main__":
    main()
```

**ما تكلفة هذا:**
- معالجة 50 مقالاً/يومياً بنموذج محلي 8B: ~$0/شهر
- وقتك: 10 دقائق لمراجعة المسودة مقابل ساعتين من التنسيق اليدوي
- الوقت الموفر أسبوعياً: ~10 ساعات إذا كنت تُصدر نشرة بريدية أسبوعية

### خط الأنابيب 2: أبحاث العملاء وتقارير الرؤى

هذا الخط يجمع بيانات عامة، يحللها بنموذج لغوي، وينتج تقريراً يمكنك بيعه.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — تحليل بيانات الشركات/المنتجات العامة وإنشاء تقارير رؤى.
هذه خدمة يمكنك بيعها: $200-500 لكل تقرير مخصص.

الاستخدام: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# استخدم نموذجاً أكبر للجودة في التقارير المدفوعة
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# أو استخدم واجهة برمجية للجودة الموجهة للعملاء:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """التوجيه إلى النموذج المحلي أو الواجهة البرمجية حسب الإعداد."""
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
    """جمع البيانات المتاحة علنياً عن شركة."""
    data = {"company": company, "domain": domain}

    # التحقق مما إذا كان النطاق قابلاً للوصول والحصول على معلومات أساسية
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

    # التحقق من التواجد على GitHub
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
    """إنشاء تقرير تحليلي باستخدام النموذج اللغوي."""
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
        print("الاستخدام: python3 research_pipeline.py 'Company Name' 'domain.com'")
        sys.exit(1)

    company = sys.argv[1]
    domain = sys.argv[2]

    print(f"البحث عن: {company} ({domain})")
    print(f"باستخدام: {'API (Claude)' if USE_API else 'Local (Ollama)'}")

    print("جمع البيانات العامة...")
    data = gather_public_data(company, domain)

    print("إنشاء التحليل...")
    report = generate_report(company, domain, data)

    # تجميع التقرير النهائي
    final_report = f"""# تقرير بحثي: {company}

**تم الإنشاء:** {datetime.now().strftime('%Y-%m-%d %H:%M')}
**النطاق:** {domain}
**نموذج التحليل:** {'Claude Sonnet' if USE_API else MODEL}

---

{report}

---

*تم إنشاء هذا التقرير باستخدام البيانات المتاحة علنياً فقط.
لم يتم الوصول إلى أي بيانات ملكية أو خاصة.*
"""

    os.makedirs(REPORTS_DIR, exist_ok=True)
    filename = f"{company.lower().replace(' ', '-')}-{datetime.now().strftime('%Y%m%d')}.md"
    filepath = os.path.join(REPORTS_DIR, filename)

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(final_report)

    print(f"\nتم حفظ التقرير: {filepath}")
    print(f"تكلفة الواجهة البرمجية: ~${'0.02-0.05' if USE_API else '0.00'}")

if __name__ == "__main__":
    main()
```

**نموذج العمل:** اطلب $200-500 لكل تقرير بحثي مخصص. تكلفتك: $0.05 في مكالمات الواجهة البرمجية و15 دقيقة مراجعة. يمكنك إنتاج 3-4 تقارير في الساعة بمجرد استقرار الخط.

### خط الأنابيب 3: مراقب إشارات السوق

هذا هو الخط الذي يخبرك بما يجب بناؤه بعد ذلك. يراقب مصادر متعددة، يصنف الإشارات، وينبهك عندما تتجاوز فرصة ما عتبتك.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — مراقبة المصادر العامة لفرص السوق.
شغّل كل ساعتين: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# تعريف تخصصك — النموذج اللغوي يستخدم هذا لتسجيل الصلة
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
    """جلب أهم قصص Hacker News."""
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
        print(f"  فشل جلب HN: {e}")
        return []

def classify_signal(item: dict) -> dict:
    """استخدام النموذج اللغوي لتصنيف إشارة لفرصة سوقية."""
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
        item["reasoning"] = f"فشل التصنيف: {e}"
        item["action"] = "none"

    return item

def alert_on_opportunity(item: dict):
    """إرسال تنبيه للفرص ذات الدرجات العالية."""
    msg = (
        f"تم اكتشاف فرصة (الدرجة: {item['opportunity_score']}/10)\n"
        f"النوع: {item['opportunity_type']}\n"
        f"العنوان: {item['title']}\n"
        f"الرابط: {item.get('url', 'N/A')}\n"
        f"السبب: {item['reasoning']}\n"
        f"الإجراء: {item['action']}"
    )

    # تسجيل في ملف
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # الإرسال إلى Slack/Discord
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  تنبيه: {msg}")

def main():
    seen = load_seen()

    # الجلب من المصادر
    print("جلب الإشارات...")
    items = fetch_hn_top(30)
    # أضف مصادر أخرى هنا: Reddit، موجزات RSS، GitHub الرائج، إلخ.

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} إشارة جديدة للتصنيف")

    # تصنيف كل إشارة
    for i, item in enumerate(new_items):
        print(f"  [{i+1}/{len(new_items)}] {item['title'][:50]}...")
        classified = classify_signal(item)
        seen.add(item["id"])

        if classified.get("opportunity_score", 0) >= 7:
            alert_on_opportunity(classified)

    save_seen(seen)
    print("تم.")

if __name__ == "__main__":
    main()
```

**ما يفعله هذا عملياً:** تحصل على إشعار Slack 2-3 مرات أسبوعياً يقول شيئاً مثل "فرصة: إطار عمل جديد صدر بدون حزمة بداية — يمكنك بناء واحدة في عطلة نهاية الأسبوع." تلك الإشارة، والتصرف بناءً عليها قبل الآخرين، هي كيف تبقى في المقدمة.

> **حديث صريح:** جودة مخرجات هذه الأنابيب تعتمد كلياً على موجهاتك وتعريف تخصصك. إذا كان تخصصك غامضاً ("أنا مطور ويب")، فالنموذج اللغوي سيُبلّغ عن كل شيء. إذا كان محدداً ("أبني تطبيقات سطح مكتب Tauri لسوق المطورين الذين يهتمون بالخصوصية أولاً")، سيكون دقيقاً جراحياً. اقضِ 30 دقيقة في إتقان تعريف تخصصك. إنه المُدخل الأعلى تأثيراً على كل خط أنابيب تبنيه.

### دورك

{? if stack.contains("python") ?}
أخبار جيدة: أمثلة الأنابيب أعلاه مكتوبة بلغتك الأساسية. يمكنك نسخها مباشرة والبدء في التكييف. ركز على إتقان تعريف التخصص والموجهات — من هنا تأتي 90% من جودة المخرجات.
{? else ?}
الأمثلة أعلاه تستخدم Python لقابلية النقل، لكن الأنماط تعمل بأي لغة. إذا كنت تفضل البناء بـ {= stack.primary | fallback("لغتك الأساسية") =}، القطع الرئيسية للتكرار هي: عميل HTTP لجلب RSS/API، تحليل JSON لاستجابات النموذج اللغوي، وعمليات الملفات لإدارة الحالة. التفاعل مع النموذج اللغوي هو مجرد HTTP POST إلى Ollama أو واجهة برمجية سحابية.
{? endif ?}

1. اختر واحداً من الأنابيب الثلاثة أعلاه (النشرة البريدية، الأبحاث، أو مراقب الإشارات).
2. كيّفه مع تخصصك. غيّر الموجزات، وصف الجمهور، معايير التصنيف.
3. شغّله يدوياً 3 مرات لاختبار جودة المخرجات.
4. اضبط الموجهات حتى تكون المخرجات مفيدة بدون تحرير كثيف.
5. جدوِله مع cron.

**الهدف:** خط أنابيب واحد مدعوم بنموذج لغوي يعمل وفق جدول خلال 48 ساعة من قراءة هذا الدرس.

---

## الدرس 4: من المستوى 3 إلى 4 — الأنظمة القائمة على الوكلاء

*"الوكيل هو مجرد حلقة تراقب وتقرر وتتصرف. ابنِ واحداً."*

### ماذا يعني "الوكيل" فعلاً في 2026

تخلص من الضجة الإعلامية. الوكيل هو برنامج يقوم بـ:

1. **يراقب** — يقرأ بعض المدخلات أو الحالة
2. **يقرر** — يستخدم نموذجاً لغوياً لتحديد ما يجب فعله
3. **يتصرف** — ينفذ القرار
4. **يتكرر** — يعود إلى الخطوة 1

هذا كل شيء. الفرق بين خط أنابيب (المستوى 3) ووكيل (المستوى 4) هو أن الوكيل يتكرر. يتصرف بناءً على مخرجاته. يتعامل مع مهام متعددة الخطوات حيث تعتمد الخطوة التالية على نتيجة السابقة.

خط الأنابيب يعالج العناصر واحداً تلو الآخر من خلال تسلسل ثابت. الوكيل يتنقل في تسلسل غير قابل للتنبؤ بناءً على ما يواجهه.

### خوادم MCP التي تخدم العملاء

خادم MCP هو أحد أكثر الأنظمة العملية المتعلقة بالوكلاء التي يمكنك بناؤها. يكشف أدوات يمكن لوكيل ذكاء اصطناعي (Claude Code، Cursor، إلخ.) استدعاؤها نيابة عن عملائك.

{? if stack.contains("typescript") ?}
مثال خادم MCP أدناه يستخدم TypeScript — في صميم خبرتك. يمكنك توسيعه بأدوات TypeScript الحالية ونشره جنباً إلى جنب مع خدمات Node.js الأخرى.
{? endif ?}

إليك مثالاً حقيقياً: خادم MCP يجيب على أسئلة العملاء من وثائق منتجك.

```typescript
// mcp-docs-server/src/index.ts
// خادم MCP يجيب على الأسئلة من وثائقك.
// عملاؤك يوجهون Claude Code الخاص بهم إلى هذا الخادم ويحصلون على إجابات فورية.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// تحميل وثائقك في الذاكرة عند بدء التشغيل
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

    // التقسيم حسب العناوين للبحث الأفضل
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
  // بحث بسيط بالكلمات المفتاحية — استبدل بالبحث المتجهي للإنتاج
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // مكافأة لتطابقات العنوان
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

// التهيئة
const docs = loadDocs();
console.error(`تم تحميل ${docs.length} قطعة وثائق من ${DOCS_DIR}`);

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

// بدء الخادم
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

**نموذج العمل:** أعطِ خادم MCP هذا لعملائك كجزء من منتجك. يحصلون على إجابات فورية لأسئلتهم دون تقديم تذاكر دعم. تقضي وقتاً أقل في الدعم. الجميع يستفيد.

للمتميز: اطلب $9-29/شهر لنسخة مستضافة مع بحث متجهي، وثائق مُرقمة الإصدار، وتحليلات حول ما يسأل عنه العملاء.

### معالجة ملاحظات العملاء الآلية

هذا الوكيل يقرأ ملاحظات العملاء (من البريد الإلكتروني، تذاكر الدعم، أو نموذج)، يصنفها، وينشئ مسودات ردود وتذاكر ميزات.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — معالجة ملاحظات العملاء إلى عناصر مصنفة وقابلة للتنفيذ.
نمط "مسودة ذكاء اصطناعي، موافقة بشرية."

شغّل كل ساعة: 0 * * * * python3 /path/to/feedback_agent.py
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
    """تصنيف الملاحظات وإنشاء مسودة رد."""

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
        feedback["draft_response"] = "[فشل التصنيف — يحتاج مراجعة يدوية]"

    feedback["processed_at"] = datetime.utcnow().isoformat() + "Z"
    return feedback

def main():
    os.makedirs(REVIEW_DIR, exist_ok=True)
    os.makedirs(PROCESSED_DIR, exist_ok=True)

    if not os.path.isdir(INBOX_DIR):
        print(f"لا يوجد مجلد وارد: {INBOX_DIR}")
        return

    inbox_files = sorted(Path(INBOX_DIR).glob("*.json"))

    if not inbox_files:
        print("لا ملاحظات جديدة.")
        return

    print(f"معالجة {len(inbox_files)} عنصر ملاحظات...")

    review_queue = []

    for filepath in inbox_files:
        try:
            with open(filepath, "r") as f:
                feedback = json.load(f)
        except (json.JSONDecodeError, Exception) as e:
            print(f"  تخطي {filepath.name}: {e}")
            continue

        print(f"  معالجة: {feedback.get('subject', 'بدون موضوع')[:50]}...")
        processed = process_feedback(feedback)

        # حفظ النسخة المُعالجة
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # إضافة إلى قائمة المراجعة
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # نقل الأصلي خارج الوارد
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # كتابة قائمة المراجعة
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # الملخص
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nتمت المعالجة: {len(review_queue)}")
    print(f"حرج: {critical}")
    print(f"يحتاج انتباهك: {needs_human}")
    print(f"قائمة المراجعة: {review_path}")

if __name__ == "__main__":
    main()
```

**كيف يعمل هذا عملياً:**
1. يقدم العملاء ملاحظات (عبر نموذج، بريد إلكتروني، أو نظام دعم)
2. الملاحظات تصل كملفات JSON في مجلد الوارد
3. الوكيل يعالج كل واحدة: يصنف، يلخص، يصيغ مسودة رد
4. تفتح قائمة المراجعة مرة أو مرتين يومياً
5. للعناصر البسيطة (إشادة، أسئلة أساسية مع مسودات ردود جيدة)، توافق على المسودة
6. للعناصر المعقدة (أخطاء، عملاء غاضبون)، تكتب رداً شخصياً
7. الوقت الصافي: 15 دقيقة يومياً بدلاً من ساعتين

### نمط المسودة الذكية، الموافقة البشرية

هذا النمط هو جوهر أتمتة المستوى 4 العملية. الوكيل يتعامل مع العمل الشاق. أنت تتعامل مع قرارات الحكم.

```
              ┌─────────────┐
              │ الوكيل يصيغ │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ قائمة المراجعة│
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │إرسال تلقائي│ │تحرير│ │ تصعيد  │
    │ (روتيني)  │ │+إرسال│ │(معقد)  │
    └───────────┘ └─────┘ └─────────┘
```

**قواعد لما يتعامل معه الوكيل بالكامل مقابل ما تراجعه:**

| الوكيل يتعامل بالكامل (بدون مراجعة) | أنت تراجع قبل الإرسال |
|--------------------------------------|----------------------|
| إيصالات الاستلام ("تلقينا رسالتك") | الردود على العملاء الغاضبين |
| تحديثات الحالة ("يتم معالجة طلبك") | تحديد أولويات طلبات الميزات |
| ردود الأسئلة الشائعة (تطابق دقيق) | أي شيء يتعلق بالمال (استردادات، تسعير) |
| تصنيف وحذف الرسائل غير المرغوبة | تقارير الأخطاء (تحتاج التحقق) |
| التسجيل والتصنيف الداخلي | أي شيء لم تره من قبل |

> **خطأ شائع:** ترك الوكيل يرد على العملاء بشكل مستقل من اليوم الأول. لا تفعل. ابدأ بجعل الوكيل يصيغ كل شيء، وأنت توافق على كل شيء. بعد أسبوع، اسمح له بالإرسال التلقائي لإيصالات الاستلام. بعد شهر، اسمح له بالإرسال التلقائي لردود الأسئلة الشائعة. ابنِ الثقة تدريجياً — مع نفسك ومع عملائك.

### دورك

1. اختر واحداً: ابنِ خادم MCP للوثائق أو وكيل معالجة الملاحظات.
2. كيّفه مع منتجك/خدمتك. إذا لم يكن لديك عملاء بعد، استخدم مراقب الإشارات من الدرس 3 كـ "عميلك" — عالج مخرجاته من خلال نمط وكيل الملاحظات.
3. شغّله يدوياً 10 مرات بمدخلات مختلفة.
4. قِس: ما نسبة المخرجات القابلة للاستخدام دون تحرير؟ هذه درجة جودة الأتمتة. استهدف 70%+ قبل الجدولة.

---

## الدرس 5: مبدأ الإشراف البشري

*"الأتمتة الكاملة فخ. الأتمتة الجزئية قوة خارقة."*

### لماذا 80% أتمتة تتفوق على 100%

هناك سبب محدد وقابل للقياس لعدم وجوب أتمتة العمليات الموجهة للعملاء بالكامل: تكلفة المخرج السيء غير متماثلة.

مخرج مؤتمت جيد يوفر لك 5 دقائق.
مخرج مؤتمت سيء يكلفك عميلاً، شكوى عامة، استرداد أموال، أو ضربة سمعة تستغرق أشهراً للتعافي منها.

الحساب:

```
100% أتمتة:
  1,000 مخرج/شهر × 95% جودة = 950 جيد + 50 سيء
  50 مخرج سيء × $50 تكلفة متوسطة (استرداد + دعم + سمعة) = $2,500/شهر في الأضرار

80% أتمتة + 20% مراجعة بشرية:
  800 مخرج معالج تلقائياً، 200 مراجعة بشرية
  800 × 95% جودة = 760 جيد + 40 سيء تلقائي
  200 × 99% جودة = 198 جيد + 2 سيء بشري
  42 إجمالي سيء × $50 = $2,100/شهر في الأضرار
  لكن: تلتقط 38 من السيئة قبل وصولها للعملاء

  المخرجات السيئة الفعلية التي تصل للعملاء: ~4
  الأضرار الفعلية: ~$200/شهر
```

هذا انخفاض 12 ضعفاً في تكلفة الأضرار. وقتك في مراجعة 200 مخرج (ربما ساعتان) يوفر لك $2,300/شهر في الأضرار.

### لا تؤتمت هذه بالكامل أبداً

بعض الأشياء يجب أن يكون فيها إشراف بشري دائماً، بغض النظر عن مدى جودة الذكاء الاصطناعي:

1. **التواصل الموجه للعملاء** — رسالة إلكترونية بصياغة سيئة يمكن أن تفقدك عميلاً للأبد. رد عام وواضح أنه مولّد بالذكاء الاصطناعي يمكن أن يُفقد الثقة. راجعه.

2. **المعاملات المالية** — الاستردادات، تغييرات الأسعار، الفوترة. راجع دائماً. تكلفة الخطأ هي أموال حقيقية.

3. **المحتوى المنشور باسمك** — سمعتك تتراكم على مدى سنوات ويمكن تدميرها في منشور واحد سيء. عشر دقائق من المراجعة تأمين رخيص.

4. **المخرجات القانونية أو المتعلقة بالامتثال** — أي شيء يتعلق بالعقود، سياسات الخصوصية، شروط الخدمة. الذكاء الاصطناعي يرتكب أخطاء قانونية بثقة.

5. **قرارات التوظيف أو الأشخاص** — إذا استعنت بمصادر خارجية في أي وقت، لا تدع الذكاء الاصطناعي يتخذ القرار النهائي بشأن من تعمل معه.

### ديون الأتمتة

{@ mirror automation_risk_profile @}

ديون الأتمتة أسوأ من الديون التقنية لأنها غير مرئية حتى تنفجر.

**ما تبدو عليه ديون الأتمتة:**
- بوت وسائل التواصل الاجتماعي ينشر في الوقت الخاطئ لأن المنطقة الزمنية تغيرت
- خط أنابيب النشرة البريدية يتضمن رابطاً معطلاً منذ 3 أسابيع لأن لا أحد يتحقق
- مراقب الأسعار توقف عن العمل عندما أعاد المنافس تصميم صفحته
- سكربت النسخ الاحتياطي يفشل بصمت لأن القرص امتلأ

**كيفية الوقاية:**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — مراقبة جميع أتمتتك للفشل الصامت.
شغّل كل صباح: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# تحديد المخرجات المتوقعة من كل أتمتة
AUTOMATIONS = [
    {
        "name": "خط أنابيب النشرة البريدية",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # يجب أن ينتج يومياً على الأقل
    },
    {
        "name": "الناشر الاجتماعي",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # التحقق من الملف مباشرة
        "max_age_hours": 2,  # يجب أن يُحدَّث كل 30 دقيقة
    },
    {
        "name": "مراقب المنافسين",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # يجب أن يعمل كل 6 ساعات
    },
    {
        "name": "نسخة العملاء الاحتياطية",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # يجب أن يعمل ليلاً
    },
    {
        "name": "خادم Ollama",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """التحقق مما إذا كانت الأتمتة قد أنتجت مخرجات حديثة."""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # التحقق من ملفات حديثة تطابق النمط
        p = Path(path)
        if not p.exists():
            return False, f"المجلد غير موجود: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"لا ملفات تطابق {automation['pattern']} في {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # التحقق من الملف مباشرة
        if not os.path.exists(path):
            return False, f"الملف غير موجود: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"آخر مخرج منذ {age.total_seconds()/3600:.1f} ساعة (الحد الأقصى: {automation['max_age_hours']} ساعة)"

    return True, f"جيد (آخر مخرج منذ {age.total_seconds()/3600:.1f} ساعة)"

def check_http(automation: dict) -> tuple[bool, str]:
    """التحقق مما إذا كانت خدمة تستجيب."""
    import requests
    try:
        resp = requests.get(automation["url"], timeout=10)
        if resp.status_code == automation.get("expected_status", 200):
            return True, f"جيد (HTTP {resp.status_code})"
        return False, f"حالة غير متوقعة: HTTP {resp.status_code}"
    except Exception as e:
        return False, f"فشل الاتصال: {e}"

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
            ok, msg = False, f"نوع فحص غير معروف: {check_type}"

        status = "جيد" if ok else "فشل"
        print(f"  [{status}] {automation['name']}: {msg}")

        if not ok:
            failures.append(f"{automation['name']}: {msg}")

    if failures:
        alert_msg = (
            f"فحص صحة الأتمتة — {len(failures)} فشل\n\n"
            + "\n".join(f"  {f}" for f in failures)
            + "\n\nتحقق من السجلات وأصلح قبل أن تتراكم."
        )
        send_alert(alert_msg)

if __name__ == "__main__":
    main()
```

شغّل هذا كل صباح. عندما تنكسر أتمتة بصمت (وسيحدث ذلك)، ستعرف خلال 24 ساعة بدلاً من 3 أسابيع.

### بناء قوائم المراجعة

مفتاح جعل الإشراف البشري فعالاً هو تجميع مراجعتك. لا تراجع عنصراً واحداً في كل مرة عند وصوله. اجمعها وراجعها في دفعات.

```python
#!/usr/bin/env python3
"""
review_queue.py — قائمة مراجعة بسيطة للمخرجات المولدة بالذكاء الاصطناعي.
راجع مرة أو مرتين يومياً بدلاً من التحقق المستمر.
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """إضافة عنصر إلى قائمة المراجعة."""
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
    """عرض جميع العناصر المعلقة للمراجعة."""
    if not os.path.isdir(QUEUE_DIR):
        print("القائمة فارغة.")
        return

    pending = sorted(Path(QUEUE_DIR).glob("*.json"))

    if not pending:
        print("القائمة فارغة.")
        return

    print(f"\n{'='*60}")
    print(f"قائمة المراجعة — {len(pending)} عنصر معلق")
    print(f"{'='*60}\n")

    for i, filepath in enumerate(pending):
        with open(filepath, "r") as f:
            item = json.load(f)

        print(f"[{i+1}] {item['type']} — {item['created_at']}")
        content = item.get("content", {})

        if item["type"] == "newsletter_draft":
            print(f"    المقالات: {content.get('article_count', '?')}")
            print(f"    المسودة: {content.get('draft_path', 'غير معروف')}")
        elif item["type"] == "customer_response":
            print(f"    إلى: {content.get('customer', 'غير معروف')}")
            print(f"    المسودة: {content.get('draft_response', '')[:100]}...")
        elif item["type"] == "social_post":
            print(f"    النص: {content.get('text', '')[:100]}...")

        print(f"    الإجراءات: [م]وافقة  [ت]حرير  [ر]فض  [ت]خطي")
        print()

    # في تطبيق حقيقي، ستضيف إدخالاً تفاعلياً هنا
    # للمعالجة الدفعية، اقرأ القرارات من ملف أو واجهة سطر أوامر بسيطة

if __name__ == "__main__":
    review_queue()
```

**عادة المراجعة:** تحقق من قائمة المراجعة في الساعة 8 صباحاً و4 مساءً. جلستان، 10-15 دقيقة لكل واحدة. كل شيء آخر يعمل بشكل مستقل بين المراجعات.

> **حديث صريح:** فكّر فيما يحدث عندما تتخطى المراجعة البشرية: تؤتمت نشرتك البريدية بالكامل، النموذج اللغوي يبدأ بإدراج روابط مُهلوسة لصفحات غير موجودة، والمشتركون يلاحظون قبلك. تفقد جزءاً من قائمتك ويستغرق الأمر أشهراً لإعادة بناء الثقة. بالمقابل، المطور الذي يؤتمت 80% من نفس العملية — النموذج اللغوي ينسق ويصيغ، ويقضي 10 دقائق في المراجعة — يلتقط تلك الهلوسات قبل إرسالها. الفرق ليس الأتمتة. إنه خطوة المراجعة.

### دورك

1. أعد سكربت `automation_healthcheck.py` لأي أتمتة بنيتها في الدرسين 2 و3. جدوِله ليعمل كل صباح.
2. نفّذ قائمة مراجعة لأعلى أتمتة خطراً (أي شيء موجه للعملاء).
3. التزم بفحص قائمة المراجعة مرتين يومياً لمدة أسبوع. سجّل كم عنصراً توافق عليه دون تغيير، كم تحرر، وكم ترفض. هذه البيانات تخبرك بمدى جودة أتمتتك فعلياً.

---

## الدرس 6: تحسين التكلفة وخط أنابيبك الأول

*"إذا لم تستطع توليد $200 من إيرادات من $200 في إنفاق API، أصلح المنتج — لا الميزانية."*

### اقتصاديات الأتمتة المدعومة بنماذج لغوية كبيرة

كل مكالمة نموذج لغوي لها تكلفة. حتى النماذج المحلية تكلف كهرباء وتآكل GPU. السؤال هو ما إذا كانت مخرجات تلك المكالمة تولّد قيمة أكثر مما تكلفه.

{? if profile.gpu.exists ?}
تشغيل النماذج المحلية على {= profile.gpu.model | fallback("GPU الخاص بك") =} يكلف تقريباً {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("بضعة دولارات") =} في الكهرباء شهرياً لأحمال عمل الأنابيب النموذجية. هذا هو الأساس الذي يجب التغلب عليه ببدائل الواجهة البرمجية.
{? endif ?}

**قاعدة ميزانية {= regional.currency_symbol | fallback("$") =}200/شهر للواجهة البرمجية:**

إذا كنت تنفق {= regional.currency_symbol | fallback("$") =}200/شهر على مكالمات الواجهة البرمجية لأتمتتك، فيجب أن تولّد تلك الأتمتة ما لا يقل عن {= regional.currency_symbol | fallback("$") =}200/شهر في القيمة — إما إيرادات مباشرة أو وقت موفر تحوله إلى إيرادات في مكان آخر.

إذا لم تكن كذلك: المشكلة ليست ميزانية الواجهة البرمجية. بل تصميم الخط أو المنتج الذي يدعمه.

### تتبع التكلفة لكل مخرج

أضف هذا إلى كل خط أنابيب تبنيه:

```python
"""
cost_tracker.py — تتبع تكلفة كل مكالمة نموذج لغوي والقيمة التي تولدها.
استورد هذا في أنابيبك للحصول على بيانات تكلفة حقيقية.
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# التسعير لكل 1 مليون رمز (حدّث عند تغير الأسعار)
PRICING = {
    # النماذج المحلية — تقدير تكلفة الكهرباء
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # نماذج الواجهة البرمجية
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
    """تسجيل تكلفة مكالمة نموذج لغوي."""
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
    """إنشاء ملخص شهري للتكلفة/الإيرادات."""
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

    # طباعة التقرير
    print(f"\nتقرير تكلفة النموذج اللغوي — {current_month}")
    print("=" * 60)

    grand_cost = 0
    grand_revenue = 0

    for name, data in sorted(pipelines.items()):
        roi = data["total_revenue"] / data["total_cost"] if data["total_cost"] > 0 else 0
        print(f"\n  {name}")
        print(f"    المكالمات:   {data['call_count']}")
        print(f"    الرموز:     {data['total_tokens']:,}")
        print(f"    التكلفة:    ${data['total_cost']:.4f}")
        print(f"    الإيرادات:  ${data['total_revenue']:.2f}")
        print(f"    العائد:     {roi:.1f}x")

        grand_cost += data["total_cost"]
        grand_revenue += data["total_revenue"]

    print(f"\n{'='*60}")
    print(f"  إجمالي التكلفة:    ${grand_cost:.4f}")
    print(f"  إجمالي الإيرادات:  ${grand_revenue:.2f}")
    if grand_cost > 0:
        print(f"  العائد الإجمالي:   {grand_revenue/grand_cost:.1f}x")

    return pipelines

if __name__ == "__main__":
    monthly_report()
```

### التجميع لكفاءة الواجهة البرمجية

إذا كنت تستخدم نماذج واجهة برمجية، التجميع يوفر مالاً حقيقياً:

```python
"""
batch_api.py — تجميع مكالمات الواجهة البرمجية للكفاءة.
بدلاً من 100 مكالمة واجهة برمجية منفصلة، اجمعها.
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
    تصنيف عناصر متعددة بكفاءة عن طريق تجميعها في مكالمات واجهة برمجية واحدة.

    بدلاً من 100 مكالمة واجهة برمجية (100 عنصر × 1 مكالمة لكل منها):
      - 100 مكالمة × ~500 رمز إدخال = 50,000 رمز إدخال
      - 100 مكالمة × ~200 رمز إخراج = 20,000 رمز إخراج
      - التكلفة مع Haiku: ~$0.12

    مع التجميع (10 عناصر لكل مكالمة، 10 مكالمات واجهة برمجية):
      - 10 مكالمات × ~2,500 رمز إدخال = 25,000 رمز إدخال
      - 10 مكالمات × ~1,000 رمز إخراج = 10,000 رمز إخراج
      - التكلفة مع Haiku: ~$0.06

    توفير 50% من التجميع وحده.
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # تنسيق الدفعة في موجه واحد
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
            # تحليل مصفوفة JSON من الاستجابة
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  فشلت الدفعة {i//batch_size + 1}: {e}")
            # التراجع إلى المعالجة الفردية
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # مراعاة حد المعدل
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### التخزين المؤقت: لا تدفع مرتين لنفس الإجابة

```python
"""
llm_cache.py — تخزين استجابات النموذج اللغوي مؤقتاً لتجنب الدفع للمعالجة المكررة.
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
    """إنشاء مفتاح تخزين مؤقت حتمي من النموذج + الموجه."""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """الحصول على استجابة مخزنة مؤقتاً إذا كانت متاحة وحديثة."""
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

    # تحديث عدد الإصابات
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """تخزين استجابة مؤقتاً."""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """عرض إحصائيات التخزين المؤقت."""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"إدخالات التخزين المؤقت: {total}")
    print(f"إجمالي إصابات التخزين المؤقت: {total_hits}")
    print(f"التوفير المقدر: ~${total_hits * 0.002:.2f} (متوسط تقريبي لكل مكالمة)")
```

**استخدمه في أنابيبك:**

```python
# في أي خط أنابيب يستدعي نموذجاً لغوياً:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # مجاني!

    response = call_llm(model, prompt)  # دالة استدعاء النموذج اللغوي الحالية
    set_cached(model, prompt, response)
    return response
```

للأنابيب التي تعالج نفس أنواع المحتوى بشكل متكرر (تصنيف، استخراج)، التخزين المؤقت يمكن أن يزيل 30-50% من مكالمات الواجهة البرمجية. هذا 30-50% من فاتورتك الشهرية.

### بناء خط أنابيبك الأول الكامل: خطوة بخطوة

إليك العملية الكاملة من "لدي سير عمل يدوي" إلى "يعمل أثناء نومي."

**الخطوة 1: ارسم عمليتك اليدوية الحالية.**

اكتب كل خطوة تقوم بها لتيار دخل محدد. مثال لنشرة بريدية:

```
1. فتح 15 موجز RSS في تبويبات المتصفح (10 دقائق)
2. تصفح العناوين، فتح المثيرة للاهتمام (20 دقيقة)
3. قراءة 8-10 مقالات بالتفصيل (40 دقيقة)
4. كتابة ملخصات لأفضل 5 (30 دقيقة)
5. كتابة فقرة المقدمة (10 دقائق)
6. التنسيق في أداة البريد الإلكتروني (15 دقيقة)
7. الإرسال إلى القائمة (5 دقائق)

المجموع: ~ساعتان و10 دقائق
```

**الخطوة 2: حدد الخطوات الثلاث الأكثر استهلاكاً للوقت.**

من المثال: قراءة المقالات (40 دقيقة)، كتابة الملخصات (30 دقيقة)، تصفح العناوين (20 دقيقة).

**الخطوة 3: أتمت الأسهل أولاً.**

تصفح العناوين هو الأسهل للأتمتة — إنه تصنيف. النموذج اللغوي يسجل الصلة، تقرأ فقط الأعلى تسجيلاً.

**الخطوة 4: قِس الوقت الموفر والجودة.**

بعد أتمتة تصفح العناوين:
- الوقت الموفر: 20 دقيقة
- الجودة: 90% توافق مع اختياراتك اليدوية
- الصافي: 20 دقيقة موفرة، فقدان جودة ضئيل

**الخطوة 5: أتمت الخطوة التالية.**

الآن أتمت كتابة الملخصات. النموذج اللغوي يصيغ المسودات، أنت تحررها.

**الخطوة 6: استمر حتى تناقص العوائد.**

```
قبل الأتمتة: ساعتان و10 دقائق لكل نشرة بريدية
بعد المستوى 2 (جلب مجدول): ساعة و45 دقيقة
بعد المستوى 3 (تسجيل + ملخصات بالنموذج اللغوي): 25 دقيقة
بعد المستوى 3+ (النموذج اللغوي يصيغ المقدمة): 10 دقائق مراجعة فقط

الوقت الموفر أسبوعياً: ~ساعتان
الوقت الموفر شهرياً: ~8 ساعات
بمعدل فعلي $100/ساعة: $800/شهر في وقت محرر
تكلفة الواجهة البرمجية: $0 (نموذج لغوي محلي) إلى $5/شهر (واجهة برمجية)
```

**الخطوة 7: خط الأنابيب الكامل، مربوط ببعضه.**

إليك GitHub Action يربط كل شيء معاً لخط أنابيب نشرة بريدية أسبوعية:

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # كل يوم أحد في الساعة 5 صباحاً UTC
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
            -d '{"text":"مسودة النشرة البريدية جاهزة للمراجعة. تحقق من مخرجات GitHub Actions."}'
```

يعمل هذا كل يوم أحد في الساعة 5 صباحاً. بحلول وقت استيقاظك، المسودة تنتظرك. تقضي 10 دقائق في مراجعتها مع قهوتك، تضغط إرسال، ونشرتك البريدية منشورة للأسبوع.

### دورك: ابنِ خط أنابيبك

هذا هو مُخرج الوحدة. بنهاية هذا الدرس، يجب أن يكون لديك خط أنابيب واحد كامل منشور ويعمل.

**متطلبات خط أنابيبك:**
1. يعمل وفق جدول دون تدخلك
2. يتضمن خطوة معالجة واحدة على الأقل بنموذج لغوي
3. فيه خطوة مراجعة بشرية لضبط الجودة
4. فيه فحص صحة حتى تعرف إذا انكسر
5. مرتبط بتيار دخل حقيقي (أو تيار تبنيه)

**قائمة المراجعة:**

- [ ] اخترت تيار دخل لأتمتته
- [ ] رسمت العملية اليدوية (جميع الخطوات، مع تقديرات الوقت)
- [ ] حددت أكثر 3 خطوات استهلاكاً للوقت
- [ ] أتمتت الخطوة الأولى على الأقل (تصنيف/تسجيل/تصفية)
- [ ] أضفت معالجة النموذج اللغوي للخطوة الثانية (تلخيص/توليد/استخراج)
- [ ] بنيت قائمة مراجعة للإشراف البشري
- [ ] أعددت فحص صحة للأتمتة
- [ ] نشرت وفق جدول (cron، GitHub Actions، أو مؤقت systemd)
- [ ] تتبعت التكلفة والوقت الموفر لدورة كاملة واحدة
- [ ] وثّقت خط الأنابيب (ماذا يفعل، كيف تصلحه، ما يجب مراقبته)

إذا أنجزت جميع العناصر العشرة في هذه القائمة، لديك أتمتة من المستوى 3 تعمل. لقد حررت ساعات من أسبوعك يمكنك إعادة استثمارها في بناء تيارات أكثر أو تحسين الحالية.

---

## الوحدة T: مكتملة

{@ temporal automation_progress @}

### ما بنيته في أسبوعين

1. **فهم لهرم الأتمتة** — تعرف أين أنت وأين يجب أن يتجه كل تيار من تيارات دخلك.
2. **أتمتة مجدولة** تعمل على cron أو مُجدولات سحابية — الأساس غير المبهرج الذي يجعل كل شيء آخر ممكناً.
3. **أنابيب مدعومة بنماذج لغوية كبيرة** تتعامل مع قرارات الحكم التي كنت تتخذها يدوياً — تصنيف، تلخيص، توليد، مراقبة.
4. **أنماط قائمة على الوكلاء** يمكنك نشرها للتفاعل مع العملاء، معالجة الملاحظات، ومنتجات مدعومة بـ MCP.
5. **إطار إشراف بشري** يحمي سمعتك مع توفير 80%+ من وقتك.
6. **تتبع وتحسين التكلفة** حتى تولّد أتمتتك ربحاً، لا مجرد نشاط.
7. **خط أنابيب واحد كامل ومنشور** يولّد قيمة دون تدخلك الفعلي.

### تأثير التراكم

إليك ما يحدث خلال الأشهر الثلاثة القادمة إذا حافظت على ما بنيته في هذه الوحدة ووسعته:

```
الشهر 1: خط أنابيب واحد، يوفر 5-8 ساعات/أسبوع
الشهر 2: خطا أنابيب، يوفران 10-15 ساعة/أسبوع
الشهر 3: ثلاثة أنابيب، توفر 15-20 ساعة/أسبوع

بمعدل فعلي $100/ساعة، هذا $1,500-2,000/شهر
في وقت محرر — وقت تستثمره في تيارات جديدة.

الوقت المحرر من الشهر 1 يبني خط أنابيب الشهر 2.
الوقت المحرر من الشهر 2 يبني خط أنابيب الشهر 3.
الأتمتة تتراكم.
```

هذه هي الطريقة التي يعمل بها مطور واحد كفريق من خمسة. ليس بالعمل أكثر. بل ببناء أنظمة تعمل بينما لا تعمل أنت.

---

### تكامل 4DA

{? if dna.identity_summary ?}
بناءً على ملفك كمطور — {= dna.identity_summary | fallback("تركيزك التطويري") =} — أدوات 4DA أدناه ترتبط مباشرة بأنماط الأتمتة التي تعلمتها للتو. أدوات تصنيف الإشارات مناسبة بشكل خاص للمطورين في مجالك.
{? endif ?}

4DA بحد ذاتها أتمتة من المستوى 3. تستوعب المحتوى من عشرات المصادر، تسجل كل عنصر بخوارزمية PASIFA، وتعرض فقط ما يتعلق بعملك — كل ذلك دون أن تحرك ساكناً. أنت لا تتحقق يدوياً من Hacker News و Reddit و 50 موجز RSS. 4DA تفعل ذلك وتُظهر لك ما يهم.

ابنِ أنابيب دخلك بنفس الطريقة.

تقرير انتباه 4DA (`/attention_report` في أدوات MCP) يُظهر لك أين يذهب وقتك فعلاً مقابل أين يجب أن يذهب. شغّله قبل أن تقرر ما تؤتمته. الفجوة بين "الوقت المُنفق" و"الوقت الذي يجب إنفاقه" هي خارطة طريق الأتمتة الخاصة بك.

أدوات تصنيف الإشارات (`/get_actionable_signals`) يمكن أن تُغذي مباشرة خط مراقبة السوق — ممكنة طبقة ذكاء 4DA من إجراء التسجيل الأولي قبل أن يقوم خطك المخصص بالتحليل الخاص بالتخصص.

إذا كنت تبني أنابيب تراقب المصادر للفرص، لا تعد اختراع ما تفعله 4DA بالفعل. استخدم خادم MCP الخاص بها كلبنة في مجموعة الأتمتة الخاصة بك.

---

### ما يأتي بعد ذلك: الوحدة S — تكديس التيارات

الوحدة T أعطتك الأدوات لجعل كل تيار دخل يعمل بكفاءة. الوحدة S (تكديس التيارات) تجيب على السؤال التالي: **كم تيار يجب أن تشغل، وكيف يتناسبون معاً؟**

إليك ما تغطيه الوحدة S:

- **نظرية المحافظ لتيارات الدخل** — لماذا 3 تيارات تتفوق على تيار واحد، ولماذا 10 تيارات تتفوق على لا شيء
- **ارتباط التيارات** — أي التيارات تعزز بعضها وأيها يتنافس على وقتك
- **أرضية الدخل** — بناء قاعدة من الإيرادات المتكررة تغطي تكاليفك قبل أن تجرب
- **إعادة التوازن** — متى تضاعف على الرابح ومتى تقتل الخاسر
- **هندسة $10,000/شهر** — تركيبات تيارات محددة تصل إلى خمسة أرقام بـ 15-20 ساعة أسبوعياً

لديك البنية التحتية (الوحدة S)، والحصون (الوحدة T)، والمحركات (الوحدة R)، ودليل الإطلاق (الوحدة E)، ورادار الاتجاهات (الوحدة E)، والآن الأتمتة (الوحدة T). الوحدة S تربطهم جميعاً في محفظة دخل مستدامة ومتنامية.

---

**خط الأنابيب يعمل. المسودة جاهزة. تقضي 10 دقائق في المراجعة.**

**هذه هي الأتمتة التكتيكية. هكذا تتوسع.**
