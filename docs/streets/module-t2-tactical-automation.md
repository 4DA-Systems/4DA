# Module T: Tactical Automation

**STREETS Developer Income Course — Paid Module**
*Weeks 12-13 | 6 Lessons | Deliverable: One Automated Pipeline Generating Value*

> "LLMs, agents, MCP, and cron jobs as force multipliers."

---

You have revenue engines running. You have customers. You have processes that work. And you're spending 60-70% of your time doing the same things over and over: processing inputs, formatting outputs, checking monitors, sending updates, reviewing queues.

That time is your most expensive resource, and you're burning it on tasks a $5/month VPS could handle.

This module is about systematically removing yourself from the loop — not completely (that's a trap we'll cover in Lesson 5), but from the 80% of work that doesn't require your judgment. The result: your income streams produce revenue while you sleep, while you're at your day job, while you're building the next thing.

By the end of these two weeks, you will have:

- A clear understanding of the four levels of automation and where you sit today
- Working cron jobs and scheduled automations running on your infrastructure
- At least one LLM-powered pipeline processing inputs without your involvement
- An understanding of agent-based systems and when they make economic sense
- A human-in-the-loop framework so automation doesn't destroy your reputation
- One complete, deployed pipeline that generates value without your active involvement

This is the most code-heavy module in the course. At least half of what follows is runnable code. Copy it, adapt it, deploy it.

Let's automate.

---

## Lesson 1: The Automation Pyramid

*"Most developers automate at Level 1. The money is at Level 3."*

### The Four Levels

Every automation in your income stack falls somewhere on this pyramid:

```
┌───────────────────────────────┐
│  Level 4: Autonomous Agents   │  ← Makes decisions for you
│  (AI decides AND acts)        │
├───────────────────────────────┤
│  Level 3: Intelligent         │  ← The money is here
│  Pipelines (LLM-powered)     │
├───────────────────────────────┤
│  Level 2: Scheduled           │  ← Most developers stop here
│  Automation (cron + scripts)  │
├───────────────────────────────┤
│  Level 1: Manual with         │  ← Where most developers are
│  Templates (copy-paste)       │
└───────────────────────────────┘
```

Let's be specific about what each level looks like in practice.

### Level 1: Manual with Templates

You do the work, but you've got checklists, templates, and snippets to speed things up.

**Examples:**
- You write a blog post using a markdown template with pre-filled frontmatter
- You invoice clients by duplicating last month's invoice and changing the numbers
- You respond to support emails using saved replies
- You publish content by manually running a deploy command

**Time cost:** 100% of your time per unit of output.
**Error rate:** Moderate — you're human, you make mistakes when tired.
**Scale ceiling:** You. Your hours. That's it.

Most developers live here and don't even realize there's a pyramid above them.

### Level 2: Scheduled Automation

Scripts run on schedules. You wrote the logic once. It executes without you.

**Examples:**
- A cron job that checks your RSS feed and posts new articles to social media
- A GitHub Action that builds and deploys your site every morning at 6 AM
- A script that runs every hour to check competitor pricing and log changes
- A daily database backup that runs at 3 AM

**Time cost:** Zero ongoing (after initial setup of 1-4 hours).
**Error rate:** Low — deterministic, same logic every time.
**Scale ceiling:** As many tasks as your machine can schedule. Hundreds.

This is where most technical developers land. It's comfortable. But it has a hard limit: it can only handle tasks with deterministic logic. If the task requires judgment, you're stuck.

### Level 3: Intelligent Pipelines

Scripts run on schedules, but they include an LLM that handles the judgment calls.

**Examples:**
- RSS feeds are ingested, LLM summarizes each article, drafts a newsletter, you review for 10 minutes and hit send
- Customer feedback emails are classified by sentiment and urgency, pre-drafted responses are queued for your approval
- New job postings in your niche are scraped, LLM evaluates relevance, you get a daily digest of 5 opportunities instead of scanning 200 listings
- Competitor blog posts are monitored, LLM extracts key product changes, you get a weekly competitive intelligence report

**Time cost:** 10-20% of manual time. You review and approve instead of create.
**Error rate:** Low for classification tasks, moderate for generation (which is why you review).
**Scale ceiling:** Thousands of items per day. Your bottleneck is API cost, not your time.

**This is where the money is.** Level 3 lets one person operate income streams that would normally require a team of 3-5 people.

