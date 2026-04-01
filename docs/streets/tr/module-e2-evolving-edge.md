# Modül E: Gelişen Sınır

**STREETS Geliştirici Gelir Kursu — Ücretli Modül (2026 Edisyonu)**
*Hafta 11 | 6 Ders | Çıktı: 2026 Fırsat Radarın*

> "Bu modül her Ocak güncellenir. Geçen yıl işe yarayan bu yıl yaramayabilir."

---

Bu modül STREETS'in diğer tüm modüllerinden farklı. Diğer altı modül prensipler öğretir — yavaş yaşlanırlar. Bu modül zamanlama öğretir — hızla tarihi geçer.

Her Ocak, bu modül sıfırdan yeniden yazılır. 2025 edisyonu prompt mühendisliği pazaryerleri, GPT sarmalayıcı uygulamalar ve erken MCP spesifikasyonundan bahsediyordu. O tavsiyelerin bir kısmı bugün sana para kaybettirir. Sarmalayıcı uygulamalar metalaştı. Prompt pazaryerleri çöktü. MCP kimsenin tahmin etmediği bir yönde patladı.

Mesele tam da bu. Piyasalar hareket eder. Geçen yılın el kitabını okuyup harfi harfine uygulayan geliştirici, her fırsata altı ay geç kalan geliştiricidir.

Bu 2026 edisyonu. Şu anda gerçekte olanları yansıtıyor — Şubat 2026 — gerçek piyasa sinyallerine, gerçek fiyatlandırma verilerine ve gerçek benimseme eğrilerine dayalı olarak. Ocak 2027'ye kadar bunun bir kısmı eskimiş olacak. Bu bir kusur değil. Bu tasarım.

Bu modülün sonunda elinde olacaklar:

- 2026 manzarasının net bir resmi ve 2025'ten ne farkı olduğu
- Giriş zorluğu, gelir potansiyeli ve zamanlamaya göre sıralanan yedi spesifik fırsat
- Bir pazara ne zaman gireceğini ve çıkacağını bilmek için bir çerçeve
- Fırsatları otomatik olarak yüzeye çıkaran çalışan bir istihbarat sistemi
- Gelirini gelecekteki değişimlere karşı koruyan bir beceri-sigortalama stratejisi
- Tamamlanmış 2026 Fırsat Radarın — bu yıl yaptığın üç bahis

Tahmin yok. Hype yok. Sadece sinyal.

{@ insight engine_ranking @}

Başlayalım.

---

## Ders 1: 2026 Manzarası — Neler Değişti

*"Zemin kaydı. El kitabın 2024'ten kalma ise, havada duruyorsun."*

### Geliştirici Gelirini Değiştiren Altı Kayma

Her yıl geliştiricilerin nasıl para kazandığını gerçekten etkileyen bir avuç değişiklik olur. "İlginç trendler" değil — gelir akışlarını açan veya kapatan yapısal kaymalar. 2026'da altı tane var.

#### Kayma 1: Yerel LLM'ler "Yeterince İyi" Eşiğini Geçti

En büyüğü bu. 2024'te yerel LLM'ler bir yenilikti — oynamak eğlenceliydi, üretim için yeterince güvenilir değildi. 2025'te yaklaştılar. 2026'da çizgiyi geçtiler.

**"Yeterince iyi" pratikte ne anlama geliyor:**

| Metrik | 2024 (Yerel) | 2026 (Yerel) | Bulut GPT-4o |
|--------|-------------|-------------|--------------|
| Kalite (MMLU benchmark) | ~%55 (7B) | ~%72 (13B) | ~%88 |
| RTX 3060'ta hız | 15-20 tok/s | 35-50 tok/s | G/D (API) |
| RTX 4070'te hız | 30-40 tok/s | 80-120 tok/s | G/D (API) |
| Bağlam penceresi | 4K token | 32K-128K token | 128K token |
| 1M token başına maliyet | ~$0.003 (elektrik) | ~$0.003 (elektrik) | $5.00-15.00 |
| Gizlilik | Tam yerel | Tam yerel | Üçüncü taraf işleme |

**Önemli modeller:**
- **Llama 3.3 (8B, 70B):** Meta'nın iş atı. 8B her şeyde çalışır. 70B, 24GB kartla sıfır marjinal maliyetle GPT-3.5 kalitesinde.
- **Mistral Large 2 (123B) ve Mistral Nemo (12B):** Avrupa dilleri için sınıfının en iyisi. Nemo modeli 12B'de boyutunun çok üstünde performans gösteriyor.
- **Qwen 2.5 (7B-72B):** Alibaba'nın açık ağırlık ailesi. Kodlama görevleri için mükemmel. 32B versiyonu tatlı nokta — yapılandırılmış çıktıda GPT-4'e yakın kalite.
- **DeepSeek V3 (damıtılmış varyantlar):** Maliyet-verimlilik kralı. Damıtılmış modeller yerelde çalışır ve bir yıl önce bu boyuttaki her şeyi zorlayan muhakeme görevlerini halleder.
- **Phi-3.5 / Phi-4 (3.8B-14B):** Microsoft'un küçük modelleri. Boyutlarına göre şaşırtıcı derecede yetenekli. 14B modeli kodlama benchmarklarında çok daha büyük açık modellerle rekabet edebiliyor.

**Bu gelir için neden önemli:**

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("GPU") =}'n seni burada güçlü bir pozisyona koyuyor. Donanımında yerel çıkarım, AI destekli hizmetler için neredeyse sıfır marjinal maliyet demektir.
{? else ?}
Özel bir GPU olmadan bile, daha küçük modellerle (3B-8B) CPU tabanlı çıkarım birçok gelir getirici görev için uygulanabilir. Bir GPU yükseltmesi aşağıdaki fırsatların tamamını açacaktır.
{? endif ?}

Maliyet denklemi tersine döndü. 2024'te bir AI destekli hizmet kuruyorduysan, en büyük süregelen maliyetin API çağrılarıydı. Milyon token başına $5-15 ile marjın API'yi ne kadar verimli kullandığına bağlıydı. Şimdi, görevlerin %80'i için çıkarımı yerelde fiilen sıfır marjinal maliyetle çalıştırabilirsin. Tek maliyetlerin elektrik (~{= regional.currency_symbol | fallback("$") =}0.003/milyon token) ve zaten sahip olduğun donanım.

Bu ne anlama geliyor:
1. **Daha yüksek marjlar** AI destekli hizmetlerde (işleme maliyetleri %99 düştü)
2. **Daha fazla ürün uygulanabilir** (API fiyatlarında kârsız olan fikirler artık işliyor)
3. **Gizlilik bedava** (yerel işleme ile kalite arasında takas yok)
4. **Özgürce deneyebilirsin** (prototipleme sırasında API faturası kaygısı yok)

{? if computed.has_nvidia ?}
NVIDIA {= profile.gpu.model | fallback("GPU") =}'nla CUDA hızlandırma ve en geniş model uyumluluğuna erişimin var. Çoğu yerel çıkarım çerçevesi (llama.cpp, vLLM, Unsloth) önce NVIDIA için optimize edilmiştir. Bu, AI destekli hizmetler kurmak için doğrudan bir rekabet avantajı.
{? endif ?}

```bash
# Bunu şu anda kendi donanımında doğrula
ollama pull qwen2.5:14b
time ollama run qwen2.5:14b "Write a professional cold email to a CTO about deploying local AI infrastructure. Include 3 specific benefits. Keep it under 150 words." --verbose

# Çıktıdaki token/saniye değerini kontrol et
# 20 tok/s üzerindeysen, bu model üzerinde üretim hizmetleri kurabilirsin
```

> **Açık Konuşalım:** "Yeterince iyi," "Claude Opus veya GPT-4o kadar iyi" anlamına gelmiyor. Müşteriye fatura ettiğin belirli görev için yeterince iyi anlamına geliyor. E-posta konu satırları yazan, destek biletlerini sınıflandıran veya faturalardan veri çıkaran yerel 13B model, bu görevler için bulut modelinden ayırt edilemez. Yerel modellerin sınır modellerini her konuda yakalamasını beklemeyi bırak. Buna ihtiyaçları yok. SENİN kullanım durumunda yakalamaları gerekiyor.

#### Kayma 2: MCP Yeni Bir Uygulama Ekosistemi Yarattı

Model Context Protocol, 2024 sonundaki spesifikasyon duyurusundan 2026 başında binlerce sunucudan oluşan bir ekosisteme ulaştı. Bu, herkesin tahmin ettiğinden daha hızlı gerçekleşti.

**MCP nedir (30 saniyelik versiyon):**

MCP, AI araçlarının (Claude Code, Cursor, Windsurf, vb.) "sunucular" aracılığıyla harici hizmetlere bağlanmasını sağlayan standart bir protokoldür. Bir MCP sunucusu, bir AI asistanının kullanabileceği araçları, kaynakları ve promptları sunar. Bunu AI için USB olarak düşün — herhangi bir AI aracının herhangi bir hizmetle konuşmasını sağlayan evrensel bir bağlayıcı.

**Mevcut durum (Şubat 2026):**

```
Yayınlanan MCP Sunucuları:              ~4,000+
100+ kullanıcılı MCP Sunucuları:        ~400
Gelir getiren MCP Sunucuları:           ~50-80
Ücretli sunucu başına ortalama gelir:   $800-2,500/ay
Baskın barındırma:                      npm (TypeScript), PyPI (Python)
Merkezi pazaryeri:                      Henüz yok (fırsat bu)
```

**Bu neden App Store anı:**

Apple 2008'de App Store'u piyasaya sürdüğünde, faydalı uygulamalar yayınlayan ilk geliştiriciler orantısız getiri elde etti — daha iyi mühendis oldukları için değil, erken oldukları için. Uygulama ekosistemi henüz inşa edilmemişti. Talep arzı çok aşıyordu.

