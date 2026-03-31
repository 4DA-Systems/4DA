# モジュール T: タクティカル・オートメーション

**STREETS 開発者収入コース — 有料モジュール**
*第12-13週 | 全6レッスン | 成果物: 価値を生み出す自動パイプライン1本*

> 「LLM、エージェント、MCP、cronジョブを力の乗数として使う。」

---

収益エンジンが稼働している。顧客がいる。うまくいくプロセスがある。そして、時間の60〜70%を同じことの繰り返しに費やしている——入力の処理、出力のフォーマット、モニターの確認、更新の送信、キューのレビュー。

その時間はあなたの最も高価なリソースであり、月額{= regional.currency_symbol | fallback("$") =}5のVPSで処理できるタスクに燃やしている。

{@ insight hardware_benchmark @}

このモジュールは、自分自身をループから体系的に外すことについてだ——完全にではなく（それはレッスン5で扱う罠だ）、判断を必要としない80%の作業から外す。結果として、あなたの収入源は、寝ている間も、本業をしている間も、次のものを作っている間も収益を生み出す。

この2週間が終わるまでに、あなたは以下を手に入れる：

- オートメーションの4つのレベルと、今日の自分がどこにいるかの明確な理解
- インフラ上で稼働するcronジョブとスケジュール自動化
- 関与なしに入力を処理するLLMパワードパイプライン最低1本
- エージェントベースのシステムと、それが経済的に意味を持つタイミングの理解
- 自動化があなたの評判を破壊しないためのヒューマン・イン・ザ・ループ・フレームワーク
- あなたの積極的な関与なしに価値を生み出す、完全にデプロイされたパイプライン1本

{? if stack.primary ?}
あなたのプライマリスタックは{= stack.primary | fallback("your primary stack") =}なので、この先のオートメーション例はそのエコシステムに適応した際に最も直接的に適用できる。ほとんどの例はポータビリティのためにPythonを使用しているが、パターンはどの言語にも変換できる。
{? endif ?}

これはコース内で最もコードが多いモジュールだ。以下の内容の少なくとも半分は実行可能なコードだ。コピーし、適応し、デプロイしよう。

さあ、自動化しよう。

---

## レッスン 1: オートメーション・ピラミッド

*「ほとんどの開発者はレベル1で自動化する。お金はレベル3にある。」*

### 4つのレベル

収入スタックのすべての自動化は、このピラミッドのどこかに位置する：

```
┌───────────────────────────────┐
│  レベル4: 自律型エージェント    │  ← あなたの代わりに意思決定する
│  (AIが判断し、行動する)        │
├───────────────────────────────┤
│  レベル3: インテリジェント      │  ← お金はここにある
│  パイプライン (LLM搭載)       │
├───────────────────────────────┤
│  レベル2: スケジュール          │  ← ほとんどの開発者がここで止まる
│  自動化 (cron + スクリプト)    │
├───────────────────────────────┤
│  レベル1: テンプレートによる     │  ← ほとんどの開発者がここにいる
│  手動作業 (コピペ)             │
└───────────────────────────────┘
```

各レベルが実際にどのようなものか、具体的に見ていこう。

### レベル1: テンプレートによる手動作業

作業は自分でやるが、スピードアップのためにチェックリスト、テンプレート、スニペットを用意している。

**例：**
- プリフィルされたフロントマター付きのMarkdownテンプレートを使ってブログ記事を書く
- 先月の請求書を複製して数字を変えてクライアントに請求する
- 保存済みの返信テンプレートを使ってサポートメールに返信する
- デプロイコマンドを手動で実行してコンテンツを公開する

**時間コスト：** 出力1単位あたりあなたの時間の100%。
**エラー率：** 中程度——人間なので、疲れていればミスをする。
**スケールの上限：** あなた自身。あなたの時間。それだけ。

ほとんどの開発者はここに住んでいて、上にピラミッドがあることすら気づいていない。

### レベル2: スケジュール自動化

スクリプトがスケジュールで実行される。ロジックを一度書いた。あなたなしで実行される。

**例：**
- RSSフィードをチェックし、新しい記事をソーシャルメディアに投稿するcronジョブ
- 毎朝6時にサイトをビルドしてデプロイするGitHub Action
- 毎時間実行され、競合他社の価格をチェックして変更をログに記録するスクリプト
- 午前3時に実行されるデータベースの日次バックアップ

**時間コスト：** 継続的にはゼロ（初期セットアップの1〜4時間後）。
**エラー率：** 低い——決定論的で、毎回同じロジック。
**スケールの上限：** マシンがスケジュールできるタスクの数だけ。数百。

ここは技術的な開発者のほとんどが到達するところだ。居心地が良い。しかし、ハードリミットがある——決定論的なロジックを持つタスクしか処理できない。タスクに判断が必要な場合、行き詰まる。

### レベル3: インテリジェント・パイプライン

スクリプトがスケジュールで実行されるが、判断をLLMが処理する。

**例：**
- RSSフィードを取り込み、LLMが各記事を要約し、ニュースレターの下書きを作成、10分レビューして送信
- 顧客のフィードバックメールがセンチメントと緊急度で分類され、下書きの返信が承認待ちでキューに入る
- ニッチの新しい求人がスクレイピングされ、LLMが関連性を評価、200件のリストをスキャンする代わりに毎日5件の機会のダイジェストが届く
- 競合他社のブログ記事がモニタリングされ、LLMが主要な製品変更を抽出、毎週の競合インテリジェンスレポートが届く

**時間コスト：** 手動時間の10〜20%。作成する代わりにレビューして承認する。
**エラー率：** 分類タスクでは低い、生成では中程度（だからレビューする）。
**スケールの上限：** 1日に数千アイテム。ボトルネックは時間ではなくAPIコスト。

**ここにお金がある。** レベル3は、通常3〜5人のチームが必要な収入源を一人で運営できるようにする。

### レベル4: 自律型エージェント

AIシステムが、あなたの関与なしに観察し、決定し、行動する。

**例：**
- SaaSのメトリクスをモニタリングし、サインアップの減少を検出し、価格変更をA/Bテストし、うまくいかなければ元に戻すエージェント
- Tier 1の顧客の質問を完全に自律的に処理し、複雑な問題のみあなたにエスカレーションするサポートエージェント
- トレンドトピックを特定し、下書きを生成し、公開をスケジュールし、パフォーマンスをモニタリングするコンテンツエージェント

**時間コスト：** 処理されたケースではほぼゼロ。個別のアクションではなくメトリクスをレビューする。
**エラー率：** ガードレール次第。なければ高い。良いガードレールがあれば、狭いドメインでは驚くほど低い。
**スケールの上限：** エージェントのスコープ内のタスクでは事実上無制限。

レベル4は現実的で達成可能だが、最初のスタート地点ではない。そしてレッスン5で扱うように、実装が不十分な場合、完全自律の顧客対応エージェントは評判にとって危険だ。

