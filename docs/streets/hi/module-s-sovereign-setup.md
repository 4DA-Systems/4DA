# मॉड्यूल S: सॉवरेन सेटअप

**STREETS डेवलपर इनकम कोर्स — फ्री मॉड्यूल**
*सप्ताह 1-2 | 6 पाठ | डिलीवरेबल: आपका सॉवरेन स्टैक डॉक्यूमेंट*

> "आपका रिग आपका बिज़नेस इंफ्रास्ट्रक्चर है। इसे उसी तरह कॉन्फ़िगर करें।"

---

आपके पास पहले से ही सबसे शक्तिशाली इनकम-जनरेटिंग टूल है जो ज़्यादातर लोगों के पास कभी नहीं होगा: एक डेवलपर वर्कस्टेशन जिसमें इंटरनेट कनेक्शन, लोकल कंप्यूट, और सब कुछ जोड़ने की स्किल्स हैं।

ज़्यादातर डेवलपर्स अपने रिग को एक कंज़्यूमर प्रोडक्ट की तरह ट्रीट करते हैं। कुछ जिस पर गेम खेलते हैं, कोड करते हैं, ब्राउज़ करते हैं। लेकिन वही मशीन — जो अभी आपकी डेस्क के नीचे रखी है — इंफ़रेंस चला सकती है, API सर्व कर सकती है, डेटा प्रोसेस कर सकती है, और जब आप सो रहे हों तब दिन में 24 घंटे रेवेन्यू जनरेट कर सकती है।

यह मॉड्यूल इस बारे में है कि आपके पास जो पहले से है उसे एक अलग नज़रिए से देखें। "मैं क्या बना सकता हूँ?" नहीं बल्कि "मैं क्या बेच सकता हूँ?"

इन दो हफ्तों के अंत तक, आपके पास होगा:

- आपकी इनकम-जनरेटिंग क्षमताओं की स्पष्ट इन्वेंटरी
- प्रोडक्शन-ग्रेड लोकल LLM स्टैक
- एक कानूनी और वित्तीय आधार (भले ही न्यूनतम हो)
- एक लिखित सॉवरेन स्टैक डॉक्यूमेंट जो आपका बिज़नेस ब्लूप्रिंट बनेगा

कोई हवा-हवाई बातें नहीं। कोई "बस अपने आप पर भरोसा रखो" नहीं। असली नंबर, असली कमांड्स, असली फ़ैसले।

{@ mirror sovereign_readiness @}

चलिए शुरू करते हैं।

---

## पाठ 1: रिग ऑडिट

*"आपको 4090 की ज़रूरत नहीं है। असल में क्या मायने रखता है यह रहा।"*

### आपकी मशीन एक बिज़नेस एसेट है

जब कोई कंपनी अपने इंफ्रास्ट्रक्चर का मूल्यांकन करती है, तो वह सिर्फ़ स्पेक्स नहीं लिस्ट करती — वह क्षमताओं को रेवेन्यू अवसरों से मैप करती है। अभी आप यही करने वाले हैं।

{? if computed.profile_completeness != "0" ?}
> **आपका वर्तमान रिग:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} कोर / {= profile.cpu.threads | fallback("?") =} थ्रेड्स), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} फ्री / {= profile.storage.total | fallback("?") =} कुल ({= profile.storage.type | fallback("unknown") =}), {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =} चल रहा है।
{? endif ?}

टर्मिनल खोलें और नीचे दिए गए कमांड्स चलाएँ। हर नंबर लिख लें। पाठ 6 में सॉवरेन स्टैक डॉक्यूमेंट के लिए इनकी ज़रूरत होगी।

### हार्डवेयर इन्वेंटरी

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

**इनकम के लिए क्या मायने रखता है:**
- कोर काउंट तय करता है कि आपका रिग कितने concurrent टास्क हैंडल कर सकता है। लोकल LLM चलाते हुए साथ में बैच जॉब प्रोसेस करने के लिए असली पैरेललिज़्म चाहिए।
{? if profile.cpu.cores ?}
- *आपके {= profile.cpu.model | fallback("CPU") =} में {= profile.cpu.cores | fallback("?") =} कोर हैं — नीचे की रिक्वायरमेंट टेबल देखें कि आपका CPU किन रेवेन्यू इंजन्स को सपोर्ट करता है।*
{? endif ?}
- इस कोर्स में ज़्यादातर रेवेन्यू इंजन्स के लिए, पिछले 5 साल का कोई भी मॉडर्न 8+ कोर CPU पर्याप्त है।
- अगर आप सिर्फ CPU पर लोकल LLM चला रहे हैं (बिना GPU), तो 16+ कोर चाहिए। Ryzen 7 5800X या Intel i7-12700 प्रैक्टिकल फ्लोर है।

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**इनकम के लिए क्या मायने रखता है:**
- 16 GB: बेयर मिनिमम। आप 7B मॉडल चला सकते हैं और बेसिक ऑटोमेशन काम कर सकते हैं।
- 32 GB: आरामदायक। लोकली 13B मॉडल चलाएँ, मल्टीपल प्रोजेक्ट्स हैंडल करें, इनकम वर्कलोड के साथ-साथ डेव एनवायरनमेंट भी चालू रखें।
- 64 GB+: आप CPU पर 30B+ मॉडल चला सकते हैं, या मल्टीपल मॉडल लोडेड रख सकते हैं। यहीं इंफ़रेंस सर्विसेज़ बेचना दिलचस्प होता है।
{? if profile.ram.total ?}
*आपके सिस्टम में {= profile.ram.total | fallback("?") =} RAM है। ऊपर की टेबल देखें कि आप किस कैपेबिलिटी टियर में हैं — यह सीधे प्रभावित करता है कि आपके इनकम वर्कलोड के लिए कौन से लोकल मॉडल प्रैक्टिकल हैं।*
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

**इनकम के लिए क्या मायने रखता है:**

यही वह स्पेक है जिस पर लोग ज़्यादा ध्यान देते हैं, और यह रही सच्चाई: **आपका GPU आपका लोकल LLM टियर तय करता है, और आपका लोकल LLM टियर तय करता है कि कौन से इनकम स्ट्रीम्स सबसे तेज़ चलते हैं।** लेकिन यह तय नहीं करता कि आप पैसे कमा सकते हैं या नहीं।

| VRAM | LLM क्षमता | इनकम रेलेवेंस |
|------|-----------|--------------|
| 0 (सिर्फ CPU) | 7B मॉडल ~5 टोकन/सेकंड पर | बैच प्रोसेसिंग, एसिंक वर्क। धीमा लेकिन काम करता है। |
| 6-8 GB (RTX 3060, आदि) | 7B मॉडल ~30 टोकन/सेकंड, 13B क्वांटाइज़्ड | ज़्यादातर ऑटोमेशन इनकम स्ट्रीम्स के लिए काफ़ी। |
| 12 GB (RTX 3060 12GB, 4070) | 13B फुल स्पीड पर, 30B क्वांटाइज़्ड | स्वीट स्पॉट। ज़्यादातर रेवेन्यू इंजन्स यहाँ अच्छे चलते हैं। |
| 16-24 GB (RTX 4090, 3090) | 30B-70B मॉडल | प्रीमियम टियर। ऐसी क्वालिटी बेचें जो दूसरे लोकली मैच नहीं कर सकते। |
| 48 GB+ (ड्यूअल GPU, A6000) | 70B+ स्पीड पर | एंटरप्राइज़-ग्रेड लोकल इंफ़रेंस। गंभीर प्रतिस्पर्धात्मक लाभ। |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ यूनिफाइड मेमोरी से | बेहतरीन एफिशिएंसी। NVIDIA इक्विवेलेंट से कम पावर कॉस्ट। |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **आपका GPU:** {= profile.gpu.model | fallback("Unknown") =} {= profile.gpu.vram | fallback("?") =} VRAM के साथ — {? if computed.gpu_tier == "premium" ?}आप प्रीमियम टियर में हैं। 30B-70B मॉडल लोकली आपकी पहुँच में हैं। यह एक गंभीर प्रतिस्पर्धात्मक लाभ है।{? elif computed.gpu_tier == "sweet_spot" ?}आप स्वीट स्पॉट में हैं। 13B फुल स्पीड पर, 30B क्वांटाइज़्ड। ज़्यादातर रेवेन्यू इंजन्स यहाँ अच्छे चलते हैं।{? elif computed.gpu_tier == "capable" ?}आप 7B मॉडल अच्छी स्पीड पर और 13B क्वांटाइज़्ड चला सकते हैं। ज़्यादातर ऑटोमेशन इनकम स्ट्रीम्स के लिए काफ़ी।{? else ?}आपके पास GPU एक्सेलरेशन उपलब्ध है। ऊपर की टेबल देखें कि आप कहाँ हैं।{? endif ?}
{? else ?}
> **कोई डेडिकेटेड GPU नहीं मिला।** आप CPU पर इंफ़रेंस चलाएँगे, जिसका मतलब है ~5-12 टोकन/सेकंड 7B मॉडल पर। बैच प्रोसेसिंग और एसिंक वर्क के लिए यह ठीक है। कस्टमर-फेसिंग आउटपुट के लिए स्पीड गैप भरने के लिए API कॉल्स इस्तेमाल करें।
{? endif ?}

> **सीधी बात:** अगर आपके पास RTX 3060 12GB है, तो आप AI को मॉनेटाइज़ करने की कोशिश कर रहे 95% डेवलपर्स से बेहतर पोज़िशन में हैं। 4090 का इंतज़ार बंद करें। RTX 3060 12GB लोकल AI की Honda Civic है — विश्वसनीय, कुशल, काम पूरा करती है। GPU अपग्रेड पर जो पैसा खर्च करेंगे वह कस्टमर-फेसिंग क्वालिटी के लिए API क्रेडिट्स पर बेहतर खर्च होगा जबकि आपके लोकल मॉडल भारी काम संभालते हैं।

#### स्टोरेज

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**इनकम के लिए क्या मायने रखता है:**
- LLM मॉडल जगह लेते हैं: 7B मॉडल = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (क्वांटाइज़्ड)।
- प्रोजेक्ट डेटा, डेटाबेस, कैश, और आउटपुट आर्टिफैक्ट्स के लिए जगह चाहिए।
- कस्टमर-फेसिंग किसी भी चीज़ के लिए SSD अनिवार्य है। HDD से मॉडल लोडिंग में 30-60 सेकंड का स्टार्टअप टाइम जुड़ता है।
- न्यूनतम प्रैक्टिकल: 500 GB SSD कम से कम 100 GB फ्री के साथ।
- आरामदायक: 1 TB SSD। मॉडल SSD पर रखें, आर्काइव HDD पर।
{? if profile.storage.free ?}
*आपके {= profile.storage.type | fallback("your drive") =} पर {= profile.storage.free | fallback("?") =} फ्री है। {? if profile.storage.type == "SSD" ?}अच्छा — SSD का मतलब है तेज़ मॉडल लोडिंग।{? elif profile.storage.type == "NVMe" ?}बेहतरीन — NVMe मॉडल लोडिंग के लिए सबसे तेज़ विकल्प है।{? else ?}अगर आप पहले से SSD पर नहीं हैं तो SSD पर सोचें — मॉडल लोड टाइम में असली फ़र्क़ पड़ता है।{? endif ?}*
{? endif ?}

