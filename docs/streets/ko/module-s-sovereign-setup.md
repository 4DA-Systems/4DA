# 모듈 S: 소버린 셋업 (Sovereign Setup)

**STREETS 개발자 수입 코스 — 무료 모듈**
*1-2주차 | 6개 레슨 | 결과물: 나만의 소버린 스택 문서*

> "당신의 작업 환경은 곧 비즈니스 인프라입니다. 그에 맞게 구성하세요."

---

여러분은 이미 대부분의 사람들이 절대 갖지 못할 가장 강력한 수입 창출 도구를 소유하고 있습니다. 인터넷에 연결된 개발자 워크스테이션, 로컬 컴퓨팅 파워, 그리고 이 모든 것을 연결할 수 있는 기술력이 바로 그것입니다.

대부분의 개발자들은 자신의 장비를 소비자 제품처럼 취급합니다. 게임을 하고, 코딩을 하고, 웹 서핑을 하는 용도로요. 하지만 바로 그 기계 — 지금 여러분의 책상 아래에 놓여 있는 바로 그 기계가 — 인퍼런스를 돌리고, API를 서빙하고, 데이터를 처리하고, 여러분이 자는 동안에도 24시간 수익을 창출할 수 있습니다.

이 모듈은 여러분이 이미 가지고 있는 것을 다른 관점으로 바라보는 것입니다. "무엇을 만들 수 있을까?"가 아니라 "무엇을 팔 수 있을까?"라는 관점으로요.

이 2주가 끝나면 여러분은 다음을 갖추게 됩니다:

- 수입 창출 역량에 대한 명확한 인벤토리
- 프로덕션 수준의 로컬 LLM 스택
- 법적, 재정적 기반 (최소한이라도)
- 비즈니스 청사진이 될 소버린 스택 문서

빈말 없습니다. "자신을 믿으세요" 같은 말도 없습니다. 실제 숫자, 실제 명령어, 실제 결정들입니다.

{@ mirror sovereign_readiness @}

시작하겠습니다.

---

## 레슨 1: 장비 감사

*"4090이 필요한 게 아닙니다. 정말 중요한 것이 무엇인지 알려드리겠습니다."*

### 여러분의 기계는 비즈니스 자산입니다

기업이 인프라를 평가할 때, 단순히 스펙을 나열하는 것이 아니라 역량을 수익 기회에 매핑합니다. 지금부터 여러분이 할 일이 바로 그것입니다.

{? if computed.profile_completeness != "0" ?}
> **현재 장비:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} 코어 / {= profile.cpu.threads | fallback("?") =} 스레드), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} 여유 / {= profile.storage.total | fallback("?") =} 전체 ({= profile.storage.type | fallback("unknown") =}), {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =} 실행 중.
{? endif ?}

터미널을 열고 다음 항목들을 확인하세요. 모든 숫자를 적어두세요. 레슨 6에서 소버린 스택 문서에 필요합니다.

### 하드웨어 인벤토리

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

**수입에 중요한 이유:**
- 코어 수는 여러분의 장비가 동시에 처리할 수 있는 작업 수를 결정합니다. 로컬 LLM을 돌리면서 동시에 배치 작업을 처리하려면 실제 병렬 처리가 필요합니다.
{? if profile.cpu.cores ?}
- *여러분의 {= profile.cpu.model | fallback("CPU") =}은(는) {= profile.cpu.cores | fallback("?") =}개의 코어를 갖고 있습니다 — 아래 요구사항 테이블을 확인하여 여러분의 CPU가 어떤 수익 엔진을 지원하는지 살펴보세요.*
{? endif ?}
- 이 코스의 대부분의 수익 엔진에는 최근 5년 이내의 8코어 이상 최신 CPU면 충분합니다.
- GPU 없이 CPU만으로 로컬 LLM을 돌리는 경우, 16코어 이상이 필요합니다. Ryzen 7 5800X 또는 Intel i7-12700이 실질적인 최소 사양입니다.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**수입에 중요한 이유:**
- 16 GB: 최소 사양입니다. 7B 모델을 돌리고 기본적인 자동화 작업을 할 수 있습니다.
- 32 GB: 쾌적합니다. 13B 모델을 로컬에서 돌리고, 여러 프로젝트를 처리하고, 수입 워크로드와 함께 개발 환경을 유지할 수 있습니다.
- 64 GB+: CPU에서 30B 이상 모델을 돌리거나 여러 모델을 동시에 로드할 수 있습니다. 인퍼런스 서비스를 판매하는 데 있어 흥미로워지는 구간입니다.
{? if profile.ram.total ?}
*여러분의 시스템에는 {= profile.ram.total | fallback("?") =} RAM이 있습니다. 위의 테이블을 확인하여 어느 역량 등급에 해당하는지 살펴보세요 — 이것이 수입 워크로드에 실용적인 로컬 모델을 직접 결정합니다.*
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

**수입에 중요한 이유:**

사람들이 가장 집착하는 스펙이 바로 이것입니다. 솔직한 진실은 이렇습니다: **여러분의 GPU가 로컬 LLM 등급을 결정하고, 로컬 LLM 등급이 어떤 수입 스트림이 가장 빠르게 작동하는지를 결정합니다.** 하지만 그것이 돈을 벌 수 있는지 여부를 결정하는 것은 아닙니다.

| VRAM | LLM 역량 | 수입 관련성 |
|------|---------|-----------|
| 0 (CPU 전용) | 7B 모델 약 5 토큰/초 | 배치 처리, 비동기 작업. 느리지만 작동합니다. |
| 6-8 GB (RTX 3060 등) | 7B 모델 약 30 토큰/초, 13B 양자화 | 대부분의 자동화 수입 스트림에 충분합니다. |
| 12 GB (RTX 3060 12GB, 4070) | 13B 풀 속도, 30B 양자화 | 최적 구간. 대부분의 수익 엔진이 여기서 잘 작동합니다. |
| 16-24 GB (RTX 4090, 3090) | 30B-70B 모델 | 프리미엄 등급. 다른 사람이 로컬에서 구현할 수 없는 품질을 판매하세요. |
| 48 GB+ (듀얼 GPU, A6000) | 70B 이상 속도 지원 | 엔터프라이즈급 로컬 인퍼런스. 진정한 경쟁 우위입니다. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 통합 메모리로 30B 이상 | 뛰어난 효율성. 동등한 NVIDIA보다 낮은 전력 비용. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **여러분의 GPU:** {= profile.gpu.model | fallback("Unknown") =}, {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}프리미엄 등급에 해당합니다. 30B-70B 모델을 로컬에서 돌릴 수 있습니다. 이것은 진정한 경쟁 우위입니다.{? elif computed.gpu_tier == "sweet_spot" ?}최적 구간에 해당합니다. 13B 풀 속도, 30B 양자화 가능. 대부분의 수익 엔진이 여기서 잘 작동합니다.{? elif computed.gpu_tier == "capable" ?}7B 모델을 좋은 속도로, 13B를 양자화로 돌릴 수 있습니다. 대부분의 자동화 수입 스트림에 충분합니다.{? else ?}GPU 가속을 사용할 수 있습니다. 위 테이블을 확인하여 어디에 해당하는지 살펴보세요.{? endif ?}
{? else ?}
> **전용 GPU가 감지되지 않았습니다.** CPU에서 인퍼런스를 실행하게 되며, 7B 모델 기준 약 5-12 토큰/초입니다. 배치 처리와 비동기 작업에는 문제없습니다. 고객 대면 출력의 속도 차이는 API 호출로 보완하세요.
{? endif ?}

> **솔직한 이야기:** RTX 3060 12GB를 가지고 있다면, AI 수익화를 시도하는 개발자의 95%보다 나은 위치에 있습니다. 4090을 기다리지 마세요. 3060 12GB는 로컬 AI의 혼다 시빅 같은 존재입니다 — 안정적이고, 효율적이며, 할 일을 해냅니다. GPU 업그레이드에 쓸 돈은 로컬 모델이 단순 작업을 처리하는 동안 고객 대면 품질을 위한 API 크레딧에 쓰는 것이 더 낫습니다.

