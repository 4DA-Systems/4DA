# 모듈 T: 전술적 자동화

**STREETS 개발자 수입 코스 — 유료 모듈**
*12-13주차 | 전 6강 | 산출물: 가치를 생성하는 자동화 파이프라인 1개*

> "LLM, 에이전트, MCP, 크론 작업을 힘의 배수로 사용합니다."

---

수익 엔진이 가동 중입니다. 고객이 있습니다. 작동하는 프로세스가 있습니다. 그리고 시간의 60-70%를 같은 일을 반복하는 데 쓰고 있습니다: 입력 처리, 출력 포맷팅, 모니터 확인, 업데이트 전송, 큐 검토.

그 시간은 가장 비싼 자원이며, 월 {= regional.currency_symbol | fallback("$") =}5짜리 VPS가 처리할 수 있는 작업에 그것을 태우고 있습니다.

{@ insight hardware_benchmark @}

이 모듈은 체계적으로 자신을 루프에서 제거하는 것에 대한 것입니다 — 완전히가 아니라(그것은 5강에서 다룰 함정입니다), 판단이 필요하지 않은 80%의 작업에서 제거하는 것입니다. 결과적으로 수입원은 잠자는 동안, 본업을 하는 동안, 다음 것을 만드는 동안에도 수익을 창출합니다.

이 2주가 끝날 때까지 다음을 갖게 됩니다:

- 자동화의 4단계와 오늘 자신이 어디에 있는지에 대한 명확한 이해
- 인프라에서 실행 중인 크론 작업과 예약 자동화
- 참여 없이 입력을 처리하는 LLM 기반 파이프라인 최소 1개
- 에이전트 기반 시스템과 그것이 경제적으로 의미 있는 시점에 대한 이해
- 자동화가 평판을 파괴하지 않도록 하는 휴먼 인 더 루프 프레임워크
- 적극적인 참여 없이 가치를 생성하는, 완전히 배포된 파이프라인 1개

{? if stack.primary ?}
주요 기술 스택이 {= stack.primary | fallback("your primary stack") =}이므로, 앞으로의 자동화 예제는 해당 생태계에 적용할 때 가장 직접적으로 활용할 수 있습니다. 대부분의 예제는 이식성을 위해 Python을 사용하지만, 패턴은 어떤 언어로든 변환할 수 있습니다.
{? endif ?}

이것은 코스에서 가장 코드가 많은 모듈입니다. 다음 내용의 최소 절반은 실행 가능한 코드입니다. 복사하고, 적응시키고, 배포하십시오.

자동화를 시작하겠습니다.

---

## 1강: 자동화 피라미드

*"대부분의 개발자는 레벨 1에서 자동화합니다. 돈은 레벨 3에 있습니다."*

### 4단계

수입 스택의 모든 자동화는 이 피라미드의 어딘가에 위치합니다:

```
┌───────────────────────────────┐
│  레벨 4: 자율 에이전트         │  ← 대신 의사결정을 합니다
│  (AI가 결정하고 행동합니다)    │
├───────────────────────────────┤
│  레벨 3: 인텔리전트            │  ← 돈은 여기 있습니다
│  파이프라인 (LLM 기반)        │
├───────────────────────────────┤
│  레벨 2: 예약 자동화           │  ← 대부분의 개발자가 여기서 멈춥니다
│  (cron + 스크립트)            │
├───────────────────────────────┤
│  레벨 1: 템플릿을 활용한       │  ← 대부분의 개발자가 여기 있습니다
│  수동 작업 (복사-붙여넣기)     │
└───────────────────────────────┘
```

각 레벨이 실제로 어떤 모습인지 구체적으로 살펴보겠습니다.

### 레벨 1: 템플릿을 활용한 수동 작업

작업은 직접 하지만, 속도를 높이기 위한 체크리스트, 템플릿, 스니펫이 있습니다.

**예시:**
- 미리 채워진 프론트매터가 있는 마크다운 템플릿으로 블로그 글을 작성합니다
- 지난달 청구서를 복제하고 숫자를 바꿔서 클라이언트에게 청구합니다
- 저장된 답변 템플릿을 사용하여 지원 이메일에 답합니다
- 배포 명령을 수동으로 실행하여 콘텐츠를 발행합니다

**시간 비용:** 출력 단위당 시간의 100%.
**오류율:** 보통 — 사람이므로 피곤하면 실수합니다.
**확장 한계:** 본인. 본인의 시간. 그것뿐입니다.

대부분의 개발자가 여기에 살면서 위에 피라미드가 있다는 것조차 인식하지 못합니다.

### 레벨 2: 예약 자동화

스크립트가 일정에 따라 실행됩니다. 로직을 한 번 작성했습니다. 본인 없이 실행됩니다.

**예시:**
- RSS 피드를 확인하고 새 글을 소셜 미디어에 게시하는 크론 작업
- 매일 오전 6시에 사이트를 빌드하고 배포하는 GitHub Action
- 매시간 실행되어 경쟁사 가격을 확인하고 변경 사항을 기록하는 스크립트
- 오전 3시에 실행되는 일일 데이터베이스 백업

**시간 비용:** 지속적으로 제로(초기 설정 1-4시간 후).
**오류율:** 낮음 — 결정론적이며 매번 같은 로직.
**확장 한계:** 머신이 예약할 수 있는 작업 수만큼. 수백 개.

대부분의 기술적 개발자가 도달하는 곳입니다. 편안합니다. 하지만 하드 리밋이 있습니다: 결정론적 로직이 있는 작업만 처리할 수 있습니다. 작업에 판단이 필요하면 막히게 됩니다.

### 레벨 3: 인텔리전트 파이프라인

스크립트가 일정에 따라 실행되지만, 판단 결정을 처리하는 LLM이 포함되어 있습니다.

**예시:**
- RSS 피드가 수집되고, LLM이 각 기사를 요약하며, 뉴스레터 초안을 작성하고, 10분 검토 후 발송합니다
- 고객 피드백 이메일이 감정과 긴급도에 따라 분류되고, 사전 작성된 답변이 승인 대기 큐에 올라갑니다
- 니치의 새 구인 공고가 스크래핑되고, LLM이 관련성을 평가하여, 200개 목록을 스캔하는 대신 매일 5개의 기회 요약을 받습니다
- 경쟁사 블로그 글이 모니터링되고, LLM이 주요 제품 변경 사항을 추출하며, 매주 경쟁 인텔리전스 보고서를 받습니다

**시간 비용:** 수동 시간의 10-20%. 생성하는 대신 검토하고 승인합니다.
**오류율:** 분류 작업에서는 낮고, 생성에서는 보통(그래서 검토하는 것입니다).
**확장 한계:** 하루에 수천 개의 항목. 병목은 시간이 아니라 API 비용입니다.

**돈은 여기에 있습니다.** 레벨 3은 보통 3-5명 팀이 필요한 수입원을 한 사람이 운영할 수 있게 합니다.

### 레벨 4: 자율 에이전트

AI 시스템이 참여 없이 관찰하고, 결정하고, 행동합니다.

**예시:**
- SaaS 지표를 모니터링하고, 가입 감소를 감지하며, 가격 변경을 A/B 테스트하고, 효과가 없으면 되돌리는 에이전트
- 1단계 고객 질문을 완전히 자율적으로 처리하고, 복잡한 문제만 에스컬레이션하는 지원 에이전트
- 트렌드 토픽을 식별하고, 초안을 생성하며, 발행을 예약하고, 성과를 모니터링하는 콘텐츠 에이전트

**시간 비용:** 처리된 건에 대해서는 거의 제로. 개별 행동이 아닌 지표를 검토합니다.
**오류율:** 전적으로 가드레일에 달려 있습니다. 없으면: 높음. 좋은 가드레일이 있으면: 좁은 도메인에서 놀라울 정도로 낮습니다.
**확장 한계:** 에이전트 범위 내의 작업에 대해 사실상 무제한.

