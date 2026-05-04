# Modül S: Egemen Kurulum

**STREETS Geliştirici Gelir Kursu — Ücretsiz Modül**
*Hafta 1-2 | 6 Ders | Çıktı: Senin Egemen Yığın Belgen*

> "Bilgisayarın senin iş altyapın. Ona öyle davran."

---

Çoğu insanın asla sahip olamayacağı en güçlü gelir üreten araca zaten sahipsin: internet bağlantısı, yerel işlem gücü ve hepsini bir araya getirecek becerilere sahip bir geliştirici iş istasyonu.

Çoğu geliştirici bilgisayarını bir tüketici ürünü gibi kullanır. Oyun oynadığı, kod yazdığı, internette gezdiği bir şey. Ama aynı makine — şu an masanın altında duran o makine — çıkarım yapabilir, API sunabilir, veri işleyebilir ve sen uyurken günde 24 saat gelir üretebilir.

Bu modül, sahip olduklarına farklı bir açıdan bakmakla ilgili. "Ne inşa edebilirim?" değil, "Ne satabilirim?"

Bu iki haftanın sonunda şunlara sahip olacaksın:

- Gelir üreten yeteneklerinin net bir envanteri
- Üretim kalitesinde yerel bir LLM yığını
- Yasal ve finansal bir temel (minimal olsa bile)
- İş planın haline gelecek yazılı bir Egemen Yığın Belgesi

Havadan konuşma yok. "Sadece kendine güven" yok. Gerçek sayılar, gerçek komutlar, gerçek kararlar.

{@ mirror sovereign_readiness @}

Hadi başlayalım.

---

## Ders 1: Donanım Denetimi

*"4090'a ihtiyacın yok. İşte asıl önemli olan."*

### Makinenin Bir İş Varlığı

Bir şirket altyapısını değerlendirirken sadece özellikleri listelemez — yetenekleri gelir fırsatlarıyla eşleştirir. Şu an yapacağın tam olarak bu.

{? if computed.profile_completeness != "0" ?}
> **Mevcut Donanımın:** {= profile.cpu.model | fallback("Bilinmeyen CPU") =} ({= profile.cpu.cores | fallback("?") =} çekirdek / {= profile.cpu.threads | fallback("?") =} iş parçacığı), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("Özel GPU yok") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} boş / {= profile.storage.total | fallback("?") =} toplam ({= profile.storage.type | fallback("bilinmiyor") =}), {= profile.os.name | fallback("bilinmeyen İS") =} {= profile.os.version | fallback("") =} çalıştırıyor.
{? endif ?}

Bir terminal aç ve aşağıdakileri çalıştır. Her sayıyı yaz. Ders 6'daki Egemen Yığın Belgen için bunlara ihtiyacın olacak.

### Donanım Envanteri

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

**Gelir için önemli olan:**
- Çekirdek sayısı, bilgisayarının aynı anda kaç görevi işleyebileceğini belirler. Yerel bir LLM çalıştırırken aynı anda bir toplu iş işlemek gerçek paralellik gerektirir.
{? if profile.cpu.cores ?}
- *Senin {= profile.cpu.model | fallback("CPU") =} işlemcinin {= profile.cpu.cores | fallback("?") =} çekirdeği var — CPU'nun hangi gelir motorlarını desteklediğini görmek için aşağıdaki gereksinimler tablosunu kontrol et.*
{? endif ?}
- Bu kurstaki çoğu gelir motoru için son 5 yıldan herhangi bir modern 8+ çekirdekli CPU yeterlidir.
- Yerel LLM'leri yalnızca CPU üzerinde çalıştırıyorsan (GPU yok), 16+ çekirdek istersin. Ryzen 7 5800X veya Intel i7-12700 pratik alt sınırdır.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Gelir için önemli olan:**
- 16 GB: Asgari minimum. 7B modeller çalıştırabilir ve temel otomasyon işleri yapabilirsin.
- 32 GB: Rahat. 13B modelleri yerel olarak çalıştır, birden fazla projeyi idare et, geliştirme ortamını gelir iş yüklerinin yanında çalışır durumda tut.
- 64 GB+: CPU üzerinde 30B+ modeller çalıştırabilir veya birden fazla modeli yüklü tutabilirsin. İşler burada çıkarım hizmetleri satmak açısından ilginçleşiyor.
{? if profile.ram.total ?}
*Sisteminde {= profile.ram.total | fallback("?") =} RAM var. Hangi yetenek seviyesinde olduğunu görmek için yukarıdaki tabloyu kontrol et — bu, gelir iş yüklerin için hangi yerel modellerin pratik olduğunu doğrudan etkiler.*
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

**Gelir için önemli olan:**

Bu, insanların takıntılı olduğu tek özellik ve işte dürüst gerçek: **GPU'n yerel LLM seviyeni belirler ve yerel LLM seviyen hangi gelir akışlarının en hızlı çalışacağını belirler.** Ama para kazanıp kazanamayacağını belirlemez.

| VRAM | LLM Yeteneği | Gelir İlgisi |
|------|---------------|------------------|
| 0 (Yalnızca CPU) | 7B modeller ~5 token/sn | Toplu işleme, asenkron iş. Yavaş ama işlevsel. |
| 6-8 GB (RTX 3060, vb.) | 7B modeller ~30 tok/sn, 13B nicelenmiş | Çoğu otomasyon gelir akışı için yeterli. |
| 12 GB (RTX 3060 12GB, 4070) | 13B tam hızda, 30B nicelenmiş | Tatlı nokta. Çoğu gelir motoru burada iyi çalışır. |
| 16-24 GB (RTX 4090, 3090) | 30B-70B modeller | Premium seviye. Başkalarının yerel olarak sunamadığı kaliteyi sat. |
| 48 GB+ (çift GPU, A6000) | 70B+ hızda | Kurumsal düzey yerel çıkarım. Ciddi rekabet avantajı. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ birleşik bellek kullanarak | Mükemmel verimlilik. NVIDIA eşdeğerinden daha düşük güç maliyeti. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **Senin GPU'n:** {= profile.gpu.model | fallback("Bilinmiyor") =}, {= profile.gpu.vram | fallback("?") =} VRAM ile — {? if computed.gpu_tier == "premium" ?}premium seviyedesin. 30B-70B modeller yerel olarak erişilebilir. Bu ciddi bir rekabet avantajı.{? elif computed.gpu_tier == "sweet_spot" ?}tatlı noktadasın. 13B tam hızda, 30B nicelenmiş. Çoğu gelir motoru burada iyi çalışır.{? elif computed.gpu_tier == "capable" ?}7B modelleri iyi hızda ve 13B'yi nicelenmiş olarak çalıştırabilirsin. Çoğu otomasyon gelir akışı için yeterli.{? else ?}GPU hızlandırman mevcut. Nerede olduğunu görmek için yukarıdaki tabloyu kontrol et.{? endif ?}
{? else ?}
> **Özel GPU algılanmadı.** Çıkarımı CPU üzerinde çalıştıracaksın, bu da 7B modellerde ~5-12 token/sn demek. Bu, toplu işleme ve asenkron iş için sorun değil. Müşteriye yönelik çıktılarda hız farkını kapatmak için API çağrıları kullan.
{? endif ?}

> **Açık Konuşalım:** RTX 3060 12GB'ın varsa, yapay zekayı paraya çevirmeye çalışan geliştiricilerin %95'inden daha iyi konumdasın. 4090 beklemeyi bırak. 3060 12GB, yerel yapay zekanın Honda Civic'idir — güvenilir, verimli, işi halleder. GPU yükseltmesine harcayacağın para, yerel modellerinin ağır işleri halletmesi sırasında müşteriye yönelik kalite için API kredilerine daha iyi harcanır.

#### Depolama

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Gelir için önemli olan:**
- LLM modelleri yer kaplar: 7B model = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (nicelenmiş).
- Proje verileri, veritabanları, önbellekler ve çıktı dosyaları için alan gerekir.
- Müşteriye yönelik herhangi bir şey için SSD tartışmasız gereklidir. HDD'den model yükleme 30-60 saniye başlangıç süresi ekler.
- Minimum pratik: En az 100 GB boş alan olan 500 GB SSD.
- Rahat: 1 TB SSD. Modelleri SSD'de tut, arşivleri HDD'ye koy.
{? if profile.storage.free ?}
*{= profile.storage.type | fallback("sürücünde") =} üzerinde {= profile.storage.free | fallback("?") =} boş alanın var. {? if profile.storage.type == "SSD" ?}İyi — SSD hızlı model yüklemesi demek.{? elif profile.storage.type == "NVMe" ?}Mükemmel — NVMe model yükleme için en hızlı seçenek.{? else ?}Henüz SSD'de değilsen bir tane düşün — model yükleme süreleri için gerçekten fark yaratır.{? endif ?}*
{? endif ?}

