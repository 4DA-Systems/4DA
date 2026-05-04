# 模块 R：收入引擎

**STREETS 开发者收入课程 — 付费模块**
*第5-8周 | 8课 | 交付物：你的第一个收入引擎 + 引擎 #2 的计划*

> "构建能产生收入的系统，而不仅仅是能交付功能的代码。"

---

你已经有了基础设施（模块 S）。你已经有了竞争对手无法轻易复制的东西（模块 T）。现在是时候把这一切转化为收入了。

这是课程中最长的模块，因为它是最重要的。八个收入引擎。八种将你的技能、硬件和时间转化为收入的方式。每一个都是包含真实代码、真实定价、真实平台和真实计算的完整剧本。

{@ insight engine_ranking @}

你不会构建所有八个。你将选择两个。

**1+1 策略：**
- **引擎 1：** 赚到第一美元的最快路径。你将在第5-6周构建它。
- **引擎 2：** 最适合你具体情况的可扩展引擎。你将在第7-8周规划它，并在模块 E 中开始构建。

为什么是两个？因为单一收入来源是脆弱的。平台修改条款、客户消失、市场转移 — 你就归零了。通过不同渠道服务不同客户类型的两个引擎能给你弹性。而且你在引擎 1 中培养的技能几乎总是会加速引擎 2。

在这个模块结束时，你将拥有：

- 来自引擎 1 的收入（或能在几天内产生收入的基础设施）
- 引擎 2 的详细构建计划
- 清楚了解哪些引擎匹配你的技能、时间和风险承受能力
- 真正部署的代码 — 不仅是计划

{? if progress.completed("T") ?}
你在模块 T 中构建了护城河。现在这些护城河成为你收入引擎的基础 — 护城河越难复制，收入就越持久。
{? endif ?}

没有理论。没有"以后再说"。开始构建吧。

---

## 第1课：数字产品

*"合法印钞最接近的方式。"*

**到第一美元的时间：** 1-2周
**持续时间投入：** 每周2-4小时（支持、更新、营销）
**利润率：** 95%以上（创建后成本接近零）

### 为什么数字产品排在第一

{@ insight stack_fit @}

数字产品是开发者最高利润率、最低风险的收入引擎。你构建一次，永远销售。没有需要管理的客户。没有按小时计费。没有范围蔓延。没有会议。

数学很简单：
- 你花20-40小时构建一个模板或入门套件
- 定价 {= regional.currency_symbol | fallback("$") =}49
- 第一个月卖出10份：{= regional.currency_symbol | fallback("$") =}490
- 之后每月卖出5份：{= regional.currency_symbol | fallback("$") =}245/月被动收入
- 创建后的总成本：{= regional.currency_symbol | fallback("$") =}0

那 {= regional.currency_symbol | fallback("$") =}245/月听起来可能不够令人兴奋，但它不需要任何持续时间。叠加三个产品，你就能在睡觉时获得 {= regional.currency_symbol | fallback("$") =}735/月。叠加十个就能替代一个初级开发者的薪水。

### 什么能卖出去

{? if stack.primary ?}
不是你能构建的所有东西都能卖出去。作为 {= stack.primary | fallback("developer") =} 开发者，你有一个优势：你知道你的技术栈有什么问题。以下是开发者实际会付费的东西，以及当前市场上产品的真实价格：
{? else ?}
不是你能构建的所有东西都能卖出去。以下是开发者实际会付费的东西，以及当前市场上产品的真实价格：
{? endif ?}

**入门套件和样板**

| 产品 | 价格 | 为什么能卖 |
|---------|-------|-------------|
| 生产就绪的 Tauri 2.0 + React 入门套件（含认证、数据库、自动更新） | $49-79 | 节省40+小时的样板代码。Tauri 文档很好但不涵盖生产模式。 |
| 含 Stripe 计费、邮件、认证、管理面板的 Next.js SaaS 入门套件 | $79-149 | ShipFast ($199) 和 Supastarter ($299) 证明了这个市场的存在。专注且更便宜的替代品仍有空间。 |
| MCP 服务器模板包（5个常见模式模板） | $29-49 | MCP 是新事物。大多数开发者还没构建过。模板消除了白纸问题。 |
| Claude Code / Cursor 的 AI 代理配置包 | $29-39 | 子代理定义、CLAUDE.md 模板、工作流配置。新市场，几乎零竞争。 |
| 含自动发布、交叉编译、homebrew 的 Rust CLI 工具模板 | $29-49 | Rust CLI 生态系统增长迅速。正确发布出人意料地困难。 |

**组件库和 UI 套件**

| 产品 | 价格 | 为什么能卖 |
|---------|-------|-------------|
| 暗色模式仪表板组件套件（React + Tailwind） | $39-69 | 每个 SaaS 都需要仪表板。好的暗色模式设计很稀缺。 |
| 邮件模板包（React Email / MJML） | $29-49 | 事务性邮件设计很繁琐。开发者讨厌它。 |
| 针对开发者工具优化的着陆页模板包 | $29-49 | 开发者能写代码但不会设计。预设计的页面能提高转化率。 |

**文档和配置**

| 产品 | 价格 | 为什么能卖 |
|---------|-------|-------------|
| 常见技术栈的生产级 Docker Compose 文件 | $19-29 | Docker 无处不在但生产配置是部落知识。 |
| 20种常见设置的 Nginx/Caddy 反向代理配置 | $19-29 | 可复制粘贴的基础设施。节省数小时的 Stack Overflow 搜索。 |
| GitHub Actions 工作流包（10种常见技术栈的 CI/CD） | $19-29 | CI/CD 配置是写一次、搜索几小时的事。模板解决了这个问题。 |

> **说实话：** 卖得最好的产品解决的是具体的、即时的痛点。"节省40小时的设置"每次都胜过"学习一个新框架"。开发者购买的是他们现在正面临的问题的解决方案，不是他们将来可能面临的问题。

### 在哪里销售

**Gumroad** — 最简单的选择。30分钟设置产品页面，立即开始销售。每笔交易抽取10%。没有月费。
- 最适合：你的第一个产品。测试需求。$100以下的简单产品。
- 缺点：自定义有限。免费计划没有内置的联盟计划。

**Lemon Squeezy** — 记录商（Merchant of Record），意味着他们为你处理全球销售税、VAT 和 GST。每笔交易收取 5% + $0.50。
- 最适合：国际销售。$50以上的产品。订阅产品。
- 优点：你不需要注册 VAT。他们处理一切。
- 缺点：比 Gumroad 设置稍微复杂一些。
{? if regional.country ?}
- *在{= regional.country | fallback("your country") =}，像 Lemon Squeezy 这样的记录商处理跨境税务合规，对国际销售特别有价值。*
{? endif ?}

**你自己的网站** — 最大的控制权和利润。使用 Stripe Checkout 处理支付，在 Vercel/Netlify 上免费托管。
- 最适合：当你有流量时。$100以上的产品。建立品牌。
- 优点：0%平台费（仅 Stripe 的 2.9% + $0.30）。
- 缺点：你自己处理税务合规（或使用 Stripe Tax）。
{? if regional.payment_processors ?}
- *在{= regional.country | fallback("your region") =}可用的支付处理器：{= regional.payment_processors | fallback("Stripe, PayPal") =}。验证哪个支持你的{= regional.currency | fallback("local currency") =}。*
{? endif ?}

> **常见错误：** 在还没有一个产品可卖之前，花两周时间构建自定义店面。第一个产品用 Gumroad 或 Lemon Squeezy。在你验证了需求并有收入来证明投入之后，再迁移到自己的网站。

### 48小时从想法到上架

这是确切的步骤。设个计时器。你有48小时。

**第0-2小时：选择你的产品**

看看你在模块 S 中的主权技术栈文档。你的主要技能是什么？你每天使用什么框架？你最近做了什么设置工作花了太长时间？

最好的第一个产品是你已经为自己构建的东西。那个你花了三天的 Tauri 应用脚手架？那就是产品。你为团队配置的 CI/CD 管道？那就是产品。你花了一个周末才搞定的 Docker 设置？产品。

**第2-16小时：构建产品**

产品本身应该是干净的、文档完善的，解决一个特定问题。最低要求：

```
my-product/
  README.md           # 安装、使用、包含内容
  LICENSE             # 你的许可证（见下文）
  CHANGELOG.md        # 版本历史
  src/                # 实际产品
  docs/               # 需要时的额外文档
  examples/           # 可运行的示例
  .env.example        # 如适用
```

{? if settings.has_llm ?}
**文档是产品的一半。** 文档完善的模板每次都比没有文档的更好模板卖得好。使用你的本地 LLM ({= settings.llm_model | fallback("your configured model") =}) 帮助起草文档：
{? else ?}
**文档是产品的一半。** 文档完善的模板每次都比没有文档的更好模板卖得好。使用本地 LLM 帮助起草文档（如果还没设置，请从模块 S 设置 Ollama）：
{? endif ?}

```bash
# 从代码库生成初始文档
ollama run llama3.1:8b "Given this project structure and these key files,
write a comprehensive README.md that covers: installation, quick start,
project structure explanation, configuration options, and common
customizations. Be specific and include real commands.

Project structure:
$(find . -type f -not -path './.git/*' | head -50)

Key file (package.json):
$(cat package.json)

Key file (src/main.tsx):
$(cat src/main.tsx | head -80)"
```

然后编辑输出。LLM 提供了文档的70%。你的专业知识提供剩余的30% — 细微差别、注意事项、"这是我为什么选择这种方法"的上下文 — 使文档真正有用。

**第16-20小时：创建商品列表**

设置你的 Lemon Squeezy 商店。结账集成很简单 — 创建你的产品，设置交付的 Webhook，就可以上线了。带代码示例的完整支付平台设置教程，请参阅模块 E 第1课。

**第20-24小时：撰写销售页面**

你的销售页面需要恰好五个部分：

1. **标题：** 产品做什么以及为谁服务。"生产就绪 Tauri 2.0 入门套件 — 跳过40小时样板代码。"
2. **痛点：** 解决什么问题。"为新 Tauri 应用设置认证、数据库、自动更新和 CI/CD 需要好几天。这个入门套件一条 `git clone` 全部搞定。"
3. **包含内容：** 包中所有内容的要点列表。要具体。"14个预构建组件、Stripe 计费集成、带迁移的 SQLite、跨平台构建的 GitHub Actions。"
4. **社会证明：** 如果有的话。GitHub 星标、推荐评语，或"由[你]构建 — [X]年生产级[框架]应用构建经验。"
5. **行动号召：** 一个按钮。一个价格。"$49 — 立即获取访问权限。"

使用你的本地 LLM 起草文案，然后用你的口吻重写。

**第24-48小时：软启动**

在这些地方发布（选择与你产品相关的）：

- **Twitter/X：** 解释你构建了什么以及为什么的推文串。包含截图或 GIF。
- **Reddit：** 在相关子版块发帖（r/reactjs、r/rust、r/webdev 等）。不要推销味太重。展示产品，解释它解决的问题，附上链接。
- **Hacker News：** "Show HN: [产品名] — [一句话描述]。" 保持事实性。
- **Dev.to / Hashnode：** 写一篇使用你产品的教程。微妙的、有价值的推广。
- **相关的 Discord 服务器：** 在适当的频道分享。大多数框架 Discord 服务器有 #showcase 或 #projects 频道。

### 数字产品的许可证

你需要一个许可证。这是你的选项：

**个人许可证 ($49)：** 一个人，无限个人和商业项目。不可再分发或转售。

**团队许可证 ($149)：** 同一团队最多10位开发者。再分发的限制相同。

**扩展许可证 ($299)：** 可用于销售给最终用户的产品中（例如，使用你的模板构建一个销售给客户的 SaaS）。

在你的产品中包含一个 `LICENSE` 文件：

