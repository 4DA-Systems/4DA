# 模块 T：战术自动化

**STREETS 开发者收入课程 — 付费模块**
*第12-13周 | 共6课 | 交付物：一个自动产生价值的流水线*

> "LLM、代理、MCP和定时任务作为力量倍增器。"

---

你已经有收入引擎在运转。你有客户。你有可行的流程。而你正把60-70%的时间花在重复做同样的事情上：处理输入、格式化输出、查看监控、发送更新、审查队列。

那段时间是你最昂贵的资源，而你正把它烧在一台月费{= regional.currency_symbol | fallback("$") =}5的VPS就能处理的任务上。

{@ insight hardware_benchmark @}

这个模块是关于系统性地把自己从循环中移除——不是完全移除（那是我们在第5课会讲到的陷阱），而是从不需要你判断的80%工作中移除。结果是：你的收入流在你睡觉时、在你做主业时、在你构建下一个项目时持续产生收入。

在这两周结束时，你将拥有：

- 对自动化四个层级的清晰理解，以及你今天所处的位置
- 在你的基础设施上运行的定时任务和计划自动化
- 至少一个不需要你参与就能处理输入的LLM驱动流水线
- 对基于代理的系统以及它们何时在经济上合理的理解
- 一个防止自动化破坏你声誉的人在回路中框架
- 一个完整部署的、无需你主动参与就能产生价值的流水线

{? if stack.primary ?}
你的主要技术栈是{= stack.primary | fallback("your primary stack") =}，所以接下来的自动化示例在适配到该生态系统时将最直接适用。大多数示例使用Python以保证可移植性，但这些模式可以转换到任何语言。
{? endif ?}

这是课程中代码最密集的模块。以下内容至少有一半是可运行的代码。复制它、适配它、部署它。

开始自动化吧。

---

## 第1课：自动化金字塔

*"大多数开发者在第1级自动化。钱在第3级。"*

### 四个层级

你收入栈中的每个自动化都落在这个金字塔的某个位置：

```
┌───────────────────────────────┐
│  第4级：自主代理               │  ← 替你做决策
│  (AI决定并行动)               │
├───────────────────────────────┤
│  第3级：智能流水线             │  ← 钱在这里
│  (LLM驱动)                   │
├───────────────────────────────┤
│  第2级：计划自动化             │  ← 大多数开发者止步于此
│  (cron + 脚本)               │
├───────────────────────────────┤
│  第1级：带模板的手动操作        │  ← 大多数开发者在这里
│  (复制粘贴)                   │
└───────────────────────────────┘
```

让我们具体看看每个层级在实践中是什么样的。

### 第1级：带模板的手动操作

你自己做工作，但你有清单、模板和代码片段来加速。

**示例：**
- 你使用预填了frontmatter的Markdown模板写博客文章
- 你复制上个月的发票并更改数字来向客户开票
- 你使用保存的回复模板来回复支持邮件
- 你手动运行部署命令来发布内容

**时间成本：** 每单位输出100%的时间。
**错误率：** 中等——你是人，疲惫时会犯错。
**扩展上限：** 你。你的工时。仅此而已。

大多数开发者住在这里，甚至没有意识到上面还有一个金字塔。

### 第2级：计划自动化

脚本按计划运行。你只写了一次逻辑。它无需你就能执行。

**示例：**
- 一个检查RSS源并将新文章发布到社交媒体的定时任务
- 一个每天早上6点构建和部署你网站的GitHub Action
- 一个每小时运行一次检查竞争对手定价并记录变化的脚本
- 一个凌晨3点运行的每日数据库备份

**时间成本：** 持续为零（初始设置1-4小时后）。
**错误率：** 低——确定性的，每次相同的逻辑。
**扩展上限：** 你的机器能调度的任务数。数百个。

这是大多数技术开发者到达的地方。很舒适。但它有一个硬性限制：它只能处理具有确定性逻辑的任务。如果任务需要判断，你就卡住了。

### 第3级：智能流水线

脚本按计划运行，但包含一个处理判断决策的LLM。

**示例：**
- RSS源被采集，LLM总结每篇文章，起草新闻通讯，你审查10分钟后点击发送
- 客户反馈邮件按情感和紧急程度分类，预起草的回复排队等待你批准
- 你所在领域的新职位被抓取，LLM评估相关性，你每天收到5个机会的摘要而不是浏览200条列表
- 竞争对手的博客文章被监控，LLM提取关键产品变更，你收到每周竞争情报报告

**时间成本：** 手动时间的10-20%。你审查和批准而不是创建。
**错误率：** 分类任务低，生成中等（这就是为什么你要审查）。
**扩展上限：** 每天数千个项目。你的瓶颈是API成本，不是你的时间。

**钱就在这里。** 第3级让一个人运营通常需要3-5人团队的收入流。

### 第4级：自主代理

AI系统在你不参与的情况下观察、决策和行动。

**示例：**
- 一个监控你SaaS指标的代理，检测到注册量下降，A/B测试价格变更，如果不奏效则回退
- 一个完全自主处理第一层客户问题的支持代理，只在复杂问题时升级到你
- 一个识别趋势话题、生成草稿、安排发布并监控表现的内容代理

**时间成本：** 已处理案例接近零。你审查指标，而不是单个操作。
**错误率：** 完全取决于你的护栏。没有护栏：高。有好的护栏：在狭窄领域出乎意料地低。
**扩展上限：** 在代理范围内的任务实际上是无限的。