> **率直に言う：** 今レベル1にいるなら、レベル4に飛ぼうとするな。本番で壊れて顧客の信頼を損なう「自律エージェント」を何週間もかけて作ることになる。ピラミッドを一段ずつ登れ。レベル2は午後の作業1回分。レベル3は週末プロジェクト。レベル4は、レベル3が1ヶ月安定して稼働した後に来る。

### セルフアセスメント：あなたはどこにいる？

各収入源について、正直に自己評価しよう：

| 収入源 | 現在のレベル | 週あたりの時間 | 自動化目標 |
|--------|------------|--------------|----------|
| [例: ニュースレター] | [1-4] | [X] 時間 | [目標レベル] |
| [例: クライアント処理] | [1-4] | [X] 時間 | [目標レベル] |
| [例: ソーシャルメディア] | [1-4] | [X] 時間 | [目標レベル] |
| [例: サポート] | [1-4] | [X] 時間 | [目標レベル] |

最も重要な列は「週あたりの時間」だ。最も多くの時間を費やし、最も低いレベルにあるストリームが、最初の自動化ターゲットだ。それが最大のROIを持つものだ。

### 各レベルの経済学

週10時間の時間をかけて月{= regional.currency_symbol | fallback("$") =}2,000を生み出す収入源があるとしよう：

| レベル | あなたの時間 | 実効時給 | 自動化コスト |
|-------|-----------|---------|------------|
| レベル 1 | 10時間/週 | $50/時間 | $0 |
| レベル 2 | 3時間/週 | $167/時間 | $5/月 (VPS) |
| レベル 3 | 1時間/週 | $500/時間 | $30-50/月 (API) |
| レベル 4 | 0.5時間/週 | $1,000/時間 | $50-100/月 (API + コンピュート) |

レベル1からレベル3への移行は収益を変えない。実効時給を$50から$500に変える。そして解放された9時間は、次の収入源の構築か、現在の収入源の改善に使える。

> **よくある間違い：** 「簡単だから」最も収益の低いストリームを最初に自動化すること。違う。収益に対して最も多くの時間を消費しているストリームを自動化しろ。そこにROIがある。

### あなたの番

1. 上のセルフアセスメント表を、持っている（または計画中の）すべての収入源について記入する。
2. 最もROIの高い自動化ターゲットを特定する：最も多くの時間を費やし、最も低い自動化レベルにあるストリーム。
3. そのストリームで最も時間のかかる3つのタスクを書き出す。レッスン2で最初の1つを自動化する。

---

## レッスン 2: レベル1から2へ — スケジュール自動化

*「cronは1975年から存在する。今でも動く。使え。」*

### Cronジョブの基礎

{? if computed.os_family == "windows" ?}
あなたはWindowsを使っているので、cronはシステムにネイティブではない。2つの選択肢がある：WSL（Windows Subsystem for Linux）を使って本物のcronを入手するか、Windowsタスクスケジューラ（下記参照）を使う。WSLに慣れているならWSLが推奨——このレッスンのすべてのcron例はWSLで直接動く。ネイティブWindowsを好むなら、この後のタスクスケジューラのセクションまでスキップしよう。
{? endif ?}

そう、2026年でもcronはスケジュールタスクの王者だ。信頼性があり、どこにでもあり、クラウドアカウントもSaaSサブスクリプションも、毎回Googleで検索するYAMLスキーマも必要ない。

**30秒でわかるcronの構文：**

```
┌───────── 分 (0-59)
│ ┌───────── 時 (0-23)
│ │ ┌───────── 日 (1-31)
│ │ │ ┌───────── 月 (1-12)
│ │ │ │ ┌───────── 曜日 (0-7, 0と7 = 日曜日)
│ │ │ │ │
* * * * *  コマンド
```

**よく使うスケジュール：**

```bash
# 毎時
0 * * * *  /path/to/script.sh

# 毎日午前6時
0 6 * * *  /path/to/script.sh

# 毎週月曜日午前9時
0 9 * * 1  /path/to/script.sh

# 15分ごと
*/15 * * * *  /path/to/script.sh

# 毎月1日の深夜
0 0 1 * *  /path/to/script.sh
```

**cronジョブの設定：**

```bash
# crontabを編集
crontab -e

# 既存のcronジョブを一覧表示
crontab -l

# 重要：常にファイルの先頭で環境変数を設定
# cronは最小限の環境で実行される — PATHにツールが含まれていない場合がある
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# 出力をログに記録して失敗をデバッグできるようにする
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **よくある間違い：** 手動で実行すると完璧に動くスクリプトを書いたのに、cronで`.bashrc`や`.zshrc`が読み込まれないため静かに失敗する。cronスクリプトでは常に絶対パスを使うこと。crontabの先頭で常に`PATH`を設定すること。常に出力をログファイルにリダイレクトすること。

### Cronでは足りないときのクラウドスケジューラ

マシンが24時間稼働していない場合、またはより堅牢なものが必要な場合、クラウドスケジューラを使おう：

**GitHub Actions（パブリックリポジトリは無料、プライベートは月2,000分）：**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # 毎日UTC午前6時
    - cron: '0 6 * * *'
  # テスト用の手動トリガーを許可
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

**Vercel Cron（Hobbyプランは無料で1日1回。Proプラン：無制限）：**

```typescript
// api/cron/daily-report.ts
// Vercel cronエンドポイント — vercel.jsonでスケジュールを設定

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Vercelからの呼び出しであることを検証し、ランダムなHTTPリクエストではないことを確認
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // ここにオートメーションロジック
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

### 今すぐ作れる実用的な自動化

今日実装できる5つの自動化を紹介する。各30〜60分で、毎週数時間の手動作業を排除する。

#### 自動化 1: スケジュールでのコンテンツ自動公開

ブログ記事を事前に書いておく。このスクリプトがスケジュールされた時間に公開する。

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — スケジュールされた日付にMarkdown記事を公開する。
cronで毎日実行: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# CMSのAPIエンドポイント (Hashnode, Dev.to, Ghost, など)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """公開済み記事のファイル名リストを読み込む。"""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """公開済み記事のファイル名リストを保存する。"""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """MarkdownファイルからYAMLスタイルのフロントマターを抽出する。"""
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
    """記事を今日公開すべきかチェックする。"""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """CMS APIに記事を公開する。"""
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

**Markdown記事はこのようになる：**

```markdown
---
title: "OllamaをNginxの背後にデプロイする方法"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

記事の本文をここに...
```

インスピレーションが湧いたときに記事を書く。日付を設定する。スクリプトが残りを処理する。

#### 自動化 2: 新しいコンテンツ公開時にソーシャルメディアへ自動投稿

ブログが新しい記事を公開すると、Twitter/XとBlueskyに自動的に投稿する。