```
[Product Name] License Agreement
Copyright (c) [Year] [Your Name/Company]

Personal License — Single Developer

This license grants the purchaser the right to:
- Use this product in unlimited personal and commercial projects
- Modify the source code for their own use

This license prohibits:
- Redistribution of the source code (modified or unmodified)
- Sharing access with others who have not purchased a license
- Reselling the product or creating derivative products for sale

For team or extended licenses, visit [your-url].
```

### 收入计算

{@ insight cost_projection @}

让我们对一个 {= regional.currency_symbol | fallback("$") =}49 的产品做真实的计算：

```
平台费用 (Lemon Squeezy, 5% + $0.50):  -$2.95
支付处理（已包含）:                       $0.00
每笔销售你的收入:                         $46.05

达到 $500/月:  每月11笔（不到每天1笔）
达到 $1,000/月: 每月22笔（不到每天1笔）
达到 $2,000/月: 每月44笔（大约每天1.5笔）
```

对于一个在活跃细分市场中定位良好的产品，这些是现实的数字。

**真实世界基准：**
- **ShipFast** (Marc Lou)：一个 Next.js 样板，定价约 $199-249。前4个月产生了 $528K。Marc Lou 运营10个数字产品，合计月收入约 $83K。（来源：starterstory.com/marc-lou-shipfast）
- **Tailwind UI** (Adam Wathan)：一个 UI 组件库，前3天赚了 $500K，前2年突破了 $4M。然而，到2025年底收入同比下降约80%，因为 AI 生成的 UI 侵蚀了需求 — 提醒我们即使成功的产品也需要进化。（来源：adamwathan.me, aibase.com）

你不需要那些数字。你需要11笔销售。

### 你的回合

{? if stack.primary ?}
1. **确定你的产品**（30分钟）：查看你的主权技术栈文档。作为 {= stack.primary | fallback("your primary stack") =} 开发者，你为自己构建了什么花了20+小时的东西？那就是你的第一个产品。写下：产品名称、解决的问题、目标买家和价格。
{? else ?}
1. **确定你的产品**（30分钟）：查看你的主权技术栈文档。你为自己构建了什么花了20+小时的东西？那就是你的第一个产品。写下：产品名称、解决的问题、目标买家和价格。
{? endif ?}

2. **创建最小可行产品**（8-16小时）：打包你现有的工作。写 README。添加示例。做到整洁。

3. **设置 Lemon Squeezy 商店**（30分钟）：创建账户，添加产品，配置定价。使用他们内置的文件交付。

4. **撰写销售页面**（2小时）：五个部分。第一稿用本地 LLM。用你的口吻重写。

5. **软启动**（1小时）：在你产品受众相关的3个地方发布。

---

## 第2课：内容变现

*"你已经知道了成千上万人愿意付费学习的东西。"*

**到第一美元的时间：** 2-4周
**持续时间投入：** 每周5-10小时
**利润率：** 70-95%（取决于平台）

### 内容经济学

{@ insight stack_fit @}

内容变现与其他所有引擎的运作方式不同。开始很慢，然后复利增长。你的第一个月可能产生 $0。第六个月可能产生 $500。第十二个月可能产生 $3,000。而且它持续增长 — 因为内容的半衰期以年计算，而非天。

基本方程式：

```
内容收入 = 流量 x 转化率 x 每次转化收入

示例（技术博客）：
  每月50,000访客 x 2%联盟点击率 x 平均$5佣金
  = $5,000/月

示例（新闻通讯）：
  5,000订阅者 x 10%转化为付费版 x $5/月
  = $2,500/月

示例（YouTube）：
  10,000订阅者，月均约50K观看量
  = $500-1,000/月广告收入
  + $500-1,500/月赞助（达到10K订阅后）
  = $1,000-2,500/月
```

### 渠道1：带联盟收入的技术博客

**工作原理：** 写真正有用的技术文章。包含你实际使用和推荐的工具和服务的联盟链接。当读者点击并购买时，你赚取佣金。

**对开发者内容付费良好的联盟计划：**

| 计划 | 佣金 | Cookie 期限 | 为什么有效 |
|---------|-----------|----------------|-------------|
| Vercel | 每个推荐 $50-500 | 90天 | 阅读部署文章的开发者已准备好部署 |
| DigitalOcean | 每个新客户 $200（消费$25+） | 30天 | 教程直接驱动注册 |
| AWS / GCP | 不等，通常 $50-150 | 30天 | 基础设施文章吸引基础设施买家 |
| Stripe | 1年内持续 25% | 90天 | 任何 SaaS 教程都涉及支付 |
| Tailwind UI | 购买额的 10% ($30-80) | 30天 | 前端教程 = Tailwind UI 买家 |
| Lemon Squeezy | 1年内持续 25% | 30天 | 如果你写关于销售数字产品的内容 |
| JetBrains | 购买额的 15% | 30天 | 开发者教程中的 IDE 推荐 |
| Hetzner | 首次付款的 20% | 30天 | 低价托管推荐 |

**真实收入示例 — 月访客50K的开发者博客：**

```
月流量：50,000独立访客（12-18个月内可达到）

收入明细：
  托管联盟 (DigitalOcean, Hetzner):  $400-800/月
  工具联盟 (JetBrains, Tailwind UI):  $200-400/月
  服务联盟 (Vercel, Stripe):          $300-600/月
  展示广告 (Carbon Ads for developers): $200-400/月
  赞助文章 (每月1-2篇 @ $500-1,000):  $500-1,000/月

总计：$1,600-3,200/月
```

**开发者的 SEO 基础（真正有效的东西）：**

忘掉你从营销人员那里听到的关于 SEO 的一切。对于开发者内容，以下是重要的：

1. **回答具体问题。** "如何用 Tauri 2.0 设置 SQLite"每次都胜过"Tauri 简介"。具体的查询竞争更少，意图更高。

2. **瞄准长尾关键词。** 使用 Ahrefs（免费试用）、Ubersuggest（免费增值）或仅仅是 Google 自动完成。输入你的主题，看看 Google 建议什么。

3. **包含可运行的代码。** Google 对开发者查询优先展示包含代码块的内容。完整的可运行示例排名高于理论解释。

4. **每年更新。** 一篇实际最新的"如何在2026年部署 X"的文章排名高于一篇有10倍反向链接的2023年文章。在标题中加入年份并保持更新。

5. **内部链接。** 将你的文章互相链接。在 Tauri 设置文章底部加上"相关：如何为你的 Tauri 应用添加认证"。Google 会跟踪这些链接。

**使用 LLM 加速内容创作：**

4步流程：(1) 用本地 LLM 生成大纲，(2) 在本地起草每个部分（免费），(3) 添加你的专业知识 — LLM 无法提供的注意事项、观点和"这是我在生产中实际使用的" (4) 用 API 模型润色到面向客户的质量。

LLM 处理70%的工作。你的专业知识是让人们阅读、信任并点击你联盟链接的30%。

> **常见错误：** 未经大量编辑就发布 LLM 生成的内容。读者看得出来。Google 看得出来。而且它不会建立让联盟链接转化所需的信任。如果没有 LLM 你不会署名，那有了 LLM 你也不应该署名。

**校准期望值的真实新闻通讯基准：**
- **TLDR Newsletter** (Dan Ni)：120万+订阅者，年收入 $5-6.4M。每个赞助位收费高达 $18K。建立在策展而非原创报道之上。（来源：growthinreverse.com/tldr）
- **Pragmatic Engineer** (Gergely Orosz)：40万+订阅者，仅靠 $15/月的订阅就年收入 $1.5M+。零赞助 — 纯订阅者收入。（来源：growthinreverse.com/gergely）
- **Cyber Corsairs AI** (Beehiiv 案例研究)：不到1年增长到5万订阅者和月收入 $16K，证明新进入者仍然可以在专注的细分市场中突围。（来源：blog.beehiiv.com）

这些不是典型结果 — 它们是顶级表现者。但它们证明了这个模式在规模上有效，收入天花板是真实的。

### 渠道2：带付费层的新闻通讯

**平台对比：**

| 平台 | 免费层 | 付费功能 | 付费订阅的抽成 | 最适合 |
|----------|-----------|--------------|-------------------|----------|
| **Substack** | 无限订阅者 | 内置付费订阅 | 10% | 最大覆盖面，简单设置 |
| **Beehiiv** | 2,500订阅者 | 自定义域名、自动化、推荐计划 | 0%（你保留一切） | 增长导向，专业 |
| **Buttondown** | 100订阅者 | 自定义域名、API、原生 Markdown | 0% | 开发者、极简主义者 |
| **Ghost** | 自托管（免费） | 完整 CMS + 会员 | 0% | 完全控制、SEO、长期品牌 |
| **ConvertKit** | 10,000订阅者 | 自动化、序列 | 0% | 如果你也卖课程/产品 |

**推荐给开发者的：** Beehiiv（增长功能，不抽成）或 Ghost（完全控制，最好的 SEO）。

**LLM 驱动的新闻通讯流水线：**

```python
#!/usr/bin/env python3
"""newsletter_pipeline.py — Semi-automated newsletter production."""
import requests, json
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
NICHE = "Rust ecosystem and systems programming"  # ← 修改这里

def fetch_hn_stories(limit=30) -> list[dict]:
    """Fetch top HN stories. Replace/extend with RSS feeds, Reddit API, etc."""
    story_ids = requests.get("https://hacker-news.firebaseio.com/v0/topstories.json").json()[:limit]
    return [requests.get(f"https://hacker-news.firebaseio.com/v0/item/{sid}.json").json()
            for sid in story_ids]

def classify_and_summarize(items: list[dict]) -> list[dict]:
    """Use local LLM to score relevance and generate summaries."""
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
    """Generate newsletter skeleton — you edit and add your expertise."""
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
    print(f"Draft: {filename} — NOW add your expertise, fix errors, publish.")
```

**时间投入：** 流水线搭建后每周3-4小时。LLM 处理策展和起草。你处理编辑、洞察和订阅者付费的个人声音。

### 渠道3：YouTube

YouTube 变现最慢但天花板最高。YouTube 上的开发者内容长期供给不足 — 需求远超供给。

**收入时间线（现实的）：**

```
第1-3个月：    $0（积累视频库，尚未变现）
第4-6个月：    $50-200/月（1,000订阅者 + 4,000观看小时后广告收入启动）
第7-12个月：   $500-1,500/月（广告收入 + 首批赞助）
第2年：        $2,000-5,000/月（成熟频道，固定赞助商）
```

**2026年开发者 YouTube 上什么有效：**

1. **"用Y构建X"教程**（15-30分钟） — "用 Rust 构建 CLI 工具"、"构建本地 AI API"
2. **工具对比** — "2026年 Tauri vs Electron — 你该用哪个？"
3. **"我试了X 30天"** — "我用自托管替代品替换了所有云服务"
4. **架构深入解析** — "我如何设计一个处理每天100万事件的系统"
5. **"我学到了什么"回顾** — "销售数字产品6个月 — 真实数字"

**你需要的设备：**

```
最低配置（从这里开始）：
  屏幕录制：OBS Studio ($0)
  麦克风：任何USB麦克风 ($30-60) — 或你的耳机麦克风
  剪辑：DaVinci Resolve ($0) 或 CapCut ($0)
  总计：$0-60

舒适配置（收入足以支持时升级）：
  麦克风：Blue Yeti 或 Audio-Technica AT2020 ($100-130)
  摄像头：Logitech C920 ($70) — 如果你想要面部镜头
  总计：$170-200
```

> **说实话：** 对于开发者内容，音频质量比视频质量重要10倍。大多数观众是在听，不是在看。$30的USB麦克风 + OBS 就足够开始了。如果你的前10个视频内容好、音频还行，你会得到订阅者。如果它们内容差但有$2,000的摄像设备，你不会。

### 你的回合