第4级是真实可实现的，但不是你的起点。正如我们在第5课将讨论的，实现不佳的完全自主客户面对代理对你的声誉是危险的。

> **直说：** 如果你现在在第1级，不要试图跳到第4级。你会花几周时间构建一个在生产中崩溃并损害客户信任的"自主代理"。一次爬一级。第2级是一个下午的工作。第3级是一个周末项目。第4级是在你的第3级稳定运行一个月之后才到来的。

### 自我评估：你在哪里？

对你的每个收入流，诚实地评估自己：

| 收入流 | 当前级别 | 每周时间 | 可自动化到 |
|--------|---------|---------|----------|
| [例：新闻通讯] | [1-4] | [X] 小时 | [目标级别] |
| [例：客户处理] | [1-4] | [X] 小时 | [目标级别] |
| [例：社交媒体] | [1-4] | [X] 小时 | [目标级别] |
| [例：支持] | [1-4] | [X] 小时 | [目标级别] |

最重要的列是"每周时间"。花费时间最多且级别最低的流就是你的第一个自动化目标。那就是ROI最大的地方。

### 各级别的经济学

假设你有一个收入流，每周花费10小时，每月产生{= regional.currency_symbol | fallback("$") =}2,000：

| 级别 | 你的时间 | 你的有效时薪 | 自动化成本 |
|------|---------|------------|----------|
| 第1级 | 10小时/周 | $50/小时 | $0 |
| 第2级 | 3小时/周 | $167/小时 | $5/月 (VPS) |
| 第3级 | 1小时/周 | $500/小时 | $30-50/月 (API) |
| 第4级 | 0.5小时/周 | $1,000/小时 | $50-100/月 (API + 计算) |

从第1级到第3级不会改变你的收入。它把你的有效时薪从$50变成$500。而那些释放出来的9小时？用来构建下一个收入流或改善当前的。

> **常见错误：** 因为"更容易"而先自动化收入最低的流。不对。自动化那个相对于收入消耗最多时间的流。那才是ROI所在。

### 你的回合

1. 为你拥有的（或计划中的）每个收入流填写上面的自我评估表。
2. 确定你ROI最高的自动化目标：花费时间最多且自动化级别最低的流。
3. 写下该流中最耗时的3个任务。你将在第2课中自动化第一个。

---

## 第2课：从第1级到第2级 — 计划自动化

*"Cron来自1975年。它仍然有效。用它。"*

### 定时任务基础

{? if computed.os_family == "windows" ?}
你使用的是Windows，所以cron不是你系统的原生功能。你有两个选择：使用WSL（Windows Subsystem for Linux）来获得真正的cron，或使用Windows任务计划程序（下面介绍）。如果你熟悉WSL，推荐使用——本课中所有cron示例都可以直接在WSL中运行。如果你更喜欢原生Windows，跳到下面的任务计划程序部分。
{? endif ?}

没错，即使在2026年，cron仍然是计划任务之王。它可靠，无处不在，不需要云账户、SaaS订阅，或每次都要Google的YAML规范。

**30秒掌握cron语法：**

```
┌───────── 分钟 (0-59)
│ ┌───────── 小时 (0-23)
│ │ ┌───────── 日 (1-31)
│ │ │ ┌───────── 月 (1-12)
│ │ │ │ ┌───────── 星期 (0-7, 0和7 = 星期日)
│ │ │ │ │
* * * * *  命令
```

**常用计划：**

```bash
# 每小时
0 * * * *  /path/to/script.sh

# 每天早上6点
0 6 * * *  /path/to/script.sh

# 每周一早上9点
0 9 * * 1  /path/to/script.sh

# 每15分钟
*/15 * * * *  /path/to/script.sh

# 每月1日午夜
0 0 1 * *  /path/to/script.sh
```

**设置定时任务：**

```bash
# 编辑crontab
crontab -e

# 列出现有定时任务
crontab -l

# 关键：始终在顶部设置环境变量
# Cron以最小环境运行 — PATH可能不包含你的工具
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# 将输出记录到日志以便调试失败
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **常见错误：** 编写一个手动运行时完美工作的脚本，然后在cron中因为不加载`.bashrc`或`.zshrc`而静默失败。在cron脚本中始终使用绝对路径。始终在crontab顶部设置`PATH`。始终将输出重定向到日志文件。

### 当Cron不够用时的云调度器

如果你的机器不是24/7运行，或者你需要更健壮的方案，使用云调度器：

**GitHub Actions（公共仓库免费，私有仓库月2,000分钟）：**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # 每天UTC早上6点
    - cron: '0 6 * * *'
  # 允许手动触发测试
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

**Vercel Cron（Hobby计划免费每天1次；Pro计划：无限制）：**

```typescript
// api/cron/daily-report.ts
// Vercel cron端点 — 在vercel.json中配置计划

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // 验证确实是Vercel在调用，而不是随机的HTTP请求
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // 你的自动化逻辑在这里
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

### 现在就可以构建的实用自动化

这里有五个你今天就可以实现的自动化。每个需要30-60分钟，消除每周数小时的手动工作。

#### 自动化 1：按计划自动发布内容