```python
#!/usr/bin/env python3
"""
social_poster.py — 新しいコンテンツが公開されたときにソーシャルプラットフォームに投稿する。
30分ごとに実行: */30 * * * * python3 /path/to/social_poster.py
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
    """RSSフィードを解析してアイテムのリストを返す。"""
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
    """AT ProtocolでBlueskyに投稿する。"""
    # ステップ1: セッションを作成
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

    # ステップ2: 投稿を作成
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

        # ソーシャル投稿をフォーマット
        text = f"{item['title']}\n\n{item['link']}"

        # Blueskyには300文字の制限がある
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

コスト：$0。あなたのマシンまたは無料のGitHub Actionで実行。

#### 自動化 3: 競合他社の価格モニター

競合他社が価格を変更した瞬間にわかる。毎週手動で確認する必要がなくなる。

```python
#!/usr/bin/env python3
"""
price_monitor.py — 競合他社の価格ページの変更をモニタリングする。
6時間ごとに実行: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # またはDiscord、メールなど

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # ページ全体のモニタリング用。特定要素にはセレクタを使用
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """ページを取得し、コンテンツハッシュとテキスト抜粋を返す。"""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # 文脈のために表示テキストの最初の500文字を取得
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
    """Slack webhookでアラートを送信する（Discord、メールなどに置き換え可能）。"""
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

#### 自動化 4: 週次収益レポート

毎週月曜日の朝、収益データからレポートを生成してメールで送信する。

```python
#!/usr/bin/env python3
"""
weekly_report.py — トラッキングスプレッドシート/データベースから週次収益レポートを生成する。
月曜日午前7時に実行: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """revenueテーブルが存在しない場合は作成する。"""
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
    """プレーンテキストの週次レポートを生成する。"""
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
    """メールでレポートを送信する。"""
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

#### 自動化 5: クライアントデータの自動バックアップ

クライアントの成果物を二度と失わない。毎晩実行され、30日分のバックアップを保持する。

```bash
#!/bin/bash
# backup_client_data.sh — クライアントプロジェクトデータの夜間バックアップ。
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# 圧縮バックアップを作成
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# 保持期間より古いバックアップを削除
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# ログ
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup complete: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# オプション：2番目の場所に同期（外付けドライブ、別のマシン）
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### より高度な制御のためのsystemdタイマー

cronが提供する以上のもの——依存関係の順序付け、リソース制限、自動リトライなど——が必要な場合、systemdタイマーを使おう：

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
# 失敗時に指数バックオフで再起動
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
# マシンが午前6時にオフだった場合、オンラインに戻ったときに実行
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# タイマーを有効化して開始
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# ステータスを確認
systemctl list-timers --all | grep income

# ログを表示
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Windowsタスクスケジューラの代替

WSLを使っていない場合、Windowsタスクスケジューラが同じ仕事をする。コマンドラインから`schtasks`を使うか、タスクスケジューラGUI（`taskschd.msc`）を使う。主な違い：cronは単一の式を使うが、タスクスケジューラはトリガー、アクション、条件に別々のフィールドを使う。このレッスンのすべてのcron例は直接変換できる——Pythonスクリプトを同じ方法でスケジュールするだけで、インターフェースが異なるだけだ。
{? endif ?}

### あなたの番

1. このレッスンの中から、自分の収入源に適用できる最もシンプルな自動化を選ぶ。
2. 実装する。「実装を計画する」ではない。コードを書き、テストし、スケジュールする。
3. 動作を確認できるようにログを設定する。3日間、毎朝ログをチェックする。
4. 安定したら、毎日のチェックをやめる。毎週チェックする。それが自動化だ。

**最低限：** 今日の終わりまでに安定して稼働するcronジョブ1つ。

---

## レッスン 3: レベル2から3へ — LLMパワードパイプライン

*「自動化にインテリジェンスを加える。ここで一人がチームのように見え始める。」*

### パターン

すべてのLLMパワードパイプラインは同じ形に従う：

```
入力ソース → 取り込み → LLM処理 → 出力フォーマット → 配信（またはレビューキューへ）
```

魔法は「LLM処理」ステップにある。あり得るすべてのケースに対して決定論的なルールを書く代わりに、望むことを自然言語で記述すれば、LLMが判断を処理する。

### ローカル vs APIの使い分け

{? if settings.has_llm ?}
{= settings.llm_provider | fallback("LLMプロバイダー") =}を{= settings.llm_model | fallback("LLMモデル") =}で設定済みだ。つまり、すぐにインテリジェントパイプラインの構築を開始できる。以下の判断は、各パイプラインでローカルセットアップとAPIのどちらを使うかを選ぶのに役立つ。
{? else ?}
まだLLMを設定していない。このレッスンのパイプラインは、ローカルモデル（Ollama）とクラウドAPIの両方で動作する。最初のパイプラインを構築する前に少なくとも1つをセットアップしよう——Ollamaは無料で、インストールに10分かかる。
{? endif ?}

この判断はあなたのマージンに直接影響する：

| 要素 | ローカル (Ollama) | API (Claude, GPT) |
|------|-----------------|-------------------|
| **100万トークンあたりのコスト** | 〜$0.003（電気代） | $0.15 - $15.00 |
| **速度（トークン/秒）** | 20-60（ミッドレンジGPUで8B） | 50-100以上 |
| **品質（ローカル8B vs API）** | 分類・抽出には良い | 生成・推論には優れている |
| **プライバシー** | データがマシンから出ない | データがプロバイダーに行く |
| **アップタイム** | マシン次第 | 99.9%以上 |
| **バッチ処理能力** | GPUメモリに制限される | レート制限と予算に制限される |

{? if profile.gpu.exists ?}
マシンに{= profile.gpu.model | fallback("GPU") =}があるので、ローカル推論は有力な選択肢だ。実行できる速度とモデルサイズはVRAMに依存する——ローカルのみのパイプラインにコミットする前に、何が収まるか確認しよう。
{? if computed.has_nvidia ?}
NVIDIA GPUはCUDAアクセラレーションのおかげで最高のOllamaパフォーマンスを得られる。7-8Bパラメータモデルは快適に実行でき、{= profile.gpu.vram | fallback("利用可能なVRAM") =}によってはさらに大きなモデルも可能かもしれない。
{? endif ?}
{? else ?}
専用GPUがないと、ローカル推論は遅くなる（CPUのみ）。小規模なバッチジョブや分類タスクには機能するが、時間的に厳しいものや大量処理には、APIモデルの方が実用的だ。
{? endif ?}

**経験則：**
- **大量・低い品質基準**（分類、抽出、タグ付け）→ ローカル
- **少量・品質重要**（顧客向けコンテンツ、複雑な分析）→ API
- **機密データ**（クライアント情報、プロプライエタリデータ）→ 常にローカル
- **月10,000アイテム以上** → ローカルで実際にお金を節約

**典型的なパイプラインの月間コスト比較：**

```
月5,000アイテム処理、アイテムあたり約500トークン:

ローカル (Ollama, llama3.1:8b):
  2,500,000トークン × $0.003/1M = $0.0075/月
  基本的に無料。

API (GPT-4o-mini):
  2,500,000入力トークン × $0.15/1M = $0.375
  2,500,000出力トークン × $0.60/1M = $1.50
  合計: 〜$1.88/月
  安いが、ローカルの250倍。

API (Claude 3.5 Sonnet):
  2,500,000入力トークン × $3.00/1M = $7.50
  2,500,000出力トークン × $15.00/1M = $37.50
  合計: 〜$45/月
  品質は優れているが、ローカルの6,000倍。
```

分類・抽出パイプラインでは、適切にプロンプトされた8BローカルモデルとフロンティアAPIモデルの品質差は、しばしば無視できる程度だ。両方をテストしよう。品質基準を満たすより安い方を使え。

{@ insight cost_projection @}

### パイプライン 1: ニュースレターコンテンツジェネレーター

これは、コンテンツベースの収入を持つ開発者にとって最も一般的なLLM自動化だ。RSSフィードが入力され、ニュースレターの下書きが出力される。

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — RSSフィードを取り込み、LLMで要約し、ニュースレターの下書きを生成する。
毎日実行: 0 5 * * * python3 /path/to/newsletter_pipeline.py

このパイプライン:
1. 複数のRSSフィードから新しい記事を取得する
2. 各記事をローカルLLMに送って要約する
3. オーディエンスとの関連性でランキングする
4. フォーマットされたニュースレターの下書きを生成する
5. レビュー用に下書きを保存する（2時間のキュレーションの代わりに10分のレビュー）
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
    # ここにニッチのフィードを追加
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
    """RSS/Atomフィードを解析して記事を返す。"""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
        # RSSとAtomの両方のフィードを処理
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
    """ローカルLLMにプロンプトを送信してレスポンスを取得する。"""
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
    """LLMを使って関連性をスコアリングし、要約を生成する。"""
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
        # LLM出力からJSONを解析する
        # LLMがマークダウンコードブロックで囲む場合を処理
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
    """スコアリングされた記事をニュースレターの下書きにフォーマットする。"""
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

    # 関連性のある記事のみをフィルタリングし、スコアでソート
    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    # 上位10件を取得
    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} articles passed relevance threshold (>= 6/10)")

    # ニュースレターの下書きを生成
    draft = generate_newsletter(top_articles)

    # 下書きを保存
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

**このコスト：**
- ローカル8Bモデルで1日50記事を処理：〜$0/月
- あなたの時間：手動キュレーション2時間の代わりに下書きレビュー10分
- 週あたりの時間節約：週刊ニュースレターなら〜10時間

### パイプライン 2: 顧客調査とインサイトレポート

このパイプラインは公開データをスクレイピングし、LLMで分析し、販売可能なレポートを作成する。

```python
#!/usr/bin/env python3
"""
research_pipeline.py — 企業/製品の公開データを分析し、インサイトレポートを生成する。
販売可能なサービス：カスタムレポート1本$200-500。

使用方法: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# 有料レポートの品質のためにより大きなモデルを使用
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# または顧客向け品質のためにAPIを使用:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """設定に基づいてローカルまたはAPIモデルにルーティングする。"""
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
    """企業に関する公開データを収集する。"""
    data = {"company": company, "domain": domain}

    # ドメインが到達可能か確認し、基本情報を取得
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

    # GitHubプレゼンスをチェック
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
    """LLMを使って分析レポートを生成する。"""
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

    # 最終レポートを組み立てる
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

**ビジネスモデル：** カスタム調査レポート1本$200-500を請求する。あなたのコスト：APIコール$0.05と15分のレビュー。パイプラインが安定すれば、1時間に3〜4本のレポートを作成できる。

### パイプライン 3: マーケットシグナルモニター

これは次に何を作るべきかを教えてくれるパイプラインだ。複数のソースをモニタリングし、シグナルを分類し、機会がしきい値を超えるとアラートを出す。

```python
#!/usr/bin/env python3
"""
signal_monitor.py — 公開ソースからマーケットの機会をモニタリングする。
2時間ごとに実行: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# あなたのニッチの定義 — LLMがこれを使って関連性をスコアリングする
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
    """Hacker Newsのトップ記事を取得する。"""
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
    """LLMを使ってシグナルをマーケット機会として分類する。"""
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
    """高スコアの機会についてアラートを送信する。"""
    msg = (
        f"OPPORTUNITY DETECTED (score: {item['opportunity_score']}/10)\n"
        f"Type: {item['opportunity_type']}\n"
        f"Title: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Why: {item['reasoning']}\n"
        f"Action: {item['action']}"
    )

    # ファイルにログ
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # Slack/Discordに送信
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERT: {msg}")

def main():
    seen = load_seen()

    # ソースから取得
    print("Fetching signals...")
    items = fetch_hn_top(30)
    # ここにさらにソースを追加: Reddit, RSSフィード, GitHubトレンドなど

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} new signals to classify")

    # 各シグナルを分類
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

**実際の動作：** 週に2〜3回、「機会：スターターキットのない新しいフレームワークがリリースされた——今週末にあなたが作れる」というSlack通知が届く。そのシグナルに、他の人より先に行動することが、先を行く方法だ。

> **率直に言う：** これらのパイプライン出力の品質は、プロンプトとニッチの定義に完全に依存する。ニッチが曖昧なら（「Webデベロッパーです」）、LLMはすべてにフラグを立てる。具体的なら（「プライバシーファースト開発者市場向けのTauriデスクトップアプリを作っている」）、外科手術のように正確になる。ニッチの定義を正しくするために30分を費やせ。構築するすべてのパイプラインに対する最もレバレッジの高い入力だ。

### あなたの番

{? if stack.contains("python") ?}
朗報：上のパイプライン例はすでにあなたのプライマリ言語で書かれている。そのままコピーして適応を始められる。ニッチの定義とプロンプトを正しくすることに集中しよう——出力品質の90%はそこから来る。
{? else ?}
上の例はポータビリティのためにPythonを使っているが、パターンはどの言語でも動く。{= stack.primary | fallback("プライマリスタック") =}で構築したい場合、再現すべき重要な要素は：RSS/API取得用のHTTPクライアント、LLMレスポンス用のJSON解析、状態管理用のファイルI/O。LLMとのやり取りは、OllamaまたはクラウドAPIへのHTTP POSTに過ぎない。
{? endif ?}

1. 上の3つのパイプライン（ニュースレター、調査、シグナルモニター）から1つを選ぶ。
2. 自分のニッチに適応させる。フィード、オーディエンスの説明、分類基準を変更する。
3. 出力品質をテストするために手動で3回実行する。
4. 出力が大幅な編集なしで使えるようになるまでプロンプトを調整する。
5. cronでスケジュールする。

