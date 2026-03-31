# Modul S: Egemen Kurulum

**STREETS Gelistirici Gelir Kursu — Ucretsiz Modul**
*Hafta 1-2 | 6 Ders | Cikti: Egemen Yigin Belgen*

> "Rigin, is altyapin. Oyle yapilandir."

---

Cogu kisinin asla sahip olamayacagi en guclu gelir uretme aracina zaten sahipsin: internet baglantisi, yerel islem gucu ve hepsini bir araya getirme becerileri olan bir gelistirici is istasyonu.

Cogu gelistirici rigini bir tuketici urunu gibi goruyor. Oyun oynadiklari, kodladiklari, gezdikleri bir sey. Ama ayni makine — su anda masanin altinda duran — cikarsama calistirabilir, API sunabilir, veri isleyebilir ve sen uyurken gunde 24 saat gelir uretebilir.

Bu modul, sahip olduklarina farkli bir acidan bakmakla ilgili. "Ne insa edebilirim?" degil, "Ne satabilirim?"

Bu iki haftanin sonunda elinde olacak:

- Gelir uretme yeteneklerinin net bir envanteri
- Uretim duzeyinde yerel LLM yigini
- Yasal ve finansal bir temel (minimal olsa bile)
- Is planin haline gelecek yazili bir Egemen Yigin Belgesi

Laf kalabaligi yok. "Sadece kendine inan" yok. Gercek sayilar, gercek komutlar, gercek kararlar.

{@ mirror sovereign_readiness @}

Baslayalim.

---

## Ders 1: Rig Denetimi

*"4090'a ihtiyacin yok. Asil onemli olan bu."*

### Makinan Bir Is Varligi

Bir sirket altyapisini degerlendirirken sadece ozellikleri listelemez — yetenekleri gelir firsatlarina eslestirir. Simdi tam olarak bunu yapacaksin.

{? if computed.profile_completeness != "0" ?}
> **Mevcut Rigin:** {= profile.cpu.model | fallback("Bilinmeyen CPU") =} ({= profile.cpu.cores | fallback("?") =} cekirdek / {= profile.cpu.threads | fallback("?") =} is parcacigi), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("Ayrilmis GPU yok") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} bos / {= profile.storage.total | fallback("?") =} toplam ({= profile.storage.type | fallback("bilinmeyen") =}), {= profile.os.name | fallback("bilinmeyen isletim sistemi") =} {= profile.os.version | fallback("") =} calistiriyor.
{? endif ?}

Bir terminal ac ve asagidakileri takip et. Her sayiyi yaz. Ders 6'daki Egemen Yigin Belgen icin bunlara ihtiyacin olacak.

### Donanim Envanteri

#### CPU

```bash
# Linux/Mac
lscpu | grep "Model name\|CPU(s)\|Thread(s)"
# veya
cat /proc/cpuinfo | grep "model name" | head -1
nproc

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# macOS
sysctl -n machdep.cpu.brand_string
sysctl -n hw.ncpu
```

**Gelir icin onemli olan:**
- Cekirdek sayisi, rigin kac esanli gorevi yonetebilecegini belirler. Yerel bir LLM calistirirken ayni anda toplu is islemek gercek paralelizm gerektirir.
{? if profile.cpu.cores ?}
- *{= profile.cpu.model | fallback("CPU") =}'un {= profile.cpu.cores | fallback("?") =} cekirdegi var — asagidaki gereksinimler tablosuna bakarak CPU'nun hangi gelir motorlarini destekledigini gor.*
{? endif ?}
- Bu kurstaki cogu gelir motoru icin son 5 yillik herhangi bir modern 8+ cekirdekli CPU yeterli.
- Yerel LLM'leri yalnizca CPU uzerinde calistiriyorsan (GPU yok), 16+ cekirdek istersin. Ryzen 7 5800X veya Intel i7-12700 pratik minimum.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Gelir icin onemli olan:**
- 16 GB: Mutlak minimum. 7B modelleri calistirabilir ve temel otomasyon isi yapabilirsin.
- 32 GB: Rahat. 13B modelleri yerel olarak calistir, birden fazla projeyi yonet, gelistirme ortamini gelir is yuklerinin yaninda calisir tut.
- 64 GB+: CPU uzerinde 30B+ modelleri calistirabilir veya birden fazla modeli yuklu tutabilirsin. Cikarsama hizmetleri satmak icin isler burada ilginclesiyor.
{? if profile.ram.total ?}
*Sisteminde {= profile.ram.total | fallback("?") =} RAM var. Hangi yetenek seviyesinde oldugunuzu gormek icin yukaridaki tabloyu kontrol et — bu, gelir is yuklerin icin hangi yerel modellerin pratik oldugunu dogrudan etkiler.*
{? endif ?}

#### GPU

```bash
# NVIDIA
nvidia-smi

# VRAM'i ozellikle kontrol et
nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv

# AMD (Linux)
rocm-smi

# macOS (Apple Silicon)
system_profiler SPDisplaysDataType
```

**Gelir icin onemli olan:**

