# モジュール S: ソブリン・セットアップ

**STREETS デベロッパー収入コース — 無料モジュール**
*第1-2週 | 全6レッスン | 成果物: ソブリン・スタック・ドキュメント*

> 「あなたの環境はビジネスインフラだ。そのつもりで構築しよう。」

---

あなたはすでに、ほとんどの人が手にすることのない最強の収益ツールを持っています。インターネット接続、ローカルの計算力、そしてそれらをすべて繋ぎ合わせるスキルを備えた開発者ワークステーションです。

多くの開発者は自分の環境をただの消費者向け製品として扱っています。ゲームをして、コードを書いて、ブラウジングするための道具として。しかしその同じマシン — 今あなたのデスクの下にあるそのマシン — は、推論を実行し、APIを提供し、データを処理し、あなたが眠っている間も24時間収益を生み出すことができます。

このモジュールは、すでに持っているものを別の視点で見ることについてです。「何を作れるか？」ではなく、「何を売れるか？」という視点です。

この2週間が終わる頃には、以下のものが手に入ります:

- 収益を生み出す能力の明確なインベントリ
- プロダクション・グレードのローカル LLM スタック
- 法的・財務的な基盤（最小限であっても）
- ビジネスの青写真となるソブリン・スタック・ドキュメント

曖昧な話はなし。「自分を信じよう」もなし。実際の数字、実際のコマンド、実際の判断です。

{@ mirror sovereign_readiness @}

さあ、始めましょう。

---

## レッスン 1: リグ監査

*「4090は必要ない。本当に重要なことはこれだ。」*

### あなたのマシンはビジネス資産

企業がインフラを評価するとき、ただスペックを並べるのではなく — 能力を収益機会にマッピングします。今からあなたがやるのはまさにそれです。

{? if computed.profile_completeness != "0" ?}
> **あなたの現在の環境:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} コア / {= profile.cpu.threads | fallback("?") =} スレッド), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} 空き / {= profile.storage.total | fallback("?") =} 合計 ({= profile.storage.type | fallback("unknown") =}), OS: {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}
{? endif ?}

ターミナルを開いて、以下を順に実行してください。すべての数値を書き留めてください。レッスン6のソブリン・スタック・ドキュメントで必要になります。

### ハードウェア・インベントリ

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

**収益にとって重要なこと:**
- コア数は、あなたの環境が同時に処理できるタスクの数を決定します。ローカル LLM を動かしながら同時にバッチジョブを処理するには、真の並列処理能力が必要です。
{? if profile.cpu.cores ?}
- *あなたの {= profile.cpu.model | fallback("CPU") =} は {= profile.cpu.cores | fallback("?") =} コアです — 下の要件テーブルで、あなたの CPU がどの収益エンジンに対応しているか確認してください。*
{? endif ?}
- このコースのほとんどの収益エンジンには、直近5年以内の8コア以上の最新 CPU があれば十分です。
- GPU なしの CPU のみでローカル LLM を実行する場合、16コア以上が望ましいです。Ryzen 7 5800X または Intel i7-12700 が実用的な最低ラインです。

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**収益にとって重要なこと:**
- 16 GB: 最低限。7Bモデルの実行と基本的な自動化作業が可能です。
- 32 GB: 快適。13Bモデルをローカルで実行し、複数プロジェクトを扱い、開発環境と収益ワークロードを並行稼働できます。
- 64 GB以上: 30B以上のモデルを CPU で実行したり、複数モデルを同時にロードできます。推論サービスの販売において本領を発揮する領域です。
{? if profile.ram.total ?}
*あなたのシステムには {= profile.ram.total | fallback("?") =} RAM が搭載されています。上の表でどの能力ティアに該当するか確認してください — これはローカルで実用的なモデルに直接影響し、収益ワークロードを左右します。*
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

**収益にとって重要なこと:**

これはみんなが執着するスペックですが、正直に言うと: **GPU がローカル LLM のティアを決め、ローカル LLM のティアがどの収益ストリームが最速で稼働するかを決めます。** ただし、稼げるかどうかそのものを決めるわけではありません。

| VRAM | LLM 能力 | 収益との関連性 |
|------|---------|--------------|
| 0 (CPU のみ) | 7Bモデルで約5トークン/秒 | バッチ処理、非同期作業。遅いが機能する。 |
| 6-8 GB (RTX 3060 等) | 7Bモデルで約30トークン/秒、13B量子化 | ほとんどの自動化収益ストリームに十分。 |
| 12 GB (RTX 3060 12GB, 4070) | 13Bをフルスピードで、30B量子化 | スイートスポット。ほとんどの収益エンジンが快適に動作。 |
| 16-24 GB (RTX 4090, 3090) | 30B-70Bモデル | プレミアムティア。他の人がローカルでは実現できない品質を販売可能。 |
| 48 GB以上 (デュアルGPU, A6000) | 70B以上を高速で | エンタープライズ・グレードのローカル推論。深刻な競争優位。 |
| Apple Silicon 32GB以上 (M2/M3 Pro/Max) | ユニファイドメモリで30B以上 | 優れた効率。NVIDIA 同等品より消費電力が低い。 |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **あなたの GPU:** {= profile.gpu.model | fallback("Unknown") =}、{= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}プレミアムティアです。30B-70Bモデルがローカルで実行可能です。これは深刻な競争優位です。{? elif computed.gpu_tier == "sweet_spot" ?}スイートスポットにいます。13Bをフルスピードで、30B量子化で実行可能。ほとんどの収益エンジンが快適に動作します。{? elif computed.gpu_tier == "capable" ?}7Bモデルを良好な速度で、13Bを量子化で実行可能です。ほとんどの自動化収益ストリームに十分です。{? else ?}GPU アクセラレーションが利用可能です。上の表でどのティアに該当するか確認してください。{? endif ?}
{? else ?}
> **専用 GPU が検出されませんでした。** CPU で推論を実行することになり、7Bモデルで約5-12トークン/秒になります。バッチ処理や非同期作業には問題ありません。顧客向け出力の速度ギャップを埋めるにはAPI呼び出しを使ってください。
{? endif ?}

> **率直に言うと:** RTX 3060 12GB を持っているなら、AI の収益化を試みている開発者の95%よりも有利な立場にいます。4090を待つのはやめましょう。RTX 3060 12GB はローカル AI のホンダ・シビックです — 信頼性が高く、効率的で、仕事を確実にこなします。GPU アップグレードに使うお金は、顧客向けの品質を確保する API クレジットに充てた方が賢明です。ローカルモデルには力仕事を任せましょう。

#### ストレージ

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**収益にとって重要なこと:**
- LLM モデルは容量を消費します: 7Bモデル = 約4 GB、13B = 約8 GB、70B = 約40 GB（量子化済み）。
- プロジェクトデータ、データベース、キャッシュ、出力アーティファクト用のスペースが必要です。
- 顧客向けの用途には SSD が必須です。HDD からのモデル読み込みは起動時間に30-60秒追加されます。
- 実用的な最低限: 500 GB SSD、空き容量100 GB以上。
- 快適: 1 TB SSD。モデルは SSD に、アーカイブは HDD に。
{? if profile.storage.free ?}
*あなたは {= profile.storage.type | fallback("your drive") =} に {= profile.storage.free | fallback("?") =} の空き容量があります。{? if profile.storage.type == "SSD" ?}良好 — SSD なので高速なモデル読み込みが可能です。{? elif profile.storage.type == "NVMe" ?}優秀 — NVMe はモデル読み込みに最速の選択肢です。{? else ?}まだ SSD を使っていなければ検討してください — モデルのロード時間に大きな差が出ます。{? endif ?}*
{? endif ?}