**目標：** このレッスンを読んでから48時間以内にスケジュール実行されるLLMパワードパイプライン1本。

---

## レッスン 4: レベル3から4へ — エージェントベースシステム

*「エージェントとは、観察し、決定し、行動するループにすぎない。1つ作ろう。」*

### 2026年における「エージェント」の本当の意味

ハイプを取り除こう。エージェントとは以下を行うプログラムだ：

1. **観察する** — 何らかの入力または状態を読む
2. **決定する** — LLMを使って何をすべきか判断する
3. **行動する** — 決定を実行する
4. **ループする** — ステップ1に戻る

それだけだ。パイプライン（レベル3）とエージェント（レベル4）の違いは、エージェントがループすることだ。自分の出力に対して行動する。次のステップが前のステップの結果に依存するマルチステップタスクを処理する。

パイプラインは固定された順序でアイテムを1つずつ処理する。エージェントは遭遇したものに基づいて予測不可能な順序をナビゲートする。

### 顧客にサービスを提供するMCPサーバー

MCPサーバーは、構築できる最も実用的なエージェント隣接システムの1つだ。AIエージェント（Claude Code、Cursorなど）が顧客に代わって呼び出せるツールを公開する。

{? if stack.contains("typescript") ?}
以下のMCPサーバーの例はTypeScriptを使っている——あなたの得意分野だ。既存のTypeScriptツールで拡張し、他のNode.jsサービスと一緒にデプロイできる。
{? endif ?}

実際の例を紹介する：製品のドキュメントから顧客の質問に回答するMCPサーバー。