你提前写好博客文章。这个脚本在预定时间发布它们。

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — 在预定日期发布Markdown文章。
通过cron每天运行: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# 你的CMS API端点 (Hashnode, Dev.to, Ghost, 等)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """加载已发布文章的文件名列表。"""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """保存已发布文章的文件名列表。"""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """从Markdown文件中提取YAML风格的frontmatter。"""
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
    """检查文章是否应该今天发布。"""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """将文章发布到CMS API。"""
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

**你的Markdown文章看起来像这样：**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

你的文章内容在这里...
```

灵感来了就写文章。设定日期。脚本处理剩下的。

#### 自动化 2：新内容发布时自动发到社交媒体

当你的博客发布新内容时，自动发布到Twitter/X和Bluesky。

```python
#!/usr/bin/env python3
"""
social_poster.py — 当新内容发布时发布到社交平台。
每30分钟运行: */30 * * * * python3 /path/to/social_poster.py
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
    """解析RSS源并返回项目列表。"""
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
    """通过AT Protocol发布到Bluesky。"""
    # 步骤1：创建会话
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

    # 步骤2：创建帖子
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

        # 格式化社交帖子
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky有300字符限制
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

成本：$0。在你的机器或免费的GitHub Action上运行。

#### 自动化 3：竞争对手价格监控

竞争对手改价的瞬间就知道。不再需要每周手动检查。

```python
#!/usr/bin/env python3
"""
price_monitor.py — 监控竞争对手定价页面的变化。
每6小时运行: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # 或Discord、邮件等

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # 用于全页监控；使用选择器监控特定元素
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """获取页面并返回其内容哈希和文本摘录。"""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # 抓取可见文本的前500个字符作为上下文
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
    """通过Slack webhook发送警报（可替换为Discord、邮件等）。"""
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

#### 自动化 4：每周收入报告

每周一早上，从你的收入数据生成报告并发邮件给你。

```python
#!/usr/bin/env python3
"""
weekly_report.py — 从你的追踪电子表格/数据库生成每周收入报告。
周一早上7点运行: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """如果不存在则创建revenue表。"""
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
    """生成纯文本的每周报告。"""
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
    """通过邮件发送报告。"""
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

#### 自动化 5：自动备份客户数据

永远不会丢失客户交付物。每晚运行，保留30天的备份。

```bash
#!/bin/bash
# backup_client_data.sh — 客户项目数据的夜间备份。
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# 创建压缩备份
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# 删除超过保留期的备份
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# 日志
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup complete: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# 可选：同步到第二个位置（外部硬盘、另一台机器）
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### 需要更多控制时用systemd定时器

如果你需要cron提供不了的东西——比如依赖排序、资源限制或自动重试——使用systemd定时器：

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
# 失败时指数退避重启
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
# 如果机器在早上6点关机，上线后运行
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# 启用并启动定时器
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# 检查状态
systemctl list-timers --all | grep income

# 查看日志
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Windows任务计划程序替代方案

如果你不使用WSL，Windows任务计划程序可以完成同样的工作。从命令行使用`schtasks`或使用任务计划程序GUI（`taskschd.msc`）。主要区别：cron使用单个表达式，任务计划程序对触发器、操作和条件使用单独的字段。本课中的每个cron示例都可以直接转换——用相同的方式调度你的Python脚本，只是通过不同的界面。
{? endif ?}

### 你的回合

1. 从本课中选择一个适用于你收入流的最简单的自动化。
2. 实现它。不是"计划实现它"。写代码，测试它，调度它。
3. 设置日志以便验证它在运行。连续3天每天早上检查日志。
4. 一旦稳定，停止每天检查。每周检查。这就是自动化。

**最低要求：** 今天结束前有一个稳定运行的定时任务。

---

## 第3课：从第2级到第3级 — LLM驱动的流水线

*"给你的自动化添加智能。这就是一个人开始看起来像一个团队的地方。"*

### 模式

每个LLM驱动的流水线都遵循相同的形状：

```
输入源 → 采集 → LLM处理 → 格式化输出 → 交付（或排队等待审查）
```

魔力在"LLM处理"步骤。你不需要为每种可能的情况编写确定性规则，而是用自然语言描述你想要什么，LLM处理判断决策。

### 何时使用本地 vs API

{? if settings.has_llm ?}
你已经配置了{= settings.llm_provider | fallback("LLM提供商") =}和{= settings.llm_model | fallback("你的LLM模型") =}。这意味着你可以立即开始构建智能流水线。下面的决策帮助你为每个流水线选择使用本地设置还是API。
{? else ?}
你还没有配置LLM。本课中的流水线可以使用本地模型（Ollama）和云API。在构建你的第一个流水线之前至少设置一个——Ollama是免费的，安装只需10分钟。
{? endif ?}

这个决策直接影响你的利润率：

| 因素 | 本地 (Ollama) | API (Claude, GPT) |
|------|-------------|-------------------|
| **每百万token成本** | ~$0.003（电费） | $0.15 - $15.00 |
| **速度（token/秒）** | 20-60（中端GPU上8B） | 50-100+ |
| **质量（本地8B vs API）** | 分类、提取较好 | 生成、推理更优 |
| **隐私** | 数据不离开你的机器 | 数据发送到提供商 |
| **可用性** | 取决于你的机器 | 99.9%+ |
| **批处理能力** | 受GPU内存限制 | 受速率限制和预算限制 |