레벨 4는 현실적이고 달성 가능하지만, 시작점은 아닙니다. 그리고 5강에서 다루듯이, 구현이 부실한 완전 자율 고객 대면 에이전트는 평판에 위험합니다.

> **솔직하게 말합니다:** 지금 레벨 1에 있다면, 레벨 4로 뛰어넘으려 하지 마십시오. 프로덕션에서 깨지고 고객 신뢰를 손상시키는 "자율 에이전트"를 만드는 데 몇 주를 보내게 됩니다. 피라미드를 한 단계씩 올라가십시오. 레벨 2는 오후 한 번의 작업입니다. 레벨 3은 주말 프로젝트입니다. 레벨 4는 레벨 3이 한 달 동안 안정적으로 운영된 후에 옵니다.

### 자기 평가: 어디에 있습니까?

각 수입원에 대해 정직하게 자기 평가를 하십시오:

| 수입원 | 현재 레벨 | 주당 시간 | 자동화 목표 |
|--------|----------|---------|-----------|
| [예: 뉴스레터] | [1-4] | [X] 시간 | [목표 레벨] |
| [예: 클라이언트 처리] | [1-4] | [X] 시간 | [목표 레벨] |
| [예: 소셜 미디어] | [1-4] | [X] 시간 | [목표 레벨] |
| [예: 지원] | [1-4] | [X] 시간 | [목표 레벨] |

가장 중요한 열은 "주당 시간"입니다. 가장 많은 시간을 소비하면서 가장 낮은 레벨에 있는 스트림이 첫 번째 자동화 대상입니다. 그것이 ROI가 가장 큰 곳입니다.

### 각 레벨의 경제학

주당 10시간을 소비하고 월 {= regional.currency_symbol | fallback("$") =}2,000을 생성하는 수입원이 있다고 가정합니다:

| 레벨 | 소요 시간 | 유효 시급 | 자동화 비용 |
|------|---------|---------|-----------|
| 레벨 1 | 10시간/주 | $50/시간 | $0 |
| 레벨 2 | 3시간/주 | $167/시간 | $5/월 (VPS) |
| 레벨 3 | 1시간/주 | $500/시간 | $30-50/월 (API) |
| 레벨 4 | 0.5시간/주 | $1,000/시간 | $50-100/월 (API + 컴퓨팅) |

레벨 1에서 레벨 3으로의 이동은 수익을 변경하지 않습니다. 유효 시급을 $50에서 $500으로 바꿉니다. 그리고 확보된 9시간은 다음 수입원 구축이나 현재 수입원 개선에 사용할 수 있습니다.

> **흔한 실수:** "쉬우니까" 수익이 가장 낮은 스트림을 먼저 자동화하는 것입니다. 아닙니다. 수익 대비 가장 많은 시간을 소비하는 스트림을 자동화하십시오. 거기에 ROI가 있습니다.

### 차례입니다

1. 보유하고 있는(또는 계획 중인) 모든 수입원에 대해 위의 자기 평가 표를 채우십시오.
2. ROI가 가장 높은 자동화 대상을 식별하십시오: 가장 많은 시간을 소비하면서 자동화 레벨이 가장 낮은 스트림.
3. 해당 스트림에서 가장 시간이 많이 드는 3가지 작업을 적으십시오. 2강에서 첫 번째를 자동화할 것입니다.

---

## 2강: 레벨 1에서 2로 — 예약 자동화

*"크론은 1975년에 만들어졌습니다. 여전히 작동합니다. 사용하십시오."*

### 크론 작업 기본

{? if computed.os_family == "windows" ?}
Windows를 사용하고 있으므로 크론은 시스템에 기본 내장되어 있지 않습니다. 두 가지 선택이 있습니다: WSL(Windows Subsystem for Linux)을 사용하여 진짜 크론을 얻거나, Windows 작업 스케줄러(아래에서 설명)를 사용하는 것입니다. WSL에 익숙하다면 추천합니다 — 이 강의의 모든 크론 예제가 WSL에서 직접 작동합니다. 네이티브 Windows를 선호한다면, 이 뒤의 작업 스케줄러 섹션으로 건너뛰십시오.
{? endif ?}

네, 2026년에도 크론은 예약 작업의 왕입니다. 안정적이고, 어디에나 있으며, 클라우드 계정이나 SaaS 구독, 또는 매번 구글링해야 하는 YAML 스키마가 필요하지 않습니다.

**30초 만에 이해하는 크론 문법:**

```
┌───────── 분 (0-59)
│ ┌───────── 시 (0-23)
│ │ ┌───────── 일 (1-31)
│ │ │ ┌───────── 월 (1-12)
│ │ │ │ ┌───────── 요일 (0-7, 0과 7 = 일요일)
│ │ │ │ │
* * * * *  명령
```

**자주 사용하는 스케줄:**

```bash
# 매시간
0 * * * *  /path/to/script.sh

# 매일 오전 6시
0 6 * * *  /path/to/script.sh

# 매주 월요일 오전 9시
0 9 * * 1  /path/to/script.sh

# 15분마다
*/15 * * * *  /path/to/script.sh

# 매월 1일 자정
0 0 1 * *  /path/to/script.sh
```

**크론 작업 설정:**

```bash
# crontab 편집
crontab -e

# 기존 크론 작업 목록 보기
crontab -l

# 중요: 항상 상단에서 환경 변수를 설정하십시오
# 크론은 최소 환경에서 실행됩니다 — PATH에 도구가 포함되지 않을 수 있습니다
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
HOME=/home/youruser

# 출력을 로그에 기록하여 실패를 디버그할 수 있도록 합니다
0 6 * * * /home/youruser/scripts/daily-report.sh >> /home/youruser/logs/daily-report.log 2>&1
```

> **흔한 실수:** 수동으로 실행하면 완벽하게 작동하는 스크립트를 작성한 후, 크론에서 `.bashrc`나 `.zshrc`를 로드하지 않아 조용히 실패하는 것입니다. 크론 스크립트에서 항상 절대 경로를 사용하십시오. crontab 상단에서 항상 `PATH`를 설정하십시오. 항상 출력을 로그 파일로 리다이렉트하십시오.

### 크론이 부족할 때의 클라우드 스케줄러

머신이 24/7 가동되지 않거나 더 강건한 것이 필요한 경우, 클라우드 스케줄러를 사용합니다:

**GitHub Actions(공개 리포는 무료, 비공개 월 2,000분):**

```yaml
# .github/workflows/scheduled-task.yml
name: Daily Content Publisher

on:
  schedule:
    # 매일 UTC 오전 6시
    - cron: '0 6 * * *'
  # 테스트를 위한 수동 트리거 허용
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

**Vercel Cron(Hobby 플랜 무료 하루 1회; Pro 플랜: 무제한):**

```typescript
// api/cron/daily-report.ts
// Vercel cron 엔드포인트 — vercel.json에서 스케줄 설정

import type { NextRequest } from 'next/server';

export const config = {
  runtime: 'edge',
};

export default async function handler(req: NextRequest) {
  // Vercel의 호출인지 확인하고, 임의의 HTTP 요청이 아닌지 검증합니다
  const authHeader = req.headers.get('authorization');
  if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
    return new Response('Unauthorized', { status: 401 });
  }

  // 여기에 자동화 로직
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

### 지금 바로 만들 수 있는 실용적인 자동화

오늘 구현할 수 있는 5가지 자동화입니다. 각각 30-60분이 소요되며, 매주 수시간의 수동 작업을 제거합니다.

#### 자동화 1: 예약된 콘텐츠 자동 발행

블로그 글을 미리 작성합니다. 이 스크립트가 예약된 시간에 발행합니다.

