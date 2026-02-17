# Module S: Sovereign Setup

**STREETS Developer Income Course — Free Module**
*Weeks 1-2 | 6 Lessons | Deliverable: Your Sovereign Stack Document*

> "Your rig is your business infrastructure. Configure it like one."

---

You already own the most powerful income-generating tool most people will never have: a developer workstation with an internet connection, local compute, and the skills to wire it all together.

Most developers treat their rig like a consumer product. Something they game on, code on, browse on. But that same machine — the one sitting under your desk right now — can run inference, serve APIs, process data, and generate revenue 24 hours a day while you sleep.

This module is about looking at what you already have through a different lens. Not "what can I build?" but "what can I sell?"

By the end of these two weeks, you will have:

- A clear inventory of your income-generating capabilities
- A production-grade local LLM stack
- A legal and financial foundation (even if minimal)
- A written Sovereign Stack Document that becomes your business blueprint

No hand-waving. No "just believe in yourself." Real numbers, real commands, real decisions.

Let's get started.

---

## Lesson 1: The Rig Audit

*"You don't need a 4090. Here's what actually matters."*

### Your Machine Is a Business Asset

When a company evaluates its infrastructure, it doesn't just list specs — it maps capabilities to revenue opportunities. That's what you're going to do right now.

Open a terminal and run through the following. Write down every number. You'll need them for your Sovereign Stack Document in Lesson 6.

### Hardware Inventory

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

**What matters for income:**
- Core count determines how many concurrent tasks your rig can handle. Running a local LLM while simultaneously processing a batch job requires real parallelism.
- For most revenue engines in this course, any modern 8+ core CPU from the last 5 years is sufficient.
- If you're running local LLMs on CPU only (no GPU), you want 16+ cores. A Ryzen 7 5800X or Intel i7-12700 is the practical floor.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**What matters for income:**
- 16 GB: Bare minimum. You can run 7B models and do basic automation work.
- 32 GB: Comfortable. Run 13B models locally, handle multiple projects, keep your dev environment running alongside income workloads.
- 64 GB+: You can run 30B+ models on CPU, or keep multiple models loaded. This is where things get interesting for selling inference services.

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

**What matters for income:**

This is the one spec people obsess over, and here's the honest truth: **your GPU determines your local LLM tier, and your local LLM tier determines which income streams run fastest.** But it doesn't determine whether you can make money at all.

| VRAM | LLM Capability | Income Relevance |
|------|---------------|------------------|
| 0 (CPU only) | 7B models at ~5 tokens/sec | Batch processing, async work. Slow but functional. |
| 6-8 GB (RTX 3060, etc.) | 7B models at ~30 tok/sec, 13B quantized | Good enough for most automation income streams. |
| 12 GB (RTX 3060 12GB, 4070) | 13B at full speed, 30B quantized | Sweet spot. Most revenue engines run well here. |
| 16-24 GB (RTX 4090, 3090) | 30B-70B models | Premium tier. Sell quality others can't match locally. |
| 48 GB+ (dual GPU, A6000) | 70B+ at speed | Enterprise-grade local inference. Serious competitive advantage. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ using unified memory | Excellent efficiency. Lower power cost than NVIDIA equivalent. |

> **Real Talk:** If you have an RTX 3060 12GB, you are in a better position than 95% of developers trying to monetize AI. Stop waiting for a 4090. The 3060 12GB is the Honda Civic of local AI — reliable, efficient, gets the job done. The money you'd spend on a GPU upgrade is better spent on API credits for customer-facing quality while your local models handle the grunt work.

#### Storage

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**What matters for income:**
- LLM models take space: 7B model = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (quantized).
- You need room for project data, databases, caches, and output artifacts.
- SSD is non-negotiable for anything customer-facing. Model loading from HDD adds 30-60 seconds of startup time.
- Minimum practical: 500 GB SSD with at least 100 GB free.
- Comfortable: 1 TB SSD. Keep models on the SSD, archive to HDD.

#### Network

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**What matters for income:**
- Download speed: 50+ Mbps. Needed for pulling models, packages, and data.
- Upload speed: This is the bottleneck most people ignore. If you're serving anything (APIs, processed results, deliverables), upload matters.
  - 10 Mbps: Adequate for async delivery (processed files, batch results).
  - 50+ Mbps: Required if you're running any kind of local API endpoint that external services hit.
  - 100+ Mbps: Comfortable for everything in this course.
- Latency: Under 50ms to major cloud providers. Run `ping api.openai.com` and `ping api.anthropic.com` to check.

#### Uptime

This is the spec nobody thinks about, but it separates hobbyists from people making money while they sleep.