1. **选择你的内容渠道**（15分钟）：博客、新闻通讯或 YouTube。选一个。不要同时尝试三个。技能不同，时间投入会快速叠加。

{? if stack.primary ?}
2. **定义你的细分市场**（30分钟）：不是"编程"。不是"Web开发"。是利用你的 {= stack.primary | fallback("primary stack") =} 专业知识的具体领域。"面向后端开发者的 Rust。""构建本地优先的桌面应用。""面向小型企业的 AI 自动化。"越具体，增长越快。
{? else ?}
2. **定义你的细分市场**（30分钟）：不是"编程"。不是"Web开发"。具体的。"面向后端开发者的 Rust。""构建本地优先的桌面应用。""面向小型企业的 AI 自动化。"越具体，增长越快。
{? endif ?}

3. **创建你的第一篇内容**（4-8小时）：一篇博客文章、一期新闻通讯或一个 YouTube 视频。发布出去。不要等待完美。

4. **设置变现基础设施**（1小时）：注册2-3个相关的联盟计划。设置你的新闻通讯平台。或者先发布，之后再添加变现 — 内容优先，收入其次。

5. **承诺一个时间表**（5分钟）：任何内容渠道最少每周一次。写下来："我每周[几]的[时间]发布。"你的受众通过一致性增长，而非质量。

---

## 第3课：微型 SaaS

*"一个小工具，为一群特定的人解决一个问题，他们会很乐意每月付$9-29。"*

**到第一美元的时间：** 4-8周
**持续时间投入：** 每周5-15小时
**利润率：** 80-90%（托管 + API 成本）

### 微型 SaaS 有什么不同

{@ insight stack_fit @}

微型 SaaS 不是创业公司。它不是在寻找风险投资。它不是试图成为下一个 Slack。微型 SaaS 是一个小而专注的工具：

- 解决恰好一个问题
- 收费 $9-29/月
- 一个人就能构建和维护
- 运行成本 $20-100/月
- 产生 $500-5,000/月的收入

美在约束中。一个问题。一个人。一个价格点。

**真实世界的微型 SaaS 基准：**
- **Pieter Levels** (Nomad List, PhotoAI 等)：零员工年收入约 $3M。仅 PhotoAI 就达到月收入 $132K。证明了单人创始人微型 SaaS 模式可以规模化。（来源：fast-saas.com）
- **Bannerbear** (Jon Yongfook)：一个图像生成 API，一个人引导到 MRR $50K+。（来源：indiepattern.com）
- **现实检查：** 70%的微型 SaaS 产品月收入低于 $1K。上面的幸存者是异常值。在构建之前验证，在有付费客户之前保持成本接近零。（来源：softwareseni.com）

### 寻找你的微型 SaaS 创意

{? if dna.top_engaged_topics ?}
看看你花最多时间参与的内容：{= dna.top_engaged_topics | fallback("your most-engaged topics") =}。最好的微型 SaaS 创意来自你在这些领域个人经历的问题。但如果你需要一个找到它们的框架，这里有一个：
{? else ?}
最好的微型 SaaS 创意来自你个人经历的问题。但如果你需要一个找到它们的框架，这里有一个：
{? endif ?}

**"电子表格替代"方法：**

寻找任何使用电子表格、手动流程或拼凑的免费工具来做本应是一个简单应用的工作流。那就是你的微型 SaaS。

示例：
- 自由职业者在 Google Sheets 中跟踪客户项目 → **自由职业者项目跟踪器** ($12/月)
- 开发者手动检查他们的副项目是否还在运行 → **独立开发者状态页** ($9/月)
- 内容创作者手动跨平台发布 → **跨平台发布自动化** ($15/月)
- 小团队在 Slack 消息中分享 API 密钥 → **团队密钥管理器** ($19/月)

**"糟糕的免费工具"方法：**

找一个人们勉强使用因为它免费、但讨厌因为它很差的免费工具。用 $9-29/月做一个更好的版本。

**"论坛挖掘"方法：**

在 Reddit、HN 和细分 Discord 服务器中搜索：
- "有没有一个工具可以..."
- "我希望有..."
- "我一直在找..."
- "有人知道一个好的..."

如果50+人在问，答案是"没有真正的"或"我用电子表格"，那就是一个微型 SaaS。

### 有收入潜力的真实微型 SaaS 创意

| 创意 | 目标用户 | 价格 | 100个客户时的收入 |
|------|------------|-------|-------------------------|
| GitHub PR 分析仪表板 | 工程经理 | $19/月 | $1,900/月 |
| 带漂亮状态页的正常运行时间监控 | 独立开发者、小型 SaaS | $9/月 | $900/月 |
| 从 git 提交生成变更日志 | 开发团队 | $12/月 | $1,200/月 |
| 开发者友好分析的 URL 缩短器 | 科技公司的营销人员 | $9/月 | $900/月 |
| 小团队 API 密钥管理器 | 创业公司 | $19/月 | $1,900/月 |
| Cron 任务监控和告警 | DevOps 工程师 | $15/月 | $1,500/月 |
| Webhook 测试和调试工具 | 后端开发者 | $12/月 | $1,200/月 |
| MCP 服务器目录和市场 | AI 开发者 | 广告支持 + 精选列表 $49/月 | 不等 |

### 构建微型 SaaS：完整教程

让我们构建一个真实的。我们将构建一个简单的正常运行时间监控服务 — 因为它简单、有用，展示了完整的技术栈。

**技术栈（为单人开发者优化）：**

```
后端：      Hono（轻量、快速、TypeScript）
数据库：    Turso（基于 SQLite，慷慨的免费层）
认证：      Lucia（简单、自托管认证）
支付：      Stripe（订阅）
托管：      Vercel（函数免费层）
着陆页：    同一 Vercel 项目上的静态 HTML
监控：      你自己的产品（吃自己的狗粮）
```

**启动时的月成本：**
```
Vercel:       $0（免费层 — 每月100K函数调用）
Turso:        $0（免费层 — 9GB存储，每月500M行读取）
Stripe:       每笔交易 2.9% + $0.30（只在你收到付款时）
域名：        $1/月（$12/年）
总计：        $1/月直到需要扩展
```

**核心 API 设置：**

```typescript
// src/index.ts — Hono API for uptime monitor
import { Hono } from "hono";
import { cors } from "hono/cors";
import { jwt } from "hono/jwt";
import Stripe from "stripe";

const app = new Hono();
const stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);
const PLAN_LIMITS = { free: 3, starter: 10, pro: 50 };

app.use("/api/*", cors());
app.use("/api/*", jwt({ secret: process.env.JWT_SECRET! }));

// Create a monitor (with plan-based limits)
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

// Get all monitors for user
app.get("/api/monitors", async (c) => {
  const userId = c.get("jwtPayload").sub;
  return c.json(await db.getMonitors(userId));
});

// Stripe webhook for subscription management
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

// The monitoring worker — runs on a cron schedule (Vercel cron, Railway cron, etc.)
export async function checkMonitors() {
  const monitors = await db.getActiveMonitors();

  const results = await Promise.allSettled(
    monitors.map(async (monitor) => {
      const start = Date.now();
      try {
        const response = await fetch(monitor.url, {
          method: "HEAD",
          signal: AbortSignal.timeout(10000),
        });
        return { monitorId: monitor.id, status: response.status,
                 responseTime: Date.now() - start };
      } catch {
        return { monitorId: monitor.id, status: 0, responseTime: Date.now() - start };
      }
    })
  );

  // Store results and alert on status changes (up → down or down → up)
  for (const result of results) {
    if (result.status === "fulfilled") {
      await db.insertCheckResult(result.value);
      const monitor = monitors.find((m) => m.id === result.value.monitorId);
      if (monitor) {
        const isDown = result.value.status === 0 || result.value.status >= 400;
        if (isDown && monitor.status !== "down") await sendAlert(monitor, "down");
        if (!isDown && monitor.status === "down") await sendAlert(monitor, "recovered");
        await db.updateMonitorStatus(monitor.id, isDown ? "down" : "up");
      }
    }
  }
}

export default app;
```

**Stripe 订阅设置（运行一次）：**

```typescript
// stripe-setup.ts — Create your product and pricing tiers
import Stripe from "stripe";
const stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);

async function createPricing() {
  const product = await stripe.products.create({
    name: "UptimeBot", description: "Simple uptime monitoring for developers",
  });

  const starter = await stripe.prices.create({
    product: product.id, unit_amount: 900, currency: "usd",
    recurring: { interval: "month" }, lookup_key: "starter",
  });
  const pro = await stripe.prices.create({
    product: product.id, unit_amount: 1900, currency: "usd",
    recurring: { interval: "month" }, lookup_key: "pro",
  });

  console.log(`Starter: ${starter.id} ($9/mo) | Pro: ${pro.id} ($19/mo)`);

  // Use in your checkout:
  // const session = await stripe.checkout.sessions.create({
  //   mode: 'subscription',
  //   line_items: [{ price: starter.id, quantity: 1 }],
  //   success_url: 'https://yourapp.com/dashboard?upgraded=true',
  //   cancel_url: 'https://yourapp.com/pricing',
  // });
}
createPricing().catch(console.error);
```

### 单位经济学

在构建任何微型 SaaS 之前，跑一下数字：

```
客户获取成本 (CAC)：
  如果做有机营销（博客、Twitter、HN）：约$0
  如果投广告：每个试用注册 $10-50，每个付费客户 $30-150

  目标：CAC < 3个月的订阅收入
  示例：CAC $30，价格 $12/月 → 2.5个月回本 ✓

客户终身价值 (LTV)：
  LTV = 月价格 x 平均客户生命周期（月）

  微型 SaaS 平均月流失率为 5-8%
  平均生命周期 = 1 / 流失率
  5%流失时：1/0.05 = 20个月 → $12/月的 LTV = $240
  8%流失时：1/0.08 = 12.5个月 → $12/月的 LTV = $150

  目标：LTV/CAC 比率 > 3

月消耗：
  托管 (Vercel/Railway)：$0-20
  数据库 (Turso/PlanetScale)：$0-20
  邮件发送 (Resend)：$0
  监控（你自己的产品）：$0
  域名：$1

  总计：$1-41/月

  盈亏平衡：1-5个客户（$9/月时）
```

> **常见错误：** 构建一个需要500个客户才能盈亏平衡的微型 SaaS。如果你的基础设施成本 $200/月而你收费 $9/月，你需要23个客户仅仅是覆盖成本。一切都从免费层开始。你第一个客户的付款应该是纯利润，而不是覆盖基础设施。

### 你的回合

1. **找到你的创意**（2小时）：使用"电子表格替代"或"论坛挖掘"方法。确定3个潜在的微型 SaaS 创意。对每个写下：问题、目标用户、价格以及达到 $1,000/月收入需要多少客户。

2. **在构建之前验证**（1-2天）：对你的首选创意，找到5-10个潜在客户并询问他们："我正在构建[X]。你会为此每月付$[Y]吗？"不要描述解决方案 — 描述问题，看他们是否眼前一亮。

3. **构建 MVP**（2-4周）：仅核心功能。认证、你的工具做的那一件事和 Stripe 计费。就这些。没有管理面板。没有团队功能。没有 API。一个用户、一个功能、一个价格。

{? if computed.os_family == "windows" ?}
4. **部署并启动**（1天）：部署到 Vercel 或 Railway。在 Windows 上，如果需要基于 Docker 的部署可以使用 WSL。购买域名。设置着陆页。在3-5个相关社区发帖。
{? elif computed.os_family == "macos" ?}
4. **部署并启动**（1天）：部署到 Vercel 或 Railway。macOS 通过 Docker Desktop 使 Docker 部署变得简单。购买域名。设置着陆页。在3-5个相关社区发帖。
{? else ?}
4. **部署并启动**（1天）：部署到 Vercel 或 Railway。购买域名。设置着陆页。在3-5个相关社区发帖。
{? endif ?}

