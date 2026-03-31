# 模块 S：主权配置

**STREETS 开发者收入课程 — 免费模块**
*第 1-2 周 | 6 节课 | 交付成果：你的主权技术栈文档*

> "你的设备就是你的商业基础设施。请以这样的标准来配置它。"

---

你已经拥有了大多数人一辈子都不会拥有的最强大的收入工具：一台开发者工作站，加上网络连接、本地算力，以及将这一切串联起来的技能。

大多数开发者把自己的设备当成消费品对待——用来打游戏、写代码、上网浏览。但那台机器——就是你桌子底下的那一台——可以运行推理、提供 API 服务、处理数据，并且在你睡觉的时候全天候为你创造收入。

这个模块要让你用全新的视角审视你已经拥有的一切。不是问"我能构建什么？"而是问"我能卖什么？"

在这两周结束时，你将拥有：

- 对你的收入创造能力的清晰盘点
- 一个生产级的本地 LLM 技术栈
- 一个法律和财务基础（即使是最简化的）
- 一份书面的主权技术栈文档，它将成为你的商业蓝图

没有空谈。没有"只要相信自己"。实实在在的数字、实实在在的命令、实实在在的决策。

{@ mirror sovereign_readiness @}

让我们开始吧。

---

## 第 1 课：设备审计

*"你不需要 4090。重要的是这些。"*

### 你的机器是一项商业资产

当一家公司评估其基础设施时，它不只是列出规格参数——它会将能力映射到收入机会。这正是你现在要做的事情。