{? if profile.gpu.exists ?}
你的机器上有{= profile.gpu.model | fallback("GPU") =}，本地推理是一个有力的选项。你能运行的速度和模型大小取决于你的VRAM——在承诺纯本地流水线之前检查适合什么。
{? if computed.has_nvidia ?}
NVIDIA GPU凭借CUDA加速获得最佳的Ollama性能。你应该能够舒适地运行7-8B参数模型，根据你的{= profile.gpu.vram | fallback("可用VRAM") =}可能还能运行更大的。
{? endif ?}
{? else ?}
没有专用GPU，本地推理会更慢（仅CPU）。对于小批量任务和分类任务仍然有效，但对于时间敏感或大批量的工作，API模型会更实际。
{? endif ?}

**经验法则：**
- **大量、较低质量要求**（分类、提取、标记）→ 本地
- **少量、质量关键**（面向客户的内容、复杂分析）→ API
- **敏感数据**（客户信息、专有数据）→ 始终本地
- **每月超过10,000项** → 本地省下真金白银

**典型流水线的月度成本比较：**

```
每月处理5,000项，每项约500 token:

本地 (Ollama, llama3.1:8b):
  2,500,000 token × $0.003/1M = $0.0075/月
  基本上免费。

API (GPT-4o-mini):
  2,500,000 输入token × $0.15/1M = $0.375
  2,500,000 输出token × $0.60/1M = $1.50
  总计: ~$1.88/月
  便宜，但是本地的250倍。

API (Claude 3.5 Sonnet):
  2,500,000 输入token × $3.00/1M = $7.50
  2,500,000 输出token × $15.00/1M = $37.50
  总计: ~$45/月
  质量很好，但是本地的6,000倍。
```

对于分类和提取流水线，一个提示词精心设计的8B本地模型和前沿API模型之间的质量差异通常可以忽略不计。两个都测试。使用满足你质量标准的更便宜的那个。

{@ insight cost_projection @}

### 流水线 1：新闻通讯内容生成器

这是基于内容收入的开发者最常见的LLM自动化。RSS源进去，新闻通讯草稿出来。

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — 采集RSS源，用LLM总结，生成新闻通讯草稿。
每天运行: 0 5 * * * python3 /path/to/newsletter_pipeline.py

该流水线:
1. 从多个RSS源获取新文章
2. 将每篇发送到本地LLM进行总结
3. 按与受众的相关性排名
4. 生成格式化的新闻通讯草稿
5. 保存草稿供你审查（你花10分钟审查，而不是2小时策划）
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
    # 在这里添加你的细分领域源
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
    """解析RSS/Atom源并返回文章。"""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # 处理RSS和Atom两种源
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
    """向本地LLM发送提示词并获取响应。"""
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
    """使用LLM对相关性评分并生成摘要。"""
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
        # 尝试从LLM输出中解析JSON
        # 处理LLM用markdown代码块包装的情况
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
    """将评分后的文章格式化为新闻通讯草稿。"""
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

    # 只筛选相关文章并按分数排序
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # 取前10篇
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} articles passed relevance threshold (>= 6/10)")

    # 生成新闻通讯草稿
    draft = generate_newsletter(top_articles)

    # 保存草稿
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

**这花费多少：**
- 用本地8B模型每天处理50篇文章：~$0/月
- 你的时间：10分钟审查草稿 vs 2小时手动策划
- 每周节省时间：如果你运行周刊通讯则~10小时

### 流水线 2：客户调研和洞察报告

这个流水线抓取公开数据，用LLM分析，生成你可以出售的报告。