### Level 4: Autonomous Agents

AI systems that observe, decide, and act without your involvement.

**Examples:**
- An agent that monitors your SaaS metrics, detects a drop in signups, A/B tests a pricing change, and reverts if it doesn't work
- A support agent that handles Tier 1 customer questions fully autonomously, only escalating to you for complex issues
- A content agent that identifies trending topics, generates drafts, schedules publication, and monitors performance

**Time cost:** Near zero for handled cases. You review metrics, not individual actions.
**Error rate:** Depends entirely on your guardrails. Without them: high. With good guardrails: surprisingly low for narrow domains.
**Scale ceiling:** Effectively unlimited for the tasks within the agent's scope.

Level 4 is real and achievable, but it's not where you start. And as we'll cover in Lesson 5, fully autonomous customer-facing agents are dangerous for your reputation if poorly implemented.

> **Real Talk:** If you're at Level 1 right now, don't try to jump to Level 4. You'll spend weeks building an "autonomous agent" that breaks in production and damages customer trust. Climb the pyramid one level at a time. Level 2 is one afternoon of work. Level 3 is a weekend project. Level 4 comes after you've had Level 3 running reliably for a month.

### Self-Assessment: Where Are You?

For each of your income streams, rate yourself honestly:

| Income Stream | Current Level | Hours/Week Spent | Could Automate To |
|---------------|--------------|-----------------|-------------------|
| [e.g., Newsletter] | [1-4] | [X] hrs | [target level] |
| [e.g., Client processing] | [1-4] | [X] hrs | [target level] |
| [e.g., Social media] | [1-4] | [X] hrs | [target level] |
| [e.g., Support] | [1-4] | [X] hrs | [target level] |

The column that matters most is "Hours/Week Spent." The stream with the highest hours and lowest level is your first automation target. That's the one with the biggest ROI.

### The Economics of Each Level

Let's say you have an income stream that takes 10 hours/week of your time and generates $2,000/month:

| Level | Your Time | Your Effective Rate | Automation Cost |
|-------|----------|-------------------|----------------|
| Level 1 | 10 hrs/week | $50/hr | $0 |
| Level 2 | 3 hrs/week | $167/hr | $5/month (VPS) |
| Level 3 | 1 hr/week | $500/hr | $30-50/month (API) |
| Level 4 | 0.5 hrs/week | $1,000/hr | $50-100/month (API + compute) |

Moving from Level 1 to Level 3 doesn't change your revenue. It changes your effective hourly rate from $50 to $500. And those 9 freed hours? They go to building the next income stream or improving the current one.

> **Common Mistake:** Automating your lowest-revenue stream first because it's "easier." No. Automate the stream that eats the most hours relative to its revenue. That's where the ROI is.

### Your Turn

1. Fill in the self-assessment table above for every income stream (or planned stream) you have.
2. Identify your highest-ROI automation target: the stream with the most hours and the lowest automation level.
3. Write down the 3 most time-consuming tasks in that stream. You'll automate the first one in Lesson 2.

---

## Lesson 2: Level 1 to 2 — Scheduled Automation

*"Cron is from 1975. It still works. Use it."*

### Cron Job Fundamentals

Yes, even in 2026, cron is king for scheduled tasks. It's reliable, it's everywhere, and it doesn't require a cloud account, a SaaS subscription, or a YAML schema you have to Google every time.

**The cron syntax in 30 seconds:**

```
┌───────── minute (0-59)
│ ┌───────── hour (0-23)
│ │ ┌───────── day of month (1-31)
│ │ │ ┌───────── month (1-12)
│ │ │ │ ┌───────── day of week (0-7, 0 and 7 = Sunday)
│ │ │ │ │
* * * * *  command
```

**Common schedules:**

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

**Setting up a cron job:**

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

> **Common Mistake:** Writing a script that works perfectly when you run it manually, then it silently fails in cron because cron doesn't load your `.bashrc` or `.zshrc`. Always use absolute paths in cron scripts. Always set `PATH` at the top of your crontab. Always redirect output to a log file.

### Cloud Schedulers for When Cron Isn't Enough

If your machine isn't on 24/7, or you need something more robust, use a cloud scheduler:

**GitHub Actions (free for public repos, 2,000 min/month on private):**

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