#### ネットワーク

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**収益にとって重要なこと:**
{? if profile.network.download ?}
*あなたの接続: {= profile.network.download | fallback("?") =} 下り / {= profile.network.upload | fallback("?") =} 上り*
{? endif ?}
- ダウンロード速度: 50 Mbps以上。モデル、パッケージ、データの取得に必要です。
- アップロード速度: ほとんどの人が無視するボトルネックです。何かを提供する場合（API、処理結果、成果物）、アップロード速度が重要になります。
  - 10 Mbps: 非同期配信（処理済みファイル、バッチ結果）には十分。
  - 50 Mbps以上: 外部サービスがアクセスするローカル API エンドポイントを運用する場合に必要。
  - 100 Mbps以上: このコースのすべてに快適。
- レイテンシ: 主要クラウドプロバイダーまで50ms未満。`ping api.openai.com` と `ping api.anthropic.com` で確認してください。

#### アップタイム

これは誰も考えないスペックですが、趣味と「寝ている間に稼ぐ人」を分ける要素です。

自問してください:
- あなたの環境は24時間365日稼働できますか？（電力、冷却、騒音）
- 停電時用の UPS はありますか？
- インターネット接続は自動化ワークフローに十分安定していますか？
- 何かが壊れた場合、リモートから SSH 接続できますか？

24時間稼働できなくても問題ありません — このコースの多くの収益ストリームは、手動でトリガーする非同期バッチジョブです。しかし、真にパッシブな収入を生むものにはアップタイムが必要です。

{? if computed.os_family == "windows" ?}
**クイック・アップタイム設定 (Windows):** タスク スケジューラで自動再起動を設定し、リモート デスクトップを有効にするか Tailscale をインストールしてリモートアクセスを確保し、停電からの復旧のために BIOS で「AC 電源復旧時に復元」を設定してください。
{? endif ?}

**クイック・アップタイム設定（必要な場合）:**

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

### 電気代の計算

電気代を無視する人と、大げさに恐れる人の両方がいます。実際の計算をしましょう。

**実際の消費電力を測定する:**

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

**月額コストの計算:**

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
あなたの電気料金: 約 {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh（{= regional.country | fallback("your region") =} の平均に基づく）。実際の電気料金は請求書で確認してください — プロバイダーや時間帯によって異なります。
{? else ?}
米国の平均電気料金は約 $0.12/kWh です。実際の料金を確認してください — 大きく変動します。カリフォルニアでは $0.25/kWh かもしれません。一部のヨーロッパ諸国は $0.35/kWh に達します。米国中西部の一部は $0.08/kWh です。
{? endif ?}

**要点:** あなたの環境を24時間365日稼働させる電気代は、月 {= regional.currency_symbol | fallback("$") =}1 〜 {= regional.currency_symbol | fallback("$") =}30 程度です。収益ストリームがそれをカバーできないなら、問題は電気代ではなく — 収益ストリームの方です。

### 収益エンジン別の最低スペック

STREETS コース全体で向かう先のプレビューです。今はあなたの環境がどこに位置するか確認するだけで十分です:

| 収益エンジン | CPU | RAM | GPU | ストレージ | ネットワーク |
|------------|-----|-----|-----|-----------|------------|
| **コンテンツ自動化**（ブログ記事、ニュースレター） | 4コア以上 | 16 GB | オプション（API フォールバック） | 50 GB 空き | 上り 10 Mbps |
| **データ処理サービス** | 8コア以上 | 32 GB | オプション | 200 GB 空き | 上り 50 Mbps |
| **ローカル AI API サービス** | 8コア以上 | 32 GB | 8 GB以上 VRAM | 100 GB 空き | 上り 50 Mbps |
| **コード生成ツール** | 8コア以上 | 16 GB | 8 GB以上 VRAM または API | 50 GB 空き | 上り 10 Mbps |
| **ドキュメント処理** | 4コア以上 | 16 GB | オプション | 100 GB 空き | 上り 10 Mbps |
| **自律エージェント** | 8コア以上 | 32 GB | 12 GB以上 VRAM | 100 GB 空き | 上り 50 Mbps |

> **よくある間違い:** 「始める前にハードウェアをアップグレードしなきゃ。」いいえ。今あるもので始めてください。ハードウェアでカバーできないギャップは API 呼び出しで埋めましょう。アップグレードは収益がそれを正当化してから — その前ではなく。

{@ insight engine_ranking @}

### レッスン 1 チェックポイント

以下を書き留めているはずです:
- [ ] CPU モデル、コア数、スレッド数
- [ ] RAM 容量
- [ ] GPU モデルと VRAM（または「なし」）
- [ ] 利用可能なストレージ
- [ ] ネットワーク速度（下り/上り）
- [ ] 24時間稼働時の月間推定電気代
- [ ] あなたの環境が該当する収益エンジンのカテゴリ

これらの数値を保存してください。レッスン6でソブリン・スタック・ドキュメントに入力します。

{? if computed.profile_completeness != "0" ?}
> **4DA はこれらの数値のほとんどをすでに収集しています。** 上のパーソナライズされたサマリーを確認してください — ハードウェア・インベントリはシステム検出から一部自動入力されています。
{? endif ?}

*完全版 STREETS コースでは、モジュール R（収益エンジン）で上記の各エンジンタイプについて、具体的なステップバイステップのプレイブックを提供します — 構築とデプロイの正確なコードを含めて。*

---

## レッスン 2: ローカル LLM スタック

*「Ollama をプロダクション用にセットアップする — チャット用ではなく。」*

### ローカル LLM が収益にとって重要な理由

OpenAI API を呼び出すたびに、あなたは家賃を払っています。ローカルでモデルを実行するたびに、初期セットアップ後の推論は無料です。計算は単純です:

- GPT-4o: 入力100万トークンあたり約 $5、出力100万トークンあたり約 $15
- Claude 3.5 Sonnet: 入力100万トークンあたり約 $3、出力100万トークンあたり約 $15
- ローカル Llama 3.1 8B: 100万トークンあたり $0（電気代のみ）

何千ものリクエストを処理するサービスを構築する場合、100万トークンあたり $0 と $5-$15 の差は、利益と損益分岐点の差です。

しかし、ほとんどの人が見落とすニュアンスがあります: **ローカルモデルと API モデルは、収益スタックにおいて異なる役割を果たします。** ローカルモデルはボリュームを処理します。API モデルは品質が重要な顧客向け出力を処理します。あなたのスタックには両方が必要です。

### Ollama のインストール

{? if settings.has_llm ?}
> **LLM はすでに設定済みです:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}。Ollama がすでに動作している場合は、下の「モデル選択ガイド」まで飛ばしてください。
{? endif ?}