```python
#!/usr/bin/env python3
"""
research_pipeline.py — 分析公司/产品的公开数据并生成洞察报告。
这是一项你可以出售的服务：每份定制报告$200-500。

用法: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# 付费报告使用更大的模型以保证质量
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# 或使用API获得面向客户的质量:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """根据配置路由到本地或API模型。"""
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
    """收集关于公司的公开可用数据。"""
    data = {"company": company, "domain": domain}

    # 检查域名是否可达并获取基本信息
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

    # 检查GitHub存在
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
    """使用LLM生成分析报告。"""
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

    # 组装最终报告
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

**商业模式：** 每份定制调研报告收费$200-500。你的成本：$0.05的API调用和15分钟的审查。流水线稳定后，你每小时可以生产3-4份报告。

### 流水线 3：市场信号监控器

这是告诉你下一步该构建什么的流水线。它监控多个来源，分类信号，当机会超过你的阈值时发出警报。

```python
#!/usr/bin/env python3
"""
signal_monitor.py — 监控公开来源的市场机会。
每2小时运行: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# 你的细分领域定义 — LLM用它来评分相关性
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
    """获取Hacker News热门文章。"""
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
    """使用LLM将信号分类为市场机会。"""
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
    """为高分机会发送警报。"""
    msg = (
        f"OPPORTUNITY DETECTED (score: {item['opportunity_score']}/10)\n"
        f"Type: {item['opportunity_type']}\n"
        f"Title: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Why: {item['reasoning']}\n"
        f"Action: {item['action']}"
    )

    # 记录到文件
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # 发送到Slack/Discord
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERT: {msg}")

def main():
    seen = load_seen()

    # 从源获取
    print("Fetching signals...")
    items = fetch_hn_top(30)
    # 在这里添加更多源：Reddit、RSS源、GitHub趋势等

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} new signals to classify")

    # 分类每个信号
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

**这在实践中如何工作：** 你每周收到2-3次Slack通知，比如"机会：新框架发布但没有入门套件——你这个周末可以做一个"。在别人之前抓住那个信号并行动，这就是你保持领先的方式。

> **直说：** 这些流水线输出的质量完全取决于你的提示词和细分领域定义。如果你的细分领域很模糊（"我是一个Web开发者"），LLM会标记所有东西。如果它很具体（"我为隐私优先的开发者市场构建Tauri桌面应用"），它会像外科手术一样精确。花30分钟把你的细分领域定义弄对。它是你构建的每个流水线中最高杠杆的输入。

### 你的回合

{? if stack.contains("python") ?}
好消息：上面的流水线示例已经是你的主要语言了。你可以直接复制并开始适配。专注于把细分领域定义和提示词弄对——90%的输出质量来自这里。
{? else ?}
上面的示例为了可移植性使用Python，但这些模式在任何语言中都有效。如果你更喜欢用{= stack.primary | fallback("你的主要技术栈") =}构建，需要复制的关键部分是：用于RSS/API获取的HTTP客户端、用于LLM响应的JSON解析、用于状态管理的文件I/O。与LLM的交互只是对Ollama或云API的HTTP POST。
{? endif ?}

1. 从上面三个流水线中选一个（新闻通讯、调研或信号监控）。
2. 适配到你的细分领域。更改源、受众描述、分类标准。
3. 手动运行3次以测试输出质量。
4. 调整提示词直到输出无需大量编辑就可用。
5. 用cron调度。

**目标：** 阅读本课后48小时内有一个按计划运行的LLM驱动流水线。

---

## 第4课：从第3级到第4级 — 基于代理的系统

*"代理就是一个观察、决策和行动的循环。构建一个。"*

### 2026年"代理"的真正含义

去掉炒作。代理是一个这样的程序：

1. **观察** — 读取某些输入或状态
2. **决策** — 使用LLM确定要做什么
3. **行动** — 执行决策
4. **循环** — 回到步骤1

就这些。流水线（第3级）和代理（第4级）之间的区别是代理会循环。它对自己的输出采取行动。它处理下一步取决于上一步结果的多步任务。

流水线按固定序列逐一处理项目。代理根据遇到的情况导航不可预测的序列。

### 为客户服务的MCP服务器

MCP服务器是你能构建的最实用的代理相邻系统之一。它暴露AI代理（Claude Code、Cursor等）可以代表你的客户调用的工具。

{? if stack.contains("typescript") ?}
下面的MCP服务器示例使用TypeScript——正好是你的强项。你可以用现有的TypeScript工具扩展它，并与其他Node.js服务一起部署。
{? endif ?}

这是一个真实的例子：一个从你产品文档回答客户问题的MCP服务器。

```typescript
// mcp-docs-server/src/index.ts
// 从你的文档回答问题的MCP服务器。
// 你的客户将Claude Code指向这个服务器就能获得即时答案。

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// 启动时将文档加载到内存
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

    // 按标题分割以实现更好的搜索
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
  // 简单的关键词搜索 — 生产环境请替换为向量搜索
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // 标题匹配加分
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

// 初始化
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

// 启动服务器
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

**商业模式：** 将这个MCP服务器作为产品的一部分提供给客户。他们不用提交支持工单就能即时获得答案。你花在支持上的时间更少。所有人都赢了。

高级版：收费$9-29/月提供托管版本，包含向量搜索、版本化文档和客户在问什么的分析。

### 自动化客户反馈处理

这个代理读取客户反馈（来自邮件、支持工单或表单），分类它，并创建回复草稿和功能工单。

```python
#!/usr/bin/env python3
"""
feedback_agent.py — 将客户反馈处理为分类的、可行动的项目。
"AI起草，人工批准"模式。

每小时运行: 0 * * * * python3 /path/to/feedback_agent.py
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
    """分类反馈并生成回复草稿。"""

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

        # 保存处理后的版本
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # 添加到审查队列
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # 将原始文件移出收件箱
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # 写入审查队列
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # 摘要
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nProcessed: {len(review_queue)}")
    print(f"Critical: {critical}")
    print(f"Needs your attention: {needs_human}")
    print(f"Review queue: {review_path}")

if __name__ == "__main__":
    main()
```

**这在实践中如何工作：**
1. 客户提交反馈（通过表单、邮件或支持系统）
2. 反馈作为JSON文件到达收件箱目录
3. 代理处理每一个：分类、总结、起草回复
4. 你每天打开审查队列一到两次
5. 简单的项目（表扬、有好的回复草稿的基本问题），你批准草稿
6. 复杂的项目（bug、愤怒的客户），你写个人回复
7. 净时间：每天15分钟而不是2小时

### AI起草，人工批准模式

这个模式是实用第4级自动化的核心。代理处理繁重工作。你处理判断决策。

```
              ┌─────────────┐
              │ 代理起草     │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │  审查队列    │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ 自动发送  │ │编辑 │ │ 升级    │
    │ (常规)    │ │+发送│ │ (复杂)  │
    └───────────┘ └─────┘ └─────────┘