{? if computed.profile_completeness != "0" ?}
> **你当前的设备：** {= profile.cpu.model | fallback("Unknown CPU") =}（{= profile.cpu.cores | fallback("?") =} 核 / {= profile.cpu.threads | fallback("?") =} 线程），{= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM，{= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}（{= profile.gpu.vram | fallback("?") =} VRAM）{? endif ?}，{= profile.storage.free | fallback("?") =} 可用 / {= profile.storage.total | fallback("?") =} 总计（{= profile.storage.type | fallback("unknown") =}），运行 {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}。
{? endif ?}

打开终端，依次运行以下命令。记下每一个数字。你在第 6 课编写主权技术栈文档时会用到它们。

### 硬件清单

#### CPU

```bash
# Linux/Mac
lscpu | grep "Model name\|CPU(s)\|Thread(s)"
# or
cat /proc/cpuinfo | grep "model name" | head -1
nproc

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# macOS
sysctl -n machdep.cpu.brand_string
sysctl -n hw.ncpu
```

**对收入的影响：**
- 核心数决定了你的设备能同时处理多少任务。在运行本地 LLM 的同时处理批量作业需要真正的并行能力。
{? if profile.cpu.cores ?}
- *你的 {= profile.cpu.model | fallback("CPU") =} 有 {= profile.cpu.cores | fallback("?") =} 个核心——查看下方的需求表，了解你的 CPU 支持哪些收入引擎。*
{? endif ?}
- 对于本课程中的大多数收入引擎来说，近 5 年内的任何 8 核以上现代 CPU 都足够了。
- 如果你只用 CPU 运行本地 LLM（没有 GPU），你需要 16 核以上。Ryzen 7 5800X 或 Intel i7-12700 是实用的最低门槛。

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**对收入的影响：**
- 16 GB：最低限度。你可以运行 7B 模型，进行基础的自动化工作。
- 32 GB：舒适区。可以本地运行 13B 模型，处理多个项目，同时保持开发环境和收入工作负载并行运行。
- 64 GB 以上：你可以在 CPU 上运行 30B+ 模型，或同时加载多个模型。这才是出售推理服务变得有趣的起点。
{? if profile.ram.total ?}
*你的系统有 {= profile.ram.total | fallback("?") =} RAM。查看上表了解你处于哪个能力层级——这直接影响哪些本地模型适合你的收入工作负载。*
{? endif ?}

#### GPU

```bash
# NVIDIA
nvidia-smi

# Check VRAM specifically
nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv

# AMD (Linux)
rocm-smi

# macOS (Apple Silicon)
system_profiler SPDisplaysDataType
```

**对收入的影响：**

这是人们最纠结的一项参数，以下是实话：**你的 GPU 决定了你的本地 LLM 层级，而你的本地 LLM 层级决定了哪些收入流运行得最快。** 但它并不决定你能否赚钱。

| VRAM | LLM 能力 | 收入相关性 |
|------|---------|----------|
| 0（仅 CPU） | 7B 模型，约 5 tokens/秒 | 批处理、异步工作。慢但可用。 |
| 6-8 GB（RTX 3060 等） | 7B 模型约 30 tok/秒，13B 量化版 | 对大多数自动化收入流来说足够了。 |
| 12 GB（RTX 3060 12GB、4070） | 13B 全速，30B 量化版 | 最佳平衡点。大多数收入引擎在这里运行良好。 |
| 16-24 GB（RTX 4090、3090） | 30B-70B 模型 | 高端层级。提供他人无法在本地匹配的质量。 |
| 48 GB 以上（双 GPU、A6000） | 70B+ 全速运行 | 企业级本地推理。严重的竞争优势。 |
| Apple Silicon 32GB+（M2/M3 Pro/Max） | 使用统一内存运行 30B+ | 出色的效率。功耗低于同等 NVIDIA 方案。 |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **你的 GPU：** {= profile.gpu.model | fallback("Unknown") =}，{= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}你处于高端层级。30B-70B 模型在本地触手可及。这是一项重要的竞争优势。{? elif computed.gpu_tier == "sweet_spot" ?}你处于最佳平衡点。13B 全速，30B 量化版。大多数收入引擎在这里运行良好。{? elif computed.gpu_tier == "capable" ?}你可以以不错的速度运行 7B 模型和 13B 量化版。对大多数自动化收入流来说足够了。{? else ?}你有 GPU 加速能力。查看上表了解你所在的层级。{? endif ?}
{? else ?}
> **未检测到独立 GPU。** 你将使用 CPU 运行推理，这意味着 7B 模型约 5-12 tokens/秒。对于批处理和异步工作来说没问题。使用 API 调用来弥补面向客户输出时的速度差距。
{? endif ?}

> **说句实话：** 如果你有一块 RTX 3060 12GB，你的条件已经优于 95% 试图用 AI 赚钱的开发者。别再等 4090 了。RTX 3060 12GB 就是本地 AI 界的本田思域——可靠、高效、能胜任工作。你升级显卡的钱不如花在 API 额度上，用于面向客户的高质量输出，而让本地模型处理繁重的基础工作。

#### 存储

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**对收入的影响：**
- LLM 模型需要空间：7B 模型约 4 GB，13B 约 8 GB，70B 约 40 GB（量化版）。
- 你需要空间存放项目数据、数据库、缓存和输出文件。
- 面向客户的任何工作，SSD 是硬性要求。从 HDD 加载模型会增加 30-60 秒的启动时间。
- 最低实用配置：500 GB SSD，至少 100 GB 可用空间。
- 舒适配置：1 TB SSD。把模型放在 SSD 上，归档文件放 HDD。
{? if profile.storage.free ?}
*你的 {= profile.storage.type | fallback("your drive") =} 有 {= profile.storage.free | fallback("?") =} 可用空间。{? if profile.storage.type == "SSD" ?}很好——SSD 意味着快速的模型加载。{? elif profile.storage.type == "NVMe" ?}非常好——NVMe 是模型加载最快的选择。{? else ?}如果你还没用 SSD 的话，考虑换一个——它对模型加载时间的影响很大。{? endif ?}*
{? endif ?}

#### 网络

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**对收入的影响：**
{? if profile.network.download ?}
*你的网络：{= profile.network.download | fallback("?") =} 下行 / {= profile.network.upload | fallback("?") =} 上行。*
{? endif ?}
- 下载速度：50+ Mbps。用于拉取模型、软件包和数据。
- 上传速度：这是大多数人忽略的瓶颈。如果你在提供任何服务（API、处理结果、交付物），上传速度很重要。
  - 10 Mbps：足够异步交付（处理后的文件、批量结果）。
  - 50+ Mbps：如果你运行任何外部服务会调用的本地 API 端点，则需要此速度。
  - 100+ Mbps：对本课程的一切都绰绰有余。
- 延迟：到主要云服务商低于 50ms。运行 `ping api.openai.com` 和 `ping api.anthropic.com` 检查。

#### 运行时间

这是没人会考虑的参数，但它区分了爱好者和睡觉也能赚钱的人。

问问你自己：
- 你的设备能 24/7 运行吗？（电力、散热、噪音）
- 你有 UPS 不间断电源应对停电吗？
- 你的网络连接是否足够稳定以支持自动化工作流？
- 出问题时你能 SSH 远程连接到你的机器吗？

如果你不能 24/7 运行，也没关系——本课程中很多收入流是你手动触发的异步批处理作业。但真正能产生被动收入的那些，需要持续运行。

{? if computed.os_family == "windows" ?}
**快速运行时间设置（Windows）：** 使用任务计划程序实现自动重启，启用远程桌面或安装 Tailscale 进行远程访问，并在 BIOS 中配置"断电后恢复供电"以从停电中恢复。
{? endif ?}

**快速运行时间设置（如果你需要的话）：**

```bash
# Enable Wake-on-LAN (check BIOS)
# Set up SSH access
sudo systemctl enable ssh  # Linux

# Auto-restart on crash (systemd service example)
# /etc/systemd/system/my-income-worker.service
[Unit]
Description=Income Worker Process
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/my-worker
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### 电费计算

人们要么忽视这件事，要么把它灾难化。让我们做真正的数学计算。

**测量你的实际功耗：**

```bash
# If you have a Kill-A-Watt meter or smart plug with monitoring:
# Measure at idle, at load (running inference), and at max (GPU full utilization)

# Rough estimates if you don't have a meter:
# Desktop (no GPU, idle): 60-100W
# Desktop (mid-range GPU, idle): 80-130W
# Desktop (high-end GPU, idle): 100-180W
# Desktop (GPU under inference load): add 50-80% of GPU TDP
# Laptop: 15-45W
# Mac Mini M2: 7-15W (seriously)
# Apple Silicon laptop: 10-30W
```

**月度费用计算：**

```
Monthly cost = (Watts / 1000) x Hours x Price per kWh

Example: Desktop with RTX 3060, running inference 8 hours/day, idle 16 hours/day
- Inference: (250W / 1000) x 8h x 30 days x $0.12/kWh = $7.20/month
- Idle: (100W / 1000) x 16h x 30 days x $0.12/kWh = $5.76/month
- Total: ~$13/month

Example: Same rig, 24/7 inference
- (250W / 1000) x 24h x 30 days x $0.12/kWh = $21.60/month

Example: Mac Mini M2, 24/7
- (12W / 1000) x 24h x 30 days x $0.12/kWh = $1.04/month
```

{? if regional.country ?}
你的电费：大约 {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh（基于 {= regional.country | fallback("your region") =} 平均值）。请查看你的实际电费账单——费率因供应商和时段而异。
{? else ?}
美国平均电费约 $0.12/kWh。请查看你的实际费率——差异很大。加州可能是 $0.25/kWh。一些欧洲国家高达 $0.35/kWh。美国中西部部分地区只要 $0.08/kWh。
{? endif ?}

**重点是：** 24/7 运行你的设备进行收入创造，电费大约在每月 {= regional.currency_symbol | fallback("$") =}1 到 {= regional.currency_symbol | fallback("$") =}30 之间。如果你的收入流连这都覆盖不了，问题不在电费——在于收入流本身。

### 按收入引擎类型划分的最低配置要求

以下是完整 STREETS 课程的预览方向。现在，先看看你的设备处于哪个水平：

| 收入引擎 | CPU | RAM | GPU | 存储 | 网络 |
|---------|-----|-----|-----|------|------|
| **内容自动化**（博客文章、邮件通讯） | 4 核以上 | 16 GB | 可选（API 备选） | 50 GB 可用 | 10 Mbps 上行 |
| **数据处理服务** | 8 核以上 | 32 GB | 可选 | 200 GB 可用 | 50 Mbps 上行 |
| **本地 AI API 服务** | 8 核以上 | 32 GB | 8+ GB VRAM | 100 GB 可用 | 50 Mbps 上行 |
| **代码生成工具** | 8 核以上 | 16 GB | 8+ GB VRAM 或 API | 50 GB 可用 | 10 Mbps 上行 |
| **文档处理** | 4 核以上 | 16 GB | 可选 | 100 GB 可用 | 10 Mbps 上行 |
| **自主智能体** | 8 核以上 | 32 GB | 12+ GB VRAM | 100 GB 可用 | 50 Mbps 上行 |

> **常见错误：** "我需要先升级硬件才能开始。"不。先用你手上有的东西开始。用 API 调用来弥补硬件覆盖不了的差距。等收入证明了升级的必要性再升级——而不是之前。

{@ insight engine_ranking @}

### 第 1 课检查点

你现在应该已经记录了：
- [ ] CPU 型号、核心数和线程数
- [ ] RAM 容量
- [ ] GPU 型号和 VRAM（或"无"）
- [ ] 可用存储空间
- [ ] 网络速度（下行/上行）
- [ ] 24/7 运行的估计月度电费
- [ ] 你的设备符合哪些收入引擎类别

保留这些数字。你将在第 6 课中把它们填入你的主权技术栈文档。

{? if computed.profile_completeness != "0" ?}
> **4DA 已经为你收集了大部分数据。** 查看上方的个性化摘要——你的硬件清单已通过系统检测部分预填。
{? endif ?}

*在完整的 STREETS 课程中，模块 R（收入引擎）为你提供上述每种引擎类型的具体、分步操作指南——包括构建和部署它们的确切代码。*

---

## 第 2 课：本地 LLM 技术栈

*"将 Ollama 配置为生产用途——不仅仅是聊天。"*

### 为什么本地 LLM 对收入很重要

每次你调用 OpenAI API，你都在付租金。每次你在本地运行模型，初始配置完成后推理就是免费的。数学很简单：

- GPT-4o：每百万输入 token 约 $5，每百万输出 token 约 $15
- Claude 3.5 Sonnet：每百万输入 token 约 $3，每百万输出 token 约 $15
- 本地 Llama 3.1 8B：每百万 token $0（只有电费）

如果你在构建处理数千请求的服务，每百万 token $0 和 $5-$15 的差距就是利润和持平之间的差距。

但这里有一个大多数人忽略的细微之处：**本地模型和 API 模型在收入技术栈中扮演不同角色。** 本地模型处理批量工作。API 模型处理对质量要求严格的、面向客户的输出。你的技术栈两者都需要。

### 安装 Ollama

{? if settings.has_llm ?}
> **你已经配置了 LLM：** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}。如果 Ollama 已经在运行，请跳到下方的"模型选择指南"。
{? endif ?}

Ollama 是基础。它把你的机器变成一台拥有干净 API 的本地推理服务器。

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
# Download from https://ollama.com or:
brew install ollama

# Windows
# Download installer from https://ollama.com
# Or use winget:
winget install Ollama.Ollama
```

{? if computed.os_family == "windows" ?}
> **Windows：** 使用 ollama.com 的安装程序或 `winget install Ollama.Ollama`。安装后 Ollama 会自动作为后台服务运行。
{? elif computed.os_family == "macos" ?}
> **macOS：** `brew install ollama` 是最快的方式。Ollama 利用 Apple Silicon 的统一内存——你的 {= profile.ram.total | fallback("system") =} RAM 在 CPU 和 GPU 工作负载之间共享。
{? elif computed.os_family == "linux" ?}
> **Linux：** 安装脚本会处理一切。如果你运行的是 {= profile.os.name | fallback("Linux") =}，Ollama 会作为 systemd 服务安装。
{? endif ?}

验证安装：

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **版本说明：** Ollama 更新频繁。本模块中的模型命令和参数已在 Ollama v0.5.x（2026 年初）上验证。如果你阅读本文的时间较晚，请访问 [ollama.com/download](https://ollama.com/download) 获取最新版本，以及 [ollama.com/library](https://ollama.com/library) 查看当前模型名称。核心概念不会改变，但具体的模型标签（例如 `llama3.1:8b`）可能会被更新的版本取代。

### 模型选择指南

不要见到什么模型就下载。要有策略。以下是该拉取什么以及何时使用每个模型。

{? if computed.llm_tier ?}
> **你的 LLM 层级（基于硬件）：** {= computed.llm_tier | fallback("unknown") =}。下方的建议已标注层级，以便你聚焦于与你设备匹配的层级。
{? endif ?}

#### 层级 1：主力模型（7B-8B 模型）

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**适用于：**
- 文本分类（"这封邮件是垃圾邮件还是正常邮件？"）
- 摘要生成（将长文档浓缩为要点）
- 简单数据提取（从文本中提取姓名、日期、金额）
- 情感分析
- 内容标记和分类
- 嵌入向量生成（如果使用支持嵌入的模型）

**典型性能：**
- RTX 3060 12GB：约 40-60 tokens/秒
- RTX 4090：约 100-130 tokens/秒
- M2 Pro 16GB：约 30-45 tokens/秒
- 仅 CPU（Ryzen 7 5800X）：约 8-12 tokens/秒

**成本对比：**
- 通过 GPT-4o-mini 处理 100 万 token：约 $0.60
- 本地处理 100 万 token（8B 模型）：电费约 $0.003
- 盈亏平衡点：约 5,000 token（从第一个请求开始你就在省钱）

#### 层级 2：均衡选择（13B-14B 模型）

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**适用于：**
- 内容起草（博客文章、文档、营销文案）
- 代码生成（函数、脚本、样板代码）
- 复杂数据转换
- 多步推理任务
- 有细微差别的翻译

**典型性能：**
- RTX 3060 12GB：约 20-30 tokens/秒（量化版）
- RTX 4090：约 60-80 tokens/秒
- M2 Pro 32GB：约 20-30 tokens/秒
- 仅 CPU：约 3-6 tokens/秒（不适合实时使用）

**何时选择它而非 7B：** 当 7B 的输出质量不够好，但你又不想为 API 调用付费时。在你的实际用例上同时测试两者——有时 7B 就足够了，而你只是在浪费算力。

{? if computed.gpu_tier == "capable" ?}
> **层级 3 边缘地带** — 你的 {= profile.gpu.model | fallback("GPU") =} 可以勉强运行 30B 量化版，但 70B 在本地不可行。考虑对需要 70B 级质量的任务使用 API 调用。
{? endif ?}

#### 层级 3：质量层级（30B-70B 模型）

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**适用于：**
- 需要出色质量的面向客户内容
- 复杂分析和推理
- 长篇内容生成
- 质量直接影响客户是否付费的任务

**典型性能：**
- RTX 4090（24GB）：70B 约 8-15 tokens/秒（可用但较慢）
- 双 GPU 或 48GB+：70B 约 20-30 tokens/秒
- M3 Max 64GB：70B 约 10-15 tokens/秒

> **说句实话：** 如果你没有 24GB 以上 VRAM，直接跳过 70B 模型。对质量关键的输出使用 API 调用。从系统内存以 3 tokens/秒运行 70B 模型理论上可行，但对任何创收工作流来说毫无实际意义。你的时间是有价值的。

#### 层级 4：API 模型（当本地不够用时）

本地模型用于处理批量和保护隐私。API 模型用于突破质量上限和利用专业能力。

**何时使用 API 模型：**
- 质量等于收入的面向客户输出（销售文案、高端内容）
- 小型模型难以处理的复杂推理链
- 视觉/多模态任务（分析图片、截图、文档）
- 当你需要高可靠性的结构化 JSON 输出时
- 当速度很重要而你的本地硬件较慢时

**成本对比表（截至 2025 年初——请核实当前定价）：**

| 模型 | 输入（每百万 token） | 输出（每百万 token） | 最适用于 |
|------|---------------------|---------------------|---------|
| GPT-4o-mini | $0.15 | $0.60 | 低成本批量工作（无本地可用时） |
| GPT-4o | $2.50 | $10.00 | 视觉、复杂推理 |
| Claude 3.5 Sonnet | $3.00 | $15.00 | 代码、分析、长上下文 |
| Claude 3.5 Haiku | $0.80 | $4.00 | 快速、便宜、质量均衡 |
| DeepSeek V3 | $0.27 | $1.10 | 预算友好，性能强劲 |

**混合策略：**
1. 本地 7B/13B 处理 80% 的请求（分类、提取、摘要）
2. API 处理 20% 的请求（质量关键的生成、复杂任务）
3. 你的有效成本：混合后约每百万 token $0.50-2.00（而非纯 API 的 $5-15）

这种混合方式就是你构建高利润率服务的方法。模块 R 中会详细讲解。

### 生产配置

为收入工作运行 Ollama 与个人聊天不同。以下是如何正确配置。

{? if computed.has_nvidia ?}
> **检测到 NVIDIA GPU（{= profile.gpu.model | fallback("unknown") =}）。** Ollama 将自动使用 CUDA 加速。确保你的 NVIDIA 驱动是最新的——运行 `nvidia-smi` 检查。为了在 {= profile.gpu.vram | fallback("your") =} VRAM 下获得最佳性能，下方的 `OLLAMA_MAX_LOADED_MODELS` 设置应与你的 VRAM 能同时容纳的模型数量匹配。
{? endif ?}

#### 设置环境变量

```bash
# Create/edit the Ollama configuration
# Linux: /etc/systemd/system/ollama.service or environment variables
# macOS: launchctl environment or ~/.zshrc
# Windows: System Environment Variables

# Key settings:
export OLLAMA_HOST=127.0.0.1:11434    # Bind to localhost only (security)
export OLLAMA_NUM_PARALLEL=4            # Concurrent request handling
export OLLAMA_MAX_LOADED_MODELS=2       # Keep 2 models in memory
export OLLAMA_KEEP_ALIVE=30m            # Keep model loaded for 30 min after last request
export OLLAMA_MAX_QUEUE=100             # Queue up to 100 requests
```

#### 为你的工作负载创建 Modelfile

不要使用默认模型设置，创建一个针对你的收入工作负载调优的自定义 Modelfile：

```dockerfile
# Save as: Modelfile-worker
FROM llama3.1:8b

# Tune for consistent, production output
PARAMETER temperature 0.3
PARAMETER top_p 0.9
PARAMETER num_ctx 4096
PARAMETER repeat_penalty 1.1

# System prompt for your most common workload
SYSTEM """You are a precise data processing assistant. You follow instructions exactly. You output only what is requested, with no preamble or explanation unless asked. When given structured output formats (JSON, CSV, etc.), you output only the structure with no markdown formatting."""
```

```bash
# Create your custom model
ollama create worker -f Modelfile-worker

# Test it
ollama run worker "Extract all email addresses from this text: Contact us at hello@example.com or support@test.org for more info."
```

#### 批处理和队列管理

对于收入工作负载，你经常需要处理大量条目。以下是基本的批处理设置：

```python
#!/usr/bin/env python3
"""
batch_processor.py — Process items through local LLM with queuing.
Production-grade batching for income workloads.
"""

import requests
import json
import time
import concurrent.futures
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "worker"  # Your custom model from above
MAX_CONCURRENT = 4
MAX_RETRIES = 3

def process_item(item: dict) -> dict:
    """Process a single item through the local LLM."""
    payload = {
        "model": MODEL,
        "prompt": item["prompt"],
        "stream": False,
        "options": {
            "num_ctx": 4096,
            "temperature": 0.3
        }
    }

    for attempt in range(MAX_RETRIES):
        try:
            response = requests.post(OLLAMA_URL, json=payload, timeout=120)
            response.raise_for_status()
            result = response.json()
            return {
                "id": item["id"],
                "input": item["prompt"][:100],
                "output": result["response"],
                "tokens": result.get("eval_count", 0),
                "duration_ms": result.get("total_duration", 0) / 1_000_000,
                "status": "success"
            }
        except Exception as e:
            if attempt == MAX_RETRIES - 1:
                return {
                    "id": item["id"],
                    "output": None,
                    "error": str(e),
                    "status": "failed"
                }
            time.sleep(2 ** attempt)  # Exponential backoff

def process_batch(items: list[dict], output_file: str = "results.jsonl"):
    """Process a batch of items with concurrent execution."""
    results = []
    start_time = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_CONCURRENT) as executor:
        future_to_item = {executor.submit(process_item, item): item for item in items}

        for i, future in enumerate(concurrent.futures.as_completed(future_to_item)):
            result = future.result()
            results.append(result)

            # Write incrementally (don't lose progress on crash)
            with open(output_file, "a") as f:
                f.write(json.dumps(result) + "\n")

            # Progress reporting
            elapsed = time.time() - start_time
            rate = (i + 1) / elapsed
            remaining = (len(items) - i - 1) / rate if rate > 0 else 0
            print(f"[{i+1}/{len(items)}] {result['status']} | "
                  f"{rate:.1f} items/sec | "
                  f"ETA: {remaining:.0f}s")

    # Summary
    succeeded = sum(1 for r in results if r["status"] == "success")
    failed = sum(1 for r in results if r["status"] == "failed")
    total_time = time.time() - start_time

    print(f"\nBatch complete: {succeeded} succeeded, {failed} failed, "
          f"{total_time:.1f}s total")

    return results

# Example usage:
if __name__ == "__main__":
    # Your items to process
    items = [
        {"id": i, "prompt": f"Summarize this in one sentence: {text}"}
        for i, text in enumerate(load_your_data())  # Replace with your data source
    ]

    results = process_batch(items)
```

### 为你的设备做基准测试

不要相信别人的基准测试数据。测量你自己的：

```bash
# Quick benchmark script
# Save as: benchmark.sh

#!/bin/bash
MODELS=("llama3.1:8b" "mistral:7b")
PROMPT="Write a detailed 200-word product description for a wireless mechanical keyboard designed for programmers."

for model in "${MODELS[@]}"; do
    echo "=== Benchmarking: $model ==="

    # Warm up (first run loads model into memory)
    ollama run "$model" "Hello" > /dev/null 2>&1

    # Timed run
    START=$(date +%s%N)
    RESULT=$(curl -s http://localhost:11434/api/generate -d "{
        \"model\": \"$model\",
        \"prompt\": \"$PROMPT\",
        \"stream\": false
    }")
    END=$(date +%s%N)

    DURATION=$(( (END - START) / 1000000 ))
    TOKENS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('eval_count', 'N/A'))")

    echo "Time: ${DURATION}ms"
    echo "Tokens generated: $TOKENS"
    if [ "$TOKENS" != "N/A" ] && [ "$DURATION" -gt 0 ]; then
        TPS=$(python3 -c "print(f'{$TOKENS / ($DURATION / 1000):.1f}')")
        echo "Speed: $TPS tokens/second"
    fi
    echo ""
done
```

```bash
chmod +x benchmark.sh
./benchmark.sh
```

记下每个模型的 tokens/秒。这个数字决定了哪些收入工作流对你的设备来说是可行的。

{@ insight stack_fit @}

**按用例划分的速度要求：**
- 批处理（异步）：5+ tokens/秒即可（你不在意延迟）
- 交互工具（用户等待）：至少 20+ tokens/秒
- 实时 API（面向客户）：30+ tokens/秒以获得良好用户体验
- 流式对话：15+ tokens/秒感觉响应流畅

### 保护你的本地推理服务器

{? if computed.os_family == "windows" ?}
> **Windows 提示：** Ollama 在 Windows 上默认绑定到 localhost。在 PowerShell 中使用 `netstat -an | findstr 11434` 验证。使用 Windows 防火墙阻止对端口 11434 的外部访问。
{? elif computed.os_family == "macos" ?}
> **macOS 提示：** Ollama 在 macOS 上默认绑定到 localhost。使用 `lsof -i :11434` 验证。macOS 防火墙应自动阻止外部连接。
{? endif ?}

你的 Ollama 实例绝不应该从互联网访问，除非你明确打算这样做。

```bash
# Verify Ollama is only listening on localhost
ss -tlnp | grep 11434
# Should show 127.0.0.1:11434, NOT 0.0.0.0:11434

# If you need remote access (e.g., from another machine on your LAN):
# Use SSH tunneling instead of exposing the port
ssh -L 11434:localhost:11434 your-rig-ip

# Firewall rules (Linux)
sudo ufw deny in 11434
sudo ufw allow from 192.168.1.0/24 to any port 11434  # LAN only, if needed
```

> **常见错误：** 为了"方便"将 Ollama 绑定到 0.0.0.0 然后就忘了这事。任何找到你 IP 的人都能免费使用你的 GPU 进行推理。更糟的是，他们可以提取模型权重和系统提示词。始终使用 localhost。始终通过隧道访问。

### 第 2 课检查点

你现在应该已经：
- [ ] 安装并运行了 Ollama
- [ ] 至少拉取了一个主力模型（llama3.1:8b 或同等模型）
- [ ] 为你预期的工作负载创建了自定义 Modelfile
- [ ] 基准测试数据：每个模型在你设备上的 tokens/秒
- [ ] Ollama 仅绑定到 localhost

*在完整的 STREETS 课程中，模块 T（技术护城河）向你展示如何构建专有模型配置、微调流水线和自定义工具链，让竞争对手无法轻易复制。模块 R（收入引擎）为你提供在这套技术栈之上构建的精确服务。*

---

## 第 3 课：隐私优势

*"你的私有化配置本身就是竞争优势——不仅仅是个人偏好。"*

### 隐私是产品特性，而非限制

大多数开发者搭建本地基础设施，是因为他们个人重视隐私，或者因为他们喜欢折腾。那没问题。但如果你没有意识到**隐私是当前科技领域最具市场价值的特性之一**，那你就在浪费赚钱的机会。

原因如下：每当一家公司向 OpenAI 的 API 发送数据时，数据都会经过第三方。对于许多企业——尤其是医疗、金融、法律、政府以及欧盟企业——这是一个真实的问题。不是理论上的问题。是"我们不能用这个工具，因为合规部门说不行"的问题。

你在本地机器上运行模型，就没有这个问题。

### 监管顺风

监管环境正在朝着对你有利的方向快速发展。

{? if regional.country == "US" ?}
> **美国用户：** 对你最重要的法规是 HIPAA、SOC 2、ITAR 以及州级隐私法（加州 CCPA 等）。欧盟法规仍然重要——它们影响你服务欧洲客户的能力，那是一个利润丰厚的市场。
{? elif regional.country == "GB" ?}
> **英国用户：** 脱欧后，英国有自己的数据保护框架（UK GDPR + Data Protection Act 2018）。你的本地处理优势对服务英国金融服务和 NHS 相关工作尤为突出。
{? elif regional.country == "DE" ?}
> **德国用户：** 你身处世界上最严格的数据保护环境之一。这是一个*优势*——德国客户已经理解为什么本地处理很重要，而且他们愿意为此付费。
{? elif regional.country == "AU" ?}
> **澳大利亚用户：** 1988 年《隐私法》和澳大利亚隐私原则（APPs）管辖你的义务。对于受《我的健康记录法》约束的政府和医疗客户，本地处理是一个强有力的卖点。
{? endif ?}

**欧盟 AI 法案（2024-2026 年执行）：**
- 高风险 AI 系统需要有文档记录的数据处理流水线
- 企业必须展示数据流向以及谁在处理数据
- 本地处理极大地简化了合规流程
- 欧盟企业正在积极寻找能保证欧盟数据驻留的 AI 服务提供商

**GDPR（已执行）：**
- "数据处理"包括将文本发送给 LLM API
- 企业需要与每个第三方签订数据处理协议
- 本地处理彻底消除了第三方
- 这是一个真正的卖点："你的数据永远不会离开你的基础设施。不需要与第三方协商数据处理协议。"

**行业特定法规：**
- **HIPAA（美国医疗）：** 患者数据在没有 BAA（商业伙伴协议）的情况下不能发送到消费级 AI API。大多数 AI 服务商不为 API 访问提供 BAA。本地处理完全绕过了这个问题。
- **SOC 2（企业级）：** 接受 SOC 2 审计的企业需要记录每一个数据处理者。处理者越少=审计越容易。
- **ITAR（美国国防）：** 受控技术数据不能离开美国管辖范围。拥有国际基础设施的云 AI 服务商存在问题。
- **PCI DSS（金融）：** 持卡人数据处理对数据传输路径有严格要求。

### 如何在销售对话中定位隐私

你不需要成为合规专家。你需要理解三句话，并知道何时使用它们：

**第一句话："你的数据永远不会离开你的基础设施。"**
使用场景：与任何注重隐私的潜在客户交谈时。这是通用的切入点。

**第二句话："不需要第三方数据处理协议。"**
使用场景：与欧洲公司或任何有法务/合规团队的公司交谈时。这为他们节省数周的法律审查。

**第三句话："完整审计轨迹，单租户处理。"**
使用场景：与企业级或受监管行业客户交谈时。他们需要向审计人员证明其 AI 流水线。

**示例定位（用于你的服务页面或提案）：**

> "与基于云的 AI 服务不同，[你的服务] 在专用硬件上本地处理所有数据。你的文档、代码和数据永远不会离开处理环境。流水线中没有第三方 API，不需要协商数据共享协议，每项操作都有完整的审计日志。这使得 [你的服务] 适合有严格数据处理要求的组织，包括 GDPR、HIPAA 和 SOC 2 合规环境。"

这段话放在着陆页上，将精准吸引那些愿意支付高价的客户。

### 溢价定价的合理性

以下是用硬数字说明的商业论证：

**标准 AI 处理服务（使用云 API）：**
- 客户的数据发送到 OpenAI/Anthropic/Google
- 你与每一个能调用 API 的开发者竞争
- 市场价格：每处理一份文档 $0.01-0.05
- 你本质上是在加价转卖 API 访问

**隐私优先 AI 处理服务（你的本地技术栈）：**
- 客户的数据留在你的机器上
- 你与一个小得多的服务商群体竞争
- 市场价格：每处理一份文档 $0.10-0.50（5-10 倍溢价）
- 你卖的是基础设施+专业能力+合规保障

隐私溢价是真实的：相同底层任务，比大宗云服务高出 **5 倍到 10 倍**。而且愿意支付这个价格的客户更忠诚、对价格不那么敏感，预算也更大。

{@ insight competitive_position @}

### 设置隔离的工作空间

如果你有正职工作（你们大多数人都有），你需要将雇主工作和收入工作做清晰的隔离。这不仅仅是法律保护——这是运营卫生。

{? if computed.os_family == "windows" ?}
> **Windows 提示：** 为收入工作创建一个单独的 Windows 用户账户（设置 > 账户 > 家庭和其他用户 > 添加其他人）。这给你一个完全隔离的环境——独立的浏览器配置文件、独立的文件路径、独立的环境变量。使用 Win+L 在账户之间切换。
{? endif ?}

**选项 1：独立用户账户（推荐）**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**选项 2：容器化工作空间**

```bash
# Docker-based isolation
# Create a dedicated workspace container

# docker-compose.yml
version: '3.8'
services:
  income-workspace:
    image: ubuntu:22.04
    volumes:
      - ./income-projects:/workspace
      - ./income-data:/data
    environment:
      - OLLAMA_HOST=host.docker.internal:11434
    network_mode: bridge
    # Your employer's VPN, tools, etc. are NOT in this container
```

**选项 3：独立物理机器（最稳妥）**

如果你认真对待这件事，且收入足以证明其合理性，一台专用机器可以消除所有疑虑。一台装了 RTX 3060 的二手 Dell OptiPlex 花费 $400-600，第一个月的客户工作就能回本。

**最低隔离清单：**
- [ ] 收入项目放在单独的目录（永远不要与雇主的仓库混在一起）
- [ ] 收入工作使用单独的 API 密钥（永远不要使用雇主提供的密钥）
- [ ] 收入相关账户使用单独的浏览器配置文件
- [ ] 收入工作绝不在雇主的硬件上进行
- [ ] 收入工作绝不在雇主的网络上进行（使用你的个人网络或 VPN）
- [ ] 收入项目使用单独的 GitHub/GitLab 账户（可选但更干净）

> **常见错误：** 使用雇主的 OpenAI API 密钥"只是测试"你的副业项目。这会在雇主的计费面板上留下痕迹，并且搅浑知识产权的水。申请你自己的密钥。它们很便宜。

### 第 3 课检查点

你现在应该理解了：
- [ ] 为什么隐私是一个有市场价值的产品特性，而不仅仅是个人偏好
- [ ] 哪些法规创造了对本地 AI 处理的需求
- [ ] 在关于隐私的销售对话中使用的三句话
- [ ] 隐私优先的服务如何获得 5-10 倍的溢价定价
- [ ] 如何将收入工作与雇主工作分离

*在完整的 STREETS 课程中，模块 E（进化前沿）教你如何跟踪监管变化，并在竞争对手还不知道新合规要求存在之前就提前布局。*

---

## 第 4 课：法律最低要求

*"现在花十五分钟做法律准备，能避免以后数月的麻烦。"*

### 这不是法律建议

我是开发者，不是律师。以下是大多数开发者在大多数情况下应该处理的实用清单。如果你的情况比较复杂（你在雇主公司持有股权、竞业禁止条款有特定条款等），花 $200 找一位雇佣法律师做 30 分钟咨询。这是你能获得的最高投资回报率。

### 第 1 步：阅读你的劳动合同

找到你的劳动合同或录用通知书。搜索以下条款：

**知识产权转让条款** — 寻找类似以下的措辞：
- "所有发明、开发和工作成果……"
- "……在雇佣期间创造的……"
- "……与公司的业务或预期业务相关的……"

**限制你的关键措辞：**
- "雇佣期间创造的所有工作成果归公司所有"（范围很广——可能有问题）
- "使用公司资源创造的工作成果"（范围较窄——如果你用自己的设备通常没问题）
- "与公司当前或预期业务相关的"（取决于你的雇主做什么业务）

**释放你的关键措辞：**
- "不包括完全在员工自己的时间内、使用员工自己的资源、且与公司业务无关的工作"（这是你的例外条款——美国很多州要求包含此条款）
- 一些州（加利福尼亚、华盛顿、明尼苏达、伊利诺伊等）有法律限制雇主对个人项目的知识产权主张，无论合同怎么写。

### 三个问题测试

对于任何收入项目，请自问：

1. **时间：** 你是在自己的时间做这项工作吗？（不在工作时间，不在值班时间）
2. **设备：** 你用的是自己的硬件、自己的网络、自己的 API 密钥吗？（不是雇主的笔记本电脑、不是雇主的 VPN、不是雇主的云账户）
3. **主题：** 这与你雇主的业务无关吗？（如果你在一家医疗 AI 公司工作，然后想卖医疗 AI 服务……那是个问题。如果你在一家医疗 AI 公司工作，想卖面向房地产中介的文档处理……那没问题。）

如果三个答案都是清楚的，你几乎可以确定没问题。如果任何答案是模糊的，在继续之前先搞清楚。

> **说句实话：** 绝大多数做副业的开发者从来没有遇到过问题。雇主关心的是保护竞争优势，而不是阻止你在不相关的项目上赚外快。但"几乎可以确定没问题"不等于"完全没问题"。如果你的合同条款异常宽泛，和你的经理或人力资源部门聊聊——或者咨询律师。不去确认的风险远比开口问的小尴尬要严重得多。

### 第 2 步：选择商业结构

你需要一个法律实体来将个人资产与商业活动分离，并为商业银行、支付处理和税收优惠打开大门。

{? if regional.country ?}
> **你的所在地：{= regional.country | fallback("Unknown") =}。** 你所在地区推荐的实体类型是 **{= regional.business_entity_type | fallback("LLC or equivalent") =}**，典型注册费用为 {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}。向下滚动到你所在国家的部分，或阅读所有部分以了解其他地区的客户如何运作。
{? endif ?}

{? if regional.country == "US" ?}
#### 美国（你所在的地区）
{? else ?}
#### 美国
{? endif ?}

| 结构 | 费用 | 保护 | 最适用于 |
|------|------|------|---------|
| **个人独资**（默认） | $0 | 无（个人承担责任） | 试水阶段。第一笔 $1K。 |
| **单人 LLC** | $50-500（因州而异） | 个人资产保护 | 积极的收入工作。大多数开发者应从这里开始。 |
| **S-Corp 选举**（基于 LLC） | LLC 费用 + 选举费 $0 | 与 LLC 相同 + 工资税优惠 | 当你年收入稳定超过 $40K 时 |

**推荐美国开发者：** 在你居住州注册单人 LLC。

**注册费最低的州：** Wyoming（$100，无州所得税）、New Mexico（$50）、Montana（$70）。但在你居住的州注册通常是最简单的，除非你有特定理由不这样做。

**如何申请：**
1. 访问你所在州的州务卿网站
2. 搜索"form LLC"或"business entity filing"
3. 提交组织章程（10 分钟的表格）
4. 从 IRS 获取 EIN（免费，在 irs.gov 上 5 分钟完成）

{? if regional.country == "GB" ?}
#### 英国（你所在的地区）
{? else ?}
#### 英国
{? endif ?}

| 结构 | 费用 | 保护 | 最适用于 |
|------|------|------|---------|
| **个体经营者** | 免费（在 HMRC 注册） | 无 | 初始收入。试水阶段。 |
| **有限公司（Ltd）** | 通过 Companies House 约 $15 | 个人资产保护 | 任何正式的收入工作。 |

**推荐：** 通过 Companies House 注册 Ltd 公司。大约需要 20 分钟，费用 GBP 12。

#### 欧盟

因国家不同差异很大，但总体模式是：

- **德国：** 起步用 Einzelunternehmer（个人独资），正式工作用 GmbH（但 GmbH 需要 EUR 25,000 注册资本——可以考虑 UG，只需 EUR 1）
- **荷兰：** Eenmanszaak（个人独资，免费注册）或 BV（相当于 Ltd）
- **法国：** Micro-entrepreneur（简化流程，推荐起步使用）
- **爱沙尼亚：** e-Residency + OUE（非居民的热门选择，完全在线操作）

{? if regional.country == "AU" ?}
#### 澳大利亚（你所在的地区）
{? else ?}
#### 澳大利亚
{? endif ?}

| 结构 | 费用 | 保护 | 最适用于 |
|------|------|------|---------|
| **个体经营者** | 免费 ABN | 无 | 起步阶段 |
| **Pty Ltd** | 通过 ASIC 约 AUD 500-800 | 个人资产保护 | 正式收入 |

**推荐：** 从个体经营者 ABN 开始（免费、即时），等你有稳定收入后再转为 Pty Ltd。

### 第 3 步：支付处理（15 分钟设置）

你需要一种收款方式。现在就设置好，不要等到第一个客户等着付款的时候才做。

{? if regional.payment_processors ?}
> **推荐用于 {= regional.country | fallback("your region") =}：** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe（推荐大多数开发者使用）：**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

时间：约 15 分钟。你可以立即开始接受付款（Stripe 对新账户的资金保留 7 天）。

**Lemon Squeezy（推荐数字产品使用）：**

如果你在销售数字产品（模板、工具、课程、SaaS），Lemon Squeezy 充当你的注册商户。这意味着：
- 他们为你在全球范围内处理销售税、增值税和 GST
- 你不需要在欧盟注册增值税
- 他们处理退款和争议

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas（适合国际开发者或想要美国实体的人）：**

如果你在美国以外，但想通过美国实体向美国客户销售：
- 一次性费用 $500
- 为你在特拉华州创建 LLC
- 开设美国银行账户（通过 Mercury 或 Stripe）
- 提供注册代理服务
- 大约需要 1-2 周

### 第 4 步：隐私政策和服务条款

如果你在线销售任何服务或产品，你需要这些文件。不要花钱请律师写样板文件。

**免费、可靠的模板来源：**
- **Termly.io** — 免费隐私政策和服务条款生成器。回答问题，获取文档。
- **Avodocs.com** — 面向初创企业的开源法律文档。免费。
- **GitHub 的 choosealicense.com** — 专门用于开源项目许可证。
- **Basecamp 的开源政策** — 搜索"Basecamp open source policies"——很好的、通俗易懂的模板。

**你的隐私政策必须涵盖的内容（如果你处理任何客户数据）：**
- 你收集了什么数据
- 你如何处理数据（本地处理——这是你的优势）
- 你保留数据多长时间
- 客户如何请求删除数据
- 是否有任何第三方访问数据（理想情况：无）

**时间：** 使用模板生成器 30 分钟。搞定。

### 第 5 步：独立银行账户

不要把商业收入通过个人支票账户运作。原因如下：

1. **税务清晰度：** 到报税时，你需要准确知道哪些是商业收入，哪些不是。
2. **法律保护：** 如果你有 LLC，将个人和商业资金混在一起可能会"刺穿公司面纱"——这意味着法院可以无视你 LLC 的责任保护。
3. **专业性：** 从"张三咨询有限公司"发出的发票打入专用商业账户看起来正规。付款到你的个人支付软件账户看起来就不那么正规了。

**免费或低成本的商业银行：**
{? if regional.country == "US" ?}
- **Mercury**（推荐） — 免费，为初创企业设计。如果你以后想自动化记账，它有出色的 API。
- **Relay** — 免费，适合将不同收入流分到子账户。
{? elif regional.country == "GB" ?}
- **Starling Bank**（推荐） — 免费商业账户，即时开户。
- **Wise Business** — 低成本多币种。如果你服务国际客户非常好用。
{? else ?}
- **Mercury**（美国） — 免费，为初创企业设计。如果你以后想自动化记账，它有出色的 API。
- **Relay**（美国） — 免费，适合将不同收入流分到子账户。
- **Starling Bank**（英国） — 免费商业账户。
{? endif ?}
- **Wise Business**（国际） — 低成本多币种。适合接收 USD、EUR、GBP 等货币的付款。
- **Qonto**（欧盟） — 面向欧洲企业的简洁商业银行。

现在就开户。在线操作需要 10-15 分钟，验证需要 1-3 天。

### 第 6 步：开发者副业的税务基础

{? if regional.tax_note ?}
> **{= regional.country | fallback("your region") =} 的税务提示：** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **说句实话：** 税务是大多数开发者忽视到四月份才恐慌的事情。现在花 30 分钟能为你节省真金白银和压力。

**美国：**
- 年副业收入超过 $400 需要缴纳自雇税（约 15.3%，用于社会保障+医疗保险）
- 加上你常规收入税率对净利润的部分
- **季度预估税：** 如果你将欠超过 $1,000 的税款，IRS 期望你季度缴纳（4月15日、6月15日、9月15日、1月15日）。少缴会触发罚款。
- 将净收入的 **25-30%** 留出来作为税款。立即存入单独的储蓄账户。

**开发者副业收入的常见抵扣项：**
- API 费用（OpenAI、Anthropic 等）— 100% 可抵扣
- 商业用途的硬件购买 — 可折旧或 Section 179 扣除
- 可归属于商业用途的电费
- 用于收入工作的软件订阅
- 家庭办公扣除（简化版：$5/平方英尺，最多 300 平方英尺 = $1,500）
- 互联网（商业使用比例）
- 域名、主机、邮件服务
- 与你的收入工作相关的专业发展（课程、书籍）

**英国：**
- 通过自我评估纳税申报表报告
- 交易收入低于 GBP 1,000：免税（交易免税额）
- 超过该金额：对利润缴纳所得税 + 4 类国民保险
- 缴纳日期：1 月 31 日和 7 月 31 日

**从第一天起就记录一切。** 最简单也要用一个电子表格：

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **常见错误：** "税的事以后再说。"等到了第四季度，你欠了 $3,000 的预估税加罚款，而你已经把钱花了。自动化：每次收入到达商业账户时，立即将 30% 转入税款储蓄账户。

### 第 4 课检查点

你现在应该已经完成（或有计划完成）：
- [ ] 阅读了你劳动合同的知识产权条款
- [ ] 你计划的收入工作通过了三个问题测试
- [ ] 选择了商业结构（或决定以个人独资开始）
- [ ] 设置了支付处理（Stripe 或 Lemon Squeezy）
- [ ] 使用模板生成器完成了隐私政策和服务条款
- [ ] 开设了独立的商业银行账户（或已提交申请）
- [ ] 税务策略：30% 预留 + 季度缴纳计划

*在完整的 STREETS 课程中，模块 E（执行手册）包含财务建模模板，可自动计算你的纳税义务、项目盈利能力以及每个收入引擎的盈亏平衡点。*

---

## 第 5 课：每月 {= regional.currency_symbol | fallback("$") =}200 的预算

*"你的业务有燃烧率。了解它。控制它。让它赚钱。"*

### 为什么是每月 {= regional.currency_symbol | fallback("$") =}200

每月两百{= regional.currency | fallback("dollars") =}是开发者收入运营的最低可行预算。它足以运行真实的服务、服务真实的客户、产生真实的收入。同时它也足够小，如果什么都行不通，你也没有押上全部身家。

目标很简单：**在 90 天内将每月 {= regional.currency_symbol | fallback("$") =}200 变成每月 {= regional.currency_symbol | fallback("$") =}600 以上。** 如果你能做到，你就有了一门生意。如果做不到，改变策略——而不是增加预算。

### 预算分配

#### 层级 1：API 额度 — $50-100/月

这是你面向客户的高质量生产算力。

**推荐的初始分配：**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**如何管理 API 支出：**

```python
# Simple API budget tracker — run daily via cron
# Save as: check_api_spend.py

import requests
import json
from datetime import datetime

# Check Anthropic usage
# (Anthropic provides usage in the dashboard; here's how to track locally)

MONTHLY_BUDGET = {
    "anthropic": 40.00,
    "openai": 20.00,
    "deepseek": 10.00,
}

# Track locally by logging every API call cost
USAGE_LOG = "api_usage.jsonl"

def get_monthly_spend(provider: str) -> float:
    """Calculate current month's spend for a provider."""
    current_month = datetime.now().strftime("%Y-%m")
    total = 0.0
    try:
        with open(USAGE_LOG, "r") as f:
            for line in f:
                entry = json.loads(line)
                if entry["provider"] == provider and entry["date"].startswith(current_month):
                    total += entry["cost"]
    except FileNotFoundError:
        pass
    return total

def log_api_call(provider: str, tokens_in: int, tokens_out: int, model: str):
    """Log an API call for budget tracking."""
    # Cost per 1M tokens (update these as pricing changes)
    PRICING = {
        "claude-3.5-sonnet": {"input": 3.00, "output": 15.00},
        "claude-3.5-haiku": {"input": 0.80, "output": 4.00},
        "gpt-4o-mini": {"input": 0.15, "output": 0.60},
        "gpt-4o": {"input": 2.50, "output": 10.00},
        "deepseek-v3": {"input": 0.27, "output": 1.10},
    }

    prices = PRICING.get(model, {"input": 1.0, "output": 5.0})
    cost = (tokens_in / 1_000_000 * prices["input"]) + \
           (tokens_out / 1_000_000 * prices["output"])

    entry = {
        "date": datetime.now().isoformat(),
        "provider": provider,
        "model": model,
        "tokens_in": tokens_in,
        "tokens_out": tokens_out,
        "cost": round(cost, 6),
    }

    with open(USAGE_LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")

    # Budget warning
    monthly_spend = get_monthly_spend(provider)
    budget = MONTHLY_BUDGET.get(provider, 0)
    if monthly_spend > budget * 0.8:
        print(f"WARNING: {provider} spend at {monthly_spend:.2f}/{budget:.2f} "
              f"({monthly_spend/budget*100:.0f}%)")

    return cost
```

**混合支出策略：**
- 使用本地 LLM 处理 80% 的工作（分类、提取、摘要、草稿）
- 使用 API 调用处理 20% 的工作（最终质量审核、复杂推理、面向客户的输出）
- 与纯 API 使用相比，你的每任务有效成本大幅下降

{? if computed.monthly_electricity_estimate ?}
> **你的估计电费：** 24/7 运行每月 {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}（按 {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh 计算）。这已经计入你的有效运营成本。
{? endif ?}

#### 层级 2：基础设施 — 每月 {= regional.currency_symbol | fallback("$") =}30-50

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**你需要 VPS 吗？**

如果你的收入模式是：
- **销售数字产品：** 不需要。在 Vercel/Netlify 上免费托管。使用 Lemon Squeezy 进行交付。
- **为客户运行异步处理：** 可能需要。你可以在本地设备上运行作业并交付结果。VPS 增加了可靠性。
- **提供 API 服务：** 是的，很可能需要。一台 $5-10 的 VPS 可以作为轻量级 API 网关，即使重计算在你的本地机器上进行。
- **销售 SaaS：** 是的。但从最低价位开始，按需扩展。

**推荐的入门基础设施：**

```
Local rig — primary compute, LLM inference, heavy processing
   |
   +-- SSH tunnel or WireGuard VPN
   |
$5 VPS (Hetzner/DigitalOcean) — API gateway, webhook receiver, static hosting
   |
   +-- Cloudflare (free) — DNS, CDN, DDoS protection
   |
Vercel/Netlify (free) — marketing site, landing pages, docs
```

基础设施总成本：$5-20/月。其余都是免费层。

#### 层级 3：工具 — 每月 {= regional.currency_symbol | fallback("$") =}20-30

```
Analytics:              $0/month    — Plausible Cloud ($9) or self-hosted,
                                      or Vercel Analytics (free tier)
                                      or just Cloudflare analytics (free)
Email marketing:        $0/month    — Buttondown (free up to 100 subs),
                                      Resend ($0 for 3K emails/month)
Monitoring:             $0/month    — UptimeRobot (free, 50 monitors),
                                      Better Stack (free tier)
Design:                 $0/month    — Figma (free), Canva (free tier)
Accounting:             $0/month    — Wave (free), or a spreadsheet
                                      Hledger (free, plaintext accounting)
```

> **说句实话：** 起步阶段你可以完全用免费层运行你的整个工具栈。这里分配的 $20-30 是为了当你超出免费层限制或需要特定的付费功能时使用。不要仅仅因为预算里有就花掉它。未花掉的预算就是利润。

#### 层级 4：储备金 — 每月 {= regional.currency_symbol | fallback("$") =}0-30

这是你的"意料之外"基金：
- 意外的大批量作业导致 API 成本飙升
- 某个特定客户项目需要一个工具
- 发现了完美域名需要紧急购买
- 一次性购买（主题、模板、图标集）

如果你没用到储备金，它就会累积。连续 3 个月未使用的储备金，考虑重新分配到 API 额度或基础设施。

### ROI 计算

这是唯一重要的数字：

```
Monthly Revenue - Monthly Costs = Net Profit
Net Profit / Monthly Costs = ROI Multiple

Example:
$600 revenue - $200 costs = $400 profit
$400 / $200 = 2x ROI

The target: 3x ROI ($600+ revenue on $200 spend)
The minimum: 1x ROI ($200 revenue = break even)
Below 1x: Change strategy or reduce costs
```

{@ insight cost_projection @}

**何时增加预算：**

仅在以下情况增加预算：
1. 你连续 2 个月以上稳定在 2 倍以上 ROI
2. 更多支出能直接增加收入（例如，更多 API 额度=更多客户承载量）
3. 增加与一个具体的、经过验证的收入流挂钩

**何时不应增加预算：**
- "我觉得这个新工具会有帮助"（先测试免费替代品）
- "大家都说要花钱才能赚钱"（在这个阶段不适用）
- "更大的 VPS 会让我的服务更快"（速度真的是瓶颈吗？）
- 你还没达到 1 倍 ROI（修复收入，而不是增加支出）

**扩展阶梯：**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **常见错误：** 把这 {= regional.currency_symbol | fallback("$") =}200 当作不需要立即回报的"投资"。不对。这是一个有 90 天截止期限的实验。如果每月 {= regional.currency_symbol | fallback("$") =}200 在 90 天内没有产生每月 {= regional.currency_symbol | fallback("$") =}200 的收入，那么策略的某个环节需要改变。资金、市场、产品——总有什么不对。对自己诚实。

### 第 5 课检查点

你现在应该已经：
- [ ] 将约 $200 的月度预算分配到四个层级
- [ ] 创建了 API 账户并设置了消费限额
- [ ] 做出了基础设施决策（仅本地 vs 本地+VPS）
- [ ] 选择了工具栈（起步阶段大多使用免费层）
- [ ] ROI 目标：90 天内达到 3 倍
- [ ] 明确的规则：只有证明了 ROI 后才增加预算

*在完整的 STREETS 课程中，模块 E（执行手册）包含一个财务看板模板，实时跟踪你的支出、收入和每个收入引擎的 ROI——让你始终了解哪些收入流在盈利，哪些需要调整。*

---

## 第 6 课：你的主权技术栈文档

*"每个企业都有一份计划。这是你的——而且只需两页纸。"*

### 交付成果

这是你在模块 S 中要创建的最重要的东西。你的主权技术栈文档是一份汇总了你所有收入创造基础设施的单一参考文档。你将在 STREETS 课程的其余部分中持续参考它，随着你的配置发展不断更新，并用它来做出清晰的决策——该构建什么、该跳过什么。

创建一个新文件。Markdown、Google Doc、Notion 页面、纯文本——用任何你真正会持续维护的格式。使用下面的模板，用第 1-5 课的数字和决策填写每个字段。

### 模板

{? if computed.profile_completeness != "0" ?}
> **抢先一步：** 4DA 已经检测到了你的一些硬件规格和技术栈信息。查找下方的预填提示——它们能帮你节省填写模板的时间。
{? endif ?}

复制整个模板并填写。每个字段。不要跳过。

```markdown
# Sovereign Stack Document
# [Your Name or Business Name]
# Created: [Date]
# Last Updated: [Date]

---

## 1. HARDWARE INVENTORY

### Primary Machine
- **Type:** [Desktop / Laptop / Mac / Server]
- **CPU:** [Model] — [X] cores, [X] threads
- **RAM:** [X] GB [DDR4/DDR5]
- **GPU:** [Model] — [X] GB VRAM (or "None — CPU inference only")
- **Storage:** [X] GB SSD free / [X] GB total
- **OS:** [Linux distro / macOS version / Windows version]

### Network
- **Download:** [X] Mbps
- **Upload:** [X] Mbps
- **Latency to cloud APIs:** [X] ms
- **ISP reliability:** [Stable / Occasional outages / Unreliable]

### Uptime Capability
- **Can run 24/7:** [Yes / No — reason]
- **UPS:** [Yes / No]
- **Remote access:** [SSH / RDP / Tailscale / None]

### Monthly Infrastructure Cost
- **Electricity (24/7 estimate):** $[X]/month
- **Internet:** $[X]/month (business portion)
- **Total fixed infrastructure cost:** $[X]/month

---

## 2. LLM STACK

### Local Models (via Ollama)
| Model | Size | Tokens/sec | Use Case |
|-------|------|-----------|----------|
| [e.g., llama3.1:8b] | [X]B | [X] tok/s | [e.g., Classification, extraction] |
| [e.g., mistral:7b] | [X]B | [X] tok/s | [e.g., Summarization, drafts] |
| [e.g., deepseek-coder] | [X]B | [X] tok/s | [e.g., Code generation] |

### API Models (for quality-critical output)
| Provider | Model | Monthly Budget | Use Case |
|----------|-------|---------------|----------|
| [e.g., Anthropic] | [Claude 3.5 Sonnet] | $[X] | [e.g., Customer-facing content] |
| [e.g., OpenAI] | [GPT-4o-mini] | $[X] | [e.g., Volume processing fallback] |

### Inference Strategy
- **Local handles:** [X]% of requests ([list tasks])
- **API handles:** [X]% of requests ([list tasks])
- **Estimated blended cost per 1M tokens:** $[X]

---

## 3. MONTHLY BUDGET

| Category | Allocation | Actual (update monthly) |
|----------|-----------|------------------------|
| API Credits | $[X] | $[  ] |
| Infrastructure (VPS, domain, email) | $[X] | $[  ] |
| Tools (analytics, email marketing) | $[X] | $[  ] |
| Reserve | $[X] | $[  ] |
| **Total** | **$[X]** | **$[  ]** |

### Revenue Target
- **Month 1-3:** $[X]/month (minimum: cover costs)
- **Month 4-6:** $[X]/month
- **Month 7-12:** $[X]/month

---

## 4. LEGAL STATUS

- **Employment status:** [Employed / Freelance / Between jobs]
- **IP clause reviewed:** [Yes / No / N/A]
- **IP clause risk level:** [Clean / Murky — needs review / Restrictive]
- **Business entity:** [LLC / Ltd / Sole Proprietor / None yet]
  - **State/Country:** [Where registered]
  - **EIN/Tax ID:** [Obtained / Pending / Not needed yet]
- **Payment processing:** [Stripe / Lemon Squeezy / Other] — [Active / Pending]
- **Business bank account:** [Open / Pending / Using personal (fix this)]
- **Privacy policy:** [Done / Not yet — URL: ___]
- **Terms of service:** [Done / Not yet — URL: ___]

---

## 5. TIME INVENTORY

- **Available hours per week for income projects:** [X] hours
  - **Weekday mornings:** [X] hours
  - **Weekday evenings:** [X] hours
  - **Weekends:** [X] hours
- **Time zone:** [Your timezone]
- **Best deep work blocks:** [e.g., "Saturday 6am-12pm, weekday evenings 8-10pm"]

### Time Allocation Plan
| Activity | Hours/week |
|----------|-----------|
| Building/coding | [X] |
| Marketing/sales | [X] |
| Client work/delivery | [X] |
| Learning/experimentation | [X] |
| Admin (invoicing, email, etc.) | [X] |

> Rule: Never allocate more than 70% of available time.
> Life happens. Burnout is real. Leave buffer.

---

## 6. SKILLS INVENTORY

### Primary Skills (things you could teach others)
1. [Skill] — [years of experience]
2. [Skill] — [years of experience]
3. [Skill] — [years of experience]

### Secondary Skills (competent but not expert)
1. [Skill]
2. [Skill]
3. [Skill]

### Exploring (learning now or want to learn)
1. [Skill]
2. [Skill]

### Unique Combinations
What makes YOUR skill combination unusual? (This becomes your moat in Module T)
- [e.g., "I know both Rust AND healthcare data standards — very few people have both"]
- [e.g., "I can build full-stack apps AND I understand supply chain logistics from a previous career"]
- [e.g., "I'm fluent in 3 languages AND I can code — I can serve non-English markets that most dev tools ignore"]

---

## 7. SOVEREIGN STACK SUMMARY

### What I Can Offer Today
(Based on hardware + skills + time, what could you sell THIS WEEK if someone asked?)
1. [e.g., "Local document processing — extract data from PDFs privately"]
2. [e.g., "Custom automation scripts for [specific domain]"]
3. [e.g., "Technical writing / documentation"]

### What I'm Building Toward
(Based on the full STREETS framework — fill this in as you progress through the course)
1. [Revenue Engine 1 — from Module R]
2. [Revenue Engine 2 — from Module R]
3. [Revenue Engine 3 — from Module R]

### Key Constraints
(Be honest — these aren't weaknesses, they're parameters)
- [e.g., "Only 10 hours/week available"]
- [e.g., "No GPU — CPU inference only, will rely on APIs for LLM tasks"]
- [e.g., "Employment contract is restrictive — need to stay in unrelated domains"]
- [e.g., "Non-US based — some payment/legal options are limited"]

---

*This document is a living reference. Update it monthly.*
*Next review date: [Date + 30 days]*
```

{? if dna.primary_stack ?}
> **从你的 Developer DNA 预填：**
> - **主要技术栈：** {= dna.primary_stack | fallback("Not detected") =}
> - **兴趣领域：** {= dna.interests | fallback("Not detected") =}
> - **身份摘要：** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **需要注意的盲点：** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **从检测到的技术栈预填：** 你的主要技术是 {= stack.primary | fallback("not yet detected") =}。{? if stack.adjacent ?}相邻技能：{= stack.adjacent | fallback("none detected") =}。{? endif ?} 用这些信息填写上方的技能清单。
{? endif ?}

{@ insight t_shape @}

### 如何使用这份文档

1. **在开始任何新项目之前：** 查看你的主权技术栈。你是否有硬件、时间、技能和预算来执行？
2. **在购买任何东西之前：** 查看你的预算分配。这笔购买在计划中吗？
3. **每月回顾：** 更新预算中的"实际"列。更新收入数据。根据效果调整分配。
4. **当别人问你做什么的时候：** 你的"我今天能提供什么"部分就是你的即时推介。
5. **当你忍不住想追逐新点子时：** 查看你的约束条件。这是否符合你的时间、技能和硬件？如果不符合，把它添加到"正在构建的方向"留到以后。

### 一小时练习

设一个 60 分钟的计时器。填写模板的每个字段。不要想太多。不要大量研究。写下你现在知道的。以后可以更新。

填不上的字段？那些就是你本周的行动项：
- 基准测试数据为空？运行第 2 课的基准测试脚本。
- 没有商业实体？开始第 4 课的注册流程。
- 没有支付处理？从第 4 课设置 Stripe。
- 技能清单空白？花 15 分钟列出过去 5 年中你因之获得报酬的所有事项。

> **常见错误：** 花 3 小时让文档"完美"，而不是花 1 小时让它"完成"。主权技术栈文档是一份工作参考，不是给投资者看的商业计划。除了你自己没人会看它。准确性很重要。格式不重要。

### 第 6 课检查点

你现在应该已经：
- [ ] 一份完整的主权技术栈文档，保存在你真正会打开的地方
- [ ] 所有六个部分都用真实数字填写（不是期望值）
- [ ] 针对你配置中的缺口有一份明确的行动项清单
- [ ] 设定了第一次月度回顾的日期（从现在起 30 天后）

---

## 模块 S：完成

{? if progress.completed("MODULE_S") ?}
> **模块 S 已完成。** 你已完成 {= progress.completed_count | fallback("1") =}/{= progress.total_count | fallback("7") =} 个 STREETS 模块。{? if progress.completed_modules ?}已完成：{= progress.completed_modules | fallback("S") =}。{? endif ?}
{? endif ?}

### 你在两周内建成了什么

看看你现在拥有的、在开始之前还没有的：

1. **一份硬件清单**，映射到收入创造能力——不仅仅是贴纸上的规格参数。
2. **一套生产级的本地 LLM 技术栈**，使用 Ollama，在你的实际硬件上做了基准测试，为真实工作负载做了配置。
3. **一项隐私优势**，你知道如何营销——针对特定受众有特定的话术。
4. **法律和财务基础** — 商业实体（或计划）、支付处理、银行账户、税务策略。
5. **受控的预算**，有明确的 ROI 目标和 90 天的验证期限。
6. **一份主权技术栈文档**，将以上所有内容汇总在一份参考文档中，你未来的每个决策都会用到它。

这比大多数开发者曾经搭建的都要多。真的。大多数想赚副业收入的人直接跳到"做个酷东西"，然后纳闷为什么收不到钱。你现在拥有了收钱的基础设施。

但没有方向的基础设施只是一个昂贵的爱好。你需要知道该把这套技术栈瞄准哪里。

{@ temporal market_timing @}

### 接下来：模块 T — 技术护城河

模块 S 为你奠定了基础。模块 T 回答那个关键问题：**你如何构建竞争对手无法轻易复制的东西？**

以下是模块 T 涵盖的内容：

- **专有数据流水线** — 如何合法且合乎道德地创建只有你能访问的数据集
- **自定义模型配置** — 微调和提示工程，让输出质量达到别人用默认设置无法匹配的水平
- **复合技能组合** — 为什么"Python+医疗"在收入方面胜过"Python+JavaScript"，以及如何识别你的独特组合
- **技术进入壁垒** — 竞争对手需要数月才能复制的基础设施设计
- **护城河审计** — 一个评估你的项目是否具有可防御优势还是只是另一个大宗商品服务的框架

一个月赚 $500 和一个月赚 $5,000 的开发者之间的差距很少是技能。是护城河。是那些让你的产品难以被复制的东西，即使别人有相同的硬件和相同的模型。

### 完整 STREETS 路线图

| 模块 | 标题 | 重点 | 持续时间 |
|------|------|------|---------|
| **S** | 主权配置 | 基础设施、法律、预算 | 第 1-2 周（已完成） |
| **T** | 技术护城河 | 可防御优势、专有资产 | 第 3-4 周 |
| **R** | 收入引擎 | 带代码的具体变现方案 | 第 5-8 周 |
| **E** | 执行手册 | 上线流程、定价、第一批客户 | 第 9-10 周 |
| **E** | 进化前沿 | 保持领先、趋势检测、适应 | 第 11-12 周 |
| **T** | 战术自动化 | 自动化运营以实现被动收入 | 第 13-14 周 |
| **S** | 叠加收入流 | 多收入来源、组合策略 | 第 15-16 周 |

模块 R（收入引擎）是赚到大部分钱的地方。但如果没有 S 和 T，你就是在沙上建楼。

---

**准备好获取完整方案了吗？**

你已经看到了基础。你亲手搭建了它。现在获取完整的系统。

**获取 STREETS Core** — 完整的 16 周课程，包含全部七个模块、收入引擎代码模板、财务看板，以及由按自己规则创造收入的开发者组成的私密社区。

*你的设备。你的规则。你的收入。*