MCP aynı aşamada. Claude Code ve Cursor kullanan geliştiriciler şunlar için MCP sunucularına ihtiyaç duyuyor:
- Şirketlerinin dahili araçlarına bağlanmak (Jira, Linear, Notion, özel API'ler)
- Belirli formatlardaki dosyaları işlemek (tıbbi kayıtlar, hukuki belgeler, finansal tablolar)
- Niş veri kaynaklarına erişmek (sektör veritabanları, devlet API'leri, araştırma araçları)
- İş akışlarını otomatikleştirmek (dağıtım, test, izleme, raporlama)

Bu sunucuların çoğu henüz mevcut değil. Mevcut olanlar genellikle kötü belgelenmiş, güvenilmez veya temel özelliklerden yoksun. "X için en iyi MCP sunucusu" çıtası şu anda oldukça düşük.

**İşte bu kadar erişilebilir olduğunu gösteren basit bir MCP sunucusu:**

```typescript
// mcp-server-example/src/index.ts
// package.json bağımlılıklarını analiz eden basit bir MCP sunucusu
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { readFileSync, existsSync } from "fs";
import { join } from "path";

const server = new McpServer({
  name: "dependency-analyzer",
  version: "1.0.0",
});

server.tool(
  "analyze_dependencies",
  "Analyze a project's dependencies for security, freshness, and cost implications",
  {
    project_path: z.string().describe("Path to the project root"),
  },
  async ({ project_path }) => {
    const pkgPath = join(project_path, "package.json");
    if (!existsSync(pkgPath)) {
      return {
        content: [{ type: "text", text: "No package.json found at " + pkgPath }],
      };
    }

    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
    const deps = Object.entries(pkg.dependencies || {});
    const devDeps = Object.entries(pkg.devDependencies || {});

    const analysis = {
      total_dependencies: deps.length,
      total_dev_dependencies: devDeps.length,
      dependencies: deps.map(([name, version]) => ({
        name,
        version,
        pinned: !String(version).startsWith("^") && !String(version).startsWith("~"),
      })),
      unpinned_count: deps.filter(([_, v]) => String(v).startsWith("^") || String(v).startsWith("~")).length,
      recommendation: deps.length > 50
        ? "High dependency count. Consider auditing for unused packages."
        : "Dependency count is reasonable.",
    };

    return {
      content: [{
        type: "text",
        text: JSON.stringify(analysis, null, 2),
      }],
    };
  }
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch(console.error);
```

```bash
# Paketle ve yayınla
npm init -y
npm install @modelcontextprotocol/sdk zod
npx tsc --init
# ... derle ve npm'e yayınla
npm publish
```

Bu yayınlanabilir bir MCP sunucusu. 50 satır gerçek mantık aldı. Ekosistem, bu kadar basit faydalı sunucuların gerçekten değerli olmasına yetecek kadar genç.

#### Kayma 3: AI Kodlama Araçları Geliştiricileri 2-5 Kat Daha Üretken Yaptı

Bu hype değil — ölçülebilir. Claude Code, Cursor ve Windsurf, tek bir geliştiricinin ne kadar hızlı teslim edebildiğini temelden değiştirdi.

**Gerçek üretkenlik çarpanları:**

| Görev | AI Araçlarından Önce | AI Araçlarıyla (2026) | Çarpan |
|-------|---------------------|----------------------|--------|
| Auth, DB, dağıtımlı yeni proje iskelesi | 2-3 gün | 2-4 saat | ~5x |
| Mevcut kod için kapsamlı testler yazma | 4-8 saat | 30-60 dakika | ~6x |
| 10+ dosyada modül refaktörü | 1-2 gün | 1-2 saat | ~8x |
| Sıfırdan CLI aracı oluşturma | 1-2 hafta | 1-2 gün | ~5x |
| Bir API için dokümantasyon yazma | 1-2 gün | 2-3 saat | ~4x |
| Karmaşık üretim sorununu hata ayıklama | Saatlerce arama | Dakikalar içinde hedefli analiz | ~3x |

**Bu gelir için ne anlama geliyor:**

Hafta sonunu alan proje artık bir akşam alıyor. Bir ay süren MVP artık bir hafta sürüyor. Bu saf kaldıraç — haftada aynı 10-15 saat yan çalışma artık 2-5 kat daha fazla çıktı üretiyor.

Ama çoğu kişinin kaçırdığı şey şu: **çarpan rakiplerin için de geçerli.** Herkes daha hızlı teslim edebiliyorsa, avantaj *doğru* şeyi teslim eden geliştiricilere gider, sadece *herhangi bir* şeyi değil. Hız artık temel gereksinim. Zevk, zamanlama ve konumlandırma farklılaştırıcılar.

> **Yaygın Hata:** AI kodlama araçlarının derin uzmanlık ihtiyacını ortadan kaldırdığını varsaymak. Kaldırmıyor. Getirdiğin beceri seviyesini yükseltiyorlar. Claude Code kullanan kıdemli geliştirici, kıdemli kalitede kodu daha hızlı üretir. Claude Code kullanan acemi geliştirici, acemi kalitede kodu daha hızlı üretir — acemi kalitede mimari kararlar, acemi kalitede hata yönetimi ve acemi kalitede güvenlik uygulamaları dahil. Araçlar seni daha hızlı yapıyor, daha iyi değil. Daha iyi olmaya yatırım yap.

#### Kayma 4: Gizlilik Düzenlemeleri Gerçek Talep Yarattı

{? if regional.country ?}
Bu kaymanın {= regional.country | fallback("bölgen") =}'da spesifik etkileri var. Aşağıdaki detayları kendi yerel düzenleyici ortamını aklında tutarak oku.
{? endif ?}

Bu 2026'da teorik olmaktan çıktı.

**AB AI Yasası uygulama takvimi (şu anda neredeyiz):**

```
Şub 2025: Yasaklanan AI uygulamaları yasağa alındı (uygulama aktif)
Ağu 2025: GPAI model yükümlülükleri yürürlüğe girdi
Şub 2026: ← ŞU ANDA BURADAYIZ — Tam şeffaflık yükümlülükleri aktif
Ağu 2026: Yüksek riskli AI sistemi gereksinimleri tamamen uygulanacak
```

Şubat 2026 kilometre taşı önemli çünkü şirketler artık AI veri işleme boru hatlarını belgelemek zorunda. Bir şirket çalışan verilerini, müşteri verilerini veya tescilli kodu bir bulut AI sağlayıcısına her gönderdiğinde, bu belgeleme, risk değerlendirmesi ve uyumluluk incelemesi gerektiren bir veri işleme ilişkisi.

**Geliştirici gelirine gerçek dünya etkisi:**

- **Hukuk firmaları** müvekkil belgelerini ChatGPT'ye gönderemez. Yerel alternatifler gerekiyor. Bütçe: kurulum için {= regional.currency_symbol | fallback("$") =}5,000-50,000.
- **Sağlık şirketleri** klinik notlar için AI'ya ihtiyaç duyuyor ama hasta verilerini harici API'lere gönderemez. Bütçe: HIPAA uyumlu yerel dağıtım için {= regional.currency_symbol | fallback("$") =}10,000-100,000.
- **Finans kuruluşları** AI destekli kod incelemesi istiyor ama güvenlik ekipleri tüm bulut AI sağlayıcılarını veto etti. Bütçe: şirket içi dağıtım için {= regional.currency_symbol | fallback("$") =}5,000-25,000.
- **Her büyüklükteki AB şirketleri** "OpenAI kullanıyoruz"un artık bir uyumluluk sorumluluğu olduğunu fark ediyor. Alternatifler gerekiyor. Bütçe: değişken, ama aktif olarak arıyorlar.

"Local-first" inek tercihinden uyumluluk gereksinimine dönüştü. Modelleri yerelde dağıtmayı biliyorsan, işletmelerin prim ücretleri ödeyeceği bir becerin var.

#### Kayma 5: "Vibe Kodlama" Ana Akıma Geçti

"Vibe kodlama" terimi — geliştirici olmayanların AI yardımıyla uygulama oluşturmasını tanımlamak için türetildi — 2025-2026'da bir memeden bir harekete dönüştü. Milyonlarca ürün yöneticisi, tasarımcı, pazarlamacı ve girişimci artık Bolt, Lovable, v0, Replit Agent ve Claude Code gibi araçlarla yazılım oluşturuyor.

**Ne inşa ediyorlar:**
- Dahili araçlar ve panolar
- Açılış sayfaları ve pazarlama siteleri
- Basit CRUD uygulamaları
- Chrome uzantıları
- Otomasyon iş akışları
- Mobil prototipler

**Nerede duvara çarpıyorlar:**
- Kimlik doğrulama ve kullanıcı yönetimi
- Veritabanı tasarımı ve veri modelleme
- Dağıtım ve DevOps
- Performans optimizasyonu
- Güvenlik (neyi bilmediklerini bilmiyorlar)
- Söz dizimi değil sistem anlayışı gerektiren her şey

**Bunun gerçek geliştiriciler için yarattığı fırsat:**

1. **Altyapı ürünleri** — Auth çözümleri, veritabanı sarmalayıcıları, "sadece çalışan" dağıtım araçları gerekiyor. Bunları inşa et.
2. **Eğitim** — Ürünleri anlayan ama sistemleri anlamayan insanlar için yazılmış rehberler gerekiyor. Onlara öğret.
3. **Kurtarma danışmanlığı** — Neredeyse çalışan bir şey inşa ediyorlar, sonra son %20'yi düzeltmek için gerçek bir geliştirici gerekiyor. Bu $100-200/saat iş.
4. **Şablonlar ve başlangıç kitleri** — Zor kısımları (auth, ödemeler, dağıtım) halleden başlangıç noktaları gerekiyor, böylece kolay kısımlara (UI, içerik, iş mantığı) odaklanabilsinler. Bunları sat.

Vibe kodlama geliştiricileri modası geçmiş yapmadı. Yeni bir müşteri segmenti yarattı: geliştirici kalitesinde altyapıya ihtiyaç duyan ama geliştirici-karmaşıklığında paketlenmiş yarı-teknik yapıcılar.

#### Kayma 6: Geliştirici Araçları Pazarı Yıldan Yıla %40 Büyüdü

Dünya çapındaki profesyonel geliştirici sayısı 2026'da yaklaşık 30 milyona ulaştı. Kullandıkları araçlar — IDE'ler, dağıtım platformları, izleme, test, CI/CD, veritabanları — 45 milyar doların üzerinde bir pazara büyüdü.

Daha fazla geliştirici, daha fazla araç, daha fazla niş, bağımsız yapıcılar için daha fazla fırsat demektir.

**2025-2026'da açılan nişler:**
- AI ajan izleme ve gözlemlenebilirlik
- MCP sunucu yönetimi ve barındırma
- Yerel model değerlendirme ve karşılaştırma
- Gizlilik öncelikli analitik alternatifleri
- Geliştirici iş akışı otomasyonu
- AI destekli kod incelemesi ve dokümantasyon

Her nişte 3-5 başarılı ürün için yer var. Çoğunda şu anda 0-1.

### Bileşik Etki

2026'nın neden istisnai olduğu buradan geliyor. Yukarıdaki her kayma tek başına anlamlı olurdu. Birlikte bileşik etki yaratıyorlar:

```
Yerel LLM'ler üretime hazır
    × AI kodlama araçları seni 5 kat daha hızlı yapıyor
    × MCP yeni bir dağıtım kanalı yarattı
    × Gizlilik düzenlemeleri alıcılarda aciliyet yarattı
    × Vibe kodlama yeni müşteri segmentleri yarattı
    × Büyüyen geliştirici nüfusu her pazarı genişletiyor

= App Store döneminden bu yana geliştirici bağımsız geliri için en büyük pencere
```

Bu pencere sonsuza dek açık kalmayacak. Büyük oyuncular MCP pazaryerini inşa ettiğinde, gizlilik danışmanlığı metalaştığında, vibe kodlama araçları geliştirici yardımına ihtiyaç duymayacak kadar olgunlaştığında — erken hareket avantajı daralır. Konumlanma zamanı şimdi.

{? if dna.is_full ?}
Developer DNA'na dayanarak, bu altı kaymayla en güçlü uyumun {= dna.top_engaged_topics | fallback("en çok etkileşim kurduğun konular") =} üzerinde yoğunlaşıyor. Ders 2'deki fırsatlar bunu dikkate alarak sıralandı — mevcut etkileşiminin piyasa zamanlamasıyla kesiştiği yerlere özellikle dikkat et.
{? endif ?}

### Sıra Sende

1. **2025 varsayımlarını denetle.** Bir yıl önce AI, piyasalar veya fırsatlar hakkında neye inanıyordun ama artık doğru değil? Değişen üç şeyi yaz.
2. **Kaymaları becerilerinle eşleştir.** Yukarıdaki altı kaymanın her biri için SENİN durumunu nasıl etkilediğine dair bir cümle yaz. Hangi kaymalar senin için rüzgar arkası? Hangileri karşı rüzgar?
3. **Bir yerel modeli test et.** Son 30 günde yerel model çalıştırmadıysan, `qwen2.5:14b`'yi çek ve işinden gerçek bir görev ver. Oyuncak prompt değil — gerçek görev. Kaliteyi not et. Gelir fikirlerinden herhangi biri için "yeterince iyi" mi?

---

## Ders 2: 2026'nın En Sıcak 7 Fırsatı

*"Spesifiklik olmadan fırsat sadece ilhamdır. İşte spesifikler."*

Aşağıdaki her fırsat için şunları alıyorsun: ne olduğu, mevcut piyasa, rekabet seviyesi, giriş zorluğu, gelir potansiyeli ve "Bu Hafta Başla" eylem planı. Bunlar soyut değil — uygulanabilir.

{? if stack.primary ?}
Bir {= stack.primary | fallback("geliştirici") =} geliştiricisi olarak, bu fırsatlardan bazıları sana diğerlerinden daha doğal gelecek. Sorun değil. En iyi fırsat gerçekten uygulayabileceğin fırsattır, en yüksek teorik tavana sahip olan değil.
{? endif ?}

{? if computed.experience_years < 3 ?}
> **Erken kariyer geliştiriciler için (3 yıldan az):** Fırsat 1 (MCP Sunucuları), 2 (AI-Yerel Geliştirici Araçları) ve 5'e (AI Destekli Geliştirici Olmayanlar İçin Araçlar) odaklan. Bunlar en düşük giriş engellerine sahip ve başlamak için derin alan uzmanlığı gerektirmiyor. Avantajın hız ve deney istekliliği — hızlı teslim et, piyasadan öğren, iterasyon yap. Bir sicil oluşturana kadar Fırsat 4 ve 6'dan kaçın.
{? elif computed.experience_years < 8 ?}
> **Orta kariyer geliştiriciler için (3-8 yıl):** Yedi fırsatın hepsi senin için uygulanabilir, ama Fırsat 3 (Yerel AI Dağıtım Hizmetleri), 4 (Hizmet Olarak İnce Ayar) ve 6 (Uyumluluk Otomasyonu) birikmiş deneyim ve üretim tecrübeni özellikle ödüllendiriyor. Bu alanlardaki müşteriler, işlerin ters gittiğini görmüş ve bunu önlemeyi bilen birisi için para ödüyor. Deneyimin farklılaştırıcı.
{? else ?}
> **Kıdemli geliştiriciler için (8+ yıl):** Fırsat 3 (Yerel AI Dağıtım Hizmetleri), 4 (Hizmet Olarak İnce Ayar) ve 6 (Uyumluluk Otomasyonu) en yüksek kaldıraçlı hamlelerin. Bunlar uzmanlığın prim ücretlere hükmettiği ve müşterilerin özellikle deneyimli uygulayıcıları aradığı pazarlar. Bunlardan birini Fırsat 7 (Geliştirici Eğitimi) ile birleştirmeyi düşün — deneyimin içeriğin. On yılda öğrendiklerini öğreten kıdemli geliştirici, blog yazılarını derleyen acemi geliştiriciden çok daha değerli.
{? endif ?}

{? if stack.contains("react") ?}
> **React geliştiricileri:** Fırsat 1 (MCP Sunucuları — MCP sunucu yönetimi için panolar ve UI'lar oluştur), 2 (AI-Yerel Geliştirici Araçları — React tabanlı geliştirici deneyimleri) ve 5 (AI Destekli Geliştirici Olmayanlar İçin Araçlar — teknik olmayan kullanıcılar için React frontend) doğrudan güçlü yönlerini kullanıyor.
{? endif ?}
{? if stack.contains("rust") ?}
> **Rust geliştiricileri:** Fırsat 1 (MCP Sunucuları — yüksek performanslı sunucular), 3 (Yerel AI Dağıtımı — sistem düzeyinde optimizasyon) ve Tauri tabanlı masaüstü araçlar oluşturma Rust'ın performans ve güvenlik garantilerinden yararlanıyor. Rust ekosisteminin sistem programlamadaki olgunluğu, sadece web geliştiricilerinin ulaşamayacağı pazarlara erişim sağlıyor.
{? endif ?}
{? if stack.contains("python") ?}
> **Python geliştiricileri:** Fırsat 3 (Yerel AI Dağıtımı), 4 (Hizmet Olarak İnce Ayar) ve 7 (Geliştirici Eğitimi) doğal uyumlar. ML/AI ekosistemi Python-yerel ve mevcut veri boru hatları, model eğitimi ve dağıtım bilgin doğrudan gelire dönüşüyor.
{? endif ?}

### Fırsat 1: MCP Sunucu Pazaryeri

**AI araçları için App Store anı.**

**Ne olduğu:** AI kodlama araçlarını harici hizmetlere bağlayan MCP sunucuları oluşturmak, derlemek ve barındırmak. Bu sunucuların kendisi VEYA onları dağıtan pazaryeri olabilir.

**Pazar büyüklüğü:** Claude Code, Cursor veya Windsurf kullanan her geliştirici MCP sunucularına ihtiyaç duyuyor. Bu 2026 başında yaklaşık 5-10 milyon geliştirici, yıllık %100+ büyüme. Çoğu 0-3 MCP sunucusu kurmuş. Doğru olanları mevcut olsa 10-20 kurarlardı.

**Rekabet:** Çok düşük. Henüz merkezi pazaryeri yok. Smithery.ai en yakın, ama erken aşamada ve listelemeye odaklanmış, barındırma veya kalite küratörlüğüne değil. npm ve PyPI fiili dağıtım görevi görüyor ama MCP'ye özel sıfır keşfedilebilirlikle.

**Giriş zorluğu:** Bireysel sunucular için düşük (faydalı bir MCP sunucusu 100-500 satır kod). Pazaryeri için orta (küratörlük, kalite standartları, barındırma altyapısı gerekiyor).

**Gelir potansiyeli:**

| Model | Fiyat Noktası | $3K/ay İçin Gereken Hacim | Zorluk |
|-------|-------------|--------------------------|--------|
| Ücretsiz sunucular + danışmanlık | $150-300/saat | 10-20 saat/ay | Düşük |
| Premium sunucu paketleri | $29-49/paket | 60-100 satış/ay | Orta |
| Barındırılan MCP sunucuları (yönetilen) | $9-19/ay/sunucu | 160-330 abone | Orta |
| MCP pazaryeri (listeleme ücreti) | $5-15/ay/yayıncı | 200-600 yayıncı | Yüksek |
| Kurumsal özel MCP geliştirme | $5K-20K/proje | 1 proje/çeyrek | Orta |

**Bu Hafta Başla:**

```bash
# Gün 1-2: Gerçek bir sorunu çözen ilk MCP sunucunu oluştur
# SENİN ihtiyacın olan bir şey seç — bu genellikle başkalarının da ihtiyacıdır

# Örnek: npm paket sağlığını kontrol eden bir MCP sunucusu
mkdir mcp-package-health && cd mcp-package-health
npm init -y
npm install @modelcontextprotocol/sdk zod node-fetch

# Gün 3-4: Claude Code veya Cursor ile test et
# claude_desktop_config.json veya .cursor/mcp.json'a ekle

# Gün 5: npm'e yayınla
npm publish

# Gün 6-7: İki sunucu daha oluştur. Yayınla. Blog yazısı yaz.
# "Bu hafta 3 MCP sunucusu oluşturdum — öğrendiklerim"
```

Şubat 2026'da 10 faydalı MCP sunucusu yayınlamış kişi, Eylül 2026'da ilkini yayınlayan kişiye göre önemli bir avantaja sahip olacak. Erken hareket burada önemli. Kalite daha önemli. Ama ortaya çıkmak en önemlisi.

### Fırsat 2: Yerel AI Danışmanlığı

**İşletmeler AI istiyor ama verilerini OpenAI'ye gönderemez.**

**Ne olduğu:** Şirketlerin LLM'leri kendi altyapılarına — şirket içi sunucular, özel bulut veya hava boşluklu ortamlar — dağıtmasına yardım etmek. Model seçimi, dağıtım, optimizasyon, güvenlik güçlendirme ve süregelen bakım dahil.

**Pazar büyüklüğü:** AI yetenekleri isteyen hassas verilere sahip her şirket. Hukuk firmaları, sağlık kuruluşları, finans kuruluşları, devlet yüklenicileri, her büyüklükteki AB şirketleri. Toplam Erişilebilir Pazar çok büyük, ama daha da önemlisi, *Hizmet Verilebilir Erişilebilir Pazar* — şu anda aktif olarak yardım arayan şirketler — AB AI Yasası kilometre taşları devreye girdikçe aylık büyüyor.

**Rekabet:** Düşük. Çoğu AI danışmanı bulut çözümlerini (OpenAI/Azure/AWS) itiyor çünkü bildikleri bu. Ollama, vLLM veya llama.cpp'yi doğru güvenlik, izleme ve uyumluluk belgeleriyle üretim ortamında dağıtabilen danışman havuzu küçücük.

{? if profile.gpu.exists ?}
**Giriş zorluğu:** Orta — ve donanımın zaten yeterli. Model dağıtımı, Docker/Kubernetes, ağ ve güvenlik konularında gerçek uzmanlık gerekiyor. {= profile.gpu.model | fallback("GPU'n") =} ile altyapılarına dokunmadan önce müşterilere kendi makinende yerel dağıtım gösterebilirsin.
{? else ?}
**Giriş zorluğu:** Orta. Model dağıtımı, Docker/Kubernetes, ağ ve güvenlik konularında gerçek uzmanlık gerekiyor. Not: danışmanlık müşterilerinin kendi donanımları olacak — dağıtım konusunda danışmanlık için güçlü GPU'ya ihtiyacın yok, ama demo için bir tane olması anlaşma kapatmaya yardımcı olur.
{? endif ?}
Ama STREETS'in Modül S'sini tamamladıysan ve Ollama'yı üretimde dağıtabiliyorsan, kendine "AI danışmanı" diyen insanların %95'inden daha fazla pratik uzmanlığın var.

**Gelir potansiyeli:**

| Angajman Türü | Fiyat Aralığı | Tipik Süre | Sıklık |
|---------------|-------------|-----------|--------|
| Keşif/denetim görüşmesi | $0 (potansiyel müşteri kazanımı) | 30-60 dk | Haftalık |
| Mimari tasarım | $2,000-5,000 | 1-2 hafta | Aylık |
| Tam dağıtım | $5,000-25,000 | 2-6 hafta | Aylık |
| Model optimizasyonu | $2,000-8,000 | 1-2 hafta | Aylık |
| Güvenlik güçlendirme | $3,000-10,000 | 1-3 hafta | Üç aylık |
| Süregelen sabit ücret | $1,000-3,000/ay | Süregelen | Aylık |
| Uyumluluk belgeleri | $2,000-5,000 | 1-2 hafta | Üç aylık |

$2,000/ay sabit ücretli tek bir kurumsal müşteri, ara sıra proje işiyle birlikte yılda $30,000-50,000 değerinde olabilir. Tam zamanlı maaşın yerine 2-3 tane gerekiyor.

**Bu Hafta Başla:**

1. Bir blog yazısı yaz: "Llama 3.3'ü Kurumsal Kullanım İçin Dağıtma: Güvenlik Öncelikli Rehber." Gerçek komutlar, gerçek yapılandırma, gerçek güvenlik değerlendirmeleri ekle. İnternetteki en iyi rehber yap.
2. LinkedIn'de şu başlıkla yayınla: "Şirketiniz AI istiyor ama güvenlik ekibiniz verileri OpenAI'ye göndermeyi onaylamıyorsa, başka bir yol var."
3. Düzenlenen sektörlerdeki orta ölçekli şirketlerde (100-1000 çalışan) 10 CTO'ya veya Mühendislik VP'sine DM at. De ki: "Şirketlerin AI'yı kendi altyapılarında dağıtmasına yardım ediyorum. Hiçbir veri ağınızı terk etmez. 15 dakikalık bir görüşme faydalı olur mu?"

Bu sıralama — uzmanlık yaz, uzmanlık yayınla, alıcılara ulaş — danışmanlık satış makinasının tamamı.

> **Açık Konuşalım:** "Kendimi uzman gibi hissetmiyorum" en çok duyduğum itiraz. Gerçek şu: bir Linux sunucusuna SSH yapabiliyor, Ollama kurabiliyor, üretim için yapılandırabiliyor, TLS ile reverse proxy kurabiliyorsan ve basit bir izleme scripti yazabiliyorsan — yerel AI dağıtımı hakkında CTO'ların %99'undan daha fazla biliyorsun. Uzmanlık dinleyicine göre görecelidir, mutlak değil. Bir hastane CTO'su AI araştırma makalesi yayınlamış birine ihtiyaç duymuyor. Modellerin donanımlarında güvenli çalışmasını sağlayabilecek birine ihtiyaç duyuyor. Bu sensin.

### Fırsat 3: AI Ajan Şablonları

**Claude Code alt ajanları, özel iş akışları ve otomasyon paketleri.**

**Ne olduğu:** Önceden oluşturulmuş ajan yapılandırmaları, iş akışı şablonları, CLAUDE.md dosyaları, özel komutlar ve AI kodlama araçları için otomasyon paketleri.

**Pazar büyüklüğü:** AI kodlama aracı kullanan her geliştirici potansiyel müşteri. Çoğu bu araçları kapasitelerinin %10-20'sinde kullanıyor çünkü yapılandırmamışlar. "Varsayılan Claude Code" ile "iyi tasarlanmış ajan sistemli Claude Code" arasındaki fark muazzam — ve çoğu insan bu farkın var olduğunu bile bilmiyor.

**Rekabet:** Çok düşük. Ajanlar yeni. Çoğu geliştirici hâlâ temel prompting'i çözmekte. Önceden oluşturulmuş ajan yapılandırmaları pazarı neredeyse yok.

**Giriş zorluğu:** Düşük. Kendi geliştirme sürecin için etkili iş akışları oluşturduysanız, bunları paketleyip satabilirsin. Zor kısım kodlama değil — iyi bir ajan iş akışını neyin oluşturduğunu bilmek.

**Gelir potansiyeli:**

| Ürün Türü | Fiyat Noktası | Hedef Hacim |
|-----------|-------------|-------------|
| Tekli ajan şablonu | $9-19 | 100-300 satış/ay |
| Ajan paketi (5-10 şablon) | $29-49 | 50-150 satış/ay |
| Özel iş akışı tasarımı | $200-500 | 5-10 müşteri/ay |
| "Ajan Mimarisi" kursu | $79-149 | 20-50 satış/ay |
| Kurumsal ajan sistemi | $2,000-10,000 | 1-2 müşteri/çeyrek |

**İnsanların bugün satın alacağı örnek ürünler:**

```markdown
# "Rust Ajan Paketi" — $39

İçerik:
- Kod inceleme ajanı (unsafe bloklar, hata yönetimi, yaşam süresi sorunları kontrol eder)
- Yeniden düzenleme ajanı (yaygın Rust anti-kalıplarını belirler ve düzeltir)
- Test üretim ajanı (uç durumlarla kapsamlı testler yazar)
- Dokümantasyon ajanı (örneklerle rustdoc üretir)
- Performans denetim ajanı (bellek tahsis sıcak noktalarını belirler, sıfır kopyalama alternatifleri önerir)

Her ajan içerir:
- CLAUDE.md kurallar dosyası
- Özel slash komutları
- Örnek iş akışları
- Yapılandırma rehberi
```

```markdown
# "Full-Stack Lansman Kiti" — $49

İçerik:
- Proje iskele ajanı (gereksinimlerden tam proje yapısı üretir)
- API tasarım ajanı (OpenAPI spec çıktısıyla REST/GraphQL API'leri tasarlar)
- Veritabanı göç ajanı (göç dosyaları üretir ve inceler)
- Dağıtım ajanı (Vercel/Railway/Fly.io için CI/CD yapılandırır)
- Güvenlik denetim ajanı (kod tabanına karşı OWASP ilk 10'u kontrol eder)
- Lansman kontrol listesi ajanı (50+ madde üzerinden lansman öncesi doğrulama)
```

**Bu Hafta Başla:**

1. Mevcut Claude Code veya Cursor yapılandırmanı paketle. Hangi CLAUDE.md dosyaları, özel komutlar ve iş akışlarını kullanıyorsan — temizle ve belgele.
2. Basit bir açılış sayfası oluştur (Vercel + şablon, 30 dakika).
3. Gumroad veya Lemon Squeezy'de $19-29'a listele.
4. Geliştiricilerin toplandığı yerlerde paylaş: Twitter/X, Reddit r/ClaudeAI, HN Show, Dev.to.
5. Geri bildirime göre iterasyon yap. Bir hafta içinde v2 teslim et.

### Fırsat 4: Gizlilik Öncelikli SaaS

**AB AI Yasası "local-first"i uyumluluk onay kutusu yaptı.**

**Ne olduğu:** Verileri tamamen kullanıcının makinesinde işleyen, temel işlevsellik için bulut bağımlılığı olmayan yazılım oluşturmak. Masaüstü uygulamalar (Tauri, Electron), local-first web uygulamaları veya kendi kendine barındırılan çözümler.

**Pazar büyüklüğü:** Hassas verileri işleyen VE AI yetenekleri isteyen her şirket. Sadece AB'de bu, düzenlemelerin yeni motive ettiği milyonlarca işletme. ABD'de sağlık (HIPAA), finans (SOC 2/PCI DSS) ve devlet (FedRAMP) benzer baskı yaratıyor.

**Rekabet:** Orta ve büyüyen, ama SaaS ürünlerinin büyük çoğunluğu hâlâ bulut öncelikli. "AI ile local-first" nişi gerçekten küçük. Çoğu geliştirici varsayılan olarak bulut mimarisini kullanıyor çünkü bildikleri bu.

**Giriş zorluğu:** Orta-Yüksek. İyi bir masaüstü uygulaması veya local-first web uygulaması oluşturmak standart SaaS'tan farklı mimari kalıplar gerektirir. Tauri önerilen çerçeve (Rust arka uç, web ön yüz, küçük ikili boyut, Electron şişkinliği yok), ama öğrenme eğrisi var.

**Gelir potansiyeli:**

| Model | Fiyat Noktası | Notlar |
|-------|-------------|-------|
| Tek seferlik masaüstü uygulama | $49-199 | Tekrarlayan gelir yok, ama barındırma maliyeti de yok |
| Yıllık lisans | $79-249/yıl | Tekrarlayan ve algılanan değer dengesi iyi |
| Freemium + Pro | $0 ücretsiz / $9-29/ay Pro | Standart SaaS modeli, ama neredeyse sıfır altyapı maliyetiyle |
| Kurumsal lisans | $499-2,999/yıl | Ekipler için toplu lisanslama |

**Birim ekonomisi olağanüstü:** İşleme kullanıcının makinesinde gerçekleştiğinden, barındırma maliyetlerin sıfıra yakın. $29/ay'lık geleneksel bir SaaS kullanıcı başına $5-10 altyapıya harcayabilir. $29/ay'lık local-first SaaS bir lisans sunucusu ve güncelleme dağıtımı için kullanıcı başına $0.10 harcar. Marjın %60-70 yerine %95+.

**Gerçek örnek:** 4DA (bu kursun parçası olduğu ürün), yerel AI çıkarımı, yerel veritabanı ve yerel dosya işleme çalıştıran bir Tauri masaüstü uygulaması. Kullanıcı başına altyapı maliyeti: fiilen sıfır. $12/ay'lık Signal katmanı neredeyse tamamen marj.

**Bu Hafta Başla:**

Hassas verileri işleyen bulut bağımlı bir araç seç ve local-first alternatifini oluştur. Tamamını değil — en önemli bir özelliği yerelde yapan bir MVP.

Fikirler:
- Local-first toplantı notu transkripsiyonu (Whisper + özetleme modeli)
- AI aramalı özel kod snippet yöneticisi (yerel gömülemeler)
- İK ekipleri için cihaz üzerinde özgeçmiş/belge analizörü
- Muhasebeciler için yerel finansal belge işleyicisi

```bash
# 5 dakikada bir Tauri uygulaması oluştur
pnpm create tauri-app my-private-tool --template react-ts
cd my-private-tool
pnpm install
pnpm run tauri dev
```

### Fırsat 5: "Vibe Kodlama" Eğitimi

**Geliştirici olmayanlara AI ile oluşturmayı öğret — kaliteli rehberlik için çaresizler.**

**Ne olduğu:** Ürün yöneticilerine, tasarımcılara, pazarlamacılara ve girişimcilere AI kodlama araçları kullanarak gerçek uygulamalar oluşturmayı öğreten kurslar, öğreticiler, koçluk ve topluluklar.

**Pazar büyüklüğü:** Muhafazakar tahmin: 2025'te 10-20 milyon geliştirici olmayan AI ile yazılım oluşturmaya çalıştı. Çoğu duvara çarptı. Beceri seviyelerine kalibre edilmiş yardıma ihtiyaçları var — "sıfırdan kodlamayı öğren" değil ve "ileri sistem tasarımı kursu" da değil.

**Rekabet:** Hızla büyüyor, ama kalite şok edici derecede düşük. "Vibe kodlama" eğitiminin çoğu ya:
- Çok sığ: "ChatGPT'ye oluşturmasını söyle!" (Gerçek bir şey gerektiği an çöker.)
- Çok derin: "AI destekli" olarak yeniden etiketlenmiş standart programlama kursları. (Dinleyicileri programlama temelleri öğrenmek istemiyor — belirli bir şey oluşturmak istiyor.)
- Çok dar: 3 ayda eskiyen tek bir araca özel öğretici.

Boşluk; AI'ya gerçek bir araç olarak (sihir değil) yaklaşan ve bilgisayar bilimi diploması gerektirmeden bilinçli kararlar vermeye yetecek kadar programlama bağlamı öğreten yapılandırılmış, pratik içerik için.

**Giriş zorluğu:** Öğretebiliyorsan düşük. Öğretemiyorsan orta (öğretme bir beceridir). Teknik engel neredeyse sıfır — bu şeyleri zaten biliyorsun. Zorluk, geliştirici gibi düşünmeyen insanlara açıklamak.

**Gelir potansiyeli:**

| Ürün | Fiyat | Aylık Potansiyel |
|------|-------|-----------------|
| YouTube kanalı (reklam geliri + sponsorlar) | Ücretsiz içerik | $500-5,000/ay, 10K+ abonede |
| Kendi hızında kurs (Gumroad/Teachable) | $49-149 | $1,000-10,000/ay |
| Kohort tabanlı kurs (canlı) | $299-799 | Kohort başına $5,000-20,000 |
| 1'e 1 koçluk | $100-200/saat | $2,000-4,000/ay (10-20 saat) |
| Topluluk üyeliği | $19-49/ay | $1,000-5,000/ay, 50-100 üyede |

**Bu Hafta Başla:**

1. 10 dakikalık bir ekran kaydı yap: "Claude Code ile sıfırdan çalışan bir uygulama oluştur — kodlama deneyimi gerekmez." Gerçek bir yapım sürecini göster. Taklit etme.
2. YouTube ve Twitter/X'te yayınla.
3. Sonunda tam kurs için bekleme listesine bağlantı ver.
4. Bir haftada 50+ kişi bekleme listesine katılırsa uygulanabilir bir ürünün var. Kursu oluştur.

> **Yaygın Hata:** Eğitimi düşük fiyatlandırma. Geliştiriciler içgüdüsel olarak bilgiyi bedava vermek ister. Ama $149'lık kursunla çalışan bir dahili araç oluşturan geliştirici olmayan, şirketine $20,000'lık geliştirme maliyeti tasarruf ettirdi. Kursun kelepir. Oluşturmak için harcanan saatlere göre değil, teslim edilen değere göre fiyatla.

### Fırsat 6: İnce Ayarlı Model Hizmetleri

**Genel amaçlı modellerin eşleşemediği alan-spesifik AI modelleri.**

**Ne olduğu:** Belirli sektörler veya kullanım durumları için özel ince ayarlı modeller oluşturmak, ardından bunları hizmet olarak (çıkarım API'si) veya dağıtılabilir paketler olarak satmak.

**Pazar büyüklüğü:** Tanım gereği niş, ama nişler tek tek kârlı. Sözleşme diline ince ayar yapılmış modele ihtiyaç duyan hukuk firması, klinik notlar üzerine eğitilmiş modele ihtiyaç duyan sağlık şirketi, düzenleyici dosyalamalar için kalibre edilmiş modele ihtiyaç duyan finans firması — her biri çalışan bir şey için $5,000-50,000 ödeyecek.

**Rekabet:** Spesifik nişlerde düşük, genelde orta. Büyük AI şirketleri bu ölçekte bireysel müşteriler için ince ayar yapmıyor. Fırsat uzun kuyrukta — OpenAI'nin dikkatini hak etmeyen spesifik kullanım durumları için özel modeller.

**Giriş zorluğu:** Orta-Yüksek. İnce ayar iş akışlarını (LoRA, QLoRA), veri hazırlığını, değerlendirme metriklerini ve model dağıtımını anlamak gerekiyor. Ama araçlar önemli ölçüde olgunlaştı — Unsloth, Axolotl ve Hugging Face TRL, tüketici GPU'larında ince ayarı erişilebilir kılıyor.

{? if stack.contains("python") ?}
Python deneyimin burada doğrudan bir avantaj — tüm ince ayar ekosistemi (Unsloth, Transformers, TRL) Python-yerel. Dil öğrenme eğrisini atlayıp doğrudan model eğitimine geçebilirsin.
{? endif ?}

**Gelir potansiyeli:**

| Hizmet | Fiyat | Tekrarlayan mı? |
|--------|------|----------------|
| Özel ince ayar (tek seferlik) | $3,000-15,000 | Hayır, ama sabit ücrete yol açar |
| Model bakım sabit ücreti | $500-2,000/ay | Evet |
| İnce ayarlı model API olarak | $99-499/ay/müşteri | Evet |
| Hizmet olarak ince ayar platformu | $299-999/ay | Evet |

**Bu Hafta Başla:**

1. Veri erişimin olan (veya yasal olarak eğitim verisi elde edebileceğin) bir alan seç.
2. QLoRA ile Llama 3.3 8B modelini belirli bir görevde ince ayar yap:

```bash
# Unsloth'u kur (2026 itibarıyla en hızlı ince ayar kütüphanesi)
pip install unsloth

# Örnek: Müşteri destek verisi üzerine ince ayar
# ~500-2000 (girdi, ideal_çıktı) çifti örneğine ihtiyacın var
# JSONL formatında:
# {"instruction": "Categorize this ticket", "input": "My login isn't working", "output": "category: authentication, priority: high, sentiment: frustrated"}
```

```python
from unsloth import FastLanguageModel

model, tokenizer = FastLanguageModel.from_pretrained(
    model_name="unsloth/llama-3.3-8b-bnb-4bit",
    max_seq_length=2048,
    load_in_4bit=True,
)

model = FastLanguageModel.get_peft_model(
    model,
    r=16,
    target_modules=["q_proj", "k_proj", "v_proj", "o_proj"],
    lora_alpha=16,
    lora_dropout=0,
    bias="none",
    use_gradient_checkpointing="unsloth",
)

# Alan-spesifik verilerinle eğit
# ... (tam eğitim döngüsü için Unsloth dokümantasyonuna bak)

# Ollama için dışa aktar
model.save_pretrained_gguf("my-domain-model", tokenizer, quantization_method="q4_k_m")
```

3. İnce ayarlı modeli temel modelle 50 alan-spesifik test vakasında karşılaştır. İyileşmeyi belgele.
4. Vaka çalışması yaz: "İnce ayarlı 8B model [alan] görev sınıflandırmasında GPT-4o'yu nasıl geçti."

### Fırsat 7: Ölçeklenmiş AI Destekli İçerik

**Niş bültenler, istihbarat raporları ve derlenmiş özetler.**

**Ne olduğu:** Yerel LLM'leri kullanarak alan-spesifik içeriği toplama, sınıflandırma ve özetleme, ardından uzmanlığını ekleyerek premium istihbarat ürünleri oluşturma.

**Pazar büyüklüğü:** Her sektörde bilgi selinde boğulan profesyoneller var. Geliştiriciler, avukatlar, doktorlar, araştırmacılar, yatırımcılar, ürün yöneticileri — hepsi derlenmiş, ilgili, zamanında istihbarata ihtiyaç duyuyor. Genel bültenler doymuş. Niş olanlar doymuş değil.

**Rekabet:** Geniş teknoloji bültenleri için orta. Derin nişler için düşük. İyi bir "Rust + AI" haftalık istihbarat raporu yok. "Yerel AI Dağıtımı" aylık özeti yok. CTO'lar için "Privacy Engineering" özeti yok. Bu nişler bekliyor.

**Giriş zorluğu:** Düşük. En zor kısım tutarlılık, teknoloji değil. Yerel LLM küratörlük işinin %80'ini halleder. Sen, zevk gerektiren %20'yi hallediyorsun.

**Gelir potansiyeli:**

| Model | Fiyat | $3K/ay İçin Abone |
|-------|------|-------------------|
| Ücretsiz bülten + ücretli premium | $7-15/ay premium | 200-430 ücretli abone |
| Sadece ücretli bülten | $10-20/ay | 150-300 abone |
| İstihbarat raporu (aylık) | $29-99/rapor | 30-100 alıcı |
| Sponsorlu ücretsiz bülten | $200-2,000/sayı | 5,000+ ücretsiz abone |

**Üretim hattı (haftalık bülteni 3-4 saatte nasıl üretirsin):**

```python
#!/usr/bin/env python3
"""
newsletter_pipeline.py
Niş bülten için otomatik istihbarat toplama.
Sınıflandırma ve özetleme için yerel LLM kullanır.
"""

import requests
import json
import feedparser
from datetime import datetime, timedelta

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "qwen2.5:14b"  # Hız ve kalite dengesi iyi

# Derlenmiş kaynak listen (10 yüksek sinyalli kaynak > 100 gürültülü)
SOURCES = [
    {"type": "rss", "url": "https://hnrss.org/newest?q=local+AI+OR+ollama+OR+llama.cpp", "name": "HN Local AI"},
    {"type": "rss", "url": "https://www.reddit.com/r/LocalLLaMA/.rss", "name": "r/LocalLLaMA"},
    # Nişine özel kaynakları buraya ekle
]

def classify_relevance(title: str, summary: str, niche: str) -> dict:
    """Bir öğenin nişine ilgi düzeyini sınıflandırmak için yerel LLM kullan."""
    prompt = f"""You are a content curator for a newsletter about {niche}.

Rate this item's relevance (1-10) and explain in one sentence why.
If relevance >= 7, write a 2-sentence summary suitable for a newsletter.

Title: {title}
Content: {summary[:500]}

Respond in JSON: {{"relevance": N, "reason": "...", "summary": "..." or null}}"""

    response = requests.post(OLLAMA_URL, json={
        "model": MODEL,
        "prompt": prompt,
        "stream": False,
        "format": "json",
        "options": {"temperature": 0.3}
    }, timeout=60)

    try:
        return json.loads(response.json()["response"])
    except (json.JSONDecodeError, KeyError):
        return {"relevance": 0, "reason": "parse error", "summary": None}

def gather_and_classify(niche: str, min_relevance: int = 7):
    """Tüm kaynaklardan öğeleri topla ve sınıflandır."""
    items = []

    for source in SOURCES:
        if source["type"] == "rss":
            feed = feedparser.parse(source["url"])
            for entry in feed.entries[:20]:  # Kaynak başına son 20 öğe
                classification = classify_relevance(
                    entry.get("title", ""),
                    entry.get("summary", ""),
                    niche
                )
                if classification.get("relevance", 0) >= min_relevance:
                    items.append({
                        "title": entry.get("title"),
                        "link": entry.get("link"),
                        "source": source["name"],
                        "relevance": classification["relevance"],
                        "summary": classification["summary"],
                        "classified_at": datetime.now().isoformat()
                    })

    # İlgi düzeyine göre sırala, ilk 10'u al
    items.sort(key=lambda x: x["relevance"], reverse=True)
    return items[:10]

if __name__ == "__main__":
    # Örnek: "Yerel AI Dağıtımı" nişi
    results = gather_and_classify("local AI deployment and privacy-first infrastructure")

    print(f"\n{'='*60}")
    print(f"Bu haftanın bülteni için en iyi {len(results)} öğe:")
    print(f"{'='*60}\n")

    for i, item in enumerate(results, 1):
        print(f"{i}. [{item['relevance']}/10] {item['title']}")
        print(f"   Kaynak: {item['source']}")
        print(f"   {item['summary']}")
        print(f"   {item['link']}\n")

    # Dosyaya kaydet — bunu bültenine düzenleyeceksin
    with open("newsletter_draft.json", "w") as f:
        json.dump(results, f, indent=2)

    print(f"Taslak newsletter_draft.json'a kaydedildi")
    print(f"Senin işin: bunları incele, analizini ekle, giriş yaz.")
    print(f"Tahmini tamamlama süresi: 2-3 saat.")
```

**Bu Hafta Başla:**

1. Nişini seç. 10 yüksek sinyalli kaynak adlandırabilecek kadar spesifik ve her hafta yeni bir hikaye olacak kadar geniş olmalı.
2. Yukarıdaki hattı (veya benzerini) bir hafta çalıştır.
3. "1. Hafta" bülteni yaz. Nişteki 10 tanıdığına gönder. Sor: "Bunun için ayda $10 öder misin?"
4. 3+ evet derse — Buttondown veya Substack'te yayınla. İlk günden ücret al.

> **Açık Konuşalım:** Bir bültenin en zor kısmı yazmak değil — devam etmek. Çoğu bülten 4. sayı ile 12. sayı arasında ölür. Yukarıdaki hat, üretimi sürdürülebilir kılmak için var. İçerik toplama 3 saat yerine 30 dakika sürüyorsa, tutarlı teslim etme olasılığın çok daha yüksek. Angarya için LLM'i kullan. Enerjini içgörü için sakla.

### Sıra Sende

{@ mirror radar_momentum @}

1. **Fırsatları sırala.** Yukarıdaki yedi fırsatı SENİN durumun için en çekiciden en az çekiciye sırala. Becerilerini, donanımını, mevcut zamanını ve risk toleransını dikkate al.
{? if radar.adopt ?}
Mevcut radarınla karşılaştır: zaten {= radar.adopt | fallback("benimseme halkandaki teknolojileri") =} takip ediyorsun. Bu yedi fırsattan hangisi zaten yatırım yaptığın şeyle uyumlu?
{? endif ?}
2. **Birini seç.** Üç değil, "hepsini eninde sonunda" değil. Birini. Bu hafta başlayacağın.
3. **"Bu Hafta Başla" eylem planını tamamla.** Yukarıdaki her fırsatın somut bir ilk hafta planı var. Yap. Pazar gününe kadar bir şey yayınla.
4. **30 günlük kontrol noktası belirle.** Seçtiğin fırsat için 30 gün sonra "başarının" nasıl göründüğünü yaz. Spesifik ol: gelir hedefi, kullanıcı sayısı, yayınlanan içerik, iletişim kurulan müşteriler.

---

## Ders 3: Piyasa Zamanlaması — Ne Zaman Girilir, Ne Zaman Çıkılır

*"Doğru fırsatı yanlış zamanda seçmek, yanlış fırsatı seçmekle aynı şeydir."*

### Geliştirici Teknoloji Benimseme Eğrisi

Her teknoloji öngörülebilir bir döngüden geçer. Bir teknolojinin bu eğrideki konumunu anlamak, ne tür para kazanılabileceğini ve ne kadar rekabetle karşılaşacağını söyler.

```
  İnovasyon        Erken            Büyüme          Olgunluk        Düşüş
  Tetikleyici      Benimseme        Aşaması         Aşaması         Aşaması
     |               |               |               |               |
  "İlginç          "Bazı            "Herkes          "Kurumsal      "Eski,
   makale/demo     geliştiriciler   kullanıyor       standart.       yeniyle
   bir konf'ta"    gerçek iş        veya             Sıkıcı."       değiştiriliyor"
                   için kullanıyor" değerlendiriyor"

  Gelir:           Gelir:           Gelir:           Gelir:          Gelir:
  $0 (çok erken)  YÜKSEK marjlar   Hacim oyunu,     Metalaşmış,     Sadece
                  Düşük rekabet    marjlar düşer    düşük marjlar   bakım
                  İlk-hareket      Rekabet          Büyük oyuncular Niş oyuncular
                  avantajı         artıyor          hükmediyor      hayatta kalıyor
```

**Her 2026 fırsatı nerede:**

| Fırsat | Aşama | Zamanlama |
|--------|-------|----------|
| MCP sunucuları/pazaryeri | Erken Benimseme → Büyüme | Tatlı nokta. Şimdi hareket et. |
| Yerel AI danışmanlığı | Erken Benimseme | Mükemmel zamanlama. Talep arzı 10:1 aşıyor. |
| AI ajan şablonları | İnovasyon → Erken Benimseme | Çok erken. Yüksek risk, yüksek potansiyel. |
| Gizlilik öncelikli SaaS | Erken Benimseme → Büyüme | İyi zamanlama. Düzenleyici baskı benimsemeyi hızlandırıyor. |
| Vibe kodlama eğitimi | Büyüme | Rekabet artıyor. Kalite farklılaştırıcı. |
| İnce ayarlı model hizmetleri | Erken Benimseme | Teknik engel rekabeti düşük tutuyor. |
| AI destekli içerik | Büyüme | Kanıtlanmış model. Niş seçimi her şey. |

### "Çok Erken / Tam Zamanında / Çok Geç" Çerçevesi

Herhangi bir fırsat için üç soru sor:

**Çok erken miyim?**
- BUGÜN bunun için ödeme yapacak bir müşteri var mı? ("Teoride isteyebilir" değil.)
- Bu ay bunu inşa etsem, ödeme yapacak 10 kişi bulabilir miyim?
- Altta yatan teknoloji, her çeyrekte yeniden yazmadan üzerine inşa edecek kadar kararlı mı?

Herhangi bir cevap "hayır"sa — çok erkensin. Bekle, ama yakından izle.

**Tam zamanında mıyım?**
- Talep mevcut ve büyüyor (sadece sabit değil)
- Arz yetersiz (az rakip veya rakipler düşük kalitede)
- Teknoloji üzerine inşa edecek kadar kararlı
- Erken girenler henüz dağıtımı kilitlemediler
- 2-4 haftada MVP teslim edebilirsin

Hepsi doğruysa — hızlı hareket et. Bu pencere.

**Çok geç miyim?**
- İyi fonlanmış startup'lar alana girdi
- Platform sağlayıcıları yerel çözümler inşa ediyor
- Fiyatlandırma dibe doğru yarışıyor
- "En iyi uygulamalar" iyi yerleşmiş (farklılaşma alanı yok)
- Bir meta ürün inşa ediyor olurdun

Herhangi biri doğruysa — fırsatın *içinde henüz metalaşmamış bir niş* ara veya tamamen geç.

### Sinyalleri Okuma: Bir Pazarın Açıldığını Nasıl Anlarsın

Geleceği tahmin etmene gerek yok. Şimdiki zamanı doğru okumanı gerekiyor. İzlenecekler:

**Sinyal 1: Hacker News Ana Sayfa Sıklığı**

Bir teknoloji HN ana sayfasında aylık yerine haftalık görünmeye başladığında dikkat kayıyor. HN yorumları "bu nedir?" den "bunu nasıl kullanırım?"a kaydığında, 3-6 ay içinde para takip eder.

```bash
# Algolia API ile hızlı HN sinyal kontrolü
curl -s "https://hn.algolia.com/api/v1/search?query=MCP+server&tags=story&hitsPerPage=5" \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
for hit in data.get('hits', []):
    print(f\"{hit.get('points', 0):4d} pts | {hit.get('created_at', '')[:10]} | {hit.get('title', '')}\")
"
```

**Sinyal 2: GitHub Yıldız Hızı**

Mutlak yıldız sayısı önemli değil. Hız önemli. 3 ayda 0'dan 5,000 yıldıza çıkan repo, 2 yıldır 50,000 yıldızda duran repodan daha güçlü sinyal.

**Sinyal 3: İş İlanı Büyümesi**

Şirketler bir teknoloji için işe almaya başladığında, bütçe ayırıyorlardır. İş ilanları benimsenmenin gecikmeli göstergesi ama kurumsal harcamanın öncü göstergesi.

**Sinyal 4: Konferans Konuşma Kabul Oranları**

Konferans CFP'leri bir teknoloji hakkında konuşmaları kabul etmeye başladığında, nişten ana akıma geçiyor. Konferanslar bunun için *özel izler* oluşturduğunda, kurumsal benimseme kaçınılmaz.

### Sinyalleri Okuma: Bir Pazarın Kapandığını Nasıl Anlarsın

Bu daha zor. Kimse geç kaldığını kabul etmek istemez. Ama bu sinyaller güvenilir.

**Sinyal 1: Kurumsal Benimseme**

Gartner bir teknoloji için Magic Quadrant yazdığında, erken hareket penceresi kapanmıştır. Büyük danışmanlık firmaları (Deloitte, Accenture, McKinsey) hakkında rapor yazıyorsa, metalaşma 12-18 ay uzakta.

**Sinyal 2: Yatırım Turları**

Alanındaki bir rakip $10M+ topladığında, benzer koşullarda rekabet penceren kapanır. Pazarlama, işe alım ve özelliklerde seni geçecekler. Hamlen niş konumlandırmaya veya çıkışa kayar.

**Sinyal 3: Platform Entegrasyonu**

Platform bunu yerel olarak inşa ettiğinde, üçüncü taraf çözümünün günleri sayılıdır. Örnekler:
- GitHub Copilot'u yerel olarak eklediğinde, bağımsız kod tamamlama araçları öldü.
- VS Code yerleşik terminal yönetimi eklediğinde, terminal eklentileri ilgisini kaybetti.
- Vercel yerel AI özellikleri eklediğinde, Vercel üzerine inşa edilmiş bazı AI sarmalayıcı ürünler gereksiz hale gelecek.

Platform duyurularını izle. Üzerine inşa ettiğin platform özelliğini inşa ettiğini duyurduğunda, farklılaşmak veya yön değiştirmek için 6-12 ayın var.

### Gerçek Tarihsel Örnekler

| Yıl | Fırsat | Pencere | Ne Oldu |
|-----|--------|---------|---------|
| 2015 | Docker araçları | 18 ay | İlk girenler izleme ve orkestrasyon araçları inşa etti. Sonra Kubernetes geldi ve çoğunu yuttu. Hayatta kalanlar: özel nişler (güvenlik taraması, görüntü optimizasyonu). |
| 2017 | React bileşen kütüphaneleri | 24 ay | Material UI, Ant Design, Chakra UI büyük pazar payı yakaladı. Geç girenler zorlandı. Mevcut kazananlar 2019'a kadar hepsi kurulmuştu. |
| 2019 | Kubernetes operatörleri | 12-18 ay | Erken operatör yapıcıları satın alındı veya standart oldu. 2021'e kadar alan kalabalıktı. |
| 2023 | AI sarmalayıcıları (GPT sarmalayıcıları) | 6 ay | Geliştirici araçları tarihinin en hızlı yükseliş-çöküşü. Binlerce GPT sarmalayıcı piyasaya çıktı. OpenAI kendi UX ve API'lerini geliştirince çoğu 6 ay içinde öldü. Hayatta kalanlar: gerçek tescilli verisi veya iş akışı olanlar. |
| 2024 | Prompt pazaryerleri | 3 ay | PromptBase ve diğerleri fırladı ve çöktü. Promptların kopyalanması çok kolay çıktı. Sıfır savunulabilirlik. |
| 2025 | AI kodlama aracı eklentileri | 12 ay | Cursor/Copilot için uzantı ekosistemleri hızla büyüdü. Erken girenler dağıtım kazandı. Pencere daralıyor. |
| 2026 | MCP araçları + yerel AI hizmetleri | ? ay | Buradasın. Pencere açık. Ne kadar açık kalacağı, büyük oyuncuların ne kadar hızlı pazaryerleri kurup dağıtımı metalaştıracağına bağlı. |

**Kalıp:** Geliştirici araç pencereleri ortalama 12-24 ay sürer. AI ile ilgili pencereler daha kısa (6-12 ay) çünkü değişim hızı daha yüksek. MCP penceresi muhtemelen bugünden itibaren 12-18 ay. Bundan sonra pazaryeri altyapısı olacak, erken kazananlar dağıtıma sahip olacak ve giriş önemli ölçüde daha fazla çaba gerektirecek.

{@ temporal market_timing @}

### Karar Çerçevesi

Herhangi bir fırsatı değerlendirirken bunu kullan:

```
1. Bu teknoloji benimseme eğrisinde nerede?
   [ ] İnovasyon → Çok erken (riski sevmiyorsan)
   [ ] Erken Benimseme → Bağımsız geliştiriciler için en iyi pencere
   [ ] Büyüme → Hâlâ uygulanabilir ama farklılaşma gerekli
   [ ] Olgunluk → Meta. Fiyatla rekabet et veya çık.
   [ ] Düşüş → Sadece zaten içindeysen ve kârlıysan

2. Öncü sinyaller ne diyor?
   HN sıklığı:     [ ] Artıyor  [ ] Sabit  [ ] Düşüyor
   GitHub hızı:     [ ] Artıyor  [ ] Sabit  [ ] Düşüyor
   İş ilanları:     [ ] Artıyor  [ ] Sabit  [ ] Düşüyor
   Yatırım:         [ ] Yok      [ ] Tohum  [ ] Seri A+  [ ] Geç aşama

3. Dürüst giriş zorluğum ne?
   [ ] Bu ay MVP teslim edebilirim
   [ ] Bu çeyrekte MVP teslim edebilirim
   [ ] 6+ ay sürer (muhtemelen çok yavaş)

4. Karar:
   [ ] Şimdi gir (sinyaller güçlü, zamanlama doğru, hızlı teslim edebilirim)
   [ ] İzle ve hazırlan (sinyaller karışık, beceri/prototip oluştur)
   [ ] Geç (çok erken, çok geç veya mevcut durum için çok zor)
```

> **Yaygın Hata:** Analiz felci — zamanlamayı değerlendirmekle o kadar uzun zaman harcamak ki pencere sen hâlâ değerlendirirken kapanıyor. Yukarıdaki çerçeve fırsat başına 15 dakika sürmeli. 15 dakikada karar veremiyorsan yeterli bilgin yok. Git bir prototip inşa et ve gerçek piyasa geri bildirimi al.

### Sıra Sende

1. **Seçtiğin fırsatı değerlendir** Ders 2'den, yukarıdaki karar çerçevesini kullanarak. Zamanlama konusunda dürüst ol.
2. **Seçtiğin alan için HN sinyalini kontrol et.** Yukarıdaki API sorgusunu çalıştır (veya manuel ara). Sıklık ve duygu ne?
3. **Seçtiğin pazar için haftalık izleyeceğin bir sinyal kaynağı belirle.** Takvim hatırlatıcısı koy: "Her Pazartesi sabahı [sinyali] kontrol et."
4. **Zamanlama tezini yaz.** 3 cümlede: Fırsatın için neden şimdi doğru zaman? Seni haksız çıkaracak ne olurdu? Bahsini ikiye katlamanı sağlayacak ne olurdu?

---

## Ders 4: İstihbarat Sistemini Kurma

*"Sinyali ilk gören geliştirici, ilk para kazanan geliştiricidir."*

### Çoğu Geliştirici Fırsatları Neden Kaçırır

Bilgi aşırı yüklemesi sorun değil. Bilgi *düzensizliği* sorun.

2026'da ortalama geliştirici şunlara maruz kalır:
- Günde 50-100 Hacker News hikayesi
- Takip ettiği kişilerden 200+ tweet
- Haftada 10-30 bülten e-postası
- Aynı anda 5-15 Slack/Discord konuşması
- Düzinelerce GitHub bildirimi
- Çeşitli blog yazıları, YouTube videoları, podcast bahisleri

Toplam girdi: haftada binlerce sinyal. Gelir kararları için gerçekten önemli olan sayı: belki 3-5.

Daha fazla bilgiye ihtiyacın yok. Bir filtreye ihtiyacın var. Binlerce girdiyi bir avuç eyleme dönüştürülebilir sinyale indiren bir istihbarat sistemi.

### "10 Yüksek Sinyalli Kaynak" Yaklaşımı

100 gürültülü kanalı izlemek yerine, 10 yüksek sinyalli kaynak seç ve iyi izle.

**Yüksek sinyalli kaynak kriterleri:**
1. Gelir nişinle ilgili içerik üretir
2. Şeyleri erken yüzeye çıkarma sicili var (sadece eski haberleri toplamak değil)
3. Oturum başına 5 dakikadan kısa sürede tüketilebilir
4. Otomatikleştirilebilir (RSS beslemesi, API veya yapılandırılmış format)

**Örnek: "Yerel AI + Gizlilik" istihbarat yığını:**

```yaml
# intelligence-sources.yml
# 10 yüksek sinyalli kaynağın — haftalık incele

sources:
  # Seviye 1: Birincil sinyaller (günlük kontrol et)
  - name: "HN — Yerel AI filtresi"
    url: "https://hnrss.org/newest?q=local+AI+OR+ollama+OR+llama.cpp+OR+private+AI&points=30"
    frequency: daily
    signal: "Geliştiriciler ne inşa ediyor ve tartışıyor"

  - name: "r/LocalLLaMA"
    url: "https://www.reddit.com/r/LocalLLaMA/top/.rss?t=week"
    frequency: daily
    signal: "Model sürümleri, karşılaştırmalar, üretim kullanım durumları"

  - name: "r/selfhosted"
    url: "https://www.reddit.com/r/selfhosted/top/.rss?t=week"
    frequency: daily
    signal: "İnsanlar yerelde ne çalıştırmak istiyor (talep sinyalleri)"

  # Seviye 2: Ekosistem sinyalleri (haftada iki kez kontrol et)
  - name: "GitHub Trending — Rust"
    url: "https://github.com/trending/rust?since=weekly"
    frequency: twice_weekly
    signal: "İvme kazanan yeni araçlar ve kütüphaneler"

  - name: "GitHub Trending — TypeScript"
    url: "https://github.com/trending/typescript?since=weekly"
    frequency: twice_weekly
    signal: "Frontend ve araç trendleri"

  - name: "Ollama Blog + Sürümler"
    url: "https://ollama.com/blog"
    frequency: twice_weekly
    signal: "Model ve altyapı güncellemeleri"

  # Seviye 3: Piyasa sinyalleri (haftalık kontrol et)
  - name: "Simon Willison'ın Blogu"
    url: "https://simonwillison.net/atom/everything/"
    frequency: weekly
    signal: "AI araçları ve trendlerinin uzman analizi"

  - name: "Changelog News"
    url: "https://changelog.com/news/feed"
    frequency: weekly
    signal: "Derlenmiş geliştirici ekosistemi haberleri"

  - name: "TLDR AI Newsletter"
    url: "https://tldr.tech/ai"
    frequency: weekly
    signal: "AI sektörü genel bakış"

  # Seviye 4: Derin sinyaller (aylık kontrol et)
  - name: "AB AI Yasası Güncellemeleri"
    url: "https://artificialintelligenceact.eu/"
    frequency: monthly
    signal: "Gizlilik-öncelikli talep üzerindeki düzenleyici değişiklikler"
```

### İstihbarat Yığınını Kurma

**Katman 1: Otomatik Toplama (4DA)**

{? if settings.has_llm ?}
4DA'yı {= settings.llm_provider | fallback("LLM sağlayıcın") =} ile kullanıyorsan, bu zaten halledilmiş. 4DA yapılandırılabilir kaynaklardan toplar, {= settings.llm_model | fallback("yapılandırılmış modelin") =} kullanarak Developer DNA'na göre ilgi düzeyine göre sınıflandırır ve en yüksek sinyalli öğeleri günlük brifinginde yüzeye çıkarır.
{? else ?}
4DA kullanıyorsan, bu zaten halledilmiş. 4DA yapılandırılabilir kaynaklardan toplar, Developer DNA'na göre ilgi düzeyine göre sınıflandırır ve en yüksek sinyalli öğeleri günlük brifinginde yüzeye çıkarır. AI destekli sınıflandırma için ayarlarda bir LLM sağlayıcısı yapılandır — yerel modelli Ollama bunun için mükemmel çalışır.
{? endif ?}

**Katman 2: Geri Kalan Her Şey İçin RSS**

4DA'nın kapsamadığı kaynaklar için RSS kullan. Her ciddi istihbarat operasyonu RSS üzerinde çalışır çünkü yapılandırılmış, otomatik ve bir algoritmanın neyi göreceğine karar vermesine bağlı değil.

```bash
# Hızlı tarama için komut satırı RSS okuyucu kur
# Seçenek 1: newsboat (Linux/Mac)
# sudo apt install newsboat   # Linux
# brew install newsboat        # macOS

# Seçenek 2: Web tabanlı okuyucu kullan
# Miniflux (kendi sunucunda barındırılan, gizliliğe saygılı) — https://miniflux.app
# Feedbin ($5/ay, mükemmel) — https://feedbin.com
# Inoreader (ücretsiz katman) — https://www.inoreader.com
```

```bash
# newsboat yapılandırma örneği
# ~/.newsboat/urls olarak kaydet

# Birincil sinyaller
https://hnrss.org/newest?q=MCP+server&points=20 "~HN: MCP Servers"
https://hnrss.org/newest?q=local+AI+OR+ollama&points=30 "~HN: Local AI"
https://www.reddit.com/r/LocalLLaMA/top/.rss?t=week "~Reddit: LocalLLaMA"

# Ekosistem sinyalleri
https://simonwillison.net/atom/everything/ "~Simon Willison"
https://changelog.com/news/feed "~Changelog"

# Nişin (bunları özelleştir)
# [Alan-spesifik RSS beslemelerini buraya ekle]
```

**Katman 3: Twitter/X Listeleri (Derlenmiş)**

Ana beslemende insanları takip etme. Nişindeki 20-30 düşünce liderinin özel listesini oluştur. Beslemeyi değil listeyi kontrol et.

**Etkili liste oluşturma:**
1. İçeriğini tutarlı olarak değerli bulduğun 5 kişiyle başla
2. Kimi retweetlediklerine ve kimlerle etkileşime girdiklerine bak
3. O kişileri ekle
4. %50'den fazla görüş/sıcak yorum paylaşanları çıkar (sinyal istiyorsun, yorum değil)
5. Hedef: bilgiyi erken yüzeye çıkaran 20-30 hesap

**Katman 4: GitHub Trending (Haftalık)**

GitHub Trending'i günlük değil haftalık kontrol et. Günlük gürültü. Haftalık, sürdürülebilir ivmeye sahip projeleri yüzeye çıkarır.

```bash
# Dillerindeki trende giren GitHub repolarını kontrol eden script
# check_trending.sh olarak kaydet

#!/bin/bash
echo "=== Bu Hafta GitHub Trending ==="
echo ""
echo "--- Rust ---"
curl -s "https://api.github.com/search/repositories?q=created:>$(date -d '7 days ago' +%Y-%m-%d)+language:rust&sort=stars&order=desc&per_page=5" \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
for repo in data.get('items', []):
    print(f\"  ★ {repo['stargazers_count']:>5} | {repo['full_name']}: {repo.get('description', 'No description')[:80]}\")
"

echo ""
echo "--- TypeScript ---"
curl -s "https://api.github.com/search/repositories?q=created:>$(date -d '7 days ago' +%Y-%m-%d)+language:typescript&sort=stars&order=desc&per_page=5" \
  | python3 -c "
import sys, json
data = json.load(sys.stdin)
for repo in data.get('items', []):
    print(f\"  ★ {repo['stargazers_count']:>5} | {repo['full_name']}: {repo.get('description', 'No description')[:80]}\")
"
```

### 15 Dakikalık Sabah Taraması

Bu rutin. Her sabah. 15 dakika. 60 değil. "Vaktim olunca" değil. On beş dakika, zamanlayıcıyla.

```
Dakika 0-3:   4DA panosunu (veya RSS okuyucuyu) gece sinyalleri için kontrol et
Dakika 3-6:   Twitter/X listesini tara (ana besle DEĞİL) — sadece başlıklara göz at
Dakika 6-9:   GitHub Trending (haftalık) veya HN ana sayfa (günlük) kontrol et
Dakika 9-12:  Herhangi bir sinyal ilginçse, yer imi ekle (şimdi okuma)
Dakika 12-15: İstihbarat günlüğüne BİR gözlem yaz

Bu kadar. Her şeyi kapat. Gerçek işine başla.
```

**İstihbarat günlüğü:**

Basit bir dosya tut. Tarih ve bir gözlem. Hepsi bu.

```markdown
# İstihbarat Günlüğü — 2026

## Şubat

### 2026-02-17
- Playwright testi için MCP sunucusu HN ana sayfasında göründü (400+ puan).
  MCP aracılığıyla test otomasyonu ısınıyor. Ajan şablonlarım bunu hedefleyebilir.

### 2026-02-14
- r/LocalLLaMA'da M4 Max (128GB) üzerinde 25 tok/s'de Qwen 2.5 72B çalıştırma yazısı.
  Apple Silicon ciddi bir yerel AI platformu oluyor. Mac odaklı danışmanlık?

### 2026-02-12
- AB AI Yasası şeffaflık yükümlülükleri artık yürürlükte. LinkedIn, uyumluluk
  telaşları hakkında yazan CTO'larla dolu. Yerel AI danışmanlık talebinde artış geliyor.
```

30 gün sonra günlüğü incele. Gerçek zamanda göremediğin kalıplar ortaya çıkacak.

### İstihbaratı Eyleme Dönüştürme: Sinyal → Fırsat → Karar Hattı

Çoğu geliştirici istihbarat toplar ve sonra hiçbir şey yapmaz. HN okurlar, başlarını sallarlar ve işlerine dönerler. Bu eğlence, istihbarat değil.

Sinyali paraya nasıl dönüştüreceğin:

```
SİNYAL (ham bilgi)
  ↓
  Filtre: Ders 2'deki 7 fırsattan herhangi biriyle ilgili mi?
  Hayırsa → at
  Evetse ↓

FIRSAT (filtrelenmiş sinyal + bağlam)
  ↓
  Değerlendir: Ders 3'teki zamanlama çerçevesini kullanarak
  - Çok erken mi? → yer imi ekle, 30 gün sonra tekrar kontrol et
  - Tam zamanında mı? ↓
  - Çok geç mi? → at

KARAR (eyleme dönüştürülebilir taahhüt)
  ↓
  Birini seç:
  a) ŞİMDİ HAREKETE GEÇ — bu hafta inşa etmeye başla
  b) HAZIRLAN — beceri/prototip oluştur, gelecek ay harekete geç
  c) İZLE — istihbarat günlüğüne ekle, 90 gün sonra yeniden değerlendir
  d) GEÇ — benim için değil, eylem gerekli değil
```

Anahtar, kararı açıkça vermek. "Bu ilginç" bir karar değil. "Bu hafta sonu Playwright testi için bir MCP sunucusu inşa edeceğim" bir karar. "30 gün boyunca MCP test araçlarını izleyeceğim ve 15 Mart'ta girip girmeyeceğime karar vereceğim" de bir karar. "Becerilerime uymadığı için atlıyorum" bile bir karar.

Kararsız kalınan öğeler zihinsel hattını tıkar. Karar ver, karar beklemek olsa bile.

### Sıra Sende

1. **Kaynak listeni oluştur.** Yukarıdaki şablonu kullanarak 10 yüksek sinyalli kaynağını listele. Spesifik ol — tam URL'ler, "tekno Twitter'ı takip et" değil.
2. **Altyapını kur.** Kaynaklarınla bir RSS okuyucu kur (veya 4DA'yı yapılandır). Bu 30 dakika sürmeli, bir hafta sonu değil.
3. **İstihbarat günlüğünü başlat.** Dosyayı oluştur. Bugünkü ilk girişi yaz. 15 dakikalık sabah taraması için günlük hatırlatıcı koy.
4. **Bir sinyali hattan geçir.** Bu hafta teknik haberlerde gördüğün bir şey al. Sinyal → Fırsat → Karar hattından geçir. Açık kararı yaz.
5. **İlk 30 günlük incelemeyi planla.** Takvimine koy: 30 gün sonra istihbarat günlüğünü incele, kalıpları belirle.

---

## Ders 5: Gelirini Geleceğe Karşı Koruma

*"Bir beceriyi öğrenmenin en iyi zamanı, piyasa bunun için ödeme yapmaya başlamadan 12 ay öncesidir."*

### 12 Aylık Beceri Öncülüğü

Bugün para kazandığın her beceriyi 1-3 yıl önce öğrendin. Bu gecikme. 2027'de sana para ödeyecek beceriler, şimdi öğrenmeye başladıkların.

Bu her trendi kovalamak anlamına gelmiyor. Açıkça pazarlanabilir hale gelmeden önce öğrenme zamanı yatırdığın küçük bir "bahis" portföyü tutmak anlamına geliyor.

2020'de Rust öğrenen geliştiriciler, 2026'da Rust danışmanlığı için $250-400/saat alan geliştiriciler. 2017'de Kubernetes öğrenenler, 2019-2022'de prim oranlar sunanlar. Kalıp tekrarlanıyor.

Soru: 2027-2028'de piyasanın ödeyeceği ŞİMDİ ne öğrenmelisin?

### 2027'de Muhtemelen Önemli Olacaklar (Eğitimli Tahminler)

Bunlar tahmin değil — gerçek kanıtlara dayanan mevcut yörüngelerin ekstrapolasyonları.

#### Tahmin 1: Cihaz Üzerinde AI (Telefonlar ve Tabletler Hesaplama Düğümleri Olarak)

Apple Intelligence 2024-2025'te sınırlı yeteneklerle çıktı. Qualcomm'un Snapdragon X Elite'i dizüstü bilgisayarlara 45 TOPS AI hesaplama koydu. Samsung ve Google telefonlara cihaz üzerinde çıkarım ekliyor.

2027'ye kadar bekle:
- Amiral gemisi telefonlarda kullanılabilir hızlarda çalışan 3B-7B modeller
- Standart bir OS özelliği olarak cihaz üzerinde AI (uygulama değil)
- Bir sunucuya hiç iletişim kurmadan hassas verileri işleyen yeni uygulama kategorileri

**Gelir etkisi:** Buluta gönderilemeyen verileri (sağlık verileri, finansal veriler, kişisel fotoğraflar) cihaz üzerinde çıkarımla işleyen uygulamalar. Geliştirme becerileri: mobil ML dağıtımı, model kuantizasyonu, cihaz üzerinde optimizasyon.

**Şimdiki öğrenme yatırımı:** Apple Core ML veya Google ML Kit'i öğren. Mobil hedefler için llama.cpp ile model kuantizasyonunu anlamak için 20 saat harca. Bu uzmanlık 18 ay içinde nadir ve değerli olacak.

#### Tahmin 2: Ajan-Ajan Ticareti

MCP insanların AI ajanlarını araçlara bağlamasını sağlıyor. Sonraki adım ajanların DİĞER ajanlara bağlanması. Hukuki analize ihtiyaç duyan ajan bir hukuk analiz ajanını çağırır. Web sitesi kuran ajan bir tasarım ajanını çağırır. Mikro hizmet olarak ajanlar.

2027'ye kadar bekle:
- Ajan-ajan keşif ve çağırma için standartlaşmış protokoller
- Ajan-ajan işlemleri için faturalandırma mekanizmaları
- Ajanının diğer ajanlara hizmet vererek para kazanabileceği bir pazaryeri

**Gelir etkisi:** Değerli bir hizmet sunan bir ajan inşa edersen, diğer ajanlar müşterilerin olabilir — sadece insanlar değil. Bu, en literal anlamda pasif gelir.

**Şimdiki öğrenme yatırımı:** MCP'yi derinlemesine anla (sadece "sunucu nasıl inşa edilir" değil, protokol spesifikasyonu). Temiz, birleştirilebilir arayüzler sunan ajanlar inşa et. API tasarımı düşün, ama AI tüketicileri için.

#### Tahmin 3: Merkeziyetsiz AI Pazaryerleri

Geliştiricilerin yedek GPU hesaplamasını sattığı eşler arası çıkarım ağları konseptten erken uygulamaya geçiyor. Petals, Exo ve çeşitli blokzincir tabanlı çıkarım ağları bunun altyapısını inşa ediyor.

2027'ye kadar bekle:
- GPU hesaplama satışı için en az bir ana akım ağ
- Kolay katılım için araçlar (sadece kripto meraklıları için değil)
- Gelir potansiyeli: boşta GPU zamanından $50-500/ay

**Gelir etkisi:** GPU'n sen uyurken, herhangi bir spesifik hizmet çalıştırmadan para kazanabilir. Sadece bir ağa hesaplama katkısı yapıp ödeme alırsın.

**Şimdiki öğrenme yatırımı:** Petals veya Exo düğümü çalıştır. Ekonomisini anla. Altyapı olgunlaşmamış ama temeller sağlam.

#### Tahmin 4: Çok Modlu Uygulamalar (Ses + Görüntü + Metin)

Yerel çok modlu modeller (LLaVA, Qwen-VL, Fuyu) hızla gelişiyor. Ses modelleri (Whisper, Bark, XTTS) zaten yerelde üretim kalitesinde. Yerel donanımda metin + görüntü + ses + video işlemenin birleşmesi yeni uygulama kategorileri açıyor.

2027'ye kadar bekle:
- Şu anda metni işlediğimiz kolaylıkla video, görüntü ve ses işleyen yerel modeller
- Görsel içeriği buluta göndermeden analiz eden uygulamalar
- Yerel modellerle güçlendirilmiş ses-öncelikli arayüzler

**Gelir etkisi:** Çok modlu içeriği yerelde işleyen uygulamalar — video analiz araçları, ses kontrollü geliştirme ortamları, imalat için görsel denetim sistemleri.

**Şimdiki öğrenme yatırımı:** Ollama aracılığıyla LLaVA veya Qwen-VL ile deney yap. Görüntüleri yerelde işleyen bir prototip oluştur. Gecikme ve kalite takas'larını anla.

```bash
# Şu anda yerelde çok modlu bir model dene
ollama pull llava:13b

# Bir görüntüyü analiz et (base64 kodlaman gerekiyor)
# İşleme tamamen makinende gerçekleşecek
curl http://localhost:11434/api/generate -d '{
  "model": "llava:13b",
  "prompt": "Describe what you see in this image in detail. Focus on any technical elements.",
  "images": ["<base64-encoded-image>"],
  "stream": false
}'
```

#### Tahmin 5: AI Düzenlemesi Küresel Olarak Genişliyor

AB AI Yasası ilk, ama son değil. Brezilya, Kanada, Japonya, Güney Kore ve birçok ABD eyaleti AI düzenlemesi geliştiriyor. Hindistan ifşa gereksinimleri düşünüyor. Küresel düzenleyici yüzey alanı genişliyor.

2027'ye kadar bekle:
- AI'ya özgü düzenlemesi olan en az 3-4 büyük yargı alanı
- Tanımlanmış bir profesyonel hizmet kategorisi haline gelen uyumluluk danışmanlığı
- Kurumsal yazılım tedarikinde standart bir gereksinim olarak "AI denetimi"

**Gelir etkisi:** Uyumluluk uzmanlığı giderek daha değerli hale geliyor. Bir şirketin AI sisteminin birden fazla yargı alanındaki düzenleyici gereksinimleri karşıladığını göstermesine yardım edebiliyorsan, $200-500/saat değerinde bir hizmet sunuyorsun.

**Şimdiki öğrenme yatırımı:** AB AI Yasasını oku (özetleri değil — gerçek metni). Risk sınıflandırma sistemini anla. NIST AI Risk Yönetimi Çerçevesini takip et. Bu bilgi bileşik büyüyor.

### Trend Kaymalarından Bağımsız Transfer Eden Beceriler

Trendler gelir ve gider. Bu beceriler her döngüde değerli kalır:

**1. Sistem Düşüncesi**
Karmaşık sistemlerde bileşenlerin nasıl etkileşim kurduğunu anlamak. İster mikro hizmet mimarisi, ister makine öğrenmesi hattı veya iş süreci olsun — bileşen etkileşimlerinden ortaya çıkan davranış hakkında akıl yürütme yeteneği kalıcı olarak değerli.

**2. Gizlilik ve Güvenlik Uzmanlığı**
Her trend verileri daha değerli kılıyor. Her düzenleme veri işlemeyi daha karmaşık kılıyor. Güvenlik ve gizlilik uzmanlığı kalıcı bir hendeğe. "Nasıl inşa edileceğini" de "güvenli nasıl inşa edileceğini" de anlayan geliştirici 1.5-2 kat ücret alıyor.

**3. API Tasarımı**
Her dönem yeni API'ler yaratır. REST, GraphQL, WebSockets, MCP, ajan protokolleri — özellikler değişiyor ama temiz, birleştirilebilir, iyi belgelenmiş arayüzler tasarlama prensipleri sabit. İyi API tasarımı nadir ve değerli.

**4. Geliştirici Deneyimi (DX) Tasarımı**
Diğer geliştiricilerin gerçekten kullanmaktan zevk aldığı araçlar inşa etme yeteneği. Bu, teknik beceri, empati ve zevkin çok az insanda bulunan bir kombinasyonu. Harika DX'li araçlar inşa edebiliyorsan, bunları herhangi bir teknolojide inşa edebilirsin ve kullanıcı bulurlar.

**5. Teknik Yazarlık**
Karmaşık teknik kavramları açıkça açıklama yeteneği. Bu her bağlamda değerli: dokümantasyon, blog yazıları, kurslar, danışmanlık çıktıları, açık kaynak README dosyaları, ürün pazarlaması. İyi teknik yazarlık kalıcı olarak nadir ve kalıcı olarak talep görüyor.

### "Beceri Sigortası" Stratejisi

Öğrenme zamanını üç ufukta dağıt:

```
|  Ufuk      |  Zaman Dağılımı  |  Örnek (2026)                      |
|------------|------------------|------------------------------------|
| ŞİMDİ     | Öğrenmenin %60'ı | Mevcut yığınını derinleştir         |
|            |                  | (bugün kazandığın beceriler)        |
|            |                  |                                    |
| 12 AY     | Öğrenmenin %30'u | Cihaz üzerinde AI, ajan protokolleri,|
|            |                  | çok modlu işleme                   |
|            |                  | (2027'de para ödeyecek beceriler)  |
|            |                  |                                    |
| 36 AY     | Öğrenmenin %10'u | Merkeziyetsiz AI, ajan ticareti,   |
|            |                  | çapraz-yargı alanı uyumluluk       |
|            |                  | (farkındalık düzeyi, uzmanlık      |
|            |                  | değil)                             |
```

**60/30/10 dağılımı kasıtlı:**

- %60 "ŞİMDİ" becerilerine kazanmaya devam etmeni ve mevcut gelir akışlarının sağlıklı kalmasını sağlar
- %30 "12 AY" becerilerine ihtiyacın olmadan önce bir sonraki gelir akışının temelini oluşturur
- %10 "36 AY" becerilerine gerçekleşmeyebilecek şeylere fazla yatırım yapmadan nelerin geldiğinden haberdar tutar

> **Yaygın Hata:** "36 AY" ufkundaki şeylere %80 öğrenme zamanı harcamak çünkü heyecan verici, mevcut gelir akışların temel becerileri korumadığın için çürürken. Geleceğe karşı koruma, şimdiyi terk etmek demek değil. Geleceği stratejik olarak keşfederken şimdiyi korumak demek.

### Nasıl Gerçekten Öğrenilir (Verimli Olarak)

Geliştirici öğrenmesinin verimlilik problemi var. "Öğrenmenin" çoğu aslında:
- Hiçbir şey inşa etmeden öğreticiler okuma (kalıcılık: ~%10)
- YouTube'u 2x hızda izleme (kalıcılık: ~%5)
- Kurs satın alıp %20'sini bitirme (kalıcılık: ~%15)
- Takılınca dokümantasyon okuma, anlık sorunu çözme ve hemen unutma (kalıcılık: ~%20)

Tutarlı olarak yüksek kalıcılığa sahip tek yöntem **yeni beceriyle gerçek bir şey inşa edip yayınlamak.**

```
Hakkında okuma:              %10 kalıcılık
Öğretici izleme:             %15 kalıcılık
Takip etme:                  %30 kalıcılık
Gerçek bir şey inşa etme:   %60 kalıcılık
İnşa edip yayınlama:        %80 kalıcılık
İnşa edip yayınlayıp        %95 kalıcılık
öğretme:
```

Yatırım yaptığın her "12 AY" becerisi için minimum çıktı şu olmalı:
1. Bir çalışan prototip (oyuncak değil — gerçek bir kullanım durumunu işleyen bir şey)
2. Bir yayınlanmış eser (blog yazısı, açık kaynak repo veya ürün)
3. Bu beceri için ödeme yapacak biriyle bir konuşma

Öğrenme zamanını gelecek gelire böyle dönüştürürsün.

### Sıra Sende

1. **60/30/10 dağılımını yaz.** ŞİMDİ becerilerin (%60), 12 AY becerilerin (%30) ve 36 AY becerilerin (%10) neler? Spesifik ol — sadece kategorileri değil teknolojileri adlandır.
2. **Bir "12 AY" becerisi seç** ve bu hafta 2 saat harca. Hakkında okuma değil — onunla bir şey inşa etme, önemsiz bile olsa.
3. **Mevcut öğrenme alışkanlıklarını denetle.** Son aydaki öğrenme zamanının ne kadarı yayınlanmış bir esere yol açtı? Cevap "hiçbiri"yse, düzeltilecek şey bu.
4. **6 ay sonrası için takvim hatırlatıcısı koy:** "Beceri tahminlerini incele. 12 aylık bahisler doğru muydu? Dağılımı ayarla."

---

### $500/Ay'dan $10K/Ay'a Ölçekleme

Çoğu geliştirici gelir akışı $500/ay ile $2,000/ay arasında takılır. Konsepti kanıtladın, müşteriler var, gelir gerçek — ama büyüme platoya çıkıyor. Bu bölüm, o platoyu kırmak için pratik el kitabı.

**Akışlar neden $500-2,000/ay'da takılır:**

1. **Kişisel kapasite tavanına çarptın.** Bir kişinin üretebileceği destek biletleri, danışmanlık saatleri veya içerik parçalarının sınırı var.
2. **Her şeyi kendin yapıyorsun.** Pazarlama, geliştirme, destek, muhasebe, içerik — bağlam değiştirme etkili çıktını öldürüyor.
3. **Fiyatın çok düşük.** Erken müşterileri çekmek için lansman fiyatları belirledin ve hiç yükseltmedin.
4. **"Hayır" demiyorsun.** Özellik talepleri, özel iş, "hızlı aramalar" — küçük dikkat dağıtıcılar büyük zaman kayıplarına bileşiyor.

**$500'dan $2K'ya Aşama: Fiyatlandırmayı Düzelt**

$500/ay kazanıyorsan, ilk hamlen neredeyse her zaman fiyat artışı, daha fazla müşteri değil. Çoğu geliştirici %30-50 düşük fiyatlandırıyor.

```
Mevcut: 100 müşteri x $5/ay = $500/ay
Seçenek A: 100 DAHA müşteri kazan (desteği, pazarlamayı, altyapıyı ikiye katla) = $1,000/ay
Seçenek B: Fiyatı $9/ay'a yükselt, müşterilerin %20'sini kaybet = 80 x $9 = $720/ay

Seçenek B, DAHA AZ müşteri ve DAHA AZ destek yüküyle %44 daha fazla gelir verir.
Aynı %20 kayıpla $15/ay'da: 80 x $15 = $1,200/ay — %140 artış.
```

**Kanıt:** Patrick McKenzie'nin binlerce SaaS ürünü analizine göre bağımsız geliştiriciler neredeyse evrensel olarak düşük fiyatlandırıyor. Fiyat artışıyla kaybettiğin müşteriler genellikle en çok destek bileti üreten ve en az iyi niyet gösteren. En iyi müşterilerin %50'lik fiyat artışını zar zor fark eder çünkü sağladığın değer maliyeti çok aşıyor.

**Cesaretini kaybetmeden fiyat nasıl yükseltilir:**

1. **Mevcut müşterilere** mevcut oranı koru (isteğe bağlı ama sürtünmeyi azaltır)
2. **30 gün öncesinden** e-postayla duyur: "[tarihten] itibaren yeni fiyatlandırma [X]. Mevcut oranın [6 ay / sonsuza dek] kilitli."
3. **Artışla birlikte küçük bir iyileştirme ekle** — yeni özellik, daha hızlı performans, daha iyi dokümanlar. İyileştirmenin fiyat artışını haklı çıkarması gerekmez, ama müşterilere değişiklikle ilişkilendirecek pozitif bir şey verir.
4. **60 gün boyunca kaybı takip et.** Kayıp %10 altında kalırsa fiyat artışı doğruydu. %20'yi aşarsa çok fazla sıçramış olabilirsin — ara katman düşün.

**$2K'dan $5K'ya Aşama: Otomatikleştir veya Delege Et**

$2K/ay'da düşük değerli görevlerden kendini çıkarmaya başlayabilecek kadar kazanıyorsun. Matematik işliyor:

```
$2K/ay, 20 saat/hafta'daki etkili saatlik oranın = $25/saat
Sanal asistan $10-20/saat maliyetli
Sözleşmeli geliştirici $30-60/saat maliyetli

İLK delege edilecek görevler (en yüksek kaldıraç):
1. Müşteri desteği (VA, $10-15/saat) — 3-5 saat/hafta serbest bırakır
2. İçerik biçimlendirme/planlama (VA, $10-15/saat) — 2-3 saat/hafta serbest bırakır
3. Defter tutma (uzman VA, $15-25/saat) — 1-2 saat/hafta serbest bırakır

Toplam maliyet: ~$400-600/ay
Serbest bırakılan zaman: 6-10 saat/hafta
Bu 6-10 saat ürün geliştirme, pazarlama veya ikinci akışa gider.
```

**İlk yükleniciyi işe alma:**

- **Tek, tanımlanmış bir görevle başla.** "İşimde yardım et" değil. Daha çok "bu el kitabı belgesini kullanarak destek biletlerine cevap ver, kod değişikliği gerektiren her şeyi eskalasyon et."
- **Nerede bulunur:** Upwork (%90+ iş başarısı, 100+ saat filtresi), OnlineJobs.ph (VA'lar için) veya diğer bağımsız geliştiricilerden kişisel öneriler.
- **Adil öde.** $8/saatlik ve sürekli gözetim gerektiren yüklenici, $15/saatlik ve bağımsız çalışan yükleniciden daha pahalı.
- **Önce talimat kitabı oluştur.** Her tekrarlanabilir görevi teslim etmeden önce belgele. Süreci yazamıyorsan, delege edemezsin.
- **Deneme süresi:** 2 hafta, ücretli, spesifik çıktıyla. Kalite yoksa denemeyi bitir. Uymayan birini "eğitmek" için aylarca yatırım yapma.

**$5K'dan $10K'ya Aşama: Sistemler, Çaba Değil**

$5K/ay'da "yan proje" aşamasını geçtin. Bu gerçek bir iş. $10K'ya sıçrama çaba değil sistem düşüncesi gerektiriyor.

**Bu aşamadaki üç kaldıraç:**

1. **Ürün hattını genişlet.** Mevcut müşterilerin en sıcak kitlen. Onlara hangi bitişik ürünü satabilirsin?
   - SaaS müşterileri şablonlar, rehberler veya danışmanlık ister
   - Şablon alıcıları şablonun elle yaptığını otomatikleştiren SaaS ister
   - Danışmanlık müşterileri ürünleştirilmiş hizmetler ister (sabit kapsam, sabit fiyat)

2. **Bileşik büyüyen dağıtım kanalları inşa et.**
   - SEO: Her blog yazısı kalıcı bir potansiyel müşteri kaynağı. Nişindeki uzun kuyruk anahtar kelimeleri hedefleyen ayda 2-4 kaliteli yazıya yatırım yap.
   - E-posta listesi: Bu en değerli varlığın. Besle. Listene haftada odaklı bir e-posta, günlük sosyal medya paylaşımını geride bırakır.
   - Ortaklıklar: Tamamlayıcı (rakip değil) ürünler bul ve çapraz tanıtım yap. Bir bileşen kütüphanesiyle ortaklık yapan tasarım sistemi aracı doğaldır.

3. **Fiyatları tekrar yükselt.** $500/ay'da fiyat yükselttiysen ve o zamandan beri yükseltmediysen, zamanı geldi. Ürünün artık daha iyi. İtibarın daha güçlü. Destek altyapın daha güvenilir. Değer arttı — fiyat bunu yansıtmalı.

**Teslimata otomasyonu:**

$5K+/ay'da manuel teslimat darboğaz olur. Önce bunları otomatikleştir:

| Süreç | Manuel Maliyet | Otomasyon Yaklaşımı |
|-------|---------------|---------------------|
| Yeni müşteri karşılama | 15-30 dk/müşteri | Otomatik hoş geldin e-posta serisi + self-servis dokümanlar |
| Lisans anahtarı teslimi | 5 dk/satış | Keygen, Gumroad veya Lemon Squeezy otomatik halleder |
| Fatura oluşturma | 10 dk/fatura | Stripe otomatik faturalama veya QuickBooks entegrasyonu |
| İçerik yayınlama | 1-2 saat/yazı | Planlanmış yayın + otomatik çapraz paylaşım |
| Metrik raporlama | 30 dk/hafta | Panel (Plausible, PostHog, özel) otomatik haftalık e-postayla |

**$10K/ay'daki zihniyet değişimi:**

$10K altında gelir büyümesini optimize ediyorsun. $10K'da zaman verimliliğini optimize etmeye başlıyorsun. Soru "nasıl daha fazla para kazanırım?" dan "aynı parayı daha az saatte nasıl kazanırım?"a dönüşüyor — çünkü o serbest bırakılan zaman büyümenin bir sonraki aşamasına yatırdığın şey.

### Bir Akışı Ne Zaman Öldürmeli: Karar Çerçevesi

Modül S2 dört öldürme kuralını derinlemesine ele alıyor ($100 Kuralı, ROI Kuralı, Enerji Kuralı, Fırsat Maliyeti Kuralı). İşte Gelişen Sınır bağlamı için tamamlayıcı çerçeve — piyasa zamanlamasının mücadele eden bir akışın sabır sorunu mu yoksa piyasa sorunu mu olduğunu belirlediği yer.

**Piyasa Zamanlaması Öldürme Kriterleri:**

Düşük performanslı her akış daha fazla çabayı hak etmez. Bazıları gerçekten erken (sabır karşılığını verir). Diğerleri geç (sen inşa ederken pencere kapandı). İkisi arasında ayrım yapmak, azim ile inatçılık arasındaki farktır.

```
AKIŞ SAĞLIK DEĞERLENDİRMESİ

Akış adı: _______________
Yaş: _____ ay
Aylık gelir: $_____
Aylık yatırılan saatler: _____
Gelir trendi (son 3 ay): [ ] Büyüyor  [ ] Sabit  [ ] Düşüyor

PİYASA SİNYALLERİ:
1. Anahtar kelimelerin için arama hacmi büyüyor mu düşüyor mu?
   [ ] Büyüyor → piyasa genişliyor (sabır karşılık verebilir)
   [ ] Sabit → piyasa olgun (farklılaş veya çık)
   [ ] Düşüyor → piyasa daralıyor (nişe hâkim değilsen çık)

2. Rakipler giriyor mu çıkıyor mu?
   [ ] Yeni rakipler geliyor → piyasa doğrulanmış ama kalabalıklaşıyor
   [ ] Rakipler çıkıyor → piyasa ölüyor ya da müşterilerini devralacaksın
   [ ] Değişiklik yok → kararlı piyasa, büyüme uygulamana bağlı

3. Bağlı olduğun platform/teknoloji yön değiştirdi mi?
   [ ] Değişiklik yok → kararlı temel
   [ ] Küçük değişiklikler (fiyatlandırma, özellikler) → uyum sağla ve devam et
   [ ] Büyük değişiklikler (kullanımdan kaldırma, satın alma, yön değiştirme) → ciddiyetle çıkışı değerlendir

KARAR:
- Gelir büyüyor VE piyasa sinyalleri pozitif → KORU (daha fazla yatırım)
- Gelir sabit VE piyasa sinyalleri pozitif → İTERASYON YAP (yaklaşımı değiştir, ürünü değil)
- Gelir sabit VE piyasa sinyalleri nötr → SON TARİH BELİRLE (90 gün büyüme göster veya öldür)
- Gelir düşüyor VE piyasa sinyalleri negatif → ÖLDÜR (piyasa konuştu)
- Gelir düşüyor VE piyasa sinyalleri pozitif → sorun uygulamanda, piyasada değil — düzelt veya düzeltebilecek birini bul
```

> **En zor öldürme:** Piyasanın istemediği bir akışa duygusal olarak bağlı olduğunda. Güzelce inşa ettin. Kod temiz. UX düşünceli. Ve kimse satın almıyor. Piyasa sana sıkı çalıştığın için gelir borçlu değil. Öldür, dersleri çıkar ve enerjiyi yönlendir. Beceriler transfer eder. Kodun transfer etmesi gerekmez.

---

## Ders 6: 2026 Fırsat Radarın

*"Yazdığın plan kafandaki planı her seferinde yener."*

### Çıktı

{? if dna.is_full ?}
Developer DNA profilin ({= dna.identity_summary | fallback("kimlik özetin") =}) sana burada avantaj sağlıyor. Seçeceğin fırsatlar DNA'nın ortaya koyduğu güçlü yönlerini kullanmalı — ve boşlukları telafi etmeli. Kör noktaların ({= dna.blind_spots | fallback("daha az etkileşim kurduğun alanlar") =}) üç bahsini seçerken dikkat etmeye değer.
{? endif ?}

İşte bu — bu modülü zamanına değer kılan çıktı. 2026 Fırsat Radarın bu yıl yaptığın üç bahsi, gerçekten uygulayabilecek yeterli spesifiklikle belgeler.

Beş bahis değil. "Birkaç fikir" değil. Üç. İnsanlar aynı anda üçten fazla şeyi takip etmekte korkunç. Bir ideal. Üç maksimum.

Neden üç?

- **Fırsat 1:** Birincil bahsin. Çabanın %70'i buraya gider. Bahislerinden sadece biri başarılı olacaksa, bunun olmasını istersin.
- **Fırsat 2:** İkincil bahsin. Çabanın %20'si buraya gider. Ya Fırsat 1'in başarısız olmasına karşı bir hedge ya da doğal tamamlayıcısı.
- **Fırsat 3:** Deneyin. Çabanın %10'u buraya gider. Joker — benimseme eğrisinde daha erken olan, devasa olabilecek veya sönebilecek bir şey.

### Şablon

Bunu kopyala. Doldur. Yazdır ve duvarına yapıştır. Her Pazartesi sabahı aç. Bu 2026 için operasyon belgen.

```markdown
# 2026 Fırsat Radarı
# [Adın]
# Oluşturulma: [Tarih]
# Sonraki İnceleme: [Tarih + 90 gün]

---

## Fırsat 1: [İSİM] — Birincil (%70 çaba)

### Ne Olduğu
[Tam olarak ne inşa ettiğini/sattığını/sunduğunu açıklayan bir paragraf]

### Neden Şimdi
[Bu fırsatın BUGÜN var olmasının ve 12 ay önce olmamasının üç spesifik nedeni]
1.
2.
3.

### Rekabet Avantajım
[Seni rastgele bir geliştiriciden daha iyi konumlandıran ne var?]
- Beceri avantajı:
- Bilgi avantajı:
- Ağ avantajı:
- Zamanlama avantajı:

### Gelir Modeli
- Fiyatlandırma: [Spesifik fiyat noktası/noktaları]
- Ay 1 gelir hedefi: $[X]
- Ay 3 gelir hedefi: $[X]
- Ay 6 gelir hedefi: $[X]
- Ay 12 gelir hedefi: $[X]

### 30 Günlük Eylem Planı
Hafta 1: [Spesifik, ölçülebilir eylemler]
Hafta 2: [Spesifik, ölçülebilir eylemler]
Hafta 3: [Spesifik, ölçülebilir eylemler]
Hafta 4: [Spesifik, ölçülebilir eylemler]

### Başarı Kriterleri
- İKİYE KATLA sinyali: [Çabayı artırmanı sağlayacak ne?]
  Örnek: "60 günde 3+ ödeme yapan müşteri"
- YÖN DEĞİŞTİR sinyali: [Yaklaşımı değiştirmeni sağlayacak ne?]
  Örnek: "500+ görüntülemeye rağmen 90 gün sonra 0 ödeme yapan müşteri"
- ÖLDÜR sinyali: [Tamamen vazgeçmeni sağlayacak ne?]
  Örnek: "Büyük platform ücretsiz rakip özellik duyuruyor"

---

## Fırsat 2: [İSİM] — İkincil (%20 çaba)

### Ne Olduğu
[Bir paragraf]

### Neden Şimdi
1.
2.
3.

### Rekabet Avantajım
- Beceri avantajı:
- Bilgi avantajı:
- Fırsat 1 ile ilişkisi:

### Gelir Modeli
- Fiyatlandırma:
- Ay 3 gelir hedefi: $[X]
- Ay 6 gelir hedefi: $[X]

### 30 Günlük Eylem Planı
Hafta 1-2: [Spesifik eylemler — unutma, buraya sadece %20 çaba gidiyor]
Hafta 3-4: [Spesifik eylemler]

### Başarı Kriterleri
- İKİYE KATLA:
- YÖN DEĞİŞTİR:
- ÖLDÜR:

---

## Fırsat 3: [İSİM] — Deney (%10 çaba)

### Ne Olduğu
[Bir paragraf]

### Neden Şimdi
[Bir ikna edici neden]

### 30 Günlük Eylem Planı
[Fırsatı doğrulamak için 2-3 spesifik, küçük deney]
1.
2.
3.

### Başarı Kriterleri
- Fırsat 2'ye YÜKSELT eğer: [ne olması gerekir]
- ÖLDÜR eğer: [ne kadar süre sonra ivme yoksa]

---

## Üç Aylık İnceleme Takvimi

- Ç1 İnceleme: [Tarih]
- Ç2 İnceleme: [Tarih]
- Ç3 İnceleme: [Tarih]
- Ç4 İnceleme: [Tarih]

Her incelemede:
1. Her fırsatın başarı kriterlerini gerçek sonuçlarla karşılaştır
2. Karar ver: ikiye katla, yön değiştir veya öldür
3. Öldürülen fırsatları istihbarat günlüğünden yenileriyle değiştir
4. Gelir hedeflerini gerçek performansa göre güncelle
5. Çaba dağılımını işe yarayan şeye göre ayarla
```

### Tamamlanmış Bir Örnek

İyi birinin nasıl göründüğünü görebilmen için gerçekçi, doldurulmuş bir Fırsat Radarı:

```markdown
# 2026 Fırsat Radarı
# Alex Chen
# Oluşturulma: 2026-02-18
# Sonraki İnceleme: 2026-05-18

---

## Fırsat 1: DevOps İçin MCP Sunucu Paketi — Birincil (%70)

### Ne Olduğu
AI kodlama araçlarını DevOps altyapısına bağlayan 5 MCP
sunucusu paketi: Docker yönetimi, Kubernetes küme durumu,
CI/CD boru hattı izleme, log analizi ve olay müdahalesi.
Gumroad/Lemon Squeezy'de paket olarak satılıyor, premium
"yönetilen barındırma" katmanıyla.

### Neden Şimdi
1. MCP ekosistemi erken — DevOps odaklı paket henüz yok
2. Claude Code ve Cursor kurumsal planlara MCP desteği ekliyor
3. DevOps mühendisleri olaylar sırasında zaman kazandıran
   araçlar için ödeme yapacak yüksek değerli kullanıcılar

### Rekabet Avantajım
- Beceri: 6 yıl DevOps deneyimi (Kubernetes, Docker, CI/CD)
- Bilgi: Sorunlu noktaları biliyorum çünkü her gün yaşıyorum
- Zamanlama: İlk kapsamlı DevOps MCP paketi

### Gelir Modeli
- Paket fiyatı: $39 (tek seferlik)
- Yönetilen barındırma katmanı: $15/ay
- Ay 1 gelir hedefi: $400 (10 paket satışı)
- Ay 3 gelir hedefi: $1,500 (25 paket + 20 yönetilen)
- Ay 6 gelir hedefi: $3,000 (40 paket + 50 yönetilen)
- Ay 12 gelir hedefi: $5,000+ (yönetilen katman büyüyor)

### 30 Günlük Eylem Planı
Hafta 1: Docker MCP sunucusu + Kubernetes MCP sunucusu inşa et (5'in çekirdeği 2)
Hafta 2: CI/CD ve log analiz sunucuları inşa et (5'in 3-4. sunucuları)
Hafta 3: Olay müdahale sunucusu inşa et, açılış sayfası oluştur, doküman yaz
Hafta 4: Gumroad'da lansman, HN Show'da paylaş, tweet dizisi, r/devops

### Başarı Kriterleri
- İKİYE KATLA: İlk 60 günde 20+ satış
- YÖN DEĞİŞTİR: 60 günde <5 satış (farklı konumlandırma veya dağıtım dene)
- ÖLDÜR: Büyük bir platform (Datadog, PagerDuty) ürünleri için ücretsiz
  MCP sunucuları yayınlıyor

---

## Fırsat 2: Yerel AI Dağıtım Blogu + Danışmanlık — İkincil (%20)

### Ne Olduğu
Gerçek yapılandırmalar ve karşılaştırmalarla yerel AI dağıtım
kalıplarını belgeleyen blog. Danışmanlık potansiyel müşterileri üretir.
Blog yazıları ücretsiz; danışmanlık $200/saat.

### Neden Şimdi
1. AB AI Yasası şeffaflık yükümlülükleri yeni başladı (Şub 2026)
2. YEREL dağıtım hakkında içerik (bulut değil) nadir
3. Her blog yazısı kalıcı danışmanlık potansiyel müşteri mıknatısı

### Rekabet Avantajım
- Beceri: Zaten ana işte üretimde yerel LLM'ler çalıştırıyorum
- Bilgi: Kimsenin yayınlamadığı karşılaştırmalar ve yapılandırmalar
- Fırsat 1 ile ilişkisi: MCP sunucuları yetkinliği gösteriyor

### Gelir Modeli
- Blog: $0 (potansiyel müşteri üretimi)
- Danışmanlık: $200/saat, hedef 5 saat/ay
- Ay 3 gelir hedefi: $1,000/ay
- Ay 6 gelir hedefi: $2,000/ay

### 30 Günlük Eylem Planı
Hafta 1-2: 2 kaliteli blog yazısı yaz ve yayınla
Hafta 3-4: LinkedIn'de tanıt, ilgili HN dizilerinde etkileşime gir

### Başarı Kriterleri
- İKİYE KATLA: 60 günde 2+ danışmanlık talebi
- YÖN DEĞİŞTİR: 90 gün sonra 0 talep (içerik alıcılara ulaşmıyor)
- ÖLDÜR: Düşük olasılık — blog yazıları her halükarda bileşik büyür

---

## Fırsat 3: Ajan-Ajan Protokolü Deneyi — Deney (%10)

### Ne Olduğu
Ajan-ajan iletişim kalıplarını keşfetme — bir MCP sunucusunun
diğerini keşfedip çağırabildiği bir prototip inşa etme.
Ajan ticareti gerçekleşirse, erken altyapı yapıcıları kazanır.

### Neden Şimdi
- Anthropic ve OpenAI ikisi de ajan birlikte çalışabilirliğine
  ipucu veriyor
- Bu 12-18 ay erken, ama altyapı oyunu küçük bir bahse değer

### 30 Günlük Eylem Planı
1. Birbirini keşfedebilen iki MCP sunucusu inşa et
2. Faturalandırma mekanizması prototiple (bir ajan diğerine ödeme yapıyor)
3. Bulguları blog yazısı olarak yaz

### Başarı Kriterleri
- Fırsat 2'ye YÜKSELT eğer: herhangi bir büyük oyuncu tarafından
  ajan birlikte çalışabilirlik protokolü duyurulursa
- ÖLDÜR eğer: 6 ay sonra protokol hareketi yoksa

---

## Üç Aylık İnceleme: 18 Mayıs 2026
```

### Üç Aylık İnceleme Ritüeli

Her 90 günde bir 2 saati blokla. 30 dakika değil — iki saat. Bu, çeyreğin en değerli planlama zamanı.

**İnceleme gündemi:**

```
Saat 1: Değerlendirme
  0:00 - 0:15  Her fırsatın başarı kriterlerini gerçek sonuçlarla incele
  0:15 - 0:30  İstihbarat günlüğünü yükselen sinyaller için incele
  0:30 - 0:45  Değerlendir: son incelemeden beri piyasada ne değişti?
  0:45 - 1:00  Dürüst öz-değerlendirme: neyi iyi uyguladım? Neyi bıraktım?

Saat 2: Planlama
  1:00 - 1:15  Her fırsat için karar: ikiye katla / yön değiştir / öldür
  1:15 - 1:30  Bir fırsatı öldürüyorsan, istihbarat günlüğünden yerine birini seç
  1:30 - 1:45  Çaba dağılımını ve gelir hedeflerini güncelle
  1:45 - 2:00  Her fırsat için sonraki 90 günlük eylem planı yaz
```

**Çoğu kişinin atladığı (ve atlamaması gereken):**

"Dürüst öz-değerlendirme" adımı. Gelir hedefleri karşılanmadığında piyasayı suçlamak kolay. Bazen piyasa sorun. Ama çoğu zaman sorun planı uygulamaman. Yeni bir fikirle dikkatin dağıldı, veya teslim etmek yerine bir şeyi "mükemmelleştirmek" için 3 hafta harcadın, veya yapacağını söylediğin ulaşımı yapmadın.

İncelemende dürüst ol. Fırsat Radarı sadece gerçek verilerle güncellersen çalışır, rahat anlatılarla değil.

### Sıra Sende

1. **Fırsat Radarı şablonunu doldur.** Üç fırsatın hepsi. Tüm alanlar. 60 dakika zamanlayıcı koy.
2. **Birincil fırsatını seç** Ders 2'deki yediden, Ders 3'teki zamanlama analizi, Ders 4'teki istihbarat sistemi ve Ders 5'teki geleceğe karşı koruma merceğiyle bilgilenmiş olarak.
3. **Fırsat 1 için 30 günlük eylem planını** haftalık kilometre taşlarıyla tamamla. İşaretlenebilecek kadar spesifik olmalılar. "MCP sunucusu üzerinde çalış" spesifik değil. "MCP sunucusunu npm'e README ve 3 örnek yapılandırmayla yayınla" spesifik.
4. **İlk üç aylık incelemeyi planla.** Takvimine koy. İki saat. Pazarlık kabul etmez.
5. **Fırsat Radarını bir kişiyle paylaş.** Hesap verebilirlik önemli. Bir arkadaşına, meslektaşına söyle veya herkese açık paylaş. "Bu yıl [X], [Y] ve [Z] peşindeyim. İşte planım." Bahislerini herkese açık ilan etme eylemi, bunları takip etme olasılığını önemli ölçüde artırır.

---

## Modül E: Tamamlandı

{? if progress.completed_count ?}
Artık STREETS modüllerinden {= progress.completed_count | fallback("bir tane daha") =}/{= progress.total_count | fallback("") =} tamamladın. Her modül bir öncekini bileşik olarak artırır — bu modüldeki istihbarat sistemi takip ettiğin her fırsatı doğrudan besler.
{? endif ?}

### Hafta 11'de Ne İnşa Ettin

Artık çoğu geliştiricinin asla oluşturmadığı bir şeye sahipsin: bu yıl zamanını ve enerjini nereye yatıracağına dair yapılandırılmış, kanıta dayalı bir plan.

Spesifik olarak, elinde şunlar var:

1. **Güncel bir manzara değerlendirmesi** — genel "AI her şeyi değiştiriyor" klişeleri değil, 2026'da yerel altyapıya sahip geliştiriciler için gelir fırsatları yaratan neyin değiştiğine dair spesifik bilgi.
2. **Yedi değerlendirilmiş fırsat** spesifik gelir potansiyeli, rekabet analizi ve eylem planlarıyla — soyut kategoriler değil, bu hafta başlayabileceğin uygulanabilir işler.
3. **Zamanlama çerçevesi** pazarlara çok erken veya çok geç girmeni engelleyen — artı her biri için izlenecek sinyaller.
4. **Çalışan bir istihbarat sistemi** şans ve gezinme alışkanlıklarına güvenmek yerine fırsatları otomatik olarak yüzeye çıkaran.
5. **Geleceğe karşı koruma stratejisi** gelirini 2027 ve sonrasında gelecek kaçınılmaz kaymalara karşı koruyan.
6. **2026 Fırsat Radarın** — yaptığın üç bahis, başarı kriterleri ve üç aylık inceleme döngüsüyle.

### Yaşayan Modül Sözü

Bu modül Ocak 2027'de yeniden yazılacak. Yedi fırsat değişecek. Bazıları yükseltilecek (hâlâ sıcaksa). Bazıları "pencere kapanıyor" olarak işaretlenecek. Yenileri eklenecek. Zamanlama çerçevesi yeniden kalibre edilecek. Tahminler gerçekliğe karşı denetlenecek.

STREETS Core aldıysan, güncellenmiş Gelişen Sınır modülünü her yıl ek ücret olmadan alırsın. Bu tamamlayıp rafa kaldırdığın bir kurs değil — koruduğun bir sistem.

### Sırada Ne Var: Modül T2 — Taktik Otomasyon

Fırsatlarını belirledin (bu modül). Şimdi bakım yerine uygulamaya odaklanabilmen için operasyonel yükü otomatikleştirmen gerekiyor.

Modül T2 (Taktik Otomasyon) şunları kapsıyor:

- **Otomatik içerik boru hatları** — istihbarat toplamadan yayınlanmış bültene minimum manuel müdahaleyle
- **Müşteri teslimat otomasyonu** — şablonlanmış teklifler, otomatik faturalama, planlanmış çıktılar
- **Gelir izleme** — akış başına geliri, edinme başına maliyeti ve ROI'yi gerçek zamanlı izleyen panolar
- **Uyarı sistemleri** — manuel kontrol yerine dikkatini gerektiren bir şey olduğunda bildirim al (piyasa kayması, müşteri sorunu, fırsat sinyali)
- **Geliştirici geliri için "4 saatlik çalışma haftası"** — operasyonel yükü haftada 4 saatin altına düşürmek, böylece geri kalan zamanın inşaya gider

Amaç: insan dikkati saati başına maksimum gelir. Makineler rutini halleder. Sen kararları hallediyorsun.

---

## 4DA Entegrasyonu

> **4DA'nın vazgeçilmez hale geldiği yer burası.**
>
> Gelişen Sınır modülü NE arayacağını söyler. 4DA NE ZAMAN olduğunu söyler.
>
> Semantik kayma algılama, bir teknolojinin "deneysel"den "üretim"e geçtiğini fark eder — giriş zamanlaması için tam ihtiyacın olan sinyal. Sinyal zincirleri, ortaya çıkan bir fırsatın hikaye yayını günler ve haftalar boyunca takip eder, HN tartışmasını GitHub sürümüne ve iş ilanı trendine bağlar. Eyleme dönüştürülebilir sinyaller, gelen içeriği Fırsat Radarınla eşleşen kategorilere sınıflandırır.
>
> Manuel kontrol etmene gerek yok. 10 RSS beslemesi ve bir Twitter listesi korumanıza gerek yok. 4DA SENİN planın için önemli olan sinyalleri, SENİN Developer DNA'na göre puanlanmış, SENİN günlük brifinginde teslim edilmiş olarak yüzeye çıkarır.
>
> 4DA kaynaklarını Ders 4'teki istihbarat yığınıyla eşleşecek şekilde ayarla. Developer DNA'nı Radarındaki fırsatları yansıtacak şekilde yapılandır. Sonra sen inşa ederken 4DA'nın taramasına izin ver.
>
> 4DA ile günde 15 dakika sinyal kontrol eden geliştirici, sistem olmadan günde 2 saat Hacker News gezen geliştiriciden önce fırsatları yakalar.
>
> İstihbarat daha fazla bilgi tüketmek değildir. Doğru bilgiyi doğru zamanda tüketmektir. 4DA'nın yaptığı tam olarak bu.

---

**Fırsat Radarın pusulan. İstihbarat sistemin radarın. Şimdi git ve inşa et.**

*Bu modül Şubat 2026'da yazılmıştır. 2027 edisyonu Ocak 2027'de mevcut olacaktır.*
*STREETS Core satın alanlar yıllık güncellemeleri ek ücret olmadan alır.*

*Senin makinan. Senin kuralların. Senin gelirin.*