```

**代理完全处理 vs 你审查后再发送的规则：**

| 代理完全处理（无需审查） | 发送前你审查 |
|------------------------|------------|
| 确认收据（"我们收到了你的消息"） | 给愤怒客户的回复 |
| 状态更新（"你的请求正在处理中"） | 功能请求优先级排序 |
| FAQ回答（精确匹配） | 涉及金钱的任何事（退款、定价） |
| 垃圾邮件分类和删除 | Bug报告（你需要验证） |
| 内部日志和分类 | 你从未见过的任何事 |

> **常见错误：** 从第一天就让代理自主回复客户。别这么做。从代理起草所有内容、你批准所有内容开始。一周后，让它自动发送确认。一个月后，让它自动发送FAQ回复。逐步建立信任——对你自己和对你的客户。

### 你的回合

1. 选一个：构建MCP文档服务器或反馈处理代理。
2. 适配到你的产品/服务。如果你还没有客户，使用第3课的信号监控器作为你的"客户"——通过反馈代理模式处理它的输出。
3. 用不同的输入手动运行10次。
4. 测量：输出中有多少百分比不用编辑就能用？那就是你的自动化质量分数。调度前目标70%以上。

---

## 第5课：人在回路中原则

*"完全自动化是陷阱。部分自动化是超能力。"*

### 为什么80%的自动化胜过100%

有一个具体的、可测量的原因说明你永远不应该完全自动化面向客户的流程：坏输出的成本是不对称的。

一个好的自动化输出节省你5分钟。
一个坏的自动化输出让你失去一个客户、一条公开投诉、一次退款，或者需要数月才能恢复的声誉打击。

算一下：

```
100%自动化:
  每月1,000个输出 × 95%质量 = 950个好的 + 50个坏的
  50个坏输出 × 平均$50成本（退款 + 支持 + 声誉）= 每月$2,500的损害

80%自动化 + 20%人工审查:
  800个自动处理，200个人工审查
  800 × 95%质量 = 760个好的 + 40个坏的自动
  200 × 99%质量 = 198个好的 + 2个坏的人工
  总共42个坏的 × $50 = 每月$2,100的损害
  但是：你在到达客户之前捕获了38个坏的

  实际到达客户的坏输出：~4个
  实际损害：~每月$200