```typescript
// mcp-docs-server/src/index.ts
// ドキュメントから質問に回答するMCPサーバー。
// 顧客がClaude Codeをこのサーバーに向けると、即座に回答を得られる。

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// 起動時にドキュメントをメモリに読み込む
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

    // 見出しで分割してより良い検索を実現
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
  // シンプルなキーワード検索 — 本番ではベクトル検索に置き換え
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // タイトルマッチにボーナス
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

// 初期化
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

// サーバーを起動
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

**ビジネスモデル：** このMCPサーバーを製品の一部として顧客に提供する。顧客はサポートチケットを出さなくても即座に質問への回答を得られる。あなたはサポートに費やす時間が減る。全員が勝つ。

プレミアム版として：ベクトル検索、バージョン管理付きドキュメント、顧客が何について質問しているかの分析機能を備えたホスト版を月$9-29で課金する。

### 自動化された顧客フィードバック処理

このエージェントは顧客のフィードバック（メール、サポートチケット、フォームから）を読み取り、分類し、下書きの返信と機能チケットを作成する。

```python
#!/usr/bin/env python3
"""
feedback_agent.py — 顧客フィードバックを分類されたアクション可能なアイテムに処理する。
「AIが下書き、人間が承認」パターン。

毎時実行: 0 * * * * python3 /path/to/feedback_agent.py
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
    """フィードバックを分類し、下書きの返信を生成する。"""

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

        # 処理済みバージョンを保存
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # レビューキューに追加
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # オリジナルをインボックスから移動
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # レビューキューを書き込み
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # サマリー
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nProcessed: {len(review_queue)}")
    print(f"Critical: {critical}")
    print(f"Needs your attention: {needs_human}")
    print(f"Review queue: {review_path}")

if __name__ == "__main__":
    main()
```

**実際の動作：**
1. 顧客がフィードバックを送信する（フォーム、メール、サポートシステム経由）
2. フィードバックがインボックスディレクトリにJSONファイルとして到着する
3. エージェントが各ファイルを処理：分類、要約、返信の下書き
4. 1日1〜2回レビューキューを開く
5. シンプルなアイテム（称賛、良い下書きの返信がある基本的な質問）は下書きを承認する
6. 複雑なアイテム（バグ、怒っている顧客）は個人的な返信を書く
7. 正味時間：2時間の代わりに1日15分

### AIが下書き、人間が承認パターン

このパターンは、実用的なレベル4自動化の核心だ。エージェントが単調な作業を処理する。あなたは判断の呼び出しを処理する。

```
              ┌─────────────┐
              │ エージェント  │
              │ が下書き      │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ レビューキュー │
              └──────┬──────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ 自動送信  │ │編集 │ │エスカ   │
    │ (定型)    │ │+送信│ │レーション│
    │           │ │     │ │(複雑)   │
    └───────────┘ └─────┘ └─────────┘
```

**エージェントが完全に処理するもの vs あなたがレビューするもののルール：**

| エージェントが完全に処理（レビュー不要） | 送信前にあなたがレビュー |
|--------------------------------------|----------------------|
| 確認受領（「メッセージを受け取りました」） | 怒っている顧客への返信 |
| ステータス更新（「リクエストは処理中です」） | 機能リクエストの優先順位付け |
| FAQ回答（完全一致） | お金に関わるもの（返金、価格設定） |
| スパムの分類と削除 | バグレポート（検証が必要） |
| 内部ログと分類 | 見たことのないもの |

> **よくある間違い：** 初日からエージェントに顧客への自律的な返信を許可すること。やるな。すべてをエージェントが下書きし、すべてをあなたが承認するところから始めよう。1週間後、確認の自動送信を許可する。1ヶ月後、FAQ回答の自動送信を許可する。信頼を段階的に構築しよう——自分自身と顧客の両方に対して。

### あなたの番

1. 1つ選ぶ：MCPドキュメントサーバーを作るか、フィードバック処理エージェントを作るか。
2. 自分の製品/サービスに適応させる。まだ顧客がいない場合、レッスン3のシグナルモニターを「顧客」として使う——その出力をフィードバックエージェントのパターンで処理する。
3. 異なる入力で手動で10回実行する。
4. 測定する：出力の何パーセントが編集なしで使えるか？ それがあなたのオートメーション品質スコアだ。スケジュールする前に70%以上を目標にする。

---

## レッスン 5: ヒューマン・イン・ザ・ループの原則

*「完全自動化は罠。部分自動化はスーパーパワー。」*

### 80%の自動化が100%に勝つ理由

顧客対応のプロセスを完全に自動化すべきでない、具体的で測定可能な理由がある：悪い出力のコストは非対称だ。

良い自動化出力は5分を節約する。
悪い自動化出力は顧客の喪失、公開の苦情、返金、または回復に数ヶ月かかるレピュテーションのダメージをもたらす。

計算：

```
100%自動化:
  月1,000件の出力 × 95%品質 = 950件良好 + 50件不良
  50件の不良出力 × 平均$50のコスト（返金 + サポート + レピュテーション）= 月$2,500のダメージ

80%自動化 + 20%人間レビュー:
  800件自動処理、200件人間レビュー
  800 × 95%品質 = 760件良好 + 40件自動不良
  200 × 99%品質 = 198件良好 + 2件人間不良
  合計42件不良 × $50 = 月$2,100のダメージ
  しかし：顧客に届く前に38件の不良をキャッチ

  実際に顧客に届く不良出力：〜4件
  実際のダメージ：〜月$200
```

ダメージコストが12倍削減される。200件の出力をレビューする時間（おそらく2時間）で月$2,300を節約する。

### これらは絶対に完全自動化するな

AIがいかに良くなっても、以下のものには常にヒューマン・イン・ザ・ループを置くべきだ：

1. **顧客対応コミュニケーション** — 不適切な表現のメールは顧客を永久に失わせる。ジェネリックで明らかにAIの返信は信頼を侵食する。レビューしろ。

2. **金融取引** — 返金、価格変更、請求。常にレビュー。ミスのコストは実際のお金だ。

3. **あなたの名前で公開されるコンテンツ** — レピュテーションは何年もかけて複利で成長し、1つの悪い投稿で破壊される。10分のレビューは安い保険だ。

4. **法的またはコンプライアンス関連の出力** — 契約、プライバシーポリシー、利用規約に関わるもの。AIは自信満々に法的なミスをする。

5. **採用や人事の決定** — 外注する場合でも、AIに誰と仕事をするかの最終判断をさせるな。

### オートメーション負債

{@ mirror automation_risk_profile @}

オートメーション負債は技術的負債より悪い。爆発するまで見えないからだ。

**オートメーション負債の例：**
- タイムゾーンが変わったために間違った時間に投稿するソーシャルメディアボット
- 誰もチェックしないので3週間壊れたリンクを含み続けるニュースレターパイプライン
- 競合他社がページを再設計したために動かなくなった価格モニター
- ディスクがいっぱいになったために静かに失敗するバックアップスクリプト

**防止方法：**

```python
#!/usr/bin/env python3
"""
automation_healthcheck.py — すべてのオートメーションの静かな失敗をモニタリングする。
毎朝実行: 0 7 * * * python3 /path/to/automation_healthcheck.py
"""

import os
import json
from datetime import datetime, timedelta
from pathlib import Path

ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")

# 各オートメーションの期待される出力を定義
AUTOMATIONS = [
    {
        "name": "Newsletter Pipeline",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/newsletter/drafts"),
        "pattern": "draft-*.md",
        "max_age_hours": 26,  # 少なくとも毎日出力されるべき
    },
    {
        "name": "Social Poster",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/logs/social_posted.json"),
        "pattern": None,  # ファイルを直接チェック
        "max_age_hours": 2,  # 30分ごとに更新されるべき
    },
    {
        "name": "Competitor Monitor",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/monitors"),
        "pattern": "*.json",
        "max_age_hours": 8,  # 6時間ごとに実行されるべき
    },
    {
        "name": "Client Backup",
        "check_type": "file_freshness",
        "path": os.path.expanduser("~/income/backups"),
        "pattern": "projects-*.tar.gz",
        "max_age_hours": 26,  # 毎晩実行されるべき
    },
    {
        "name": "Ollama Server",
        "check_type": "http",
        "url": "http://127.0.0.1:11434/api/tags",
        "expected_status": 200,
    },
]

def check_file_freshness(automation: dict) -> tuple[bool, str]:
    """オートメーションが最近の出力を生成しているかチェックする。"""
    path = automation["path"]
    max_age = timedelta(hours=automation["max_age_hours"])

    if automation.get("pattern"):
        # パターンに一致する最近のファイルをチェック
        p = Path(path)
        if not p.exists():
            return False, f"Directory not found: {path}"

        files = sorted(p.glob(automation["pattern"]), key=os.path.getmtime, reverse=True)
        if not files:
            return False, f"No files matching {automation['pattern']} in {path}"

        newest = files[0]
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(newest))
    else:
        # ファイルを直接チェック
        if not os.path.exists(path):
            return False, f"File not found: {path}"
        age = datetime.now() - datetime.fromtimestamp(os.path.getmtime(path))

    if age > max_age:
        return False, f"Last output {age.total_seconds()/3600:.1f}h ago (max: {automation['max_age_hours']}h)"

    return True, f"OK (last output {age.total_seconds()/3600:.1f}h ago)"

def check_http(automation: dict) -> tuple[bool, str]:
    """サービスが応答しているかチェックする。"""
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

毎朝これを実行しよう。オートメーションが静かに壊れたとき（必ず壊れる）、3週間後ではなく24時間以内にわかる。

### レビューキューの構築

ヒューマン・イン・ザ・ループを効率的にする鍵は、レビューをバッチ処理することだ。到着するたびに1つずつレビューするな。キューに入れてバッチでレビューしよう。

```python
#!/usr/bin/env python3
"""
review_queue.py — AI生成出力のシンプルなレビューキュー。
常にチェックする代わりに1日1〜2回レビューする。
"""

import os
import json
from datetime import datetime
from pathlib import Path

QUEUE_DIR = os.path.expanduser("~/income/review-queue")

def add_to_queue(item_type: str, content: dict):
    """レビューキューにアイテムを追加する。"""
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
    """レビュー待ちのすべてのアイテムを表示する。"""
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

    # 実際の実装では、ここにインタラクティブな入力を追加する
    # バッチ処理用に、ファイルまたはシンプルなCLIから決定を読み取る

if __name__ == "__main__":
    review_queue()
```

**レビューの習慣：** 午前8時と午後4時にレビューキューをチェックする。2回のセッション、各10〜15分。それ以外はレビューの間に自律的に実行される。

> **率直に言う：** 人間のレビューをスキップしたらどうなるか考えてみよう：ニュースレターを完全に自動化し、LLMが存在しないページへのハルシネーションされたリンクを挿入し始め、購読者がより先に気づく。リストの一部を失い、信頼の再構築に数ヶ月かかる。対照的に、同じプロセスの80%を自動化する開発者——LLMがキュレーションと下書きをし、10分レビューする——は、出荷前にそれらのハルシネーションをキャッチする。違いは自動化ではない。レビューステップだ。

### あなたの番

1. レッスン2と3で作ったオートメーションに`automation_healthcheck.py`スクリプトをセットアップする。毎朝実行するようにスケジュールする。
2. 最もリスクの高いオートメーション出力（顧客対応のもの）のレビューキューを実装する。
3. 1週間、1日2回レビューキューをチェックすることをコミットする。何件を変更なしで承認し、何件を編集し、何件を拒否したかログをとる。このデータがあなたのオートメーションの実際の品質を教えてくれる。

---

## レッスン 6: コスト最適化と最初のパイプライン

*「API支出$200から$200の収益を生み出せないなら、予算ではなく製品を修正しろ。」*

### LLMパワードオートメーションの経済学

すべてのLLM呼び出しにはコストがある。ローカルモデルでさえ、電気代とGPUの摩耗がかかる。問題は、その呼び出しの出力が、呼び出しのコスト以上の価値を生み出すかどうかだ。

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("GPU") =}でローカルモデルを実行するコストは、典型的なパイプラインワークロードで月約{= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("数ドル") =}の電気代だ。それがAPI代替案と比較するベースラインだ。
{? endif ?}

**月{= regional.currency_symbol | fallback("$") =}200のAPI予算ルール：**

オートメーションに月{= regional.currency_symbol | fallback("$") =}200をAPI呼び出しに費やしているなら、それらのオートメーションは少なくとも月{= regional.currency_symbol | fallback("$") =}200の価値を生み出すべきだ——直接的な収益か、他で収益に変換する時間節約として。

そうでないなら、問題はAPI予算ではない。パイプラインの設計か、それが支えている製品だ。

### 出力あたりのコスト追跡

構築するすべてのパイプラインにこれを追加しよう：

```python
"""
cost_tracker.py — すべてのLLM呼び出しのコストとそれが生み出す価値を追跡する。
パイプラインでこれをインポートして実際のコストデータを取得する。
"""

import os
import json
from datetime import datetime
from pathlib import Path

COST_LOG = os.path.expanduser("~/income/logs/llm_costs.jsonl")

# 100万トークンあたりの価格（価格が変わったら更新）
PRICING = {
    # ローカルモデル — 電気代の見積もり
    "llama3.1:8b": {"input": 0.003, "output": 0.003},
    "llama3.1:70b": {"input": 0.01, "output": 0.01},
    # APIモデル
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
    """LLM呼び出しのコストをログする。"""
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
    """月間コスト/収益サマリーを生成する。"""
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

    # レポートを表示
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

### APIの効率性のためのバッチ処理

APIモデルを使用している場合、バッチ処理で実際のお金を節約できる：

```python
"""
batch_api.py — 効率のためにAPIコールをバッチ処理する。
100回の個別APIコールの代わりにバッチ処理する。
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
    複数のアイテムを単一のAPIコールにバッチ処理して効率的に分類する。

    100回のAPIコール（100アイテム × 各1回）の代わりに：
      - 100回 × 約500入力トークン = 50,000トークン入力
      - 100回 × 約200出力トークン = 20,000トークン出力
      - Haikuでのコスト: 約$0.12

    バッチ処理（1回あたり10アイテム、10回のAPIコール）：
      - 10回 × 約2,500入力トークン = 25,000トークン入力
      - 10回 × 約1,000出力トークン = 10,000トークン出力
      - Haikuでのコスト: 約$0.06

    バッチ処理だけで50%の削減。
    """
    results = []

    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # バッチを単一のプロンプトにフォーマット
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
            # レスポンスからJSON配列を解析
            cleaned = response_text.strip()
            if cleaned.startswith("```"):
                cleaned = cleaned.split("\n", 1)[1].rsplit("```", 1)[0]

            batch_results = json.loads(cleaned)
            results.extend(batch_results)

        except Exception as e:
            print(f"  Batch {i//batch_size + 1} failed: {e}")
            # 個別処理にフォールバック
            for item in batch:
                results.append({"item_index": i, "category": "unknown", "score": 0, "error": str(e)})

        # レート制限の礼儀
        if delay_between_batches > 0:
            time.sleep(delay_between_batches)

    return results
```

### キャッシング：同じ回答に二度支払うな

```python
"""
llm_cache.py — LLMレスポンスをキャッシュして重複処理の支払いを避ける。
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
    """モデル + プロンプトから決定論的なキャッシュキーを生成する。"""
    return hashlib.sha256(f"{model}:{prompt}".encode()).hexdigest()