**Vercel Cron (free on Hobby plan, 1 per day; Pro plan: unlimited):**

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

### Real Automations to Build Right Now

Here are five automations you can implement today. Each one takes 30-60 minutes and eliminates hours of weekly manual work.

#### Automation 1: Auto-Publish Content on Schedule

You write blog posts in advance. This script publishes them at the scheduled time.

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

**Your markdown posts look like this:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

Your post content here...
```

Write posts when inspiration hits. Set the date. The script handles the rest.

#### Automation 2: Auto-Post to Social Media on New Content

When your blog publishes something new, this posts to Twitter/X and Bluesky automatically.

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

Cost: $0. Runs on your machine or a free GitHub Action.

#### Automation 3: Competitor Price Monitor

Know the instant a competitor changes their pricing. No more manually checking every week.

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

#### Automation 4: Weekly Revenue Report

Every Monday morning, this generates a report from your revenue data and emails it to you.

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

#### Automation 5: Auto-Backup Client Data

Never lose client deliverables. This runs nightly and keeps 30 days of backups.

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

### Systemd Timers for More Control

If you need more than cron offers — like dependency ordering, resource limits, or automatic retry — use systemd timers:

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

### Your Turn

1. Pick the simplest automation from this lesson that applies to your income stream.
2. Implement it. Not "plan to implement it." Write the code, test it, schedule it.
3. Set up logging so you can verify it's running. Check the logs every morning for 3 days.
4. Once it's stable, stop checking daily. Check weekly. That's automation.

**Minimum:** One cron job running reliably by the end of today.

---

## Lesson 3: Level 2 to 3 — LLM-Powered Pipelines

*"Add intelligence to your automations. This is where one person starts to look like a team."*

### The Pattern

Every LLM-powered pipeline follows the same shape:

```
Input Sources → Ingest → LLM Process → Format Output → Deliver (or Queue for Review)
```

The magic is in the "LLM Process" step. Instead of writing deterministic rules for every possible case, you describe what you want in natural language, and the LLM handles the judgment calls.

### When to Use Local vs API

This decision has a direct impact on your margins:

| Factor | Local (Ollama) | API (Claude, GPT) |
|--------|---------------|-------------------|
| **Cost per 1M tokens** | ~$0.003 (electricity) | $0.15 - $15.00 |
| **Speed (tokens/sec)** | 20-60 (8B on mid-range GPU) | 50-100+ |
| **Quality (8B local vs API)** | Good for classification, extraction | Better for generation, reasoning |
| **Privacy** | Data never leaves your machine | Data goes to provider |
| **Uptime** | Depends on your machine | 99.9%+ |
| **Batch capacity** | Limited by GPU memory | Limited by rate limits and budget |

**Rules of thumb:**
- **High volume, lower quality bar** (classification, extraction, tagging) → Local
- **Low volume, quality-critical** (customer-facing content, complex analysis) → API
- **Sensitive data** (client info, proprietary data) → Local, always
- **More than 10,000 items/month** → Local saves real money

**Monthly cost comparison for a typical pipeline:**

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

For classification and extraction pipelines, the quality difference between a well-prompted 8B local model and a frontier API model is often negligible. Test both. Use the cheaper one that meets your quality bar.

### Pipeline 1: Newsletter Content Generator

This is the most common LLM automation for developers with content-based income. RSS feeds go in, a draft newsletter comes out.

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

**What this costs:**
- Processing 50 articles/day with a local 8B model: ~$0/month
- Your time: 10 minutes reviewing the draft vs 2 hours manually curating
- Time saved per week: ~10 hours if you run a weekly newsletter

### Pipeline 2: Customer Research and Insight Reports

This pipeline scrapes public data, analyzes it with an LLM, and produces a report you can sell.

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

**Business model:** Charge $200-500 per custom research report. Your cost: $0.05 in API calls and 15 minutes of review. You can produce 3-4 reports per hour once the pipeline is stable.

### Pipeline 3: Market Signal Monitor

This is the pipeline that tells you what to build next. It monitors multiple sources, classifies signals, and alerts you when an opportunity crosses your threshold.

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

**What this does in practice:** You get a Slack notification 2-3 times per week saying something like "OPPORTUNITY: New framework released with no starter kit — you could build one this weekend." That signal, acting on it before others, is how you stay ahead.

> **Real Talk:** The quality of these pipeline outputs depends entirely on your prompts and your niche definition. If your niche is vague ("I'm a web developer"), the LLM will flag everything. If it's specific ("I build Tauri desktop apps for the privacy-first developer market"), it'll be surgically precise. Spend 30 minutes getting your niche definition right. It's the single highest-leverage input to every pipeline you build.

### Your Turn

1. Choose one of the three pipelines above (newsletter, research, or signal monitor).
2. Adapt it to your niche. Change the feeds, the audience description, the classification criteria.
3. Run it manually 3 times to test the output quality.
4. Tune the prompts until the output is useful without heavy editing.
5. Schedule it with cron.

**Target:** One LLM-powered pipeline running on schedule within 48 hours of reading this lesson.

---

## Lesson 4: Level 3 to 4 — Agent-Based Systems

*"An agent is just a loop that observes, decides, and acts. Build one."*

### What "Agent" Actually Means in 2026

Strip away the hype. An agent is a program that:

1. **Observes** — reads some input or state
2. **Decides** — uses an LLM to determine what to do
3. **Acts** — executes the decision
4. **Loops** — goes back to step 1

That's it. The difference between a pipeline (Level 3) and an agent (Level 4) is that the agent loops. It acts on its own output. It handles multi-step tasks where the next step depends on the result of the previous one.

A pipeline processes items one at a time through a fixed sequence. An agent navigates an unpredictable sequence based on what it encounters.

### MCP Servers That Serve Customers

An MCP server is one of the most practical agent-adjacent systems you can build. It exposes tools that an AI agent (Claude Code, Cursor, etc.) can call on behalf of your customers.

Here's a real example: an MCP server that answers customer questions from your product's documentation.

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

**Business model:** Give this MCP server to your customers as part of your product. They get instant answers to their questions without filing support tickets. You spend less time on support. Everyone wins.

For premium: charge $9-29/month for a hosted version with vector search, versioned docs, and analytics on what customers are asking about.

### Automated Customer Feedback Processing

This agent reads customer feedback (from email, support tickets, or a form), classifies it, and creates draft responses and feature tickets.

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

**How this works in practice:**
1. Customers submit feedback (via form, email, or support system)
2. Feedback lands as JSON files in the inbox directory
3. Agent processes each one: classifies, summarizes, drafts a response
4. You open the review queue once or twice a day
5. For simple items (praise, basic questions with good draft responses), you approve the draft
6. For complex items (bugs, angry customers), you write a personal response
7. Net time: 15 minutes per day instead of 2 hours

### The AI Draft, Human Approve Pattern

This pattern is the core of practical Level 4 automation. The agent handles the grunt work. You handle the judgment calls.

```
              ┌─────────────┐
              │ Agent drafts │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ Review Queue │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ Auto-send │ │Edit │ │Escalate │
    │ (routine) │ │+send│ │(complex)│
    └───────────┘ └─────┘ └─────────┘