#### Ağ

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**Gelir için önemli olan:**
{? if profile.network.download ?}
*Bağlantın: {= profile.network.download | fallback("?") =} indirme / {= profile.network.upload | fallback("?") =} yükleme.*
{? endif ?}
- İndirme hızı: 50+ Mbps. Modelleri, paketleri ve verileri çekmek için gerekli.
- Yükleme hızı: Çoğu kişinin görmezden geldiği darboğaz budur. Herhangi bir şey sunuyorsan (API'ler, işlenmiş sonuçlar, teslim edilecekler), yükleme önemlidir.
  - 10 Mbps: Asenkron teslimat için yeterli (işlenmiş dosyalar, toplu sonuçlar).
  - 50+ Mbps: Harici servislerin eriştiği herhangi bir yerel API uç noktası çalıştırıyorsan gereklidir.
  - 100+ Mbps: Bu kurstaki her şey için rahat.
- Gecikme: Büyük bulut sağlayıcılara 50ms altında. Kontrol etmek için `ping api.openai.com` ve `ping api.anthropic.com` çalıştır.

#### Çalışma Süresi

Bu, kimsenin düşünmediği özellik, ama hobi olarak yapanları uyurken para kazananlardan ayıran şey.

Kendine sor:
- Bilgisayarın 7/24 çalışabilir mi? (Güç, soğutma, gürültü)
- Elektrik kesintileri için bir UPS'in var mı?
- İnternet bağlantın otomatik iş akışları için yeterince kararlı mı?
- Bir şey bozulursa makinene uzaktan SSH ile bağlanabiliyor musun?

7/24 çalıştıramıyorsan sorun değil — bu kurstaki birçok gelir akışı, elle tetiklediğin asenkron toplu işlerdir. Ama gerçekten pasif gelir üreten işler çalışma süresi gerektirir.

{? if computed.os_family == "windows" ?}
**Hızlı çalışma süresi kurulumu (Windows):** Otomatik yeniden başlatma için Görev Zamanlayıcı'yı kullan, uzak erişim için Uzak Masaüstü'nü etkinleştir veya Tailscale kur, ve kesintilerden kurtulmak için BIOS'unda "AC güç kaybında geri yükle" ayarını yapılandır.
{? endif ?}

**Hızlı çalışma süresi kurulumu (istersen):**

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

### Elektrik Hesabı

İnsanlar ya bunu görmezden gelir ya da felaket senaryoları çizer. Gerçek matematik yapalım.

**Gerçek güç tüketimini ölçmek:**

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

**Aylık maliyet hesabı:**

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
Elektrik tarifen: yaklaşık {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh ({= regional.country | fallback("bölgen") =} ortalamalarına göre). Gerçek faturanı kontrol et — tarifeler sağlayıcıya ve günün saatine göre değişir.
{? else ?}
ABD ortalama elektrik fiyatı yaklaşık $0.12/kWh. Gerçek tarifeni kontrol et — büyük farklılıklar gösterir. Kaliforniya $0.25/kWh olabilir. Bazı Avrupa ülkeleri $0.35/kWh'e çıkar. ABD Ortabatısı'nın bazı bölgeleri $0.08/kWh'dir.
{? endif ?}

**Sonuç:** Bilgisayarını gelir için 7/24 çalıştırmak elektrik olarak aylık {= regional.currency_symbol | fallback("$") =}1 ile {= regional.currency_symbol | fallback("$") =}30 arasında bir maliyete sahip. Gelir akışların bunu karşılayamıyorsa, sorun elektrik değildir — sorun gelir akışıdır.

### Gelir Motoru Türüne Göre Minimum Özellikler

İşte tam STREETS kursunda nereye gidiyoruz diye bir ön izleme. Şimdilik sadece bilgisayarının nereye düştüğünü kontrol et:

| Gelir Motoru | CPU | RAM | GPU | Depolama | Ağ |
|---------------|-----|-----|-----|---------|---------|
| **İçerik otomasyonu** (blog yazıları, bültenler) | 4+ çekirdek | 16 GB | İsteğe bağlı (API yedek) | 50 GB boş | 10 Mbps yükleme |
| **Veri işleme hizmetleri** | 8+ çekirdek | 32 GB | İsteğe bağlı | 200 GB boş | 50 Mbps yükleme |
| **Yerel AI API hizmetleri** | 8+ çekirdek | 32 GB | 8+ GB VRAM | 100 GB boş | 50 Mbps yükleme |
| **Kod üretim araçları** | 8+ çekirdek | 16 GB | 8+ GB VRAM veya API | 50 GB boş | 10 Mbps yükleme |
| **Doküman işleme** | 4+ çekirdek | 16 GB | İsteğe bağlı | 100 GB boş | 10 Mbps yükleme |
| **Otonom ajanlar** | 8+ çekirdek | 32 GB | 12+ GB VRAM | 100 GB boş | 50 Mbps yükleme |

> **Yaygın Hata:** "Başlamadan önce donanımımı yükseltmem gerekiyor." Hayır. Sahip olduğunla başla. Donanımının karşılayamadığı boşlukları doldurmak için API çağrıları kullan. Gelir haklı çıkardığında yükselt — öncesinde değil.

{@ insight engine_ranking @}

### Ders 1 Kontrol Noktası

Şimdi şunları yazmış olmalısın:
- [ ] CPU modeli, çekirdek ve iş parçacık sayısı
- [ ] RAM miktarı
- [ ] GPU modeli ve VRAM (veya "yok")
- [ ] Mevcut depolama alanı
- [ ] Ağ hızları (indirme/yükleme)
- [ ] 7/24 çalışma için tahmini aylık elektrik maliyeti
- [ ] Bilgisayarının hangi gelir motoru kategorilerine uygun olduğu

Bu sayıları sakla. Ders 6'da Egemen Yığın Belgene gireceksin.

{? if computed.profile_completeness != "0" ?}
> **4DA bu sayıların çoğunu senin için zaten topladı.** Yukarıdaki kişiselleştirilmiş özetleri kontrol et — donanım envanterin sistem algılamasından kısmen önceden doldurulmuş.
{? endif ?}

*Tam STREETS kursunda, Modül R (Gelir Motorları), yukarıda listelenen her motor türü için adım adım taktik kitapları verir — hizmetleri oluşturmak ve dağıtmak için gereken tam kodu dahil.*

---

## Ders 2: Yerel LLM Yığını

*"Ollama'yı üretim kullanımı için kur — sadece sohbet için değil."*

### Yerel LLM'ler Gelir İçin Neden Önemli

OpenAI API'sini her çağırdığında kira ödüyorsun. Bir modeli her yerel olarak çalıştırdığında, ilk kurulumdan sonra o çıkarım ücretsiz. Matematik basit:

- GPT-4o: ~1 milyon giriş tokenı başına $5, ~1 milyon çıkış tokenı başına $15
- Claude 3.5 Sonnet: ~1 milyon giriş tokenı başına $3, ~1 milyon çıkış tokenı başına $15
- Yerel Llama 3.1 8B: 1 milyon token başına $0 (sadece elektrik)

Binlerce istek işleyen hizmetler oluşturuyorsan, milyon token başına $0 ile $5-$15 arasındaki fark, kâr ile başabaş arasındaki farktır.

Ama işte çoğu kişinin kaçırdığı nüans: **yerel ve API modelleri bir gelir yığınında farklı roller üstlenir.** Yerel modeller hacmi halleder. API modelleri kalite açısından kritik, müşteriye yönelik çıktıyı halleder. Yığınının her ikisine de ihtiyacı var.

### Ollama Kurulumu

{? if settings.has_llm ?}
> **Zaten yapılandırılmış bir LLM'in var:** {= settings.llm_provider | fallback("Yerel") =} / {= settings.llm_model | fallback("bilinmeyen model") =}. Ollama zaten çalışıyorsa, aşağıdaki "Model Seçim Rehberi" bölümüne atla.
{? endif ?}

Ollama temeldir. Makineni temiz bir API'ye sahip yerel bir çıkarım sunucusuna dönüştürür.

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
> **Windows:** ollama.com'dan yükleyiciyi kullan veya `winget install Ollama.Ollama`. Ollama, kurulumdan sonra otomatik olarak arka plan hizmeti olarak çalışır.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` en hızlı yoldur. Ollama, Apple Silicon'un birleşik belleğinden yararlanır — {= profile.ram.total | fallback("sistem") =} RAM'in CPU ve GPU iş yükleri arasında paylaşılır.
{? elif computed.os_family == "linux" ?}
> **Linux:** Kurulum betiği her şeyi halleder. {= profile.os.name | fallback("Linux") =} çalıştırıyorsan, Ollama bir systemd hizmeti olarak kurulur.
{? endif ?}

Kurulumu doğrula:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **Sürüm notu:** Ollama sık sık güncelleme yayınlar. Bu modüldeki model komutları ve bayrakları Ollama v0.5.x (2026 başı) ile doğrulanmıştır. Bunu daha sonra okuyorsan, en son sürüm için [ollama.com/download](https://ollama.com/download) ve güncel model adları için [ollama.com/library](https://ollama.com/library) adreslerini kontrol et. Temel kavramlar değişmez, ancak belirli model etiketleri (ör. `llama3.1:8b`) daha yeni sürümlerle değişmiş olabilir.

### Model Seçim Rehberi

Gördüğün her modeli indirme. Stratejik ol. İşte ne indirmen ve her birini ne zaman kullanman gerektiği.

{? if computed.llm_tier ?}
> **LLM seviyen (donanıma göre):** {= computed.llm_tier | fallback("bilinmiyor") =}. Aşağıdaki öneriler etiketlenmiştir, böylece donanımına uyan seviyeye odaklanabilirsin.
{? endif ?}

#### Seviye 1: İş Beygiri (7B-8B modeller)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**Kullanım alanları:**
- Metin sınıflandırma ("Bu e-posta spam mı yoksa meşru mu?")
- Özetleme (uzun belgeleri madde işaretlerine dönüştür)
- Basit veri çıkarma (metinden isimler, tarihler, tutarlar çek)
- Duygu analizi
- İçerik etiketleme ve kategorilendirme
- Gömme vektörü üretimi (gömme desteği olan bir model kullanıyorsan)

**Performans (tipik):**
- RTX 3060 12GB: ~40-60 token/saniye
- RTX 4090: ~100-130 token/saniye
- M2 Pro 16GB: ~30-45 token/saniye
- Yalnızca CPU (Ryzen 7 5800X): ~8-12 token/saniye

**Maliyet karşılaştırması:**
- GPT-4o-mini ile 1 milyon token: ~$0.60
- Yerel olarak 1 milyon token (8B model): ~$0.003 elektrik
- Başabaş noktası: ~5,000 token (kelimenin tam anlamıyla ilk istekten itibaren tasarruf edersin)

#### Seviye 2: Dengeli Seçim (13B-14B modeller)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**Kullanım alanları:**
- İçerik taslağı hazırlama (blog yazıları, dokümantasyon, pazarlama metni)
- Kod üretimi (fonksiyonlar, betikler, şablonlar)
- Karmaşık veri dönüşümü
- Çok adımlı muhakeme görevleri
- Nüanslı çeviri

**Performans (tipik):**
- RTX 3060 12GB: ~20-30 token/saniye (nicelenmiş)
- RTX 4090: ~60-80 token/saniye
- M2 Pro 32GB: ~20-30 token/saniye
- Yalnızca CPU: ~3-6 token/saniye (gerçek zamanlı için pratik değil)

**7B yerine ne zaman kullanılmalı:** 7B'nin çıktı kalitesi yeterli olmadığında ama API çağrıları için ödeme yapmana gerek olmadığında. İkisini de gerçek kullanım senaryonda test et — bazen 7B yeterlidir ve sadece işlem gücü boşa harcıyorsundur.

{? if computed.gpu_tier == "capable" ?}
> **Seviye 3 uzanma bölgesi** — {= profile.gpu.model | fallback("GPU") =}'n biraz çabayla 30B nicelenmiş modeli kaldırabilir, ama 70B yerel olarak erişim dışında. 70B düzeyinde kalite gerektiren görevler için API çağrılarını değerlendir.
{? endif ?}

#### Seviye 3: Kalite Seviyesi (30B-70B modeller)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**Kullanım alanları:**
- Mükemmel olması gereken müşteriye yönelik içerik
- Karmaşık analiz ve muhakeme
- Uzun form içerik üretimi
- Kalitenin birinin sana para ödeyip ödemeyeceğini doğrudan etkilediği görevler

**Performans (tipik):**
- RTX 4090 (24GB): 70B ~8-15 token/saniye (kullanılabilir ama yavaş)
- Çift GPU veya 48GB+: 70B ~20-30 token/saniye
- M3 Max 64GB: 70B ~10-15 token/saniye

> **Açık Konuşalım:** 24GB+ VRAM'ın yoksa, 70B modelleri tamamen atla. Kalite açısından kritik çıktı için API çağrıları kullan. Sistem RAM'inden 3 token/saniye hızında çalışan bir 70B model teknik olarak mümkündür ama herhangi bir gelir üreten iş akışı için pratik olarak işe yaramaz. Zamanının bir değeri var.

#### Seviye 4: API Modelleri (Yerel Yetmediğinde)

Yerel modeller hacim ve gizlilik içindir. API modelleri kalite tavanları ve özelleşmiş yetenekler içindir.

**API modelleri ne zaman kullanılmalı:**
- Kalite = gelir olan müşteriye yönelik çıktı (satış metni, premium içerik)
- Küçük modellerin beceremedikleri karmaşık muhakeme zincirleri
- Görsel/çok modlu görevler (görüntüleri, ekran görüntülerini, belgeleri analiz etme)
- Yüksek güvenilirlikle yapılandırılmış JSON çıktısına ihtiyaç duyduğunda
- Hız önemli olduğunda ve yerel donanımın yavaş olduğunda

**Maliyet karşılaştırma tablosu (2025 başı itibarıyla — güncel fiyatlandırmayı kontrol et):**

| Model | Giriş (1M token başına) | Çıkış (1M token başına) | En İyi Kullanım |
|-------|----------------------|------------------------|----------|
| GPT-4o-mini | $0.15 | $0.60 | Ucuz hacim işi (yerel mevcut olmadığında) |
| GPT-4o | $2.50 | $10.00 | Görsel, karmaşık muhakeme |
| Claude 3.5 Sonnet | $3.00 | $15.00 | Kod, analiz, uzun bağlam |
| Claude 3.5 Haiku | $0.80 | $4.00 | Hızlı, ucuz, iyi kalite dengesi |
| DeepSeek V3 | $0.27 | $1.10 | Bütçe dostu, güçlü performans |

**Hibrit strateji:**
1. Yerel 7B/13B isteklerin %80'ini halleder (sınıflandırma, çıkarma, özetleme)
2. API isteklerin %20'sini halleder (kalite açısından kritik üretim, karmaşık görevler)
3. Etkin maliyetin: karışık olarak ~1M token başına $0.50-2.00 (saf API kullanımındaki $5-15 yerine)

Bu hibrit yaklaşım, sağlıklı kâr marjlarıyla hizmet oluşturmanın yolu. Modül R'de bununla ilgili daha fazlası var.

### Üretim Yapılandırması

Ollama'yı gelir işi için çalıştırmak, kişisel sohbet için çalıştırmaktan farklıdır. İşte doğru şekilde nasıl yapılandıracağın.

{? if computed.has_nvidia ?}
> **NVIDIA GPU algılandı ({= profile.gpu.model | fallback("bilinmiyor") =}).** Ollama otomatik olarak CUDA hızlandırmasını kullanacak. NVIDIA sürücülerinin güncel olduğundan emin ol — kontrol etmek için `nvidia-smi` çalıştır. {= profile.gpu.vram | fallback("senin") =} VRAM ile en iyi performans için, aşağıdaki `OLLAMA_MAX_LOADED_MODELS` ayarı VRAM'ına aynı anda kaç modelin sığacağıyla eşleşmeli.
{? endif ?}

#### Ortam Değişkenlerini Ayarla

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

#### İş Yüküne Göre Bir Modelfile Oluştur

Varsayılan model ayarlarını kullanmak yerine, gelir iş yüküne göre ayarlanmış özel bir Modelfile oluştur:

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

#### Toplu İşleme ve Kuyruk Yönetimi

Gelir iş yüklerinde genellikle birçok öğeyi işlemen gerekecek. İşte temel bir toplu işleme kurulumu:

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

### Kendi Donanımında Kıyaslama

Başkasının kıyaslamalarına güvenme. Kendininkini ölç:

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

Her model için token/saniye değerini yaz. Bu sayı, donanımın için hangi gelir iş akışlarının pratik olduğunu belirler.

{@ insight stack_fit @}

**Kullanım senaryosuna göre hız gereksinimleri:**
- Toplu işleme (asenkron): 5+ token/sn yeterli (gecikmeyi umursamazsın)
- Etkileşimli araçlar (kullanıcı bekler): Minimum 20+ token/sn
- Gerçek zamanlı API (müşteriye yönelik): İyi kullanıcı deneyimi için 30+ token/sn
- Akışlı sohbet: 15+ token/sn duyarlı hissettirir

### Yerel Çıkarım Sunucunu Güvenli Hale Getirme

{? if computed.os_family == "windows" ?}
> **Windows notu:** Windows'ta Ollama varsayılan olarak localhost'a bağlanır. PowerShell'de `netstat -an | findstr 11434` ile doğrula. 11434 portuna harici erişimi engellemek için Windows Güvenlik Duvarı'nı kullan.
{? elif computed.os_family == "macos" ?}
> **macOS notu:** macOS'ta Ollama varsayılan olarak localhost'a bağlanır. `lsof -i :11434` ile doğrula. macOS güvenlik duvarı harici bağlantıları otomatik olarak engellemelidir.
{? endif ?}

Ollama sunucun, açıkça amaçlamadığın sürece asla internetten erişilebilir olmamalı.

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

> **Yaygın Hata:** Ollama'yı "kolaylık" için 0.0.0.0'a bağlamak ve sonra unutmak. IP adresini bulan herkes GPU'nu ücretsiz çıkarım için kullanabilir. Daha kötüsü, model ağırlıklarını ve sistem istemlerini çıkarabilirler. Her zaman localhost. Her zaman tünelleme.

### Ders 2 Kontrol Noktası

Şimdi şunlara sahip olmalısın:
- [ ] Ollama kurulu ve çalışıyor
- [ ] En az bir iş beygiri model indirilmiş (llama3.1:8b veya eşdeğeri)
- [ ] Beklenen iş yüküne göre özel bir Modelfile
- [ ] Kıyaslama sayıları: donanımındaki her model için token/saniye
- [ ] Ollama yalnızca localhost'a bağlı

*Tam STREETS kursunda, Modül T (Teknik Hendekler), rakiplerin kolayca kopyalayamayacağı özel model yapılandırmaları, ince ayar hattı ve özel araç zincirleri nasıl oluşturulacağını gösterir. Modül R (Gelir Motorları), bu yığının üzerine inşa edeceğin tam hizmetleri verir.*

---

## Ders 3: Gizlilik Avantajı

*"Özel kurulumun bir rekabet avantajı — sadece bir tercih değil."*

### Gizlilik Bir Ürün Özelliğidir, Sınırlama Değil

Çoğu geliştirici, kişisel olarak gizliliğe değer verdiği veya kurcalamayı sevdiği için yerel altyapı kurar. Bu sorun değil. Ama **gizliliğin şu an teknolojide en pazarlanabilir özelliklerden biri olduğunun** farkına varmazsan masada para bırakıyorsun.

Sebebi şu: bir şirket OpenAI API'sine her veri gönderdiğinde, o veri bir üçüncü taraftan geçer. Birçok işletme için — özellikle sağlık, finans, hukuk, devlet ve AB merkezli şirketler — bu gerçek bir sorundur. Teorik değil. "Uyumluluk ekibi hayır dediği için bu aracı kullanamıyoruz" düzeyinde bir sorun.

Sen, kendi makinende yerel olarak modeller çalıştıran biri olarak, bu soruna sahip değilsin.

### Düzenleyici Rüzgâr

Düzenleyici ortam senin yönüne doğru ilerliyor. Hızla.

{? if regional.country == "US" ?}
> **ABD merkezli:** Senin için en önemli düzenlemeler HIPAA, SOC 2, ITAR ve eyalet düzeyindeki gizlilik yasalarıdır (Kaliforniya CCPA, vb.). AB düzenlemeleri hâlâ önemli — kârlı bir pazar olan Avrupa müşterilerine hizmet verme yeteneğini etkiler.
{? elif regional.country == "GB" ?}
> **İngiltere merkezli:** Brexit sonrası, İngiltere'nin kendi veri koruma çerçevesi var (UK GDPR + Veri Koruma Kanunu 2018). Yerel işleme avantajın özellikle İngiltere finansal hizmetleri ve NHS ilişkili çalışmalar için güçlüdür.
{? elif regional.country == "DE" ?}
> **Almanya merkezli:** Dünyanın en katı veri koruma ortamlarından birindeyken. Bu bir *avantaj* — Alman müşteriler yerel işlemenin neden önemli olduğunu zaten anlıyor ve bunun için ödeme yapacaklar.
{? elif regional.country == "AU" ?}
> **Avustralya merkezli:** Gizlilik Kanunu 1988 ve Avustralya Gizlilik İlkeleri (APP'ler) yükümlülüklerini yönetir. Yerel işleme, My Health Records Kanunu kapsamındaki devlet ve sağlık müşterileri için güçlü bir satış noktasıdır.
{? endif ?}

**AB AI Yasası (2024-2026'dan itibaren yürürlükte):**
- Yüksek riskli AI sistemleri belgelenmiş veri işleme hatları gerektirir
- Şirketler verinin nereye aktığını ve kimin işlediğini göstermek zorundadır
- Yerel işleme uyumluluğu dramatik şekilde basitleştirir
- AB şirketleri AB veri ikametini garanti edebilen AI hizmet sağlayıcıları aktif olarak arıyor

**GDPR (halihazırda yürürlükte):**
- "Veri işleme" bir LLM API'sine metin göndermeyi içerir
- Şirketler her üçüncü tarafla Veri İşleme Sözleşmeleri yapmalıdır
- Yerel işleme üçüncü tarafı tamamen ortadan kaldırır
- Bu gerçek bir satış noktasıdır: "Veriniz altyapınızdan asla ayrılmaz. Müzakere edilecek üçüncü taraf DPA'sı yoktur."

**Sektöre özel düzenlemeler:**
- **HIPAA (ABD Sağlık):** Hasta verileri, bir BAA (İş Ortağı Sözleşmesi) olmadan tüketici AI API'lerine gönderilemez. Çoğu AI sağlayıcı API erişimi için BAA sunmaz. Yerel işleme bunu tamamen atlar.
- **SOC 2 (Kurumsal):** SOC 2 denetiminden geçen şirketlerin her veri işleyiciyi belgelemesi gerekir. Daha az işleyici = daha kolay denetim.
- **ITAR (ABD Savunma):** Kontrollü teknik veriler ABD yetki alanından çıkamaz. Uluslararası altyapıya sahip bulut AI sağlayıcıları sorunludur.
- **PCI DSS (Finans):** Kart sahibi veri işleme, verinin nereye gittiği konusunda katı gereksinimler içerir.

### Gizliliği Satış Konuşmalarında Nasıl Konumlandırırsın

Uyumluluk uzmanı olman gerekmiyor. Üç cümle bilmen ve ne zaman kullanacağını bilmen gerekiyor:

**Cümle 1: "Veriniz altyapınızdan asla ayrılmaz."**
Ne zaman kullanılır: Gizlilik bilincine sahip herhangi bir potansiyel müşteriyle konuşurken. Bu evrensel kancadır.

**Cümle 2: "Üçüncü taraf veri işleme sözleşmesi gerekmez."**
Ne zaman kullanılır: Avrupa şirketleriyle veya hukuk/uyumluluk ekibi olan herhangi bir şirketle konuşurken. Bu onlara haftalarca hukuki inceleme süresinden tasarruf ettirir.

**Cümle 3: "Tam denetim izi, tek kiracılı işleme."**
Ne zaman kullanılır: Kurumsal veya düzenlenmiş sektörlerle konuşurken. AI hattını denetçilere kanıtlamaları gerekiyor.

**Örnek konumlandırma (hizmet sayfan veya tekliflerin için):**

> "Bulut tabanlı AI hizmetlerinin aksine, [Hizmetin] tüm verileri özel donanım üzerinde yerel olarak işler. Belgeleriniz, kodlarınız ve verileriniz işleme ortamından asla ayrılmaz. Hatta üçüncü taraf API yoktur, müzakere edilecek veri paylaşım sözleşmesi yoktur ve her işlemin tam denetim kaydı tutulur. Bu, [Hizmetinizi] GDPR, HIPAA ve SOC 2 uyumluluk ortamları dahil olmak üzere katı veri işleme gereksinimleri olan kuruluşlar için uygun hale getirir."

Bu paragraf, bir açılış sayfasında, tam olarak premium fiyat ödeyecek müşterileri çekecektir.

### Premium Fiyatlandırma Gerekçesi

İşte somut rakamlarla iş gerekçesi:

**Standart AI işleme hizmeti (bulut API'leri kullanarak):**
- Müşterinin verileri OpenAI/Anthropic/Google'a gider
- API çağrısı yapabilen her geliştiriciyle rekabet ediyorsun
- Piyasa fiyatı: İşlenen doküman başına $0.01-0.05
- Esasen API erişimini kâr marjıyla yeniden satıyorsun

**Gizlilik öncelikli AI işleme hizmeti (senin yerel yığının):**
- Müşterinin verileri senin makinen üzerinde kalır
- Çok daha küçük bir sağlayıcı havuzuyla rekabet ediyorsun
- Piyasa fiyatı: İşlenen doküman başına $0.10-0.50 (5-10x premium)
- Altyapı + uzmanlık + uyumluluk satıyorsun

Gizlilik primi gerçek: aynı temel görev için standart bulut tabanlı hizmetlere kıyasla **5x ile 10x**. Ve bunu ödeyen müşteriler daha sadık, fiyata daha az duyarlı ve daha büyük bütçelere sahip.

{@ insight competitive_position @}

### İzole Çalışma Alanları Kurma

Bir günlük işin varsa (çoğunuzun var), işveren işi ile gelir işi arasında temiz ayrım gerekir. Bu sadece yasal koruma değil — operasyonel hijyen.

{? if computed.os_family == "windows" ?}
> **Windows ipucu:** Gelir işi için ayrı bir Windows kullanıcı hesabı oluştur (Ayarlar > Hesaplar > Aile ve diğer kullanıcılar > Başka birini ekle). Bu sana tamamen izole bir ortam verir — ayrı tarayıcı profilleri, ayrı dosya yolları, ayrı ortam değişkenleri. Win+L ile hesaplar arasında geçiş yap.
{? endif ?}

**Seçenek 1: Ayrı kullanıcı hesapları (önerilen)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**Seçenek 2: Konteynerleştirilmiş çalışma alanları**

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

**Seçenek 3: Ayrı fiziksel makine (en sağlam)**

Bu konuda ciddiysen ve gelirin haklı çıkarıyorsa, özel bir makine tüm soruları ortadan kaldırır. RTX 3060'lı kullanılmış bir Dell OptiPlex $400-600'a mal olur ve ilk ay müşteri çalışmasında kendini amorti eder.

**Minimum ayrım kontrol listesi:**
- [ ] Gelir projeleri ayrı bir dizinde (asla işveren depolarıyla karışmaz)
- [ ] Gelir işi için ayrı API anahtarları (asla işverenin sağladığı anahtarları kullanma)
- [ ] Gelirle ilgili hesaplar için ayrı tarayıcı profili
- [ ] Gelir işi asla işveren donanımı üzerinde yapılmaz
- [ ] Gelir işi asla işveren ağında yapılmaz (kişisel internetini veya VPN kullan)
- [ ] Gelir projeleri için ayrı GitHub/GitLab hesabı (isteğe bağlı ama temiz)

> **Yaygın Hata:** Yan projen için işvereninin OpenAI API anahtarını "sadece test için" kullanmak. Bu, işvereninin fatura panosunun görebileceği bir kayıt oluşturur ve fikri mülkiyet durumunu bulanıklaştırır. Kendi anahtarlarını al. Ucuz.

### Ders 3 Kontrol Noktası

Şimdi şunları anlamış olmalısın:
- [ ] Gizlilik neden sadece kişisel bir tercih değil, pazarlanabilir bir ürün özelliğidir
- [ ] Hangi düzenlemeler yerel AI işleme talebi yaratır
- [ ] Gizlilik hakkındaki satış konuşmalarında kullanılacak üç cümle
- [ ] Gizlilik öncelikli hizmetler nasıl 5-10x premium fiyatlandırma sağlar
- [ ] Gelir işini işveren işinden nasıl ayırırsın

*Tam STREETS kursunda, Modül E (Gelişen Kenar), düzenleyici değişiklikleri nasıl takip edeceğini ve rakiplerin henüz var olduklarını bilmeden yeni uyumluluk gereksinimlerinin önünde nasıl konumlanacağını öğretir.*

---

## Ders 4: Yasal Asgari

*"Şimdi on beş dakikalık yasal kurulum, sonraki aylardaki sorunları önler."*

### Bu Hukuki Tavsiye Değil

Ben bir geliştiriciyim, avukat değil. Aşağıdaki, çoğu geliştirinin çoğu durumda ele alması gereken pratik bir kontrol listesidir. Durumun karmaşıksa (işvereninde hisse, belirli koşulları olan rekabet yasağı, vb.), bir iş hukuku avukatıyla 30 dakikalık danışmanlık için $200 harca. Alacağın en iyi yatırım getirisi bu.

### Adım 1: İş Sözleşmeni Oku

İş sözleşmeni veya teklif mektubunu bul. Şu bölümleri ara:

**Fikri Mülkiyet Devir maddesi** — Şöyle bir dil ara:
- "Tüm buluşlar, geliştirmeler ve iş ürünleri..."
- "...istihdam süresi boyunca oluşturulan..."
- "...Şirketin işi veya öngörülen işiyle ilgili..."

**Seni kısıtlayan anahtar ifadeler:**
- "İstihdam süresince oluşturulan tüm iş ürünleri Şirkete aittir" (geniş — potansiyel olarak sorunlu)
- "Şirket kaynakları kullanılarak oluşturulan iş ürünleri" (daha dar — kendi ekipmanını kullanıyorsan genellikle sorun yok)
- "Şirketin mevcut veya öngörülen işiyle ilgili" (işvereninin ne yaptığına bağlı)

**Seni özgür bırakan anahtar ifadeler:**
- "Tamamen Çalışanın kendi zamanında, kendi kaynakları ile ve Şirket işiyle ilgisi olmayan işler hariç" (bu senin muafiyet madden — birçok ABD eyaleti bunu gerektirir)
- Bazı eyaletler (California, Washington, Minnesota, Illinois ve diğerleri) sözleşme ne derse desin, işverenlerin kişisel projeler üzerindeki fikri mülkiyet taleplerini sınırlayan yasalara sahiptir.

### 3 Soru Testi

Her gelir projesi için sor:

1. **Zaman:** Bu işi kendi zamanında mı yapıyorsun? (Çalışma saatlerinde değil, nöbet vardiyalarında değil)
2. **Ekipman:** Kendi donanımını, kendi internetini, kendi API anahtarlarını mı kullanıyorsun? (İşveren dizüstü bilgisayarı değil, işveren VPN'i değil, işveren bulut hesapları değil)
3. **Konu:** Bu işvereninin işiyle ilgisiz mi? (Bir sağlık AI şirketinde çalışıyorsan ve sağlık AI hizmetleri satmak istiyorsan... bu bir sorun. Bir sağlık AI şirketinde çalışıyorsan ve emlakçılar için belge işleme hizmeti satmak istiyorsan... bu sorun değil.)

Üç cevabın da temizse, neredeyse kesinlikle sorun yok. Herhangi bir cevap bulanıksa, devam etmeden önce netlik kazan.

> **Açık Konuşalım:** Yan iş yapan geliştiricilerin büyük çoğunluğu asla bir sorun yaşamaz. İşverenler rekabet avantajlarını korumayı önemser, ilgisiz projelerde ekstra para kazanmanı engellemeyi değil. Ama "neredeyse kesinlikle sorun yok" ile "kesinlikle sorun yok" aynı şey değil. Sözleşmen olağandışı genişse, yöneticinle veya İK ile bir konuşma yap — veya bir avukata danış. Kontrol etmemenin dezavantajı, sormanın hafif rahatsızlığından çok daha kötü.

### Adım 2: Bir İşletme Yapısı Seç

Kişisel varlıklarını iş faaliyetlerinden ayırmak ve işletme bankacılığı, ödeme işleme ve vergi avantajlarının kapısını açmak için yasal bir tüzel kişilik gerekir.

{? if regional.country ?}
> **Konumun: {= regional.country | fallback("Bilinmiyor") =}.** Bölgen için önerilen tüzel kişilik türü **{= regional.business_entity_type | fallback("LLC veya eşdeğeri") =}**, tipik kayıt maliyetleri {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Aşağıdaki ülke bölümüne git veya diğer bölgelerdeki müşterilerin nasıl çalıştığını anlamak için tüm bölümleri oku.
{? endif ?}

{? if regional.country == "US" ?}
#### Amerika Birleşik Devletleri (Senin Bölgen)
{? else ?}
#### Amerika Birleşik Devletleri
{? endif ?}

| Yapı | Maliyet | Koruma | En İyi Kullanım |
|-----------|------|------------|----------|
| **Şahıs Şirketi** (varsayılan) | $0 | Yok (kişisel sorumluluk) | Deneme aşaması. İlk $1K. |
| **Tek Üyeli LLC** | $50-500 (eyalete göre değişir) | Kişisel varlık koruması | Aktif gelir işi. Çoğu geliştirici buradan başlamalı. |
| **S-Corp seçimi** (LLC üzerine) | LLC maliyeti + seçim için $0 | LLC ile aynı + bordro vergisi avantajları | Bundan tutarlı olarak yılda $40K+ kazandığında |

**ABD geliştiricileri için önerilen:** İkamet ettiğin eyalette Tek Üyeli LLC.

**En ucuz kuruluş eyaletleri:** Wyoming ($100, eyalet gelir vergisi yok), New Mexico ($50), Montana ($70). Ama belirli bir nedenin yoksa, kendi eyaletinde kurmak genellikle en basit olanıdır.

**Nasıl başvurulur:**
1. Eyaletinin Sekreterlik (Secretary of State) web sitesine git
2. "form LLC" veya "business entity filing" ara
3. Kuruluş Sözleşmesini (Articles of Organization) doldur (10 dakikalık form)
4. IRS'den bir EIN al (ücretsiz, irs.gov'da 5 dakika sürer)

{? if regional.country == "GB" ?}
#### Birleşik Krallık (Senin Bölgen)
{? else ?}
#### Birleşik Krallık
{? endif ?}

| Yapı | Maliyet | Koruma | En İyi Kullanım |
|-----------|------|------------|----------|
| **Şahıs Şirketi (Sole Trader)** | Ücretsiz (HMRC'ye kayıt) | Yok | İlk gelir. Deneme. |
| **Limited Şirket (Ltd)** | ~$15, Companies House üzerinden | Kişisel varlık koruması | Ciddi herhangi bir gelir işi. |

**Önerilen:** Companies House üzerinden Ltd şirket. Yaklaşık 20 dakika sürer ve 12 GBP'ye mal olur.

#### Avrupa Birliği

Ülkeye göre önemli ölçüde değişir, ama genel kalıp:

- **Almanya:** Einzelunternehmer (şahıs şirketi) başlangıç için, ciddi iş için GmbH (ama GmbH 25,000 EUR sermaye gerektirir — 1 EUR'ya UG düşünülebilir)
- **Hollanda:** Eenmanszaak (şahıs şirketi, kayıt ücretsiz) veya BV (Ltd'ye benzer)
- **Fransa:** Micro-entrepreneur (basitleştirilmiş, başlamak için önerilen)
- **Estonya:** e-Residency + OUE (yerleşik olmayanlar için popüler, tamamen çevrimiçi)

{? if regional.country == "AU" ?}
#### Avustralya (Senin Bölgen)
{? else ?}
#### Avustralya
{? endif ?}

| Yapı | Maliyet | Koruma | En İyi Kullanım |
|-----------|------|------------|----------|
| **Şahıs Şirketi (Sole Trader)** | Ücretsiz ABN | Yok | Başlangıç |
| **Pty Ltd** | ~800-1200 AUD, ASIC üzerinden | Kişisel varlık koruması | Ciddi gelir |

**Önerilen:** Şahıs Şirketi ABN ile başla (ücretsiz, anında), tutarlı kazanç elde ettiğinde Pty Ltd'ye geç.

### Adım 3: Ödeme İşleme (15 dakikalık kurulum)

Ödeme alabilmenin bir yoluna ihtiyacın var. Bunu ilk müşterin beklerken değil, şimdi kur.

{? if regional.payment_processors ?}
> **{= regional.country | fallback("bölgen") =} için önerilen:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (çoğu geliştirici için önerilen):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

Süre: ~15 dakika. Hemen ödeme almaya başlayabilirsin (Stripe yeni hesaplarda fonları 7 gün tutar).

**Lemon Squeezy (dijital ürünler için önerilen):**

Dijital ürünler satıyorsan (şablonlar, araçlar, kurslar, SaaS), Lemon Squeezy senin Kayıtlı Satıcın (Merchant of Record) olarak hareket eder. Bu şu anlama gelir:
- Küresel olarak satış vergisini, KDV'yi ve GST'yi senin için halleder
- AB'de KDV kaydı yaptırmana gerek kalmaz
- İadeleri ve anlaşmazlıkları halleder

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (uluslararası geliştiriciler veya ABD tüzel kişiliği isteyenler için):**

ABD dışındaysan ama ABD tüzel kişiliğiyle ABD müşterilerine satmak istiyorsan:
- $500 tek seferlik ücret
- Senin için Delaware LLC oluşturur
- ABD banka hesabı açar (Mercury veya Stripe aracılığıyla)
- Kayıtlı temsilci hizmeti sağlar
- Yaklaşık 1-2 hafta sürer

### Adım 4: Gizlilik Politikası ve Hizmet Şartları

Çevrimiçi herhangi bir hizmet veya ürün satıyorsan, bunlara ihtiyacın var. Şablon belgeler için avukata para ödeme.

**Şablonlar için ücretsiz, güvenilir kaynaklar:**
- **Termly.io** — Ücretsiz gizlilik politikası ve hizmet şartları oluşturucu. Soruları yanıtla, belgeleri al.
- **Avodocs.com** — Girişimler için açık kaynaklı yasal belgeler. Ücretsiz.
- **GitHub'ın choosealicense.com** — Özellikle açık kaynak proje lisansları için.
- **Basecamp'in açık kaynaklı politikaları** — "Basecamp open source policies" ara — iyi, sade dilde şablonlar.

**Gizlilik politikanın neyi kapsaması gerekir (herhangi bir müşteri verisi işliyorsan):**
- Hangi verileri topladığın
- Nasıl işlediğin (yerel olarak — bu senin avantajın)
- Ne kadar süre sakladığın
- Müşteriler silme isteği nasıl yapabilir
- Herhangi bir üçüncü tarafın verilere erişip erişmediği (ideal olarak: hiçbiri)

**Süre:** Bir şablon oluşturucu ile 30 dakika. Tamam.

### Adım 5: Ayrı Banka Hesabı

İşletme gelirini kişisel hesabın üzerinden geçirme. Nedenleri:

1. **Vergi netliği:** Vergi zamanı geldiğinde, neyin işletme geliri olup neyin olmadığını kesin olarak bilmen gerekir.
2. **Yasal koruma:** LLC'n varsa, kişisel ve işletme fonlarını karıştırmak "kurumsal perdeyi delme"ye yol açabilir — yani mahkeme LLC'nin sorumluluk korumasını görmezden gelebilir.
3. **Profesyonellik:** "Ahmet'in Danışmanlık LLC"sinden özel bir işletme hesabına gelen faturalar meşru görünür. Kişisel hesabına gelen ödemeler ise görünmez.

**Ücretsiz veya düşük maliyetli işletme bankacılığı:**
{? if regional.country == "US" ?}
- **Mercury** (senin için önerilen) — Ücretsiz, girişimler için tasarlanmış. Daha sonra muhasebe otomasyonu istersen mükemmel API.
- **Relay** — Ücretsiz, gelir akışlarını alt hesaplara ayırmak için iyi.
{? elif regional.country == "GB" ?}
- **Starling Bank** (senin için önerilen) — Ücretsiz işletme hesabı, anında kurulum.
- **Wise Business** — Düşük maliyetli çok para birimli. Uluslararası müşterilere hizmet veriyorsan harika.
{? else ?}
- **Mercury** (ABD) — Ücretsiz, girişimler için tasarlanmış. Daha sonra muhasebe otomasyonu istersen mükemmel API.
- **Relay** (ABD) — Ücretsiz, gelir akışlarını alt hesaplara ayırmak için iyi.
- **Starling Bank** (İngiltere) — Ücretsiz işletme hesabı.
{? endif ?}
- **Wise Business** (Uluslararası) — Düşük maliyetli çok para birimli. USD, EUR, GBP, vb. cinsinden ödeme almak için harika.
- **Qonto** (AB) — Avrupa şirketleri için temiz işletme bankacılığı.

Hesabı şimdi aç. Çevrimiçi 10-15 dakika sürer ve doğrulama için 1-3 gün.

### Adım 6: Geliştirici Yan Gelir İçin Vergi Temelleri

{? if regional.tax_note ?}
> **{= regional.country | fallback("bölgen") =} için vergi notu:** {= regional.tax_note | fallback("Ayrıntılar için yerel bir vergi uzmanına danış.") =}
{? endif ?}

> **Açık Konuşalım:** Vergiler, çoğu geliştiricinin Nisan'a kadar görmezden gelip sonra panikle karşılaştığı şeydir. Şimdi 30 dakika harcamak sana gerçek para ve stres tasarrufu sağlar.

**Amerika Birleşik Devletleri:**
- Yılda $400'ın üzerindeki yan gelir, serbest meslek vergisi gerektirir (~%15.3 Sosyal Güvenlik + Medicare için)
- Artı net kâr üzerinden normal gelir vergisi diliminiz
- **Üç aylık tahmini vergiler:** $1,000'dan fazla vergi borcu olacaksa, IRS üç aylık ödemeler bekler (15 Nisan, 15 Haziran, 15 Eylül, 15 Ocak). Eksik ödeme cezalara yol açar.
- Net gelirin **%25-30'unu** vergiler için ayır. Hemen ayrı bir tasarruf hesabına koy.

**Geliştirici yan gelir için yaygın vergi indirimleri:**
- API maliyetleri (OpenAI, Anthropic, vb.) — %100 indirilebilir
- İş için kullanılan donanım alımları — amortismanlı veya Section 179 indirimi
- İş kullanımına atfedilebilir elektrik maliyeti
- Gelir işi için kullanılan yazılım abonelikleri
- Ev ofis indirimi (basitleştirilmiş: metrekare başına $5, 300 sq ft'e kadar = $1,500)
- İnternet (iş kullanım yüzdesi)
- Alan adları, barındırma, e-posta hizmetleri
- Gelir işinle ilgili mesleki gelişim (kurslar, kitaplar)

**Birleşik Krallık:**
- Self Assessment vergi beyannamesi ile bildir
- 1,000 GBP altındaki ticaret geliri: vergisiz (Ticaret İndirimi)
- Bunun üstünde: kârlar üzerinden Gelir Vergisi + Class 4 NIC'ler öde
- Ödeme tarihleri: 31 Ocak ve 31 Temmuz

**Her şeyi birinci günden takip et.** Başka bir şey yoksa basit bir elektronik tablo kullan:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **Yaygın Hata:** "Vergileri sonra halledeyim." Sonra 4. çeyrek oluyor, tahmini vergilerde $3,000 artı cezalar borçlusun ve parayı harcamış oluyorsun. Otomatikleştir: işletme hesabına her gelir geldiğinde, hemen %30'unu vergi tasarruf hesabına aktar.

### Ders 4 Kontrol Noktası

Şimdi şunlara sahip olmalısın (veya plan yapmalısın):
- [ ] İş sözleşmenin fikri mülkiyet maddesini okudun
- [ ] Planlanan gelir işin için 3 Soru Testini geçtin
- [ ] Bir işletme yapısı seçtin (veya şahıs şirketi olarak başlamaya karar verdin)
- [ ] Ödeme işleme kuruldu (Stripe veya Lemon Squeezy)
- [ ] Bir şablon oluşturucudan gizlilik politikası ve hizmet şartları
- [ ] Ayrı işletme banka hesabı (veya başvuru yapıldı)
- [ ] Vergi stratejisi: %30 ayrım + üç aylık ödeme takvimi

*Tam STREETS kursunda, Modül E (Uygulama Taktik Kitabı), vergi yükümlülüklerini, proje kârlılığını ve her gelir motoru için başabaş noktalarını otomatik hesaplayan finansal modelleme şablonları içerir.*

---

## Ders 5: Aylık {= regional.currency_symbol | fallback("$") =}200 Bütçe

*"İşletmenin bir yanma oranı var. Bil. Kontrol et. Kazandır."*

### Neden {= regional.currency_symbol | fallback("$") =}200/ay

Ayda iki yüz {= regional.currency | fallback("dolar") =}, bir geliştirici gelir operasyonu için minimum uygulanabilir bütçedir. Gerçek hizmetler çalıştırmak, gerçek müşterilere hizmet vermek ve gerçek gelir üretmek için yeterli. Ayrıca hiçbir şey işe yaramazsa çiftliği kaybetmeyecek kadar küçük.

Hedef basit: **ayda {= regional.currency_symbol | fallback("$") =}200'ü 90 gün içinde aylık {= regional.currency_symbol | fallback("$") =}600+'ya çevir.** Bunu yapabilirsen, bir işletmen var. Yapamazsan, strateji değiştirirsin — bütçeyi artırmazsın.

### Bütçe Dağılımı

#### Seviye 1: API Kredileri — $50-100/ay

Bu, müşteriye yönelik kalite için üretim işlem gücün.

**Önerilen başlangıç tahsisi:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**API harcamasını nasıl yönetirsin:**

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

**Hibrit harcama stratejisi:**
- İşlemelerin %80'i için yerel LLM'leri kullan (sınıflandırma, çıkarma, özetleme, taslaklar)
- İşlemelerin %20'si için API çağrıları kullan (son kalite geçişi, karmaşık muhakeme, müşteriye yönelik çıktı)
- Görev başına etkin maliyetin, saf API kullanımına kıyasla dramatik şekilde düşer

{? if computed.monthly_electricity_estimate ?}
> **Tahmini elektrik maliyetin:** {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh'de 7/24 çalışma için aylık {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}. Bu zaten etkin işletme maliyetine dahil edilmiş.
{? endif ?}

#### Seviye 2: Altyapı — {= regional.currency_symbol | fallback("$") =}30-50/ay

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**VPS'e ihtiyacın var mı?**

Gelir modelin:
- **Dijital ürünler satıyorsan:** Hayır. Ücretsiz olarak Vercel/Netlify'da barındır. Teslimat için Lemon Squeezy kullan.
- **Müşteriler için asenkron işleme yapıyorsan:** Belki. İşleri yerel makinende çalıştırıp sonuçları teslim edebilirsin. VPS güvenilirlik ekler.
- **API hizmeti sunuyorsan:** Muhtemelen evet. $5-10'luk bir VPS, ağır işlem yerel makinende olsa bile hafif bir API geçidi görevi görür.
- **SaaS satıyorsan:** Evet. Ama en ucuz seviyeyle başla ve ölçekle.

**Önerilen başlangıç altyapısı:**

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

Toplam altyapı maliyeti: $5-20/ay. Geri kalan ücretsiz seviyeler.

#### Seviye 3: Araçlar — {= regional.currency_symbol | fallback("$") =}20-30/ay

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

> **Açık Konuşalım:** Başlarken tüm araç yığınını ücretsiz seviyelerde çalıştırabilirsin. Burada ayrılan $20-30, ücretsiz seviyeleri aştığında veya belirli bir premium özellik istediğinde içindir. Bütçede olduğu için harcama. Harcanmamış bütçe kârdır.

#### Seviye 4: Yedek — {= regional.currency_symbol | fallback("$") =}0-30/ay

Bu senin "öngörmediğim şeyler" fonun:
- Beklenmedik derecede büyük bir toplu işten kaynaklanan API maliyet artışı
- Belirli bir müşteri projesi için ihtiyaç duyduğun bir araç
- Mükemmel ismi bulduğunda acil alan adı satın alımı
- Tek seferlik bir alım (tema, şablon, simge seti)

Yedek fonunu kullanmazsan, birikir. 3 ay kullanılmayan yedek fondan sonra, API kredilerine veya altyapıya yeniden tahsis etmeyi düşün.

### Yatırım Getirisi Hesabı

Önemli olan tek sayı bu:

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

**Bütçeyi ne zaman artırmalısın:**

Bütçeni YALNIZCA şu durumlarda artır:
1. 2+ aydır tutarlı olarak 2x+ yatırım getirisi elde ediyorsun
2. Daha fazla harcama doğrudan geliri artıracak (ör. daha fazla API kredisi = daha fazla müşteri kapasitesi)
3. Artış belirli, test edilmiş bir gelir akışına bağlı

**Bütçeyi ne zaman artırMAMALISIN:**
- "Bu yeni araç yardımcı olacak diye düşünüyorum" (önce ücretsiz alternatifleri test et)
- "Herkes para kazanmak için para harcaman gerektiğini söylüyor" (bu aşamada değil)
- "Daha büyük VPS hizmetimi hızlandıracak" (hız gerçekten darboğaz mı?)
- Henüz 1x yatırım getirisi elde edemedin (geliri düzelt, harcamayı değil)

**Ölçekleme merdiveni:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **Yaygın Hata:** {= regional.currency_symbol | fallback("$") =}200'ü hemen para getirmesi gerekmeyen bir "yatırım" olarak görmek. Hayır. Bu, 90 günlük süresi olan bir deneydir. Ayda {= regional.currency_symbol | fallback("$") =}200, 90 gün içinde ayda {= regional.currency_symbol | fallback("$") =}200 gelir üretmiyorsa, stratejiyle ilgili bir şey değişmeli. Para, pazar, teklif — bir şey çalışmıyor. Kendinle dürüst ol.

### Ders 5 Kontrol Noktası

Şimdi şunlara sahip olmalısın:
- [ ] Dört seviyeye dağıtılmış ~$200 aylık bütçe
- [ ] Harcama limitleri belirlenmiş API hesapları oluşturulmuş
- [ ] Altyapı kararları verilmiş (yalnızca yerel vs. yerel + VPS)
- [ ] Araç yığını seçilmiş (başlangıçta çoğunlukla ücretsiz seviyeler)
- [ ] Yatırım getirisi hedefleri: 90 gün içinde 3x
- [ ] Net bir kural: bütçeyi yalnızca yatırım getirisini kanıtladıktan sonra artır

*Tam STREETS kursunda, Modül E (Uygulama Taktik Kitabı), harcamanı, gelirini ve her gelir motoru başına yatırım getirisini gerçek zamanlı takip eden bir finansal gösterge paneli şablonu içerir — böylece hangi akışların kârlı olduğunu ve hangilerinin ayarlama gerektirdiğini her zaman bilirsin.*

---

## Ders 6: Egemen Yığın Belgen

*"Her işletmenin bir planı var. Bu seninki — ve iki sayfaya sığıyor."*

### Teslim Edilecek Çıktı

Bu, Modül S'de oluşturacağın en önemli şey. Egemen Yığın Belgen, gelir üreten altyapın hakkındaki her şeyi yakalayan tek bir referans. STREETS kursunun geri kalanında buna başvuracaksın, kurulumun geliştikçe güncelleyeceksin ve neyi inşa edip neyi atlayacağın konusunda net kararlar vermek için kullanacaksın.

Yeni bir dosya oluştur. Markdown, Google Docs, Notion sayfası, düz metin — gerçekten güncel tutacağın ne varsa. Aşağıdaki şablonu kullan, her alanı Ders 1-5'teki sayılar ve kararlarla doldur.

### Şablon

{? if computed.profile_completeness != "0" ?}
> **Avantajlı başlangıç:** 4DA donanım özelliklerini ve yığın bilgilerini zaten algılamış. Aşağıdaki önceden doldurulmuş ipuçlarına bak — şablonu doldurmanda sana zaman kazandıracak.
{? endif ?}

Bu şablonun tamamını kopyala ve doldur. Her alanı. Atlama yok.

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
> **Geliştirici DNA'ndan ön doldurma:**
> - **Birincil yığın:** {= dna.primary_stack | fallback("Algılanmadı") =}
> - **İlgi alanları:** {= dna.interests | fallback("Algılanmadı") =}
> - **Kimlik özeti:** {= dna.identity_summary | fallback("Henüz profillenmedi") =}
{? if dna.blind_spots ?}> - **İzlenecek kör noktalar:** {= dna.blind_spots | fallback("Algılanmadı") =}
{? endif ?}
{? elif stack.primary ?}
> **Algılanan yığından ön doldurma:** Birincil teknolojilerin {= stack.primary | fallback("henüz algılanmadı") =}. {? if stack.adjacent ?}Yakın beceriler: {= stack.adjacent | fallback("algılanmadı") =}.{? endif ?} Yukarıdaki Beceri Envanterini doldurmak için bunları kullan.
{? endif ?}

{@ insight t_shape @}

### Bu Belgeyi Nasıl Kullanırsın

1. **Yeni bir projeye başlamadan önce:** Egemen Yığınını kontrol et. Projeyi yürütecek donanım, zaman, beceri ve bütçen var mı?
2. **Bir şey satın almadan önce:** Bütçe tahsisini kontrol et. Bu alım planda var mı?
3. **Aylık inceleme:** Bütçendeki "Gerçek" sütununu güncelle. Gelir rakamlarını güncelle. İşe yarayan şeylere göre tahsisleri ayarla.
4. **Biri ne yaptığını sorduğunda:** "Bugün Sunabileceklerim" bölümün anında sunumundur.
5. **Parlak yeni bir fikrin peşinden koşmak istediğinde:** Kısıtlamalarını kontrol et. Bu, zamanına, becerilerine ve donanımına uyuyor mu? Uymuyorsa, daha sonrası için "İnşa Ettiğim Yön" bölümüne ekle.

### Bir Saatlik Egzersiz

60 dakikalık bir zamanlayıcı kur. Şablonun her alanını doldur. Fazla düşünme. Kapsamlı araştırma yapma. Şu an bildiklerini yaz. Daha sonra güncelleyebilirsin.

Dolduramadığın alanlar? Bunlar bu hafta için yapılacaklar listendeki eylem maddeleri:
- Boş kıyaslama numaraları? Ders 2'deki kıyaslama betiğini çalıştır.
- İşletme tüzel kişiliği yok? Ders 4'teki başvuru sürecini başlat.
- Ödeme işleme yok? Ders 4'ten Stripe'ı kur.
- Boş beceri envanteri? Son 5 yılda para karşılığı yaptığın her şeyi listeleyerek 15 dakika harca.

> **Yaygın Hata:** Belgeyi 1 saatte "bitti" yapmak yerine 3 saat "mükemmel" yapmak için harcamak. Egemen Yığın Belgesi çalışan bir referans, yatırımcılar için iş planı değil. Senden başka kimse görmeyecek. Doğruluk önemli. Biçimlendirme önemli değil.

### Ders 6 Kontrol Noktası

Şimdi şunlara sahip olmalısın:
- [ ] Gerçekten açacağın bir yere kaydedilmiş eksiksiz bir Egemen Yığın Belgesi
- [ ] Altı bölümün tamamı gerçek sayılarla doldurulmuş (hayali olanlarla değil)
- [ ] Kurulumundaki boşluklar için net bir eylem maddeleri listesi
- [ ] İlk aylık inceleme için belirlenmiş bir tarih (şu andan itibaren 30 gün sonra)

---

## Modül S: Tamamlandı

{? if progress.completed("MODULE_S") ?}
> **Modül S tamamlandı.** {= progress.total_count | fallback("7") =} STREETS modülünden {= progress.completed_count | fallback("1") =} tanesini bitirdin. {? if progress.completed_modules ?}Tamamlananlar: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### İki Haftada İnşa Ettiklerin

Şimdi başladığında sahip olmadığın şeylere bak:

1. **Gelir üreten yeteneklerle eşlenmiş bir donanım envanteri** — sadece etiketteki özellikler değil.
2. **Üretim kalitesinde yerel bir LLM yığını** — Ollama ile, gerçek donanımında kıyaslanmış, gerçek iş yükleri için yapılandırılmış.
3. **Nasıl pazarlayacağını bildiğin bir gizlilik avantajı** — belirli kitleler için belirli bir dille.
4. **Yasal ve finansal bir temel** — işletme tüzel kişiliği (veya planı), ödeme işleme, banka hesabı, vergi stratejisi.
5. **Net yatırım getirisi hedefleri ve modeli kanıtlamak için 90 günlük bir son tarih** ile kontrollü bir bütçe.
6. **Bundan sonraki her karar için kullanacağın tek bir referansta yukarıdakilerin tamamını yakalayan bir Egemen Yığın Belgesi.**

Bu, çoğu geliştiricinin asla kurmayacağından daha fazlası. Ciddi söylüyorum. Yan gelir elde etmek isteyen çoğu kişi direkt "havalı bir şey yap" aşamasına atlar ve sonra neden para kazanamadığını merak eder. Sen artık para kazanmak için altyapıya sahipsin.

Ama yönsüz altyapı sadece pahalı bir hobidir. Bu yığını nereye yönlendireceğini bilmen gerekiyor.

{@ temporal market_timing @}

### Sırada Ne Var: Modül T — Teknik Hendekler

Modül S sana temeli verdi. Modül T kritik soruyu yanıtlıyor: **rakiplerin kolayca kopyalayamayacağı bir şeyi nasıl inşa edersin?**

İşte Modül T'nin kapsadıkları:

- **Özel veri hatları** — yasal ve etik olarak yalnızca senin erişebildiğin veri setleri nasıl oluşturulur
- **Özel model yapılandırmaları** — başkalarının varsayılan ayarlarla sunamayacağı çıktı kalitesi üreten ince ayar ve istem mühendisliği
- **Birleşen beceri yığınları** — neden gelir için "Python + sağlık" kombinasyonu "Python + JavaScript"ten daha iyi olur ve benzersiz kombinasyonunu nasıl belirlersin
- **Teknik giriş engelleri** — bir rakibin kopyalamasının aylar süreceği altyapı tasarımları
- **Hendek Denetimi** — projenin savunulabilir bir avantaja mı sahip yoksa sıradan bir hizmet mi olduğunu değerlendirmek için bir çerçeve

Ayda $500 kazanan bir geliştirici ile ayda $5,000 kazanan biri arasındaki fark nadiren beceridir. Hendeklerdir. Teklifini kopyalanması zor kılan şeyler, birisi aynı donanıma ve aynı modellere sahip olsa bile.

### Tam STREETS Yol Haritası

| Modül | Başlık | Odak | Süre |
|--------|-------|-------|----------|
| **S** | Egemen Kurulum | Altyapı, hukuk, bütçe | Hafta 1-2 (tamamlandı) |
| **T** | Teknik Hendekler | Savunulabilir avantajlar, özel varlıklar | Hafta 3-4 |
| **R** | Gelir Motorları | Kodlu belirli paraya çevirme taktik kitapları | Hafta 5-8 |
| **E** | Uygulama Taktik Kitabı | Lansman sekansları, fiyatlandırma, ilk müşteriler | Hafta 9-10 |
| **E** | Gelişen Kenar | Öncü kalma, trend algılama, uyum | Hafta 11-12 |
| **T** | Taktik Otomasyon | Pasif gelir için operasyonları otomatikleştirme | Hafta 13-14 |
| **S** | Akışları Biriktirme | Birden fazla gelir kaynağı, portföy stratejisi | Hafta 15-16 |

Modül R (Gelir Motorları) paranın çoğunun kazanıldığı yerdir. Ama S ve T olmadan, kum üzerine inşa ediyorsun.

---

**Tam taktik kitabına hazır mısın?**

Temeli gördün. Kendin inşa ettin. Şimdi tam sistemi al.

**STREETS Core'u Al** — yedi modülün tamamını, gelir motoru kod şablonlarını, finansal gösterge panellerini ve kendi koşullarında gelir inşa eden geliştiricilerin özel topluluğunu içeren tam 16 haftalık kurs.

*Senin donanımın. Senin kuralların. Senin gelirin.*