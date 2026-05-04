# الوحدة S: الإعداد السيادي

**دورة STREETS لدخل المطورين — وحدة مجانية**
*الأسبوعان 1-2 | 6 دروس | المُخرَج: وثيقة المكدس السيادي الخاصة بك*

> "جهازك هو بنيتك التحتية التجارية. اضبط إعداداته على هذا الأساس."

---

أنت تمتلك بالفعل أقوى أداة لتوليد الدخل لن يمتلكها معظم الناس أبدًا: محطة عمل مطور متصلة بالإنترنت، بقدرة حوسبة محلية، والمهارات اللازمة لربط كل ذلك معًا.

معظم المطورين يتعاملون مع أجهزتهم كمنتج استهلاكي. شيء يلعبون عليه، يبرمجون عليه، يتصفحون عليه. لكن هذا الجهاز نفسه — الموجود تحت مكتبك الآن — يمكنه تشغيل الاستدلال، خدمة الـ API، معالجة البيانات، وتوليد الإيرادات 24 ساعة يوميًا بينما أنت نائم.

هذه الوحدة تدور حول النظر إلى ما تمتلكه بالفعل من منظور مختلف. ليس "ماذا يمكنني بناؤه؟" بل "ماذا يمكنني بيعه؟"

بنهاية هذين الأسبوعين، ستملك:

- جردة واضحة لقدراتك في توليد الدخل
- مكدس LLM محلي بمستوى إنتاجي
- أساس قانوني ومالي (حتى لو كان بسيطًا)
- وثيقة مكدس سيادي مكتوبة تصبح مخطط عملك التجاري

لا كلام فارغ. لا "فقط آمن بنفسك." أرقام حقيقية، أوامر حقيقية، قرارات حقيقية.

{@ mirror sovereign_readiness @}

لنبدأ.

---

## الدرس 1: تدقيق الجهاز

*"لا تحتاج إلى 4090. إليك ما يهم فعلًا."*

### جهازك هو أصل تجاري

عندما تقيّم شركة بنيتها التحتية، لا تكتفي بسرد المواصفات — بل تربط القدرات بفرص الإيرادات. هذا ما ستفعله الآن.