5. **跟踪你的单位经济学**（持续的）：从第一天起跟踪 CAC、流失率和 MRR。如果在10个客户时数字不成立，100个客户时也不会。

---

## 第4课：自动化即服务

*"企业会付你数千美元来将他们的工具连接在一起。"*

**到第一美元的时间：** 1-2周
**持续时间投入：** 不等（基于项目）
**利润率：** 80-95%（你的时间是主要成本）

### 为什么自动化收费这么高

{@ insight stack_fit @}

大多数企业都有消耗员工每周10-40小时的手动工作流。前台手动将网站表单提交录入 CRM。会计从邮件中复制粘贴发票数据到 QuickBooks。营销经理手动向五个平台交叉发布内容。

这些企业知道自动化的存在。他们听说过 Zapier。但他们自己搞不定 — 而且 Zapier 的预建集成很少能完美处理他们的特定工作流。

这就是你登场的时候。你收费 $500-$5,000 来构建一个每周为他们节省10-40小时的自定义自动化。即使那个员工的时薪只有 $20，你也在为他们每月节省 $800-$3,200。你的一次性 $2,500 费用一个月就收回成本。

这是整个课程中最容易的销售之一。

### 隐私卖点

{? if settings.has_llm ?}
这里你在模块 S 中的本地 LLM 技术栈成为武器。你已经在本地运行着 {= settings.llm_model | fallback("a model") =} — 这是大多数自动化机构没有的基础设施。
{? else ?}
这里你在模块 S 中的本地 LLM 技术栈成为武器。（如果你还没设置本地 LLM，回到模块 S 第3课。这是高价自动化工作的基础。）
{? endif ?}

大多数自动化机构使用基于云的 AI。客户的数据经过 Zapier，然后到 OpenAI，然后返回。对于很多企业 — 特别是律师事务所、医疗机构、金融顾问以及任何欧盟公司 — 这是不可接受的。

{? if regional.country == "US" ?}
你的宣传语：**"我构建的自动化在私密环境中处理你的数据。你的客户记录、发票和通讯永远不会离开你的基础设施。没有第三方 AI 处理器。完全符合 HIPAA/SOC 2 合规要求。"**
{? else ?}
你的宣传语：**"我构建的自动化在私密环境中处理你的数据。你的客户记录、发票和通讯永远不会离开你的基础设施。没有第三方 AI 处理器。完全符合 GDPR 和当地数据保护法规。"**
{? endif ?}

这个宣传语能成交云自动化机构触及不到的交易。而且你可以收取溢价。

### 真实项目示例及定价

**项目1：房产中介的潜在客户筛选器 — $3,000**

```
问题：中介每周通过网站、邮件和社交媒体收到200+咨询。
     经纪人浪费时间回复不合格的潜客（闲逛的、区域外的、
     未预批准的）。

解决方案：
  1. Webhook 将所有咨询来源捕获到单一队列
  2. 本地 LLM 对每个潜客分类：热门 / 温暖 / 冷淡 / 垃圾
  3. 热门潜客：通过短信立即通知指定经纪人
  4. 温暖潜客：自动回复相关房源并安排后续跟进
  5. 冷淡潜客：加入培育邮件序列
  6. 垃圾：静默归档

工具：n8n（自托管）、Ollama、Twilio（短信）、他们现有的 CRM API

构建时间：15-20小时
你的成本：约$0（自托管工具 + 他们的基础设施）
他们的节省：经纪人每周约20小时 = 每月$2,000+
```

**项目2：律师事务所的发票处理器 — $2,500**

```
问题：事务所每月收到50-100份供应商发票作为PDF附件。
     法务助理手动将每份录入计费系统。
     每月耗时10+小时。容易出错。

解决方案：
  1. 邮件规则将发票转发到处理收件箱
  2. PDF 提取拉取文本（pdf-extract 或 OCR）
  3. 本地 LLM 提取：供应商、金额、日期、类别、计费代码
  4. 结构化数据发送到计费系统 API
  5. 异常（低置信度提取）进入审核队列
  6. 每周向管理合伙人发送汇总邮件

工具：自定义 Python 脚本、Ollama、邮件 API、计费系统 API

构建时间：12-15小时
你的成本：约$0
他们的节省：每月约10小时法务助理时间 + 更少的错误
```

**项目3：营销机构的内容再利用管道 — $1,500**

```
问题：机构每周为每个客户创建一篇长文博客文章。
     然后从每篇文章手动创建社交媒体片段、邮件摘要和
     LinkedIn 帖子。每篇文章耗时5小时。

解决方案：
  1. 新博客文章触发管道（RSS 或 Webhook）
  2. 本地 LLM 生成：
     - 5条 Twitter/X 帖子（不同角度、不同钩子）
     - 1条 LinkedIn 帖子（较长、专业语调）
     - 1份邮件新闻通讯摘要
     - 3个 Instagram 说明文字选项
  3. 所有生成内容进入审核仪表板
  4. 人工审核、编辑并通过 Buffer/Hootsuite 安排发布

工具：n8n、Ollama、Buffer API

构建时间：8-10小时
你的成本：约$0
他们的节省：每篇文章约4小时 x 每周4篇 = 每周16小时
```

### 构建自动化：n8n 示例

n8n 是一个可以自托管的开源工作流自动化工具（`docker run -d --name n8n -p 5678:5678 n8nio/n8n`）。它是专业的选择因为客户数据留在你/他们的基础设施上。

{? if stack.contains("python") ?}
对于更简单的部署，这是同样的发票处理作为纯 Python 脚本 — 正是你的专长：
{? else ?}
对于更简单的部署，这是同样的发票处理作为纯 Python 脚本（Python 是自动化工作的标准，即使它不是你的主要技术栈）：
{? endif ?}

```python
#!/usr/bin/env python3
"""
invoice_processor.py — Automated invoice data extraction.
Processes PDF invoices using local LLM, outputs structured data.
"""
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
    if not pdfs: return print("No invoices to process.")

    for pdf_path in pdfs:
        text = extract_text_from_pdf(pdf_path)
        if not text.strip():
            pdf_path.rename(REVIEW_DIR / pdf_path.name); continue

        invoice = extract_invoice_data(text, pdf_path.name)
        dest = REVIEW_DIR if invoice.needs_review else PROCESSED_DIR
        pdf_path.rename(dest / pdf_path.name)

        with open("./invoices/extracted.jsonl", "a") as f:
            f.write(json.dumps(asdict(invoice)) + "\n")
        print(f"  {'Review' if invoice.needs_review else 'OK'}: "
              f"{invoice.vendor} ${invoice.amount:.2f} ({invoice.confidence:.0%})")

if __name__ == "__main__":
    process_invoices()
```

### 寻找自动化客户

**LinkedIn（寻找自动化客户 ROI 最高的方式）：**

1. 修改你的标题为："我自动化繁琐的业务流程 | 隐私优先的 AI 自动化"
2. 每周发2-3篇关于自动化成果的帖子："为[客户类型]的[流程]实现自动化，每周节省15小时。数据不离开他们的基础设施。"
3. 加入目标行业的 LinkedIn 群组（房产经纪人、律所管理者、营销机构老板）
4. 每天向你所在地区的小企业主发送5-10个个性化连接请求

**本地商业网络：**

- 商会活动（参加一个，提到你"自动化业务流程"）
- BNI（Business Network International）团体
- 联合办公空间社区

**Upwork（用于你的前2-3个项目）：**

搜索："automation"、"data processing"、"workflow automation"、"Zapier expert"、"API integration"。每天申请5个项目，附上具体的、相关的提案。前2-3个项目会以较低的费率（$500-1,000）来积累评价。之后按市场价收费。

### 自动化合同模板

始终使用合同。你的合同至少需要这7个部分：

1. **工作范围** — 具体描述 + 交付物清单 + 文档
2. **时间线** — 预计完成天数，开始日期 = 收到定金时
3. **定价** — 总费用，50%预付（不可退还），50%交付时付
4. **数据处理** — "所有数据在本地处理。不使用第三方服务。开发者在完成后30天内删除所有客户数据。"
5. **修改** — 包含2轮，额外的按$150/小时收费
6. **维护** — 可选的漏洞修复和监控聘用费
7. **知识产权** — 客户拥有自动化。开发者保留重用通用模式的权利。

{? if regional.business_entity_type ?}
使用 Avodocs.com 或 Bonsai 的免费模板作为起点，然后添加数据处理条款（第4部分）— 这是大多数模板缺失的，也是你的竞争优势。在{= regional.country | fallback("your country") =}，合同头部使用你的{= regional.business_entity_type | fallback("business entity") =}。
{? else ?}
使用 Avodocs.com 或 Bonsai 的免费模板作为起点，然后添加数据处理条款（第4部分）— 这是大多数模板缺失的，也是你的竞争优势。
{? endif ?}

> **说实话：** 50%的预付定金没有商量余地。它保护你免受范围蔓延和交付后消失的客户。如果一个客户不愿预付50%，他们就是之后不会付100%的客户。

### 你的回合

1. **确定3个潜在的自动化项目**（1小时）：想想你打交道的企业（你的牙医、你房东的管理公司、你去的咖啡店、你的理发师）。他们做的什么手动流程是你可以自动化的？

2. **为其中一个定价**（30分钟）：计算：构建需要多少小时，对客户的价值是多少（节省的小时 x 那些小时的成本），以及合理的价格是多少？你的价格应该是你创造的节省的1-3个月。

3. **构建一个演示**（4-8小时）：拿上面的发票处理器为你的目标行业定制。录制一个2分钟的屏幕录像展示它的运行。这个演示是你的销售工具。

4. **联系5个潜在客户**（2小时）：LinkedIn、邮件或走进一家本地企业。给他们看演示。问他们的手动流程。

5. **设置你的合同模板**（30分钟）：用你的信息自定义上面的模板。准备好，这样客户说"好"的当天就能发送。

---

## 第5课：API 产品

*"把你的本地 LLM 变成一个创收端点。"*

**到第一美元的时间：** 2-4周
**持续时间投入：** 每周5-10小时（维护 + 营销）
**利润率：** 70-90%（取决于计算成本）

### API 产品模式

{@ insight stack_fit @}

API 产品将某种能力 — 通常是带自定义处理的本地 LLM — 包装在一个干净的 HTTP 端点后面，让其他开发者付费使用。你负责基础设施、模型和领域专业知识。他们得到一个简单的 API 调用。

对于熟悉后端工作的开发者来说，这是课程中最具可扩展性的引擎。一旦构建完成，每个新客户以最小的额外成本增加收入。

{? if profile.gpu.exists ?}
有了你的 {= profile.gpu.model | fallback("GPU") =}，你可以在开发期间和为第一批客户本地运行推理层，在需要扩展之前保持成本为零。
{? endif ?}

### 什么是好的 API 产品

不是每个 API 都值得付费。开发者会在以下情况下为 API 付费：

1. **它节省的时间比它的成本多。** 你的简历解析 API 月费 $29，为他们的团队每月节省20小时手动工作。容易卖。
2. **它做了他们不容易自己做的事情。** 微调的模型、专有数据集或复杂的处理管道。
3. **它比自建更可靠。** 有维护、有文档、有监控。他们不想照看一个 LLM 部署。

**带定价的真实 API 产品创意：**

| API 产品 | 目标客户 | 定价 | 为什么他们会付费 |
|------------|----------------|---------|---------------|
| 代码审查 API（对照自定义标准检查） | 开发团队 | $49/月/团队 | 无需高级开发者瓶颈的一致审查 |
| 简历解析器（从 PDF 简历中提取结构化数据） | HR 科技公司、ATS 构建者 | $29/月/500次解析 | 可靠地解析简历出人意料地困难 |
| 文档分类器（法律、金融、医疗） | 文档管理系统 | $99/月/1000份文档 | 领域特定的分类需要专业知识 |
| 内容审核 API（本地、私密） | 不能使用云 AI 的平台 | $79/月/10K次检查 | 隐私合规的审核很稀缺 |
| SEO 内容评分器（分析草稿 vs 竞争对手） | 内容机构、SEO 工具 | $39/月/100次分析 | 写作中的实时评分 |