Bu, insanlarin takilip kaldigi tek ozellik ve iste durustce gercek: **GPU'n yerel LLM seviyeni belirler ve yerel LLM seviyen hangi gelir akislarinin en hizli calistigini belirler.** Ama para kazanip kazanamayacagini belirlemez.

| VRAM | LLM Yetenegini | Gelir Iliskisi |
|------|----------------|----------------|
| 0 (yalnizca CPU) | 7B modeller ~5 token/sn | Toplu isleme, asenkron is. Yavas ama fonksiyonel. |
| 6-8 GB (RTX 3060, vb.) | 7B modeller ~30 tok/sn, 13B nicemlendirilmis | Cogu otomasyon gelir akisi icin yeterli. |
| 12 GB (RTX 3060 12GB, 4070) | 13B tam hizda, 30B nicemlendirilmis | Tatli nokta. Cogu gelir motoru burada iyi calisir. |
| 16-24 GB (RTX 4090, 3090) | 30B-70B modeller | Premium seviye. Baskalarinin yerel olarak esdegerini sunamayacagi kalite sat. |
| 48 GB+ (cift GPU, A6000) | 70B+ hizda | Kurumsal duzeyde yerel cikarsama. Ciddi rekabet avantaji. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ birlesik bellek kullanarak | Mukemmel verimlilik. NVIDIA esdegerinden daha dusuk enerji maliyeti. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **GPU'n:** {= profile.gpu.model | fallback("Bilinmeyen") =} {= profile.gpu.vram | fallback("?") =} VRAM ile — {? if computed.gpu_tier == "premium" ?}premium seviyedesin. 30B-70B modeller yerel olarak erisilebilir. Bu ciddi bir rekabet avantaji.{? elif computed.gpu_tier == "sweet_spot" ?}tatli noktadasin. 13B tam hizda, 30B nicemlendirilmis. Cogu gelir motoru burada iyi calisir.{? elif computed.gpu_tier == "capable" ?}7B modelleri iyi hizda ve 13B nicemlendirilmis calistirabilirsin. Cogu otomasyon gelir akisi icin yeterli.{? else ?}GPU hizlandirman mevcut. Nereye dustugunu gormek icin yukaridaki tabloyu kontrol et.{? endif ?}
{? else ?}
> **Ayrilmis GPU algilanmadi.** Cikarsamayi CPU uzerinde calistiracaksin, bu da 7B modellerde ~5-12 token/sn anlamina gelir. Toplu isleme ve asenkron is icin sorun yok. Musteriye donuk cikti icin hiz farki kapatmak icin API cagrilarini kullan.
{? endif ?}

> **Acik Konusalim:** RTX 3060 12GB'in varsa, yapay zekayi paraya cevirmeye calisan gelistiricilerin %95'inden daha iyi bir konumdasin. 4090 beklemeyi birak. 3060 12GB, yerel yapay zekanin Honda Civic'i — guvenilir, verimli, isini gorur. GPU yukseltmesine harcayacagin para, yerel modellerin agir isi hallederken musteriye donuk kalite icin API kredilerine daha iyi harcanir.

#### Depolama

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Gelir icin onemli olan:**
- LLM modelleri yer kaplar: 7B model = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (nicemlendirilmis).
- Proje verileri, veritabanlari, onbellekler ve cikti icin yer lazim.
- Musteriye yonelik her sey icin SSD sart. HDD'den model yukleme 30-60 saniye baslatma suresi ekler.
- Minimum pratik: 500 GB SSD en az 100 GB bos.
- Rahat: 1 TB SSD. Modelleri SSD'de tut, arsivle HDD'de.
{? if profile.storage.free ?}
*{= profile.storage.type | fallback("surucu") =} uzerinde {= profile.storage.free | fallback("?") =} bosun var. {? if profile.storage.type == "SSD" ?}Iyi — SSD hizli model yukleme demek.{? elif profile.storage.type == "NVMe" ?}Mukemmel — NVMe, model yukleme icin en hizli secenek.{? else ?}Henuz SSD kullanmiyorsan dusun — model yukleme sureleri icin gercek bir fark yaratir.{? endif ?}*
{? endif ?}

#### Ag

```bash
# Hizli hiz testi (gerekirse speedtest-cli kur)
# pip install speedtest-cli
speedtest-cli --simple

# Veya sadece tarifeni kontrol et
# Yukleme hizi, sunum icin indirmeden daha onemli
```