```python
#!/usr/bin/env python3
"""
scheduled_publisher.py — 예약된 날짜에 마크다운 글을 발행합니다.
크론으로 매일 실행: 0 6 * * * python3 /path/to/scheduled_publisher.py
"""

import os
import json
import glob
import requests
from datetime import datetime, timezone
from pathlib import Path

CONTENT_DIR = os.path.expanduser("~/income/content/posts")
PUBLISHED_LOG = os.path.expanduser("~/income/content/published.json")

# CMS API 엔드포인트 (Hashnode, Dev.to, Ghost 등)
CMS_API_URL = os.environ.get("CMS_API_URL", "https://api.example.com/posts")
CMS_API_KEY = os.environ.get("CMS_API_KEY", "")

def load_published():
    """이미 발행된 글의 파일명 목록을 로드합니다."""
    try:
        with open(PUBLISHED_LOG, "r") as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()

def save_published(published: set):
    """발행된 글의 파일명 목록을 저장합니다."""
    with open(PUBLISHED_LOG, "w") as f:
        json.dump(sorted(published), f, indent=2)

def parse_frontmatter(filepath: str) -> dict:
    """마크다운 파일에서 YAML 스타일 프론트매터를 추출합니다."""
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
    """글을 오늘 발행해야 하는지 확인합니다."""
    publish_date = metadata.get("publish_date", "")
    if not publish_date:
        return False

    try:
        scheduled = datetime.strptime(publish_date, "%Y-%m-%d").date()
        return scheduled <= datetime.now(timezone.utc).date()
    except ValueError:
        return False

def publish_post(metadata: dict) -> bool:
    """CMS API에 글을 발행합니다."""
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

**마크다운 글은 다음과 같습니다:**

```markdown
---
title: "How to Deploy Ollama Behind Nginx"
publish_date: "2026-03-15"
tags: ollama, deployment, nginx
---

글 내용은 여기에...
```

영감이 떠오를 때 글을 작성합니다. 날짜를 설정합니다. 스크립트가 나머지를 처리합니다.

#### 자동화 2: 새 콘텐츠 발행 시 소셜 미디어에 자동 게시

블로그가 새 글을 발행하면, Twitter/X와 Bluesky에 자동으로 게시합니다.

```python
#!/usr/bin/env python3
"""
social_poster.py — 새 콘텐츠가 발행되면 소셜 플랫폼에 게시합니다.
30분마다 실행: */30 * * * * python3 /path/to/social_poster.py
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
    """RSS 피드를 파싱하고 항목 목록을 반환합니다."""
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
    """AT Protocol로 Bluesky에 게시합니다."""
    # 단계 1: 세션 생성
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

    # 단계 2: 포스트 생성
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

        # 소셜 포스트 포맷팅
        text = f"{item['title']}\n\n{item['link']}"

        # Bluesky는 300자 제한이 있습니다
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

비용: $0. 본인의 머신이나 무료 GitHub Action에서 실행합니다.

#### 자동화 3: 경쟁사 가격 모니터

경쟁사가 가격을 변경하는 순간 알 수 있습니다. 더 이상 매주 수동으로 확인하지 않아도 됩니다.

```python
#!/usr/bin/env python3
"""
price_monitor.py — 경쟁사 가격 페이지의 변경을 모니터링합니다.
6시간마다 실행: 0 */6 * * * python3 /path/to/price_monitor.py
"""

import os
import json
import hashlib
import requests
from datetime import datetime
from pathlib import Path

MONITOR_DIR = os.path.expanduser("~/income/monitors")
ALERT_WEBHOOK = os.environ.get("SLACK_WEBHOOK_URL", "")  # 또는 Discord, 이메일 등

COMPETITORS = [
    {
        "name": "CompetitorA",
        "url": "https://competitor-a.com/pricing",
        "css_selector": None  # 전체 페이지 모니터링용; 특정 요소에는 선택자 사용
    },
    {
        "name": "CompetitorB",
        "url": "https://competitor-b.com/pricing",
        "css_selector": None
    },
]

def get_page_hash(url: str) -> tuple[str, str]:
    """페이지를 가져와 콘텐츠 해시와 텍스트 발췌를 반환합니다."""
    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; PriceMonitor/1.0)"
    }
    response = requests.get(url, headers=headers, timeout=30)
    response.raise_for_status()
    content = response.text
    content_hash = hashlib.sha256(content.encode()).hexdigest()
    # 문맥을 위해 가시적 텍스트의 처음 500자를 가져옵니다
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
    """Slack 웹훅으로 알림을 보냅니다(Discord, 이메일 등으로 교체 가능)."""
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

#### 자동화 4: 주간 수익 보고서

매주 월요일 아침, 수익 데이터에서 보고서를 생성하고 이메일로 보냅니다.

```python
#!/usr/bin/env python3
"""
weekly_report.py — 추적 스프레드시트/데이터베이스에서 주간 수익 보고서를 생성합니다.
월요일 오전 7시에 실행: 0 7 * * 1 python3 /path/to/weekly_report.py
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
    """revenue 테이블이 없으면 생성합니다."""
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
    """일반 텍스트 주간 보고서를 생성합니다."""
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
    """이메일로 보고서를 보냅니다."""
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

#### 자동화 5: 클라이언트 데이터 자동 백업

클라이언트 산출물을 다시는 잃어버리지 않습니다. 매일 밤 실행되며 30일치 백업을 유지합니다.

```bash
#!/bin/bash
# backup_client_data.sh — 클라이언트 프로젝트 데이터의 야간 백업.
# Cron: 0 3 * * * /home/youruser/scripts/backup_client_data.sh

BACKUP_DIR="$HOME/income/backups"
SOURCE_DIR="$HOME/income/projects"
DATE=$(date +%Y-%m-%d)
RETENTION_DAYS=30

mkdir -p "$BACKUP_DIR"

# 압축 백업 생성
tar -czf "$BACKUP_DIR/projects-$DATE.tar.gz" \
    -C "$SOURCE_DIR" . \
    --exclude='node_modules' \
    --exclude='.git' \
    --exclude='target' \
    --exclude='__pycache__'

# 보존 기간보다 오래된 백업 삭제
find "$BACKUP_DIR" -name "projects-*.tar.gz" -mtime +"$RETENTION_DAYS" -delete

# 로그
BACKUP_SIZE=$(du -h "$BACKUP_DIR/projects-$DATE.tar.gz" | cut -f1)
echo "$(date -Iseconds) Backup complete: $BACKUP_SIZE" >> "$HOME/income/logs/backup.log"

# 선택사항: 두 번째 위치에 동기화 (외장 드라이브, 다른 머신)
# rsync -a "$BACKUP_DIR/projects-$DATE.tar.gz" /mnt/external/backups/
```

### 더 많은 제어가 필요할 때의 systemd 타이머

크론이 제공하는 것 이상 — 의존성 순서 지정, 리소스 제한, 자동 재시도 등 — 이 필요한 경우 systemd 타이머를 사용합니다:

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
# 실패 시 지수 백오프로 재시작
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
# 오전 6시에 머신이 꺼져 있었으면, 온라인으로 돌아올 때 실행
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
```

```bash
# 타이머 활성화 및 시작
sudo systemctl enable income-publisher.timer
sudo systemctl start income-publisher.timer

# 상태 확인
systemctl list-timers --all | grep income