{? if computed.profile_completeness != "0" ?}
> **جهازك الحالي:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} أنوية / {= profile.cpu.threads | fallback("?") =} خيوط)، {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM، {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}، {= profile.storage.free | fallback("?") =} فارغ / {= profile.storage.total | fallback("?") =} إجمالي ({= profile.storage.type | fallback("unknown") =})، يعمل بنظام {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}.
{? endif ?}

افتح الطرفية ونفذ الأوامر التالية. دوّن كل رقم. ستحتاجها لوثيقة المكدس السيادي في الدرس 6.

### جرد العتاد

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

**ما يهم للدخل:**
- عدد الأنوية يحدد كم مهمة متزامنة يستطيع جهازك التعامل معها. تشغيل LLM محلي مع معالجة مهمة دفعية في نفس الوقت يتطلب توازيًا حقيقيًا.
{? if profile.cpu.cores ?}
- *معالجك {= profile.cpu.model | fallback("CPU") =} يملك {= profile.cpu.cores | fallback("?") =} أنوية — راجع جدول المتطلبات أدناه لترى أي محركات دخل يدعمها معالجك.*
{? endif ?}
- لمعظم محركات الدخل في هذه الدورة، أي معالج حديث بـ 8+ أنوية من السنوات الخمس الأخيرة كافٍ.
- إذا كنت تشغل LLM محليًا على CPU فقط (بدون GPU)، تريد 16+ نواة. معالج Ryzen 7 5800X أو Intel i7-12700 هو الحد الأدنى العملي.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**ما يهم للدخل:**
- 16 GB: الحد الأدنى المطلق. يمكنك تشغيل نماذج 7B والقيام بأعمال أتمتة أساسية.
- 32 GB: مريح. شغّل نماذج 13B محليًا، تعامل مع مشاريع متعددة، أبقِ بيئة التطوير تعمل بجانب أحمال العمل المدرة للدخل.
- 64 GB+: يمكنك تشغيل نماذج 30B+ على CPU، أو الاحتفاظ بنماذج متعددة محملة. هنا تصبح الأمور مثيرة لبيع خدمات الاستدلال.
{? if profile.ram.total ?}
*نظامك يملك {= profile.ram.total | fallback("?") =} RAM. راجع الجدول أعلاه لترى في أي مستوى قدرة أنت — هذا يؤثر مباشرة على النماذج المحلية العملية لأحمال عملك المدرة للدخل.*
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

**ما يهم للدخل:**

هذه المواصفة التي يهتم بها الجميع، وإليك الحقيقة الصريحة: **GPU الخاص بك يحدد مستوى LLM المحلي، ومستوى LLM المحلي يحدد أي مصادر دخل تعمل بأسرع ما يمكن.** لكنه لا يحدد إن كنت تستطيع كسب المال أصلًا.

| VRAM | قدرة LLM | الصلة بالدخل |
|------|----------|-------------|
| 0 (CPU فقط) | نماذج 7B بسرعة ~5 رموز/ثانية | معالجة دفعية، عمل غير متزامن. بطيء لكن وظيفي. |
| 6-8 GB (RTX 3060، إلخ.) | نماذج 7B بسرعة ~30 رمز/ثانية، 13B مضغوطة | كافٍ لمعظم مصادر دخل الأتمتة. |
| 12 GB (RTX 3060 12GB، 4070) | 13B بسرعة كاملة، 30B مضغوطة | النقطة المثالية. معظم محركات الدخل تعمل جيدًا هنا. |
| 16-24 GB (RTX 4090، 3090) | نماذج 30B-70B | المستوى المميز. بِع جودة لا يستطيع الآخرون مطابقتها محليًا. |
| 48 GB+ (GPU مزدوج، A6000) | 70B+ بسرعة | استدلال محلي بمستوى المؤسسات. ميزة تنافسية جدية. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ باستخدام الذاكرة الموحدة | كفاءة ممتازة. تكلفة طاقة أقل من معادل NVIDIA. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **GPU الخاص بك:** {= profile.gpu.model | fallback("Unknown") =} مع {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}أنت في المستوى المميز. نماذج 30B-70B في متناول يدك محليًا. هذه ميزة تنافسية جدية.{? elif computed.gpu_tier == "sweet_spot" ?}أنت في النقطة المثالية. 13B بسرعة كاملة، 30B مضغوطة. معظم محركات الدخل تعمل جيدًا هنا.{? elif computed.gpu_tier == "capable" ?}يمكنك تشغيل نماذج 7B بسرعة جيدة و13B مضغوطة. كافٍ لمعظم مصادر دخل الأتمتة.{? else ?}لديك تسريع GPU متاح. راجع الجدول أعلاه لترى أين تقع.{? endif ?}
{? else ?}
> **لم يتم اكتشاف GPU مخصص.** ستشغل الاستدلال على CPU، مما يعني ~5-12 رمز/ثانية على نماذج 7B. هذا مقبول للمعالجة الدفعية والعمل غير المتزامن. استخدم استدعاءات API لسد فجوة السرعة للمخرجات الموجهة للعملاء.
{? endif ?}

> **كلام صريح:** إذا كنت تملك RTX 3060 12GB، فأنت في وضع أفضل من 95% من المطورين الذين يحاولون تحقيق الربح من الذكاء الاصطناعي. توقف عن الانتظار للحصول على 4090. الـ RTX 3060 12GB هي سيارة Honda Civic في مجال الذكاء الاصطناعي المحلي — موثوقة، فعالة، تنجز المهمة. المال الذي ستنفقه على ترقية GPU أفضل أن يُنفق على أرصدة API للجودة الموجهة للعملاء بينما تتولى نماذجك المحلية العمل الشاق.

#### التخزين

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**ما يهم للدخل:**
- نماذج LLM تأخذ مساحة: نموذج 7B = ~4 GB، 13B = ~8 GB، 70B = ~40 GB (مضغوط).
- تحتاج مساحة لبيانات المشاريع، قواعد البيانات، التخزين المؤقت، ومخرجات العمل.
- SSD ضرورة لا نقاش فيها لأي شيء موجه للعملاء. تحميل النموذج من HDD يضيف 30-60 ثانية وقت بدء تشغيل.
- الحد الأدنى العملي: 500 GB SSD مع 100 GB فارغة على الأقل.
- مريح: 1 TB SSD. احتفظ بالنماذج على SSD، أرشف على HDD.
{? if profile.storage.free ?}
*لديك {= profile.storage.free | fallback("?") =} فارغة على {= profile.storage.type | fallback("your drive") =}. {? if profile.storage.type == "SSD" ?}جيد — SSD يعني تحميل سريع للنماذج.{? elif profile.storage.type == "NVMe" ?}ممتاز — NVMe هو الخيار الأسرع لتحميل النماذج.{? else ?}فكر في SSD إن لم تكن تستخدمه بالفعل — يُحدث فرقًا حقيقيًا في أوقات تحميل النماذج.{? endif ?}*
{? endif ?}

#### الشبكة

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**ما يهم للدخل:**
{? if profile.network.download ?}
*اتصالك: {= profile.network.download | fallback("?") =} تنزيل / {= profile.network.upload | fallback("?") =} رفع.*
{? endif ?}
- سرعة التنزيل: 50+ Mbps. مطلوبة لسحب النماذج والحزم والبيانات.
- سرعة الرفع: هذه هي نقطة الاختناق التي يتجاهلها معظم الناس. إذا كنت تقدم أي خدمة (API، نتائج معالجة، مخرجات)، سرعة الرفع مهمة.
  - 10 Mbps: كافية للتسليم غير المتزامن (ملفات معالجة، نتائج دفعية).
  - 50+ Mbps: مطلوبة إذا كنت تشغل أي نوع من نقاط نهاية API المحلية التي تستقبل طلبات خارجية.
  - 100+ Mbps: مريحة لكل شيء في هذه الدورة.
- زمن الاستجابة: أقل من 50ms لمزودي الخدمات السحابية الرئيسيين. شغّل `ping api.openai.com` و `ping api.anthropic.com` للتحقق.

#### وقت التشغيل

هذه المواصفة التي لا يفكر فيها أحد، لكنها تفصل الهواة عن الأشخاص الذين يكسبون المال أثناء نومهم.

اسأل نفسك:
- هل يمكن لجهازك العمل 24/7؟ (الطاقة، التبريد، الضوضاء)
- هل لديك UPS لانقطاعات الكهرباء؟
- هل اتصالك بالإنترنت مستقر بما يكفي لسير العمل الآلي؟
- هل يمكنك الوصول عن بعد عبر SSH إلى جهازك إذا تعطل شيء ما؟

إذا لم تستطع التشغيل 24/7، فلا بأس — العديد من مصادر الدخل في هذه الدورة هي مهام دفعية غير متزامنة تشغلها يدويًا. لكن المصادر التي تولد دخلًا سلبيًا حقيقيًا تتطلب وقت تشغيل مستمر.

{? if computed.os_family == "windows" ?}
**إعداد سريع لوقت التشغيل (Windows):** استخدم Task Scheduler لإعادة التشغيل التلقائي، فعّل Remote Desktop أو ثبّت Tailscale للوصول عن بعد، واضبط BIOS على "restore on AC power loss" للتعافي من الانقطاعات.
{? endif ?}

**إعداد سريع لوقت التشغيل (إذا أردت):**

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

### حسابات الكهرباء

الناس إما يتجاهلون هذا أو يبالغون في تهويله. لنقم بحسابات حقيقية.

**قياس استهلاك الطاقة الفعلي:**

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

**حساب التكلفة الشهرية:**

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
سعر الكهرباء لديك: تقريبًا {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (بناءً على متوسطات {= regional.country | fallback("your region") =}). تحقق من فاتورة الخدمات الفعلية — الأسعار تختلف حسب المزود والوقت من اليوم.
{? else ?}
متوسط الكهرباء في الولايات المتحدة حوالي $0.12/kWh. تحقق من سعرك الفعلي — يختلف بشكل كبير. كاليفورنيا قد تكون $0.25/kWh. بعض الدول الأوروبية تصل إلى $0.35/kWh. أجزاء من وسط غرب الولايات المتحدة بسعر $0.08/kWh.
{? endif ?}

**الخلاصة:** تشغيل جهازك 24/7 للدخل يكلف بين {= regional.currency_symbol | fallback("$") =}1-{= regional.currency_symbol | fallback("$") =}30/شهر في الكهرباء. إذا لم تستطع مصادر دخلك تغطية ذلك، المشكلة ليست في الكهرباء — بل في مصدر الدخل.

### الحد الأدنى للمواصفات حسب نوع محرك الدخل

إليك نظرة مسبقة على ما نتجه إليه في دورة STREETS الكاملة. الآن، فقط تحقق أين يقع جهازك:

| محرك الدخل | CPU | RAM | GPU | التخزين | الشبكة |
|------------|-----|-----|-----|---------|--------|
| **أتمتة المحتوى** (مقالات، نشرات بريدية) | 4+ أنوية | 16 GB | اختياري (API بديل) | 50 GB فارغ | 10 Mbps رفع |
| **خدمات معالجة البيانات** | 8+ أنوية | 32 GB | اختياري | 200 GB فارغ | 50 Mbps رفع |
| **خدمات AI API محلية** | 8+ أنوية | 32 GB | 8+ GB VRAM | 100 GB فارغ | 50 Mbps رفع |
| **أدوات توليد الكود** | 8+ أنوية | 16 GB | 8+ GB VRAM أو API | 50 GB فارغ | 10 Mbps رفع |
| **معالجة المستندات** | 4+ أنوية | 16 GB | اختياري | 100 GB فارغ | 10 Mbps رفع |
| **الوكلاء المستقلون** | 8+ أنوية | 32 GB | 12+ GB VRAM | 100 GB فارغ | 50 Mbps رفع |

> **خطأ شائع:** "أحتاج لترقية عتادي قبل أن أبدأ." لا. ابدأ بما لديك. استخدم استدعاءات API لسد الفجوات التي لا يغطيها عتادك. قم بالترقية عندما تبرر الإيرادات ذلك — وليس قبل.

{@ insight engine_ranking @}

### نقطة تفتيش الدرس 1

يجب أن تكون قد دوّنت الآن:
- [ ] موديل CPU وعدد الأنوية والخيوط
- [ ] حجم RAM
- [ ] موديل GPU وحجم VRAM (أو "لا يوجد")
- [ ] التخزين المتاح
- [ ] سرعات الشبكة (تنزيل/رفع)
- [ ] تكلفة الكهرباء الشهرية المقدرة للتشغيل 24/7
- [ ] فئات محركات الدخل التي يتأهل لها جهازك

احتفظ بهذه الأرقام. ستدخلها في وثيقة المكدس السيادي في الدرس 6.

{? if computed.profile_completeness != "0" ?}
> **4DA جمعت بالفعل معظم هذه الأرقام لك.** راجع الملخصات المخصصة أعلاه — جرد عتادك مملوء جزئيًا من اكتشاف النظام.
{? endif ?}

*في دورة STREETS الكاملة، الوحدة R (محركات الدخل) تعطيك أدلة تنفيذ محددة خطوة بخطوة لكل نوع محرك مذكور أعلاه — بما في ذلك الكود الدقيق للبناء والنشر.*

---

## الدرس 2: مكدس LLM المحلي

*"إعداد Ollama للاستخدام الإنتاجي — وليس للدردشة فقط."*

### لماذا تهم نماذج LLM المحلية للدخل

في كل مرة تستدعي API الخاص بـ OpenAI، أنت تدفع إيجارًا. في كل مرة تشغل نموذجًا محليًا، هذا الاستدلال مجاني بعد الإعداد الأولي. الحساب بسيط:

- GPT-4o: ~$5 لكل مليون رمز مدخل، ~$15 لكل مليون رمز مخرج
- Claude 3.5 Sonnet: ~$3 لكل مليون رمز مدخل، ~$15 لكل مليون رمز مخرج
- Llama 3.1 8B المحلي: $0 لكل مليون رمز (فقط كهرباء)

إذا كنت تبني خدمات تعالج آلاف الطلبات، الفرق بين $0 و $5-$15 لكل مليون رمز هو الفرق بين الربح والتعادل.

لكن إليك الفارق الدقيق الذي يفوت معظم الناس: **النماذج المحلية ونماذج API تؤدي أدوارًا مختلفة في مكدس الدخل.** النماذج المحلية تتعامل مع الحجم. نماذج API تتعامل مع المخرجات الحرجة الجودة الموجهة للعملاء. مكدسك يحتاج كليهما.

### تثبيت Ollama

{? if settings.has_llm ?}
> **لديك بالفعل LLM مُعَد:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}. إذا كان Ollama يعمل بالفعل، انتقل إلى "دليل اختيار النموذج" أدناه.
{? endif ?}

Ollama هو الأساس. يحول جهازك إلى خادم استدلال محلي بواجهة API نظيفة.

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
> **Windows:** استخدم المثبت من ollama.com أو `winget install Ollama.Ollama`. يعمل Ollama كخدمة خلفية تلقائيًا بعد التثبيت.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` هو أسرع طريق. يستفيد Ollama من الذاكرة الموحدة لـ Apple Silicon — ذاكرة {= profile.ram.total | fallback("system") =} RAM مشتركة بين أحمال عمل CPU وGPU.
{? elif computed.os_family == "linux" ?}
> **Linux:** سكريبت التثبيت يتولى كل شيء. إذا كنت تعمل بنظام {= profile.os.name | fallback("Linux") =}، يُثبت Ollama كخدمة systemd.
{? endif ?}

تحقق من التثبيت:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **ملاحظة الإصدار:** Ollama يصدر تحديثات بشكل متكرر. أوامر النماذج والخيارات في هذه الوحدة تم التحقق منها مقابل Ollama v0.5.x (أوائل 2026). إذا كنت تقرأ هذا لاحقًا، تحقق من [ollama.com/download](https://ollama.com/download) لأحدث إصدار و [ollama.com/library](https://ollama.com/library) لأسماء النماذج الحالية. المفاهيم الأساسية لا تتغير، لكن وسوم النماذج المحددة (مثل `llama3.1:8b`) قد تُستبدل بإصدارات أحدث.

### دليل اختيار النموذج

لا تحمّل كل نموذج تراه. كن استراتيجيًا. إليك ما يجب سحبه ومتى تستخدم كل واحد.

{? if computed.llm_tier ?}
> **مستوى LLM الخاص بك (بناءً على العتاد):** {= computed.llm_tier | fallback("unknown") =}. التوصيات أدناه موسومة حتى تركز على المستوى المناسب لجهازك.
{? endif ?}

#### المستوى 1: حصان العمل (نماذج 7B-8B)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**الاستخدام:**
- تصنيف النصوص ("هل هذا البريد إلكتروني عشوائي أم شرعي؟")
- التلخيص (تكثيف المستندات الطويلة في نقاط)
- استخراج البيانات البسيط (سحب الأسماء والتواريخ والمبالغ من النصوص)
- تحليل المشاعر
- وسم وتصنيف المحتوى
- توليد التضمينات (إذا كنت تستخدم نموذجًا يدعم التضمين)

**الأداء (نموذجي):**
- RTX 3060 12GB: ~40-60 رمز/ثانية
- RTX 4090: ~100-130 رمز/ثانية
- M2 Pro 16GB: ~30-45 رمز/ثانية
- CPU فقط (Ryzen 7 5800X): ~8-12 رمز/ثانية

**مقارنة التكلفة:**
- مليون رمز عبر GPT-4o-mini: ~$0.60
- مليون رمز محليًا (نموذج 8B): ~$0.003 في الكهرباء
- نقطة التعادل: ~5,000 رمز (توفر المال حرفيًا من الطلب الأول)

#### المستوى 2: الخيار المتوازن (نماذج 13B-14B)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**الاستخدام:**
- صياغة المحتوى (مقالات المدونة، التوثيق، نصوص التسويق)
- توليد الكود (دوال، سكريبتات، قوالب أساسية)
- تحويل البيانات المعقد
- مهام الاستنتاج متعددة الخطوات
- الترجمة مع الفروق الدقيقة

**الأداء (نموذجي):**
- RTX 3060 12GB: ~20-30 رمز/ثانية (مضغوط)
- RTX 4090: ~60-80 رمز/ثانية
- M2 Pro 32GB: ~20-30 رمز/ثانية
- CPU فقط: ~3-6 رمز/ثانية (غير عملي للاستخدام الفوري)

**متى تستخدمه بدل 7B:** عندما لا تكون جودة مخرجات 7B كافية لكنك لا تحتاج لدفع تكلفة استدعاءات API. اختبر كليهما على حالة استخدامك الفعلية — أحيانًا 7B يكون كافيًا وأنت فقط تهدر القدرة الحاسوبية.

{? if computed.gpu_tier == "capable" ?}
> **منطقة توسع المستوى 3** — {= profile.gpu.model | fallback("GPU") =} الخاص بك يستطيع التعامل مع 30B مضغوط بجهد، لكن 70B بعيد المنال محليًا. فكر في استدعاءات API للمهام التي تحتاج جودة مستوى 70B.
{? endif ?}

#### المستوى 3: مستوى الجودة (نماذج 30B-70B)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**الاستخدام:**
- المحتوى الموجه للعملاء الذي يجب أن يكون ممتازًا
- التحليل والاستنتاج المعقد
- توليد محتوى طويل
- المهام التي تؤثر فيها الجودة مباشرة على ما إذا كان شخص ما سيدفع لك

**الأداء (نموذجي):**
- RTX 4090 (24GB): 70B بسرعة ~8-15 رمز/ثانية (قابل للاستخدام لكن بطيء)
- GPU مزدوج أو 48GB+: 70B بسرعة ~20-30 رمز/ثانية
- M3 Max 64GB: 70B بسرعة ~10-15 رمز/ثانية

> **كلام صريح:** إذا لم يكن لديك 24GB+ VRAM، تخطَّ نماذج 70B تمامًا. استخدم استدعاءات API للمخرجات الحرجة الجودة. نموذج 70B يعمل بسرعة 3 رموز/ثانية من ذاكرة النظام ممكن تقنيًا لكنه عديم الفائدة عمليًا لأي سير عمل مدر للدخل. وقتك له قيمة.

#### المستوى 4: نماذج API (عندما لا يكفي المحلي)

النماذج المحلية للحجم والخصوصية. نماذج API للسقف الأعلى للجودة والقدرات المتخصصة.

**متى تستخدم نماذج API:**
- المخرجات الموجهة للعملاء حيث الجودة = الإيرادات (نصوص المبيعات، المحتوى المميز)
- سلاسل الاستنتاج المعقدة التي تتعثر فيها النماذج الأصغر
- المهام البصرية/متعددة الوسائط (تحليل الصور، لقطات الشاشة، المستندات)
- عندما تحتاج مخرجات JSON منظمة بموثوقية عالية
- عندما تهم السرعة وعتادك المحلي بطيء

**جدول مقارنة التكلفة (أوائل 2025 — تحقق من الأسعار الحالية):**

| النموذج | المدخلات (لكل مليون رمز) | المخرجات (لكل مليون رمز) | الأفضل لـ |
|---------|--------------------------|--------------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | عمل الحجم الرخيص (عندما لا يتوفر المحلي) |
| GPT-4o | $2.50 | $10.00 | الرؤية، الاستنتاج المعقد |
| Claude 3.5 Sonnet | $3.00 | $15.00 | الكود، التحليل، السياق الطويل |
| Claude 3.5 Haiku | $0.80 | $4.00 | سريع، رخيص، توازن جودة جيد |
| DeepSeek V3 | $0.27 | $1.10 | صديق للميزانية، أداء قوي |

**الاستراتيجية الهجينة:**
1. LLM المحلي 7B/13B يتعامل مع 80% من الطلبات (التصنيف، الاستخراج، التلخيص)
2. API يتعامل مع 20% من الطلبات (المراجعة النهائية للجودة، المهام المعقدة، المخرجات الموجهة للعملاء)
3. تكلفتك الفعالة لكل مهمة تنخفض بشكل كبير مقارنة بالاستخدام الصرف لـ API

هذا النهج الهجين هو كيف تبني خدمات بهوامش ربح صحية. المزيد عن هذا في الوحدة R.

### التكوين الإنتاجي

تشغيل Ollama لعمل الدخل مختلف عن تشغيله للدردشة الشخصية. إليك كيف تضبطه بشكل صحيح.

{? if computed.has_nvidia ?}
> **تم اكتشاف GPU من NVIDIA ({= profile.gpu.model | fallback("unknown") =}).** سيستخدم Ollama تسريع CUDA تلقائيًا. تأكد من تحديث تعريفات NVIDIA — شغّل `nvidia-smi` للتحقق. لأفضل أداء مع {= profile.gpu.vram | fallback("your") =} VRAM، إعداد `OLLAMA_MAX_LOADED_MODELS` أدناه يجب أن يتطابق مع عدد النماذج التي تتسع في VRAM الخاص بك في نفس الوقت.
{? endif ?}

#### ضبط متغيرات البيئة

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

#### إنشاء Modelfile لحمل العمل الخاص بك

بدلًا من استخدام إعدادات النموذج الافتراضية، أنشئ Modelfile مخصص مُعدّ لحمل عمل الدخل الخاص بك:

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

#### الدفعات وإدارة قائمة الانتظار

لأحمال عمل الدخل، ستحتاج غالبًا لمعالجة عناصر كثيرة. إليك إعداد دفعات أساسي:

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

### قياس أداء جهازك

لا تثق بقياسات أداء أي شخص آخر. قِس بنفسك:

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

دوّن عدد الرموز/ثانية لكل نموذج. هذا الرقم يحدد أي سير عمل دخل عملي لجهازك.

{@ insight stack_fit @}

**متطلبات السرعة حسب حالة الاستخدام:**
- المعالجة الدفعية (غير متزامنة): 5+ رمز/ثانية كافية (لا يهمك زمن الاستجابة)
- الأدوات التفاعلية (المستخدم ينتظر): 20+ رمز/ثانية كحد أدنى
- API فوري (موجه للعملاء): 30+ رمز/ثانية لتجربة مستخدم جيدة
- الدردشة المتدفقة: 15+ رمز/ثانية تبدو سريعة الاستجابة

### تأمين خادم الاستدلال المحلي

{? if computed.os_family == "windows" ?}
> **ملاحظة Windows:** Ollama على Windows يرتبط بـ localhost افتراضيًا. تحقق بـ `netstat -an | findstr 11434` في PowerShell. استخدم Windows Firewall لحظر الوصول الخارجي للمنفذ 11434.
{? elif computed.os_family == "macos" ?}
> **ملاحظة macOS:** Ollama على macOS يرتبط بـ localhost افتراضيًا. تحقق بـ `lsof -i :11434`. جدار حماية macOS يحظر الاتصالات الخارجية تلقائيًا.
{? endif ?}

يجب ألا يكون مثيل Ollama الخاص بك متاحًا من الإنترنت إلا إذا كنت تنوي ذلك صراحة.

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

> **خطأ شائع:** ربط Ollama بـ 0.0.0.0 "للراحة" ونسيان الأمر. أي شخص يجد عنوان IP الخاص بك يمكنه استخدام GPU الخاص بك للاستدلال المجاني. أسوأ من ذلك، يمكنه استخراج أوزان النموذج وموجهات النظام. دائمًا localhost. دائمًا أنفاق.

### نقطة تفتيش الدرس 2

يجب أن يكون لديك الآن:
- [ ] Ollama مثبت ويعمل
- [ ] نموذج عمل واحد على الأقل محمّل (llama3.1:8b أو ما يعادله)
- [ ] Modelfile مخصص لحمل العمل المتوقع
- [ ] أرقام قياس الأداء: رمز/ثانية لكل نموذج على جهازك
- [ ] Ollama مرتبط بـ localhost فقط

*في دورة STREETS الكاملة، الوحدة T (الخنادق التقنية) تريك كيف تبني تكوينات نماذج ملكية، خطوط أنابيب ضبط دقيق، وسلاسل أدوات مخصصة يصعب على المنافسين تكرارها. الوحدة R (محركات الدخل) تعطيك الخدمات المحددة لبنائها فوق هذا المكدس.*

---

## الدرس 3: ميزة الخصوصية

*"إعدادك الخاص هو ميزة تنافسية — وليس مجرد تفضيل."*

### الخصوصية ميزة منتج، وليست قيدًا

معظم المطورين يبنون بنية تحتية محلية لأنهم شخصيًا يقدرون الخصوصية، أو لأنهم يستمتعون بالعبث التقني. هذا جيد. لكنك تترك أموالًا على الطاولة إذا لم تدرك أن **الخصوصية واحدة من أكثر الميزات قابلية للتسويق في التقنية الآن.**

إليك السبب: في كل مرة ترسل شركة بيانات إلى API الخاص بـ OpenAI، تمر هذه البيانات عبر طرف ثالث. بالنسبة للكثير من الشركات — خاصة في الرعاية الصحية والمالية والقانون والحكومة والشركات الأوروبية — هذه مشكلة حقيقية. ليست نظرية. مشكلة "لا نستطيع استخدام هذه الأداة لأن الامتثال قال لا".

أنت، الذي يشغل النماذج محليًا على جهازه، ليس لديك هذه المشكلة.

### الرياح التنظيمية المواتية

البيئة التنظيمية تتحرك في اتجاهك. بسرعة.

{? if regional.country == "US" ?}
> **مقيم في الولايات المتحدة:** اللوائح الأكثر أهمية لك هي HIPAA و SOC 2 و ITAR وقوانين الخصوصية على مستوى الولايات (California CCPA، إلخ.). لوائح الاتحاد الأوروبي لا تزال مهمة — تؤثر على قدرتك على خدمة العملاء الأوروبيين، وهو سوق مربح.
{? elif regional.country == "GB" ?}
> **مقيم في المملكة المتحدة:** بعد Brexit، لدى المملكة المتحدة إطار حماية بيانات خاص (UK GDPR + Data Protection Act 2018). ميزة المعالجة المحلية قوية بشكل خاص لخدمة الخدمات المالية البريطانية والعمل المرتبط بـ NHS.
{? elif regional.country == "DE" ?}
> **مقيم في ألمانيا:** أنت في واحدة من أشد بيئات حماية البيانات صرامة في العالم. هذه *ميزة* — العملاء الألمان يفهمون بالفعل لماذا المعالجة المحلية مهمة، وسيدفعون مقابلها.
{? elif regional.country == "AU" ?}
> **مقيم في أستراليا:** قانون الخصوصية 1988 ومبادئ الخصوصية الأسترالية (APPs) تحكم التزاماتك. المعالجة المحلية نقطة بيع قوية للعملاء الحكوميين والرعاية الصحية تحت قانون My Health Records.
{? endif ?}

**قانون الذكاء الاصطناعي للاتحاد الأوروبي (يُطبق 2024-2026):**
- أنظمة الذكاء الاصطناعي عالية المخاطر تحتاج خطوط معالجة بيانات موثقة
- يجب على الشركات إثبات أين تتدفق البيانات ومن يعالجها
- المعالجة المحلية تبسط الامتثال بشكل كبير
- الشركات الأوروبية تبحث بنشاط عن مزودي خدمات ذكاء اصطناعي يمكنهم ضمان إقامة البيانات في الاتحاد الأوروبي

**GDPR (مُطبق بالفعل):**
- "معالجة البيانات" تشمل إرسال النص إلى API الخاص بـ LLM
- الشركات تحتاج اتفاقيات معالجة بيانات مع كل طرف ثالث
- المعالجة المحلية تلغي الطرف الثالث تمامًا
- هذه نقطة بيع حقيقية: "بياناتك لا تغادر بنيتك التحتية أبدًا. لا يوجد اتفاقية معالجة بيانات مع طرف ثالث للتفاوض عليها."

**لوائح خاصة بالصناعة:**
- **HIPAA (الرعاية الصحية الأمريكية):** لا يمكن إرسال بيانات المرضى إلى API الذكاء الاصطناعي الاستهلاكي بدون BAA (اتفاقية شريك تجاري). معظم مزودي الذكاء الاصطناعي لا يقدمون BAA للوصول عبر API. المعالجة المحلية تتجاوز هذا تمامًا.
- **SOC 2 (المؤسسات):** الشركات التي تخضع لتدقيق SOC 2 تحتاج لتوثيق كل معالج بيانات. معالجون أقل = تدقيقات أسهل.
- **ITAR (الدفاع الأمريكي):** البيانات التقنية الخاضعة للرقابة لا يمكن أن تغادر الولاية القضائية الأمريكية. مزودو الذكاء الاصطناعي السحابيون ذوو البنية التحتية الدولية يمثلون مشكلة.
- **PCI DSS (المالية):** معالجة بيانات حامل البطاقة لها متطلبات صارمة حول مسار البيانات.

### كيف تضع الخصوصية في محادثات المبيعات

لا تحتاج أن تكون خبيرًا في الامتثال. تحتاج أن تفهم ثلاث عبارات وتعرف متى تستخدمها:

**العبارة 1: "بياناتك لا تغادر بنيتك التحتية أبدًا."**
استخدمها عند: الحديث مع أي عميل محتمل يهتم بالخصوصية. هذا هو الخطاف العالمي.

**العبارة 2: "لا حاجة لاتفاقية معالجة بيانات مع طرف ثالث."**
استخدمها عند: الحديث مع شركات أوروبية أو أي شركة لديها فريق قانوني/امتثال. هذا يوفر لهم أسابيع من المراجعة القانونية.

**العبارة 3: "مسار تدقيق كامل، معالجة بمستأجر واحد."**
استخدمها عند: الحديث مع المؤسسات أو الصناعات المنظمة. يحتاجون لإثبات خط أنابيب الذكاء الاصطناعي للمدققين.

**مثال على الموضعية (لصفحة خدمتك أو عروضك):**

> "بخلاف خدمات الذكاء الاصطناعي السحابية، [خدمتك] تعالج جميع البيانات محليًا على عتاد مخصص. مستنداتك وكودك وبياناتك لا تغادر بيئة المعالجة أبدًا. لا يوجد API لطرف ثالث في خط الأنابيب، لا اتفاقيات مشاركة بيانات للتفاوض عليها، وتسجيل تدقيق كامل لكل عملية. هذا يجعل [خدمتك] مناسبة للمؤسسات ذات متطلبات معالجة البيانات الصارمة، بما في ذلك بيئات الامتثال لـ GDPR و HIPAA و SOC 2."

هذه الفقرة، على صفحة هبوط، ستجذب بالضبط العملاء الذين سيدفعون أسعارًا مميزة.

### مبرر التسعير المميز

إليك الحجة التجارية بأرقام صلبة:

**خدمة معالجة ذكاء اصطناعي قياسية (باستخدام API السحابية):**
- بيانات العميل تذهب إلى OpenAI/Anthropic/Google
- أنت تنافس كل مطور يستطيع استدعاء API
- سعر السوق: $0.01-0.05 لكل مستند معالج
- أنت في الأساس تعيد بيع الوصول لـ API بهامش

**خدمة معالجة ذكاء اصطناعي أولوية الخصوصية (مكدسك المحلي):**
- بيانات العميل تبقى على جهازك
- أنت تنافس مجموعة أصغر بكثير من المزودين
- سعر السوق: $0.10-0.50 لكل مستند معالج (علاوة 5-10 أضعاف)
- أنت تبيع بنية تحتية + خبرة + امتثال

علاوة الخصوصية حقيقية: **من 5 إلى 10 أضعاف** فوق خدمات السحابة السلعية لنفس المهمة الأساسية. والعملاء الذين يدفعونها أكثر ولاءً، أقل حساسية للسعر، وميزانياتهم أكبر.

{@ insight competitive_position @}

### إعداد مساحات عمل معزولة

إذا كنت تعمل بوظيفة يومية (معظمكم كذلك)، تحتاج فصلًا نظيفًا بين عمل صاحب العمل وعمل الدخل. هذا ليس فقط حماية قانونية — إنه نظافة تشغيلية.

{? if computed.os_family == "windows" ?}
> **نصيحة Windows:** أنشئ حساب مستخدم Windows منفصل لعمل الدخل (Settings > Accounts > Family & other users > Add someone else). هذا يعطيك بيئة معزولة تمامًا — ملفات تعريف متصفح منفصلة، مسارات ملفات منفصلة، متغيرات بيئة منفصلة. بدّل بين الحسابات بـ Win+L.
{? endif ?}

**الخيار 1: حسابات مستخدم منفصلة (موصى به)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**الخيار 2: مساحات عمل محتواة**

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

**الخيار 3: جهاز فعلي منفصل (الأكثر صلابة)**

إذا كنت جادًا بشأن هذا ودخلك يبرره، جهاز مخصص يزيل كل التساؤلات. جهاز Dell OptiPlex مستعمل مع RTX 3060 يكلف $400-600 ويسترد تكلفته في أول شهر من عمل العملاء.

**قائمة الحد الأدنى للفصل:**
- [ ] مشاريع الدخل في مجلد منفصل (لا تخلط أبدًا مع مستودعات صاحب العمل)
- [ ] مفاتيح API منفصلة لعمل الدخل (لا تستخدم أبدًا مفاتيح صاحب العمل)
- [ ] ملف تعريف متصفح منفصل لحسابات الدخل
- [ ] عمل الدخل لا يُنفذ أبدًا على عتاد صاحب العمل
- [ ] عمل الدخل لا يُنفذ أبدًا على شبكة صاحب العمل (استخدم إنترنتك الشخصي أو VPN)
- [ ] حساب GitHub/GitLab منفصل لمشاريع الدخل (اختياري لكن نظيف)

> **خطأ شائع:** استخدام مفتاح API الخاص بـ OpenAI من صاحب العمل "فقط للاختبار" في مشروعك الجانبي. هذا ينشئ أثرًا ورقيًا يمكن لمصاحب العمل رؤيته في لوحة الفواتير، ويعكر مياه الملكية الفكرية. احصل على مفاتيحك الخاصة. إنها رخيصة.

### نقطة تفتيش الدرس 3

يجب أن تفهم الآن:
- [ ] لماذا الخصوصية ميزة منتج قابلة للتسويق، وليست مجرد تفضيل شخصي
- [ ] أي اللوائح تخلق طلبًا على معالجة الذكاء الاصطناعي المحلية
- [ ] ثلاث عبارات لاستخدامها في محادثات المبيعات حول الخصوصية
- [ ] كيف تحصل خدمات الخصوصية أولًا على علاوة تسعير 5-10 أضعاف
- [ ] كيف تفصل عمل الدخل عن عمل صاحب العمل

*في دورة STREETS الكاملة، الوحدة E (الحافة المتطورة) تعلمك كيف تتابع التغييرات التنظيمية وتضع نفسك أمام متطلبات الامتثال الجديدة قبل أن يعرف منافسوك أنها موجودة.*

---

## الدرس 4: الحد الأدنى القانوني

*"خمس عشرة دقيقة من الإعداد القانوني الآن تمنع أشهرًا من المشاكل لاحقًا."*

### هذه ليست استشارة قانونية

أنا مطور، لست محاميًا. ما يلي هو قائمة مراجعة عملية يجب على معظم المطورين في معظم الحالات معالجتها. إذا كان وضعك معقدًا (أسهم في صاحب العمل، عدم منافسة بشروط محددة، إلخ.)، أنفق $200 على استشارة 30 دقيقة مع محامي عمل. إنه أفضل عائد على الاستثمار ستحصل عليه.

### الخطوة 1: اقرأ عقد عملك

ابحث عن عقد العمل أو خطاب العرض. ابحث عن هذه الأقسام:

**بند التنازل عن الملكية الفكرية** — ابحث عن عبارات مثل:
- "جميع الاختراعات والتطويرات ومنتجات العمل..."
- "...المُنشأة خلال فترة التوظيف..."
- "...المتعلقة بأعمال الشركة أو أعمالها المتوقعة..."

**عبارات رئيسية تقيدك:**
- "جميع منتجات العمل المُنشأة خلال التوظيف ملك للشركة" (واسعة — قد تكون إشكالية)
- "منتجات العمل المُنشأة باستخدام موارد الشركة" (أضيق — عادة لا بأس إذا استخدمت معداتك الخاصة)
- "المتعلقة بأعمال الشركة الحالية أو المتوقعة" (يعتمد على ماذا يفعل صاحب العمل)

**عبارات رئيسية تحررك:**
- "باستثناء العمل المنجز بالكامل في وقت الموظف الخاص بموارده الخاصة وغير المتعلق بأعمال الشركة" (هذا هو استثناؤك — العديد من الولايات الأمريكية تتطلب هذا)
- بعض الولايات (كاليفورنيا، واشنطن، مينيسوتا، إلينوي، وأخرى) لديها قوانين تحد من مطالبات صاحب العمل بالملكية الفكرية على المشاريع الشخصية، بغض النظر عما يقوله العقد.

### اختبار الأسئلة الثلاثة

لأي مشروع دخل، اسأل:

1. **الوقت:** هل تنجز هذا العمل في وقتك الخاص؟ (ليس خلال ساعات العمل، ليس خلال نوبات الطوارئ)
2. **المعدات:** هل تستخدم عتادك الخاص، إنترنتك الخاص، مفاتيح API الخاصة بك؟ (ليس حاسوب صاحب العمل، ليس VPN صاحب العمل، ليس حسابات سحاب صاحب العمل)
3. **الموضوع:** هل هذا غير متعلق بأعمال صاحب العمل؟ (إذا كنت تعمل في شركة ذكاء اصطناعي للرعاية الصحية وتريد بيع خدمات ذكاء اصطناعي للرعاية الصحية... هذه مشكلة. إذا كنت تعمل في شركة ذكاء اصطناعي للرعاية الصحية وتريد بيع معالجة مستندات لوكلاء العقارات... هذا مقبول.)

إذا كانت جميع الإجابات الثلاث نظيفة، فأنت على الأرجح بخير. إذا كانت أي إجابة غامضة، احصل على وضوح قبل المتابعة.

> **كلام صريح:** الغالبية العظمى من المطورين الذين يعملون عملًا جانبيًا لا يواجهون أي مشكلة أبدًا. أصحاب العمل يهتمون بحماية المزايا التنافسية، وليس منعك من كسب أموال إضافية في مشاريع غير متعلقة. لكن "على الأرجح بخير" ليس "بالتأكيد بخير." إذا كان عقدك واسعًا بشكل غير عادي، أجرِ محادثة مع مديرك أو الموارد البشرية — أو استشر محاميًا. الجانب السلبي لعدم التحقق أسوأ بكثير من الإحراج البسيط للسؤال.

### الخطوة 2: اختر هيكلًا تجاريًا

تحتاج كيانًا قانونيًا لفصل أصولك الشخصية عن أنشطتك التجارية، ولفتح الباب أمام الخدمات المصرفية التجارية ومعالجة الدفع والمزايا الضريبية.

{? if regional.country ?}
> **موقعك: {= regional.country | fallback("Unknown") =}.** نوع الكيان الموصى به لمنطقتك هو **{= regional.business_entity_type | fallback("LLC or equivalent") =}**، بتكلفة تسجيل نموذجية {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. انتقل إلى قسم بلدك أدناه، أو اقرأ جميع الأقسام لفهم كيف يعمل العملاء في المناطق الأخرى.
{? endif ?}

{? if regional.country == "US" ?}
#### الولايات المتحدة (منطقتك)
{? else ?}
#### الولايات المتحدة
{? endif ?}

| الهيكل | التكلفة | الحماية | الأفضل لـ |
|--------|---------|---------|----------|
| **Sole Proprietorship** (افتراضي) | $0 | لا شيء (مسؤولية شخصية) | اختبار المياه. أول $1K. |
| **Single-Member LLC** | $50-500 (يختلف حسب الولاية) | حماية الأصول الشخصية | عمل الدخل النشط. معظم المطورين يبدأون هنا. |
| **انتخاب S-Corp** (على LLC) | تكلفة LLC + $0 للانتخاب | نفس LLC + مزايا ضريبة الرواتب | عندما تكسب باستمرار $40K+/سنة من هذا |

**موصى به للمطورين الأمريكيين:** Single-Member LLC في ولاية إقامتك.

**أرخص الولايات للتأسيس:** Wyoming ($100، بدون ضريبة دخل ولاية)، New Mexico ($50)، Montana ($70). لكن التأسيس في ولاية إقامتك عادة الأبسط ما لم يكن لديك سبب محدد لعدم القيام بذلك.

**كيفية التقديم:**
1. اذهب إلى موقع وزير الخارجية في ولايتك
2. ابحث عن "form LLC" أو "business entity filing"
3. قدم Articles of Organization (نموذج 10 دقائق)
4. احصل على EIN من IRS (مجاني، يستغرق 5 دقائق على irs.gov)

{? if regional.country == "GB" ?}
#### المملكة المتحدة (منطقتك)
{? else ?}
#### المملكة المتحدة
{? endif ?}

| الهيكل | التكلفة | الحماية | الأفضل لـ |
|--------|---------|---------|----------|
| **Sole Trader** | مجاني (تسجيل مع HMRC) | لا شيء | أول دخل. اختبار. |
| **Limited Company (Ltd)** | ~$15 عبر Companies House | حماية الأصول الشخصية | أي عمل دخل جدي. |

**موصى به:** شركة Ltd عبر Companies House. يستغرق حوالي 20 دقيقة ويكلف GBP 12.

#### الاتحاد الأوروبي

يختلف بشكل كبير حسب البلد، لكن النمط العام:

- **ألمانيا:** Einzelunternehmer (تاجر فردي) للبداية، GmbH للعمل الجدي (لكن GmbH يتطلب EUR 25,000 رأس مال — فكر في UG بـ EUR 1)
- **هولندا:** Eenmanszaak (تاجر فردي، تسجيل مجاني) أو BV (مشابه لـ Ltd)
- **فرنسا:** Micro-entrepreneur (مبسط، موصى به للبداية)
- **إستونيا:** e-Residency + OUE (شائع لغير المقيمين، بالكامل عبر الإنترنت)

{? if regional.country == "AU" ?}
#### أستراليا (منطقتك)
{? else ?}
#### أستراليا
{? endif ?}

| الهيكل | التكلفة | الحماية | الأفضل لـ |
|--------|---------|---------|----------|
| **Sole Trader** | ABN مجاني | لا شيء | البداية |
| **Pty Ltd** | ~AUD 500-800 عبر ASIC | حماية الأصول الشخصية | دخل جدي |

**موصى به:** ابدأ بـ Sole Trader ABN (مجاني، فوري)، انتقل إلى Pty Ltd عندما تكسب باستمرار.

### الخطوة 3: معالجة الدفع (إعداد 15 دقيقة)

تحتاج طريقة للحصول على الأموال. اضبط هذا الآن، وليس عندما يكون أول عميل ينتظر.

{? if regional.payment_processors ?}
> **موصى به لـ {= regional.country | fallback("your region") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (موصى به لمعظم المطورين):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

الوقت: ~15 دقيقة. يمكنك البدء في قبول المدفوعات فورًا (Stripe يحتجز الأموال لمدة 7 أيام على الحسابات الجديدة).

**Lemon Squeezy (موصى به للمنتجات الرقمية):**

إذا كنت تبيع منتجات رقمية (قوالب، أدوات، دورات، SaaS)، Lemon Squeezy يعمل كتاجر السجل الخاص بك. هذا يعني:
- يتولون ضريبة المبيعات وVAT وGST نيابة عنك عالميًا
- لا تحتاج للتسجيل في VAT في الاتحاد الأوروبي
- يتولون الاسترداد والنزاعات

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (للمطورين الدوليين أو الراغبين في كيان أمريكي):**

إذا كنت خارج الولايات المتحدة لكن تريد البيع لعملاء أمريكيين بكيان أمريكي:
- $500 رسوم لمرة واحدة
- ينشئ Delaware LLC لك
- يفتح حساب مصرفي أمريكي (عبر Mercury أو Stripe)
- يوفر خدمة وكيل مسجل
- يستغرق حوالي 1-2 أسبوع

### الخطوة 4: سياسة الخصوصية وشروط الخدمة

إذا كنت تبيع أي خدمة أو منتج عبر الإنترنت، تحتاج هذه. لا تدفع لمحامٍ مقابل نماذج جاهزة.

**مصادر مجانية وموثوقة للقوالب:**
- **Termly.io** — مولد سياسة خصوصية وشروط خدمة مجاني. أجب على الأسئلة، احصل على المستندات.
- **Avodocs.com** — مستندات قانونية مفتوحة المصدر للشركات الناشئة. مجاني.
- **GitHub's choosealicense.com** — لتراخيص المشاريع مفتوحة المصدر تحديدًا.
- **سياسات Basecamp مفتوحة المصدر** — ابحث عن "Basecamp open source policies" — قوالب جيدة بلغة واضحة.

**ما يجب أن تغطيه سياسة الخصوصية (إذا كنت تعالج أي بيانات عميل):**
- ما البيانات التي تجمعها
- كيف تعالجها (محليًا — هذه ميزتك)
- كم تحتفظ بها
- كيف يمكن للعملاء طلب الحذف
- هل يصل أي طرف ثالث للبيانات (مثاليًا: لا أحد)

**الوقت:** 30 دقيقة مع مولد قوالب. انتهى.

### الخطوة 5: حساب مصرفي منفصل

لا تمرر دخل الأعمال عبر حسابك الشخصي. الأسباب:

1. **وضوح ضريبي:** عندما يحين وقت الضرائب، تحتاج أن تعرف بالضبط ما كان دخل أعمال وما لم يكن.
2. **حماية قانونية:** إذا كان لديك LLC، خلط الأموال الشخصية والتجارية يمكن أن "يخترق حجاب الشركة" — مما يعني أن المحكمة يمكنها تجاهل حماية المسؤولية لـ LLC الخاص بك.
3. **الاحترافية:** فواتير من "شركة جون للاستشارات" تصل إلى حساب تجاري مخصص تبدو شرعية. المدفوعات لحسابك الشخصي على Venmo لا تبدو كذلك.

**خدمات مصرفية تجارية مجانية أو منخفضة التكلفة:**
{? if regional.country == "US" ?}
- **Mercury** (موصى به لك) — مجاني، مصمم للشركات الناشئة. API ممتاز إذا أردت أتمتة المحاسبة لاحقًا.
- **Relay** — مجاني، جيد لفصل مصادر الدخل إلى حسابات فرعية.
{? elif regional.country == "GB" ?}
- **Starling Bank** (موصى به لك) — حساب تجاري مجاني، إعداد فوري.
- **Wise Business** — متعدد العملات منخفض التكلفة. رائع إذا كنت تخدم عملاء دوليين.
{? else ?}
- **Mercury** (الولايات المتحدة) — مجاني، مصمم للشركات الناشئة. API ممتاز إذا أردت أتمتة المحاسبة لاحقًا.
- **Relay** (الولايات المتحدة) — مجاني، جيد لفصل مصادر الدخل إلى حسابات فرعية.
- **Starling Bank** (المملكة المتحدة) — حساب تجاري مجاني.
{? endif ?}
- **Wise Business** (دولي) — متعدد العملات منخفض التكلفة. رائع لاستلام المدفوعات بالدولار واليورو والجنيه الإسترليني وغيرها.
- **Qonto** (الاتحاد الأوروبي) — خدمات مصرفية تجارية نظيفة للشركات الأوروبية.

افتح الحساب الآن. يستغرق 10-15 دقيقة عبر الإنترنت و1-3 أيام للتحقق.

### الخطوة 6: أساسيات الضرائب لدخل المطور الجانبي

{? if regional.tax_note ?}
> **ملاحظة ضريبية لـ {= regional.country | fallback("your region") =}:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **كلام صريح:** الضرائب هي الشيء الذي يتجاهله معظم المطورين حتى أبريل، ثم يصابون بالذعر. قضاء 30 دقيقة الآن يوفر لك أموالًا وتوترًا حقيقيًا.

**الولايات المتحدة:**
- الدخل الجانبي فوق $400/سنة يتطلب ضريبة العمل الحر (~15.3% للضمان الاجتماعي + Medicare)
- بالإضافة إلى شريحة ضريبة الدخل العادية على صافي الربح
- **مدفوعات ضريبية ربع سنوية مقدرة:** إذا كنت ستدين بأكثر من $1,000 في الضرائب، تتوقع IRS مدفوعات ربع سنوية (15 أبريل، 15 يونيو، 15 سبتمبر، 15 يناير). التأخر يترتب عليه عقوبات.
- اجعل **25-30%** من صافي الدخل جانبًا للضرائب. ضعها في حساب توفير منفصل فورًا.

**خصومات شائعة لدخل المطور الجانبي:**
- تكاليف API (OpenAI، Anthropic، إلخ.) — قابلة للخصم 100%
- مشتريات العتاد المستخدم للأعمال — قابلة للإهلاك أو خصم Section 179
- تكلفة الكهرباء المنسوبة للاستخدام التجاري
- اشتراكات البرمجيات المستخدمة لعمل الدخل
- خصم المكتب المنزلي (مبسط: $5/قدم مربع، حتى 300 قدم مربع = $1,500)
- الإنترنت (نسبة الاستخدام التجاري)
- أسماء النطاقات، الاستضافة، خدمات البريد الإلكتروني
- التطوير المهني (دورات، كتب) المتعلقة بعمل الدخل

**المملكة المتحدة:**
- أبلغ عبر إقرار ضريبي Self Assessment
- دخل التجارة تحت GBP 1,000: معفي من الضرائب (Trading Allowance)
- فوق ذلك: ادفع ضريبة الدخل + Class 4 NICs على الأرباح
- تواريخ الدفع: 31 يناير و31 يوليو

**تتبع كل شيء من اليوم الأول.** استخدم جدول بيانات بسيط إذا لم يكن هناك بديل:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **خطأ شائع:** "سأحسب الضرائب لاحقًا." لاحقًا يعني الربع الرابع، تدين بـ $3,000 في الضرائب المقدرة مع عقوبات، وقد أنفقت المال. أتمت العملية: في كل مرة يصل دخل لحسابك التجاري، حوّل 30% إلى حساب توفير ضريبي فورًا.

### نقطة تفتيش الدرس 4

يجب أن يكون لديك الآن (أو خطة لـ):
- [ ] قراءة بند الملكية الفكرية في عقد العمل
- [ ] اجتياز اختبار الأسئلة الثلاثة لعمل الدخل المخطط
- [ ] اختيار هيكل تجاري (أو قرار البدء كتاجر فردي)
- [ ] إعداد معالجة الدفع (Stripe أو Lemon Squeezy)
- [ ] سياسة خصوصية وشروط خدمة من مولد قوالب
- [ ] حساب مصرفي تجاري منفصل (أو طلب مقدم)
- [ ] استراتيجية ضريبية: تخصيص 30% + جدول مدفوعات ربع سنوية

*في دورة STREETS الكاملة، الوحدة E (دليل التنفيذ) تتضمن قوالب نمذجة مالية تحسب تلقائيًا التزاماتك الضريبية، ربحية المشروع، ونقاط التعادل لكل محرك دخل.*

---

## الدرس 5: ميزانية {= regional.currency_symbol | fallback("$") =}200/شهر

*"عملك له معدل حرق. اعرفه. تحكم فيه. اجعله يكسب."*

### لماذا {= regional.currency_symbol | fallback("$") =}200/شهر

مئتا {= regional.currency | fallback("dollars") =} شهريًا هي الميزانية الدنيا القابلة للتطبيق لعملية دخل مطور. إنها كافية لتشغيل خدمات حقيقية، خدمة عملاء حقيقيين، وتوليد إيرادات حقيقية. وهي أيضًا صغيرة بما يكفي بحيث إذا لم ينجح شيء، لم تراهن بكل شيء.

الهدف بسيط: **حوّل {= regional.currency_symbol | fallback("$") =}200/شهر إلى {= regional.currency_symbol | fallback("$") =}600+/شهر خلال 90 يومًا.** إذا استطعت ذلك، لديك عمل تجاري. إذا لم تستطع، غيّر الاستراتيجية — وليس الميزانية.

### تفصيل الميزانية

#### المستوى 1: أرصدة API — $50-100/شهر

هذه هي قدرتك الحاسوبية الإنتاجية للجودة الموجهة للعملاء.

**التخصيص المبدئي الموصى به:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**كيفية إدارة إنفاق API:**

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

**استراتيجية الإنفاق الهجينة:**
- استخدم LLM المحلي لـ 80% من المعالجة (التصنيف، الاستخراج، التلخيص، المسودات)
- استخدم استدعاءات API لـ 20% من المعالجة (مراجعة الجودة النهائية، الاستنتاج المعقد، المخرجات الموجهة للعملاء)
- تكلفتك الفعالة لكل مهمة تنخفض بشكل كبير مقارنة بالاستخدام الصرف لـ API

{? if computed.monthly_electricity_estimate ?}
> **تكلفة الكهرباء المقدرة لديك:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/شهر للتشغيل 24/7 بسعر {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. هذا محسوب بالفعل في تكلفة التشغيل الفعالة.
{? endif ?}

#### المستوى 2: البنية التحتية — {= regional.currency_symbol | fallback("$") =}30-50/شهر

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**هل تحتاج VPS؟**

إذا كان نموذج دخلك:
- **بيع منتجات رقمية:** لا. استضف على Vercel/Netlify مجانًا. استخدم Lemon Squeezy للتسليم.
- **تشغيل معالجة غير متزامنة للعملاء:** ربما. يمكنك تشغيل المهام على جهازك المحلي وتسليم النتائج. VPS يضيف الموثوقية.
- **تقديم خدمة API:** نعم، على الأرجح. VPS بـ $5-10 يعمل كبوابة API خفيفة، حتى لو حدثت المعالجة الثقيلة على جهازك المحلي.
- **بيع SaaS:** نعم. لكن ابدأ بأرخص مستوى وتوسع تدريجيًا.

**البنية التحتية المبدئية الموصى بها:**

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

إجمالي تكلفة البنية التحتية: $5-20/شهر. الباقي مستويات مجانية.

#### المستوى 3: الأدوات — {= regional.currency_symbol | fallback("$") =}20-30/شهر

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

> **كلام صريح:** يمكنك تشغيل مكدس أدواتك بالكامل على المستويات المجانية عند البداية. المبلغ $20-30 المخصص هنا لعندما تتجاوز المستويات المجانية أو تريد ميزة مميزة محددة. لا تنفقها فقط لأنها في الميزانية. الميزانية غير المنفقة هي ربح.

#### المستوى 4: الاحتياطي — {= regional.currency_symbol | fallback("$") =}0-30/شهر

هذا صندوق "الأشياء التي لم أتوقعها":
- ارتفاع مفاجئ في تكلفة API من مهمة دفعية كبيرة بشكل غير متوقع
- أداة تحتاجها لمشروع عميل محدد
- شراء نطاق طارئ عندما تجد الاسم المثالي
- شراء لمرة واحدة (قالب، مجموعة أيقونات)

إذا لم تستخدم الاحتياطي، يتراكم. بعد 3 أشهر من الاحتياطي غير المستخدم، فكر في إعادة التخصيص لأرصدة API أو البنية التحتية.

### حساب العائد على الاستثمار

هذا هو الرقم الوحيد المهم:

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

**متى تزيد الميزانية:**

زد ميزانيتك فقط عندما:
1. تحقق باستمرار 2x+ عائد استثمار لشهرين أو أكثر
2. إنفاق أكثر سيزيد الإيرادات مباشرة (مثلًا: أرصدة API أكثر = قدرة أكبر على خدمة العملاء)
3. الزيادة مرتبطة بمصدر دخل محدد ومُختبر

**متى لا تزيد الميزانية:**
- "أعتقد أن هذه الأداة الجديدة ستساعد" (جرب البدائل المجانية أولًا)
- "الجميع يقول يجب أن تنفق مالًا لتكسب مالًا" (ليس في هذه المرحلة)
- "VPS أكبر سيجعل خدمتي أسرع" (هل السرعة هي فعلًا نقطة الاختناق؟)
- لم تصل إلى 1x عائد استثمار بعد (أصلح الإيرادات، ليس الإنفاق)

**سلم التوسع:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **خطأ شائع:** التعامل مع {= regional.currency_symbol | fallback("$") =}200 كـ "استثمار" لا يحتاج لعائد فوري. لا. هذه تجربة بموعد نهائي 90 يومًا. إذا لم تولد {= regional.currency_symbol | fallback("$") =}200/شهر إيرادات {= regional.currency_symbol | fallback("$") =}200/شهر خلال 90 يومًا، شيء ما في الاستراتيجية يحتاج للتغيير. المال، السوق، العرض — شيء لا يعمل. كن صادقًا مع نفسك.

### نقطة تفتيش الدرس 5

يجب أن يكون لديك الآن:
- [ ] ميزانية شهرية بحوالي $200 موزعة على أربعة مستويات
- [ ] حسابات API منشأة مع حدود إنفاق مضبوطة
- [ ] قرارات البنية التحتية متخذة (محلي فقط مقابل محلي + VPS)
- [ ] مكدس أدوات مختار (معظمه مستويات مجانية للبداية)
- [ ] أهداف العائد على الاستثمار: 3x خلال 90 يومًا
- [ ] قاعدة واضحة: زد الميزانية فقط بعد إثبات العائد على الاستثمار

*في دورة STREETS الكاملة، الوحدة E (دليل التنفيذ) تتضمن قالب لوحة معلومات مالية يتتبع إنفاقك وإيراداتك وعائد الاستثمار لكل محرك دخل في الوقت الفعلي — حتى تعرف دائمًا أي المصادر مربحة وأيها يحتاج تعديل.*

---

## الدرس 6: وثيقة المكدس السيادي

*"كل عمل تجاري لديه خطة. هذه خطتك — وتتسع في صفحتين."*

### المُخرَج

هذا هو أهم شيء ستنشئه في الوحدة S. وثيقة المكدس السيادي هي مرجع واحد يلتقط كل شيء عن بنيتك التحتية المدرة للدخل. ستراجعها طوال بقية دورة STREETS، تحدثها مع تطور إعدادك، وتستخدمها لاتخاذ قرارات واضحة حول ما تبنيه وما تتخطاه.

أنشئ ملفًا جديدًا. Markdown أو Google Doc أو صفحة Notion أو نص عادي — أيًا كان ما ستحافظ عليه فعلًا. استخدم القالب أدناه، واملأ كل حقل بالأرقام والقرارات من الدروس 1-5.

### القالب

{? if computed.profile_completeness != "0" ?}
> **بداية متقدمة:** 4DA اكتشفت بالفعل بعض مواصفات عتادك ومعلومات مكدسك. ابحث عن التلميحات المملوءة مسبقًا أدناه — ستوفر لك وقتًا في ملء القالب.
{? endif ?}

انسخ هذا القالب بالكامل واملأه. كل حقل. بدون تخطي.

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
(Based on the full STREETS framework — fill this in as you progress through the playbook)
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
> **ملء مسبق من Developer DNA الخاص بك:**
> - **المكدس الأساسي:** {= dna.primary_stack | fallback("Not detected") =}
> - **الاهتمامات:** {= dna.interests | fallback("Not detected") =}
> - **ملخص الهوية:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **نقاط عمياء يجب مراقبتها:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **ملء مسبق من المكدس المكتشف:** تقنياتك الأساسية هي {= stack.primary | fallback("not yet detected") =}. {? if stack.adjacent ?}مهارات مجاورة: {= stack.adjacent | fallback("none detected") =}.{? endif ?} استخدمها لملء جرد المهارات أعلاه.
{? endif ?}

{@ insight t_shape @}

### كيف تستخدم هذه الوثيقة

1. **قبل بدء أي مشروع جديد:** راجع مكدسك السيادي. هل لديك العتاد والوقت والمهارات والميزانية للتنفيذ؟
2. **قبل شراء أي شيء:** راجع تخصيص ميزانيتك. هل هذا الشراء في الخطة؟
3. **المراجعة الشهرية:** حدّث عمود "الفعلي" في ميزانيتك. حدّث أرقام الإيرادات. اضبط التخصيصات بناءً على ما يعمل.
4. **عندما يسألك أحد ماذا تفعل:** قسم "ما يمكنني تقديمه اليوم" هو عرضك الفوري.
5. **عندما تُغريك فكرة لامعة جديدة:** راجع قيودك. هل هذا يتناسب مع وقتك ومهاراتك وعتادك؟ إذا لا، أضفها إلى "ما أبنيه نحوه" للاحقًا.

### تمرين الساعة الواحدة

اضبط مؤقتًا على 60 دقيقة. املأ كل حقل في القالب. لا تفرط في التفكير. لا تبحث بشكل مكثف. اكتب ما تعرفه الآن. يمكنك التحديث لاحقًا.

الحقول التي لا تستطيع ملأها؟ تلك هي بنود العمل لهذا الأسبوع:
- أرقام قياس أداء فارغة؟ شغّل سكريبت القياس من الدرس 2.
- لا كيان تجاري؟ ابدأ عملية التسجيل من الدرس 4.
- لا معالجة دفع؟ اضبط Stripe من الدرس 4.
- جرد مهارات فارغ؟ اقضِ 15 دقيقة في سرد كل ما تقاضيت أجرًا عليه في السنوات الخمس الأخيرة.

> **خطأ شائع:** قضاء 3 ساعات في جعل الوثيقة "مثالية" بدل ساعة واحدة في جعلها "منجزة." وثيقة المكدس السيادي هي مرجع عمل، وليست خطة عمل للمستثمرين. لن يراها أحد غيرك. الدقة مهمة. التنسيق لا يهم.

### نقطة تفتيش الدرس 6

يجب أن يكون لديك الآن:
- [ ] وثيقة مكدس سيادي كاملة محفوظة في مكان ستفتحه فعلًا
- [ ] جميع الأقسام الستة مملوءة بأرقام حقيقية (وليس طموحية)
- [ ] قائمة واضحة ببنود العمل للفجوات في إعدادك
- [ ] تاريخ محدد لأول مراجعة شهرية (بعد 30 يومًا من الآن)

---

## الوحدة S: مكتملة

{? if progress.completed("MODULE_S") ?}
> **الوحدة S مكتملة.** أنهيت {= progress.completed_count | fallback("1") =} من {= progress.total_count | fallback("7") =} وحدات STREETS. {? if progress.completed_modules ?}المكتملة: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### ما بنيته في أسبوعين

انظر إلى ما تملكه الآن ولم تكن تملكه عندما بدأت:

1. **جرد عتاد** مربوط بقدرات توليد الدخل — وليس مجرد مواصفات على ملصق.
2. **مكدس LLM محلي بمستوى إنتاجي** مع Ollama، مُقاس على عتادك الفعلي، مضبوط لأحمال العمل الحقيقية.
3. **ميزة خصوصية** تفهم كيف تسوقها — بلغة محددة لجماهير محددة.
4. **أساس قانوني ومالي** — كيان تجاري (أو خطة)، معالجة دفع، حساب مصرفي، استراتيجية ضريبية.
5. **ميزانية مُتحكم بها** بأهداف عائد استثمار واضحة وموعد نهائي 90 يومًا لإثبات النموذج.
6. **وثيقة مكدس سيادي** تلتقط كل ما سبق في مرجع واحد ستستخدمه لكل قرار قادم.

هذا أكثر مما يعده معظم المطورين على الإطلاق. بجدية. معظم الأشخاص الذين يريدون تحقيق دخل جانبي يقفزون مباشرة إلى "بناء شيء رائع" ثم يتساءلون لماذا لا يستطيعون الحصول على أجر. أنت الآن تملك البنية التحتية للحصول على أجر.

لكن البنية التحتية بدون اتجاه هي مجرد هواية مكلفة. تحتاج أن تعرف أين توجه هذا المكدس.

{@ temporal market_timing @}

### ما يأتي بعد ذلك: الوحدة T — الخنادق التقنية

الوحدة S أعطتك الأساس. الوحدة T تجيب على السؤال الحاسم: **كيف تبني شيئًا لا يستطيع المنافسون نسخه بسهولة؟**

إليك ما تغطيه الوحدة T:

- **خطوط أنابيب بيانات ملكية** — كيف تنشئ مجموعات بيانات لا يملك الوصول إليها إلا أنت، بشكل قانوني وأخلاقي
- **تكوينات نماذج مخصصة** — الضبط الدقيق وهندسة الموجهات التي تنتج جودة مخرجات لا يستطيع الآخرون مطابقتها بالإعدادات الافتراضية
- **مكدسات مهارات متراكمة** — لماذا "Python + رعاية صحية" تتفوق على "Python + JavaScript" للدخل، وكيف تحدد تركيبتك الفريدة
- **حواجز دخول تقنية** — تصميمات بنية تحتية تتطلب من المنافس أشهرًا لتكرارها
- **تدقيق الخندق** — إطار عمل لتقييم ما إذا كان مشروعك يملك ميزة قابلة للدفاع عنها أم أنه مجرد خدمة سلعية أخرى

الفرق بين مطور يكسب $500/شهر وآخر يكسب $5,000/شهر نادرًا ما يكون المهارة. إنه الخنادق. أشياء تجعل عرضك صعب التكرار، حتى لو كان لدى شخص ما نفس العتاد ونفس النماذج.

### خريطة طريق STREETS الكاملة

| الوحدة | العنوان | التركيز | المدة |
|--------|---------|---------|-------|
| **S** | الإعداد السيادي | البنية التحتية، القانون، الميزانية | الأسبوعان 1-2 (مكتمل) |
| **T** | الخنادق التقنية | المزايا القابلة للدفاع عنها، الأصول الملكية | الأسبوعان 3-4 |
| **R** | محركات الدخل | أدلة تنفيذ محددة مع كود | الأسابيع 5-8 |
| **E** | دليل التنفيذ | تسلسلات الإطلاق، التسعير، العملاء الأوائل | الأسبوعان 9-10 |
| **E** | الحافة المتطورة | البقاء في المقدمة، اكتشاف الاتجاهات، التكيف | الأسبوعان 11-12 |
| **T** | الأتمتة التكتيكية | أتمتة العمليات للدخل السلبي | الأسبوعان 13-14 |
| **S** | تكديس المصادر | مصادر دخل متعددة، استراتيجية المحفظة | الأسبوعان 15-16 |

الوحدة R (محركات الدخل) هي حيث يُكسب معظم المال. لكن بدون S و T، أنت تبني على رمال.

---

**مستعد للدليل الكامل؟**

لقد رأيت الأساس. لقد بنيته بنفسك. الآن احصل على النظام الكامل.

**احصل على STREETS Core** — الدورة الكاملة لـ 16 أسبوعًا بجميع الوحدات السبع، قوالب كود محركات الدخل، لوحات المعلومات المالية، والمجتمع الخاص من المطورين الذين يبنون دخلًا بشروطهم الخاصة.

*جهازك. قواعدك. إيراداتك.*