#### नेटवर्क

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**इनकम के लिए क्या मायने रखता है:**
{? if profile.network.download ?}
*आपका कनेक्शन: {= profile.network.download | fallback("?") =} डाउन / {= profile.network.upload | fallback("?") =} अप।*
{? endif ?}
- डाउनलोड स्पीड: 50+ Mbps। मॉडल, पैकेज, और डेटा पुल करने के लिए ज़रूरी।
- अपलोड स्पीड: यह वो बॉटलनेक है जिसे ज़्यादातर लोग नज़रअंदाज़ करते हैं। अगर आप कुछ भी सर्व कर रहे हैं (API, प्रोसेस्ड रिज़ल्ट्स, डिलीवरेबल्स), तो अपलोड मायने रखता है।
  - 10 Mbps: एसिंक डिलीवरी (प्रोसेस्ड फ़ाइलें, बैच रिज़ल्ट्स) के लिए पर्याप्त।
  - 50+ Mbps: अगर कोई भी लोकल API एंडपॉइंट चला रहे हैं जो बाहरी सर्विसेज़ हिट करती हैं तो ज़रूरी।
  - 100+ Mbps: इस कोर्स में सब कुछ के लिए आरामदायक।
- लेटेंसी: प्रमुख क्लाउड प्रोवाइडर्स तक 50ms से कम। `ping api.openai.com` और `ping api.anthropic.com` चलाकर चेक करें।

#### अपटाइम

यह वो स्पेक है जिसके बारे में कोई नहीं सोचता, लेकिन यह शौकियों को सोते हुए पैसे कमाने वालों से अलग करती है।

खुद से पूछें:
- क्या आपका रिग 24/7 चल सकता है? (पावर, कूलिंग, शोर)
- क्या आपके पास पावर आउटेज के लिए UPS है?
- क्या आपका इंटरनेट कनेक्शन ऑटोमेटेड वर्कफ़्लो के लिए पर्याप्त स्टेबल है?
- क्या कुछ टूटने पर आप रिमोटली SSH से अपनी मशीन में जा सकते हैं?

अगर 24/7 नहीं चला सकते, तो ठीक है — इस कोर्स में कई इनकम स्ट्रीम्स एसिंक बैच जॉब हैं जो आप मैन्युअली ट्रिगर करते हैं। लेकिन जो सच में पैसिव इनकम जनरेट करते हैं उन्हें अपटाइम चाहिए।

{? if computed.os_family == "windows" ?}
**क्विक अपटाइम सेटअप (Windows):** ऑटो-रीस्टार्ट के लिए Task Scheduler इस्तेमाल करें, Remote Desktop एनेबल करें या रिमोट एक्सेस के लिए Tailscale इंस्टॉल करें, और आउटेज से रिकवरी के लिए BIOS में "restore on AC power loss" कॉन्फ़िगर करें।
{? endif ?}

**क्विक अपटाइम सेटअप (अगर चाहते हैं):**

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

### बिजली का गणित

लोग या तो इसे नज़रअंदाज़ करते हैं या इसे बहुत बड़ा बना देते हैं। चलिए असली गणित करते हैं।

**अपना असल पावर ड्रॉ मापें:**

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

**मासिक कॉस्ट कैलकुलेशन:**

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
आपकी बिजली दर: लगभग {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh ({= regional.country | fallback("your region") =} के औसत पर आधारित)। अपना असली यूटिलिटी बिल चेक करें — रेट्स प्रोवाइडर और दिन के समय के अनुसार बदलते हैं।
{? else ?}
US में औसत बिजली लगभग $0.12/kWh है। अपनी असली रेट चेक करें — यह बहुत अलग-अलग होती है। कैलिफ़ोर्निया $0.25/kWh हो सकती है। कुछ यूरोपीय देश $0.35/kWh तक पहुँचते हैं। US मिडवेस्ट के कुछ हिस्से $0.08/kWh हैं।
{? endif ?}

**मुख्य बात:** इनकम के लिए अपना रिग 24/7 चलाने में बिजली का खर्च {= regional.currency_symbol | fallback("$") =}1-{= regional.currency_symbol | fallback("$") =}30/महीने के बीच आता है। अगर आपके इनकम स्ट्रीम्स इतना भी कवर नहीं कर सकते, तो समस्या बिजली नहीं है — इनकम स्ट्रीम है।

### रेवेन्यू इंजन टाइप के अनुसार न्यूनतम स्पेक्स

यह एक प्रीव्यू है कि पूरे STREETS कोर्स में हम कहाँ जा रहे हैं। अभी, बस चेक करें कि आपका रिग कहाँ आता है:

| रेवेन्यू इंजन | CPU | RAM | GPU | स्टोरेज | नेटवर्क |
|--------------|-----|-----|-----|---------|---------|
| **कंटेंट ऑटोमेशन** (ब्लॉग पोस्ट, न्यूज़लेटर) | 4+ कोर | 16 GB | वैकल्पिक (API फ़ॉलबैक) | 50 GB फ्री | 10 Mbps अप |
| **डेटा प्रोसेसिंग सर्विसेज़** | 8+ कोर | 32 GB | वैकल्पिक | 200 GB फ्री | 50 Mbps अप |
| **लोकल AI API सर्विसेज़** | 8+ कोर | 32 GB | 8+ GB VRAM | 100 GB फ्री | 50 Mbps अप |
| **कोड जनरेशन टूल्स** | 8+ कोर | 16 GB | 8+ GB VRAM या API | 50 GB फ्री | 10 Mbps अप |
| **डॉक्यूमेंट प्रोसेसिंग** | 4+ कोर | 16 GB | वैकल्पिक | 100 GB फ्री | 10 Mbps अप |
| **ऑटोनॉमस एजेंट्स** | 8+ कोर | 32 GB | 12+ GB VRAM | 100 GB फ्री | 50 Mbps अप |

> **आम गलती:** "मुझे शुरू करने से पहले अपना हार्डवेयर अपग्रेड करना होगा।" नहीं। जो है उससे शुरू करें। जो गैप आपका हार्डवेयर कवर नहीं कर सकता उसके लिए API कॉल्स इस्तेमाल करें। रेवेन्यू जस्टिफ़ाई करे तब अपग्रेड करें — पहले नहीं।

{@ insight engine_ranking @}

### पाठ 1 चेकपॉइंट

अब तक आपने यह लिख लिया होना चाहिए:
- [ ] CPU मॉडल, कोर, और थ्रेड्स
- [ ] RAM की मात्रा
- [ ] GPU मॉडल और VRAM (या "कोई नहीं")
- [ ] उपलब्ध स्टोरेज
- [ ] नेटवर्क स्पीड (डाउन/अप)
- [ ] 24/7 ऑपरेशन के लिए अनुमानित मासिक बिजली खर्च
- [ ] आपका रिग किन रेवेन्यू इंजन कैटेगरीज़ के लिए क्वालिफ़ाई करता है

ये नंबर रखें। पाठ 6 में सॉवरेन स्टैक डॉक्यूमेंट में इन्हें डालेंगे।

{? if computed.profile_completeness != "0" ?}
> **4DA ने पहले ही ज़्यादातर नंबर आपके लिए कलेक्ट कर लिए हैं।** ऊपर पर्सनलाइज़्ड समरीज़ चेक करें — आपकी हार्डवेयर इन्वेंटरी सिस्टम डिटेक्शन से आंशिक रूप से प्री-फ़िल्ड है।
{? endif ?}

*पूरे STREETS कोर्स में, मॉड्यूल R (रेवेन्यू इंजन्स) आपको ऊपर लिस्टेड हर इंजन टाइप के लिए विशिष्ट, स्टेप-बाई-स्टेप प्लेबुक देता है — जिसमें बिल्ड और डिप्लॉय करने का सटीक कोड शामिल है।*

---

## पाठ 2: लोकल LLM स्टैक

*"Ollama को प्रोडक्शन यूज़ के लिए सेट करें — सिर्फ़ चैट के लिए नहीं।"*

### लोकल LLM इनकम के लिए क्यों मायने रखते हैं

हर बार जब आप OpenAI API कॉल करते हैं, आप किराया दे रहे हैं। हर बार जब आप लोकली मॉडल चलाते हैं, वो इंफ़रेंस इनिशियल सेटअप के बाद फ्री है। गणित सीधा है:

- GPT-4o: ~$5 प्रति मिलियन इनपुट टोकन, ~$15 प्रति मिलियन आउटपुट टोकन
- Claude 3.5 Sonnet: ~$3 प्रति मिलियन इनपुट टोकन, ~$15 प्रति मिलियन आउटपुट टोकन
- लोकल Llama 3.1 8B: $0 प्रति मिलियन टोकन (सिर्फ़ बिजली)

अगर आप ऐसी सर्विसेज़ बना रहे हैं जो हज़ारों रिक्वेस्ट प्रोसेस करती हैं, $0 और $5-$15 प्रति मिलियन टोकन के बीच का फ़र्क़ प्रॉफ़िट और ब्रेक-ईवन के बीच का फ़र्क़ है।

लेकिन यह बात ज़्यादातर लोग मिस करते हैं: **लोकल और API मॉडल इनकम स्टैक में अलग-अलग रोल निभाते हैं।** लोकल मॉडल वॉल्यूम हैंडल करते हैं। API मॉडल क्वालिटी-क्रिटिकल, कस्टमर-फेसिंग आउटपुट हैंडल करते हैं। आपके स्टैक में दोनों चाहिए।

### Ollama इंस्टॉल करना

{? if settings.has_llm ?}
> **आपके पास पहले से LLM कॉन्फ़िगर्ड है:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}। अगर Ollama पहले से चल रहा है, नीचे "मॉडल सेलेक्शन गाइड" पर जाएँ।
{? endif ?}