Ask yourself:
- Can your rig run 24/7? (Power, cooling, noise)
- Do you have a UPS for power outages?
- Is your internet connection stable enough for automated workflows?
- Can you SSH into your machine remotely if something breaks?

If you can't run 24/7, that's fine — many income streams in this course are async batch jobs you trigger manually. But the ones that generate truly passive income require uptime.

**Quick uptime setup (if you want it):**

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

### The Electricity Math

People either ignore this or catastrophize it. Let's do real math.

**Measuring your actual power draw:**

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

**Monthly cost calculation:**

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

US average electricity is about $0.12/kWh. Check your actual rate — it varies wildly. California might be $0.25/kWh. Some European countries hit $0.35/kWh. Parts of the US Midwest are $0.08/kWh.

**The point:** Running your rig 24/7 for income costs somewhere between $1-$30/month in electricity. If your income streams can't cover that, the problem isn't electricity — it's the income stream.

### Minimum Specs by Revenue Engine Type

Here's a preview of where we're headed in the full STREETS course. For now, just check where your rig lands:

| Revenue Engine | CPU | RAM | GPU | Storage | Network |
|---------------|-----|-----|-----|---------|---------|
| **Content automation** (blog posts, newsletters) | 4+ cores | 16 GB | Optional (API fallback) | 50 GB free | 10 Mbps up |
| **Data processing services** | 8+ cores | 32 GB | Optional | 200 GB free | 50 Mbps up |
| **Local AI API services** | 8+ cores | 32 GB | 8+ GB VRAM | 100 GB free | 50 Mbps up |
| **Code generation tools** | 8+ cores | 16 GB | 8+ GB VRAM or API | 50 GB free | 10 Mbps up |
| **Document processing** | 4+ cores | 16 GB | Optional | 100 GB free | 10 Mbps up |
| **Autonomous agents** | 8+ cores | 32 GB | 12+ GB VRAM | 100 GB free | 50 Mbps up |

> **Common Mistake:** "I need to upgrade my hardware before I can start." No. Start with what you have. Use API calls to fill gaps your hardware can't cover. Upgrade when revenue justifies it — not before.

### Lesson 1 Checkpoint

You should now have written down:
- [ ] CPU model, cores, and threads
- [ ] RAM amount
- [ ] GPU model and VRAM (or "none")
- [ ] Available storage
- [ ] Network speeds (down/up)
- [ ] Estimated monthly electricity cost for 24/7 operation
- [ ] Which revenue engine categories your rig qualifies for

Keep these numbers. You'll plug them into your Sovereign Stack Document in Lesson 6.

*In the full STREETS course, Module R (Revenue Engines) gives you specific, step-by-step playbooks for each engine type listed above — including the exact code to build and deploy them.*

---

## Lesson 2: The Local LLM Stack

*"Set up Ollama for production use — not just chat."*

### Why Local LLMs Matter for Income

Every time you call the OpenAI API, you're paying rent. Every time you run a model locally, that inference is free after the initial setup. The math is simple:

- GPT-4o: ~$5 per million input tokens, ~$15 per million output tokens
- Claude 3.5 Sonnet: ~$3 per million input tokens, ~$15 per million output tokens
- Local Llama 3.1 8B: $0 per million tokens (just electricity)

If you're building services that process thousands of requests, the difference between $0 and $5-$15 per million tokens is the difference between profit and break-even.

But here's the nuance most people miss: **local and API models serve different roles in an income stack.** Local models handle volume. API models handle quality-critical, customer-facing output. Your stack needs both.

### Installing Ollama

Ollama is the foundation. It turns your machine into a local inference server with a clean API.

```bash
# Linux
curl -fsSL https://ollama.ai/install.sh | sh

# macOS
# Download from https://ollama.ai or:
brew install ollama

# Windows
# Download installer from https://ollama.ai
# Or use winget:
winget install Ollama.Ollama
```

Verify the installation:

```bash
ollama --version
# Should show version 0.3.x or higher

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

### Model Selection Guide

Don't download every model you see. Be strategic. Here's what to pull and when to use each.

#### Tier 1: The Workhorse (7B-8B models)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**Use for:**
- Text classification ("Is this email spam or legitimate?")
- Summarization (condense long documents into bullet points)
- Simple data extraction (pull names, dates, amounts from text)
- Sentiment analysis
- Content tagging and categorization
- Embedding generation (if using a model with embedding support)

**Performance (typical):**
- RTX 3060 12GB: ~40-60 tokens/second
- RTX 4090: ~100-130 tokens/second
- M2 Pro 16GB: ~30-45 tokens/second
- CPU only (Ryzen 7 5800X): ~8-12 tokens/second

**Cost comparison:**
- 1 million tokens via GPT-4o-mini: ~$0.60
- 1 million tokens locally (8B model): ~$0.003 in electricity
- Break-even point: ~5,000 tokens (you save money from literally the first request)

#### Tier 2: The Balanced Choice (13B-14B models)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**Use for:**
- Content drafting (blog posts, documentation, marketing copy)
- Code generation (functions, scripts, boilerplate)
- Complex data transformation
- Multi-step reasoning tasks
- Translation with nuance

**Performance (typical):**
- RTX 3060 12GB: ~20-30 tokens/second (quantized)
- RTX 4090: ~60-80 tokens/second
- M2 Pro 32GB: ~20-30 tokens/second
- CPU only: ~3-6 tokens/second (not practical for real-time)

**When to use over 7B:** When the output quality of 7B isn't good enough but you don't need to pay for API calls. Test both on your actual use case — sometimes 7B is fine and you're just wasting compute.

#### Tier 3: The Quality Tier (30B-70B models)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**Use for:**
- Customer-facing content that needs to be excellent
- Complex analysis and reasoning
- Long-form content generation
- Tasks where quality directly impacts whether someone pays you

**Performance (typical):**
- RTX 4090 (24GB): 70B at ~8-15 tokens/second (usable but slow)
- Dual GPU or 48GB+: 70B at ~20-30 tokens/second
- M3 Max 64GB: 70B at ~10-15 tokens/second

> **Real Talk:** If you don't have 24GB+ VRAM, skip the 70B models entirely. Use API calls for quality-critical output. A 70B model running at 3 tokens/second from system RAM is technically possible but practically useless for any income-generating workflow. Your time has value.

#### Tier 4: API Models (When Local Isn't Enough)

Local models are for volume and privacy. API models are for quality ceilings and specialized capabilities.

**When to use API models:**
- Customer-facing output where quality = revenue (sales copy, premium content)
- Complex reasoning chains that smaller models fumble
- Vision/multimodal tasks (analyzing images, screenshots, documents)
- When you need structured JSON output with high reliability
- When speed matters and your local hardware is slow

**Cost comparison table (as of early 2025 — check current pricing):**

| Model | Input (per 1M tokens) | Output (per 1M tokens) | Best For |
|-------|----------------------|------------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | Cheap volume work (when local isn't available) |
| GPT-4o | $2.50 | $10.00 | Vision, complex reasoning |
| Claude 3.5 Sonnet | $3.00 | $15.00 | Code, analysis, long context |
| Claude 3.5 Haiku | $0.80 | $4.00 | Fast, cheap, good quality balance |
| DeepSeek V3 | $0.27 | $1.10 | Budget-friendly, strong performance |

**The hybrid strategy:**
1. Local 7B/13B handles 80% of requests (classification, extraction, summarization)
2. API handles 20% of requests (quality-critical generation, complex tasks)
3. Your effective cost: ~$0.50-2.00 per million tokens blended (instead of $5-15 pure API)

This hybrid approach is how you build services with healthy margins. More on this in Module R.

### Production Configuration

Running Ollama for income work is different from running it for personal chat. Here's how to configure it properly.

#### Set Environment Variables

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

#### Create a Modelfile for Your Workload

Instead of using default model settings, create a custom Modelfile tuned for your income workload:

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

#### Batching and Queue Management

For income workloads, you'll often need to process many items. Here's a basic batching setup:

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

### Benchmarking YOUR Rig

Don't trust anyone else's benchmarks. Measure your own:

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

Write down your tokens/second for each model. This number determines which income workflows are practical for your rig.

**Speed requirements by use case:**
- Batch processing (async): 5+ tokens/sec is fine (you don't care about latency)
- Interactive tools (user waits): 20+ tokens/sec minimum
- Real-time API (customer-facing): 30+ tokens/sec for good UX
- Streaming chat: 15+ tokens/sec feels responsive

### Securing Your Local Inference Server

Your Ollama instance should never be accessible from the internet unless you explicitly intend it.

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

> **Common Mistake:** Binding Ollama to 0.0.0.0 for "convenience" and forgetting about it. Anyone who finds your IP can use your GPU for free inference. Worse, they can extract model weights and system prompts. Always localhost. Always tunnel.

### Lesson 2 Checkpoint

You should now have:
- [ ] Ollama installed and running
- [ ] At least one workhorse model pulled (llama3.1:8b or equivalent)
- [ ] A custom Modelfile for your expected workload
- [ ] Benchmark numbers: tokens/second for each model on your rig
- [ ] Ollama bound to localhost only

*In the full STREETS course, Module T (Technical Moats) shows you how to build proprietary model configurations, fine-tuned pipelines, and custom toolchains that competitors can't easily replicate. Module R (Revenue Engines) gives you the exact services to build on top of this stack.*

---

## Lesson 3: The Privacy Advantage

*"Your private setup IS a competitive advantage — not just a preference."*

### Privacy Is a Product Feature, Not a Limitation

Most developers set up local infrastructure because they personally value privacy, or because they enjoy tinkering. That's fine. But you're leaving money on the table if you don't realize that **privacy is one of the most marketable features in tech right now.**

Here's why: every time a company sends data to OpenAI's API, that data passes through a third party. For many businesses — especially those in healthcare, finance, legal, government, and EU-based companies — this is a real problem. Not a theoretical one. A "we can't use this tool because compliance said no" problem.

You, running models locally on your machine, don't have that problem.

### The Regulatory Tailwind

The regulatory environment is moving in your direction. Fast.

**EU AI Act (enforced from 2024-2026):**
- High-risk AI systems need documented data processing pipelines
- Companies must demonstrate where data flows and who processes it
- Local processing simplifies compliance dramatically
- EU companies are actively looking for AI service providers who can guarantee EU data residency

**GDPR (already enforced):**
- "Data processing" includes sending text to an LLM API
- Companies need Data Processing Agreements with every third party
- Local processing eliminates the third party entirely
- This is a real selling point: "Your data never leaves your infrastructure. There is no third-party DPA to negotiate."

**Industry-specific regulations:**
- **HIPAA (US Healthcare):** Patient data cannot be sent to consumer AI APIs without a BAA (Business Associate Agreement). Most AI providers don't offer BAAs for API access. Local processing sidesteps this entirely.
- **SOC 2 (Enterprise):** Companies undergoing SOC 2 audits need to document every data processor. Fewer processors = easier audits.
- **ITAR (US Defense):** Controlled technical data cannot leave US jurisdiction. Cloud AI providers with international infrastructure are problematic.
- **PCI DSS (Finance):** Cardholder data processing has strict requirements about where data travels.

### How to Position Privacy in Sales Conversations

You don't need to be a compliance expert. You need to understand three phrases and know when to use them:

**Phrase 1: "Your data never leaves your infrastructure."**
Use when: Talking to any privacy-conscious prospect. This is the universal hook.

**Phrase 2: "No third-party data processing agreement required."**
Use when: Talking to European companies or any company with a legal/compliance team. This saves them weeks of legal review.

**Phrase 3: "Full audit trail, single-tenant processing."**
Use when: Talking to enterprise or regulated industries. They need to prove their AI pipeline to auditors.

**Example positioning (for your service page or proposals):**

> "Unlike cloud-based AI services, [Your Service] processes all data locally on dedicated hardware. Your documents, code, and data never leave the processing environment. There are no third-party APIs in the pipeline, no data sharing agreements to negotiate, and full audit logging of every operation. This makes [Your Service] suitable for organizations with strict data handling requirements, including GDPR, HIPAA, and SOC 2 compliance environments."

That paragraph, on a landing page, will attract exactly the clients who will pay premium rates.

### The Premium Pricing Justification

Here's the business case in hard numbers:

**Standard AI processing service (using cloud APIs):**
- The client's data goes to OpenAI/Anthropic/Google
- You're competing with every developer who can call an API
- Market rate: $0.01-0.05 per document processed
- You're essentially reselling API access with a markup

**Privacy-first AI processing service (your local stack):**
- The client's data stays on your machine
- You're competing with a much smaller pool of providers
- Market rate: $0.10-0.50 per document processed (5-10x premium)
- You're selling infrastructure + expertise + compliance

The privacy premium is real: **5x to 10x** over commodity cloud-based services for the same underlying task. And the clients who pay it are more loyal, less price-sensitive, and have larger budgets.

### Setting Up Isolated Workspaces

If you have a day job (most of you do), you need clean separation between employer work and income work. This isn't just legal protection — it's operational hygiene.

**Option 1: Separate user accounts (recommended)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**Option 2: Containerized workspaces**

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

**Option 3: Separate physical machine (most bulletproof)**

If you're serious about this and your income justifies it, a dedicated machine eliminates all questions. A used Dell OptiPlex with an RTX 3060 costs $400-600 and pays for itself in the first month of client work.

**Minimum separation checklist:**
- [ ] Income projects in a separate directory (never mixed with employer repos)
- [ ] Separate API keys for income work (never use employer-provided keys)
- [ ] Separate browser profile for income-related accounts
- [ ] Income work never done on employer hardware
- [ ] Income work never done on employer network (use your personal internet or a VPN)
- [ ] Separate GitHub/GitLab account for income projects (optional but clean)

> **Common Mistake:** Using your employer's OpenAI API key "just for testing" your side project. This creates a paper trail that your employer's billing dashboard can see, and it muddies the IP waters. Get your own keys. They're cheap.

### Lesson 3 Checkpoint

You should now understand:
- [ ] Why privacy is a marketable product feature, not just a personal preference
- [ ] Which regulations create demand for local AI processing
- [ ] Three phrases to use in sales conversations about privacy
- [ ] How privacy-first services command 5-10x premium pricing
- [ ] How to separate income work from employer work

*In the full STREETS course, Module E (Evolving Edge) teaches you how to track regulatory changes and position yourself ahead of new compliance requirements before your competitors even know they exist.*

---

## Lesson 4: The Legal Minimum

*"Fifteen minutes of legal setup now prevents months of problems later."*

### This Is Not Legal Advice

I'm a developer, not a lawyer. What follows is a practical checklist that most developers in most situations should address. If your situation is complex (equity in your employer, non-compete with specific terms, etc.), spend $200 on a 30-minute consultation with an employment attorney. It's the best ROI you'll get.

### Step 1: Read Your Employment Contract

Find your employment contract or offer letter. Search for these sections:

**Intellectual Property Assignment clause** — Look for language like:
- "All inventions, developments, and work product..."
- "...created during the term of employment..."
- "...related to the Company's business or anticipated business..."

**Key phrases that restrict you:**
- "All work product created during employment belongs to the Company" (broad — potentially problematic)
- "Work product created using Company resources" (narrower — usually fine if you use your own equipment)
- "Related to the Company's current or anticipated business" (depends on what your employer does)

**Key phrases that free you:**
- "Excluding work done entirely on Employee's own time with Employee's own resources and unrelated to Company business" (this is your carve-out — many US states require this)
- Some states (California, Washington, Minnesota, Illinois, others) have laws that limit employer IP claims on personal projects, regardless of what the contract says.

### The 3 Questions Test

For any income project, ask:

1. **Time:** Are you doing this work on your own time? (Not during work hours, not during on-call shifts)
2. **Equipment:** Are you using your own hardware, your own internet, your own API keys? (Not employer laptop, not employer VPN, not employer cloud accounts)
3. **Subject matter:** Is this unrelated to your employer's business? (If you work at a healthcare AI company and want to sell healthcare AI services... that's a problem. If you work at a healthcare AI company and want to sell document processing for real estate agents... that's fine.)

If all three answers are clean, you're almost certainly fine. If any answer is murky, get clarity before proceeding.

> **Real Talk:** The vast majority of developers who do side work never have an issue. Employers care about protecting competitive advantages, not preventing you from making extra money on unrelated projects. But "almost certainly fine" is not "definitely fine." If your contract is unusually broad, have a conversation with your manager or HR — or consult a lawyer. The downside of not checking is much worse than the mild awkwardness of asking.

### Step 2: Choose a Business Structure

You need a legal entity to separate your personal assets from your business activities, and to open the door for business banking, payment processing, and tax benefits.

#### United States

| Structure | Cost | Protection | Best For |
|-----------|------|------------|----------|
| **Sole Proprietorship** (default) | $0 | None (personal liability) | Testing the waters. First $1K. |
| **Single-Member LLC** | $50-500 (varies by state) | Personal asset protection | Active income work. Most developers should start here. |
| **S-Corp election** (on an LLC) | LLC cost + $0 for election | Same as LLC + payroll tax benefits | When you're consistently earning $40K+/year from this |

**Recommended for US developers:** Single-Member LLC in your state of residence.

**Cheapest states to form:** Wyoming ($100, no state income tax), New Mexico ($50), Montana ($70). But forming in your home state is usually simplest unless you have a specific reason not to.

**How to file:**
1. Go to your state's Secretary of State website
2. Search "form LLC" or "business entity filing"
3. File Articles of Organization (10-minute form)
4. Get an EIN from the IRS (free, takes 5 minutes at irs.gov)

#### United Kingdom

| Structure | Cost | Protection | Best For |
|-----------|------|------------|----------|
| **Sole Trader** | Free (register with HMRC) | None | First income. Testing. |
| **Limited Company (Ltd)** | ~$15 via Companies House | Personal asset protection | Any serious income work. |

**Recommended:** Ltd company via Companies House. It takes about 20 minutes and costs GBP 12.

#### European Union

Varies significantly by country, but the general pattern:

- **Germany:** Einzelunternehmer (sole proprietor) for start, GmbH for serious work (but GmbH requires EUR 25,000 capital — consider UG for EUR 1)
- **Netherlands:** Eenmanszaak (sole proprietor, free to register) or BV (comparable to Ltd)
- **France:** Micro-entrepreneur (simplified, recommended for starting)
- **Estonia:** e-Residency + OUE (popular for non-residents, fully online)

#### Australia

| Structure | Cost | Protection | Best For |
|-----------|------|------------|----------|
| **Sole Trader** | Free ABN | None | Starting out |
| **Pty Ltd** | ~AUD 500-800 via ASIC | Personal asset protection | Serious income |

**Recommended:** Start with a Sole Trader ABN (free, instant), move to Pty Ltd when you're earning consistently.

### Step 3: Payment Processing (15-minute setup)

You need a way to get paid. Set this up now, not when your first client is waiting.

**Stripe (recommended for most developers):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

Time: ~15 minutes. You can start accepting payments immediately (Stripe holds funds for 7 days on new accounts).

**Lemon Squeezy (recommended for digital products):**

If you're selling digital products (templates, tools, courses, SaaS), Lemon Squeezy acts as your Merchant of Record. This means:
- They handle sales tax, VAT, and GST for you globally
- You don't need to register for VAT in the EU
- They handle refunds and disputes

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (for international developers or those wanting a US entity):**

If you're outside the US but want to sell to US customers with a US entity:
- $500 one-time fee
- Creates a Delaware LLC for you
- Sets up a US bank account (via Mercury or Stripe)
- Provides registered agent service
- Takes about 1-2 weeks

### Step 4: Privacy Policy and Terms of Service

If you're selling any service or product online, you need these. Don't pay a lawyer for boilerplate.

**Free, reputable sources for templates:**
- **Termly.io** — Free privacy policy and ToS generator. Answer questions, get documents.
- **Avodocs.com** — Open-source legal documents for startups. Free.
- **GitHub's choosealicense.com** — For open-source project licenses specifically.
- **Basecamp's open-sourced policies** — Search "Basecamp open source policies" — good, plain-English templates.

**What your privacy policy must cover (if you process any client data):**
- What data you collect
- How you process it (locally — this is your advantage)
- How long you retain it
- How clients can request deletion
- Whether any third parties access the data (ideally: none)

**Time:** 30 minutes with a template generator. Done.

### Step 5: Separate Bank Account

Do not run business income through your personal checking account. The reasons:

1. **Tax clarity:** When tax time comes, you need to know exactly what was business income and what wasn't.
2. **Legal protection:** If you have an LLC, commingling personal and business funds can "pierce the corporate veil" — meaning a court can ignore your LLC's liability protection.
3. **Professionalism:** Invoices from "John's Consulting LLC" hitting a dedicated business account looks legitimate. Payments to your personal Venmo do not.

**Free or low-cost business banking:**
- **Mercury** (US) — Free, designed for startups. Excellent API if you want to automate bookkeeping later.
- **Relay** (US) — Free, good for separating income streams into sub-accounts.
- **Wise Business** (International) — Low-cost multi-currency. Great for receiving payments in USD, EUR, GBP, etc.
- **Starling Bank** (UK) — Free business account.
- **Qonto** (EU) — Clean business banking for European companies.

Open the account now. It takes 10-15 minutes online and 1-3 days for verification.

### Step 6: Tax Basics for Developer Side Income

> **Real Talk:** Taxes are the thing most developers ignore until April, and then panic about. Spending 30 minutes now saves you actual money and stress.

**United States:**
- Side income over $400/year requires self-employment tax (~15.3% for Social Security + Medicare)
- Plus your regular income tax bracket on the net profit
- **Quarterly estimated taxes:** If you'll owe more than $1,000 in taxes, the IRS expects quarterly payments (April 15, June 15, Sept 15, Jan 15). Underpayment triggers penalties.
- Set aside **25-30%** of net income for taxes. Put it in a separate savings account immediately.

**Common write-offs for developer side income:**
- API costs (OpenAI, Anthropic, etc.) — 100% deductible
- Hardware purchases used for business — depreciable or Section 179 deduction
- Electricity cost attributable to business use
- Software subscriptions used for income work
- Home office deduction (simplified: $5/sq ft, up to 300 sq ft = $1,500)
- Internet (business-use percentage)
- Domain names, hosting, email services
- Professional development (courses, books) related to your income work

**United Kingdom:**
- Report via Self Assessment tax return
- Trading income under GBP 1,000: tax-free (Trading Allowance)
- Above that: pay Income Tax + Class 4 NICs on profits
- Payment dates: January 31 and July 31

**Track everything from day one.** Use a simple spreadsheet if nothing else:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **Common Mistake:** "I'll figure out taxes later." Later is Q4, you owe $3,000 in estimated taxes plus penalties, and you've spent the money. Automate: every time income hits your business account, transfer 30% to a tax savings account immediately.

### Lesson 4 Checkpoint

You should now have (or have a plan for):
- [ ] Read your employment contract's IP clause
- [ ] Passed the 3 Questions Test for your planned income work
- [ ] Chosen a business structure (or decided to start as sole proprietor)
- [ ] Payment processing set up (Stripe or Lemon Squeezy)
- [ ] Privacy policy and ToS from a template generator
- [ ] Separate business bank account (or application submitted)
- [ ] Tax strategy: 30% set-aside + quarterly payment schedule

*In the full STREETS course, Module E (Execution Playbook) includes financial modeling templates that automatically calculate your tax obligations, project profitability, and break-even points for each revenue engine.*

---

## Lesson 5: The $200/month Budget

*"Your business has a burn rate. Know it. Control it. Make it earn."*

### Why $200/month

Two hundred dollars per month is the minimum viable budget for a developer income operation. It's enough to run real services, serve real customers, and generate real revenue. It's also small enough that if nothing works, you haven't bet the farm.

The goal is simple: **turn $200/month into $600+/month within 90 days.** If you can do that, you have a business. If you can't, you change strategy — not increase budget.

### The Budget Breakdown

#### Tier 1: API Credits — $50-100/month

This is your production compute for customer-facing quality.

**Recommended starting allocation:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**How to manage API spend:**

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

**The hybrid spend strategy:**
- Use local LLMs for 80% of processing (classification, extraction, summarization, drafts)
- Use API calls for 20% of processing (final quality pass, complex reasoning, customer-facing output)
- Your effective cost per task drops dramatically vs. pure API usage

#### Tier 2: Infrastructure — $30-50/month

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**Do you need a VPS?**

If your income model is:
- **Selling digital products:** No. Host on Vercel/Netlify for free. Use Lemon Squeezy for delivery.
- **Running async processing for clients:** Maybe. You can run jobs on your local rig and deliver results. A VPS adds reliability.
- **Offering an API service:** Yes, probably. A $5-10 VPS acts as a lightweight API gateway, even if the heavy processing happens on your local machine.
- **Selling SaaS:** Yes. But start with the cheapest tier and scale up.

**Recommended starter infrastructure:**

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

Total infrastructure cost: $5-20/month. The rest is free tiers.

#### Tier 3: Tools — $20-30/month

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

> **Real Talk:** You can run your entire tool stack on free tiers when starting. The $20-30 allocated here is for when you outgrow free tiers or want a specific premium feature. Don't spend it just because it's in the budget. Unspent budget is profit.

#### Tier 4: Reserve — $0-30/month

This is your "things I didn't anticipate" fund:
- An API cost spike from an unexpectedly large batch job
- A tool you need for one specific client project
- Emergency domain purchase when you find the perfect name
- A one-time purchase (theme, template, icon set)

If you don't use the reserve, it accumulates. After 3 months of unused reserve, consider reallocating to API credits or infrastructure.

### The ROI Calculation

This is the only number that matters:

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

**When to increase budget:**

Increase your budget ONLY when:
1. You're consistently at 2x+ ROI for 2+ months
2. More spend would directly increase revenue (e.g., more API credits = more client capacity)
3. The increase is tied to a specific, tested revenue stream

**When NOT to increase budget:**
- "I think this new tool will help" (test free alternatives first)
- "Everyone says you need to spend money to make money" (not at this stage)
- "A bigger VPS will make my service faster" (is speed actually the bottleneck?)
- You haven't hit 1x ROI yet (fix the revenue, not the spend)

**The scaling ladder:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **Common Mistake:** Treating the $200 as an "investment" that doesn't need to return money immediately. No. This is an experiment with a 90-day deadline. If $200/month doesn't generate $200/month in revenue within 90 days, something about the strategy needs to change. The money, the market, the offer — something isn't working. Be honest with yourself.

### Lesson 5 Checkpoint

You should now have:
- [ ] A monthly budget of ~$200 allocated across four tiers
- [ ] API accounts created with spending limits set
- [ ] Infrastructure decisions made (local-only vs. local + VPS)
- [ ] A tool stack selected (mostly free tiers to start)
- [ ] ROI targets: 3x within 90 days
- [ ] A clear rule: increase budget only after proving ROI

*In the full STREETS course, Module E (Execution Playbook) includes a financial dashboard template that tracks your spend, revenue, and ROI per revenue engine in real-time — so you always know which streams are profitable and which need adjustment.*

---

## Lesson 6: Your Sovereign Stack Document

*"Every business has a plan. This is yours — and it fits on two pages."*

### The Deliverable

This is the most important thing you'll create in Module S. Your Sovereign Stack Document is a single reference that captures everything about your income-generating infrastructure. You'll reference it throughout the rest of the STREETS course, update it as your setup evolves, and use it to make clear-headed decisions about what to build and what to skip.

Create a new file. Markdown, Google Doc, Notion page, plain text — whatever you'll actually maintain. Use the template below, filling in every field with the numbers and decisions from Lessons 1-5.

### The Template

Copy this entire template and fill it in. Every field. No skipping.

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

### How to Use This Document

1. **Before starting any new project:** Check your Sovereign Stack. Do you have the hardware, time, skills, and budget to execute?
2. **Before buying anything:** Check your budget allocation. Is this purchase in the plan?
3. **Monthly review:** Update the "Actual" column in your budget. Update revenue numbers. Adjust allocations based on what's working.
4. **When someone asks what you do:** Your "What I Can Offer Today" section is your instant pitch.
5. **When you're tempted to chase a shiny new idea:** Check your constraints. Does this fit within your time, skills, and hardware? If not, add it to "Building Toward" for later.

### The One-Hour Exercise

Set a timer for 60 minutes. Fill in every field of the template. Don't overthink it. Don't research extensively. Write what you know right now. You can update it later.

The fields you can't fill in? Those are your action items for this week:
- Empty benchmark numbers? Run the benchmark script from Lesson 2.
- No business entity? Start the filing process from Lesson 4.
- No payment processing? Set up Stripe from Lesson 4.
- Blank skills inventory? Spend 15 minutes listing everything you've been paid to do in the last 5 years.

> **Common Mistake:** Spending 3 hours making the document "perfect" instead of 1 hour making it "done." The Sovereign Stack Document is a working reference, not a business plan for investors. No one will see it but you. Accuracy matters. Formatting doesn't.

### Lesson 6 Checkpoint

You should now have:
- [ ] A complete Sovereign Stack Document saved somewhere you'll actually open it
- [ ] All six sections filled in with real numbers (not aspirational ones)
- [ ] A clear list of action items for gaps in your setup
- [ ] A date set for your first monthly review (30 days from now)

---

## Module S: Complete

### What You've Built in Two Weeks

Look at what you now have that you didn't have when you started:

1. **A hardware inventory** mapped to income-generating capabilities — not just specs on a sticker.
2. **A production-grade local LLM stack** with Ollama, benchmarked on your actual hardware, configured for real workloads.
3. **A privacy advantage** you understand how to market — with specific language for specific audiences.
4. **A legal and financial foundation** — business entity (or plan), payment processing, bank account, tax strategy.
5. **A controlled budget** with clear ROI targets and a 90-day deadline to prove the model.
6. **A Sovereign Stack Document** that captures all of the above in a single reference you'll use for every decision going forward.

This is more than most developers ever set up. Seriously. Most people who want to make side income skip straight to "build something cool" and then wonder why they can't get paid. You now have the infrastructure to get paid.

But infrastructure without direction is just an expensive hobby. You need to know where to aim this stack.

### What Comes Next: Module T — Technical Moats

Module S gave you the foundation. Module T answers the critical question: **how do you build something competitors can't easily copy?**

Here's what Module T covers:

- **Proprietary data pipelines** — how to create datasets that only you have access to, legally and ethically
- **Custom model configurations** — fine-tuning and prompt engineering that produces output quality others can't match with default settings
- **Compounding skill stacks** — why "Python + healthcare" beats "Python + JavaScript" for income, and how to identify your unique combination
- **Technical barriers to entry** — infrastructure designs that would take a competitor months to replicate
- **The Moat Audit** — a framework for evaluating whether your project has a defensible advantage or is just another commodity service

The difference between a developer making $500/month and one making $5,000/month is rarely skill. It's moats. Things that make your offering hard to replicate, even if someone has the same hardware and the same models.

### The Full STREETS Roadmap

| Module | Title | Focus | Duration |
|--------|-------|-------|----------|
| **S** | Sovereign Setup | Infrastructure, legal, budget | Weeks 1-2 (complete) |
| **T** | Technical Moats | Defensible advantages, proprietary assets | Weeks 3-4 |
| **R** | Revenue Engines | Specific monetization playbooks with code | Weeks 5-8 |
| **E** | Execution Playbook | Launch sequences, pricing, first customers | Weeks 9-10 |
| **E** | Evolving Edge | Staying ahead, trend detection, adaptation | Weeks 11-12 |
| **T** | Tactical Automation | Automating operations for passive income | Weeks 13-14 |
| **S** | Stacking Streams | Multiple income sources, portfolio strategy | Weeks 15-16 |

Module R (Revenue Engines) is where most of the money is made. But without S and T, you're building on sand.

---

**Ready for the full playbook?**

You've seen the foundation. You've built it yourself. Now get the complete system.

**Get STREETS Core** — the full 16-week course with all seven modules, revenue engine code templates, financial dashboards, and the private community of developers building income on their own terms.

*Your rig. Your rules. Your revenue.*