```

**Rules for what the agent handles fully vs what you review:**

| Agent handles fully (no review) | You review before sending |
|-------------------------------|--------------------------|
| Acknowledgment receipts ("We got your message") | Responses to angry customers |
| Status updates ("Your request is being processed") | Feature request prioritization |
| FAQ responses (exact match) | Anything involving money (refunds, pricing) |
| Spam classification and deletion | Bug reports (you need to verify) |
| Internal logging and categorization | Anything you've never seen before |

> **Common Mistake:** Letting the agent respond to customers autonomously from day one. Don't. Start with the agent drafting everything, you approving everything. After a week, let it auto-send acknowledgments. After a month, let it auto-send FAQ responses. Build trust incrementally — with yourself and with your customers.

### Your Turn

1. Choose one: build the MCP docs server OR the feedback processing agent.
2. Adapt it to your product/service. If you don't have customers yet, use the signal monitor from Lesson 3 as your "customer" — process its output through the feedback agent pattern.
3. Run it manually 10 times with different inputs.
4. Measure: what percentage of outputs are usable without editing? That's your automation quality score. Target 70%+ before scheduling.

---

## Lesson 5: The Human-in-the-Loop Principle

*"Full automation is a trap. Partial automation is a superpower."*

### Why 80% Automation Beats 100%

There's a specific, measurable reason why you should never fully automate customer-facing processes: the cost of a bad output is asymmetric.

A good automated output saves you 5 minutes.
A bad automated output costs you a customer, a public complaint, a refund, or a reputation hit that takes months to recover from.

The math:

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

That's a 12x reduction in damage cost. Your time reviewing 200 outputs (maybe 2 hours) saves you $2,300/month in damage.

### Never Fully Automate These

Some things should always have a human in the loop, regardless of how good the AI gets:

1. **Customer-facing communication** — A badly worded email can lose a customer forever. A generic, clearly-AI response can erode trust. Review it.

2. **Financial transactions** — Refunds, pricing changes, invoicing. Always review. The cost of a mistake is real dollars.

3. **Published content with your name on it** — Your reputation compounds over years and can be destroyed in one bad post. Ten minutes of review is cheap insurance.

4. **Legal or compliance-related output** — Anything touching contracts, privacy policies, terms of service. AI makes confident-sounding legal mistakes.

5. **Hiring or people decisions** — If you ever outsource, never let an AI make the final call on who to work with.

### Automation Debt

Automation debt is worse than technical debt because it's invisible until it explodes.

**What automation debt looks like:**
- A social media bot that posts at the wrong time because the timezone changed
- A newsletter pipeline that's been including a broken link for 3 weeks because nobody checks
- A price monitor that stopped working when the competitor redesigned their page
- A backup script that silently fails because the disk filled up

**How to prevent it:**

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

Run this every morning. When an automation silently breaks (and it will), you'll know within 24 hours instead of 3 weeks.

### Building Review Queues

The key to making human-in-the-loop efficient is batching your review. Don't review one item at a time as they arrive. Queue them and review in batches.

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

**The review habit:** Check your review queue at 8 AM and 4 PM. Two sessions, 10-15 minutes each. Everything else runs autonomously between reviews.

> **Real Talk:** Consider what happens when you skip human review: you fully automate your newsletter, the LLM starts inserting hallucinated links to pages that don't exist, and subscribers notice before you do. You lose a chunk of your list and it takes months to rebuild trust. By contrast, the developer who automates 80% of the same process — LLM curates and drafts, they spend 10 minutes reviewing — catches those hallucinations before they ship. The difference isn't the automation. It's the review step.

### Your Turn

1. Set up the `automation_healthcheck.py` script for whatever automations you built in Lessons 2 and 3. Schedule it to run every morning.
2. Implement a review queue for your highest-risk automation output (anything customer-facing).
3. Commit to checking the review queue twice daily for one week. Log how many items you approve unchanged, how many you edit, and how many you reject. This data tells you how good your automation actually is.

---

## Lesson 6: Cost Optimization and Your First Pipeline

*"If you can't generate $200 in revenue from $200 in API spend, fix the product — not the budget."*

### The Economics of LLM-Powered Automation

Every LLM call has a cost. Even local models cost electricity and GPU wear. The question is whether the output of that call generates more value than the call costs.

**The $200/month API budget rule:**

If you're spending $200/month on API calls for your automations, those automations should be generating at least $200/month in value — either direct revenue or time saved that you convert to revenue elsewhere.

If they're not: the problem isn't the API budget. It's the pipeline design or the product it supports.

### Cost-Per-Output Tracking

Add this to every pipeline you build:

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

### Batching for API Efficiency

If you're using API models, batching saves real money:

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

### Caching: Don't Pay Twice for the Same Answer

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

**Use it in your pipelines:**

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

For pipelines that process the same types of content repeatedly (classification, extraction), caching can eliminate 30-50% of your API calls. That's 30-50% off your monthly bill.

### Building Your First Complete Pipeline: Step by Step

Here's the complete process from "I have a manual workflow" to "it runs while I sleep."

**Step 1: Map your current manual process.**

Write down every step you take for one specific income stream. Example for a newsletter:

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

**Step 2: Identify the three most time-consuming steps.**

From the example: Reading articles (40 min), writing summaries (30 min), scanning headlines (20 min).

**Step 3: Automate the easiest one first.**

Scanning headlines is the easiest to automate — it's classification. An LLM scores relevance, you only read the top-scored ones.

**Step 4: Measure time saved and quality.**

After automating headline scanning:
- Time saved: 20 minutes
- Quality: 90% agreement with your manual choices
- Net: 20 minutes saved, negligible quality loss

**Step 5: Automate the next step.**

Now automate summary writing. The LLM drafts summaries, you edit them.

**Step 6: Keep going until diminishing returns.**

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

**Step 7: The complete pipeline, wired together.**

Here's a GitHub Action that ties everything together for a weekly newsletter pipeline:

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

This runs every Sunday at 5 AM. By the time you wake up, the draft is waiting. You spend 10 minutes reviewing it over coffee, hit send, and your newsletter is published for the week.

### Your Turn: Build Your Pipeline

This is the module deliverable. By the end of this lesson, you should have one complete pipeline deployed and running.

**Requirements for your pipeline:**
1. It runs on a schedule without your involvement
2. It includes at least one LLM processing step
3. It has a human review step for quality control
4. It has a health check so you know if it breaks
5. It's connected to a real income stream (or a stream you're building)

**Checklist:**

- [ ] Chose an income stream to automate
- [ ] Mapped the manual process (all steps, with time estimates)
- [ ] Identified the 3 most time-consuming steps
- [ ] Automated at least the first step (classification/scoring/filtering)
- [ ] Added LLM processing for the second step (summarization/generation/extraction)
- [ ] Built a review queue for human oversight
- [ ] Set up a health check for the automation
- [ ] Deployed on a schedule (cron, GitHub Actions, or systemd timer)
- [ ] Tracked the cost and time savings for one full cycle
- [ ] Documented the pipeline (what it does, how to fix it, what to monitor)

If you've done all ten items on this checklist, you have a Level 3 automation running. You've just freed up hours of your week that you can reinvest into building more streams or improving existing ones.

---

## Module T: Complete

### What You've Built in Two Weeks

1. **An understanding of the automation pyramid** — you know where you are and where each of your income streams should be heading.
2. **Scheduled automations** running on cron or cloud schedulers — the unglamorous foundation that makes everything else possible.
3. **LLM-powered pipelines** that handle the judgment calls you used to make manually — classifying, summarizing, generating, monitoring.
4. **Agent-based patterns** you can deploy for customer interaction, feedback processing, and MCP-powered products.
5. **A human-in-the-loop framework** that protects your reputation while still saving 80%+ of your time.
6. **Cost tracking and optimization** so your automations generate profit, not just activity.
7. **One complete, deployed pipeline** generating value without your active involvement.

### The Compound Effect

Here's what happens over the next 3 months if you maintain and extend what you built in this module:

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

This is how one developer operates like a team of five. Not by working harder. By building systems that work while you don't.

---

### 4DA Integration

4DA is itself a Level 3 automation. It ingests content from dozens of sources, scores each item with the PASIFA algorithm, and surfaces only what's relevant to your work — all without you lifting a finger. You don't manually check Hacker News, Reddit, and 50 RSS feeds. 4DA does it and shows you what matters.

Build your income pipelines the same way.

4DA's attention report (`/attention_report` in the MCP tools) shows you where your time actually goes versus where it should go. Run it before deciding what to automate. The gap between "time spent" and "time should be spent" is your automation roadmap.

The signal classification tools (`/get_actionable_signals`) can feed directly into your market monitoring pipeline — letting 4DA's intelligence layer do the initial scoring before your custom pipeline does niche-specific analysis.

If you're building pipelines that monitor sources for opportunities, don't reinvent what 4DA already does. Use its MCP server as a building block in your automation stack.

---

### What Comes Next: Module S — Stacking Streams

Module T gave you the tools to make each income stream run efficiently. Module S (Stacking Streams) answers the next question: **how many streams should you run, and how do they fit together?**

Here's what Module S covers:

- **Portfolio theory for income streams** — why 3 streams beats 1 stream, and why 10 streams beats none
- **Stream correlation** — which streams reinforce each other and which compete for your time
- **The income floor** — building a base of recurring revenue that covers your costs before you experiment
- **Rebalancing** — when to double down on a winner and when to kill an underperformer
- **The $10K/month architecture** — specific stream combinations that reach five figures with 15-20 hours per week

You have the infrastructure (Module S), the moats (Module T), the engines (Module R), the launch playbook (Module E), the trend radar (Module E), and now the automation (Module T). Module S ties them all together into a sustainable, growing income portfolio.

---

**The pipeline runs. The draft is ready. You spend 10 minutes reviewing.**

**That's tactical automation. That's how you scale.**

*Your rig. Your rules. Your revenue.*