# 로그 보기
journalctl -u income-publisher.service --since today
```

{? if computed.os_family == "windows" ?}
### Windows 작업 스케줄러 대안

WSL을 사용하지 않는 경우, Windows 작업 스케줄러가 같은 역할을 합니다. 명령줄에서 `schtasks`를 사용하거나 작업 스케줄러 GUI(`taskschd.msc`)를 사용합니다. 주요 차이점: 크론은 단일 표현식을 사용하고, 작업 스케줄러는 트리거, 작업, 조건에 별도의 필드를 사용합니다. 이 강의의 모든 크론 예제는 직접 변환할 수 있습니다 — Python 스크립트를 같은 방식으로 예약하되, 다른 인터페이스를 통해서 할 뿐입니다.
{? endif ?}

### 차례입니다

1. 이 강의에서 수입원에 적용할 수 있는 가장 간단한 자동화를 선택합니다.
2. 구현합니다. "구현을 계획한다"가 아닙니다. 코드를 작성하고, 테스트하고, 예약합니다.
3. 실행을 확인할 수 있도록 로깅을 설정합니다. 3일 동안 매일 아침 로그를 확인합니다.
4. 안정되면 매일 확인을 중단합니다. 매주 확인합니다. 그것이 자동화입니다.

**최소 요건:** 오늘 하루가 끝나기 전에 안정적으로 실행되는 크론 작업 1개.

---

## 3강: 레벨 2에서 3으로 — LLM 기반 파이프라인

*"자동화에 지능을 추가합니다. 여기서 한 사람이 팀처럼 보이기 시작합니다."*

### 패턴

모든 LLM 기반 파이프라인은 같은 형태를 따릅니다:

```
입력 소스 → 수집 → LLM 처리 → 출력 포맷팅 → 전달 (또는 검토 큐)
```

마법은 "LLM 처리" 단계에 있습니다. 가능한 모든 경우에 대해 결정론적 규칙을 작성하는 대신, 원하는 것을 자연어로 설명하면 LLM이 판단을 처리합니다.

### 로컬 vs API 사용 시점

{? if settings.has_llm ?}
{= settings.llm_provider | fallback("LLM 제공자") =}를 {= settings.llm_model | fallback("LLM 모델") =}로 설정했습니다. 즉, 인텔리전트 파이프라인 구축을 즉시 시작할 수 있습니다. 아래 결정은 각 파이프라인에서 로컬 설정과 API 중 어떤 것을 사용할지 선택하는 데 도움이 됩니다.
{? else ?}
아직 LLM을 설정하지 않았습니다. 이 강의의 파이프라인은 로컬 모델(Ollama)과 클라우드 API 모두에서 작동합니다. 첫 번째 파이프라인을 구축하기 전에 최소 하나를 설정하십시오 — Ollama는 무료이며 설치에 10분이 걸립니다.
{? endif ?}

이 결정은 마진에 직접적인 영향을 미칩니다:

| 요소 | 로컬 (Ollama) | API (Claude, GPT) |
|------|-------------|-------------------|
| **100만 토큰당 비용** | ~$0.003 (전기료) | $0.15 - $15.00 |
| **속도 (토큰/초)** | 20-60 (중급 GPU에서 8B) | 50-100+ |
| **품질 (로컬 8B vs API)** | 분류, 추출에 양호 | 생성, 추론에 우수 |
| **프라이버시** | 데이터가 머신을 떠나지 않음 | 데이터가 제공자에게 감 |
| **가동률** | 머신에 의존 | 99.9%+ |
| **배치 처리 용량** | GPU 메모리에 제한 | 속도 제한과 예산에 제한 |

{? if profile.gpu.exists ?}
머신에 {= profile.gpu.model | fallback("GPU") =}가 있으므로 로컬 추론이 강력한 옵션입니다. 실행할 수 있는 속도와 모델 크기는 VRAM에 따라 다릅니다 — 로컬 전용 파이프라인을 확정하기 전에 무엇이 맞는지 확인하십시오.
{? if computed.has_nvidia ?}
NVIDIA GPU는 CUDA 가속 덕분에 최고의 Ollama 성능을 얻을 수 있습니다. 7-8B 파라미터 모델은 편안하게 실행할 수 있으며, {= profile.gpu.vram | fallback("사용 가능한 VRAM") =}에 따라 더 큰 모델도 가능할 수 있습니다.
{? endif ?}
{? else ?}
전용 GPU 없이는 로컬 추론이 느려집니다(CPU 전용). 소규모 배치 작업과 분류 작업에는 여전히 작동하지만, 시간이 중요하거나 대량인 경우 API 모델이 더 실용적입니다.
{? endif ?}

**경험 법칙:**
- **대량, 낮은 품질 기준** (분류, 추출, 태깅) → 로컬
- **소량, 품질 중요** (고객 대면 콘텐츠, 복잡한 분석) → API
- **민감한 데이터** (클라이언트 정보, 독점 데이터) → 항상 로컬
- **월 10,000개 이상** → 로컬이 실제 비용을 절약합니다

**일반적인 파이프라인의 월간 비용 비교:**

```
월 5,000개 항목 처리, 항목당 약 500토큰:

로컬 (Ollama, llama3.1:8b):
  2,500,000 토큰 × $0.003/1M = $0.0075/월
  사실상 무료입니다.

API (GPT-4o-mini):
  2,500,000 입력 토큰 × $0.15/1M = $0.375
  2,500,000 출력 토큰 × $0.60/1M = $1.50
  합계: ~$1.88/월
  저렴하지만 로컬의 250배입니다.

API (Claude 3.5 Sonnet):
  2,500,000 입력 토큰 × $3.00/1M = $7.50
  2,500,000 출력 토큰 × $15.00/1M = $37.50
  합계: ~$45/월
  품질은 우수하지만 로컬의 6,000배입니다.
```

분류 및 추출 파이프라인에서는, 잘 설계된 프롬프트의 8B 로컬 모델과 프론티어 API 모델 사이의 품질 차이가 종종 무시할 수 있을 정도입니다. 둘 다 테스트하십시오. 품질 기준을 충족하는 더 저렴한 것을 사용하십시오.

{@ insight cost_projection @}

### 파이프라인 1: 뉴스레터 콘텐츠 생성기

이것은 콘텐츠 기반 수입을 가진 개발자에게 가장 일반적인 LLM 자동화입니다. RSS 피드가 들어가고, 뉴스레터 초안이 나옵니다.

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py — RSS 피드를 수집하고, LLM으로 요약하며, 뉴스레터 초안을 생성합니다.
매일 실행: 0 5 * * * python3 /path/to/newsletter_pipeline.py

이 파이프라인:
1. 여러 RSS 피드에서 새 기사를 가져옵니다
2. 각각을 로컬 LLM에 보내 요약합니다
3. 청중과의 관련성에 따라 순위를 매깁니다
4. 포맷된 뉴스레터 초안을 생성합니다
5. 검토용 초안을 저장합니다 (2시간 큐레이션 대신 10분 검토)
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
    # 여기에 니치 피드를 추가하십시오
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
    """RSS/Atom 피드를 파싱하고 기사를 반환합니다."""
    try:
        resp = requests.get(url, timeout=30, headers={
            "User-Agent": "NewsletterBot/1.0"
        })
        resp.raise_for_status()
        root = ET.fromstring(resp.content)

        articles = []
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
    """로컬 LLM에 프롬프트를 보내고 응답을 받습니다."""
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
    """LLM을 사용하여 관련성 점수를 매기고 요약을 생성합니다."""
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
    """점수가 매겨진 기사를 뉴스레터 초안으로 포맷합니다."""
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

    relevant = [a for a in scored if a.get("relevance", 0) >= 6]
    relevant.sort(key=lambda x: x.get("relevance", 0), reverse=True)

    top_articles = relevant[:10]

    print(f"\n{len(top_articles)} articles passed relevance threshold (>= 6/10)")

    draft = generate_newsletter(top_articles)

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

**비용:**
- 로컬 8B 모델로 하루 50개 기사 처리: ~$0/월
- 소요 시간: 수동 큐레이션 2시간 대신 초안 검토 10분
- 주당 절약 시간: 주간 뉴스레터를 운영한다면 ~10시간

### 파이프라인 2: 고객 조사 및 인사이트 보고서

이 파이프라인은 공개 데이터를 스크래핑하고, LLM으로 분석하며, 판매할 수 있는 보고서를 생성합니다.

```python
#!/usr/bin/env python3
"""
research_pipeline.py — 공개 기업/제품 데이터를 분석하고 인사이트 보고서를 생성합니다.
판매할 수 있는 서비스: 맞춤 보고서 1건당 $200-500.

사용법: python3 research_pipeline.py "Company Name" "their-website.com"
"""