### 构建 API 产品：完整示例

让我们构建一个文档分类 API — 法律科技创业公司会为此付 $99/月的那种。

**技术栈：**

```
运行时：        Hono (TypeScript) on Vercel Edge Functions
LLM：           Ollama（本地，开发用）+ Anthropic API（生产回退）
认证：          基于 API 密钥（简单、开发者友好）
速率限制：      Upstash Redis（免费层：每天10K请求）
计费：          Stripe 基于用量计费
文档：          OpenAPI 规范 + 托管文档
```

**完整的 API 实现：**

```typescript
// src/api.ts — Document Classification API
import { Hono } from "hono";
import { cors } from "hono/cors";
import { Ratelimit } from "@upstash/ratelimit";
import { Redis } from "@upstash/redis";

const app = new Hono();
const ratelimit = new Ratelimit({
  redis: new Redis({ url: process.env.UPSTASH_REDIS_URL!, token: process.env.UPSTASH_REDIS_TOKEN! }),
  limiter: Ratelimit.slidingWindow(100, "1 h"),
});

// Auth middleware: API key → user lookup → rate limit → track usage
async function authMiddleware(c: any, next: any) {
  const apiKey = c.req.header("X-API-Key") || c.req.header("Authorization")?.replace("Bearer ", "");
  if (!apiKey) return c.json({ error: "Missing API key." }, 401);

  const user = await db.getUserByApiKey(apiKey);
  if (!user) return c.json({ error: "Invalid API key." }, 401);

  const { success, remaining, reset } = await ratelimit.limit(user.id);
  c.header("X-RateLimit-Remaining", remaining.toString());
  if (!success) return c.json({ error: "Rate limit exceeded.", reset_at: new Date(reset).toISOString() }, 429);

  await db.incrementUsage(user.id);
  c.set("user", user);
  return next();
}

app.use("/v1/*", cors());
app.use("/v1/*", authMiddleware);

// Main classification endpoint
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
    // Try local Ollama first, fallback to Anthropic API
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
    await db.logApiCall(c.get("user").id, "classify", result.processing_time_ms);
    return c.json(result);
  } catch (error: any) {
    return c.json({ error: "Classification failed", message: error.message }, 500);
  }
});

app.get("/v1/usage", async (c) => {
  const user = c.get("user");
  const usage = await db.getMonthlyUsage(user.id);
  const plan = await db.getUserPlan(user.id);
  return c.json({ requests_used: usage.count, requests_limit: plan.requestLimit, plan: plan.name });
});

export default app;
```

**你的 API 的定价页面内容：**

```
免费层：        每月100请求，5K字符限制        $0
Starter：       每月2,000请求，50K字符限制     $29/月
Professional：  每月10,000请求，50K字符限制    $99/月
Enterprise：    自定义限制、SLA、专属支持       联系我们
```

### Stripe 基于用量的计费

```typescript
// billing.ts — Report usage to Stripe for metered billing

async function reportUsageToStripe(userId: string) {
  const user = await db.getUser(userId);
  if (!user.stripeSubscriptionItemId) return;

  const usage = await db.getUnreportedUsage(userId);

  if (usage.count > 0) {
    await stripe.subscriptionItems.createUsageRecord(
      user.stripeSubscriptionItemId,
      {
        quantity: usage.count,
        timestamp: Math.floor(Date.now() / 1000),
        action: "increment",
      }
    );

    await db.markUsageReported(userId, usage.ids);
  }
}

// Run this hourly via cron
// Vercel: vercel.json cron config
// Railway: railway cron
// Self-hosted: system cron
```

### 获得牵引力时的扩展

{? if profile.gpu.exists ?}
当你的 API 开始获得实际使用时，你的 {= profile.gpu.model | fallback("GPU") =} 给你一个先发优势 — 在支付云推理费用之前，你可以从自己的硬件为初始客户提供服务。扩展路径：
{? else ?}
当你的 API 开始获得实际使用时，扩展路径如下。没有专用 GPU，你会希望更早进入云推理（Replicate, Together.ai）：
{? endif ?}

```
阶段1：0-100客户
  - 本地 Ollama + Vercel edge functions
  - 月总成本：$0-20
  - 收入：$0-5,000/月

阶段2：100-500客户
  - 将 LLM 推理迁移到专用 VPS (Hetzner GPU, {= regional.currency_symbol | fallback("$") =}50-150/月)
  - 为重复查询添加 Redis 缓存
  - 月总成本：$50-200
  - 收入：$5,000-25,000/月

阶段3：500+客户
  - 负载均衡器后面的多个推理节点
  - 考虑托管推理 (Replicate, Together.ai) 处理溢出
  - 月总成本：$200-1,000
  - 收入：$25,000+/月
```

> **常见错误：** 在有10个客户之前就为规模过度工程。你的第一个版本应该在免费层上运行。扩展问题是好问题。它们到来时再解决，而不是提前。

### 你的回合

1. **确定你的 API 细分市场**（1小时）：你深入了解哪个领域？法律？金融？医疗？电子商务？最好的 API 产品来自深厚的领域知识加上 AI 能力。

2. **构建概念验证**（8-16小时）：一个端点、一个功能、没有认证（只在本地测试）。让分类/提取/分析在10个样本文档上正确工作。

3. **添加认证和计费**（4-8小时）：API 密钥管理、Stripe 集成、使用量跟踪。上面的代码给了你80%。

4. **编写 API 文档**（2-4小时）：使用 Stoplight 或手写 OpenAPI 规范。好的文档是 API 产品采用的第一因素。

5. **在开发者市场上启动**（1小时）：在 Product Hunt、Hacker News、相关子版块发帖。开发者对开发者的营销对 API 产品最有效。

---

## 第6课：咨询和兼职 CTO

*"启动最快的引擎，也是为其他一切提供资金的最佳方式。"*

**到第一美元的时间：** 1周（真的）
**持续时间投入：** 每周5-20小时（你控制调节旋钮）
**利润率：** 95%以上（你的时间是唯一的成本）

### 为什么咨询是大多数开发者的引擎 #1

{@ insight stack_fit @}

如果你需要这个月的收入而不是这个季度，答案就是咨询。不需要构建产品。不需要培养受众。不需要设置营销漏斗。只有你、你的专业知识和需要它的人。

计算：

```
$200/小时 x 每周5小时 = $4,000/月
$300/小时 x 每周5小时 = $6,000/月
$400/小时 x 每周5小时 = $8,000/月

这是在你的全职工作之外。
```

"但我不能收$200/小时。"是的你可以。稍后详述。

### 你实际在卖什么

{? if stack.primary ?}
你卖的不是"{= stack.primary | fallback("programming") =}"。你卖的是以下之一：
{? else ?}
你卖的不是"编程"。你卖的是以下之一：
{? endif ?}

1. **节省时间的专业知识。** "我将在10小时内正确设置你的 Kubernetes 集群，而不是你的团队花80小时摸索。"
2. **降低风险的知识。** "我将在你启动前审计你的架构，这样你不会在第一天有10,000用户时才发现扩展问题。"
3. **做出决策的判断力。** "我将评估你的三个供应商选项并推荐最符合你约束条件的那个。"
4. **解除团队阻塞的领导力。** "我将领导你的工程团队完成向[新技术]的迁移，同时不减慢功能开发。"

框架很重要。"我写 Python"值$50/小时。"我将在两周内将你的数据管道处理时间减少60%"值$300/小时。

**作为背景的真实费率数据：**
- **Rust 咨询：** 平均$78/小时，经验丰富的顾问标准工作最高可达$143/小时。架构和迁移咨询远高于此。（来源：ziprecruiter.com）
- **AI/ML 咨询：** 实施工作$120-250/小时。战略性 AI 咨询（架构、部署规划）在企业规模下$250-500/小时。（来源：debutinfotech.com）

### 2026年热门咨询细分市场

{? if stack.contains("rust") ?}
你的 Rust 专业知识使你处于最高需求、最高费率的咨询细分市场之一。Rust 迁移咨询因供给严重不足而获得溢价费率。
{? endif ?}

| 细分市场 | 费率范围 | 需求 | 为什么热门 |
|-------|-----------|--------|-------------|
| 本地 AI 部署 | $200-400/小时 | 非常高 | EU AI 法案 + 隐私担忧。很少有顾问具备此技能。 |
| 隐私优先架构 | $200-350/小时 | 高 | 法规驱动需求。"我们需要停止向 OpenAI 发送数据。" |
| Rust 迁移 | $250-400/小时 | 高 | 企业想要 Rust 的安全保证但缺乏 Rust 开发者。 |
| AI 编码工具设置 | $150-300/小时 | 高 | 工程团队想采用 Claude Code/Cursor 但需要关于代理、工作流、安全的指导。 |
| 数据库性能 | $200-350/小时 | 中高 | 永恒的需求。AI 工具帮你3倍速诊断。 |
| 安全审计（AI辅助） | $250-400/小时 | 中高 | AI 工具让你更加彻底。企业在融资轮前需要这个。 |

### 如何在本周内获得你的第一个咨询客户

**第1天：** 更新你的 LinkedIn 标题。差的："BigCorp 的高级软件工程师。"好的："我帮助工程团队在自有基础设施上部署 AI 模型 | Rust + 本地 AI"

**第2天：** 写3条 LinkedIn 帖子。(1) 分享一个带真实数字的技术洞察。(2) 分享一个你取得的具体成果。(3) 直接提供帮助："本月接受2个寻求[你的细分领域]的团队的咨询。免费30分钟评估请私信。"

**第3-5天：** 向 CTO 和工程经理发送10条个性化的外发消息。模板："我注意到[公司]正在[具体观察]。我帮助团队[价值主张]。最近帮助[类似公司]达成了[成果]。20分钟的通话是否有帮助？"

**第5-7天：** 申请咨询平台：**Toptal**（高端，$100-200+/小时，2-4周筛选），**Arc.dev**（远程为主，更快入职），**Lemon.io**（欧洲市场），**Clarity.fm**（按分钟收费的咨询）。

### 费率谈判

**如何设定你的费率：**

```
步骤1：找到你所在细分市场的市场费率
  - 查看 Toptal 公布的范围
  - 在开发者 Slack/Discord 社区中询问
  - 查看类似顾问的公开费率

步骤2：从范围的顶端开始
  - 如果市场是$150-300/小时，报$250-300
  - 如果他们还价，你落在市场价
  - 如果他们不还价，你就赚了高于市场的价格

步骤3：永远不要降低费率 — 而是增加范围
  差的："我可以$200而不是$300。"
  好的："$200/小时的话，我可以做X和Y。$300/小时的话，
       我还会做Z并提供持续支持。"
```

**价值锚定技巧：**

在报出你的费率之前，量化你将交付的价值：

```
"根据你所描述的情况，这次迁移将在下个季度为你的团队
节省大约200个工程小时。按照你团队的满载成本$150/小时，
这是$30,000的节省。我领导这个项目的费用是$8,000。"

（$30,000节省对应$8,000费用 = 客户获得3.75倍ROI）
```

### 为最大杠杆构建咨询

咨询的陷阱是用时间换金钱。突破出来：

1. **记录一切** — 每次参与都会产生迁移指南、架构文档、设置步骤。去掉客户特定的细节，你就有了一个产品（第1课）或博客文章（第2课）。
2. **模板化重复工作** — 3个客户同样的问题？那就是一个微型 SaaS（第3课）或数字产品（第1课）。
3. **演讲获客** — 一次30分钟的见面会演讲能产生2-3个客户对话。教一些有用的东西；人们会找上门来。
4. **写作然后收费** — 一篇关于特定技术挑战的博客文章会吸引恰好遇到这个问题需要帮助的人。