Ollama फ़ाउंडेशन है। यह आपकी मशीन को क्लीन API के साथ लोकल इंफ़रेंस सर्वर में बदल देता है।

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
> **Windows:** ollama.com से इंस्टॉलर इस्तेमाल करें या `winget install Ollama.Ollama`। इंस्टॉलेशन के बाद Ollama बैकग्राउंड सर्विस के रूप में ऑटोमैटिकली चलता है।
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` सबसे तेज़ रास्ता है। Ollama Apple Silicon की यूनिफाइड मेमोरी का फ़ायदा उठाता है — आपकी {= profile.ram.total | fallback("system") =} RAM CPU और GPU वर्कलोड के बीच शेयर होती है।
{? elif computed.os_family == "linux" ?}
> **Linux:** इंस्टॉल स्क्रिप्ट सब कुछ हैंडल करती है। अगर आप {= profile.os.name | fallback("Linux") =} चला रहे हैं, Ollama systemd सर्विस के रूप में इंस्टॉल होता है।
{? endif ?}

इंस्टॉलेशन वेरिफ़ाई करें:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **वर्शन नोट:** Ollama बार-बार रिलीज़ करता है। इस मॉड्यूल में मॉडल कमांड्स और फ़्लैग्स Ollama v0.5.x (अर्ली 2026) के ख़िलाफ़ वेरिफ़ाई किए गए थे। अगर आप यह बाद में पढ़ रहे हैं, लेटेस्ट वर्शन के लिए [ollama.com/download](https://ollama.com/download) और करंट मॉडल नेम्स के लिए [ollama.com/library](https://ollama.com/library) चेक करें। कोर कॉन्सेप्ट्स नहीं बदलते, लेकिन स्पेसिफ़िक मॉडल टैग्स (जैसे `llama3.1:8b`) नए रिलीज़ से रिप्लेस हो सकते हैं।

### मॉडल सेलेक्शन गाइड

हर मॉडल जो दिखे उसे डाउनलोड न करें। स्ट्रैटेजिक बनें। यह रहा क्या पुल करना है और हर एक कब इस्तेमाल करना है।

{? if computed.llm_tier ?}
> **आपका LLM टियर (हार्डवेयर पर आधारित):** {= computed.llm_tier | fallback("unknown") =}। नीचे की सिफ़ारिशें टैग्ड हैं ताकि आप अपने रिग से मैचिंग टियर पर फ़ोकस कर सकें।
{? endif ?}

#### टियर 1: वर्कहॉर्स (7B-8B मॉडल)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**इस्तेमाल करें:**
- टेक्स्ट क्लासिफ़िकेशन ("क्या यह ईमेल स्पैम है या लेजिटिमेट?")
- समराइज़ेशन (लंबे डॉक्यूमेंट्स को बुलेट पॉइंट्स में कंडेंस करें)
- सिंपल डेटा एक्सट्रैक्शन (टेक्स्ट से नाम, तारीख, अमाउंट निकालें)
- सेंटिमेंट एनालिसिस
- कंटेंट टैगिंग और कैटेगोराइज़ेशन
- एम्बेडिंग जनरेशन (अगर एम्बेडिंग सपोर्ट वाला मॉडल इस्तेमाल कर रहे हैं)

**परफ़ॉर्मेंस (टिपिकल):**
- RTX 3060 12GB: ~40-60 टोकन/सेकंड
- RTX 4090: ~100-130 टोकन/सेकंड
- M2 Pro 16GB: ~30-45 टोकन/सेकंड
- सिर्फ CPU (Ryzen 7 5800X): ~8-12 टोकन/सेकंड

**कॉस्ट कम्पेरिज़न:**
- GPT-4o-mini से 1 मिलियन टोकन: ~$0.60
- लोकली 1 मिलियन टोकन (8B मॉडल): ~$0.003 बिजली में
- ब्रेक-ईवन पॉइंट: ~5,000 टोकन (आप पहली रिक्वेस्ट से ही पैसे बचाते हैं)

#### टियर 2: बैलेंस्ड चॉइस (13B-14B मॉडल)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**इस्तेमाल करें:**
- कंटेंट ड्राफ़्टिंग (ब्लॉग पोस्ट, डॉक्यूमेंटेशन, मार्केटिंग कॉपी)
- कोड जनरेशन (फ़ंक्शन्स, स्क्रिप्ट्स, बॉयलरप्लेट)
- कॉम्प्लेक्स डेटा ट्रांसफ़ॉर्मेशन
- मल्टी-स्टेप रीज़निंग टास्क
- न्यूआंस के साथ ट्रांसलेशन

**परफ़ॉर्मेंस (टिपिकल):**
- RTX 3060 12GB: ~20-30 टोकन/सेकंड (क्वांटाइज़्ड)
- RTX 4090: ~60-80 टोकन/सेकंड
- M2 Pro 32GB: ~20-30 टोकन/सेकंड
- सिर्फ CPU: ~3-6 टोकन/सेकंड (रियल-टाइम के लिए प्रैक्टिकल नहीं)

**7B की जगह कब इस्तेमाल करें:** जब 7B की आउटपुट क्वालिटी काफ़ी अच्छी नहीं है लेकिन API कॉल्स के लिए पैसे नहीं देने। दोनों को अपने असल यूज़ केस पर टेस्ट करें — कभी-कभी 7B काफ़ी होता है और आप बस कंप्यूट बर्बाद कर रहे हैं।

{? if computed.gpu_tier == "capable" ?}
> **टियर 3 स्ट्रेच टेरिटरी** — आपका {= profile.gpu.model | fallback("GPU") =} 30B क्वांटाइज़्ड कुछ एफ़र्ट से हैंडल कर सकता है, लेकिन 70B लोकली पहुँच से बाहर है। 70B-लेवल क्वालिटी वाले टास्क के लिए API कॉल्स पर विचार करें।
{? endif ?}

#### टियर 3: क्वालिटी टियर (30B-70B मॉडल)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**इस्तेमाल करें:**
- कस्टमर-फेसिंग कंटेंट जो उत्कृष्ट होना चाहिए
- कॉम्प्लेक्स एनालिसिस और रीज़निंग
- लॉन्ग-फ़ॉर्म कंटेंट जनरेशन
- टास्क जहाँ क्वालिटी सीधे प्रभावित करती है कि कोई आपको पैसे देगा या नहीं

**परफ़ॉर्मेंस (टिपिकल):**
- RTX 4090 (24GB): 70B ~8-15 टोकन/सेकंड पर (यूज़ेबल लेकिन धीमा)
- ड्यूअल GPU या 48GB+: 70B ~20-30 टोकन/सेकंड पर
- M3 Max 64GB: 70B ~10-15 टोकन/सेकंड पर

> **सीधी बात:** अगर आपके पास 24GB+ VRAM नहीं है, 70B मॉडल पूरी तरह स्किप करें। क्वालिटी-क्रिटिकल आउटपुट के लिए API कॉल्स इस्तेमाल करें। सिस्टम RAM से 3 टोकन/सेकंड पर 70B मॉडल चलाना तकनीकी रूप से संभव है लेकिन किसी भी इनकम-जनरेटिंग वर्कफ़्लो के लिए प्रैक्टिकली बेकार है। आपके समय की क़ीमत है।

#### टियर 4: API मॉडल (जब लोकल काफ़ी नहीं है)

लोकल मॉडल वॉल्यूम और प्राइवेसी के लिए हैं। API मॉडल क्वालिटी सीलिंग्स और स्पेशलाइज़्ड कैपेबिलिटीज़ के लिए हैं।

**API मॉडल कब इस्तेमाल करें:**
- कस्टमर-फेसिंग आउटपुट जहाँ क्वालिटी = रेवेन्यू (सेल्स कॉपी, प्रीमियम कंटेंट)
- कॉम्प्लेक्स रीज़निंग चेन जिनमें छोटे मॉडल लड़खड़ाते हैं
- विज़न/मल्टीमोडल टास्क (इमेज, स्क्रीनशॉट, डॉक्यूमेंट एनालाइज़ करना)
- जब हाई रिलायबिलिटी के साथ स्ट्रक्चर्ड JSON आउटपुट चाहिए
- जब स्पीड मायने रखती है और आपका लोकल हार्डवेयर धीमा है

**कॉस्ट कम्पेरिज़न टेबल (अर्ली 2025 — करंट प्राइसिंग चेक करें):**

| मॉडल | इनपुट (प्रति 1M टोकन) | आउटपुट (प्रति 1M टोकन) | बेस्ट फ़ॉर |
|------|----------------------|------------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | सस्ता वॉल्यूम वर्क (जब लोकल उपलब्ध नहीं) |
| GPT-4o | $2.50 | $10.00 | विज़न, कॉम्प्लेक्स रीज़निंग |
| Claude 3.5 Sonnet | $3.00 | $15.00 | कोड, एनालिसिस, लॉन्ग कॉन्टेक्स्ट |
| Claude 3.5 Haiku | $0.80 | $4.00 | फ़ास्ट, सस्ता, अच्छी क्वालिटी बैलेंस |
| DeepSeek V3 | $0.27 | $1.10 | बजट-फ्रेंडली, मज़बूत परफ़ॉर्मेंस |

**हाइब्रिड स्ट्रैटेजी:**
1. लोकल 7B/13B 80% रिक्वेस्ट हैंडल करता है (क्लासिफ़िकेशन, एक्सट्रैक्शन, समराइज़ेशन)
2. API 20% रिक्वेस्ट हैंडल करता है (फ़ाइनल क्वालिटी पास, कॉम्प्लेक्स टास्क, कस्टमर-फेसिंग आउटपुट)
3. आपकी इफ़ेक्टिव कॉस्ट प्रति टास्क प्योर API यूसेज की तुलना में नाटकीय रूप से गिरती है

यह हाइब्रिड अप्रोच है कि आप हेल्दी मार्जिन वाली सर्विसेज़ कैसे बनाते हैं। मॉड्यूल R में और बताएँगे।

### प्रोडक्शन कॉन्फ़िगरेशन

इनकम वर्क के लिए Ollama चलाना पर्सनल चैट के लिए चलाने से अलग है। यह रहा इसे ठीक से कॉन्फ़िगर करने का तरीका।

{? if computed.has_nvidia ?}
> **NVIDIA GPU डिटेक्ट हुआ ({= profile.gpu.model | fallback("unknown") =})।** Ollama ऑटोमैटिकली CUDA एक्सेलरेशन इस्तेमाल करेगा। सुनिश्चित करें कि आपके NVIDIA ड्राइवर्स अप टू डेट हैं — चेक करने के लिए `nvidia-smi` चलाएँ। {= profile.gpu.vram | fallback("your") =} VRAM के साथ बेस्ट परफ़ॉर्मेंस के लिए, नीचे `OLLAMA_MAX_LOADED_MODELS` सेटिंग आपके VRAM में एक साथ कितने मॉडल फ़िट होते हैं उससे मैच होनी चाहिए।
{? endif ?}

#### एनवायरनमेंट वेरिएबल सेट करें

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

#### अपने वर्कलोड के लिए Modelfile बनाएँ

डिफ़ॉल्ट मॉडल सेटिंग्स इस्तेमाल करने के बजाय, अपने इनकम वर्कलोड के लिए ट्यून्ड कस्टम Modelfile बनाएँ:

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

#### बैचिंग और क्यू मैनेजमेंट

इनकम वर्कलोड के लिए, आपको अक्सर कई आइटम्स प्रोसेस करने होंगे। यह रहा एक बेसिक बैचिंग सेटअप:

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

### अपने रिग का बेंचमार्क

किसी और के बेंचमार्क पर भरोसा न करें। अपना खुद मापें:

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

हर मॉडल के लिए अपना टोकन/सेकंड लिख लें। यह नंबर तय करता है कि कौन से इनकम वर्कफ़्लो आपके रिग के लिए प्रैक्टिकल हैं।

{@ insight stack_fit @}

**यूज़ केस के अनुसार स्पीड रिक्वायरमेंट:**
- बैच प्रोसेसिंग (एसिंक): 5+ टोकन/सेकंड ठीक है (आपको लेटेंसी की चिंता नहीं)
- इंटरैक्टिव टूल्स (यूज़र वेट करता है): न्यूनतम 20+ टोकन/सेकंड
- रियल-टाइम API (कस्टमर-फेसिंग): अच्छे UX के लिए 30+ टोकन/सेकंड
- स्ट्रीमिंग चैट: 15+ टोकन/सेकंड रिस्पॉन्सिव लगता है

### अपने लोकल इंफ़रेंस सर्वर को सुरक्षित करें

{? if computed.os_family == "windows" ?}
> **Windows नोट:** Windows पर Ollama डिफ़ॉल्ट रूप से localhost से बाइंड होता है। PowerShell में `netstat -an | findstr 11434` से वेरिफ़ाई करें। पोर्ट 11434 तक बाहरी एक्सेस ब्लॉक करने के लिए Windows Firewall इस्तेमाल करें।
{? elif computed.os_family == "macos" ?}
> **macOS नोट:** macOS पर Ollama डिफ़ॉल्ट रूप से localhost से बाइंड होता है। `lsof -i :11434` से वेरिफ़ाई करें। macOS फ़ायरवॉल बाहरी कनेक्शन ऑटोमैटिकली ब्लॉक करेगा।
{? endif ?}

आपका Ollama इंस्टेंस कभी भी इंटरनेट से एक्सेसिबल नहीं होना चाहिए जब तक आप जानबूझकर ऐसा न चाहें।

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

> **आम गलती:** "सुविधा" के लिए Ollama को 0.0.0.0 से बाइंड करना और इसे भूल जाना। जो कोई भी आपका IP ढूँढ ले वो आपके GPU का फ्री इंफ़रेंस के लिए इस्तेमाल कर सकता है। इससे भी बुरा, वो मॉडल वेट्स और सिस्टम प्रॉम्प्ट्स एक्सट्रैक्ट कर सकते हैं। हमेशा localhost। हमेशा टनल।

### पाठ 2 चेकपॉइंट

आपके पास अब होना चाहिए:
- [ ] Ollama इंस्टॉल और रनिंग
- [ ] कम से कम एक वर्कहॉर्स मॉडल पुल्ड (llama3.1:8b या इक्विवेलेंट)
- [ ] अपने एक्सपेक्टेड वर्कलोड के लिए कस्टम Modelfile
- [ ] बेंचमार्क नंबर: हर मॉडल के लिए आपके रिग पर टोकन/सेकंड
- [ ] Ollama सिर्फ़ localhost से बाउंड

*पूरे STREETS कोर्स में, मॉड्यूल T (टेक्निकल मोट्स) दिखाता है कि प्रोप्राइटरी मॉडल कॉन्फ़िगरेशन, फ़ाइन-ट्यून्ड पाइपलाइन, और कस्टम टूलचेन कैसे बनाएँ जो कॉम्पिटिटर्स आसानी से रेप्लिकेट नहीं कर सकते। मॉड्यूल R (रेवेन्यू इंजन्स) इस स्टैक के ऊपर बनाने वाली सटीक सर्विसेज़ देता है।*

---

## पाठ 3: प्राइवेसी एडवांटेज

*"आपका प्राइवेट सेटअप एक प्रतिस्पर्धात्मक लाभ है — सिर्फ़ प्रेफ़रेंस नहीं।"*

### प्राइवेसी एक प्रोडक्ट फ़ीचर है, सीमा नहीं

ज़्यादातर डेवलपर्स लोकल इंफ्रास्ट्रक्चर इसलिए सेट करते हैं क्योंकि वो व्यक्तिगत रूप से प्राइवेसी वैल्यू करते हैं, या टिंकरिंग एन्जॉय करते हैं। ठीक है। लेकिन अगर आप यह नहीं समझते कि **प्राइवेसी अभी टेक में सबसे मार्केटेबल फ़ीचर्स में से एक है** तो आप टेबल पर पैसे छोड़ रहे हैं।

यह रहा कारण: हर बार कोई कंपनी OpenAI के API को डेटा भेजती है, वो डेटा थर्ड पार्टी से होकर गुज़रता है। कई बिज़नेसेज़ के लिए — ख़ासकर हेल्थकेयर, फ़ाइनेंस, लीगल, गवर्नमेंट, और EU-बेस्ड कंपनियों — यह एक असली समस्या है। थ्योरेटिकल नहीं। "हम यह टूल इस्तेमाल नहीं कर सकते क्योंकि कंप्लायंस ने मना कर दिया" वाली समस्या।

आप, जो अपनी मशीन पर लोकली मॉडल चलाते हैं, आपको यह समस्या नहीं है।

### रेगुलेटरी टेलविंड

रेगुलेटरी एनवायरनमेंट आपकी दिशा में मूव कर रहा है। तेज़ी से।

{? if regional.country == "US" ?}
> **US-बेस्ड:** आपके लिए सबसे ज़्यादा मायने रखने वाले रेगुलेशन हैं HIPAA, SOC 2, ITAR, और स्टेट-लेवल प्राइवेसी लॉज़ (California CCPA, आदि)। EU रेगुलेशन अभी भी मायने रखते हैं — ये यूरोपीयन क्लाइंट्स को सर्व करने की आपकी क्षमता को प्रभावित करते हैं, जो एक ल्यूक्रेटिव मार्केट है।
{? elif regional.country == "GB" ?}
> **UK-बेस्ड:** Brexit के बाद, UK का अपना डेटा प्रोटेक्शन फ्रेमवर्क है (UK GDPR + Data Protection Act 2018)। UK फ़ाइनेंशियल सर्विसेज़ और NHS-एडजेसेंट वर्क सर्व करने के लिए आपकी लोकल प्रोसेसिंग एडवांटेज ख़ासतौर पर मज़बूत है।
{? elif regional.country == "DE" ?}
> **जर्मनी-बेस्ड:** आप दुनिया के सबसे सख़्त डेटा प्रोटेक्शन एनवायरनमेंट में से एक में हैं। यह एक *एडवांटेज* है — जर्मन क्लाइंट्स पहले से समझते हैं कि लोकल प्रोसेसिंग क्यों मायने रखती है, और वो इसके लिए पैसे देंगे।
{? elif regional.country == "AU" ?}
> **ऑस्ट्रेलिया-बेस्ड:** Privacy Act 1988 और Australian Privacy Principles (APPs) आपकी बाध्यताओं को गवर्न करते हैं। My Health Records Act के तहत गवर्नमेंट और हेल्थकेयर क्लाइंट्स के लिए लोकल प्रोसेसिंग एक मज़बूत सेलिंग पॉइंट है।
{? endif ?}

**EU AI Act (2024-2026 से लागू):**
- हाई-रिस्क AI सिस्टम को डॉक्यूमेंटेड डेटा प्रोसेसिंग पाइपलाइन चाहिए
- कंपनियों को दिखाना होगा कि डेटा कहाँ फ़्लो होता है और कौन प्रोसेस करता है
- लोकल प्रोसेसिंग कंप्लायंस को काफ़ी सिंपलिफ़ाई करती है
- EU कंपनियाँ सक्रिय रूप से ऐसे AI सर्विस प्रोवाइडर ढूँढ रही हैं जो EU डेटा रेज़िडेंसी गारंटी कर सकें

**GDPR (पहले से लागू):**
- "डेटा प्रोसेसिंग" में LLM API को टेक्स्ट भेजना शामिल है
- कंपनियों को हर थर्ड पार्टी के साथ Data Processing Agreements चाहिए
- लोकल प्रोसेसिंग थर्ड पार्टी को पूरी तरह ख़त्म करती है
- यह असली सेलिंग पॉइंट है: "आपका डेटा कभी आपके इंफ्रास्ट्रक्चर से बाहर नहीं जाता। नेगोशिएट करने के लिए कोई थर्ड-पार्टी DPA नहीं है।"

**इंडस्ट्री-स्पेसिफ़िक रेगुलेशन:**
- **HIPAA (US हेल्थकेयर):** पेशेंट डेटा BAA (Business Associate Agreement) के बिना कंज़्यूमर AI API को नहीं भेजा जा सकता। ज़्यादातर AI प्रोवाइडर API एक्सेस के लिए BAA ऑफ़र नहीं करते। लोकल प्रोसेसिंग इसे पूरी तरह बाईपास करती है।
- **SOC 2 (एंटरप्राइज़):** SOC 2 ऑडिट से गुज़रने वाली कंपनियों को हर डेटा प्रोसेसर डॉक्यूमेंट करना होता है। कम प्रोसेसर = आसान ऑडिट।
- **ITAR (US डिफ़ेंस):** कंट्रोल्ड टेक्निकल डेटा US जूरिसडिक्शन से बाहर नहीं जा सकता। इंटरनेशनल इंफ्रास्ट्रक्चर वाले क्लाउड AI प्रोवाइडर प्रॉब्लेमेटिक हैं।
- **PCI DSS (फ़ाइनेंस):** कार्डहोल्डर डेटा प्रोसेसिंग में डेटा कहाँ ट्रैवल करता है इसकी सख़्त रिक्वायरमेंट हैं।

### सेल्स कन्वर्सेशन में प्राइवेसी कैसे पोज़िशन करें

आपको कंप्लायंस एक्सपर्ट बनने की ज़रूरत नहीं। तीन फ़्रेज़ समझने और सही समय पर इस्तेमाल करने की ज़रूरत है:

**फ़्रेज़ 1: "आपका डेटा कभी आपके इंफ्रास्ट्रक्चर से बाहर नहीं जाता।"**
कब इस्तेमाल करें: किसी भी प्राइवेसी-कॉन्शस प्रॉस्पेक्ट से बात करते समय। यह यूनिवर्सल हुक है।

**फ़्रेज़ 2: "कोई थर्ड-पार्टी डेटा प्रोसेसिंग एग्रीमेंट ज़रूरी नहीं।"**
कब इस्तेमाल करें: यूरोपीयन कंपनियों या लीगल/कंप्लायंस टीम वाली किसी भी कंपनी से बात करते समय। यह उन्हें हफ्तों की लीगल रिव्यू बचाता है।

**फ़्रेज़ 3: "पूरा ऑडिट ट्रेल, सिंगल-टेनेंट प्रोसेसिंग।"**
कब इस्तेमाल करें: एंटरप्राइज़ या रेगुलेटेड इंडस्ट्रीज़ से बात करते समय। उन्हें ऑडिटर्स को अपनी AI पाइपलाइन प्रूव करनी होती है।

**पोज़िशनिंग का उदाहरण (आपकी सर्विस पेज या प्रपोज़ल के लिए):**

> "क्लाउड-बेस्ड AI सर्विसेज़ के विपरीत, [आपकी सर्विस] सभी डेटा को डेडिकेटेड हार्डवेयर पर लोकली प्रोसेस करती है। आपके डॉक्यूमेंट, कोड, और डेटा कभी प्रोसेसिंग एनवायरनमेंट से बाहर नहीं जाते। पाइपलाइन में कोई थर्ड-पार्टी API नहीं है, नेगोशिएट करने के लिए कोई डेटा शेयरिंग एग्रीमेंट नहीं, और हर ऑपरेशन की पूरी ऑडिट लॉगिंग। यह [आपकी सर्विस] को सख़्त डेटा हैंडलिंग रिक्वायरमेंट वाली ऑर्गनाइज़ेशन के लिए उपयुक्त बनाता है, जिसमें GDPR, HIPAA, और SOC 2 कंप्लायंस एनवायरनमेंट शामिल हैं।"

यह पैराग्राफ़, एक लैंडिंग पेज पर, बिल्कुल उन क्लाइंट्स को अट्रैक्ट करेगा जो प्रीमियम रेट्स पर पैसे देंगे।

### प्रीमियम प्राइसिंग जस्टिफ़िकेशन

यह रहा हार्ड नंबर्स में बिज़नेस केस:

**स्टैंडर्ड AI प्रोसेसिंग सर्विस (क्लाउड API इस्तेमाल करके):**
- क्लाइंट का डेटा OpenAI/Anthropic/Google को जाता है
- आप हर उस डेवलपर से कॉम्पिटीट कर रहे हैं जो API कॉल कर सकता है
- मार्केट रेट: $0.01-0.05 प्रति डॉक्यूमेंट प्रोसेस्ड
- आप एसेंशियली मार्कअप के साथ API एक्सेस रीसेल कर रहे हैं

**प्राइवेसी-फ़र्स्ट AI प्रोसेसिंग सर्विस (आपका लोकल स्टैक):**
- क्लाइंट का डेटा आपकी मशीन पर रहता है
- आप बहुत छोटे प्रोवाइडर पूल से कॉम्पिटीट कर रहे हैं
- मार्केट रेट: $0.10-0.50 प्रति डॉक्यूमेंट प्रोसेस्ड (5-10x प्रीमियम)
- आप इंफ्रास्ट्रक्चर + एक्सपर्टीज़ + कंप्लायंस बेच रहे हैं

प्राइवेसी प्रीमियम असली है: उसी अंडरलाइंग टास्क के लिए कमोडिटी क्लाउड-बेस्ड सर्विसेज़ से **5x से 10x** ज़्यादा। और जो क्लाइंट्स इसे पे करते हैं वो ज़्यादा लॉयल, कम प्राइस-सेंसिटिव, और बड़ी बजट वाले होते हैं।

{@ insight competitive_position @}

### आइसोलेटेड वर्कस्पेस सेट करना

अगर आपकी डे जॉब है (ज़्यादातर लोगों की है), तो एम्प्लॉयर वर्क और इनकम वर्क के बीच क्लीन सेपरेशन चाहिए। यह सिर्फ़ लीगल प्रोटेक्शन नहीं — ऑपरेशनल हाइजीन है।

{? if computed.os_family == "windows" ?}
> **Windows टिप:** इनकम वर्क के लिए अलग Windows यूज़र अकाउंट बनाएँ (Settings > Accounts > Family & other users > Add someone else)। इससे पूरी तरह आइसोलेटेड एनवायरनमेंट मिलता है — अलग ब्राउज़र प्रोफ़ाइल, अलग फ़ाइल पाथ, अलग एनवायरनमेंट वेरिएबल्स। Win+L से अकाउंट स्विच करें।
{? endif ?}

**ऑप्शन 1: अलग यूज़र अकाउंट (रिकमेंडेड)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**ऑप्शन 2: कंटेनराइज़्ड वर्कस्पेस**

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

**ऑप्शन 3: अलग फ़िज़िकल मशीन (सबसे बुलेटप्रूफ़)**

अगर आप इस बारे में गंभीर हैं और इनकम जस्टिफ़ाई करती है, तो एक डेडिकेटेड मशीन सारे सवाल ख़त्म कर देती है। RTX 3060 वाला यूज़्ड Dell OptiPlex $400-600 में आता है और पहले महीने के क्लाइंट वर्क में अपनी कॉस्ट रिकवर कर लेता है।

**मिनिमम सेपरेशन चेकलिस्ट:**
- [ ] इनकम प्रोजेक्ट्स अलग डायरेक्टरी में (कभी एम्प्लॉयर रिपोज़ के साथ मिक्स न करें)
- [ ] इनकम वर्क के लिए अलग API कीज़ (कभी एम्प्लॉयर-प्रोवाइडेड कीज़ इस्तेमाल न करें)
- [ ] इनकम-रिलेटेड अकाउंट्स के लिए अलग ब्राउज़र प्रोफ़ाइल
- [ ] इनकम वर्क कभी एम्प्लॉयर हार्डवेयर पर न करें
- [ ] इनकम वर्क कभी एम्प्लॉयर नेटवर्क पर न करें (पर्सनल इंटरनेट या VPN इस्तेमाल करें)
- [ ] इनकम प्रोजेक्ट्स के लिए अलग GitHub/GitLab अकाउंट (वैकल्पिक लेकिन क्लीन)

> **आम गलती:** अपने साइड प्रोजेक्ट को "बस टेस्टिंग" के लिए एम्प्लॉयर का OpenAI API की इस्तेमाल करना। यह एक पेपर ट्रेल बनाता है जो आपके एम्प्लॉयर की बिलिंग डैशबोर्ड में दिखता है, और IP वॉटर्स मडी करता है। अपनी खुद की कीज़ लें। सस्ती हैं।

### पाठ 3 चेकपॉइंट

आपको अब समझना चाहिए:
- [ ] प्राइवेसी एक मार्केटेबल प्रोडक्ट फ़ीचर क्यों है, सिर्फ़ पर्सनल प्रेफ़रेंस नहीं
- [ ] कौन से रेगुलेशन लोकल AI प्रोसेसिंग की डिमांड बनाते हैं
- [ ] प्राइवेसी के बारे में सेल्स कन्वर्सेशन में इस्तेमाल करने के लिए तीन फ़्रेज़
- [ ] प्राइवेसी-फ़र्स्ट सर्विसेज़ 5-10x प्रीमियम प्राइसिंग कैसे कमांड करती हैं
- [ ] इनकम वर्क को एम्प्लॉयर वर्क से कैसे सेपरेट करें

*पूरे STREETS कोर्स में, मॉड्यूल E (इवॉल्विंग एज) सिखाता है कि रेगुलेटरी चेंजेज़ को कैसे ट्रैक करें और नई कंप्लायंस रिक्वायरमेंट से पहले खुद को पोज़िशन करें — इससे पहले कि आपके कॉम्पिटिटर्स को पता भी चले कि ये एक्ज़िस्ट करती हैं।*

---

## पाठ 4: न्यूनतम कानूनी ज़रूरतें

*"अभी पंद्रह मिनट का लीगल सेटअप बाद में महीनों की समस्याएँ रोकता है।"*

### यह लीगल एडवाइस नहीं है

मैं एक डेवलपर हूँ, वकील नहीं। जो आगे है वो एक प्रैक्टिकल चेकलिस्ट है जिसे ज़्यादातर डेवलपर्स को ज़्यादातर सिचुएशन में एड्रेस करना चाहिए। अगर आपकी सिचुएशन कॉम्प्लेक्स है (एम्प्लॉयर में इक्विटी, स्पेसिफ़िक टर्म्स वाला नॉन-कॉम्पीट, आदि), एम्प्लॉयमेंट अटॉर्नी से 30-मिनट की कंसल्टेशन पर $200 खर्च करें। यह सबसे अच्छा ROI होगा जो आपको मिलेगा।

### स्टेप 1: अपना एम्प्लॉयमेंट कॉन्ट्रैक्ट पढ़ें

अपना एम्प्लॉयमेंट कॉन्ट्रैक्ट या ऑफ़र लेटर ढूँढें। ये सेक्शन सर्च करें:

**Intellectual Property Assignment क्लॉज़** — ऐसी भाषा ढूँढें:
- "All inventions, developments, and work product..."
- "...created during the term of employment..."
- "...related to the Company's business or anticipated business..."

**मुख्य फ़्रेज़ जो आपको रिस्ट्रिक्ट करते हैं:**
- "एम्प्लॉयमेंट के दौरान बनाया गया सभी वर्क प्रोडक्ट कंपनी का है" (ब्रॉड — पोटेंशियली प्रॉब्लेमेटिक)
- "कंपनी रिसोर्सेज़ से बनाया गया वर्क प्रोडक्ट" (नैरोअर — अगर अपना इक्विपमेंट इस्तेमाल करें तो आमतौर पर ठीक)
- "कंपनी के करंट या एंटिसिपेटेड बिज़नेस से रिलेटेड" (आपका एम्प्लॉयर क्या करता है उस पर निर्भर)

**मुख्य फ़्रेज़ जो आपको फ्री करते हैं:**
- "एम्प्लॉई के अपने समय पर, अपने रिसोर्सेज़ से और कंपनी बिज़नेस से अनरिलेटेड किया गया काम छोड़कर" (यह आपका कार्व-आउट है — कई US स्टेट्स इसकी ज़रूरत रखते हैं)
- कुछ स्टेट्स (California, Washington, Minnesota, Illinois, अन्य) में ऐसे कानून हैं जो पर्सनल प्रोजेक्ट्स पर एम्प्लॉयर IP क्लेम्स को लिमिट करते हैं, चाहे कॉन्ट्रैक्ट कुछ भी कहे।

### 3 सवालों का टेस्ट

किसी भी इनकम प्रोजेक्ट के लिए, पूछें:

1. **समय:** क्या आप यह काम अपने खुद के समय में कर रहे हैं? (वर्क ऑवर्स में नहीं, ऑन-कॉल शिफ्ट में नहीं)
2. **इक्विपमेंट:** क्या आप अपना हार्डवेयर, अपना इंटरनेट, अपनी API कीज़ इस्तेमाल कर रहे हैं? (एम्प्लॉयर लैपटॉप नहीं, एम्प्लॉयर VPN नहीं, एम्प्लॉयर क्लाउड अकाउंट्स नहीं)
3. **सब्जेक्ट मैटर:** क्या यह आपके एम्प्लॉयर के बिज़नेस से अनरिलेटेड है? (अगर आप हेल्थकेयर AI कंपनी में काम करते हैं और हेल्थकेयर AI सर्विसेज़ बेचना चाहते हैं... वो प्रॉब्लम है। अगर आप हेल्थकेयर AI कंपनी में काम करते हैं और रियल एस्टेट एजेंट्स के लिए डॉक्यूमेंट प्रोसेसिंग बेचना चाहते हैं... वो ठीक है।)

अगर तीनों जवाब क्लीन हैं, तो आप लगभग निश्चित रूप से ठीक हैं। अगर कोई जवाब मर्की है, आगे बढ़ने से पहले क्लैरिटी लें।

> **सीधी बात:** साइड वर्क करने वाले अधिकांश डेवलपर्स को कभी कोई समस्या नहीं होती। एम्प्लॉयर्स कॉम्पिटिटिव एडवांटेज प्रोटेक्ट करने में इंटरेस्टेड हैं, अनरिलेटेड प्रोजेक्ट्स पर एक्स्ट्रा पैसे कमाने से रोकने में नहीं। लेकिन "लगभग निश्चित" का मतलब "बिल्कुल निश्चित" नहीं। अगर कॉन्ट्रैक्ट असामान्य रूप से ब्रॉड है, मैनेजर या HR से बात करें — या वकील से सलाह लें। चेक न करने का डाउनसाइड पूछने की हल्की ऑक्वर्डनेस से कहीं ज़्यादा बुरा है।

### स्टेप 2: बिज़नेस स्ट्रक्चर चुनें

पर्सनल एसेट्स को बिज़नेस एक्टिविटीज़ से अलग करने के लिए लीगल एंटिटी चाहिए, और बिज़नेस बैंकिंग, पेमेंट प्रोसेसिंग, और टैक्स बेनिफ़िट्स का दरवाज़ा खोलने के लिए।

{? if regional.country ?}
> **आपकी लोकेशन: {= regional.country | fallback("Unknown") =}।** आपके रीजन के लिए रिकमेंडेड एंटिटी टाइप है **{= regional.business_entity_type | fallback("LLC or equivalent") =}**, टिपिकल रजिस्ट्रेशन कॉस्ट {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}। नीचे अपने देश के सेक्शन पर स्क्रॉल करें, या सभी सेक्शन पढ़ें ताकि समझ सकें कि दूसरे रीजन के क्लाइंट्स कैसे ऑपरेट करते हैं।
{? endif ?}

{? if regional.country == "US" ?}
#### United States (आपका रीजन)
{? else ?}
#### United States
{? endif ?}

| स्ट्रक्चर | कॉस्ट | प्रोटेक्शन | बेस्ट फ़ॉर |
|-----------|------|-----------|----------|
| **Sole Proprietorship** (डिफ़ॉल्ट) | $0 | कोई नहीं (पर्सनल लायबिलिटी) | पानी टेस्ट करना। पहले $1K। |
| **Single-Member LLC** | $50-500 (स्टेट अनुसार अलग) | पर्सनल एसेट प्रोटेक्शन | एक्टिव इनकम वर्क। ज़्यादातर डेवलपर्स यहाँ से शुरू करें। |
| **S-Corp इलेक्शन** (LLC पर) | LLC कॉस्ट + इलेक्शन के लिए $0 | LLC जैसा + पेरोल टैक्स बेनिफ़िट्स | जब कंसिस्टेंटली $40K+/साल कमा रहे हों |

**US डेवलपर्स के लिए रिकमेंडेड:** आपके रेज़िडेंस स्टेट में Single-Member LLC।

**फ़ॉर्म करने के लिए सबसे सस्ते स्टेट:** Wyoming ($100, कोई स्टेट इनकम टैक्स नहीं), New Mexico ($50), Montana ($70)। लेकिन अपने होम स्टेट में फ़ॉर्म करना आमतौर पर सबसे सिंपल है जब तक स्पेसिफ़िक कारण न हो।

**फ़ाइल कैसे करें:**
1. अपने स्टेट की Secretary of State वेबसाइट पर जाएँ
2. "form LLC" या "business entity filing" सर्च करें
3. Articles of Organization फ़ाइल करें (10-मिनट का फ़ॉर्म)
4. IRS से EIN लें (फ्री, irs.gov पर 5 मिनट लगते हैं)

{? if regional.country == "GB" ?}
#### United Kingdom (आपका रीजन)
{? else ?}
#### United Kingdom
{? endif ?}

| स्ट्रक्चर | कॉस्ट | प्रोटेक्शन | बेस्ट फ़ॉर |
|-----------|------|-----------|----------|
| **Sole Trader** | फ्री (HMRC से रजिस्टर) | कोई नहीं | पहली इनकम। टेस्टिंग। |
| **Limited Company (Ltd)** | ~$15 Companies House से | पर्सनल एसेट प्रोटेक्शन | कोई भी गंभीर इनकम वर्क। |

**रिकमेंडेड:** Companies House से Ltd कंपनी। लगभग 20 मिनट लगते हैं और GBP 12 कॉस्ट है।

#### European Union

देश के अनुसार काफ़ी अलग-अलग, लेकिन जनरल पैटर्न:

- **जर्मनी:** शुरुआत के लिए Einzelunternehmer (सोल प्रोप्राइटर), गंभीर काम के लिए GmbH (लेकिन GmbH को EUR 25,000 कैपिटल चाहिए — EUR 1 में UG पर विचार करें)
- **नीदरलैंड:** Eenmanszaak (सोल प्रोप्राइटर, फ्री रजिस्ट्रेशन) या BV (Ltd जैसा)
- **फ़्रांस:** Micro-entrepreneur (सिंप्लिफ़ाइड, शुरू करने के लिए रिकमेंडेड)
- **एस्टोनिया:** e-Residency + OUE (नॉन-रेज़िडेंट्स में पॉपुलर, पूरी तरह ऑनलाइन)

{? if regional.country == "AU" ?}
#### Australia (आपका रीजन)
{? else ?}
#### Australia
{? endif ?}

| स्ट्रक्चर | कॉस्ट | प्रोटेक्शन | बेस्ट फ़ॉर |
|-----------|------|-----------|----------|
| **Sole Trader** | फ्री ABN | कोई नहीं | शुरुआत |
| **Pty Ltd** | ~AUD 500-800 ASIC से | पर्सनल एसेट प्रोटेक्शन | गंभीर इनकम |

**रिकमेंडेड:** Sole Trader ABN (फ्री, इंस्टेंट) से शुरू करें, कंसिस्टेंटली कमाने लगें तब Pty Ltd पर मूव करें।

### स्टेप 3: पेमेंट प्रोसेसिंग (15-मिनट सेटअप)

पैसे लेने का तरीका चाहिए। यह अभी सेट करें, पहला क्लाइंट इंतज़ार करे तब नहीं।

{? if regional.payment_processors ?}
> **{= regional.country | fallback("your region") =} के लिए रिकमेंडेड:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (ज़्यादातर डेवलपर्स के लिए रिकमेंडेड):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

समय: ~15 मिनट। तुरंत पेमेंट एक्सेप्ट करना शुरू कर सकते हैं (Stripe नए अकाउंट पर 7 दिन फ़ंड होल्ड करता है)।

**Lemon Squeezy (डिजिटल प्रोडक्ट्स के लिए रिकमेंडेड):**

अगर डिजिटल प्रोडक्ट्स बेच रहे हैं (टेम्पलेट, टूल्स, कोर्सेज़, SaaS), Lemon Squeezy आपके Merchant of Record के रूप में काम करता है। इसका मतलब:
- वो ग्लोबली आपके लिए सेल्स टैक्स, VAT, और GST हैंडल करते हैं
- EU में VAT रजिस्टर करने की ज़रूरत नहीं
- वो रिफ़ंड और डिस्प्यूट हैंडल करते हैं

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (इंटरनेशनल डेवलपर्स या US एंटिटी चाहने वालों के लिए):**

अगर US के बाहर हैं लेकिन US एंटिटी से US कस्टमर्स को बेचना चाहते हैं:
- $500 वन-टाइम फ़ीस
- आपके लिए Delaware LLC बनाता है
- US बैंक अकाउंट सेट करता है (Mercury या Stripe से)
- रजिस्टर्ड एजेंट सर्विस प्रोवाइड करता है
- लगभग 1-2 हफ़्ते लगते हैं

### स्टेप 4: प्राइवेसी पॉलिसी और Terms of Service

अगर ऑनलाइन कोई सर्विस या प्रोडक्ट बेच रहे हैं, ये चाहिए। बॉयलरप्लेट के लिए वकील को पैसे न दें।

**टेम्पलेट के लिए फ्री, रिप्यूटेबल सोर्सेज़:**
- **Termly.io** — फ्री प्राइवेसी पॉलिसी और ToS जनरेटर। सवालों के जवाब दें, डॉक्यूमेंट पाएँ।
- **Avodocs.com** — स्टार्टअप्स के लिए ओपन-सोर्स लीगल डॉक्यूमेंट्स। फ्री।
- **GitHub's choosealicense.com** — ओपन-सोर्स प्रोजेक्ट लाइसेंस के लिए स्पेसिफ़िकली।
- **Basecamp की ओपन-सोर्स्ड पॉलिसीज़** — "Basecamp open source policies" सर्च करें — अच्छे, प्लेन-इंग्लिश टेम्पलेट।

**प्राइवेसी पॉलिसी में क्या कवर होना चाहिए (अगर कोई क्लाइंट डेटा प्रोसेस कर रहे हैं):**
- क्या डेटा कलेक्ट करते हैं
- कैसे प्रोसेस करते हैं (लोकली — यह आपकी एडवांटेज है)
- कितने समय रिटेन करते हैं
- क्लाइंट्स डिलीशन कैसे रिक्वेस्ट कर सकते हैं
- कोई थर्ड पार्टी डेटा एक्सेस करती है या नहीं (आदर्श: कोई नहीं)

**समय:** टेम्पलेट जनरेटर के साथ 30 मिनट। हो गया।

### स्टेप 5: अलग बैंक अकाउंट

बिज़नेस इनकम पर्सनल चेकिंग अकाउंट से न चलाएँ। कारण:

1. **टैक्स क्लैरिटी:** टैक्स टाइम पर, बिल्कुल पता होना चाहिए क्या बिज़नेस इनकम थी और क्या नहीं।
2. **लीगल प्रोटेक्शन:** अगर LLC है, पर्सनल और बिज़नेस फ़ंड्स मिलाना "कॉर्पोरेट वेल पियर्स" कर सकता है — मतलब कोर्ट आपके LLC की लायबिलिटी प्रोटेक्शन इग्नोर कर सकती है।
3. **प्रोफ़ेशनलिज़्म:** "जॉन'स कंसल्टिंग LLC" से इनवॉइस डेडिकेटेड बिज़नेस अकाउंट में आना लेजिटिमेट दिखता है। पर्सनल Venmo में पेमेंट नहीं।

**फ्री या लो-कॉस्ट बिज़नेस बैंकिंग:**
{? if regional.country == "US" ?}
- **Mercury** (आपके लिए रिकमेंडेड) — फ्री, स्टार्टअप्स के लिए डिज़ाइन्ड। बाद में बुककीपिंग ऑटोमेट करने के लिए एक्सीलेंट API।
- **Relay** — फ्री, इनकम स्ट्रीम्स को सब-अकाउंट्स में सेपरेट करने के लिए अच्छा।
{? elif regional.country == "GB" ?}
- **Starling Bank** (आपके लिए रिकमेंडेड) — फ्री बिज़नेस अकाउंट, इंस्टेंट सेटअप।
- **Wise Business** — लो-कॉस्ट मल्टी-करेंसी। इंटरनेशनल क्लाइंट्स सर्व करने के लिए बढ़िया।
{? else ?}
- **Mercury** (US) — फ्री, स्टार्टअप्स के लिए डिज़ाइन्ड। बाद में बुककीपिंग ऑटोमेट करने के लिए एक्सीलेंट API।
- **Relay** (US) — फ्री, इनकम स्ट्रीम्स को सब-अकाउंट्स में सेपरेट करने के लिए अच्छा।
- **Starling Bank** (UK) — फ्री बिज़नेस अकाउंट।
{? endif ?}
- **Wise Business** (इंटरनेशनल) — लो-कॉस्ट मल्टी-करेंसी। USD, EUR, GBP आदि में पेमेंट रिसीव करने के लिए बढ़िया।
- **Qonto** (EU) — यूरोपीयन कंपनियों के लिए क्लीन बिज़नेस बैंकिंग।

अकाउंट अभी खोलें। ऑनलाइन 10-15 मिनट लगते हैं और वेरिफ़िकेशन में 1-3 दिन।

### स्टेप 6: डेवलपर साइड इनकम के लिए टैक्स बेसिक्स

{? if regional.tax_note ?}
> **{= regional.country | fallback("your region") =} के लिए टैक्स नोट:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **सीधी बात:** टैक्स वो चीज़ है जिसे ज़्यादातर डेवलपर्स अप्रैल तक इग्नोर करते हैं, और फिर पैनिक करते हैं। अभी 30 मिनट खर्च करना आपके असली पैसे और स्ट्रेस बचाता है।

**United States:**
- $400/साल से ज़्यादा साइड इनकम पर सेल्फ़-एम्प्लॉयमेंट टैक्स (~15.3% Social Security + Medicare के लिए) ज़रूरी है
- प्लस नेट प्रॉफ़िट पर आपका रेगुलर इनकम टैक्स ब्रैकेट
- **क्वार्टरली एस्टिमेटेड टैक्स:** अगर $1,000 से ज़्यादा टैक्स देना होगा, IRS क्वार्टरली पेमेंट एक्सपेक्ट करता है (15 अप्रैल, 15 जून, 15 सितंबर, 15 जनवरी)। अंडरपेमेंट पर पेनल्टी लगती है।
- नेट इनकम का **25-30%** टैक्स के लिए अलग रखें। तुरंत अलग सेविंग्स अकाउंट में डालें।

**डेवलपर साइड इनकम के लिए कॉमन राइट-ऑफ़:**
- API कॉस्ट (OpenAI, Anthropic, आदि) — 100% डिडक्टिबल
- बिज़नेस यूज़ के लिए हार्डवेयर परचेज़ — डिप्रीशिएबल या Section 179 डिडक्शन
- बिज़नेस यूज़ के लिए बिजली कॉस्ट
- इनकम वर्क के लिए सॉफ़्टवेयर सब्सक्रिप्शन
- होम ऑफ़िस डिडक्शन (सिंप्लिफ़ाइड: $5/sq ft, 300 sq ft तक = $1,500)
- इंटरनेट (बिज़नेस-यूज़ परसेंटेज)
- डोमेन नेम्स, होस्टिंग, ईमेल सर्विसेज़
- इनकम वर्क से रिलेटेड प्रोफ़ेशनल डेवलपमेंट (कोर्सेज़, किताबें)

**United Kingdom:**
- Self Assessment टैक्स रिटर्न से रिपोर्ट करें
- GBP 1,000 से कम ट्रेडिंग इनकम: टैक्स-फ्री (Trading Allowance)
- उससे ज़्यादा: प्रॉफ़िट पर Income Tax + Class 4 NICs दें
- पेमेंट डेट: 31 जनवरी और 31 जुलाई

**पहले दिन से सब कुछ ट्रैक करें।** कुछ न हो तो सिंपल स्प्रेडशीट इस्तेमाल करें:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **आम गलती:** "टैक्स बाद में फ़िगर कर लूँगा।" बाद में Q4 है, एस्टिमेटेड टैक्स में $3,000 देना है प्लस पेनल्टी, और पैसे खर्च हो चुके हैं। ऑटोमेट करें: जब भी बिज़नेस अकाउंट में इनकम आए, तुरंत 30% टैक्स सेविंग्स अकाउंट में ट्रांसफ़र करें।

### पाठ 4 चेकपॉइंट

आपके पास अब (या प्लान) होना चाहिए:
- [ ] एम्प्लॉयमेंट कॉन्ट्रैक्ट का IP क्लॉज़ पढ़ा
- [ ] प्लान्ड इनकम वर्क के लिए 3 सवालों का टेस्ट पास किया
- [ ] बिज़नेस स्ट्रक्चर चुना (या सोल प्रोप्राइटर शुरू करने का फ़ैसला)
- [ ] पेमेंट प्रोसेसिंग सेट अप (Stripe या Lemon Squeezy)
- [ ] टेम्पलेट जनरेटर से प्राइवेसी पॉलिसी और ToS
- [ ] अलग बिज़नेस बैंक अकाउंट (या एप्लिकेशन सबमिटेड)
- [ ] टैक्स स्ट्रैटेजी: 30% सेट-असाइड + क्वार्टरली पेमेंट शेड्यूल

*पूरे STREETS कोर्स में, मॉड्यूल E (एक्ज़ीक्यूशन प्लेबुक) में फ़ाइनेंशियल मॉडलिंग टेम्पलेट्स हैं जो ऑटोमैटिकली टैक्स ऑब्लिगेशन, प्रोजेक्ट प्रॉफ़िटेबिलिटी, और हर रेवेन्यू इंजन के ब्रेक-ईवन पॉइंट कैलकुलेट करते हैं।*

---

## पाठ 5: {= regional.currency_symbol | fallback("$") =}200/महीने का बजट

*"आपके बिज़नेस की बर्न रेट है। जानो। कंट्रोल करो। कमाओ।"*

### {= regional.currency_symbol | fallback("$") =}200/महीने क्यों

दो सौ {= regional.currency | fallback("dollars") =} प्रति महीना डेवलपर इनकम ऑपरेशन के लिए मिनिमम वाइएबल बजट है। यह काफ़ी है रियल सर्विसेज़ चलाने, रियल कस्टमर्स सर्व करने, और रियल रेवेन्यू जनरेट करने के लिए। और यह इतना छोटा भी है कि अगर कुछ काम नहीं करता, तो सब कुछ दांव पर नहीं लगाया।

गोल सिंपल है: **{= regional.currency_symbol | fallback("$") =}200/महीने को 90 दिनों में {= regional.currency_symbol | fallback("$") =}600+/महीने में बदलो।** अगर कर सकते हो, तो बिज़नेस है। अगर नहीं कर सकते, स्ट्रैटेजी बदलो — बजट नहीं।

### बजट ब्रेकडाउन

#### टियर 1: API क्रेडिट्स — $50-100/महीने

यह कस्टमर-फेसिंग क्वालिटी के लिए आपका प्रोडक्शन कंप्यूट है।

**रिकमेंडेड स्टार्टिंग एलोकेशन:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**API स्पेंड कैसे मैनेज करें:**

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

**हाइब्रिड स्पेंड स्ट्रैटेजी:**
- प्रोसेसिंग का 80% लोकल LLM इस्तेमाल करें (क्लासिफ़िकेशन, एक्सट्रैक्शन, समराइज़ेशन, ड्राफ्ट)
- प्रोसेसिंग का 20% API कॉल्स इस्तेमाल करें (फ़ाइनल क्वालिटी पास, कॉम्प्लेक्स रीज़निंग, कस्टमर-फेसिंग आउटपुट)
- प्योर API यूसेज की तुलना में आपकी इफ़ेक्टिव कॉस्ट प्रति टास्क नाटकीय रूप से गिरती है

{? if computed.monthly_electricity_estimate ?}
> **आपकी एस्टिमेटेड बिजली कॉस्ट:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/महीने 24/7 ऑपरेशन के लिए {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh पर। यह पहले से आपकी इफ़ेक्टिव ऑपरेटिंग कॉस्ट में फ़ैक्टर है।
{? endif ?}

#### टियर 2: इंफ्रास्ट्रक्चर — {= regional.currency_symbol | fallback("$") =}30-50/महीने

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**VPS चाहिए?**

अगर आपका इनकम मॉडल है:
- **डिजिटल प्रोडक्ट्स बेचना:** नहीं। Vercel/Netlify पर फ्री होस्ट करें। डिलीवरी के लिए Lemon Squeezy।
- **क्लाइंट्स के लिए एसिंक प्रोसेसिंग:** शायद। लोकल रिग पर जॉब चला सकते हैं और रिज़ल्ट डिलीवर कर सकते हैं। VPS रिलायबिलिटी जोड़ता है।
- **API सर्विस ऑफ़र करना:** हाँ, शायद। $5-10 VPS लाइटवेट API गेटवे के रूप में काम करता है, भले ही हेवी प्रोसेसिंग लोकल मशीन पर हो।
- **SaaS बेचना:** हाँ। लेकिन सबसे सस्ते टियर से शुरू करें और स्केल अप करें।

**रिकमेंडेड स्टार्टर इंफ्रास्ट्रक्चर:**

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

टोटल इंफ्रास्ट्रक्चर कॉस्ट: $5-20/महीने। बाकी फ्री टियर।

#### टियर 3: टूल्स — {= regional.currency_symbol | fallback("$") =}20-30/महीने

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

> **सीधी बात:** शुरू करते समय पूरा टूल स्टैक फ्री टियर पर चला सकते हैं। यहाँ एलोकेट किए $20-30 तब के लिए हैं जब फ्री टियर आउटग्रो करें या कोई स्पेसिफ़िक प्रीमियम फ़ीचर चाहें। बस बजट में है इसलिए खर्च न करें। अनस्पेंट बजट प्रॉफ़िट है।

#### टियर 4: रिज़र्व — {= regional.currency_symbol | fallback("$") =}0-30/महीने

यह "जो एंटिसिपेट नहीं किया" फ़ंड है:
- अनएक्सपेक्टेडली बड़ी बैच जॉब से API कॉस्ट स्पाइक
- एक स्पेसिफ़िक क्लाइंट प्रोजेक्ट के लिए ज़रूरी टूल
- परफ़ेक्ट नेम मिलने पर इमरजेंसी डोमेन परचेज
- वन-टाइम परचेज (थीम, टेम्पलेट, आइकन सेट)

अगर रिज़र्व यूज़ नहीं होता, एक्यूम्युलेट होता है। 3 महीने अनयूज़्ड रिज़र्व के बाद, API क्रेडिट्स या इंफ्रास्ट्रक्चर में रीएलोकेट करने पर विचार करें।

### ROI कैलकुलेशन

सिर्फ़ यही एक नंबर मायने रखता है:

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

**बजट कब बढ़ाएँ:**

बजट सिर्फ़ तब बढ़ाएँ जब:
1. 2+ महीनों से कंसिस्टेंटली 2x+ ROI हो
2. ज़्यादा खर्च सीधे रेवेन्यू बढ़ाए (जैसे: ज़्यादा API क्रेडिट्स = ज़्यादा क्लाइंट कैपेसिटी)
3. बढ़ोतरी किसी स्पेसिफ़िक, टेस्टेड रेवेन्यू स्ट्रीम से जुड़ी हो

**बजट कब न बढ़ाएँ:**
- "मुझे लगता है यह नया टूल हेल्प करेगा" (पहले फ्री अल्टरनेटिव टेस्ट करें)
- "सब कहते हैं पैसे कमाने के लिए पैसे खर्च करने होते हैं" (इस स्टेज पर नहीं)
- "बड़ा VPS मेरी सर्विस फ़ास्टर बनाएगा" (क्या स्पीड सच में बॉटलनेक है?)
- अभी तक 1x ROI नहीं हुआ (रेवेन्यू फ़िक्स करो, स्पेंड नहीं)

**स्केलिंग लैडर:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **आम गलती:** {= regional.currency_symbol | fallback("$") =}200 को "इनवेस्टमेंट" मानना जिसे तुरंत रिटर्न नहीं चाहिए। नहीं। यह 90-दिन की डेडलाइन वाला एक्सपेरिमेंट है। अगर {= regional.currency_symbol | fallback("$") =}200/महीने 90 दिनों में {= regional.currency_symbol | fallback("$") =}200/महीने रेवेन्यू जनरेट नहीं करता, स्ट्रैटेजी में कुछ बदलना चाहिए। पैसा, मार्केट, ऑफ़र — कुछ काम नहीं कर रहा। अपने आप से ईमानदार रहें।

### पाठ 5 चेकपॉइंट

आपके पास अब होना चाहिए:
- [ ] ~$200 का मंथली बजट चार टियर में एलोकेटेड
- [ ] स्पेंडिंग लिमिट्स सेट के साथ API अकाउंट्स बने
- [ ] इंफ्रास्ट्रक्चर डिसीज़न लिए (लोकल-ओनली vs. लोकल + VPS)
- [ ] टूल स्टैक सेलेक्ट (ज़्यादातर फ्री टियर शुरुआत में)
- [ ] ROI टार्गेट: 90 दिनों में 3x
- [ ] क्लीयर रूल: ROI प्रूव होने के बाद ही बजट बढ़ाएँ

*पूरे STREETS कोर्स में, मॉड्यूल E (एक्ज़ीक्यूशन प्लेबुक) में फ़ाइनेंशियल डैशबोर्ड टेम्पलेट है जो आपका स्पेंड, रेवेन्यू, और ROI प्रति रेवेन्यू इंजन रियल-टाइम में ट्रैक करता है — ताकि आप हमेशा जानें कौन से स्ट्रीम्स प्रॉफ़िटेबल हैं और कौन से एडजस्टमेंट चाहते हैं।*

---

## पाठ 6: आपका सॉवरेन स्टैक डॉक्यूमेंट

*"हर बिज़नेस का एक प्लान होता है। यह आपका है — और दो पेज में आ जाता है।"*

### डिलीवरेबल

यह मॉड्यूल S में आपकी सबसे ज़रूरी चीज़ है। सॉवरेन स्टैक डॉक्यूमेंट एक सिंगल रेफ़रेंस है जो आपके इनकम-जनरेटिंग इंफ्रास्ट्रक्चर के बारे में सब कुछ कैप्चर करता है। बाकी STREETS कोर्स में इसे रेफ़र करेंगे, सेटअप इवॉल्व होने पर अपडेट करेंगे, और क्या बनाना है क्या स्किप करना है के बारे में क्लीयर-हेडेड डिसीज़न लेने के लिए इस्तेमाल करेंगे।

नई फ़ाइल बनाएँ। Markdown, Google Doc, Notion पेज, प्लेन टेक्स्ट — जो भी आप वाकई मेंटेन करेंगे। नीचे का टेम्पलेट इस्तेमाल करें, हर फ़ील्ड पाठ 1-5 के नंबर और डिसीज़न से भरें।

### टेम्पलेट

{? if computed.profile_completeness != "0" ?}
> **हेड स्टार्ट:** 4DA ने पहले ही आपके कुछ हार्डवेयर स्पेक्स और स्टैक इन्फ़ो डिटेक्ट कर ली हैं। नीचे प्री-फ़िल्ड हिंट्स ढूँढें — टेम्पलेट भरने में समय बचाएँगे।
{? endif ?}

यह पूरा टेम्पलेट कॉपी करें और भरें। हर फ़ील्ड। बिना स्किप।

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
> **आपके Developer DNA से प्री-फ़िल:**
> - **प्राइमरी स्टैक:** {= dna.primary_stack | fallback("Not detected") =}
> - **इंटरेस्ट:** {= dna.interests | fallback("Not detected") =}
> - **आइडेंटिटी समरी:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **ध्यान रखने वाले ब्लाइंड स्पॉट:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **डिटेक्टेड स्टैक से प्री-फ़िल:** आपकी प्राइमरी टेक्नोलॉजीज़ हैं {= stack.primary | fallback("not yet detected") =}। {? if stack.adjacent ?}एडजेसेंट स्किल्स: {= stack.adjacent | fallback("none detected") =}।{? endif ?} ऊपर स्किल्स इन्वेंटरी भरने के लिए इन्हें इस्तेमाल करें।
{? endif ?}

{@ insight t_shape @}

### यह डॉक्यूमेंट कैसे इस्तेमाल करें

1. **कोई भी नया प्रोजेक्ट शुरू करने से पहले:** सॉवरेन स्टैक चेक करें। क्या हार्डवेयर, टाइम, स्किल्स, और बजट है एक्ज़ीक्यूट करने के लिए?
2. **कुछ भी खरीदने से पहले:** बजट एलोकेशन चेक करें। क्या यह परचेज प्लान में है?
3. **मंथली रिव्यू:** बजट में "Actual" कॉलम अपडेट करें। रेवेन्यू नंबर अपडेट करें। जो काम कर रहा है उसके बेसिस पर एलोकेशन एडजस्ट करें।
4. **जब कोई पूछे आप क्या करते हैं:** "What I Can Offer Today" सेक्शन आपकी इंस्टेंट पिच है।
5. **जब कोई चमकदार नया आइडिया लुभाए:** अपने कंस्ट्रेंट्स चेक करें। क्या यह टाइम, स्किल्स, और हार्डवेयर में फ़िट होता है? अगर नहीं, "Building Toward" में बाद के लिए एड करें।

### एक घंटे की एक्सरसाइज़

60 मिनट का टाइमर लगाएँ। टेम्पलेट का हर फ़ील्ड भरें। ज़्यादा न सोचें। एक्सटेंसिव रिसर्च न करें। जो अभी जानते हैं वो लिखें। बाद में अपडेट कर सकते हैं।

जो फ़ील्ड भर नहीं सकते? वो इस हफ़्ते के एक्शन आइटम हैं:
- बेंचमार्क नंबर खाली? पाठ 2 से बेंचमार्क स्क्रिप्ट चलाएँ।
- कोई बिज़नेस एंटिटी नहीं? पाठ 4 से फ़ाइलिंग प्रोसेस शुरू करें।
- कोई पेमेंट प्रोसेसिंग नहीं? पाठ 4 से Stripe सेट करें।
- स्किल्स इन्वेंटरी खाली? 15 मिनट लगाकर पिछले 5 साल में जिसके लिए पैसे मिले सब लिस्ट करें।

> **आम गलती:** डॉक्यूमेंट "परफ़ेक्ट" बनाने में 3 घंटे लगाना बजाय "डन" बनाने में 1 घंटा। सॉवरेन स्टैक डॉक्यूमेंट एक वर्किंग रेफ़रेंस है, इनवेस्टर्स के लिए बिज़नेस प्लान नहीं। आपके अलावा कोई नहीं देखेगा। एक्युरेसी मायने रखती है। फ़ॉर्मेटिंग नहीं।

### पाठ 6 चेकपॉइंट

आपके पास अब होना चाहिए:
- [ ] कम्प्लीट सॉवरेन स्टैक डॉक्यूमेंट कहीं सेव जहाँ वाकई खोलेंगे
- [ ] सभी छह सेक्शन रियल नंबर से भरे (एस्पिरेशनल नहीं)
- [ ] सेटअप में गैप के लिए एक्शन आइटम की क्लीयर लिस्ट
- [ ] पहली मंथली रिव्यू के लिए डेट सेट (अभी से 30 दिन)

---

## मॉड्यूल S: कम्प्लीट

{? if progress.completed("MODULE_S") ?}
> **मॉड्यूल S कम्प्लीट।** {= progress.total_count | fallback("7") =} STREETS मॉड्यूल में से {= progress.completed_count | fallback("1") =} पूरे किए। {? if progress.completed_modules ?}कम्प्लीटेड: {= progress.completed_modules | fallback("S") =}।{? endif ?}
{? endif ?}

### दो हफ्तों में आपने क्या बनाया

देखें आपके पास अब क्या है जो शुरू करते वक्त नहीं था:

1. **हार्डवेयर इन्वेंटरी** इनकम-जनरेटिंग कैपेबिलिटीज़ से मैप्ड — सिर्फ़ स्टिकर पर स्पेक्स नहीं।
2. **प्रोडक्शन-ग्रेड लोकल LLM स्टैक** Ollama के साथ, आपके असल हार्डवेयर पर बेंचमार्क्ड, रियल वर्कलोड के लिए कॉन्फ़िगर्ड।
3. **प्राइवेसी एडवांटेज** जिसे मार्केट करना आप जानते हैं — स्पेसिफ़िक ऑडियंस के लिए स्पेसिफ़िक लैंग्वेज।
4. **लीगल और फ़ाइनेंशियल फ़ाउंडेशन** — बिज़नेस एंटिटी (या प्लान), पेमेंट प्रोसेसिंग, बैंक अकाउंट, टैक्स स्ट्रैटेजी।
5. **कंट्रोल्ड बजट** क्लीयर ROI टार्गेट और मॉडल प्रूव करने की 90-दिन डेडलाइन के साथ।
6. **सॉवरेन स्टैक डॉक्यूमेंट** जो ऊपर सब कुछ एक सिंगल रेफ़रेंस में कैप्चर करता है जो आगे हर डिसीज़न के लिए इस्तेमाल करेंगे।

यह ज़्यादातर डेवलपर्स जितना सेट करते हैं उससे ज़्यादा है। सीरियसली। ज़्यादातर लोग जो साइड इनकम चाहते हैं सीधे "कुछ कूल बनाओ" पर कूद जाते हैं और फिर सोचते हैं कि पैसे क्यों नहीं मिल रहे। आपके पास अब पैसे पाने का इंफ्रास्ट्रक्चर है।

लेकिन दिशा के बिना इंफ्रास्ट्रक्चर बस एक महंगी हॉबी है। आपको जानना होगा कि इस स्टैक को कहाँ एम करें।

{@ temporal market_timing @}

### आगे क्या: मॉड्यूल T — टेक्निकल मोट्स

मॉड्यूल S ने फ़ाउंडेशन दिया। मॉड्यूल T क्रिटिकल सवाल का जवाब देता है: **ऐसा कुछ कैसे बनाएँ जो कॉम्पिटिटर्स आसानी से कॉपी न कर सकें?**

मॉड्यूल T यह कवर करता है:

- **प्रोप्राइटरी डेटा पाइपलाइन** — ऐसे डेटासेट कैसे बनाएँ जो सिर्फ़ आपके पास हों, कानूनी और एथिकल तरीके से
- **कस्टम मॉडल कॉन्फ़िगरेशन** — फ़ाइन-ट्यूनिंग और प्रॉम्प्ट इंजीनियरिंग जो ऐसी आउटपुट क्वालिटी प्रोड्यूस करे जो दूसरे डिफ़ॉल्ट सेटिंग्स से मैच न कर सकें
- **कम्पाउंडिंग स्किल स्टैक** — "Python + हेल्थकेयर" इनकम के लिए "Python + JavaScript" को क्यों हराता है, और अपनी यूनीक कॉम्बिनेशन कैसे आइडेंटिफ़ाई करें
- **टेक्निकल बैरियर्स टू एंट्री** — इंफ्रास्ट्रक्चर डिज़ाइन जो कॉम्पिटिटर को रेप्लिकेट करने में महीने लगें
- **मोट ऑडिट** — यह इवैल्यूएट करने का फ्रेमवर्क कि क्या आपके प्रोजेक्ट में डिफ़ेंसिबल एडवांटेज है या बस एक और कमोडिटी सर्विस है

$500/महीने कमाने वाले डेवलपर और $5,000/महीने कमाने वाले में फ़र्क़ शायद ही कभी स्किल होता है। मोट्स होती हैं। वो चीज़ें जो आपकी ऑफ़रिंग को हार्ड टू रेप्लिकेट बनाती हैं, भले ही किसी के पास वही हार्डवेयर और वही मॉडल हों।

### पूरा STREETS रोडमैप

| मॉड्यूल | टाइटल | फ़ोकस | अवधि |
|---------|-------|-------|------|
| **S** | सॉवरेन सेटअप | इंफ्रास्ट्रक्चर, कानूनी, बजट | सप्ताह 1-2 (कम्प्लीट) |
| **T** | टेक्निकल मोट्स | डिफ़ेंसिबल एडवांटेज, प्रोप्राइटरी एसेट्स | सप्ताह 3-4 |
| **R** | रेवेन्यू इंजन्स | कोड सहित स्पेसिफ़िक मॉनेटाइज़ेशन प्लेबुक | सप्ताह 5-8 |
| **E** | एक्ज़ीक्यूशन प्लेबुक | लॉन्च सीक्वेंसेज़, प्राइसिंग, पहले कस्टमर्स | सप्ताह 9-10 |
| **E** | इवॉल्विंग एज | आगे रहना, ट्रेंड डिटेक्शन, अडैप्टेशन | सप्ताह 11-12 |
| **T** | टैक्टिकल ऑटोमेशन | पैसिव इनकम के लिए ऑपरेशन ऑटोमेट करना | सप्ताह 13-14 |
| **S** | स्टैकिंग स्ट्रीम्स | मल्टीपल इनकम सोर्सेज़, पोर्टफ़ोलियो स्ट्रैटेजी | सप्ताह 15-16 |

मॉड्यूल R (रेवेन्यू इंजन्स) वो है जहाँ ज़्यादातर पैसे बनते हैं। लेकिन S और T के बिना, रेत पर बना रहे हैं।

---

**पूरे प्लेबुक के लिए तैयार?**

आपने फ़ाउंडेशन देखा है। खुद बनाया है। अब पूरा सिस्टम पाएँ।

**STREETS Core पाएँ** — पूरा 16-हफ़्ते का कोर्स सातों मॉड्यूल के साथ, रेवेन्यू इंजन कोड टेम्पलेट्स, फ़ाइनेंशियल डैशबोर्ड, और डेवलपर्स का प्राइवेट कम्युनिटी जो अपनी शर्तों पर इनकम बना रहे हैं।

*आपका रिग। आपके नियम। आपकी कमाई।*