import os
import sys
import json
import requests
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
# 유료 보고서 품질을 위해 더 큰 모델 사용
MODEL = os.environ.get("RESEARCH_MODEL", "llama3.1:8b")
# 또는 고객 대면 품질을 위해 API 사용:
ANTHROPIC_KEY = os.environ.get("ANTHROPIC_API_KEY", "")
USE_API = bool(ANTHROPIC_KEY)

REPORTS_DIR = os.path.expanduser("~/income/reports")

def llm_query(prompt: str, max_tokens: int = 2000) -> str:
    """설정에 따라 로컬 또는 API 모델로 라우팅합니다."""
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
    """기업에 대한 공개 데이터를 수집합니다."""
    data = {"company": company, "domain": domain}

    # 도메인 접근 가능 여부를 확인하고 기본 정보를 가져옵니다
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

    # GitHub 존재 확인
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
    """LLM을 사용하여 분석 보고서를 생성합니다."""
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

    # 최종 보고서 조립
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

**비즈니스 모델:** 맞춤 조사 보고서 1건당 $200-500을 청구합니다. 비용: API 호출 $0.05와 15분의 검토. 파이프라인이 안정되면 시간당 3-4건의 보고서를 생산할 수 있습니다.

### 파이프라인 3: 시장 신호 모니터

이것은 다음에 무엇을 만들어야 하는지 알려주는 파이프라인입니다. 여러 소스를 모니터링하고, 신호를 분류하며, 기회가 임계값을 넘으면 알림을 보냅니다.

```python
#!/usr/bin/env python3
"""
signal_monitor.py — 공개 소스에서 시장 기회를 모니터링합니다.
2시간마다 실행: 0 */2 * * * python3 /path/to/signal_monitor.py
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

# 니치 정의 — LLM이 관련성 점수 매기기에 사용합니다
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
    """Hacker News 인기 기사를 가져옵니다."""
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
    """LLM을 사용하여 시장 기회로 신호를 분류합니다."""
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
    """높은 점수의 기회에 대해 알림을 보냅니다."""
    msg = (
        f"OPPORTUNITY DETECTED (score: {item['opportunity_score']}/10)\n"
        f"Type: {item['opportunity_type']}\n"
        f"Title: {item['title']}\n"
        f"URL: {item.get('url', 'N/A')}\n"
        f"Why: {item['reasoning']}\n"
        f"Action: {item['action']}"
    )

    # 파일에 기록
    os.makedirs(DATA_DIR, exist_ok=True)
    with open(ALERTS_FILE, "a") as f:
        entry = {**item, "alerted_at": datetime.utcnow().isoformat() + "Z"}
        f.write(json.dumps(entry) + "\n")

    # Slack/Discord로 전송
    if SLACK_WEBHOOK:
        try:
            requests.post(SLACK_WEBHOOK, json={"text": msg}, timeout=10)
        except Exception:
            pass

    print(f"  ALERT: {msg}")

def main():
    seen = load_seen()

    # 소스에서 가져오기
    print("Fetching signals...")
    items = fetch_hn_top(30)
    # 여기에 더 많은 소스 추가: Reddit, RSS 피드, GitHub 트렌드 등

    new_items = [i for i in items if i["id"] not in seen]
    print(f"  {len(new_items)} new signals to classify")

    # 각 신호를 분류
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

**실제 동작:** 주에 2-3번 "기회: 스타터 킷이 없는 새 프레임워크가 출시됨 — 이번 주말에 만들 수 있습니다"라는 Slack 알림을 받습니다. 다른 사람보다 먼저 그 신호에 행동하는 것이 앞서가는 방법입니다.

> **솔직하게 말합니다:** 이러한 파이프라인 출력의 품질은 프롬프트와 니치 정의에 전적으로 달려 있습니다. 니치가 모호하면("저는 웹 개발자입니다"), LLM이 모든 것에 플래그를 세웁니다. 구체적이면("프라이버시 우선 개발자 시장을 위한 Tauri 데스크탑 앱을 만듭니다"), 외과 수술처럼 정확해집니다. 니치 정의를 올바르게 하는 데 30분을 투자하십시오. 구축하는 모든 파이프라인에 대한 가장 레버리지가 높은 입력입니다.

### 차례입니다

{? if stack.contains("python") ?}
좋은 소식입니다: 위의 파이프라인 예제는 이미 주요 언어로 되어 있습니다. 그대로 복사하고 적응을 시작할 수 있습니다. 니치 정의와 프롬프트를 올바르게 하는 데 집중하십시오 — 출력 품질의 90%가 거기에서 나옵니다.
{? else ?}
위의 예제는 이식성을 위해 Python을 사용하지만, 패턴은 어떤 언어에서든 작동합니다. {= stack.primary | fallback("주요 기술 스택") =}으로 구축하고 싶다면, 복제해야 할 핵심 요소는: RSS/API 가져오기용 HTTP 클라이언트, LLM 응답용 JSON 파싱, 상태 관리용 파일 I/O입니다. LLM과의 상호작용은 Ollama나 클라우드 API에 대한 HTTP POST에 불과합니다.
{? endif ?}

1. 위의 세 파이프라인(뉴스레터, 조사, 신호 모니터) 중 하나를 선택합니다.
2. 자신의 니치에 맞게 적응시킵니다. 피드, 청중 설명, 분류 기준을 변경합니다.
3. 출력 품질을 테스트하기 위해 수동으로 3번 실행합니다.
4. 출력이 큰 편집 없이 사용 가능할 때까지 프롬프트를 조정합니다.
5. 크론으로 예약합니다.

**목표:** 이 강의를 읽은 후 48시간 이내에 예약 실행되는 LLM 기반 파이프라인 1개.

---

## 4강: 레벨 3에서 4로 — 에이전트 기반 시스템

*"에이전트는 관찰하고, 결정하고, 행동하는 루프에 불과합니다. 하나 만드십시오."*

### 2026년 "에이전트"의 실제 의미

과대 광고를 벗겨냅니다. 에이전트는 다음을 하는 프로그램입니다:

1. **관찰합니다** — 어떤 입력이나 상태를 읽습니다
2. **결정합니다** — LLM을 사용하여 무엇을 할지 결정합니다
3. **행동합니다** — 결정을 실행합니다
4. **반복합니다** — 1단계로 돌아갑니다

그것뿐입니다. 파이프라인(레벨 3)과 에이전트(레벨 4)의 차이는 에이전트가 루프한다는 것입니다. 자신의 출력에 대해 행동합니다. 다음 단계가 이전 단계의 결과에 따라 달라지는 다단계 작업을 처리합니다.

파이프라인은 고정된 순서로 항목을 하나씩 처리합니다. 에이전트는 만나는 것에 따라 예측 불가능한 순서를 탐색합니다.

### 고객에게 서비스하는 MCP 서버

MCP 서버는 구축할 수 있는 가장 실용적인 에이전트 인접 시스템 중 하나입니다. AI 에이전트(Claude Code, Cursor 등)가 고객을 대신하여 호출할 수 있는 도구를 노출합니다.

{? if stack.contains("typescript") ?}
아래 MCP 서버 예제는 TypeScript를 사용합니다 — 바로 전문 분야입니다. 기존 TypeScript 도구로 확장하고 다른 Node.js 서비스와 함께 배포할 수 있습니다.
{? endif ?}

실제 예제를 보겠습니다: 제품 문서에서 고객의 질문에 답변하는 MCP 서버입니다.

```typescript
// mcp-docs-server/src/index.ts
// 문서에서 질문에 답변하는 MCP 서버.
// 고객이 Claude Code를 이 서버에 연결하면 즉시 답변을 받습니다.

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import * as fs from "fs";
import * as path from "path";