### 4DA 作为你的秘密武器

{@ mirror feed_predicts_engine @}

这是大多数顾问没有的竞争优势：**你比客户更早知道你的细分领域正在发生什么。**

4DA 浮现信号 — 新漏洞、趋势技术、破坏性变更、法规更新。当你向客户提到"顺便说一下，[他们使用的库]昨天公布了一个新漏洞，这是我对解决方案的建议"，你看起来就像有超自然感知力。

这种感知力证明了溢价费率的合理性。客户为主动知情的顾问付更多钱，而不是被动搜索的。

> **说实话：** 咨询是为其他引擎提供资金的最佳方式。用第1-3个月的咨询收入来资助你的微型 SaaS（第3课）或内容运营（第2课）。目标不是永远咨询 — 而是现在咨询，这样你就有跑道来构建不需要你时间就能产生收入的东西。

### 你的回合

1. **更新你的 LinkedIn**（30分钟）：新标题、新"关于"部分，以及一篇关于你专业知识的精选帖子。这是你的店面。

2. **撰写并发布一条 LinkedIn 帖子**（1小时）：分享一个技术洞察、一个成果或一个提供。不是推销 — 价值优先。

3. **发送5条直接外发消息**（1小时）：个性化的、具体的、价值导向的。使用上面的模板。

4. **申请一个咨询平台**（30分钟）：Toptal、Arc 或 Lemon.io。开始流程 — 需要时间。

5. **设定你的费率**（15分钟）：研究你细分市场的市场费率。写下你的费率。不要往下凑整。

---

## 第7课：开源 + 付费

*"公开构建，获取信任，变现金字塔顶端。"*

**到第一美元的时间：** 4-12周
**持续时间投入：** 每周10-20小时
**利润率：** 80-95%（取决于托管版本的基础设施成本）

### 开源商业模式

{@ insight stack_fit @}

开源不是慈善。它是一种分发策略。

逻辑是这样的：
1. 你构建一个工具并开源它
2. 开发者发现它、使用它、依赖它
3. 其中一些开发者在企业工作
4. 这些企业需要个人不需要的功能：SSO、团队管理、审计日志、优先支持、SLA、托管版本
5. 这些企业为付费版本买单

免费版本是你的营销。付费版本是你的收入。

### 许可证选择

你的许可证决定了你的护城河。谨慎选择。

| 许可证 | 含义 | 收入策略 | 示例 |
|---------|--------------|------------------|---------|
| **MIT** | 任何人都可以做任何事。分叉它、卖它、和你竞争。 | 付费功能/托管版本必须有足够的吸引力使 DIY 不值得。 | Express.js, React |
| **AGPLv3** | 任何通过网络使用它的人必须开源他们的修改。企业讨厌这个 — 他们会转而购买商业许可证。 | 双重许可：开源用 AGPL，不想要 AGPL 的企业用商业许可。 | MongoDB（最初）, Grafana |
| **FSL (Functional Source License)** | 2-3年内源代码可见但不是开源。之后转换为 Apache 2.0。在你关键的增长阶段防止直接竞争对手。 | 在你建立市场地位期间阻止直接竞争。付费功能获取额外收入。 | 4DA, Sentry |
| **BUSL (Business Source License)** | 类似 FSL。在指定期间限制竞争对手的生产使用。 | 与 FSL 相同。 | HashiCorp (Terraform, Vault) |

**推荐给个人开发者：** FSL 或 AGPL。

{? if regional.country == "US" ?}
- 如果你构建的东西企业会自托管：**AGPL**（他们会买商业许可证来避免 AGPL 义务）。美国企业特别反感在商业产品中使用 AGPL。
{? else ?}
- 如果你构建的东西企业会自托管：**AGPL**（他们会买商业许可证来避免 AGPL 义务）
{? endif ?}
- 如果你想要2年内完全控制的东西：**FSL**（在你建立市场地位时防止分叉竞争）

> **常见错误：** 因为"开源应该是免费的"而选择 MIT。MIT 很慷慨，这值得钦佩。但如果一家 VC 资助的公司分叉你的 MIT 项目，加上支付层，在营销上超过你，你就是在向他们的投资者捐赠你的工作。在保护你的工作足够长的时间来建立业务后，再开放它。

### 开源项目的营销

GitHub 星标是虚荣指标，但它们也是驱动采用的社会证明。获取方法：

**1. README 就是你的着陆页**

你的 README 应该有：
- **一句话描述** — 工具做什么以及为谁服务
- **截图或 GIF** — 展示工具运行效果（仅此一项就能使点击率翻倍）
- **快速开始** — `npm install x` 或 `cargo install x` 和第一个命令
- **功能列表** — 免费与付费的清晰标签
- **徽章墙** — 构建状态、版本、许可证、下载量
- **"为什么选择这个工具？"** — 3-5句说明它的不同之处

**2. Show HN 帖子（你的启动日）**

Hacker News 的 "Show HN" 帖子是开发者工具最有效的单一启动渠道。写一个清晰、事实性的标题："Show HN: [工具名] — [10个词以内描述它做什么]。"在评论中解释你的动机、技术决策和你希望获得反馈的方面。

**3. Reddit 启动策略**

在相关子版块发帖（Rust 工具发 r/rust，自托管工具发 r/selfhosted，Web 工具发 r/webdev）。写一篇真诚的帖子，讲述你解决的问题以及如何解决。链接到 GitHub。不要推销。

**4. "Awesome" 列表提交**

每个框架和语言在 GitHub 上都有一个 "awesome-X" 列表。被列入那里会带来持续的流量。找到相关列表，检查是否符合标准，提交一个 PR。

### 收入模式：开放核心

个人开发者最常见的开源收入模式：

```
免费（开源）：
  - 核心功能
  - CLI 界面
  - 本地存储
  - 社区支持（GitHub Issues）
  - 仅自托管

PRO ($12-29/月/用户)：
  - 免费版的一切
  - GUI / 仪表板
  - 云同步或托管版本
  - 优先支持（24小时响应）
  - 高级功能（分析、报告、集成）
  - 邮件支持

TEAM ($49-99/月/团队)：
  - Pro 的一切
  - SSO / SAML 认证
  - 基于角色的访问控制
  - 审计日志
  - 共享工作空间
  - 团队管理

ENTERPRISE（自定义定价）：
  - Team 的一切
  - 本地部署协助
  - SLA（99.9%正常运行时间保证）
  - 专属支持渠道
  - 自定义集成
  - 发票支付（net-30）
```

### 真实收入示例

**用于校准的真实开源业务：**
- **Plausible Analytics：** 隐私优先的网络分析。AGPL 许可，完全自力更生。12K 订阅者达到 ARR $3.1M。没有风险投资。证明了 AGPL 双重许可模式对个人/小团队产品有效。（来源：plausible.io/blog）
- **Ghost：** 开源发布平台。2024年收入 $10.4M，24K 客户。作为开放核心项目起步，通过社区优先策略增长。（来源：getlatka.com）

以下是带付费层的较小开源项目的典型增长曲线：

| 阶段 | 星标 | Pro 用户 | Team/Enterprise | MRR | 你的时间 |
|-------|-------|-----------|----------------|-----|-----------|
| 6个月 | 500 | 12 ($12/月) | 0 | $144 | 每周5小时 |
| 12个月 | 2,000 | 48 ($12/月) | 3个团队 ($49/月) | $723 | 每周8小时 |
| 18个月 | 5,000 | 150 ($19/月) | 20个团队 + 2个企业 | $5,430 | 每周15小时 |

模式：慢启动，复利增长。18个月时 MRR $5,430 的工具 = 年收入 $65K。大部分工作在第1-6个月。之后社区驱动增长。Plausible 的轨迹展示了复利超过18个月后会发生什么。

### 设置许可和功能门控

```typescript
// license.ts — Simple feature gating for open core
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
      // Find the minimum plan that includes this feature
      const requiredPlan = (Object.entries(PLAN_CONFIG) as [Plan, any][])
        .find(([_, config]) => config.features.has(feature))?.[0] || "enterprise";
      throw new Error(
        `"${feature}" requires ${requiredPlan} plan. ` +
        `You're on ${this.plan}. Upgrade at https://yourapp.com/pricing`
      );
    }
  }
}

// Usage: const license = new LicenseManager(user.plan);
//        license.requireFeature("cloud_sync"); // throws if not on correct plan
```

### 你的回合

1. **确定你的开源项目**（1小时）：你自己会用什么工具？你用脚本解决了什么问题应该做成一个正式工具？最好的开源项目始于个人实用工具。

2. **选择你的许可证**（15分钟）：FSL 或 AGPL 用于收入保护。MIT 仅在你为社区贡献且没有变现计划时使用。

3. **构建核心并发布**（1-4周）：开源核心。写 README。推到 GitHub。不要等待完美。

4. **定义你的定价层级**（1小时）：Free / Pro / Team。每个层级有什么功能？在构建付费功能之前写下来。

5. **启动**（1天）：Show HN 帖子、2-3个相关子版块和 "Awesome" 列表的 PR。

---

## 第8课：数据产品和情报

*"信息只有在被处理、过滤并在上下文中传递时才有价值。"*

**到第一美元的时间：** 4-8周
**持续时间投入：** 每周5-15小时
**利润率：** 85-95%

### 什么是数据产品

{@ insight stack_fit @}

数据产品获取原始信息 — 公共数据、研究论文、市场趋势、生态系统变化 — 并将其转化为对特定受众可操作的东西。你的本地 LLM 处理加工。你的专业知识处理策展。这个组合值得付费。

这与内容变现（第2课）不同。内容是"这是一篇关于 React 趋势的博客文章。"数据产品是"这是一份结构化的周报，包含评分信号、趋势分析以及针对 React 生态系统决策者的具体可操作建议。"

### 数据产品的类型

**1. 策展情报报告**

| 产品 | 受众 | 格式 | 价格 |
|---------|----------|--------|-------|
| "带实施笔记的每周 AI 论文摘要" | ML 工程师、AI 研究者 | 每周邮件 + 可搜索档案 | $15/月 |
| "Rust 生态系统情报报告" | Rust 开发者、评估 Rust 的 CTO | 月度 PDF + 每周提醒 | $29/月 |
| "开发者就业市场趋势" | 招聘经理、求职者 | 月度报告 | $49（单次） |
| "隐私工程通报" | 隐私工程师、合规团队 | 双周邮件 | $19/月 |
| "独立 SaaS 基准" | 自力更生的 SaaS 创始人 | 月度数据集 + 分析 | $29/月 |

**2. 加工数据集**

| 产品 | 受众 | 格式 | 价格 |
|---------|----------|--------|-------|
| 开源项目指标的策展数据库 | VC、OSS 投资者 | API 或 CSV 导出 | $99/月 |
| 按城市、角色和公司的技术薪资数据 | 职业教练、HR | 季度数据集 | 每个数据集 $49 |
| 100个热门服务的 API 正常运行时间基准 | DevOps、SRE 团队 | 仪表板 + API | $29/月 |

**3. 趋势提醒**

| 产品 | 受众 | 格式 | 价格 |
|---------|----------|--------|-------|
| 带修复指南的依赖漏洞突发新闻 | 开发团队 | 实时邮件/Slack 提醒 | $19/月/团队 |
| 带迁移指南的新框架发布 | 工程经理 | 即时提醒 | $9/月 |
| 影响 AI/隐私的法规变更 | 法务团队、CTO | 每周摘要 | $39/月 |

### 构建数据管道

{? if settings.has_llm ?}
这是一个用于制作每周情报报告的完整管道。这是真实的、可运行的代码 — 而且由于你已经设置了 {= settings.llm_model | fallback("a local model") =}，你可以以零边际成本运行这个管道。
{? else ?}
这是一个用于制作每周情报报告的完整管道。这是真实的、可运行的代码。你需要本地运行 Ollama（见模块 S）才能以零成本处理项目。
{? endif ?}

```python
#!/usr/bin/env python3
"""
intelligence_pipeline.py — Weekly intelligence report generator.
Fetches → Scores → Formats → Delivers. Customize NICHE and RSS_FEEDS for your domain.
"""
import requests, json, time, feedparser
from datetime import datetime, timedelta
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