def get_cached(model: str, prompt: str, max_age_hours: int = 168) -> str | None:
    """キャッシュされたレスポンスが利用可能で新鮮なら取得する。"""
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

    # ヒットカウントを更新
    conn.execute("UPDATE cache SET hit_count = hit_count + 1 WHERE key = ?", (key,))
    conn.commit()
    conn.close()
    return response

def set_cached(model: str, prompt: str, response: str):
    """レスポンスをキャッシュする。"""
    conn = get_cache_db()
    key = cache_key(model, prompt)

    conn.execute("""
        INSERT OR REPLACE INTO cache (key, model, response, created_at, hit_count)
        VALUES (?, ?, ?, ?, 0)
    """, (key, model, response, datetime.utcnow().isoformat()))
    conn.commit()
    conn.close()

def cache_stats():
    """キャッシュの統計情報を表示する。"""
    conn = get_cache_db()
    total = conn.execute("SELECT COUNT(*) FROM cache").fetchone()[0]
    total_hits = conn.execute("SELECT SUM(hit_count) FROM cache").fetchone()[0] or 0
    conn.close()
    print(f"Cache entries: {total}")
    print(f"Total cache hits: {total_hits}")
    print(f"Estimated savings: ~${total_hits * 0.002:.2f} (rough avg per call)")
```

**パイプラインでの使用：**

```python
# LLMを呼び出すパイプラインで:
from llm_cache import get_cached, set_cached

def llm_with_cache(model: str, prompt: str) -> str:
    cached = get_cached(model, prompt)
    if cached is not None:
        return cached  # 無料！

    response = call_llm(model, prompt)  # 既存のLLM呼び出し関数
    set_cached(model, prompt, response)
    return response
```

同じ種類のコンテンツを繰り返し処理するパイプライン（分類、抽出）では、キャッシングによりAPI呼び出しの30〜50%を排除できる。月額料金が30〜50%引きになる。

### 最初の完全パイプラインの構築：ステップバイステップ

「手動ワークフローがある」状態から「寝ている間に実行される」状態までの完全なプロセスがここにある。

**ステップ 1: 現在の手動プロセスをマッピングする。**

1つの特定の収入源で行うすべてのステップを書き出す。ニュースレターの例：

```
1. ブラウザで15個のRSSフィードを開く (10分)
2. 見出しをスキャンし、興味のあるものを開く (20分)
3. 8-10本の記事を詳細に読む (40分)
4. トップ5の要約を書く (30分)
5. イントロの段落を書く (10分)
6. メールツールでフォーマット (15分)
7. リストに送信 (5分)

合計: 約2時間10分
```

**ステップ 2: 最も時間がかかる3つのステップを特定する。**

例から：記事を読む（40分）、要約を書く（30分）、見出しをスキャンする（20分）。

**ステップ 3: 最も簡単なものから自動化する。**

見出しのスキャンは最も自動化しやすい——分類だ。LLMが関連性をスコアリングし、上位スコアのものだけを読む。

**ステップ 4: 節約された時間と品質を測定する。**

見出しスキャンの自動化後：
- 節約された時間：20分
- 品質：手動での選択と90%一致
- 正味：20分の節約、品質低下はわずか

**ステップ 5: 次のステップを自動化する。**

次に要約の作成を自動化する。LLMが要約の下書きを作成し、あなたが編集する。

**ステップ 6: 収穫逓減まで続ける。**

```
自動化前: ニュースレター1通あたり2時間10分
レベル2後（スケジュール取得）: 1時間45分
レベル3後（LLMスコアリング + 要約）: 25分
レベル3+後（LLMイントロ下書き）: レビューのみ10分