#### 스토리지

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**수입에 중요한 이유:**
- LLM 모델은 용량을 차지합니다: 7B 모델 = 약 4 GB, 13B = 약 8 GB, 70B = 약 40 GB (양자화 기준).
- 프로젝트 데이터, 데이터베이스, 캐시, 출력 아티팩트를 위한 공간이 필요합니다.
- 고객 대면 작업에는 SSD가 필수입니다. HDD에서 모델을 로드하면 시작 시간이 30-60초 추가됩니다.
- 실용적 최소 사양: 500 GB SSD에 최소 100 GB 여유 공간.
- 쾌적한 수준: 1 TB SSD. 모델은 SSD에, 아카이브는 HDD에 보관하세요.
{? if profile.storage.free ?}
*여러분의 {= profile.storage.type | fallback("your drive") =}에 {= profile.storage.free | fallback("?") =} 여유 공간이 있습니다. {? if profile.storage.type == "SSD" ?}좋습니다 — SSD이므로 모델 로딩이 빠릅니다.{? elif profile.storage.type == "NVMe" ?}훌륭합니다 — NVMe는 모델 로딩에 가장 빠른 옵션입니다.{? else ?}SSD를 사용하지 않고 있다면 고려해보세요 — 모델 로드 시간에 확실한 차이가 있습니다.{? endif ?}*
{? endif ?}

#### 네트워크

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**수입에 중요한 이유:**
{? if profile.network.download ?}
*여러분의 연결: {= profile.network.download | fallback("?") =} 다운 / {= profile.network.upload | fallback("?") =} 업.*
{? endif ?}
- 다운로드 속도: 50+ Mbps. 모델, 패키지, 데이터를 받는 데 필요합니다.
- 업로드 속도: 대부분의 사람들이 무시하는 병목 지점입니다. 무언가를 서빙하고 있다면 (API, 처리된 결과, 산출물), 업로드가 중요합니다.
  - 10 Mbps: 비동기 전달에 적합합니다 (처리된 파일, 배치 결과).
  - 50+ Mbps: 외부 서비스가 접근하는 어떤 종류의 로컬 API 엔드포인트를 운영한다면 필요합니다.
  - 100+ Mbps: 이 코스의 모든 것에 쾌적합니다.
- 지연 시간: 주요 클라우드 프로바이더까지 50ms 미만. `ping api.openai.com`과 `ping api.anthropic.com`을 실행하여 확인하세요.

#### 업타임

아무도 생각하지 않는 스펙이지만, 취미 수준과 자면서 돈을 버는 사람을 구분하는 것이 바로 이것입니다.

스스로에게 물어보세요:
- 장비를 24/7 가동할 수 있나요? (전력, 냉각, 소음)
- 정전에 대비한 UPS가 있나요?
- 인터넷 연결이 자동화된 워크플로를 돌릴 만큼 안정적인가요?
- 문제가 생겼을 때 원격으로 SSH 접속할 수 있나요?

24/7 가동이 불가능하더라도 괜찮습니다 — 이 코스의 많은 수입 스트림은 수동으로 트리거하는 비동기 배치 작업입니다. 하지만 진정한 패시브 인컴을 생성하는 것들은 업타임이 필요합니다.

{? if computed.os_family == "windows" ?}
**빠른 업타임 설정 (Windows):** 자동 재시작을 위해 작업 스케줄러를 사용하고, 원격 데스크톱을 활성화하거나 Tailscale을 설치하여 원격 접속을 설정하고, 정전 복구를 위해 BIOS에서 "AC 전원 손실 시 복원"을 설정하세요.
{? endif ?}

**빠른 업타임 설정 (필요한 경우):**

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

### 전기 비용 계산

사람들은 이것을 무시하거나 과장합니다. 실제 계산을 해봅시다.

**실제 전력 소비 측정:**

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

**월간 비용 계산:**

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
여러분의 전기 요금: 약 {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh ({= regional.country | fallback("your region") =} 평균 기준). 실제 공과금 청구서를 확인하세요 — 요금은 공급자와 시간대에 따라 다릅니다.
{? else ?}
미국 평균 전기 요금은 약 $0.12/kWh입니다. 실제 요금을 확인하세요 — 지역에 따라 크게 다릅니다. 캘리포니아는 $0.25/kWh일 수 있습니다. 일부 유럽 국가는 $0.35/kWh에 달합니다. 미국 중서부 일부 지역은 $0.08/kWh입니다.
{? endif ?}

**핵심:** 수입 창출을 위해 장비를 24/7 가동하는 비용은 월 {= regional.currency_symbol | fallback("$") =}1~{= regional.currency_symbol | fallback("$") =}30 사이입니다. 수입 스트림이 이 비용을 감당할 수 없다면, 문제는 전기가 아니라 수입 스트림입니다.

### 수익 엔진 유형별 최소 사양

전체 STREETS 코스에서 다룰 내용을 미리 보여드립니다. 지금은 여러분의 장비가 어디에 해당하는지만 확인하세요:

| 수익 엔진 | CPU | RAM | GPU | 스토리지 | 네트워크 |
|----------|-----|-----|-----|---------|---------|
| **콘텐츠 자동화** (블로그 포스트, 뉴스레터) | 4코어 이상 | 16 GB | 선택사항 (API 대체 가능) | 50 GB 여유 | 10 Mbps 업 |
| **데이터 처리 서비스** | 8코어 이상 | 32 GB | 선택사항 | 200 GB 여유 | 50 Mbps 업 |
| **로컬 AI API 서비스** | 8코어 이상 | 32 GB | 8 GB 이상 VRAM | 100 GB 여유 | 50 Mbps 업 |
| **코드 생성 도구** | 8코어 이상 | 16 GB | 8 GB 이상 VRAM 또는 API | 50 GB 여유 | 10 Mbps 업 |
| **문서 처리** | 4코어 이상 | 16 GB | 선택사항 | 100 GB 여유 | 10 Mbps 업 |
| **자율 에이전트** | 8코어 이상 | 32 GB | 12 GB 이상 VRAM | 100 GB 여유 | 50 Mbps 업 |

> **흔한 실수:** "시작하기 전에 하드웨어를 업그레이드해야 해." 아닙니다. 가지고 있는 것으로 시작하세요. 하드웨어가 감당하지 못하는 부분은 API 호출로 보완하세요. 수익이 정당화할 때 업그레이드하세요 — 그 전이 아닙니다.

{@ insight engine_ranking @}

### 레슨 1 체크포인트

지금까지 다음을 적어두셨어야 합니다:
- [ ] CPU 모델, 코어 수, 스레드 수
- [ ] RAM 용량
- [ ] GPU 모델과 VRAM (또는 "없음")
- [ ] 사용 가능한 스토리지
- [ ] 네트워크 속도 (다운/업)
- [ ] 24/7 가동 시 예상 월간 전기 비용
- [ ] 여러분의 장비가 충족하는 수익 엔진 카테고리

이 숫자들을 보관하세요. 레슨 6에서 소버린 스택 문서에 넣을 것입니다.

{? if computed.profile_completeness != "0" ?}
> **4DA가 이미 이 숫자들의 대부분을 수집해두었습니다.** 위의 개인화된 요약을 확인하세요 — 시스템 감지를 통해 하드웨어 인벤토리가 부분적으로 미리 채워져 있습니다.
{? endif ?}

*전체 STREETS 코스에서 모듈 R (수익 엔진)은 위에 나열된 각 엔진 유형에 대해 구축 및 배포를 위한 구체적인 단계별 플레이북을 제공합니다 — 정확한 코드를 포함하여.*

---

## 레슨 2: 로컬 LLM 스택

*"Ollama를 프로덕션용으로 설정하세요 — 단순한 채팅용이 아닙니다."*

### 로컬 LLM이 수입에 중요한 이유

OpenAI API를 호출할 때마다 여러분은 임대료를 내고 있는 것입니다. 모델을 로컬에서 돌릴 때마다 그 인퍼런스는 초기 설정 이후 무료입니다. 계산은 간단합니다:

- GPT-4o: 입력 토큰 100만 개당 약 $5, 출력 토큰 100만 개당 약 $15
- Claude 3.5 Sonnet: 입력 토큰 100만 개당 약 $3, 출력 토큰 100만 개당 약 $15
- 로컬 Llama 3.1 8B: 토큰 100만 개당 $0 (전기료만)

수천 건의 요청을 처리하는 서비스를 구축하고 있다면, 100만 토큰당 $0과 $5-$15의 차이는 이익과 손익분기점의 차이입니다.

하지만 대부분의 사람들이 놓치는 뉘앙스가 있습니다: **수입 스택에서 로컬 모델과 API 모델은 서로 다른 역할을 합니다.** 로컬 모델은 대량 처리를 담당합니다. API 모델은 품질이 중요한 고객 대면 출력을 담당합니다. 여러분의 스택에는 둘 다 필요합니다.

### Ollama 설치

{? if settings.has_llm ?}
> **이미 LLM이 구성되어 있습니다:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}. Ollama가 이미 실행 중이라면 아래의 "모델 선택 가이드"로 건너뛰세요.
{? endif ?}