// 시작 시 문서를 메모리에 로드
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

    // 제목으로 분할하여 더 나은 검색 제공
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
  // 간단한 키워드 검색 — 프로덕션에서는 벡터 검색으로 교체
  const queryWords = query.toLowerCase().split(/\s+/);

  const scored = docs.map((chunk) => {
    const text = `${chunk.section} ${chunk.content}`.toLowerCase();
    let score = 0;
    for (const word of queryWords) {
      if (text.includes(word)) score++;
      // 제목 매치에 보너스
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

// 초기화
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

// 서버 시작
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

**비즈니스 모델:** 이 MCP 서버를 제품의 일부로 고객에게 제공합니다. 고객은 지원 티켓을 제출하지 않고도 즉시 답변을 받습니다. 지원에 소요되는 시간이 줄어듭니다. 모두가 이깁니다.

프리미엄: 벡터 검색, 버전 관리 문서, 고객이 무엇을 묻고 있는지에 대한 분석이 포함된 호스팅 버전을 월 $9-29에 청구합니다.

### 자동화된 고객 피드백 처리

이 에이전트는 고객 피드백(이메일, 지원 티켓, 폼에서)을 읽고, 분류하며, 답변 초안과 기능 티켓을 생성합니다.

```python
#!/usr/bin/env python3
"""
feedback_agent.py — 고객 피드백을 분류된 실행 가능 항목으로 처리합니다.
"AI가 초안, 인간이 승인" 패턴.

매시간 실행: 0 * * * * python3 /path/to/feedback_agent.py
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
    """피드백을 분류하고 답변 초안을 생성합니다."""

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

        # 처리된 버전 저장
        processed_path = os.path.join(PROCESSED_DIR, filepath.name)
        with open(processed_path, "w") as f:
            json.dump(processed, f, indent=2)

        # 검토 큐에 추가
        review_queue.append({
            "file": filepath.name,
            "from": processed.get("from", "Unknown"),
            "category": processed.get("category", "unknown"),
            "urgency": processed.get("urgency", "medium"),
            "summary": processed.get("summary", ""),
            "needs_human": processed.get("needs_human", True),
            "draft_response": processed.get("draft_response", "")
        })

        # 원본을 인박스에서 이동
        filepath.rename(os.path.join(PROCESSED_DIR, f"original-{filepath.name}"))

    # 검토 큐 기록
    review_path = os.path.join(REVIEW_DIR, f"review-{datetime.now().strftime('%Y%m%d-%H%M')}.json")
    with open(review_path, "w") as f:
        json.dump(review_queue, f, indent=2)

    # 요약
    critical = sum(1 for item in review_queue if item["urgency"] == "critical")
    needs_human = sum(1 for item in review_queue if item["needs_human"])

    print(f"\nProcessed: {len(review_queue)}")
    print(f"Critical: {critical}")
    print(f"Needs your attention: {needs_human}")
    print(f"Review queue: {review_path}")

if __name__ == "__main__":
    main()
```

**실제 동작:**
1. 고객이 피드백을 제출합니다 (폼, 이메일 또는 지원 시스템)
2. 피드백이 인박스 디렉토리에 JSON 파일로 도착합니다
3. 에이전트가 각각을 처리합니다: 분류, 요약, 답변 초안
4. 하루에 한두 번 검토 큐를 엽니다
5. 간단한 항목(칭찬, 좋은 초안 답변이 있는 기본 질문)은 초안을 승인합니다
6. 복잡한 항목(버그, 화난 고객)은 개인적인 답변을 작성합니다
7. 순 시간: 2시간 대신 하루 15분

### AI 초안, 인간 승인 패턴

이 패턴은 실용적인 레벨 4 자동화의 핵심입니다. 에이전트가 단순 작업을 처리합니다. 판단이 필요한 것은 직접 처리합니다.

```
              ┌──────────────┐
              │ 에이전트가    │
              │ 초안 작성     │
              └──────┬───────┘
                     │
              ┌──────▼───────┐
              │  검토 큐      │
              └──────┬───────┘
                     │
          ┌──────────┼──────────┐
          │          │          │
    ┌─────▼─────┐ ┌──▼──┐ ┌────▼────┐
    │ 자동 전송 │ │편집 │ │에스컬   │
    │ (일상)    │ │+전송│ │레이션   │
    │           │ │     │ │(복잡)   │
    └───────────┘ └─────┘ └─────────┘
```

**에이전트가 완전히 처리하는 것 vs 보내기 전에 검토하는 것의 규칙:**

| 에이전트가 완전히 처리 (검토 불필요) | 보내기 전에 검토 |
|------------------------------------|----------------|
| 수신 확인 ("메시지를 받았습니다") | 화난 고객에 대한 답변 |
| 상태 업데이트 ("요청이 처리 중입니다") | 기능 요청 우선순위 결정 |
| FAQ 답변 (정확한 일치) | 돈과 관련된 모든 것 (환불, 가격 책정) |
| 스팸 분류 및 삭제 | 버그 보고서 (검증이 필요합니다) |
| 내부 로깅 및 분류 | 이전에 본 적 없는 모든 것 |

> **흔한 실수:** 첫날부터 에이전트가 고객에게 자율적으로 응답하도록 하는 것입니다. 하지 마십시오. 에이전트가 모든 것을 초안하고, 모든 것을 승인하는 것부터 시작하십시오. 1주일 후, 수신 확인의 자동 전송을 허용합니다. 1개월 후, FAQ 답변의 자동 전송을 허용합니다. 신뢰를 점진적으로 구축하십시오 — 자신과 고객 모두에 대해서.

### 차례입니다

1. 하나를 선택합니다: MCP 문서 서버를 만들거나 피드백 처리 에이전트를 만듭니다.
2. 자신의 제품/서비스에 맞게 적응시킵니다. 아직 고객이 없다면, 3강의 신호 모니터를 "고객"으로 사용합니다 — 그 출력을 피드백 에이전트 패턴으로 처리합니다.
3. 다른 입력으로 수동으로 10번 실행합니다.
4. 측정합니다: 출력의 몇 퍼센트가 편집 없이 사용 가능합니까? 그것이 자동화 품질 점수입니다. 예약하기 전에 70% 이상을 목표로 합니다.

---

## 5강: 휴먼 인 더 루프 원칙

*"완전 자동화는 함정입니다. 부분 자동화는 초능력입니다."*

### 80% 자동화가 100%를 이기는 이유

고객 대면 프로세스를 완전히 자동화해서는 안 되는 구체적이고 측정 가능한 이유가 있습니다: 나쁜 출력의 비용이 비대칭적이기 때문입니다.

좋은 자동화 출력은 5분을 절약합니다.
나쁜 자동화 출력은 고객 상실, 공개 불만, 환불, 또는 회복에 몇 달이 걸리는 평판 타격을 초래합니다.

계산:

```
100% 자동화:
  월 1,000건 출력 × 95% 품질 = 950건 양호 + 50건 불량
  50건 불량 출력 × 평균 $50 비용 (환불 + 지원 + 평판) = 월 $2,500 손해

80% 자동화 + 20% 인간 검토:
  800건 자동 처리, 200건 인간 검토
  800 × 95% 품질 = 760건 양호 + 40건 자동 불량
  200 × 99% 품질 = 198건 양호 + 2건 인간 불량
  총 42건 불량 × $50 = 월 $2,100 손해
  그러나: 고객에게 도달하기 전에 38건의 불량을 포착합니다

  실제로 고객에게 도달하는 불량 출력: ~4건
  실제 손해: ~월 $200
```

손해 비용이 12배 감소합니다. 200건의 출력을 검토하는 시간(약 2시간)으로 월 $2,300을 절약합니다.

### 이것들은 절대 완전 자동화하지 마십시오

AI가 아무리 좋아져도, 일부는 항상 휴먼 인 더 루프가 있어야 합니다:

1. **고객 대면 커뮤니케이션** — 잘못된 표현의 이메일은 고객을 영구적으로 잃게 합니다. 일반적이고 명백히 AI인 응답은 신뢰를 침식합니다. 검토하십시오.

2. **금융 거래** — 환불, 가격 변경, 청구. 항상 검토합니다. 실수의 비용은 진짜 돈입니다.

3. **이름을 걸고 발행되는 콘텐츠** — 평판은 수년에 걸쳐 복리로 성장하며, 하나의 나쁜 게시물로 파괴될 수 있습니다. 10분의 검토는 저렴한 보험입니다.

4. **법적 또는 컴플라이언스 관련 출력** — 계약, 개인정보 정책, 이용 약관에 관련된 모든 것. AI는 자신감 넘치는 법적 실수를 합니다.

5. **채용 또는 인사 결정** — 외주를 하더라도, 누구와 일할지에 대한 최종 결정을 AI에게 맡기지 마십시오.

### 자동화 부채

{@ mirror automation_risk_profile @}

자동화 부채는 기술 부채보다 나쁩니다. 폭발하기 전까지 보이지 않기 때문입니다.

**자동화 부채의 모습:**
- 시간대가 바뀌어서 잘못된 시간에 게시하는 소셜 미디어 봇
- 아무도 확인하지 않아 3주 동안 깨진 링크를 포함하는 뉴스레터 파이프라인
- 경쟁사가 페이지를 리디자인해서 작동을 멈춘 가격 모니터
- 디스크가 가득 차서 조용히 실패하는 백업 스크립트

**예방 방법:**

매일 아침 자동화 헬스체크 스크립트를 실행합니다. 자동화가 조용히 고장났을 때(반드시 고장납니다), 3주 후가 아니라 24시간 이내에 알 수 있습니다.

### 검토 큐 구축

휴먼 인 더 루프를 효율적으로 만드는 핵심은 검토를 배치로 처리하는 것입니다. 도착하는 대로 하나씩 검토하지 마십시오. 큐에 넣고 배치로 검토합니다.

**검토 습관:** 오전 8시와 오후 4시에 검토 큐를 확인합니다. 두 번의 세션, 각 10-15분. 나머지는 검토 사이에 자율적으로 실행됩니다.

> **솔직하게 말합니다:** 인간 검토를 건너뛰면 무슨 일이 일어나는지 생각해 보십시오: 뉴스레터를 완전히 자동화하고, LLM이 존재하지 않는 페이지로의 환각된 링크를 삽입하기 시작하며, 구독자가 먼저 알아챕니다. 목록의 일부를 잃고 신뢰를 재구축하는 데 몇 달이 걸립니다. 반면, 같은 프로세스의 80%를 자동화하는 개발자 — LLM이 큐레이션과 초안을 하고, 10분 검토 — 는 발송 전에 그 환각을 포착합니다. 차이는 자동화가 아닙니다. 검토 단계입니다.

### 차례입니다

1. 2강과 3강에서 만든 자동화에 대해 `automation_healthcheck.py` 스크립트를 설정합니다. 매일 아침 실행하도록 예약합니다.
2. 가장 위험도가 높은 자동화 출력(고객 대면 항목)에 대한 검토 큐를 구현합니다.
3. 1주일 동안 하루 2번 검토 큐를 확인하겠다고 약속합니다. 변경 없이 승인한 것, 편집한 것, 거부한 것의 수를 기록합니다. 이 데이터가 자동화가 실제로 얼마나 좋은지 알려줍니다.

---

## 6강: 비용 최적화와 첫 번째 파이프라인

*"API 지출 $200에서 $200의 수익을 생성할 수 없다면, 예산이 아니라 제품을 고치십시오."*

### LLM 기반 자동화의 경제학

모든 LLM 호출에는 비용이 있습니다. 로컬 모델도 전기료와 GPU 마모가 듭니다. 문제는 그 호출의 출력이 호출 비용보다 더 많은 가치를 생성하는지 여부입니다.

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("GPU") =}에서 로컬 모델을 실행하는 비용은 전형적인 파이프라인 워크로드에서 월 약 {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("몇 달러") =}의 전기료입니다. 이것이 API 대안과 비교할 기준입니다.
{? endif ?}

**월 {= regional.currency_symbol | fallback("$") =}200 API 예산 규칙:**

자동화의 API 호출에 월 {= regional.currency_symbol | fallback("$") =}200을 지출하고 있다면, 그 자동화는 월 최소 {= regional.currency_symbol | fallback("$") =}200의 가치를 생성해야 합니다 — 직접 수익이든, 다른 곳에서 수익으로 전환하는 시간 절약이든.

그렇지 않다면: 문제는 API 예산이 아닙니다. 파이프라인 설계나 그것이 지원하는 제품입니다.

### 출력당 비용 추적

구축하는 모든 파이프라인에 비용 추적을 추가합니다. 이를 통해 각 LLM 호출의 비용과 생성하는 가치에 대한 실제 데이터를 얻을 수 있습니다.

### API 효율을 위한 배치 처리

API 모델을 사용하고 있다면, 배치 처리가 실제 비용을 절약합니다. 100번의 개별 API 호출 대신 10개 항목을 하나의 호출로 배치하면, 비용이 약 50% 절감됩니다.

### 캐싱: 같은 답변에 두 번 비용을 내지 마십시오

같은 유형의 콘텐츠를 반복 처리하는 파이프라인(분류, 추출)에서 캐싱은 API 호출의 30-50%를 제거할 수 있습니다. 월간 청구서가 30-50% 할인됩니다.

### 첫 번째 완전한 파이프라인 구축: 단계별

"수동 워크플로가 있다"에서 "잠자는 동안 실행된다"까지의 완전한 프로세스입니다.

**단계 1: 현재 수동 프로세스를 매핑합니다.**

하나의 특정 수입원에 대해 수행하는 모든 단계를 적습니다. 뉴스레터 예시:

```
1. 브라우저에서 15개 RSS 피드를 엽니다 (10분)
2. 제목을 스캔하고, 관심 있는 것을 엽니다 (20분)
3. 8-10개 기사를 상세히 읽습니다 (40분)
4. 상위 5개의 요약을 작성합니다 (30분)
5. 도입 문단을 작성합니다 (10분)
6. 이메일 도구에서 포맷합니다 (15분)
7. 목록에 전송합니다 (5분)

합계: 약 2시간 10분
```

**단계 2: 가장 시간이 많이 드는 3단계를 식별합니다.**

예시에서: 기사 읽기(40분), 요약 작성(30분), 제목 스캔(20분).

**단계 3: 가장 쉬운 것부터 자동화합니다.**

제목 스캔이 자동화하기 가장 쉽습니다 — 분류이기 때문입니다. LLM이 관련성 점수를 매기고, 최상위 점수의 것만 읽습니다.

**단계 4: 절약된 시간과 품질을 측정합니다.**

제목 스캔 자동화 후:
- 절약된 시간: 20분
- 품질: 수동 선택과 90% 일치
- 순 결과: 20분 절약, 무시할 수 있는 품질 저하

**단계 5: 다음 단계를 자동화합니다.**

이제 요약 작성을 자동화합니다. LLM이 요약 초안을 작성하고, 직접 편집합니다.

**단계 6: 수확 체감까지 계속합니다.**

```
자동화 전: 뉴스레터 1건당 2시간 10분
레벨 2 후 (예약 가져오기): 1시간 45분
레벨 3 후 (LLM 점수 매기기 + 요약): 25분
레벨 3+ 후 (LLM 도입 초안): 검토만 10분

주당 절약 시간: 약 2시간
월당 절약 시간: 약 8시간
유효 시급 $100에서: 해방된 시간으로 월 $800
API 비용: $0 (로컬 LLM) ~ $5/월 (API)
```

**단계 7: 완전한 파이프라인을 연결합니다.**

매주 일요일 오전 5시에 실행되는 GitHub Action으로 모든 것을 연결합니다. 일어나면 초안이 기다리고 있습니다. 커피를 마시며 10분 검토하고, 전송 버튼을 누르면 이번 주 뉴스레터가 발행됩니다.

### 차례입니다: 파이프라인을 만드십시오

이것이 모듈의 산출물입니다. 이 강의가 끝날 때까지 하나의 완전한 파이프라인이 배포되어 실행 중이어야 합니다.

**파이프라인 요구사항:**
1. 참여 없이 예약에 따라 실행됩니다
2. 최소 하나의 LLM 처리 단계를 포함합니다
3. 품질 관리를 위한 인간 검토 단계가 있습니다
4. 고장나면 알 수 있는 헬스 체크가 있습니다
5. 실제 수입원(또는 구축 중인 스트림)에 연결되어 있습니다

**체크리스트:**

- [ ] 자동화할 수입원을 선택했습니다
- [ ] 수동 프로세스를 매핑했습니다 (모든 단계, 시간 추정치 포함)
- [ ] 가장 시간이 많이 드는 3단계를 식별했습니다
- [ ] 최소 첫 번째 단계를 자동화했습니다 (분류/점수 매기기/필터링)
- [ ] 두 번째 단계에 LLM 처리를 추가했습니다 (요약/생성/추출)
- [ ] 인간 감독을 위한 검토 큐를 구축했습니다
- [ ] 자동화를 위한 헬스 체크를 설정했습니다
- [ ] 예약에 배포했습니다 (cron, GitHub Actions 또는 systemd 타이머)
- [ ] 한 전체 사이클의 비용과 시간 절약을 추적했습니다
- [ ] 파이프라인을 문서화했습니다 (무엇을 하는지, 수정 방법, 모니터링 대상)

이 체크리스트의 10개 항목을 모두 완료했다면, 레벨 3 자동화가 실행 중입니다. 매주의 시간이 해방되어 더 많은 스트림 구축이나 기존 것의 개선에 재투자할 수 있습니다.

---

## 모듈 T: 완료

{@ temporal automation_progress @}

### 이 2주간 구축한 것

1. **자동화 피라미드에 대한 이해** — 자신이 어디에 있고, 각 수입원이 어디로 향해야 하는지 알고 있습니다.
2. **크론이나 클라우드 스케줄러에서 실행되는 예약 자동화** — 다른 모든 것을 가능하게 하는 화려하지 않은 기초입니다.
3. **이전에 수동으로 했던 판단을 처리하는 LLM 기반 파이프라인** — 분류, 요약, 생성, 모니터링.
4. **고객 상호작용, 피드백 처리, MCP 기반 제품에 배포할 수 있는 에이전트 기반 패턴**.
5. **평판을 보호하면서 시간의 80% 이상을 절약하는 휴먼 인 더 루프 프레임워크**.
6. **자동화가 활동이 아닌 수익을 생성하도록 하는 비용 추적 및 최적화**.
7. **적극적인 참여 없이 가치를 생성하는 완전히 배포된 파이프라인 1개**.

### 복리 효과

이 모듈에서 구축한 것을 향후 3개월 동안 유지하고 확장하면 다음과 같이 됩니다:

```
1개월 차: 파이프라인 1개, 주 5-8시간 절약
2개월 차: 파이프라인 2개, 주 10-15시간 절약
3개월 차: 파이프라인 3개, 주 15-20시간 절약

유효 시급 $100에서, 월 $1,500-2,000의
해방된 시간 — 새 스트림에 투자하는 시간입니다.

1개월 차의 해방된 시간이 2개월 차의 파이프라인을 구축합니다.
2개월 차의 해방된 시간이 3개월 차의 파이프라인을 구축합니다.
자동화는 복리로 성장합니다.
```

이것이 한 명의 개발자가 5인 팀처럼 운영하는 방법입니다. 더 열심히 일하는 것이 아닙니다. 일하지 않는 동안 일하는 시스템을 구축하는 것입니다.

---

### 4DA 통합

{? if dna.identity_summary ?}
개발자 프로필 — {= dna.identity_summary | fallback("개발 포커스") =} — 을 기반으로, 아래 4DA 도구는 방금 배운 자동화 패턴에 직접 매핑됩니다. 신호 분류 도구는 해당 분야의 개발자에게 특히 관련이 있습니다.
{? endif ?}

4DA 자체가 레벨 3 자동화입니다. 수십 개의 소스에서 콘텐츠를 수집하고, PASIFA 알고리즘으로 각 항목에 점수를 매기며, 작업에 관련된 것만 표시합니다 — 손가락 하나 까딱하지 않고도. Hacker News, Reddit, 50개의 RSS 피드를 수동으로 확인하지 않아도 됩니다. 4DA가 그것을 하고 중요한 것을 보여줍니다.

같은 방식으로 수입 파이프라인을 구축하십시오.

4DA의 어텐션 보고서(MCP 도구의 `/attention_report`)는 시간이 실제로 어디에 가고 있는지 vs 어디에 가야 하는지를 보여줍니다. 무엇을 자동화할지 결정하기 전에 실행하십시오. "소비된 시간"과 "소비되어야 할 시간" 사이의 격차가 자동화 로드맵입니다.

신호 분류 도구(`/get_actionable_signals`)는 시장 모니터링 파이프라인에 직접 연결할 수 있습니다 — 커스텀 파이프라인이 니치 특화 분석을 하기 전에 4DA의 인텔리전스 레이어가 초기 점수를 매기도록 합니다.

기회를 위해 소스를 모니터링하는 파이프라인을 구축하고 있다면, 4DA가 이미 하고 있는 것을 다시 발명하지 마십시오. 그 MCP 서버를 자동화 스택의 빌딩 블록으로 사용하십시오.

---

### 다음 단계: 모듈 S — 스트림 쌓기

모듈 T는 각 수입원을 효율적으로 운영하는 도구를 제공했습니다. 모듈 S(스트림 쌓기)는 다음 질문에 답합니다: **몇 개의 스트림을 운영해야 하며, 어떻게 조합해야 합니까?**

모듈 S에서 다루는 내용:

- **수입원의 포트폴리오 이론** — 왜 3개 스트림이 1개를 이기고, 10개가 0개를 이기는지
- **스트림 상관관계** — 어떤 스트림이 서로 강화하고 어떤 것이 시간을 두고 경쟁하는지
- **인컴 플로어** — 실험하기 전에 비용을 커버하는 반복 수익 기반 구축
- **리밸런싱** — 언제 승자에 배팅을 늘리고 언제 부진한 것을 중단하는지
- **월 $10K 아키텍처** — 주 15-20시간으로 5자릿수에 도달하는 구체적인 스트림 조합

인프라(모듈 S), 해자(모듈 T), 엔진(모듈 R), 런칭 플레이북(모듈 E), 트렌드 레이더(모듈 E), 그리고 자동화(모듈 T)가 있습니다. 모듈 S는 이 모든 것을 지속 가능하고 성장하는 수입 포트폴리오로 엮습니다.

---

**파이프라인이 실행됩니다. 초안이 준비되어 있습니다. 10분 검토합니다.**

**이것이 전술적 자동화입니다. 이것이 스케일하는 방법입니다.**

*나의 장비. 나의 규칙. 나의 수익.*