```

损害成本减少了12倍。你审查200个输出的时间（大概2小时）每月节省$2,300。

### 绝对不要完全自动化这些

无论AI变得多好，有些事情应该始终有人在回路中：

1. **面向客户的沟通** — 一封措辞不当的邮件可以永远失去一个客户。一个通用的、明显是AI的回复会侵蚀信任。审查它。

2. **金融交易** — 退款、价格变更、开发票。始终审查。错误的代价是真金白银。

3. **以你的名字发布的内容** — 你的声誉经过多年复利增长，可以被一篇糟糕的帖子摧毁。10分钟的审查是廉价的保险。

4. **法律或合规相关的输出** — 任何涉及合同、隐私政策、服务条款的内容。AI会自信满满地犯法律错误。

5. **招聘或人事决策** — 如果你外包，永远不要让AI对与谁合作做最终决定。

### 自动化债务

{@ mirror automation_risk_profile @}

自动化债务比技术债务更糟糕，因为它在爆发之前是看不见的。

**自动化债务的样子：**
- 一个因为时区变了而在错误时间发帖的社交媒体机器人
- 一个因为没人检查而持续3周包含坏链接的新闻通讯流水线
- 一个因为竞争对手重新设计页面而停止工作的价格监控
- 一个因为磁盘满了而静默失败的备份脚本

**如何预防：**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — 监控所有自动化的静默故障。
每天早上运行: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# 定义每个自动化的预期输出
AUTOMATIONS = [
    {
        "name": "Newsletter Pipeline",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # 应该至少每天产出
    },
    {
        "name": "Social Poster",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # 直接检查文件
        "max_age_hours": 2,  # 应该每30分钟更新
    },
    {
        "name": "Competitor Monitor",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # 应该每6小时运行
    },
    {
        "name": "Client Backup",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # 应该每晚运行
    },
    {
        "name": "Ollama Server",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """检查自动化是否产出了最近的输出。"""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # 检查匹配模式的最近文件
        p = Path(path)
        if not p.exists():
            return False, f"Directory not found: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"No files matching {automation['pattern']} in {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # 直接检查文件
        if not os.path.exists(path):
            return False, f"File not found: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Last output {age.total_seconds()/3600:.1f}h ago (max: {automation['max_age_hours']}h)"

    return True, f"OK (last output {age.total_seconds()/3600:.1f}h ago)"

def check_http(automation: dict) -> tuple[bool, str]:
    """检查服务是否在响应。"""
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

每天早上运行这个。当自动化静默故障时（一定会的），你会在24小时内而不是3周后知道。

### 构建审查队列

使人在回路中高效的关键是批量审查。不要在项目到达时逐一审查。排队并批量审查。

```python
#!/usr/bin/env python3
"""
review_queue.py — 用于AI生成输出的简单审查队列。
每天审查一到两次而不是持续检查。
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """向审查队列添加一个项目。"""
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
    """显示所有待审查的项目。"""
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

    # 在实际实现中，你会在这里添加交互式输入
    # 对于批处理，从文件或简单CLI读取决策

if __name__ == "__main__":
    review_queue()
```

**审查习惯：** 在早上8点和下午4点检查你的审查队列。两次会话，每次10-15分钟。其他一切在审查之间自主运行。

> **直说：** 想想如果你跳过人工审查会发生什么：你完全自动化了你的新闻通讯，LLM开始插入指向不存在页面的幻觉链接，订阅者比你更先发现。你失去一部分订阅列表，重建信任需要几个月。相比之下，自动化同一流程80%的开发者——LLM策划和起草，他们花10分钟审查——在发送前捕获了那些幻觉。区别不在于自动化。而在于审查步骤。

### 你的回合

1. 为你在第2课和第3课中构建的任何自动化设置`automation_healthcheck.py`脚本。安排它每天早上运行。
2. 为你风险最高的自动化输出（任何面向客户的）实现一个审查队列。
3. 承诺连续一周每天检查两次审查队列。记录你不加修改批准了多少，编辑了多少，拒绝了多少。这些数据告诉你你的自动化实际上有多好。

---

## 第6课：成本优化和你的第一个流水线

*"如果你不能从$200的API支出中产生$200的收入，修复产品——而不是预算。"*

### LLM驱动自动化的经济学

每次LLM调用都有成本。即使是本地模型也有电费和GPU磨损。问题是那次调用的输出是否产生了比调用成本更多的价值。

{? if profile.gpu.exists ?}
在{= profile.gpu.model | fallback("你的GPU") =}上运行本地模型，典型的流水线工作负载大约每月{= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("几美元") =}的电费。这是与API替代方案比较的基准。
{? endif ?}

**每月{= regional.currency_symbol | fallback("$") =}200的API预算规则：**

如果你每月在自动化的API调用上花费{= regional.currency_symbol | fallback("$") =}200，那些自动化应该产生至少每月{= regional.currency_symbol | fallback("$") =}200的价值——要么是直接收入，要么是你在其他地方转换为收入的时间节省。

如果没有：问题不在于API预算。而在于流水线设计或它支持的产品。

### 每输出成本追踪

在你构建的每个流水线中添加这个：

```python
"""
cost_tracker.py — 追踪每次LLM调用的成本和它产生的价值。
在你的流水线中导入这个以获取真实成本数据。
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# 每百万token的定价（价格变化时更新）
PRICING = {
    # 本地模型 — 电费估算
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # API模型
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
    """记录LLM调用的成本。"""
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
    """生成月度成本/收入摘要。"""
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

    # 打印报告
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

### 为API效率而批处理

如果你使用API模型，批处理能省下真金白银：

```python
"""
batch_api.py — 批量API调用以提高效率。
用批处理替代100次单独的API调用。
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
    通过将多个项目批量到单次API调用中来高效分类。

    不用100次API调用（100项 × 每项1次）：
      - 100次 × ~500输入token = 50,000 token输入
      - 100次 × ~200输出token = 20,000 token输出
      - Haiku成本: ~$0.12

    用批处理（每次10项，10次API调用）：
      - 10次 × ~2,500输入token = 25,000 token输入
      - 10次 × ~1,000输出token = 10,000 token输出
      - Haiku成本: ~$0.06

    仅批处理就节省50%。
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # 将批次格式化为单个提示词
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
            # 从响应中解析JSON数组
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  Batch {i//batch_size + 1} failed: {e}")
            # 回退到单独处理
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # 速率限制礼貌
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### 缓存：别为同一个答案付两次钱

```python
"""
llm_cache.py — 缓存LLM响应以避免为重复处理付费。
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
    """从模型 + 提示词生成确定性缓存键。"""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """如果缓存响应可用且新鲜则获取。"""
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

    # 更新命中计数
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """缓存一个响应。"""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """显示缓存统计信息。"""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Cache entries: {total}")
    print(f"Total cache hits: {total_hits}")
    print(f"Estimated savings: ~${total_hits * 0.002:.2f} (rough avg per call)")
```

**在你的流水线中使用：**

```python
# 在任何调用LLM的流水线中：
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # 免费！

    response = call_llm(model, prompt)  # 你现有的LLM调用函数
    set_cached(model, prompt, response)
    return response
```

对于重复处理同类内容的流水线（分类、提取），缓存可以消除30-50%的API调用。你的月账单打七到五折。

### 构建你的第一个完整流水线：分步指南

从"我有一个手动工作流"到"它在我睡觉时运行"的完整过程在这里。

**步骤 1：映射你当前的手动流程。**

写下你为一个特定收入流做的每一步。新闻通讯的例子：

```
1. 在浏览器中打开15个RSS源 (10分钟)
2. 扫描标题，打开有趣的 (20分钟)
3. 详细阅读8-10篇文章 (40分钟)
4. 为前5篇写摘要 (30分钟)
5. 写开头段落 (10分钟)
6. 在邮件工具中格式化 (15分钟)
7. 发送到列表 (5分钟)

总计：约2小时10分钟
```

**步骤 2：确定最耗时的3个步骤。**

从例子中：阅读文章（40分钟），写摘要（30分钟），扫描标题（20分钟）。

**步骤 3：先自动化最简单的那个。**

扫描标题最容易自动化——它是分类。LLM对相关性评分，你只读评分最高的。

**步骤 4：衡量节省的时间和质量。**

自动化标题扫描后：
- 节省的时间：20分钟
- 质量：与你手动选择90%一致
- 净结果：节省20分钟，质量损失可以忽略

**步骤 5：自动化下一步。**

现在自动化摘要写作。LLM起草摘要，你编辑它们。

**步骤 6：一直继续直到收益递减。**

```
自动化前：每份新闻通讯2小时10分钟
第2级后（计划获取）：1小时45分钟
第3级后（LLM评分 + 摘要）：25分钟
第3级+后（LLM起草开头）：仅10分钟审查

每周节省时间：约2小时
每月节省时间：约8小时
按$100/小时有效时薪：释放时间价值月$800
API成本：$0（本地LLM）到$5/月（API）
```

**步骤 7：完整的流水线，连接起来。**

将周刊新闻通讯流水线的一切整合在一起的GitHub Action：

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # 每周日UTC早上5点
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

每周日早上5点运行。等你醒来时草稿已经在等你了。你一边喝咖啡一边花10分钟审查，按发送，你这周的新闻通讯就发布了。

### 你的回合：构建你的流水线

这是模块的交付物。到本课结束时，你应该有一个完整的流水线已部署并运行。

**流水线要求：**
1. 无需你参与按计划运行
2. 包含至少一个LLM处理步骤
3. 有一个用于质量控制的人工审查步骤
4. 有一个健康检查，让你知道它是否坏了
5. 连接到一个真实的收入流（或你正在构建的流）

**检查清单：**

- [ ] 选择了要自动化的收入流
- [ ] 映射了手动流程（所有步骤，附时间估算）
- [ ] 确定了最耗时的3个步骤
- [ ] 至少自动化了第一步（分类/评分/筛选）
- [ ] 为第二步添加了LLM处理（总结/生成/提取）
- [ ] 构建了用于人工监督的审查队列
- [ ] 为自动化设置了健康检查
- [ ] 部署到了计划调度（cron、GitHub Actions或systemd定时器）
- [ ] 追踪了一个完整周期的成本和时间节省
- [ ] 记录了流水线文档（它做什么、如何修复、要监控什么）

如果你完成了这个检查清单上的所有十项，你就有了一个第3级自动化在运行。你刚刚释放了每周数小时的时间，可以重新投资到构建更多流或改善现有的。

---

## 模块 T：完成

{@ temporal automation_progress @}

### 这两周你构建了什么

1. **对自动化金字塔的理解** — 你知道自己在哪里，以及你的每个收入流应该往哪个方向发展。
2. **在cron或云调度器上运行的计划自动化** — 使其他一切成为可能的不起眼的基础。
3. **处理你以前手动做的判断的LLM驱动流水线** — 分类、总结、生成、监控。
4. **你可以部署用于客户交互、反馈处理和MCP驱动产品的基于代理的模式**。
5. **保护你声誉同时节省80%以上时间的人在回路中框架**。
6. **成本追踪和优化**，使你的自动化产生利润而不仅仅是活动。
7. **一个完整部署的、无需你主动参与就能产生价值的流水线**。

### 复利效应

如果你在接下来3个月维护和扩展你在这个模块中构建的东西，会发生这些：

```
第1月：一个流水线，每周节省5-8小时
第2月：两个流水线，每周节省10-15小时
第3月：三个流水线，每周节省15-20小时

按$100/小时有效时薪，那是每月$1,500-2,000
的释放时间——你投资到新流上的时间。

第1月释放的时间构建第2月的流水线。
第2月释放的时间构建第3月的流水线。
自动化是复利增长的。
```

这就是一个开发者如何像五人团队一样运营。不是更努力地工作。而是构建在你不工作时工作的系统。

---

### 4DA 集成

{? if dna.identity_summary ?}
基于你的开发者档案 — {= dna.identity_summary | fallback("你的开发重点") =} — 下面的4DA工具直接映射到你刚学到的自动化模式。信号分类工具对你领域的开发者特别相关。
{? endif ?}

4DA本身就是一个第3级自动化。它从数十个来源采集内容，用PASIFA算法为每个项目评分，只展示与你工作相关的——而你不用动一根手指。你不用手动检查Hacker News、Reddit和50个RSS源。4DA做这些并展示重要的内容。

用同样的方式构建你的收入流水线。

4DA的注意力报告（MCP工具中的`/attention_report`）展示你的时间实际去了哪里 vs 应该去哪里。在决定要自动化什么之前运行它。"花费的时间"和"应该花费的时间"之间的差距就是你的自动化路线图。

信号分类工具（`/get_actionable_signals`）可以直接馈入你的市场监控流水线——在你的定制流水线进行细分领域特定分析之前，让4DA的智能层做初始评分。

如果你正在构建监控来源寻找机会的流水线，不要重新发明4DA已经做的事。将它的MCP服务器用作你自动化栈中的构建块。

---

### 下一步：模块 S — 叠加收入流

模块T给了你使每个收入流高效运行的工具。模块S（叠加收入流）回答下一个问题：**你应该运行多少个流，它们如何组合在一起？**

模块S涵盖的内容：

- **收入流的投资组合理论** — 为什么3个流胜过1个流，为什么10个流胜过0个
- **流的相关性** — 哪些流互相增强，哪些竞争你的时间
- **收入底线** — 在你实验之前建立覆盖成本的经常性收入基础
- **再平衡** — 什么时候加倍投入赢家，什么时候砍掉表现不佳的
- **月$10K的架构** — 用每周15-20小时达到五位数的具体流组合

你有了基础设施（模块S）、护城河（模块T）、引擎（模块R）、发布手册（模块E）、趋势雷达（模块E）和自动化（模块T）。模块S将它们全部绑定为一个可持续的、不断增长的收入组合。

---

**流水线运行。草稿准备好了。你花10分钟审查。**

**这就是战术自动化。这就是你扩展的方式。**

*你的设备。你的规则。你的收入。*