Ollama が基盤です。あなたのマシンを、クリーンな API を備えたローカル推論サーバーに変えます。

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
> **Windows:** ollama.com からインストーラーをダウンロードするか、`winget install Ollama.Ollama` を使用してください。Ollama はインストール後、バックグラウンドサービスとして自動的に実行されます。
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` が最も手早い方法です。Ollama は Apple Silicon のユニファイドメモリを活用します — あなたの {= profile.ram.total | fallback("system") =} RAM は CPU と GPU のワークロード間で共有されます。
{? elif computed.os_family == "linux" ?}
> **Linux:** インストールスクリプトがすべてを処理します。{= profile.os.name | fallback("Linux") =} を実行している場合、Ollama は systemd サービスとしてインストールされます。
{? endif ?}

インストールを確認します:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **バージョンに関する注意:** Ollama は頻繁にリリースされます。このモジュールのモデルコマンドとフラグは、Ollama v0.5.x（2026年初頭）で検証されています。後でこの文書を読んでいる場合は、最新バージョンは [ollama.com/download](https://ollama.com/download)、現在のモデル名は [ollama.com/library](https://ollama.com/library) で確認してください。コアコンセプトは変わりませんが、特定のモデルタグ（例: `llama3.1:8b`）は新しいリリースで置き換えられている場合があります。

### モデル選択ガイド

見つけたモデルを片っ端からダウンロードしないでください。戦略的に。何を取得し、それぞれをいつ使うかを示します。

{? if computed.llm_tier ?}
> **あなたの LLM ティア（ハードウェアに基づく）:** {= computed.llm_tier | fallback("unknown") =}。以下の推奨事項にはタグが付いているので、あなたの環境に合ったティアに集中できます。
{? endif ?}

#### ティア 1: ワークホース（7B-8B モデル）

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**用途:**
- テキスト分類（「このメールはスパムか正当か？」）
- 要約（長いドキュメントを箇条書きに凝縮）
- 簡単なデータ抽出（テキストから名前、日付、金額を抽出）
- 感情分析
- コンテンツのタグ付けとカテゴリ分類
- エンベディング生成（エンベディング対応モデルを使用する場合）

**パフォーマンス（一般的）:**
- RTX 3060 12GB: 約40-60トークン/秒
- RTX 4090: 約100-130トークン/秒
- M2 Pro 16GB: 約30-45トークン/秒
- CPU のみ (Ryzen 7 5800X): 約8-12トークン/秒

**コスト比較:**
- GPT-4o-mini で100万トークン: 約 $0.60
- ローカル（8Bモデル）で100万トークン: 電気代約 $0.003
- 損益分岐点: 約5,000トークン（文字通り最初のリクエストからコスト削減開始）

#### ティア 2: バランス型（13B-14B モデル）

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**用途:**
- コンテンツ作成（ブログ記事、ドキュメント、マーケティングコピー）
- コード生成（関数、スクリプト、ボイラープレート）
- 複雑なデータ変換
- マルチステップの推論タスク
- ニュアンスのある翻訳

**パフォーマンス（一般的）:**
- RTX 3060 12GB: 約20-30トークン/秒（量子化）
- RTX 4090: 約60-80トークン/秒
- M2 Pro 32GB: 約20-30トークン/秒
- CPU のみ: 約3-6トークン/秒（リアルタイムには実用的でない）

**7B より使うべきタイミング:** 7B の出力品質が十分でないが、API 呼び出しにお金を払う必要がない場合。実際のユースケースで両方テストしてください — 7B で十分な場合もあり、計算リソースを無駄にしているだけかもしれません。

{? if computed.gpu_tier == "capable" ?}
> **ティア 3 の境界領域** — あなたの {= profile.gpu.model | fallback("GPU") =} は 30B 量子化をある程度処理できますが、70B はローカルでは手が届きません。70B レベルの品質が必要なタスクには API 呼び出しを検討してください。
{? endif ?}

#### ティア 3: 品質重視ティア（30B-70B モデル）

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**用途:**
- 優れた品質が求められる顧客向けコンテンツ
- 複雑な分析と推論
- 長文コンテンツの生成
- 品質が支払い意思に直結するタスク

**パフォーマンス（一般的）:**
- RTX 4090 (24GB): 70B で約8-15トークン/秒（使用可能だが遅い）
- デュアル GPU または 48GB以上: 70B で約20-30トークン/秒
- M3 Max 64GB: 70B で約10-15トークン/秒

> **率直に言うと:** 24GB以上の VRAM がなければ、70B モデルは完全にスキップしてください。品質重視の出力には API 呼び出しを使いましょう。システム RAM から 3トークン/秒で 70B モデルを実行することは技術的には可能ですが、どの収益ワークフローにも実質的に使えません。あなたの時間には価値があります。

#### ティア 4: API モデル（ローカルでは不十分な場合）

ローカルモデルはボリュームとプライバシーのため。API モデルは品質の上限と特殊な能力のためです。

**API モデルを使うべきとき:**
- 品質 = 収益となる顧客向け出力（セールスコピー、プレミアムコンテンツ）
- 小規模モデルでは失敗する複雑な推論チェーン
- ビジョン/マルチモーダルタスク（画像、スクリーンショット、ドキュメントの分析）
- 高い信頼性で構造化 JSON 出力が必要な場合
- 速度が重要でローカルハードウェアが遅い場合

**コスト比較表（2025年初頭時点 — 最新の料金を確認してください）:**

| モデル | 入力（100万トークンあたり） | 出力（100万トークンあたり） | 最適な用途 |
|-------|--------------------------|--------------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | 安価な大量処理（ローカルが使えない場合） |
| GPT-4o | $2.50 | $10.00 | ビジョン、複雑な推論 |
| Claude 3.5 Sonnet | $3.00 | $15.00 | コード、分析、ロングコンテキスト |
| Claude 3.5 Haiku | $0.80 | $4.00 | 高速、低コスト、良好な品質バランス |
| DeepSeek V3 | $0.27 | $1.10 | コスト効率が良く、強力なパフォーマンス |

**ハイブリッド戦略:**
1. ローカル 7B/13B がリクエストの80%を処理（分類、抽出、要約）
2. API がリクエストの20%を処理（品質重視の生成、複雑なタスク）
3. あなたの実効コスト: ブレンドで100万トークンあたり約 $0.50-2.00（純粋な API の $5-15 に対して）

このハイブリッドアプローチが、健全な利益率でサービスを構築する方法です。詳細はモジュール R で。

### プロダクション構成

収益ワークのために Ollama を運用するのは、個人的なチャットのために動かすのとは異なります。適切に構成する方法を紹介します。

{? if computed.has_nvidia ?}
> **NVIDIA GPU が検出されました ({= profile.gpu.model | fallback("unknown") =})。** Ollama は自動的に CUDA アクセラレーションを使用します。NVIDIA ドライバーが最新であることを確認してください — `nvidia-smi` で確認できます。{= profile.gpu.vram | fallback("your") =} VRAM で最高のパフォーマンスを得るには、以下の `OLLAMA_MAX_LOADED_MODELS` 設定を、VRAM に同時に収まるモデル数に合わせてください。
{? endif ?}

#### 環境変数の設定

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

#### ワークロード用の Modelfile を作成する

デフォルトのモデル設定を使う代わりに、収益ワークロードに最適化したカスタム Modelfile を作成します:

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

#### バッチ処理とキュー管理

収益ワークロードでは、多数のアイテムを処理する必要があることが多いです。基本的なバッチ処理のセットアップを紹介します:

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

### あなたの環境をベンチマークする

他人のベンチマークを信用しないでください。自分で測定しましょう:

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

各モデルのトークン/秒を書き留めてください。この数値が、あなたの環境でどの収益ワークフローが実用的かを決定します。

{@ insight stack_fit @}

**ユースケース別の速度要件:**
- バッチ処理（非同期）: 5トークン/秒以上で問題なし（レイテンシは気にしない）
- インタラクティブツール（ユーザーが待つ）: 最低20トークン/秒
- リアルタイム API（顧客向け）: 良好な UX のために30トークン/秒以上
- ストリーミングチャット: 15トークン/秒以上でレスポンシブに感じる

### ローカル推論サーバーのセキュリティ

{? if computed.os_family == "windows" ?}
> **Windows に関する注意:** Windows 上の Ollama はデフォルトで localhost にバインドします。PowerShell で `netstat -an | findstr 11434` を使って確認してください。Windows ファイアウォールでポート 11434 への外部アクセスをブロックしてください。
{? elif computed.os_family == "macos" ?}
> **macOS に関する注意:** macOS 上の Ollama はデフォルトで localhost にバインドします。`lsof -i :11434` で確認してください。macOS のファイアウォールが外部接続を自動的にブロックするはずです。
{? endif ?}

あなたの Ollama インスタンスは、明示的に意図しない限り、インターネットからアクセスできてはいけません。

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

> **よくある間違い:** 「便利だから」と Ollama を 0.0.0.0 にバインドして、そのまま忘れること。あなたの IP を見つけた誰でも、あなたの GPU を無料推論に使えます。さらに悪いことに、モデルの重みやシステムプロンプトを抽出できます。常に localhost。常にトンネル経由で。

### レッスン 2 チェックポイント

以下が完了しているはずです:
- [ ] Ollama がインストールされ、動作している
- [ ] 少なくとも1つのワークホースモデルを取得済み（llama3.1:8b または同等品）
- [ ] 予想されるワークロード用のカスタム Modelfile
- [ ] ベンチマーク数値: あなたの環境での各モデルのトークン/秒
- [ ] Ollama が localhost のみにバインドされている

*完全版 STREETS コースでは、モジュール T（テクニカル・モート）で、競合が簡単に複製できない独自のモデル構成、ファインチューニングパイプライン、カスタムツールチェーンの構築方法を紹介します。モジュール R（収益エンジン）では、このスタックの上に構築する具体的なサービスを提供します。*

---

## レッスン 3: プライバシーの優位性

*「プライベートなセットアップ自体が競争優位だ — 単なる好みではなく。」*

### プライバシーは製品機能であり、制約ではない

ほとんどの開発者は、個人的にプライバシーを重視しているから、あるいは技術をいじるのが楽しいからローカルインフラを構築します。それは良いことです。しかし、**プライバシーは今のテック業界で最も市場価値の高い機能の一つ**だということに気づかなければ、大きな収益機会を逃しています。

理由はこうです: 企業が OpenAI の API にデータを送るたびに、そのデータはサードパーティを通過します。多くの企業 — 特にヘルスケア、金融、法律、政府、EU に拠点を置く企業 — にとって、これは現実の問題です。理論上の問題ではありません。「コンプライアンス部門がダメと言ったのでこのツールは使えません」という問題です。

あなたは、自分のマシンでモデルをローカル実行しているので、その問題がありません。

### 規制の追い風

規制環境はあなたに有利な方向に動いています。急速に。

{? if regional.country == "US" ?}
> **米国在住:** 以下の規制で最も重要なのは HIPAA、SOC 2、ITAR、および州レベルのプライバシー法（カリフォルニア CCPA 等）です。EU の規制も重要です — ヨーロッパのクライアントにサービスを提供する能力に影響し、それは収益性の高い市場です。
{? elif regional.country == "GB" ?}
> **英国在住:** Brexit 後、英国は独自のデータ保護フレームワーク（UK GDPR + Data Protection Act 2018）を持っています。あなたのローカル処理の優位性は、英国の金融サービスや NHS 関連の仕事に特に強力です。
{? elif regional.country == "DE" ?}
> **ドイツ在住:** あなたは世界で最も厳しいデータ保護環境の一つにいます。これは*優位性*です — ドイツのクライアントはなぜローカル処理が重要かすでに理解しており、それに対して対価を払います。
{? elif regional.country == "AU" ?}
> **オーストラリア在住:** Privacy Act 1988 と Australian Privacy Principles (APPs) があなたの義務を規定しています。ローカル処理は、My Health Records Act の下にある政府機関やヘルスケアクライアントへの強力なセールスポイントです。
{? endif ?}

**EU AI Act（2024-2026年に施行）:**
- 高リスク AI システムには文書化されたデータ処理パイプラインが必要
- 企業はデータの流れと処理者を証明する必要がある
- ローカル処理はコンプライアンスを劇的に簡素化する
- EU 企業は EU データレジデンシーを保証できる AI サービスプロバイダーを積極的に探している

**GDPR（すでに施行中）:**
- 「データ処理」には LLM API へのテキスト送信が含まれる
- 企業はすべてのサードパーティとデータ処理契約（DPA）を結ぶ必要がある
- ローカル処理はサードパーティを完全に排除する
- これは本物のセールスポイントだ: 「あなたのデータがインフラを離れることはありません。交渉すべきサードパーティの DPA はありません。」

**業界固有の規制:**
- **HIPAA（米国ヘルスケア）:** 患者データは BAA（業務提携契約）なしに消費者向け AI API に送信できません。ほとんどの AI プロバイダーは API アクセスに BAA を提供していません。ローカル処理はこれを完全に回避します。
- **SOC 2（エンタープライズ）:** SOC 2 監査を受ける企業は、すべてのデータ処理者を文書化する必要があります。処理者が少ない = 監査が容易。
- **ITAR（米国防衛）:** 規制技術データは米国の管轄を離れることができません。国際的なインフラを持つクラウド AI プロバイダーは問題があります。
- **PCI DSS（金融）:** カード会員データの処理には、データの移動先に関する厳格な要件があります。

### セールス会話でプライバシーをポジショニングする方法

コンプライアンスの専門家である必要はありません。3つのフレーズを理解し、いつ使うかを知っていればいいのです:

**フレーズ 1: 「あなたのデータがインフラを離れることはありません。」**
使うタイミング: プライバシーを重視する見込み客と話すとき。これは万能のフックです。

**フレーズ 2: 「サードパーティのデータ処理契約は不要です。」**
使うタイミング: ヨーロッパの企業や法務/コンプライアンスチームを持つ企業と話すとき。これにより数週間の法的レビューが不要になります。

**フレーズ 3: 「完全な監査証跡、シングルテナント処理。」**
使うタイミング: エンタープライズまたは規制産業と話すとき。彼らは AI パイプラインを監査人に証明する必要があります。

**ポジショニングの例（サービスページや提案書向け）:**

> 「クラウドベースの AI サービスとは異なり、[あなたのサービス] はすべてのデータを専用ハードウェア上でローカル処理します。あなたのドキュメント、コード、データは処理環境を離れることがありません。パイプラインにサードパーティ API は存在せず、交渉すべきデータ共有契約もなく、すべての操作の完全な監査ログがあります。これにより [あなたのサービス] は、GDPR、HIPAA、SOC 2 コンプライアンス環境を含む、厳格なデータ取り扱い要件を持つ組織に適しています。」

このパラグラフをランディングページに載せれば、プレミアム料金を支払うまさにそのクライアントを引き寄せるでしょう。

### プレミアム価格設定の正当化

ハードナンバーでのビジネスケースはこうです:

**標準的な AI 処理サービス（クラウド API を使用）:**
- クライアントのデータは OpenAI/Anthropic/Google に送られる
- API を呼べるすべての開発者と競合している
- 市場レート: ドキュメント1件あたり $0.01-0.05
- 本質的にマークアップ付きで API アクセスを再販している

**プライバシーファーストの AI 処理サービス（あなたのローカルスタック）:**
- クライアントのデータはあなたのマシンに留まる
- はるかに小さなプロバイダープールと競合している
- 市場レート: ドキュメント1件あたり $0.10-0.50（5-10倍のプレミアム）
- インフラ + 専門知識 + コンプライアンスを販売している

プライバシー・プレミアムは現実のものです: 同じ基盤タスクに対して、コモディティのクラウドベースサービスの **5倍から10倍**。そしてそれを支払うクライアントは、より忠実で、価格に敏感でなく、より大きな予算を持っています。

{@ insight competitive_position @}

### 隔離されたワークスペースの構築

本業がある場合（ほとんどの方がそうでしょう）、雇用主の仕事と副業の間にクリーンな分離が必要です。これは法的保護だけでなく — 運用上の衛生でもあります。

{? if computed.os_family == "windows" ?}
> **Windows のコツ:** 副業用に別の Windows ユーザーアカウントを作成してください（設定 > アカウント > 家族とその他のユーザー > その他のユーザーを追加）。これにより完全に隔離された環境が得られます — 別のブラウザプロファイル、別のファイルパス、別の環境変数。Win+L でアカウントを切り替えられます。
{? endif ?}

**オプション 1: 別のユーザーアカウント（推奨）**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**オプション 2: コンテナ化されたワークスペース**

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

**オプション 3: 別の物理マシン（最も確実）**

これに真剣で、収益がそれを正当化するなら、専用マシンはすべての疑問を排除します。RTX 3060 搭載の中古 Dell OptiPlex は $400-600 で、最初の1ヶ月のクライアント作業で元が取れます。

**最低限の分離チェックリスト:**
- [ ] 副業プロジェクトは別のディレクトリに（雇用主のリポジトリと混在させない）
- [ ] 副業用の API キーは別に（雇用主が提供するキーを絶対に使わない）
- [ ] 副業関連アカウント用に別のブラウザプロファイル
- [ ] 副業は雇用主のハードウェアでは絶対に行わない
- [ ] 副業は雇用主のネットワークでは絶対に行わない（個人のインターネットまたは VPN を使用）
- [ ] 副業プロジェクト用の別の GitHub/GitLab アカウント（任意だがクリーン）

> **よくある間違い:** サイドプロジェクトの「テスト用に」雇用主の OpenAI API キーを使うこと。これは雇用主の請求ダッシュボードから見えるペーパートレイルを作り、IP（知的財産）の境界を曖昧にします。自分のキーを取得しましょう。安いです。

### レッスン 3 チェックポイント

以下を理解しているはずです:
- [ ] プライバシーが単なる個人的な好みではなく、市場価値のある製品機能である理由
- [ ] どの規制がローカル AI 処理への需要を生み出すか
- [ ] プライバシーに関するセールス会話で使う3つのフレーズ
- [ ] プライバシーファーストのサービスが5-10倍のプレミアム価格を実現する理由
- [ ] 副業を雇用主の仕事から分離する方法

*完全版 STREETS コースでは、モジュール E（進化するエッジ）で、規制の変化を追跡し、競合が存在すら知らない新しいコンプライアンス要件に先んじてポジショニングする方法を教えます。*

---

## レッスン 4: 法的最低限

*「今15分の法的準備をすれば、後で数ヶ月の問題を防げる。」*

### これは法的助言ではない

私は開発者であって、弁護士ではありません。以下は、ほとんどの状況でほとんどの開発者が対処すべき実践的なチェックリストです。あなたの状況が複雑な場合（雇用主の株式保有、特定の条件の競業避止義務など）、雇用弁護士との30分の相談に $200 を使ってください。最高の ROI が得られます。

### ステップ 1: 雇用契約を読む

雇用契約書または内定通知書を見つけてください。以下のセクションを探してください:

**知的財産譲渡条項** — 以下のような文言を探してください:
- "All inventions, developments, and work product..."（すべての発明、開発、成果物...）
- "...created during the term of employment..."（...雇用期間中に作成された...）
- "...related to the Company's business or anticipated business..."（...会社の事業または予定事業に関連する...）

**あなたを制限するキーフレーズ:**
- 「雇用中に作成されたすべての成果物は会社に帰属する」（広範 — 潜在的に問題あり）
- 「会社のリソースを使用して作成された成果物」（より狭い — 自分の機材を使えば通常問題なし）
- 「会社の現在または予定の事業に関連する」（雇用主が何をしているかによる）

**あなたを自由にするキーフレーズ:**
- 「従業員が完全に自身の時間で、自身のリソースを使い、会社の事業に無関係に行った仕事を除く」（これがあなたのカーブアウト — 米国の多くの州ではこれが求められている）
- 一部の州（カリフォルニア、ワシントン、ミネソタ、イリノイ等）には、契約書の内容に関係なく、個人プロジェクトに対する雇用主の IP 請求を制限する法律がある。

### 3つの質問テスト

収益プロジェクトについて、以下を自問してください:

1. **時間:** 自分の時間にこの仕事をしていますか？（勤務時間中でなく、オンコールシフト中でもなく）
2. **機材:** 自分のハードウェア、自分のインターネット、自分の API キーを使っていますか？（雇用主のラップトップでなく、雇用主の VPN でなく、雇用主のクラウドアカウントでもなく）
3. **対象分野:** 雇用主の事業と無関係ですか？（ヘルスケア AI 企業で働いていて、ヘルスケア AI サービスを売りたい場合...それは問題です。ヘルスケア AI 企業で働いていて、不動産エージェント向けのドキュメント処理を売りたい場合...それは問題ありません。）

3つすべてがクリーンなら、ほぼ確実に問題ありません。どれか一つでも曖昧なら、進める前に明確にしてください。

> **率直に言うと:** 副業をする開発者の大多数は何の問題もありません。雇用主が気にするのは競争優位の保護であり、無関係なプロジェクトで追加収入を得ることを阻止することではありません。しかし「ほぼ確実に問題ない」は「確実に問題ない」とは異なります。契約が異常に広範な場合は、上司や人事と会話するか — 弁護士に相談してください。確認しないことのデメリットは、確認する際の多少の気まずさよりもはるかに大きいです。

### ステップ 2: 事業形態を選ぶ

個人資産と事業活動を分離し、事業用銀行口座、決済処理、税制優遇への道を開くために、法人格が必要です。

{? if regional.country ?}
> **あなたの所在地: {= regional.country | fallback("Unknown") =}。** あなたの地域で推奨される法人形態は **{= regional.business_entity_type | fallback("LLC or equivalent") =}** で、一般的な登録費用は {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =} です。下のあなたの国のセクションまでスクロールするか、他の地域のクライアントがどのように運営しているかを理解するためにすべてのセクションを読んでください。
{? endif ?}

{? if regional.country == "US" ?}
#### 米国（あなたの地域）
{? else ?}
#### 米国
{? endif ?}

| 形態 | 費用 | 保護 | 最適な用途 |
|------|------|------|----------|
| **個人事業主**（デフォルト） | $0 | なし（個人責任） | 試しに始める段階。最初の $1K。 |
| **シングルメンバー LLC** | $50-500（州により異なる） | 個人資産の保護 | アクティブな収益活動。ほとんどの開発者はここから始めるべき。 |
| **S-Corp 選択**（LLC 上） | LLC の費用 + 選択自体は $0 | LLC と同じ + 給与税の優遇 | この副業から年間 $40K 以上を安定して稼いでいるとき |

**米国の開発者への推奨:** 居住州でのシングルメンバー LLC。

**設立費用が最も安い州:** ワイオミング（$100、州所得税なし）、ニューメキシコ（$50）、モンタナ（$70）。ただし、特別な理由がなければ居住州での設立が最もシンプルです。

**申請方法:**
1. 州の Secretary of State のウェブサイトにアクセス
2. 「form LLC」または「business entity filing」で検索
3. Articles of Organization を提出（10分のフォーム）
4. IRS から EIN を取得（無料、irs.gov で5分）

{? if regional.country == "GB" ?}
#### 英国（あなたの地域）
{? else ?}
#### 英国
{? endif ?}

| 形態 | 費用 | 保護 | 最適な用途 |
|------|------|------|----------|
| **個人事業主** | 無料（HMRC に登録） | なし | 最初の収入。テスト段階。 |
| **有限会社 (Ltd)** | Companies House で約 £15 | 個人資産の保護 | 本格的な収益活動全般。 |

**推奨:** Companies House 経由の Ltd 会社。約20分で GBP 12 です。

#### 欧州連合

国によって大きく異なりますが、一般的なパターン:

- **ドイツ:** 開始時は Einzelunternehmer（個人事業主）、本格的な仕事には GmbH（ただし GmbH は EUR 25,000 の資本が必要 — EUR 1 で始められる UG を検討）
- **オランダ:** Eenmanszaak（個人事業主、登録無料）または BV（Ltd に相当）
- **フランス:** Micro-entrepreneur（簡易型、開始時に推奨）
- **エストニア:** e-Residency + OUE（非居住者に人気、完全オンライン）

{? if regional.country == "AU" ?}
#### オーストラリア（あなたの地域）
{? else ?}
#### オーストラリア
{? endif ?}

| 形態 | 費用 | 保護 | 最適な用途 |
|------|------|------|----------|
| **個人事業主** | 無料 ABN | なし | 開始時 |
| **Pty Ltd** | ASIC 経由で約 AUD 500-800 | 個人資産の保護 | 本格的な収入 |

**推奨:** 個人事業主 ABN（無料、即時発行）で始め、安定して稼げるようになったら Pty Ltd に移行。

### ステップ 3: 決済処理（15分でセットアップ）

支払いを受ける方法が必要です。最初のクライアントが待っているときではなく、今セットアップしてください。

{? if regional.payment_processors ?}
> **{= regional.country | fallback("your region") =} への推奨:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe（ほとんどの開発者に推奨）:**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

所要時間: 約15分。すぐに支払いの受け付けを開始できます（Stripe は新規アカウントでは7日間資金を保留します）。

**LemonSqueezy（デジタル製品に推奨）:**

デジタル製品（テンプレート、ツール、コース、SaaS）を販売する場合、LemonSqueezy があなたの Merchant of Record として機能します。これは:
- 世界中の消費税、VAT、GST を代行処理してくれる
- EU での VAT 登録が不要になる
- 返金やチャージバックを処理してくれる

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas（海外の開発者や米国法人が欲しい方向け）:**

米国外にいるが、米国法人で米国の顧客に販売したい場合:
- $500 の一回払い
- デラウェア LLC を設立してくれる
- 米国の銀行口座を開設（Mercury または Stripe 経由）
- 登録代理人サービスを提供
- 約1-2週間

### ステップ 4: プライバシーポリシーと利用規約

オンラインでサービスや製品を販売するなら、これらが必要です。定型文に弁護士費用を払う必要はありません。

**無料で信頼できるテンプレートのソース:**
- **Termly.io** — 無料のプライバシーポリシーと利用規約ジェネレーター。質問に答えれば文書が生成されます。
- **Avodocs.com** — スタートアップ向けのオープンソース法的文書。無料。
- **GitHub の choosealicense.com** — オープンソースプロジェクトのライセンス専用。
- **Basecamp のオープンソースポリシー** — 「Basecamp open source policies」で検索 — 分かりやすい英語の良いテンプレートです。

**プライバシーポリシーがカバーすべき内容（クライアントデータを処理する場合）:**
- どのデータを収集するか
- どのように処理するか（ローカル — これがあなたの優位性）
- どのくらいの期間保持するか
- クライアントがどのように削除をリクエストできるか
- サードパーティがデータにアクセスするかどうか（理想的には: なし）

**所要時間:** テンプレートジェネレーターで30分。完了。

### ステップ 5: 事業用銀行口座

事業収入を個人の銀行口座に混ぜないでください。理由:

1. **税務の明確さ:** 確定申告の時期に、何が事業収入で何がそうでないかを正確に知る必要があります。
2. **法的保護:** LLC がある場合、個人と事業の資金を混同すると「法人格の否認」が起こり得ます — つまり裁判所が LLC の責任保護を無視できるということです。
3. **プロフェッショナリズム:** 「John's Consulting LLC」からの請求書が専用の事業口座に入金されるのは正当に見えます。個人の Venmo への支払いはそうではありません。

**無料または低コストの事業用銀行口座:**
{? if regional.country == "US" ?}
- **Mercury**（あなたに推奨） — 無料、スタートアップ向け設計。後で経理を自動化したい場合に優れた API。
- **Relay** — 無料、収入源をサブアカウントに分離するのに最適。
{? elif regional.country == "GB" ?}
- **Starling Bank**（あなたに推奨） — 無料のビジネスアカウント、即時セットアップ。
- **Wise Business** — 低コストのマルチカレンシー。国際的なクライアントにサービスを提供する場合に最適。
{? else ?}
- **Mercury**（米国） — 無料、スタートアップ向け設計。後で経理を自動化したい場合に優れた API。
- **Relay**（米国） — 無料、収入源をサブアカウントに分離するのに最適。
- **Starling Bank**（英国） — 無料のビジネスアカウント。
{? endif ?}
- **Wise Business**（国際） — 低コストのマルチカレンシー。USD、EUR、GBP などで支払いを受けるのに最適。
- **Qonto**（EU） — ヨーロッパ企業向けのクリーンな事業用銀行口座。

今すぐ口座を開設してください。オンラインで10-15分、認証に1-3日です。

### ステップ 6: 開発者の副業収入の税金の基本

{? if regional.tax_note ?}
> **{= regional.country | fallback("your region") =} の税金に関する注意:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **率直に言うと:** 税金は、ほとんどの開発者が4月まで無視して、それからパニックになるものです。今30分使えば、実際のお金とストレスが節約できます。

**米国:**
- 年間 $400 を超える副業収入には自営業税（社会保障 + メディケアで約15.3%）が必要
- さらに純利益に対する通常の所得税率
- **四半期ごとの予定納税:** 税金が $1,000 以上になる場合、IRS は四半期ごとの支払いを期待（4月15日、6月15日、9月15日、1月15日）。過少納付にはペナルティが発生。
- 純収入の **25-30%** を税金用に確保してください。すぐに別の貯蓄口座に入れましょう。

**開発者の副業収入でよくある控除対象:**
- API コスト（OpenAI、Anthropic 等） — 100% 控除可能
- 事業に使用するハードウェアの購入 — 減価償却または Section 179 控除
- 事業使用に帰属する電気代
- 収入活動に使用するソフトウェアサブスクリプション
- ホームオフィス控除（簡易版: 1平方フィートあたり $5、最大300平方フィート = $1,500）
- インターネット（事業使用割合分）
- ドメイン名、ホスティング、メールサービス
- 収入活動に関連する専門的な自己研鑽（コース、書籍）

**英国:**
- Self Assessment 確定申告で報告
- GBP 1,000 未満の取引収入: 非課税（Trading Allowance）
- それ以上: 利益に対して Income Tax + Class 4 NICs を支払い
- 支払期日: 1月31日と7月31日

**初日からすべてを記録してください。** 何もなければシンプルなスプレッドシートでも:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **よくある間違い:** 「税金は後で考えよう。」後とは第4四半期のことで、予定納税が $3,000 にペナルティがプラスされ、そのお金はすでに使ってしまっている。自動化しましょう: 事業口座に収入が入るたびに、すぐに30%を税金用貯蓄口座に振り替えてください。

### レッスン 4 チェックポイント

以下が完了している（またはその計画がある）はずです:
- [ ] 雇用契約の IP 条項を読んだ
- [ ] 計画している副業が3つの質問テストに合格した
- [ ] 事業形態を選択した（または個人事業主として開始することを決定）
- [ ] 決済処理をセットアップした（Stripe または LemonSqueezy）
- [ ] テンプレートジェネレーターからプライバシーポリシーと利用規約を作成
- [ ] 事業用銀行口座を開設（または申請済み）
- [ ] 税金戦略: 30%の確保 + 四半期ごとの支払いスケジュール

*完全版 STREETS コースでは、モジュール E（実行プレイブック）に、税金義務の自動計算、プロジェクトの収益性、各収益エンジンの損益分岐点を計算する財務モデリングテンプレートが含まれています。*

---

## レッスン 5: 月額 {= regional.currency_symbol | fallback("$") =}200 の予算

*「あなたのビジネスにはバーンレートがある。把握し、コントロールし、稼がせよう。」*

### なぜ {= regional.currency_symbol | fallback("$") =}200/月なのか

月200 {= regional.currency | fallback("dollars") =} は、開発者の収益事業を運営するための最低限実行可能な予算です。実際のサービスを運営し、実際の顧客にサービスを提供し、実際の収益を生み出すのに十分です。また、何もうまくいかなくても、全財産を賭けたことにはならない程度に小さい金額でもあります。

目標はシンプルです: **{= regional.currency_symbol | fallback("$") =}200/月を90日以内に {= regional.currency_symbol | fallback("$") =}600以上/月に変える。** それができればビジネスです。できなければ、戦略を変えましょう — 予算を増やすのではなく。

### 予算の内訳

#### ティア 1: API クレジット — $50-100/月

これは顧客向け品質のためのプロダクション・コンピュートです。

**推奨される初期配分:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**API 支出の管理方法:**

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

**ハイブリッド支出戦略:**
- ローカル LLM で処理の80%を実行（分類、抽出、要約、下書き）
- API 呼び出しで処理の20%を実行（最終品質パス、複雑な推論、顧客向け出力）
- 純粋な API 利用と比べて、タスクあたりの実効コストが大幅に低下

{? if computed.monthly_electricity_estimate ?}
> **あなたの推定電気代:** 24時間稼働で {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh の場合、月額 {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}。これはすでに実効運営コストに算入されています。
{? endif ?}

#### ティア 2: インフラ — {= regional.currency_symbol | fallback("$") =}30-50/月

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**VPS は必要ですか？**

あなたの収益モデルが:
- **デジタル製品の販売:** 不要。Vercel/Netlify で無料ホスティング。LemonSqueezy で配信。
- **クライアント向け非同期処理の実行:** おそらく必要。ローカル環境でジョブを実行して結果を納品できます。VPS は信頼性を向上させます。
- **API サービスの提供:** おそらく必要。$5-10 の VPS は、重い処理がローカルマシンで行われる場合でも、軽量な API ゲートウェイとして機能します。
- **SaaS の販売:** 必要。ただし最も安いティアから始めてスケールアップしましょう。

**推奨される初期インフラ:**

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

インフラの総コスト: $5-20/月。残りは無料ティアです。

#### ティア 3: ツール — {= regional.currency_symbol | fallback("$") =}20-30/月

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

> **率直に言うと:** 開始時は無料ティアだけでツールスタック全体を運用できます。ここに割り当てた $20-30 は、無料ティアを超えたときや特定のプレミアム機能が欲しいとき用です。予算にあるからといって使わないでください。使わなかった予算は利益です。

#### ティア 4: 予備費 — {= regional.currency_symbol | fallback("$") =}0-30/月

これは「予想しなかったこと」のための資金です:
- 予想外に大きなバッチジョブによる API コストの急増
- 特定のクライアントプロジェクトに必要なツール
- 完璧な名前を見つけたときの緊急ドメイン購入
- 一回限りの購入（テーマ、テンプレート、アイコンセット）

予備費を使わなければ、蓄積されます。3ヶ月間未使用の予備費があれば、API クレジットやインフラへの再配分を検討してください。

### ROI の計算

重要なのはこの数字だけです:

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

**予算を増やすべきとき:**

予算を増やすのは以下の場合のみ:
1. 2ヶ月以上安定して 2x 以上の ROI を達成している
2. 支出を増やすことが直接的に収益増加に繋がる（例: API クレジットを増やす = クライアント対応能力の向上）
3. 増加が特定の、テスト済みの収益ストリームに紐づいている

**予算を増やすべきでないとき:**
- 「この新しいツールが役立つと思う」（まず無料の代替手段をテストしてください）
- 「みんな、お金を稼ぐにはお金を使わなきゃって言う」（この段階ではそうではない）
- 「大きい VPS にすればサービスが速くなる」（速度が本当にボトルネックですか？）
- まだ 1x ROI に達していない（支出ではなく、収益を修正してください）

**スケーリングの階段:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **よくある間違い:** {= regional.currency_symbol | fallback("$") =}200 をすぐにリターンを生む必要のない「投資」として扱うこと。違います。これは90日間の期限付き実験です。{= regional.currency_symbol | fallback("$") =}200/月が90日以内に {= regional.currency_symbol | fallback("$") =}200/月の収益を生まなければ、戦略の何かを変える必要があります。お金、市場、オファー — 何かがうまくいっていません。自分に正直になりましょう。

### レッスン 5 チェックポイント

以下が完了しているはずです:
- [ ] 4つのティアに配分された月額約 $200 の予算
- [ ] 支出上限を設定して API アカウントを作成
- [ ] インフラの決定を完了（ローカルのみ vs. ローカル + VPS）
- [ ] ツールスタックを選択（開始時はほとんど無料ティア）
- [ ] ROI 目標: 90日以内に 3x
- [ ] 明確なルール: ROI を証明した後にのみ予算を増やす

*完全版 STREETS コースでは、モジュール E（実行プレイブック）に、支出、収益、収益エンジンごとの ROI をリアルタイムで追跡する財務ダッシュボードテンプレートが含まれています — どのストリームが収益性があり、どれを調整する必要があるか常に把握できます。*

---

## レッスン 6: ソブリン・スタック・ドキュメント

*「すべてのビジネスにはプランがある。これがあなたのプランだ — そして2ページに収まる。」*

### 成果物

これはモジュール S で作成する最も重要なものです。ソブリン・スタック・ドキュメントは、あなたの収益を生み出すインフラのすべてを記録した単一のリファレンスです。STREETS コースの残りの部分を通じて参照し、セットアップの進化に合わせて更新し、何を構築し何をスキップするかについて明確な判断を下すために使用します。

新しいファイルを作成してください。Markdown、Google Doc、Notion ページ、プレーンテキスト — 実際にメンテナンスするものなら何でも。以下のテンプレートを使い、レッスン1-5の数値と決定事項ですべてのフィールドを埋めてください。

### テンプレート

{? if computed.profile_completeness != "0" ?}
> **有利なスタート:** 4DA はすでにあなたのハードウェアスペックとスタック情報の一部を検出しています。以下の自動入力されたヒントを確認してください — テンプレートの入力時間を節約できます。
{? endif ?}

このテンプレート全体をコピーして入力してください。すべてのフィールド。スキップなし。

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
> **デベロッパー DNA からの事前入力:**
> - **プライマリスタック:** {= dna.primary_stack | fallback("Not detected") =}
> - **興味:** {= dna.interests | fallback("Not detected") =}
> - **アイデンティティ概要:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **注意すべき盲点:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **検出されたスタックからの事前入力:** あなたのプライマリ技術は {= stack.primary | fallback("not yet detected") =} です。{? if stack.adjacent ?}隣接スキル: {= stack.adjacent | fallback("none detected") =}。{? endif ?} これらを使って上のスキル・インベントリを入力してください。
{? endif ?}

{@ insight t_shape @}

### このドキュメントの使い方

1. **新しいプロジェクトを始める前に:** ソブリン・スタックを確認。そのハードウェア、時間、スキル、予算で実行可能ですか？
2. **何かを購入する前に:** 予算配分を確認。この購入は計画に含まれていますか？
3. **毎月のレビュー:** 予算の「実績」列を更新。収益数値を更新。うまくいっているものに基づいて配分を調整。
4. **あなたが何をしているか聞かれたとき:** 「今提供できること」セクションが即座のピッチになります。
5. **新しいアイデアに飛びつきたくなったとき:** 制約条件を確認。時間、スキル、ハードウェアの範囲内に収まりますか？ 収まらなければ「構築に向けて」に追加して後回しに。

### 1時間の演習

タイマーを60分にセットしてください。テンプレートのすべてのフィールドを入力してください。考えすぎないでください。広範なリサーチもしないでください。今知っていることをそのまま書いてください。後で更新できます。

入力できないフィールドは？ それが今週のアクションアイテムです:
- ベンチマーク数値が空？ レッスン2のベンチマークスクリプトを実行。
- 法人がない？ レッスン4の申請プロセスを開始。
- 決済処理がない？ レッスン4の Stripe セットアップを実行。
- スキル・インベントリが空白？ 15分かけて、過去5年間で報酬を受けたすべてのことをリストアップ。

> **よくある間違い:** 1時間で「完了」させる代わりに、3時間かけてドキュメントを「完璧」にしようとすること。ソブリン・スタック・ドキュメントは実用的なリファレンスであり、投資家向けのビジネスプランではありません。見るのはあなただけです。正確さは重要。フォーマットは重要ではありません。

### レッスン 6 チェックポイント

以下が完了しているはずです:
- [ ] 実際に開くであろう場所にソブリン・スタック・ドキュメントを保存した
- [ ] 全6セクションに実際の数値を入力（願望ではなく）
- [ ] セットアップの不足箇所に対する明確なアクションアイテムリスト
- [ ] 初回の月次レビュー日を設定（今日から30日後）

---

## モジュール S: 完了

{? if progress.completed("MODULE_S") ?}
> **モジュール S 完了。** STREETS の全 {= progress.total_count | fallback("7") =} モジュール中 {= progress.completed_count | fallback("1") =} を完了しました。{? if progress.completed_modules ?}完了済み: {= progress.completed_modules | fallback("S") =}。{? endif ?}
{? endif ?}

### 2週間で構築したもの

始める前にはなかったものが今あるか、見てみましょう:

1. **ハードウェア・インベントリ** — ステッカーのスペックではなく、収益を生み出す能力にマッピングされたもの。
2. **プロダクション・グレードのローカル LLM スタック** — Ollama を使い、実際のハードウェアでベンチマークし、実際のワークロード用に構成されたもの。
3. **プライバシーの優位性** — 特定の聴衆に対する特定の言葉で、どうマーケティングするか理解したもの。
4. **法的・財務的基盤** — 法人（または計画）、決済処理、銀行口座、税金戦略。
5. **コントロールされた予算** — 明確な ROI 目標と、モデルを証明するための90日間の期限。
6. **ソブリン・スタック・ドキュメント** — 上記すべてを、今後のあらゆる決定に使用する単一のリファレンスに集約したもの。

これはほとんどの開発者がこれまでにセットアップした以上のものです。本当に。副業収入を得たい人のほとんどは「何かクールなものを作る」に直行し、なぜ報酬を得られないのか不思議に思います。あなたには今、報酬を得るためのインフラがあります。

しかし、方向性のないインフラは高価な趣味に過ぎません。このスタックをどこに向けるか知る必要があります。

{@ temporal market_timing @}

### 次のステップ: モジュール T — テクニカル・モート

モジュール S で基盤を手に入れました。モジュール T は重要な問いに答えます: **競合が簡単にコピーできないものをどう構築するか？**

モジュール T がカバーする内容:

- **独自のデータパイプライン** — 合法かつ倫理的に、あなただけがアクセスできるデータセットの作り方
- **カスタムモデル構成** — デフォルト設定では他の人が実現できない出力品質を生み出すファインチューニングとプロンプトエンジニアリング
- **複利的スキルスタック** — なぜ「Python + ヘルスケア」が収入面で「Python + JavaScript」を上回るのか、そしてあなた独自の組み合わせをどう見つけるか
- **参入障壁としての技術** — 競合が複製するのに数ヶ月かかるインフラ設計
- **モート監査** — あなたのプロジェクトに防御可能な優位性があるか、単なるコモディティサービスかを評価するフレームワーク

月 $500 を稼ぐ開発者と月 $5,000 を稼ぐ開発者の差は、スキルであることはめったにありません。モートです。同じハードウェアと同じモデルを持っていても、あなたの提供物を複製しにくくするものです。

### STREETS 全体ロードマップ

| モジュール | タイトル | フォーカス | 期間 |
|----------|---------|----------|------|
| **S** | ソブリン・セットアップ | インフラ、法務、予算 | 第1-2週（完了） |
| **T** | テクニカル・モート | 防御可能な優位性、独自資産 | 第3-4週 |
| **R** | 収益エンジン | コード付きの具体的な収益化プレイブック | 第5-8週 |
| **E** | 実行プレイブック | ローンチシーケンス、価格設定、最初の顧客 | 第9-10週 |
| **E** | 進化するエッジ | 先を行く、トレンド検知、適応 | 第11-12週 |
| **T** | 戦術的自動化 | パッシブインカムのための運用自動化 | 第13-14週 |
| **S** | ストリームの積み上げ | 複数の収入源、ポートフォリオ戦略 | 第15-16週 |

モジュール R（収益エンジン）が最も収益を生む部分です。しかし S と T がなければ、砂の上に建てているようなものです。

---

**完全なプレイブックを手に入れる準備はできましたか？**

あなたは基盤を見てきました。自分で構築しました。今度は完全なシステムを手に入れましょう。

**STREETS Core を入手** — 全7モジュール、16週間の完全コース。収益エンジンのコードテンプレート、財務ダッシュボード、そして自分の条件で収入を築く開発者のプライベートコミュニティ付き。