**Gelir icin onemli olan:**
{? if profile.network.download ?}
*Baglantini: {= profile.network.download | fallback("?") =} indirme / {= profile.network.upload | fallback("?") =} yukleme.*
{? endif ?}
- Indirme hizi: 50+ Mbps. Modelleri, paketleri ve verileri cekmek icin gerekli.
- Yukleme hizi: Cogu kisinin goz ardi ettigi darbogazdir. Herhangi bir sey sunuyorsan (API'ler, islenmis sonuclar, teslimatlar), yukleme onemlidir.
  - 10 Mbps: Asenkron teslimat icin yeterli (islenmis dosyalar, toplu sonuclar).
  - 50+ Mbps: Dis hizmetlerin eristigi herhangi bir yerel API ucu calistiriyorsan gerekli.
  - 100+ Mbps: Bu kurstaki her sey icin rahat.
- Gecikme: Buyuk bulut saglayicilara 50ms alti. Kontrol icin `ping api.openai.com` ve `ping api.anthropic.com` calistir.

#### Calisma Suresi

Bu, kimsenin dusunmedigi ama hobicileri uyurken para kazananlardan ayiran ozellik.

Kendine sor:
- Rigin 7/24 calisabilir mi? (Guc, sogutma, gurultu)
- Elektrik kesintileri icin UPS'in var mi?
- Internet baglantini otomatik is akislari icin yeterince kararli mi?
- Bir sey bozulursa makinana uzaktan SSH ile erisebilir misin?

7/24 calisamiyorsan sorun yok — bu kurstaki bircok gelir akisi, elle tetikledigin asenkron toplu islerdir. Ama gercekten pasif gelir uretenler calisma suresi gerektirir.

{? if computed.os_family == "windows" ?}
**Hizli calisma suresi kurulumu (Windows):** Otomatik yeniden baslatma icin Gorev Zamanlayici kullan, uzaktan erisim icin Uzak Masaustu'nu etkinlestir veya Tailscale kur, ve elektrik kesintilerinden kurtulmak icin BIOS'u "AC guc kaybinda geri yukle" olarak yapilandir.
{? endif ?}

**Hizli calisma suresi kurulumu (istersen):**

```bash
# Wake-on-LAN'i etkinlestir (BIOS'u kontrol et)
# SSH erisimini kur
sudo systemctl enable ssh  # Linux

# Cokme durumunda otomatik yeniden baslatma (systemd hizmeti ornegi)
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

### Elektrik Matematigi

Insanlar ya bunu goz ardi eder ya da felaket senaryolari yapar. Gercek matematik yapalim.

**Gercek guc tuketimini olcme:**

```bash
# Kill-A-Watt olcer veya izleme ozellikli akilli priz varsa:
# Bosta, yuk altinda (cikarsama calistirirken) ve maksimumda (GPU tam kullanim) olc

# Olcerin yoksa yaklasik tahminler:
# Masaustu (GPU yok, bosta): 60-100W
# Masaustu (orta sinif GPU, bosta): 80-130W
# Masaustu (ust sinif GPU, bosta): 100-180W
# Masaustu (GPU cikarsama yuku altinda): GPU TDP'sinin %50-80'ini ekle
# Dizustu: 15-45W
# Mac Mini M2: 7-15W (ciddi)
# Apple Silicon dizustu: 10-30W
```

**Aylik maliyet hesaplama:**

```
Aylik maliyet = (Watt / 1000) x Saat x kWh basi fiyat

Ornek: RTX 3060 ile masaustu, gunde 8 saat cikarsama, 16 saat bosta
- Cikarsama: (250W / 1000) x 8s x 30 gun x $0.12/kWh = $7.20/ay
- Bosta: (100W / 1000) x 16s x 30 gun x $0.12/kWh = $5.76/ay
- Toplam: ~$13/ay

Ornek: Ayni rig, 7/24 cikarsama
- (250W / 1000) x 24s x 30 gun x $0.12/kWh = $21.60/ay

Ornek: Mac Mini M2, 7/24
- (12W / 1000) x 24s x 30 gun x $0.12/kWh = $1.04/ay
```

{? if regional.country ?}
Elektrik tarifen: yaklasik {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh ({= regional.country | fallback("bolgen") =} ortalamalaryna gore). Gercek faturani kontrol et — tarifeler saglayiciya ve gun icindeki saate gore degisir.
{? else ?}
ABD ortalama elektrigi yaklasik $0.12/kWh. Gercek tarifeni kontrol et — buyuk olcude degisir. Kaliforniya $0.25/kWh olabilir. Bazi Avrupa ulkeleri $0.35/kWh'a ulasiyor. ABD Ortabati'nin bazi bolgeleri $0.08/kWh.
{? endif ?}

**Mesele su:** Gelir icin rigini 7/24 calistirmak elektrikte {= regional.currency_symbol | fallback("$") =}1 ile {= regional.currency_symbol | fallback("$") =}30/ay arasina mal olur. Gelir akislarin bunu karsilayamiyorsa, sorun elektrik degil — gelir akisi.

### Gelir Motoru Turune Gore Minimum Ozellikler

Iste tam STREETS kursunda nereye gittigimizin bir on gorunumu. Simdilik sadece riginin nereye dustugunu kontrol et:

| Gelir Motoru | CPU | RAM | GPU | Depolama | Ag |
|-------------|-----|-----|-----|----------|----|
| **Icerik otomasyonu** (blog yazilari, bultenler) | 4+ cekirdek | 16 GB | Istege bagli (API yedegi) | 50 GB bos | 10 Mbps yukleme |
| **Veri isleme hizmetleri** | 8+ cekirdek | 32 GB | Istege bagli | 200 GB bos | 50 Mbps yukleme |
| **Yerel YZ API hizmetleri** | 8+ cekirdek | 32 GB | 8+ GB VRAM | 100 GB bos | 50 Mbps yukleme |
| **Kod uretim araclari** | 8+ cekirdek | 16 GB | 8+ GB VRAM veya API | 50 GB bos | 10 Mbps yukleme |
| **Belge isleme** | 4+ cekirdek | 16 GB | Istege bagli | 100 GB bos | 10 Mbps yukleme |
| **Otonom ajanlar** | 8+ cekirdek | 32 GB | 12+ GB VRAM | 100 GB bos | 50 Mbps yukleme |

> **Yaygin Hata:** "Baslamadan once donanimi yukseltmeliyim." Hayir. Elindekiyle basla. Donanininin kapatamadigi bosluklar icin API cagrilarini kullan. Gelir hakli kildiginda yukselt — oncesinde degil.

{@ insight engine_ranking @}

### Ders 1 Kontrol Noktasi

Artik yazili olarak elinde olmali:
- [ ] CPU modeli, cekirdekler ve is parcaciklari
- [ ] RAM miktari
- [ ] GPU modeli ve VRAM (veya "yok")
- [ ] Kullanilabilir depolama
- [ ] Ag hizlari (indirme/yukleme)
- [ ] 7/24 isletim icin tahmini aylik elektrik maliyeti
- [ ] Riginin hangi gelir motoru kategorileri icin uygun oldugu

Bu sayilari sakla. Ders 6'daki Egemen Yigin Belgene gireceksin.

{? if computed.profile_completeness != "0" ?}
> **4DA bu sayilarin cogunu senin icin zaten topladi.** Yukaridaki kisisellestirilmis ozetleri kontrol et — donanim envanterin sistem algilamadan kismen onceden doldurumus.
{? endif ?}

*Tam STREETS kursunda, Modul R (Gelir Motorlari) yukarida listelenen her motor turu icin adim adim kiskirtici rehberler verir — bunlari insa etmek ve dagitmak icin gereken tam kodu da dahil.*

---

## Ders 2: Yerel LLM Yigini

*"Ollama'yi uretim kullanimi icin kur — sadece sohbet icin degil."*

### Yerel LLM'ler Gelir Icin Neden Onemli

OpenAI API'sini her aradiginda kira oduyorsun. Yerel bir model her calistirisinda, bu cikarsama baslangic kurulumdan sonra ucretsiz. Matematik basit:

- GPT-4o: ~$5/milyon giris tokeni, ~$15/milyon cikis tokeni
- Claude 3.5 Sonnet: ~$3/milyon giris tokeni, ~$15/milyon cikis tokeni
- Yerel Llama 3.1 8B: $0/milyon token (sadece elektrik)

Binlerce istegi isleyen hizmetler insa ediyorsan, milyon token basina $0 ile $5-$15 arasindaki fark kar ile bas-bas arasindaki farktir.

Ama cogu kisinin kacirdigi nüans su: **yerel ve API modelleri bir gelir yigininda farkli roller ustlenir.** Yerel modeller hacmi halleder. API modelleri kalite-kritik, musteriye donuk ciktilari halleder. Yiginin her ikisine de ihtiyac duyar.

### Ollama Kurulumu

{? if settings.has_llm ?}
> **Zaten yapilandirilmis bir LLM'in var:** {= settings.llm_provider | fallback("Yerel") =} / {= settings.llm_model | fallback("bilinmeyen model") =}. Ollama zaten calisiyorsa, asagidaki "Model Secim Rehberi"ne atla.
{? endif ?}

Ollama temeldir. Makineni temiz bir API ile yerel bir cikarsama sunucusuna donusturur.

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
# https://ollama.com adresinden indir veya:
brew install ollama

# Windows
# https://ollama.com adresinden yukleyiciyi indir
# Veya winget kullan:
winget install Ollama.Ollama
```

{? if computed.os_family == "windows" ?}
> **Windows:** ollama.com'dan yukleyiciyi veya `winget install Ollama.Ollama` kullan. Ollama kurulumdan sonra otomatik olarak arka plan hizmeti olarak calisir.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` en hizli yol. Ollama, Apple Silicon'un birlesik belleginden yararlanir — {= profile.ram.total | fallback("sistem") =} RAM, CPU ve GPU is yukleri arasinda paylasiliyor.
{? elif computed.os_family == "linux" ?}
> **Linux:** Kurulum betigi her seyi halleder. {= profile.os.name | fallback("Linux") =} calistiriyorsan, Ollama systemd hizmeti olarak kurulur.
{? endif ?}

Kurulumu dogrula:

```bash
ollama --version
# Surum 0.5.x veya ustu gostermeli (en son surum icin https://ollama.com/download kontrol et)

# Sunucuyu baslat (otomatik baslamadiysa)
ollama serve

# Baska bir terminalde test et:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

### Model Secim Rehberi

Gordügun her modeli indirme. Stratejik ol. Iste ne indirecen ve her birini ne zaman kullanacagin.

#### Seviye 1: Is Ati (7B-8B modeller)

```bash
# Is ati modelini indir
ollama pull llama3.1:8b
# Alternatif: mistral (Avrupa dilleri icin iyi)
ollama pull mistral:7b
```

**Bunun icin kullan:**
- Metin siniflandirma ("Bu e-posta spam mi yoksa gercek mi?")
- Ozet (uzun belgeleri madde isaretlerine yogunlastir)
- Basit veri cikarma (metinden isim, tarih, tutar cek)
- Duygu analizi
- Icerik etiketleme ve kategorilendirme
- Gomme uretimi (gomme destegi olan bir model kullaniyorsan)

**Performans (tipik):**
- RTX 3060 12GB: ~40-60 token/saniye
- RTX 4090: ~100-130 token/saniye
- M2 Pro 16GB: ~30-45 token/saniye
- Yalnizca CPU (Ryzen 7 5800X): ~8-12 token/saniye

**Maliyet karsilastirmasi:**
- 1 milyon token GPT-4o-mini ile: ~$0.60
- 1 milyon token yerel olarak (8B model): ~$0.003 elektrik
- Basabaslama noktasi: ~5.000 token (kelimenin tam anlamiyla ilk istekten tasarruf edersin)

#### Seviye 2: Dengeli Secim (13B-14B modeller)

```bash
# Dengeli modelini indir
ollama pull llama3.1:14b
# Veya kodlama gorevleri icin:
ollama pull deepseek-coder-v2:16b
```

**Bunun icin kullan:**
- Icerik hazirlama (blog yazilari, dokümantasyon, pazarlama metni)
- Kod uretimi (fonksiyonlar, betikler, sablonlar)
- Karmasik veri donusumu
- Cok adimli muhakeme gorevleri
- Nüansli ceviri

#### Seviye 3: Kalite Seviyesi (30B-70B modeller)

```bash
# Bunlari yalnizca VRAM'in varsa indir
# 30B ~20GB VRAM, 70B ~40GB VRAM gerektirir (nicemlendirilmis)
ollama pull llama3.1:70b-instruct-q4_K_M
# Veya daha kucuk ama mukemmel:
ollama pull qwen2.5:32b
```

> **Acik Konusalim:** 24GB+ VRAM'in yoksa, 70B modellerini tamamen atla. Kalite-kritik cikti icin API cagrilarini kullan. Sistem RAM'inden 3 token/saniye hizda calisan bir 70B model teknik olarak mumkun ama gelir ureten herhangi bir is akisi icin pratik olarak islevsiz. Zamaninin degeri var.

#### Seviye 4: API Modelleri (Yerel Yetmediginde)

Yerel modeller hacim ve gizlilik icindir. API modelleri kalite tavanlari ve ozellestirilmis yetenekler icindir.

**Hibrit strateji:**
1. Yerel LLM 7B/13B isteklerin %80'ini halleder (siniflandirma, cikarma, ozet)
2. API isteklerin %20'sini halleder (son kalite gecisi, karmasik gorevler, musteriye donuk cikti)
3. Etkin maliyetin: ortalama milyon token basina ~$0.50-2.00 (saf API ile $5-15 yerine)

Bu hibrit yaklasim, saglikli marjlarla hizmet kurmanin yoludur. Modul R'de daha fazlasi.

### Uretim Yapilandirmasi

Ollama'yi gelir isi icin calistirmak kisisel sohbet icin calistirmaktan farklidir. Dogru yapilandirma soyle:

#### Ortam Degiskenlerini Ayarla

```bash
# Ollama yapilandimasini olustur/duzenle
# Linux: /etc/systemd/system/ollama.service veya ortam degiskenleri
# macOS: launchctl ortami veya ~/.zshrc
# Windows: Sistem Ortam Degiskenleri

# Ana ayarlar:
export OLLAMA_HOST=127.0.0.1:11434    # Yalnizca localhost'a bagla (guvenlik)
export OLLAMA_NUM_PARALLEL=4            # Esanli istek isleme
export OLLAMA_MAX_LOADED_MODELS=2       # Bellekte 2 model tut
export OLLAMA_KEEP_ALIVE=30m            # Son istekten sonra modeli 30 dk yuklu tut
export OLLAMA_MAX_QUEUE=100             # 100 istege kadar siraya al
```

### Yerel Cikarsama Sunucunu Guvenli Kilma

Ollama ornegi, acikca niyet etmedikce asla internetten erisilebilir olmamali.

```bash
# Ollama'nin yalnizca localhost'ta dinledigini dogrula
ss -tlnp | grep 11434
# 127.0.0.1:11434 gostermeli, 0.0.0.0:11434 DEGIL

# Uzaktan erisime ihtiyacin varsa (orn., LAN'daki baska bir makineden):
# Portu acmak yerine SSH tuneli kullan
ssh -L 11434:localhost:11434 your-rig-ip

# Guvenlik duvari kurallari (Linux)
sudo ufw deny in 11434
sudo ufw allow from 192.168.1.0/24 to any port 11434  # Gerekirse yalnizca LAN
```

> **Yaygin Hata:** "Kolaylik" icin Ollama'yi 0.0.0.0'a baglamak ve sonra unutmak. IP'ni bulan herkes GPU'nu ucretsiz cikarsama icin kullanabilir. Dahasi, model agirliklarini ve sistem istemlerini cikarabilirler. Her zaman localhost. Her zaman tunel.

### Ders 2 Kontrol Noktasi

Artik elinde olmali:
- [ ] Ollama kurulu ve calisiyor
- [ ] En az bir is ati model indirilmis (llama3.1:8b veya esdegeri)
- [ ] Beklenen is yukun icin ozel bir Modelfile
- [ ] Karsilastirma sayilari: rigindeki her model icin token/saniye
- [ ] Ollama yalnizca localhost'a bagli

---

## Ders 3: Gizlilik Avantaji

*"Ozel kurulumun rekabet avantaji — sadece bir tercih degil."*

### Gizlilik Bir Urun Ozelligi, Kisitlama Degil

Cogu gelistirici yerel altyapi kurar cunku kisisel olarak gizlilige deger verir veya kurcalamayi sever. Sorun yok. Ama **gizliligin su anda teknolojide en pazarlanabilir ozelliklerden biri oldugunu** fark etmezsen masada para birakiyorsun.

Sebebi: bir sirket OpenAI API'sine her veri gonderdignde, bu veriler ucuncu bir taraftan gecer. Bircok isletme icin — ozellikle saglik, finans, hukuk, devlet ve AB merkezli sirketler — bu gercek bir sorun. Teorik degil. "Bu araci kullanamayiz cunku uyumluluk hayir dedi" sorunu.

Sen, makinende yerel olarak model calistiran, bu soruna sahip degilsin.

### Gizliligi Satis Konusmalarinda Nasil Konumlandirirsin

Uyum uzmani olmana gerek yok. Uc cumle anlamam ve ne zaman kullanacagini bilmen gerekiyor:

**Cumle 1: "Verileriniz asla altyapinizdan cikmaz."**
Su durumlarda kullan: Gizlilige duyarli herhangi bir potansiyel musteriye konusurken. Bu evrensel kanca.

**Cumle 2: "Ucuncu taraf veri isleme sozlesmesi gerekli degil."**
Su durumlarda kullan: Avrupa sirketleri veya hukuk/uyumluluk ekibi olan herhangi bir sirketle konusurken. Bu onlara haftalarca hukuki incelemeden tasarruf ettirir.

**Cumle 3: "Tam denetim izi, tek kiracili isleme."**
Su durumlarda kullan: Kurumsal musteriler veya duzenlenmis sektorlerle konusurken. YZ hatlarini denetcilere kanitlamalari gerekiyor.

**Premium Fiyatlandirma Gerekcelendirmesi:**

**Standart YZ isleme hizmeti (bulut API'ler kullanarak):**
- Musterinin verileri OpenAI/Anthropic/Google'a gider
- API cagirabilen her gelistiriciyle rekabet ediyorsun
- Piyasa fiyati: islenen belge basina $0.01-0.05

**Gizlilik-oncelikli YZ isleme hizmeti (yerel yiginin):**
- Musterinin verileri makinende kalir
- Cok daha kucuk bir saglayici havuzuyla rekabet ediyorsun
- Piyasa fiyati: islenen belge basina $0.10-0.50 (5-10x prim)

Gizlilik primi gercek: ayni temel gorev icin emtia bulut hizmetlerine gore **5x ila 10x**.

{@ insight competitive_position @}

### Ders 3 Kontrol Noktasi

Artik anlamis olmalisin:
- [ ] Gizliligin neden pazarlanabilir bir urun ozelligi oldugunu
- [ ] Hangi düzenlemelerin yerel YZ islemesi icin talep yarattigi
- [ ] Gizlilikle ilgili satis konusmalarinda kullanilacak uc cumle
- [ ] Gizlilik-oncelikli hizmetlerin 5-10x prim fiyatlandirma nasil emrettigi
- [ ] Gelir isini isveren isindan nasil ayiracagini

---

## Ders 4: Yasal Minimum

*"Simdi on bes dakikalik yasal hazirlk daha sonra aylarca sorunu onler."*

### Bu Hukuki Tavsiye Degil

Ben bir gelistiriciyim, avukat degil. Asagidaki, cogu gelisiricinin cogu durumda ele almasi gereken pratik bir kontrol listesi. Durumun karmasiksa, bir is hukuku avukatiyla 30 dakikalik bir gorusmeye $200 harca. Alacagin en iyi ROI bu.

### Adim 1: Is Sozlesmeni Oku

Is sozlesmeni veya is teklifi mektubunu bul. Su bolumleri ara:

**Fikri mulkiyet devir maddesi** — Su tur ifadeler ara:
- "Tum buluslar, gelistirmeler ve is urunleri..."
- "...istihdam suresi boyunca olusturulan..."
- "...Sirketin isine veya ongordugu ise iliskin..."

### 3 Soru Testi

Herhangi bir gelir projesi icin sor:

1. **Zaman:** Bu isi kendi zamaninda mi yapiyorsun? (Is saatleri disinda, nobetci oldugum zamanlar disinda)
2. **Ekipman:** Kendi donanim, kendi internet, kendi API anahtarlarini mi kullaniyorsun? (Isveren dizustu degil, isveren VPN'i degil, isveren bulut hesaplari degil)
3. **Konu:** Bu isvereninin isiyle ilgisiz mi?

Uc cevap da temizse, neredeyse kesinlikle sorun yok. Herhangi bir cevap bulaniksa, devam etmeden once netlige kavus.

### Adim 2: Is Yapisi Sec

Kisisel varliklarini ticari faaliyetlerinden ayirmak icin bir tuzel kisilge ihtiyacin var.

### Adim 3: Odeme Isleme (15 dakikalik kurulum)

Odeme almanin bir yoluna ihtiyacin var. Bunu simdi kur, ilk musterinin beklediginde degil.

**Stripe (cogu gelistirici icin onerilen):**

```
1. stripe.com'a git
2. Is e-postan ile hesap olustur
3. Kimlik dogrulamasini tamamla
4. Is banka hesabini bagla
5. Artik odeme alabilir, fatura olusturabilir ve abonelik kurabilirsin
```

### Adim 4: Gizlilik Politikasi ve Hizmet Sartlari

Herhangi bir hizmet veya urun cevrimici satiyorsan bunlara ihtiyacin var. Sablon icin avukata odeme.

### Adim 5: Ayri Banka Hesabi

Is gelirini kisisel hesabindan gecirme.

### Adim 6: Gelistirici Ek Geliri Icin Vergi Temelleri

> **Acik Konusalim:** Vergiler, cogu gelistiricinin Nisan'a kadar goz ardi ettigi ve sonra panik yaptigi seydir. Simdi 30 dakika harcamak gercek para ve stres tasarrufu saglar.

**Her seyi ilk gunden izle.** Baska bir sey yoksa basit bir tablo kullan.

### Ders 4 Kontrol Noktasi

Artik elinde olmali (veya bunlar icin bir planin):
- [ ] Is sozlesmendeki FM maddesini okumali
- [ ] 3 Soru Testini planladgin gelir isin icin gecmeli
- [ ] Bir is yapisi secmeli
- [ ] Odeme islemesi kurulmali (Stripe veya Lemon Squeezy)
- [ ] Gizlilik politikasi ve sablon ureteciyle ToS
- [ ] Ayri is banka hesabi
- [ ] Vergi stratejisi: %30 ayirma + ceyreksaylik odeme takvimi

---

## Ders 5: Aylik {= regional.currency_symbol | fallback("$") =}200 Butce

*"Isinin bir yakim orani var. Bil. Kontrol et. Kazandirt."*

### Neden {= regional.currency_symbol | fallback("$") =}200/ay

Ayda iki yuz {= regional.currency | fallback("dolar") =}, bir gelistirici gelir operasyonu icin minimum yasayabilir butcedir. Gercek hizmetler calistirmak, gercek musterilere hizmet vermek ve gercek gelir uretmek icin yeterli. Ayni zamanda hicbir sey calismasa, her seyi riske atacak kadar buyuk de degil.

Hedef basit: **90 gun icinde {= regional.currency_symbol | fallback("$") =}200/ay'i {= regional.currency_symbol | fallback("$") =}600+/ay'a cevir.** Basarabilirsen, bir isin var. Basaramazsan, strateji degistir — butceyi artirma.

### ROI Hesaplamasi

Bu onemli olan tek sayi:

```
Aylik Gelir - Aylik Maliyetler = Net Kar
Net Kar / Aylik Maliyetler = ROI Carpani

Ornek:
$600 gelir - $200 maliyet = $400 kar
$400 / $200 = 2x ROI

Hedef: 3x ROI ($200 harcamada $600+ gelir)
Minimum: 1x ROI ($200 gelir = basabas)
1x altinda: Strateji degistir veya maliyetleri azalt
```

{@ insight cost_projection @}

> **Yaygin Hata:** {= regional.currency_symbol | fallback("$") =}200'u hemen para getirmesi gerekmeyen bir "yatirim" olarak gormek. Hayir. Bu, 90 gunluk bir son tarihi olan bir deney. {= regional.currency_symbol | fallback("$") =}200/ay 90 gun icinde {= regional.currency_symbol | fallback("$") =}200/ay gelir uretmezse, stratejide bir sey degismeli. Kendine karsi durüst ol.

### Ders 5 Kontrol Noktasi

Artik elinde olmali:
- [ ] Dort seviyeye dagilmis ~$200'lik aylik butce
- [ ] Harcama limitleri ayarlanmis API hesaplari
- [ ] Altyapi kararlari alinmis (yalnizca yerel vs. yerel + VPS)
- [ ] Secilmis arac yigini (baslamak icin cogunlukla ucretsiz planlar)
- [ ] ROI hedefleri: 90 gun icinde 3x
- [ ] Net bir kural: ROI kantiladiktan sonra butceyi artir

---

## Ders 6: Egemen Yigin Belgen

*"Her isin bir plani var. Bu seninki — ve iki sayfaya sigiyor."*

### Cikti

Bu, Modul S'te olusturacagin en onemli sey. Egemen Yigin Belgen, gelir uretme altyapin hakkindaki her seyi yakalayan tek bir referans. STREETS kursunun geri kalaninda ona basvuracak, kurulumus gelistikce guncelleyecek ve ne insa edecegini neyi atlayacagini belirlerken sagduyulu kararlar almak icin kullanacaksin.

Yeni bir dosya olustur. Markdown, Google Doc, Notion sayfasi, duz metin — gercekten bakacagin ne olursa olsun. Asagidaki sablonu kullan, Ders 1-5'teki sayilar ve kararlarla her alani doldur.

### Bu Belgeyi Nasil Kullanirsin

1. **Herhangi bir yeni projeye baslamadan once:** Egemen Yiginini kontrol et. Uygulamak icin donanim, zaman, beceri ve butcen var mi?
2. **Herhangi bir sey almadan once:** Butce dagitimini kontrol et. Bu alis planda mi?
3. **Aylik gozden gecirme:** Butcedeki "Gercek" sutununu guncelle. Gelir sayilarini guncelle.
4. **Biri ne yaptiginiz sorudugunda:** "Bugun Ne Sunabilirim" bolunum aninda sunumun.
5. **Yeni parlak bir fikrin pesinden kosmaya cazip geldiginde:** Kisitlamalarini kontrol et.

> **Yaygin Hata:** Belgeyi "mukemmel" yapmak icin 3 saat harcamak yerine "bitmis" yapmak icin 1 saat harcamak. Egemen Yigin Belgesi bir calisma referansidir, yatirimcilar icin is plani degil. Senden baska kimse gormeyecek. Dogruluk onemli. Bicimlendirme degil.

### Ders 6 Kontrol Noktasi

Artik elinde olmali:
- [ ] Gercekten acacagin bir yere kaydedilmis tamamlanmis bir Egemen Yigin Belgesi
- [ ] Gercek sayilarla doldurulmus alti bolumlun hepsi (arzu edilen degil)
- [ ] Kurulumundaki bosluklar icin net bir eylem listesi
- [ ] Ilk aylik gozden gecirme icin belirlenmis bir tarih (bugundan 30 gun sonra)

---

## Modul S: Tamamlandi

{? if progress.completed("MODULE_S") ?}
> **Modul S tamamlandi.** {= progress.total_count | fallback("7") =} STREETS modülünden {= progress.completed_count | fallback("1") =}'ini bitirdin. {? if progress.completed_modules ?}Tamamlananlar: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### Iki Haftada Ne Insa Ettin

Basladigindan bu yana sahip oldugun ve onceden sahip olmadigin seylere bak:

1. **Gelir uretme yeteneklerine eslenmis bir donanim envanteri** — sadece bir etiketteki ozellikler degil.
2. **Ollama ile uretim duzeyinde yerel LLM yigini**, gercek donaniminda test edilmis, gercek is yukleri icin yapilandirilmis.
3. **Nasil pazarlayacagini bildigin bir gizlilik avantaji** — belirli kitleler icin belirli dil ile.
4. **Yasal ve finansal bir temel** — is varligi (veya plan), odeme isleme, banka hesabi, vergi stratejisi.
5. **Net ROI hedefleri ve modeli kanitlamak icin 90 gunluk son tarih ile kontrol edilen bir butce**.
6. **Yukaridakilerin tumunu tek bir referansta yakayan bir Egemen Yigin Belgesi**.

Bu, cogu gelistiricinin kurdugundan fazla. Ciddi.

{@ temporal market_timing @}

### Sirada Ne Var: Modul T — Teknik Hendekler

Modul S sana temeli verdi. Modul T kritik soruyu yanitlar: **rakiplerin kolayca kopyalayamayacagi bir seyi nasil insa edersin?**

### Tam STREETS Yol Haritasi

| Modul | Baslik | Odak | Sure |
|-------|--------|------|------|
| **S** | Egemen Kurulum | Altyapi, hukuk, butce | Hafta 1-2 (tamamlandi) |
| **T** | Teknik Hendekler | Savunulabilir avantajlar, tescilli varliklar | Hafta 3-4 |
| **R** | Gelir Motorlari | Kodlu belirli monetizasyon oyun kitaplari | Hafta 5-8 |
| **E** | Yurutme Oyun Kitabi | Lansman siralari, fiyatlandirma, ilk musteriler | Hafta 9-10 |
| **E** | Gelisen Avantaj | Onde kalmak, trend algilama, adaptasyon | Hafta 11-12 |
| **T** | Taktik Otomasyon | Pasif gelir icin operasyonlari otomatlestirme | Hafta 13-14 |
| **S** | Akis Yigma | Birden fazla gelir kaynagi, portfoy stratejisi | Hafta 15-16 |

Modul R (Gelir Motorlari) paranin cogununun kazanildigi yerdir. Ama S ve T olmadan kum uzerine insa ediyorsun.

---

**Tam oyun kitabi icin hazir misin?**

Temeli gordun. Kendin insa ettin. Simdi tam sistemi al.

**STREETS Core'u Edin** — yedi modulun tumunu, gelir motoru kod sablonlarini, finansal kontrol panellerini ve kendi sartlarinda gelir ureten gelistiricilerin ozel topluluguyla tam 16 haftalik kurs.