週あたりの時間節約: 約2時間
月あたりの時間節約: 約8時間
実効時給$100で: 解放された時間で月$800
APIコスト: $0（ローカルLLM）〜$5/月（API）
```

**ステップ 7: 完全なパイプラインを接続する。**

週刊ニュースレターパイプラインのためにすべてを結びつけるGitHub Action：

```yaml
# .github/workflows/newsletter-pipeline.yml
name: Weekly Newsletter Pipeline

on:
  schedule:
    # 毎週日曜日UTC午前5時
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

毎週日曜日の午前5時に実行される。起きるころには下書きが待っている。コーヒーを飲みながら10分レビューし、送信ボタンを押せば、今週のニュースレターが公開される。

### あなたの番：パイプラインを作ろう

これがモジュールの成果物だ。このレッスンの終わりまでに、1つの完全なパイプラインがデプロイされて稼働しているべきだ。

**パイプラインの要件：**
1. あなたの関与なしにスケジュールで実行される
2. 少なくとも1つのLLM処理ステップを含む
3. 品質管理のためのヒューマンレビューステップがある
4. 壊れたときにわかるヘルスチェックがある
5. 実際の収入源（または構築中のストリーム）に接続されている

**チェックリスト：**

- [ ] 自動化する収入源を選んだ
- [ ] 手動プロセスをマッピングした（すべてのステップ、時間見積もり付き）
- [ ] 最も時間がかかる3つのステップを特定した
- [ ] 少なくとも最初のステップを自動化した（分類/スコアリング/フィルタリング）
- [ ] 2番目のステップにLLM処理を追加した（要約/生成/抽出）
- [ ] 人間の監視のためのレビューキューを構築した
- [ ] オートメーションのヘルスチェックをセットアップした
- [ ] スケジュールにデプロイした（cron、GitHub Actions、またはsystemdタイマー）
- [ ] 1サイクル分のコストと時間節約を追跡した
- [ ] パイプラインを文書化した（何をするか、修正方法、監視すべきもの）

このチェックリストの10項目すべてを完了したなら、レベル3のオートメーションが稼働している。毎週の時間が解放され、より多くのストリームの構築や既存のものの改善に再投資できる。

---

## モジュール T: 完了

{@ temporal automation_progress @}

### この2週間で構築したもの

1. **オートメーション・ピラミッドの理解** — 自分がどこにいて、各収入源がどこに向かうべきかがわかる。
2. **cronまたはクラウドスケジューラで稼働するスケジュール自動化** — 他のすべてを可能にする華やかではない基盤。
3. **以前は手動で行っていた判断を処理するLLMパワードパイプライン** — 分類、要約、生成、モニタリング。
4. **顧客対応、フィードバック処理、MCPパワード製品のためのエージェントベースのパターン**。
5. **レピュテーションを守りながら時間の80%以上を節約するヒューマン・イン・ザ・ループ・フレームワーク**。
6. **オートメーションが活動ではなく利益を生み出すためのコスト追跡と最適化**。
7. **あなたの積極的な関与なしに価値を生み出す、完全にデプロイされたパイプライン1本**。

### 複利効果

このモジュールで構築したものを維持し拡張すると、今後3ヶ月間でこうなる：

```
1ヶ月目: パイプライン1本、週5-8時間節約
2ヶ月目: パイプライン2本、週10-15時間節約
3ヶ月目: パイプライン3本、週15-20時間節約

実効時給$100で、月$1,500-2,000の
解放された時間 — 新しいストリームに投資する時間。

1ヶ月目の解放された時間が2ヶ月目のパイプラインを構築する。
2ヶ月目の解放された時間が3ヶ月目のパイプラインを構築する。
オートメーションは複利で成長する。
```

これが、一人の開発者が5人のチームのように運営する方法だ。より懸命に働くことではない。あなたが働かない間に働くシステムを構築することだ。

---

### 4DA インテグレーション

{? if dna.identity_summary ?}
あなたの開発者プロファイル — {= dna.identity_summary | fallback("あなたの開発フォーカス") =} — に基づき、以下の4DAツールは学んだばかりのオートメーションパターンに直接マッピングされる。シグナル分類ツールはあなたの分野の開発者に特に関連がある。
{? endif ?}

4DA自体がレベル3のオートメーションだ。数十のソースからコンテンツを取り込み、PASIFAアルゴリズムで各アイテムをスコアリングし、あなたの仕事に関連するものだけを表示する——指一本動かすことなく。Hacker News、Reddit、50のRSSフィードを手動でチェックしない。4DAがそれをやり、重要なものを見せる。

同じ方法で収入パイプラインを構築しよう。

4DAのアテンションレポート（MCPツールの`/attention_report`）は、時間が実際にどこに行っているか対どこに行くべきかを示す。何を自動化するか決める前にこれを実行しよう。「費やした時間」と「費やすべき時間」のギャップがあなたのオートメーションロードマップだ。

シグナル分類ツール（`/get_actionable_signals`）は、マーケットモニタリングパイプラインに直接フィードできる——カスタムパイプラインがニッチ固有の分析を行う前に、4DAのインテリジェンスレイヤーに初期スコアリングをさせる。

機会のためにソースをモニタリングするパイプラインを構築するなら、4DAがすでに行っていることを再発明するな。そのMCPサーバーをオートメーションスタックの構成要素として使え。

---

### 次に来るもの: モジュール S — ストリームの積み重ね

モジュール Tは各収入源を効率的に運営するためのツールを提供した。モジュール S（ストリームの積み重ね）は次の質問に答える：**いくつのストリームを運営すべきで、どう組み合わせるか？**

モジュール Sで扱う内容：

- **収入源のポートフォリオ理論** — なぜ3本のストリームが1本に勝ち、10本のストリームがゼロに勝つのか
- **ストリームの相関** — どのストリームが相互に強化し合い、どれが時間を奪い合うか
- **インカムフロア** — 実験する前にコストをカバーする経常収益の基盤を構築する
- **リバランス** — いつ勝者に倍賭けし、いつ不振者を切るか
- **月$10Kのアーキテクチャ** — 週15〜20時間で5桁に達する具体的なストリームの組み合わせ

インフラ（モジュール S）、堀（モジュール T）、エンジン（モジュール R）、ローンチプレイブック（モジュール E）、トレンドレーダー（モジュール E）、そしてオートメーション（モジュール T）がある。モジュール Sはこれらすべてを持続可能で成長する収入ポートフォリオに結びつける。

---

**パイプラインが稼働する。下書きが準備される。あなたは10分レビューする。**

**それがタクティカル・オートメーションだ。それがスケールの方法だ。**

*あなたの機材。あなたのルール。あなたの収益。*