# ── Stage 1: Fetch from RSS + HN ─────────────────────────────────

def fetch_items(feeds: list[dict], hn_min_score: int = 50) -> list[dict]:
    items = []
    cutoff = datetime.now() - timedelta(days=7)

    # RSS feeds
    for feed_cfg in feeds:
        try:
            for entry in feedparser.parse(feed_cfg["url"]).entries[:20]:
                items.append({"title": entry.get("title", ""), "url": entry.get("link", ""),
                    "source": feed_cfg["name"], "content": entry.get("summary", "")[:2000]})
        except Exception as e:
            print(f"  Warning: {feed_cfg['name']}: {e}")

    # Hacker News (Algolia API, time-filtered)
    week_ago = int(cutoff.timestamp())
    resp = requests.get(f"https://hn.algolia.com/api/v1/search?tags=story"
        f"&numericFilters=points>{hn_min_score},created_at_i>{week_ago}&hitsPerPage=30")
    for hit in resp.json().get("hits", []):
        items.append({"title": hit.get("title", ""), "source": "Hacker News",
            "url": hit.get("url", f"https://news.ycombinator.com/item?id={hit['objectID']}"),
            "content": hit.get("title", "")})

    # Deduplicate
    seen = set()
    return [i for i in items if i["title"][:50].lower() not in seen and not seen.add(i["title"][:50].lower())]

# ── Stage 2: Score with Local LLM ────────────────────────────────

def score_items(items: list[dict], niche: str, criteria: str) -> list[dict]:
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

# ── Stage 3: Generate Markdown Report ─────────────────────────────

def generate_report(items: list[dict], niche: str, issue: int) -> str:
    date_str = datetime.now().strftime('%B %d, %Y')
    report = f"# {niche} Intelligence — Issue #{issue}\n**Week of {date_str}**\n\n---\n\n"

    if items:
        top = items[0]
        report += f"## Top Signal: {top['title']}\n\n{top.get('summary','')}\n\n"
        report += f"**Why it matters:** {top.get('key_takeaway','')}\n\n"
        report += f"**Action:** {top.get('actionable_insight','')}\n\n[Read more]({top['url']})\n\n---\n\n"

    for item in items[1:12]:
        report += f"### [{item['title']}]({item['url']})\n"
        report += f"*{item['source']} | {item.get('category','')} | Score: {item.get('relevance_score',0)}/10*\n\n"
        report += f"{item.get('summary','')}\n\n> **Action:** {item.get('actionable_insight','')}\n\n"

    report += f"\n---\n*{len(items)} items analyzed. Generated locally on {date_str}.*\n"
    return report

# ── Run ───────────────────────────────────────────────────────────

if __name__ == "__main__":
    NICHE = "Rust Ecosystem"  # ← 修改这里
    CRITERIA = "High: new releases, critical crate updates, security vulns, RFC merges. " \
               "Medium: blog posts, new crates, job data. Low: peripheral mentions, rehashed tutorials."
    FEEDS = [
        {"name": "This Week in Rust", "url": "https://this-week-in-rust.org/rss.xml"},
        {"name": "Rust Blog", "url": "https://blog.rust-lang.org/feed.xml"},
        {"name": "r/rust", "url": "https://www.reddit.com/r/rust/.rss"},
    ]

    items = fetch_items(FEEDS)
    print(f"Fetched {len(items)} items")
    scored = score_items(items, NICHE, CRITERIA)
    print(f"Scored {len(scored)} above threshold")
    report = generate_report(scored, NICHE, issue=1)

    output = Path(f"./reports/report-{datetime.now().strftime('%Y-%m-%d')}.md")
    output.parent.mkdir(exist_ok=True)
    output.write_text(report)
    print(f"Report saved: {output}")
```

### 交付数据产品

**交付：** 使用 Resend（每月3,000封邮件免费）或 Buttondown。用 `marked` 将 Markdown 报告转换为 HTML，通过 Resend 的批量 API 发送。总交付代码：约15行。

**数据产品的定价策略：**

```
免费层：     月度摘要（预告）— 建立受众
个人：      $15-29/月 — 完整周报 + 档案访问
团队：      $49-99/月 — 多个席位 + 原始数据的 API 访问
企业：      $199-499/月 — 自定义信号、专属分析师时间
```

### 收入预测

```
第1个月：    10个订阅者 @ $15/月  = $150/月   （朋友、早期采用者）
第3个月：    50个订阅者 @ $15/月  = $750/月   （有机增长、HN/Reddit 帖子）
第6个月：    150个订阅者 @ $15/月 = $2,250/月  （SEO + 推荐开始生效）
第12个月：   400个订阅者 @ $15/月 = $6,000/月  （建立的品牌 + 团队计划）

运营成本：  约$10/月（邮件发送 + 域名）
你的时间：  每周5-8小时（大部分自动化，你添加专业知识）
```

{@ temporal revenue_benchmarks @}

**用于参考的真实内容创作者基准：**
- **Fireship** (Jeff Delaney)：YouTube 400万订阅者，仅广告年收入约 $550K+。面向开发者，短格式内容。（来源：networthspot.com）
- **Wes Bos：** 课程总销售额 $10M+，55K 付费学生。证明了技术教育可以远远超过新闻通讯收入的规模。（来源：foundershut.com）
- **Josh Comeau：** CSS 课程预售首周 $550K。证明了专注的、高质量的技术教育可以获得溢价。（来源：failory.com）

这些是顶级成果，但上面的管道方法是他们中许多人的起步方式：一致的、细分领域专注的、价值清晰的内容。

{? if profile.gpu.exists ?}
关键：管道做了繁重的工作。你的 {= profile.gpu.model | fallback("GPU") =} 在本地处理推理，保持每份报告的成本接近零。你的专业知识是护城河。没有其他人拥有你特定的领域知识 + 策展判断 + 处理基础设施的组合。
{? else ?}
关键：管道做了繁重的工作。即使仅用 CPU 推理，每周处理30-50篇文章对于批处理管道来说是可行的。你的专业知识是护城河。没有其他人拥有你特定的领域知识 + 策展判断 + 处理基础设施的组合。
{? endif ?}

### 你的回合

1. **选择你的细分市场**（30分钟）：你对什么领域了解到足以有自己的观点？那就是你的数据产品细分市场。

2. **确定5-10个数据源**（1小时）：RSS 源、API、子版块、HN 搜索、你目前阅读的新闻通讯。这些是你的原始输入。

3. **运行管道一次**（2小时）：为你的细分市场定制上面的代码。运行它。看看输出。有用吗？你会付费吗？

4. **制作你的第一份报告**（2-4小时）：编辑管道输出。添加你的分析、你的观点、你的"所以呢"。这是让它值得付费的20%。

5. **发给10个人**（30分钟）：不是作为产品 — 作为样本。"我正在考虑推出一份每周[细分市场]情报报告。这是第一期。这对你有用吗？你会每月付$15吗？"

---

## 引擎选择：选择你的两个

*"你现在知道了八个引擎。你需要两个。以下是如何选择。"*

### 决策矩阵

{@ insight engine_ranking @}

根据你的具体情况，在这四个维度上对每个引擎打1-5分：

| 维度 | 含义 | 如何打分 |
|-----------|--------------|-------------|
| **技能匹配** | 这个引擎与你已知的东西匹配度如何？ | 5 = 完美匹配，1 = 完全是新领域 |
| **时间适配** | 你能用可用时间执行这个引擎吗？ | 5 = 完美适合，1 = 需要辞职 |
| **速度** | 多快能看到第一美元？ | 5 = 本周，1 = 3个月以上 |
| **规模** | 不按比例增加时间的情况下能增长多少？ | 5 = 无限（产品），1 = 线性（用时间换钱） |

**填写这个矩阵：**

```
引擎                      技能  时间  速度  规模  总计
─────────────────────────────────────────────────────────
1. 数字产品                /5     /5     /5     /5     /20
2. 内容变现                /5     /5     /5     /5     /20
3. 微型 SaaS              /5     /5     /5     /5     /20
4. 自动化即服务            /5     /5     /5     /5     /20
5. API 产品               /5     /5     /5     /5     /20
6. 咨询                   /5     /5     /5     /5     /20
7. 开源 + 付费            /5     /5     /5     /5     /20
8. 数据产品               /5     /5     /5     /5     /20
```

### 1+1 策略

{? if dna.identity_summary ?}
根据你的开发者资料 — {= dna.identity_summary | fallback("your unique combination of skills and interests") =} — 考虑哪些引擎与你已经在做的事情最自然地契合。
{? endif ?}

{? if computed.experience_years < 3 ?}
> **以你的经验水平：** 从**数字产品**（引擎1）或**内容变现**（引擎2）开始 — 最低风险、最快反馈循环。你在了解市场需求的同时建立你的作品集。在你有更多已发布的工作可以展示之前，避免咨询和 API 产品。你现在的优势是精力和速度，不是深度。
{? elif computed.experience_years < 8 ?}
> **以你的经验水平：** 你3-8年的经验解锁了**咨询**和**API 产品** — 更高利润的引擎，奖励深度。客户为判断力付费，不仅是产出。考虑将咨询（快速现金）与微型 SaaS 或 API 产品（可扩展）配对。你的经验是护城河 — 你已经看过足够多的生产系统来知道什么真正有效。
{? else ?}
> **以你的经验水平：** 8年以上，专注于随时间复利的引擎：**开源 + 付费**、**数据产品**或**高费率咨询**（$250-500/小时）。你有信誉和人脉来要求溢价。你的优势是信任和声誉 — 利用它。考虑建立内容品牌（博客、新闻通讯、YouTube）作为你选择的引擎的放大器。
{? endif ?}

{? if stack.contains("react") ?}
> **React 开发者**在以下方面有强劲需求：UI 组件库、Next.js 模板和入门套件、设计系统工具、Tauri 桌面应用模板。React 生态系统足够大，细分产品能找到受众。考虑引擎 1（数字产品）和 3（微型 SaaS）作为你技术栈的自然选择。
{? endif ?}
{? if stack.contains("python") ?}
> **Python 开发者**在以下方面有强劲需求：数据管道工具、ML/AI 工具、自动化脚本和包、FastAPI 模板和 CLI 工具。Python 延伸到数据科学和 ML 创造了高端咨询机会。除了咨询外，考虑引擎 4（自动化即服务）和 5（API 产品）。
{? endif ?}
{? if stack.contains("rust") ?}
> **Rust 开发者**由于供给约束可以要求溢价费率。以下方面有强劲需求：CLI 工具、WebAssembly 模块、系统编程咨询和性能关键库。Rust 生态系统仍然足够年轻，构建良好的 crate 会吸引大量关注。考虑引擎 6（$250-400/小时的咨询）和 7（开源 + 付费）。
{? endif ?}
{? if stack.contains("typescript") ?}
> **TypeScript 开发者**有最广泛的市场覆盖：npm 包、VS Code 扩展、全栈 SaaS 产品和开发者工具。竞争比 Rust 或 Python-ML 更激烈，所以差异化更重要。专注于特定细分市场而不是通用工具。考虑在专注垂直领域中的引擎 1（数字产品）和 3（微型 SaaS）。
{? endif ?}

**引擎 1：你的 FAST 引擎** — 选择速度分数最高的引擎（决胜：最高总分）。这是你在第5-6周构建的。目标是14天内有收入。

**引擎 2：你的 SCALE 引擎** — 选择规模分数最高的引擎（决胜：最高总分）。这是你在第7-8周规划并通过模块 E 构建的。目标是6-12个月的复利增长。

**相性良好的常见组合：**

| Fast 引擎 | Scale 引擎 | 为什么相性好 |
|------------|-------------|-------------------|
| 咨询 | 微型 SaaS | 咨询收入资助 SaaS 开发。客户问题变成 SaaS 功能。 |
| 数字产品 | 内容变现 | 产品给你内容的信誉。内容驱动产品销售。 |
| 自动化即服务 | API 产品 | 客户的自动化项目揭示共同模式 → 打包为 API 产品。 |
| 咨询 | 开源 + 付费 | 咨询建立专业知识和声誉。开源将其捕获为产品。 |
| 数字产品 | 数据产品 | 模板建立你的细分市场专业知识。情报报告深化它。 |

### 收入预测工作表

{@ insight cost_projection @}

{? if regional.electricity_kwh ?}
在计算依赖本地推理的引擎的月成本时，记得考虑你当地的电费（{= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh）。
{? endif ?}

为你选择的两个引擎填写：

```
引擎 1 (Fast)：_______________________________

  到第一美元：_____ 周
  第1个月收入：      $________
  第3个月收入：      $________
  第6个月收入：      $________

  每月所需时间：      _____ 小时
  每月成本：          $________

  第一个里程碑：      $________ 在 __________ 前