Ollama가 기반입니다. 여러분의 기계를 깔끔한 API를 갖춘 로컬 인퍼런스 서버로 바꿔줍니다.

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
> **Windows:** ollama.com의 설치 프로그램이나 `winget install Ollama.Ollama`를 사용하세요. Ollama는 설치 후 자동으로 백그라운드 서비스로 실행됩니다.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama`가 가장 빠른 방법입니다. Ollama는 Apple Silicon의 통합 메모리를 활용합니다 — 여러분의 {= profile.ram.total | fallback("system") =} RAM이 CPU와 GPU 워크로드 사이에 공유됩니다.
{? elif computed.os_family == "linux" ?}
> **Linux:** 설치 스크립트가 모든 것을 처리합니다. {= profile.os.name | fallback("Linux") =}을(를) 실행 중이라면 Ollama가 systemd 서비스로 설치됩니다.
{? endif ?}

설치를 확인하세요:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **버전 참고:** Ollama는 자주 릴리스됩니다. 이 모듈의 모델 명령어와 플래그는 Ollama v0.5.x (2026년 초) 기준으로 검증되었습니다. 나중에 읽고 계신다면 [ollama.com/download](https://ollama.com/download)에서 최신 버전을, [ollama.com/library](https://ollama.com/library)에서 현재 모델 이름을 확인하세요. 핵심 개념은 변하지 않지만, 특정 모델 태그(예: `llama3.1:8b`)는 새로운 릴리스로 대체될 수 있습니다.

### 모델 선택 가이드

보이는 모델을 모두 다운로드하지 마세요. 전략적으로 접근하세요. 무엇을 받고 언제 사용할지 알려드리겠습니다.

{? if computed.llm_tier ?}
> **여러분의 LLM 등급 (하드웨어 기반):** {= computed.llm_tier | fallback("unknown") =}. 아래 권장 사항에 등급이 표시되어 있으므로 여러분의 장비에 맞는 등급에 집중하세요.
{? endif ?}

#### 1등급: 주력 모델 (7B-8B 모델)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**용도:**
- 텍스트 분류 ("이 이메일은 스팸인가 정상인가?")
- 요약 (긴 문서를 글머리 기호로 압축)
- 간단한 데이터 추출 (텍스트에서 이름, 날짜, 금액 추출)
- 감성 분석
- 콘텐츠 태깅 및 분류
- 임베딩 생성 (임베딩을 지원하는 모델 사용 시)

**성능 (일반적):**
- RTX 3060 12GB: 약 40-60 토큰/초
- RTX 4090: 약 100-130 토큰/초
- M2 Pro 16GB: 약 30-45 토큰/초
- CPU 전용 (Ryzen 7 5800X): 약 8-12 토큰/초

**비용 비교:**
- GPT-4o-mini로 100만 토큰: 약 $0.60
- 로컬 8B 모델로 100만 토큰: 전기료 약 $0.003
- 손익분기점: 약 5,000 토큰 (말 그대로 첫 번째 요청부터 비용을 절약합니다)

#### 2등급: 균형 잡힌 선택 (13B-14B 모델)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**용도:**
- 콘텐츠 작성 (블로그 포스트, 문서, 마케팅 카피)
- 코드 생성 (함수, 스크립트, 보일러플레이트)
- 복잡한 데이터 변환
- 다단계 추론 작업
- 뉘앙스가 있는 번역

**성능 (일반적):**
- RTX 3060 12GB: 약 20-30 토큰/초 (양자화)
- RTX 4090: 약 60-80 토큰/초
- M2 Pro 32GB: 약 20-30 토큰/초
- CPU 전용: 약 3-6 토큰/초 (실시간으로는 실용적이지 않음)

**7B 대신 사용해야 할 때:** 7B의 출력 품질이 충분하지 않지만 API 호출 비용을 지불할 필요가 없을 때. 실제 사용 사례에서 둘 다 테스트해보세요 — 때때로 7B로 충분하며 컴퓨팅만 낭비하고 있을 수 있습니다.

{? if computed.gpu_tier == "capable" ?}
> **3등급 스트레치 영역** — 여러분의 {= profile.gpu.model | fallback("GPU") =}은(는) 약간의 노력으로 30B 양자화를 처리할 수 있지만, 70B는 로컬에서 불가능합니다. 70B 수준의 품질이 필요한 작업에는 API 호출을 고려하세요.
{? endif ?}

#### 3등급: 품질 등급 (30B-70B 모델)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**용도:**
- 우수한 품질이 필요한 고객 대면 콘텐츠
- 복잡한 분석 및 추론
- 장문 콘텐츠 생성
- 품질이 직접적으로 누군가의 결제 여부에 영향을 미치는 작업

**성능 (일반적):**
- RTX 4090 (24GB): 70B 약 8-15 토큰/초 (사용 가능하지만 느림)
- 듀얼 GPU 또는 48GB 이상: 70B 약 20-30 토큰/초
- M3 Max 64GB: 70B 약 10-15 토큰/초

> **솔직한 이야기:** 24GB 이상의 VRAM이 없다면, 70B 모델은 완전히 건너뛰세요. 품질이 중요한 출력에는 API 호출을 사용하세요. 시스템 RAM에서 3 토큰/초로 돌아가는 70B 모델은 기술적으로는 가능하지만 수입 창출 워크플로에는 실용적이지 않습니다. 여러분의 시간에는 가치가 있습니다.

#### 4등급: API 모델 (로컬만으로 부족할 때)

로컬 모델은 대량 처리와 프라이버시를 위한 것입니다. API 모델은 품질 상한선과 특화된 역량을 위한 것입니다.

**API 모델을 사용해야 할 때:**
- 품질 = 매출인 고객 대면 출력 (세일즈 카피, 프리미엄 콘텐츠)
- 작은 모델이 실패하는 복잡한 추론 체인
- 비전/멀티모달 작업 (이미지, 스크린샷, 문서 분석)
- 높은 신뢰성의 구조화된 JSON 출력이 필요할 때
- 속도가 중요하고 로컬 하드웨어가 느릴 때

**비용 비교 테이블 (2025년 초 기준 — 현재 가격을 확인하세요):**

| 모델 | 입력 (100만 토큰당) | 출력 (100만 토큰당) | 최적 용도 |
|------|-------------------|-------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | 저렴한 대량 작업 (로컬을 사용할 수 없을 때) |
| GPT-4o | $2.50 | $10.00 | 비전, 복잡한 추론 |
| Claude 3.5 Sonnet | $3.00 | $15.00 | 코드, 분석, 긴 컨텍스트 |
| Claude 3.5 Haiku | $0.80 | $4.00 | 빠르고, 저렴하고, 좋은 품질 밸런스 |
| DeepSeek V3 | $0.27 | $1.10 | 가성비 좋고, 강력한 성능 |

**하이브리드 전략:**
1. 로컬 7B/13B가 요청의 80%를 처리 (분류, 추출, 요약)
2. API가 요청의 20%를 처리 (품질이 중요한 생성, 복잡한 작업)
3. 효과적인 비용: 순수 API 사용 시 $5-15 대비 혼합 기준 100만 토큰당 약 $0.50-2.00

이 하이브리드 접근법이 건전한 마진으로 서비스를 구축하는 방법입니다. 모듈 R에서 더 자세히 다룹니다.

### 프로덕션 구성

수입 업무를 위한 Ollama 운영은 개인 채팅용과 다릅니다. 올바르게 구성하는 방법을 알려드리겠습니다.

{? if computed.has_nvidia ?}
> **NVIDIA GPU 감지됨 ({= profile.gpu.model | fallback("unknown") =}).** Ollama가 자동으로 CUDA 가속을 사용합니다. NVIDIA 드라이버가 최신인지 확인하세요 — `nvidia-smi`를 실행하여 확인하세요. {= profile.gpu.vram | fallback("your") =} VRAM으로 최적의 성능을 위해, 아래의 `OLLAMA_MAX_LOADED_MODELS` 설정은 VRAM에 동시에 들어가는 모델 수와 일치해야 합니다.
{? endif ?}

#### 환경 변수 설정

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

#### 워크로드에 맞는 Modelfile 생성

기본 모델 설정을 사용하는 대신, 수입 워크로드에 최적화된 커스텀 Modelfile을 생성하세요:

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

#### 배칭 및 큐 관리

수입 워크로드에서는 많은 항목을 처리해야 할 때가 많습니다. 기본적인 배칭 설정입니다:

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

### 나만의 장비 벤치마크

다른 사람의 벤치마크를 신뢰하지 마세요. 직접 측정하세요:

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

각 모델의 토큰/초를 적어두세요. 이 숫자가 여러분의 장비에서 어떤 수입 워크플로가 실용적인지 결정합니다.

{@ insight stack_fit @}

**사용 사례별 속도 요구사항:**
- 배치 처리 (비동기): 5+ 토큰/초면 충분합니다 (지연 시간은 신경 쓰지 않아도 됩니다)
- 인터랙티브 도구 (사용자가 기다림): 최소 20+ 토큰/초
- 실시간 API (고객 대면): 좋은 UX를 위해 30+ 토큰/초
- 스트리밍 채팅: 15+ 토큰/초면 반응이 빠르게 느껴집니다

### 로컬 인퍼런스 서버 보안

{? if computed.os_family == "windows" ?}
> **Windows 참고:** Windows에서 Ollama는 기본적으로 localhost에 바인딩됩니다. PowerShell에서 `netstat -an | findstr 11434`로 확인하세요. Windows 방화벽을 사용하여 포트 11434에 대한 외부 접근을 차단하세요.
{? elif computed.os_family == "macos" ?}
> **macOS 참고:** macOS에서 Ollama는 기본적으로 localhost에 바인딩됩니다. `lsof -i :11434`로 확인하세요. macOS 방화벽이 자동으로 외부 연결을 차단합니다.
{? endif ?}

Ollama 인스턴스는 명시적으로 의도하지 않는 한 인터넷에서 접근할 수 없어야 합니다.

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

> **흔한 실수:** "편의를 위해" Ollama를 0.0.0.0에 바인딩하고 잊어버리는 것. 여러분의 IP를 찾는 사람은 누구나 GPU를 무료 인퍼런스에 사용할 수 있습니다. 더 나쁜 것은 모델 가중치와 시스템 프롬프트를 추출할 수 있다는 것입니다. 항상 localhost. 항상 터널링.

### 레슨 2 체크포인트

지금까지 다음을 갖추셨어야 합니다:
- [ ] Ollama 설치 및 실행 중
- [ ] 최소 하나의 주력 모델 다운로드 완료 (llama3.1:8b 또는 동급)
- [ ] 예상 워크로드에 맞는 커스텀 Modelfile
- [ ] 벤치마크 수치: 장비에서의 각 모델별 토큰/초
- [ ] Ollama가 localhost에만 바인딩됨

*전체 STREETS 코스에서 모듈 T (기술적 해자)는 경쟁자가 쉽게 복제할 수 없는 독점적 모델 구성, 파인튜닝 파이프라인, 커스텀 툴체인을 구축하는 방법을 보여줍니다. 모듈 R (수익 엔진)은 이 스택 위에 구축할 정확한 서비스를 제공합니다.*

---

## 레슨 3: 프라이버시 이점

*"여러분의 프라이빗 환경은 단순한 선호가 아닌 경쟁 우위입니다."*

### 프라이버시는 제한이 아닌 제품 기능입니다

대부분의 개발자들은 개인적으로 프라이버시를 중시하거나 환경 구성을 즐기기 때문에 로컬 인프라를 설정합니다. 괜찮습니다. 하지만 **프라이버시가 지금 기술 업계에서 가장 마케팅 가능한 기능 중 하나라는 것**을 깨닫지 못하면 돈을 테이블 위에 남겨두는 것입니다.

이유는 이렇습니다: 기업이 OpenAI API에 데이터를 보낼 때마다 그 데이터는 제3자를 거칩니다. 많은 기업들 — 특히 의료, 금융, 법률, 정부, EU 소재 기업들에게 — 이것은 실질적인 문제입니다. 이론적인 것이 아닙니다. "컴플라이언스팀이 안 된다고 해서 이 도구를 사용할 수 없다"는 문제입니다.

여러분은 자신의 기계에서 모델을 로컬로 돌리기 때문에 이 문제가 없습니다.

### 규제 순풍

규제 환경이 여러분에게 유리한 방향으로 빠르게 이동하고 있습니다.

{? if regional.country == "US" ?}
> **미국 기반:** 여러분에게 가장 중요한 규제는 HIPAA, SOC 2, ITAR, 그리고 주별 프라이버시법 (캘리포니아 CCPA 등)입니다. EU 규제도 중요합니다 — 수익성 높은 시장인 유럽 고객을 서비스할 수 있는 능력에 영향을 미칩니다.
{? elif regional.country == "GB" ?}
> **영국 기반:** 브렉시트 이후 영국은 자체 데이터 보호 프레임워크(UK GDPR + Data Protection Act 2018)를 갖추고 있습니다. 로컬 처리 이점은 영국 금융 서비스 및 NHS 관련 업무를 서비스하는 데 특히 강력합니다.
{? elif regional.country == "DE" ?}
> **독일 기반:** 세계에서 가장 엄격한 데이터 보호 환경 중 하나에 계십니다. 이것은 *장점*입니다 — 독일 고객들은 이미 로컬 처리가 왜 중요한지 이해하고 있으며, 그에 대해 기꺼이 비용을 지불합니다.
{? elif regional.country == "AU" ?}
> **호주 기반:** Privacy Act 1988과 호주 프라이버시 원칙(APPs)이 여러분의 의무를 규정합니다. 로컬 처리는 My Health Records Act 하의 정부 및 의료 고객에게 강력한 판매 포인트입니다.
{? endif ?}

**EU AI Act (2024-2026년 시행):**
- 고위험 AI 시스템은 문서화된 데이터 처리 파이프라인이 필요합니다
- 기업은 데이터가 어디로 흐르고 누가 처리하는지 입증해야 합니다
- 로컬 처리는 컴플라이언스를 극적으로 단순화합니다
- EU 기업들은 EU 데이터 상주를 보장할 수 있는 AI 서비스 제공자를 적극적으로 찾고 있습니다

**GDPR (이미 시행 중):**
- "데이터 처리"에는 텍스트를 LLM API로 보내는 것이 포함됩니다
- 기업은 모든 제3자와 데이터 처리 계약(DPA)이 필요합니다
- 로컬 처리는 제3자를 완전히 제거합니다
- 이것은 실질적인 판매 포인트입니다: "여러분의 데이터는 절대 인프라를 떠나지 않습니다. 협상할 제3자 DPA가 없습니다."

**산업별 규제:**
- **HIPAA (미국 의료):** 환자 데이터는 BAA(Business Associate Agreement) 없이 소비자 AI API로 보낼 수 없습니다. 대부분의 AI 제공자는 API 접근에 대한 BAA를 제공하지 않습니다. 로컬 처리는 이것을 완전히 우회합니다.
- **SOC 2 (엔터프라이즈):** SOC 2 감사를 받는 기업은 모든 데이터 처리자를 문서화해야 합니다. 처리자가 적을수록 감사가 쉬워집니다.
- **ITAR (미국 방위):** 통제 기술 데이터는 미국 관할권을 떠날 수 없습니다. 국제 인프라를 가진 클라우드 AI 제공자는 문제가 됩니다.
- **PCI DSS (금융):** 카드 소유자 데이터 처리는 데이터가 이동하는 경로에 대한 엄격한 요구사항이 있습니다.

### 영업 대화에서 프라이버시를 포지셔닝하는 방법

컴플라이언스 전문가가 될 필요는 없습니다. 세 가지 문구를 이해하고 언제 사용할지 알면 됩니다:

**문구 1: "여러분의 데이터는 절대 인프라를 떠나지 않습니다."**
사용 시점: 프라이버시를 중시하는 잠재 고객과 대화할 때. 보편적인 훅입니다.

**문구 2: "제3자 데이터 처리 계약이 필요 없습니다."**
사용 시점: 유럽 기업이나 법무/컴플라이언스 팀이 있는 기업과 대화할 때. 이것으로 몇 주간의 법률 검토를 절약할 수 있습니다.

**문구 3: "완전한 감사 추적, 싱글 테넌트 처리."**
사용 시점: 엔터프라이즈 또는 규제 산업과 대화할 때. 감사관에게 AI 파이프라인을 증명해야 합니다.

**포지셔닝 예시 (서비스 페이지나 제안서용):**

> "클라우드 기반 AI 서비스와 달리, [서비스명]은 모든 데이터를 전용 하드웨어에서 로컬로 처리합니다. 여러분의 문서, 코드, 데이터는 처리 환경을 절대 떠나지 않습니다. 파이프라인에 제3자 API가 없으며, 협상할 데이터 공유 계약이 없고, 모든 작업에 대한 완전한 감사 로깅이 있습니다. 이것은 [서비스명]을 GDPR, HIPAA, SOC 2 컴플라이언스 환경을 포함하여 엄격한 데이터 처리 요구사항을 가진 조직에 적합하게 만듭니다."

이 문단을 랜딩 페이지에 올리면, 프리미엄 요금을 지불할 바로 그 고객들이 찾아올 것입니다.

### 프리미엄 가격 정당화

구체적인 숫자로 비즈니스 케이스를 보여드리겠습니다:

**표준 AI 처리 서비스 (클라우드 API 사용):**
- 고객의 데이터가 OpenAI/Anthropic/Google로 갑니다
- API를 호출할 수 있는 모든 개발자와 경쟁합니다
- 시장 요금: 문서당 $0.01-0.05
- 본질적으로 마크업을 붙여 API 접근을 재판매하는 것입니다

**프라이버시 우선 AI 처리 서비스 (여러분의 로컬 스택):**
- 고객의 데이터가 여러분의 기계에 머무릅니다
- 훨씬 작은 공급자 풀과 경쟁합니다
- 시장 요금: 문서당 $0.10-0.50 (5-10배 프리미엄)
- 인프라 + 전문성 + 컴플라이언스를 판매합니다

프라이버시 프리미엄은 실재합니다: 동일한 기본 작업에 대해 일반 클라우드 기반 서비스 대비 **5배에서 10배**. 그리고 이 비용을 지불하는 고객은 더 충성도가 높고, 가격에 덜 민감하며, 더 큰 예산을 가지고 있습니다.

{@ insight competitive_position @}

### 격리된 워크스페이스 설정

직장이 있다면 (대부분 그렇겠지만), 고용주 업무와 수입 업무 사이에 깔끔한 분리가 필요합니다. 이것은 단순한 법적 보호가 아닙니다 — 운영 위생입니다.

{? if computed.os_family == "windows" ?}
> **Windows 팁:** 수입 업무를 위한 별도의 Windows 사용자 계정을 만드세요 (설정 > 계정 > 가족 및 기타 사용자 > 다른 사용자 추가). 이렇게 하면 완전히 격리된 환경을 얻을 수 있습니다 — 별도의 브라우저 프로필, 별도의 파일 경로, 별도의 환경 변수. Win+L로 계정을 전환하세요.
{? endif ?}

**옵션 1: 별도의 사용자 계정 (권장)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**옵션 2: 컨테이너화된 워크스페이스**

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

**옵션 3: 별도의 물리적 기계 (가장 확실한 방법)**

이 일에 진지하고 수입이 정당화한다면, 전용 기계가 모든 의문을 제거합니다. RTX 3060이 장착된 중고 Dell OptiPlex는 $400-600이며 첫 달 고객 작업으로 본전을 뽑을 수 있습니다.

**최소 분리 체크리스트:**
- [ ] 수입 프로젝트를 별도 디렉토리에 보관 (고용주 레포와 절대 섞지 않기)
- [ ] 수입 업무용 별도 API 키 사용 (고용주가 제공한 키는 절대 사용하지 않기)
- [ ] 수입 관련 계정을 위한 별도 브라우저 프로필
- [ ] 수입 업무를 고용주 하드웨어에서 절대 하지 않기
- [ ] 수입 업무를 고용주 네트워크에서 절대 하지 않기 (개인 인터넷이나 VPN 사용)
- [ ] 수입 프로젝트를 위한 별도 GitHub/GitLab 계정 (선택사항이지만 깔끔함)

> **흔한 실수:** 사이드 프로젝트에서 "그냥 테스트용으로" 고용주의 OpenAI API 키를 사용하는 것. 이것은 고용주의 결제 대시보드에서 볼 수 있는 기록을 남기며, IP 문제를 복잡하게 만듭니다. 나만의 키를 받으세요. 비싸지 않습니다.

### 레슨 3 체크포인트

지금까지 다음을 이해하셨어야 합니다:
- [ ] 프라이버시가 단순한 개인적 선호가 아닌 마케팅 가능한 제품 기능인 이유
- [ ] 어떤 규제가 로컬 AI 처리에 대한 수요를 만드는지
- [ ] 프라이버시에 대한 영업 대화에서 사용할 세 가지 문구
- [ ] 프라이버시 우선 서비스가 5-10배 프리미엄 가격을 받는 방법
- [ ] 수입 업무와 고용주 업무를 분리하는 방법

*전체 STREETS 코스에서 모듈 E (진화하는 엣지)는 규제 변화를 추적하고 경쟁자가 존재조차 알기 전에 새로운 컴플라이언스 요구사항에 앞서 포지셔닝하는 방법을 가르쳐줍니다.*

---

## 레슨 4: 법적 최소 요건

*"지금 15분의 법적 설정이 나중에 몇 달의 문제를 예방합니다."*

### 이것은 법률 자문이 아닙니다

저는 개발자이지 변호사가 아닙니다. 아래 내용은 대부분의 상황에서 대부분의 개발자가 다뤄야 할 실용적인 체크리스트입니다. 상황이 복잡한 경우 (고용주 지분, 구체적인 조건이 있는 경업금지 등), 고용 전문 변호사와 30분 상담에 $200을 쓰세요. 가장 좋은 ROI를 얻을 수 있습니다.

### 1단계: 고용 계약서 읽기

고용 계약서 또는 오퍼 레터를 찾으세요. 다음 섹션을 찾으세요:

**지식재산권 양도 조항** — 다음과 같은 문구를 찾으세요:
- "모든 발명, 개발물, 작업 산출물..."
- "...고용 기간 동안 생성된..."
- "...회사의 사업 또는 예정된 사업과 관련된..."

**여러분을 제한하는 핵심 문구:**
- "고용 기간 동안 생성된 모든 작업 산출물은 회사에 귀속된다" (광범위 — 잠재적으로 문제가 됨)
- "회사 자원을 사용하여 생성된 작업 산출물" (더 좁음 — 보통 자기 장비를 사용하면 괜찮음)
- "회사의 현재 또는 예정된 사업과 관련된" (고용주가 무엇을 하는지에 따라 다름)

**여러분을 자유롭게 하는 핵심 문구:**
- "직원의 자체 시간에 직원의 자체 자원으로 회사 사업과 무관하게 수행된 업무를 제외한다" (이것이 예외 조항입니다 — 많은 미국 주에서 이것을 요구합니다)
- 일부 주 (캘리포니아, 워싱턴, 미네소타, 일리노이 등)는 계약서에 무엇이 쓰여 있든 개인 프로젝트에 대한 고용주의 IP 주장을 제한하는 법률이 있습니다.

### 3가지 질문 테스트

모든 수입 프로젝트에 대해 물어보세요:

1. **시간:** 이 업무를 자신의 시간에 하고 있나요? (근무 시간이 아닌, 온콜 근무 시간이 아닌)
2. **장비:** 자신의 하드웨어, 자신의 인터넷, 자신의 API 키를 사용하고 있나요? (고용주 노트북이 아닌, 고용주 VPN이 아닌, 고용주 클라우드 계정이 아닌)
3. **주제:** 이것이 고용주의 사업과 무관한가요? (의료 AI 회사에서 일하면서 의료 AI 서비스를 판매하고 싶다면... 문제가 됩니다. 의료 AI 회사에서 일하면서 부동산 중개인을 위한 문서 처리를 판매하고 싶다면... 괜찮습니다.)

세 가지 답이 모두 깨끗하다면, 거의 확실히 괜찮습니다. 어떤 답이 모호하다면, 진행하기 전에 명확히 하세요.

> **솔직한 이야기:** 사이드 업무를 하는 개발자 대다수는 문제가 생기지 않습니다. 고용주는 경쟁 우위를 보호하는 데 관심이 있지, 무관한 프로젝트로 추가 수입을 올리는 것을 막는 데 관심이 없습니다. 하지만 "거의 확실히 괜찮다"는 "확실히 괜찮다"가 아닙니다. 계약서가 비정상적으로 광범위하다면, 매니저나 HR과 대화하세요 — 또는 변호사에게 상담하세요. 확인하지 않는 것의 불이익이 물어보는 약간의 어색함보다 훨씬 큽니다.

### 2단계: 사업체 유형 선택

개인 자산과 사업 활동을 분리하고, 사업용 은행 계좌, 결제 처리, 세금 혜택의 문을 열기 위해 법인이 필요합니다.

{? if regional.country ?}
> **여러분의 위치: {= regional.country | fallback("Unknown") =}.** 여러분의 지역에 권장되는 법인 유형은 **{= regional.business_entity_type | fallback("LLC or equivalent") =}**이며, 일반적인 등록 비용은 {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}입니다. 아래에서 여러분의 국가 섹션으로 스크롤하거나, 다른 지역의 고객이 어떻게 운영하는지 이해하기 위해 모든 섹션을 읽으세요.
{? endif ?}

{? if regional.country == "US" ?}
#### 미국 (여러분의 지역)
{? else ?}
#### 미국
{? endif ?}

| 구조 | 비용 | 보호 | 적합한 경우 |
|------|------|------|-----------|
| **개인사업 (Sole Proprietorship)** (기본) | $0 | 없음 (개인 책임) | 시험적으로 시작. 첫 $1K. |
| **단일 회원 LLC** | $50-500 (주마다 다름) | 개인 자산 보호 | 적극적인 수입 활동. 대부분의 개발자는 여기서 시작해야 합니다. |
| **S-Corp 선택** (LLC에 대해) | LLC 비용 + 선택 $0 | LLC와 동일 + 급여세 혜택 | 연간 $40K 이상을 꾸준히 벌 때 |

**미국 개발자 권장:** 거주 주의 단일 회원 LLC.

**가장 저렴한 주:** 와이오밍 ($100, 주 소득세 없음), 뉴멕시코 ($50), 몬태나 ($70). 하지만 특별한 이유가 없다면 거주 주에 설립하는 것이 가장 간단합니다.

**신청 방법:**
1. 해당 주의 국무장관(Secretary of State) 웹사이트를 방문하세요
2. "form LLC" 또는 "business entity filing"을 검색하세요
3. 조직 설립 증서(Articles of Organization)를 제출하세요 (10분 양식)
4. IRS에서 EIN을 받으세요 (무료, irs.gov에서 5분 소요)

{? if regional.country == "GB" ?}
#### 영국 (여러분의 지역)
{? else ?}
#### 영국
{? endif ?}

| 구조 | 비용 | 보호 | 적합한 경우 |
|------|------|------|-----------|
| **개인사업자 (Sole Trader)** | 무료 (HMRC 등록) | 없음 | 첫 수입. 테스트. |
| **유한회사 (Ltd)** | Companies House를 통해 약 $15 | 개인 자산 보호 | 진지한 수입 활동. |

**권장:** Companies House를 통한 Ltd 회사. 약 20분이 소요되며 비용은 GBP 12입니다.

#### 유럽연합

국가마다 크게 다르지만, 일반적인 패턴:

- **독일:** 시작은 Einzelunternehmer(개인사업자), 진지한 업무에는 GmbH (하지만 GmbH는 EUR 25,000 자본 필요 — EUR 1로 UG 고려)
- **네덜란드:** Eenmanszaak (개인사업자, 등록 무료) 또는 BV (Ltd에 해당)
- **프랑스:** Micro-entrepreneur (간소화, 시작에 권장)
- **에스토니아:** e-Residency + OUE (비거주자에게 인기, 완전 온라인)

{? if regional.country == "AU" ?}
#### 호주 (여러분의 지역)
{? else ?}
#### 호주
{? endif ?}

| 구조 | 비용 | 보호 | 적합한 경우 |
|------|------|------|-----------|
| **개인사업자 (Sole Trader)** | ABN 무료 | 없음 | 시작 단계 |
| **Pty Ltd** | ASIC를 통해 약 AUD 500-800 | 개인 자산 보호 | 진지한 수입 |

**권장:** 개인사업자 ABN으로 시작 (무료, 즉시 발급), 꾸준히 벌기 시작하면 Pty Ltd로 전환.

### 3단계: 결제 처리 (15분 설정)

지불받을 수 있는 방법이 필요합니다. 첫 고객이 기다리고 있을 때가 아닌 지금 설정하세요.

{? if regional.payment_processors ?}
> **{= regional.country | fallback("your region") =} 권장:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (대부분의 개발자에게 권장):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

소요 시간: 약 15분. 즉시 결제를 수락할 수 있습니다 (Stripe은 신규 계정의 자금을 7일간 보유합니다).

**Lemon Squeezy (디지털 제품에 권장):**

디지털 제품 (템플릿, 도구, 코스, SaaS)을 판매하는 경우, Lemon Squeezy가 판매자 대행(Merchant of Record) 역할을 합니다. 이것은 다음을 의미합니다:
- 전 세계적으로 판매세, VAT, GST를 대신 처리해줍니다
- EU에서 VAT 등록을 할 필요가 없습니다
- 환불과 분쟁을 처리해줍니다

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (해외 개발자 또는 미국 법인을 원하는 경우):**

미국 외에 있지만 미국 법인으로 미국 고객에게 판매하고 싶다면:
- $500 일회성 수수료
- 델라웨어 LLC를 만들어줍니다
- 미국 은행 계좌를 설정해줍니다 (Mercury 또는 Stripe을 통해)
- 등록 대리인 서비스를 제공합니다
- 약 1-2주 소요

### 4단계: 개인정보 보호정책 및 이용약관

온라인으로 서비스나 제품을 판매한다면 이것들이 필요합니다. 기본 양식에 변호사 비용을 쓰지 마세요.

**무료, 신뢰할 수 있는 템플릿 소스:**
- **Termly.io** — 무료 개인정보 보호정책 및 이용약관 생성기. 질문에 답하면 문서를 받습니다.
- **Avodocs.com** — 스타트업을 위한 오픈소스 법률 문서. 무료.
- **GitHub의 choosealicense.com** — 특히 오픈소스 프로젝트 라이선스용.
- **Basecamp의 오픈소스 정책** — "Basecamp open source policies"를 검색하세요 — 좋고 이해하기 쉬운 영어 템플릿.

**개인정보 보호정책에 반드시 포함해야 하는 내용 (고객 데이터를 처리하는 경우):**
- 어떤 데이터를 수집하는지
- 어떻게 처리하는지 (로컬로 — 이것이 여러분의 장점입니다)
- 얼마나 오래 보관하는지
- 고객이 삭제를 요청하는 방법
- 제3자가 데이터에 접근하는지 (이상적으로: 없음)

**소요 시간:** 템플릿 생성기로 30분. 완료.

### 5단계: 별도 은행 계좌

사업 수입을 개인 체크 계좌로 처리하지 마세요. 이유:

1. **세금 명확성:** 세금 시즌이 되면, 사업 수입이 정확히 무엇이었는지 알아야 합니다.
2. **법적 보호:** LLC가 있다면, 개인 자금과 사업 자금을 혼합하면 "법인격 부인(pierce the corporate veil)"이 될 수 있습니다 — 법원이 LLC의 책임 보호를 무시할 수 있다는 뜻입니다.
3. **전문성:** "John's Consulting LLC"에서 발행한 인보이스가 전용 사업 계좌로 들어오면 합법적으로 보입니다. 개인 Venmo로의 결제는 그렇지 않습니다.

**무료 또는 저비용 사업용 은행:**
{? if regional.country == "US" ?}
- **Mercury** (여러분에게 권장) — 무료, 스타트업을 위해 설계됨. 나중에 회계를 자동화하고 싶다면 훌륭한 API.
- **Relay** — 무료, 수입 스트림을 서브 계좌로 분리하는 데 좋음.
{? elif regional.country == "GB" ?}
- **Starling Bank** (여러분에게 권장) — 무료 사업 계좌, 즉시 설정.
- **Wise Business** — 저비용 다중 통화. 국제 고객을 서비스한다면 좋음.
{? else ?}
- **Mercury** (미국) — 무료, 스타트업을 위해 설계됨. 나중에 회계를 자동화하고 싶다면 훌륭한 API.
- **Relay** (미국) — 무료, 수입 스트림을 서브 계좌로 분리하는 데 좋음.
- **Starling Bank** (영국) — 무료 사업 계좌.
{? endif ?}
- **Wise Business** (국제) — 저비용 다중 통화. USD, EUR, GBP 등으로 결제를 받는 데 좋음.
- **Qonto** (EU) — 유럽 기업을 위한 깔끔한 사업 은행.

지금 계좌를 개설하세요. 온라인으로 10-15분이 소요되며 확인까지 1-3일이 걸립니다.

### 6단계: 개발자 부업 소득의 세금 기초

{? if regional.tax_note ?}
> **{= regional.country | fallback("your region") =}의 세금 참고:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **솔직한 이야기:** 세금은 대부분의 개발자가 4월까지 무시하다가 패닉에 빠지는 것입니다. 지금 30분을 투자하면 실제로 돈과 스트레스를 절약합니다.

**미국:**
- 연간 $400 이상의 부업 소득은 자영업세가 필요합니다 (사회보장 + 메디케어 약 15.3%)
- 순이익에 대한 일반 소득세 구간도 추가
- **분기별 예상 세금:** 세금이 $1,000 이상이 될 것으로 예상되면, IRS는 분기별 납부를 기대합니다 (4월 15일, 6월 15일, 9월 15일, 1월 15일). 과소 납부는 벌금을 유발합니다.
- 순수입의 **25-30%**를 세금으로 별도 적립하세요. 즉시 별도 저축 계좌에 넣으세요.

**일반적인 공제 항목:**
- API 비용 (OpenAI, Anthropic 등) — 100% 공제 가능
- 사업용 하드웨어 구매 — 감가상각 또는 Section 179 공제
- 사업용 전기 비용
- 수입 업무에 사용하는 소프트웨어 구독
- 홈 오피스 공제 (간소화: $5/sq ft, 최대 300 sq ft = $1,500)
- 인터넷 (사업 사용 비율)
- 도메인명, 호스팅, 이메일 서비스
- 수입 업무 관련 전문성 개발 (코스, 서적)

**영국:**
- Self Assessment 세금 신고서로 보고
- GBP 1,000 이하 거래 수입: 비과세 (Trading Allowance)
- 그 이상: 이익에 대해 소득세 + Class 4 NIC 납부
- 납부일: 1월 31일 및 7월 31일

**첫날부터 모든 것을 기록하세요.** 다른 것이 없다면 간단한 스프레드시트라도 사용하세요:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **흔한 실수:** "세금은 나중에 알아보자." 나중은 Q4이고, 예상 세금 $3,000에 벌금까지 붙어 있으며, 돈은 이미 써버린 상태입니다. 자동화하세요: 사업 계좌에 수입이 들어올 때마다 즉시 30%를 세금 저축 계좌로 이체하세요.

### 레슨 4 체크포인트

지금까지 다음을 갖추셨어야 합니다 (또는 계획이 있어야 합니다):
- [ ] 고용 계약서의 IP 조항을 읽었음
- [ ] 계획된 수입 업무에 대해 3가지 질문 테스트를 통과
- [ ] 사업체 유형 선택 (또는 개인사업자로 시작하기로 결정)
- [ ] 결제 처리 설정 완료 (Stripe 또는 Lemon Squeezy)
- [ ] 템플릿 생성기에서 개인정보 보호정책 및 이용약관
- [ ] 별도 사업용 은행 계좌 (또는 신청 완료)
- [ ] 세금 전략: 30% 별도 적립 + 분기별 납부 일정

*전체 STREETS 코스에서 모듈 E (실행 플레이북)는 각 수익 엔진의 세금 의무, 프로젝트 수익성, 손익분기점을 자동으로 계산하는 재무 모델링 템플릿을 포함합니다.*

---

## 레슨 5: 월 {= regional.currency_symbol | fallback("$") =}200 예산

*"여러분의 사업에는 소진 비율(burn rate)이 있습니다. 파악하세요. 통제하세요. 벌게 만드세요."*

### 왜 월 {= regional.currency_symbol | fallback("$") =}200인가

월 200{= regional.currency | fallback("dollars") =}은 개발자 수입 운영을 위한 최소 실행 가능 예산입니다. 실제 서비스를 운영하고, 실제 고객을 응대하고, 실제 수익을 창출하기에 충분합니다. 또한 아무것도 작동하지 않더라도 전 재산을 걸지 않을 만큼 적습니다.

목표는 간단합니다: **월 {= regional.currency_symbol | fallback("$") =}200을 90일 이내에 월 {= regional.currency_symbol | fallback("$") =}600 이상으로 바꾸는 것.** 그렇게 할 수 있다면 사업이 있는 것입니다. 그렇게 할 수 없다면 전략을 바꾸세요 — 예산을 늘리는 것이 아닙니다.

### 예산 배분

#### 1등급: API 크레딧 — 월 $50-100

고객 대면 품질을 위한 프로덕션 컴퓨팅입니다.

**권장 초기 배분:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**API 지출 관리 방법:**

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

**하이브리드 지출 전략:**
- 로컬 LLM이 처리의 80%를 담당 (분류, 추출, 요약, 초안)
- API 호출이 처리의 20%를 담당 (최종 품질 검수, 복잡한 추론, 고객 대면 출력)
- 순수 API 사용 대비 작업당 효과적인 비용이 극적으로 감소

{? if computed.monthly_electricity_estimate ?}
> **예상 전기 비용:** {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh 기준 24/7 가동 시 월 {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}. 이것은 이미 효과적인 운영 비용에 포함되어 있습니다.
{? endif ?}

#### 2등급: 인프라 — 월 {= regional.currency_symbol | fallback("$") =}30-50

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**VPS가 필요한가요?**

수입 모델이 다음과 같다면:
- **디지털 제품 판매:** 아니요. Vercel/Netlify에서 무료로 호스팅하세요. 전달은 Lemon Squeezy를 사용하세요.
- **고객을 위한 비동기 처리 운영:** 아마도. 로컬 장비에서 작업을 실행하고 결과를 전달할 수 있습니다. VPS는 안정성을 추가합니다.
- **API 서비스 제공:** 네, 아마도. $5-10 VPS가 경량 API 게이트웨이 역할을 하며, 무거운 처리는 로컬 기계에서 이루어집니다.
- **SaaS 판매:** 네. 하지만 가장 저렴한 등급부터 시작하고 확장하세요.

**권장 스타터 인프라:**

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

총 인프라 비용: 월 $5-20. 나머지는 무료 등급입니다.

#### 3등급: 도구 — 월 {= regional.currency_symbol | fallback("$") =}20-30

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

> **솔직한 이야기:** 시작할 때 전체 도구 스택을 무료 등급으로 운영할 수 있습니다. 여기에 배정된 $20-30은 무료 등급을 초과하거나 특정 프리미엄 기능이 필요할 때를 위한 것입니다. 예산에 있다고 해서 쓰지 마세요. 쓰지 않은 예산은 이익입니다.

#### 4등급: 예비 — 월 {= regional.currency_symbol | fallback("$") =}0-30

"예상치 못한 일" 기금입니다:
- 예상치 못하게 큰 배치 작업으로 인한 API 비용 급증
- 특정 고객 프로젝트에 필요한 도구
- 완벽한 이름을 찾았을 때의 긴급 도메인 구매
- 일회성 구매 (테마, 템플릿, 아이콘 세트)

예비금을 사용하지 않으면 누적됩니다. 3개월간 사용하지 않은 예비금은 API 크레딧이나 인프라로 재배분을 고려하세요.

### ROI 계산

중요한 숫자는 하나뿐입니다:

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

**예산을 늘려야 할 때:**

다음 조건을 모두 충족할 때만 예산을 늘리세요:
1. 2개월 이상 꾸준히 2배 이상 ROI를 달성
2. 추가 지출이 직접적으로 수익을 증가시킬 때 (예: API 크레딧 추가 = 고객 처리 용량 증가)
3. 증가가 구체적이고 검증된 수익 스트림에 연결됨

**예산을 늘리지 말아야 할 때:**
- "이 새로운 도구가 도움이 될 것 같아" (먼저 무료 대안을 테스트하세요)
- "돈을 벌려면 돈을 써야 한다고 다들 말해" (이 단계에서는 아닙니다)
- "더 큰 VPS가 서비스를 더 빠르게 만들 거야" (속도가 정말 병목인가요?)
- 아직 1배 ROI에 도달하지 못했을 때 (수익을 고치세요, 지출이 아니라)

**확장 사다리:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **흔한 실수:** 월 {= regional.currency_symbol | fallback("$") =}200을 즉시 수익을 돌려받을 필요가 없는 "투자"로 취급하는 것. 아닙니다. 이것은 90일 기한이 있는 실험입니다. 월 {= regional.currency_symbol | fallback("$") =}200이 90일 이내에 월 {= regional.currency_symbol | fallback("$") =}200의 수익을 창출하지 못한다면, 전략의 무언가가 바뀌어야 합니다. 돈, 시장, 제안 — 무언가가 작동하지 않는 것입니다. 자신에게 솔직하세요.

### 레슨 5 체크포인트

지금까지 다음을 갖추셨어야 합니다:
- [ ] 4개 등급에 걸쳐 약 $200의 월간 예산 배분
- [ ] API 계정 생성 및 지출 한도 설정
- [ ] 인프라 결정 완료 (로컬 전용 vs. 로컬 + VPS)
- [ ] 도구 스택 선택 (시작은 대부분 무료 등급)
- [ ] ROI 목표: 90일 이내 3배
- [ ] 명확한 규칙: ROI를 증명한 후에만 예산 증가

*전체 STREETS 코스에서 모듈 E (실행 플레이북)는 수익 엔진별 지출, 수익, ROI를 실시간으로 추적하는 재무 대시보드 템플릿을 포함합니다 — 어떤 스트림이 수익성이 있고 어떤 것이 조정이 필요한지 항상 알 수 있습니다.*

---

## 레슨 6: 소버린 스택 문서

*"모든 사업에는 계획이 있습니다. 이것이 여러분의 것이며, 두 페이지에 들어갑니다."*

### 결과물

이것이 모듈 S에서 만들 가장 중요한 것입니다. 소버린 스택 문서는 수입 창출 인프라에 대한 모든 것을 담는 단일 참고 문서입니다. STREETS 코스의 나머지 부분에서 이것을 참조하고, 셋업이 발전함에 따라 업데이트하고, 무엇을 구축하고 무엇을 건너뛸지에 대한 냉철한 결정을 내리는 데 사용할 것입니다.

새 파일을 만드세요. 마크다운, Google Doc, Notion 페이지, 일반 텍스트 — 실제로 유지할 수 있는 것이면 무엇이든. 아래 템플릿을 사용하여, 레슨 1-5에서 나온 숫자와 결정으로 모든 필드를 채우세요.

### 템플릿

{? if computed.profile_completeness != "0" ?}
> **헤드 스타트:** 4DA가 이미 일부 하드웨어 사양과 스택 정보를 감지했습니다. 아래의 미리 채워진 힌트를 찾아보세요 — 템플릿을 채우는 시간을 절약해줄 것입니다.
{? endif ?}

이 전체 템플릿을 복사하고 채우세요. 모든 필드. 건너뛰기 없이.

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
> **개발자 DNA에서 미리 채우기:**
> - **기본 스택:** {= dna.primary_stack | fallback("Not detected") =}
> - **관심사:** {= dna.interests | fallback("Not detected") =}
> - **아이덴티티 요약:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **주의할 맹점:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **감지된 스택에서 미리 채우기:** 여러분의 기본 기술은 {= stack.primary | fallback("not yet detected") =}입니다. {? if stack.adjacent ?}인접 기술: {= stack.adjacent | fallback("none detected") =}.{? endif ?} 이것을 위의 기술 인벤토리를 채우는 데 사용하세요.
{? endif ?}

{@ insight t_shape @}

### 이 문서 사용 방법

1. **새 프로젝트를 시작하기 전에:** 소버린 스택을 확인하세요. 실행할 하드웨어, 시간, 기술, 예산이 있나요?
2. **무언가를 구매하기 전에:** 예산 배분을 확인하세요. 이 구매가 계획에 있나요?
3. **월간 리뷰:** 예산의 "실제" 열을 업데이트하세요. 수익 숫자를 업데이트하세요. 작동하는 것에 기반하여 배분을 조정하세요.
4. **누군가가 무엇을 하는지 물을 때:** "오늘 제공할 수 있는 것" 섹션이 여러분의 즉시 피치입니다.
5. **새로운 번쩍이는 아이디어를 쫓고 싶을 때:** 제약 조건을 확인하세요. 이것이 시간, 기술, 하드웨어 내에 맞나요? 안 맞으면 "구축 목표"에 나중을 위해 추가하세요.

### 1시간 연습

60분 타이머를 설정하세요. 템플릿의 모든 필드를 채우세요. 너무 깊이 생각하지 마세요. 광범위하게 조사하지 마세요. 지금 아는 것을 쓰세요. 나중에 업데이트할 수 있습니다.

채울 수 없는 필드? 그것이 이번 주의 액션 아이템입니다:
- 빈 벤치마크 숫자? 레슨 2의 벤치마크 스크립트를 실행하세요.
- 사업체가 없다? 레슨 4의 등록 프로세스를 시작하세요.
- 결제 처리가 없다? 레슨 4에서 Stripe을 설정하세요.
- 빈 기술 인벤토리? 지난 5년간 돈을 받고 했던 모든 일을 15분간 나열하세요.

> **흔한 실수:** 1시간 안에 "완료"로 만드는 대신 3시간을 들여 문서를 "완벽하게" 만들려는 것. 소버린 스택 문서는 투자자를 위한 사업 계획이 아닌 작업 참고 문서입니다. 여러분 외에는 아무도 보지 않습니다. 정확성이 중요합니다. 형식은 중요하지 않습니다.

### 레슨 6 체크포인트

지금까지 다음을 갖추셨어야 합니다:
- [ ] 실제로 열어볼 곳에 저장된 완성된 소버린 스택 문서
- [ ] 실제 숫자로 채워진 6개 섹션 전부 (희망적인 숫자가 아닌)
- [ ] 셋업의 빈 부분에 대한 명확한 액션 아이템 목록
- [ ] 첫 월간 리뷰 날짜 설정 (지금부터 30일 후)

---

## 모듈 S: 완료

{? if progress.completed("MODULE_S") ?}
> **모듈 S 완료.** {= progress.total_count | fallback("7") =}개 STREETS 모듈 중 {= progress.completed_count | fallback("1") =}개를 완료했습니다. {? if progress.completed_modules ?}완료: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### 2주 동안 구축한 것

시작할 때 없었던 것 중 지금 가지고 있는 것을 보세요:

1. **하드웨어 인벤토리** — 스티커에 적힌 스펙이 아닌, 수입 창출 역량에 매핑된 것.
2. **프로덕션 수준의 로컬 LLM 스택** — Ollama 기반, 실제 하드웨어에서 벤치마크 완료, 실제 워크로드에 맞게 구성.
3. **프라이버시 이점** — 특정 대상에게 특정 언어로 마케팅하는 방법을 이해한 것.
4. **법적, 재정적 기반** — 사업체 (또는 계획), 결제 처리, 은행 계좌, 세금 전략.
5. **통제된 예산** — 명확한 ROI 목표와 모델을 증명할 90일 기한.
6. **소버린 스택 문서** — 위의 모든 것을 앞으로의 모든 결정에 사용할 단일 참고 문서에 담은 것.

이것은 대부분의 개발자가 결코 설정하지 않는 것보다 많습니다. 진심입니다. 부업 수입을 원하는 대부분의 사람들은 바로 "멋진 것을 만들자"로 건너뛰고 왜 돈을 받지 못하는지 궁금해합니다. 여러분은 이제 돈을 받을 수 있는 인프라를 갖추었습니다.

하지만 방향 없는 인프라는 비싼 취미일 뿐입니다. 이 스택을 어디에 겨눌지 알아야 합니다.

{@ temporal market_timing @}

### 다음 단계: 모듈 T — 기술적 해자

모듈 S는 기반을 마련했습니다. 모듈 T는 핵심 질문에 답합니다: **경쟁자가 쉽게 복제할 수 없는 것을 어떻게 구축하는가?**

모듈 T가 다루는 내용:

- **독점 데이터 파이프라인** — 합법적이고 윤리적으로, 여러분만 접근할 수 있는 데이터셋을 만드는 방법
- **커스텀 모델 구성** — 다른 사람이 기본 설정으로는 따라올 수 없는 출력 품질을 만드는 파인튜닝과 프롬프트 엔지니어링
- **복리 기술 스택** — "Python + 의료"가 수입 면에서 "Python + JavaScript"를 이기는 이유, 그리고 여러분만의 고유한 조합을 식별하는 방법
- **기술적 진입 장벽** — 경쟁자가 복제하는 데 수개월이 걸리는 인프라 설계
- **해자 감사** — 여러분의 프로젝트에 방어 가능한 우위가 있는지 아니면 또 다른 커머디티 서비스일 뿐인지 평가하는 프레임워크

월 $500을 버는 개발자와 월 $5,000을 버는 개발자의 차이는 드물게 기술입니다. 해자입니다. 누군가가 같은 하드웨어와 같은 모델을 가지고 있어도 여러분의 제안을 복제하기 어렵게 만드는 것들.

### 전체 STREETS 로드맵

| 모듈 | 제목 | 집중 영역 | 기간 |
|------|------|----------|------|
| **S** | 소버린 셋업 | 인프라, 법적 기반, 예산 | 1-2주차 (완료) |
| **T** | 기술적 해자 | 방어 가능한 우위, 독점 자산 | 3-4주차 |
| **R** | 수익 엔진 | 코드를 포함한 구체적인 수익화 플레이북 | 5-8주차 |
| **E** | 실행 플레이북 | 런칭 시퀀스, 가격 책정, 첫 고객 | 9-10주차 |
| **E** | 진화하는 엣지 | 앞서가기, 트렌드 감지, 적응 | 11-12주차 |
| **T** | 전술적 자동화 | 패시브 인컴을 위한 운영 자동화 | 13-14주차 |
| **S** | 스트림 누적 | 복수 수입원, 포트폴리오 전략 | 15-16주차 |

모듈 R (수익 엔진)이 대부분의 수익이 만들어지는 곳입니다. 하지만 S와 T 없이는 모래 위에 짓는 것입니다.

---

**전체 플레이북이 준비되셨나요?**

기초를 보셨습니다. 직접 구축하셨습니다. 이제 완전한 시스템을 받으세요.

**STREETS Core를 받으세요** — 7개 모듈 전체, 수익 엔진 코드 템플릿, 재무 대시보드, 그리고 자기 주도적으로 수입을 만들어가는 개발자들의 프라이빗 커뮤니티가 포함된 16주 풀 코스입니다.