引擎 2 (Scale)：_______________________________

  到第一美元：_____ 周
  第1个月收入：      $________
  第3个月收入：      $________
  第6个月收入：      $________
  第12个月收入：     $________

  每月所需时间：      _____ 小时
  每月成本：          $________

  第一个里程碑：      $________ 在 __________ 前

合计预测：

  第3个月总计：    $________/月
  第6个月总计：    $________/月
  第12个月总计：   $________/月

  每月总时间：      _____ 小时
  每月总成本：      $________
```

> **说实话：** 这些预测会是错的。没关系。重点不是准确 — 而是强迫你在开始构建之前想清楚数学。一个需要你每周30小时但只产生 $200/月的收入引擎是一笔糟糕的交易。你需要在纸上看到它之后再投入时间。

### 平台风险与多元化

每个收入引擎都坐在你无法控制的平台之上。Gumroad 可以改变费率结构。YouTube 可以取消你频道的变现。Vercel 可以停止联盟计划。Stripe 可以在审查期间冻结你的账户。这不是假设的 — 它经常发生。

**40%规则：** 永远不要让超过40%的收入依赖于单一平台。如果 Gumroad 产生了你60%的收入，他们一夜之间将费率从5%提高到15%（就像他们在2023年初宣布但后来撤回的那样），你的利润就崩溃了。如果 YouTube 占你收入的70%，算法变化使你的观看量减半，你就麻烦了。

**平台风险的真实案例：**

| 年份 | 平台 | 发生了什么 | 对开发者的影响 |
|------|----------|---------------|---------------------|
| 2022 | Heroku | 免费层取消 | 数千个业余项目和小企业被迫迁移或付费 |
| 2023 | Gumroad | 宣布统一10%费率（后来撤回） | 创作者争相评估替代方案；有 Lemon Squeezy 或 Stripe 备份的不受影响 |
| 2023 | Twitter/X API | 免费层取消，付费层重新定价 | 机器人开发者、内容自动化工具和数据产品一夜之间受到干扰 |
| 2024 | Unity | 宣布追溯性的按安装收费（后来修改） | 多年投资 Unity 的游戏开发者面临突然的成本增加 |
| 2025 | Reddit | API 定价变更 | 第三方应用开发者完全失去了他们的业务 |

**模式：** 平台为自身增长优化，不是为你。在平台生命周期早期，他们补贴创作者以吸引供给。当他们有了足够的供给，就开始提取价值。这不是恶意 — 是生意。你的工作是永远不要被它惊到。

**平台依赖度审计：**

每季度运行此审计。对每个收入流回答：

```
平台依赖度审计

收入流：_______________
依赖的平台：_______________

1. 这个收入流的多少百分比通过这个平台流动？
   [ ] <25%（低风险）  [ ] 25-40%（中等）  [ ] >40%（高 — 多元化）

2. 你能在30天内迁移到替代平台吗？
   [ ] 是的，替代方案存在且迁移简单
   [ ] 部分可以 — 有一些锁定（受众、声誉、集成）
   [ ] 不能 — 深度锁定（专有格式、无数据导出）

3. 这个平台是否有过不利变更的历史？
   [ ] 无有害变更历史  [ ] 轻微变更  [ ] 重大不利变更

4. 你是否拥有客户关系？
   [ ] 是的 — 我有邮箱地址，可以直接联系客户
   [ ] 部分 — 一些客户可以找到，一些不行
   [ ] 不是 — 平台控制所有客户访问

行动项目：
- 如果>40%依赖度：本月确定并测试替代方案
- 如果无法数据导出：立即导出你能导出的一切，设置月度提醒
- 如果你不拥有客户关系：立即开始收集邮箱
```

**按引擎的多元化策略：**

| 引擎 | 主要平台风险 | 缓解措施 |
|--------|----------------------|------------|
| 数字产品 | Gumroad/Lemon Squeezy 费率变更 | 保持你自己的 Stripe 结账作为备份。拥有你的客户邮件列表。 |
| 内容变现 | YouTube 取消变现、算法变化 | 建立邮件列表。跨平台发布。在你自己的域名上拥有你的博客。 |
| 微型 SaaS | 支付处理器保留、托管成本 | 多提供商支付设置。保持基础设施成本低于收入的10%。 |
| API 产品 | 云托管价格变更 | 为可移植性设计。使用容器。记录你的迁移手册。 |
| 咨询 | LinkedIn 算法、求职板变更 | 建立直接推荐网络。维护带作品集的个人网站。 |
| 开源 | GitHub 政策变更、npm 注册表规则 | 镜像发布。拥有你的项目网站和文档域名。 |

> **平台多元化的黄金法则：** 如果你不能直接给客户发邮件，你就没有客户 — 你拥有的是平台的客户。无论你运营哪个引擎，从第一天起就建立你的邮件列表。

### 反模式

{? if dna.blind_spots ?}
你确定的盲点 — {= dna.blind_spots | fallback("areas you haven't explored") =} — 可能会诱使你走向感觉"创新"的引擎。抵制这种诱惑。选择适合你当前优势的。
{? endif ?}

不要做这些：

1. **不要选3个以上的引擎。** 最多两个。三个会分散你的注意力，什么都做不好。

2. **不要选两个慢引擎。** 如果两个引擎都需要8周以上才能产生收入，你会在看到结果之前失去动力。至少一个引擎应该在2周内产生收入。

3. **不要选同一类别的两个引擎。** 微型 SaaS 和 API 产品都是"构建产品" — 你没有在多元化。将产品引擎与服务引擎或内容引擎配对。

4. **不要跳过计算。** "定价以后再说"是你最终得到运营成本超过收入的产品的方式。

5. **不要为最令人印象深刻的引擎优化。** 咨询不华丽。数字产品不"创新"。但它们赚钱。选择适合你情况的，而不是在 Twitter 上看起来好的。

6. **不要忽视平台集中度。** 运行上面的平台依赖度审计。如果任何单一平台控制了你收入的40%以上，多元化应该是你的下一个优先事项 — 在添加新引擎之前。

---

## 4DA 集成

{@ mirror feed_predicts_engine @}

> **4DA 如何连接到模块 R：**
>
> 4DA 的信号检测发现你的收入引擎填补的市场空白。没有入门套件的热门框架？构建一个（引擎1）。没有教程的新 LLM 技术？写一个（引擎2）。没有迁移指南的依赖漏洞？创建一个并收费（引擎1、2或8）。
>
> 4DA 的 `get_actionable_signals` 工具按紧迫性（战术 vs. 战略）和优先级分类内容。每种信号类型自然映射到收入引擎：
>
> | 信号分类 | 优先级 | 最佳收入引擎 | 示例 |
> |----------------------|----------|-------------------|---------|
> | 战术 / 高优先级 | 紧急 | 咨询、数字产品 | 新漏洞披露 — 写一份迁移指南或提供修复咨询 |
> | 战术 / 中优先级 | 本周 | 内容变现、数字产品 | 热门库发布 — 写第一个教程或构建入门套件 |
> | 战略 / 高优先级 | 本季度 | 微型 SaaS、API 产品 | 跨多个信号的新兴模式 — 在市场成熟前构建工具 |
> | 战略 / 中优先级 | 今年 | 开源 + 付费、数据产品 | 技术领域的叙事转变 — 通过开源工作或情报报告定位为专家 |
>
> 将 `get_actionable_signals` 与其他 4DA 工具结合以深入分析：
> - **`daily_briefing`** — AI 生成的执行摘要每天早上浮现最高优先级的信号
> - **`knowledge_gaps`** — 发现项目依赖中的空白，揭示填补这些空白的产品机会
> - **`trend_analysis`** — 统计模式和预测显示哪些技术正在加速
> - **`semantic_shifts`** — 检测技术从"实验性"到"生产级"采用的跨越时刻，发出市场时机信号
>
> 这个组合就是反馈循环：**4DA 检测机会。STREETS 给你执行的剧本。你的收入引擎将信号转化为收入。**

---

## 模块 R：完成

### 四周内你构建了什么

回顾这个模块开始时你在哪里。你有了基础设施（模块 S）和防御力（模块 T）。现在你有了：

1. **一个产生收入的运行中的引擎 1**（或能在几天内产生收入的基础设施）
2. **引擎 2 的详细计划** — 包含时间线、收入预测和第一步
3. **真实的、已部署的代码** — 不仅是想法，而是可运行的支付流程、API 端点、内容管道或产品列表
4. **一个在新机会出现时可以参考的决策矩阵**
5. **准确告诉你需要多少销售、客户或订阅者才能达到目标的收入计算**

### 关键交付物检查

在进入模块 E（执行手册）之前，验证：

- [ ] 引擎 1 已上线。有东西已部署、已列出或可供购买/雇用。
- [ ] 引擎 1 已产生至少 $1 的收入（或你有7天内达到 $1 的清晰路径）
- [ ] 引擎 2 已规划。你有一份包含里程碑和时间线的书面计划。
- [ ] 你的决策矩阵已填写。你知道为什么选择了这两个引擎。
- [ ] 你的收入预测工作表已完成。你知道第1、3、6和12个月的目标。

如果其中任何一项未完成，花时间完成。模块 E 建立在所有这些之上。没有运行中的引擎 1 就向前走，就像试图优化一个不存在的产品。

{? if progress.completed_modules ?}
### 你的 STREETS 进度

你已经完成了 {= progress.total_count | fallback("7") =} 个模块中的 {= progress.completed_count | fallback("0") =} 个（{= progress.completed_modules | fallback("none yet") =}）。模块 R 是转折点 — 在此之前的一切都是准备。在此之后的一切都是执行。
{? endif ?}

### 接下来：模块 E — 执行手册

模块 R 给了你引擎。模块 E 教你如何运营它们：

- **启动序列** — 每个引擎的前24小时、第一周和第一个月该做什么
- **定价心理学** — 为什么 $49 比 $39 卖得更好，以及何时打折（几乎永远不要）
- **找到你的前10个客户** — 每种引擎类型的具体、可操作的策略
- **重要的指标** — 在每个阶段跟踪什么、忽略什么
- **何时转型** — 告诉你引擎不工作的信号以及该怎么办

引擎已经构建好了。现在你学习如何驾驶它们。

---

*你的设备。你的规则。你的收入。*
