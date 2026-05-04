# Modül R: Gelir Motorları

**STREETS Geliştirici Gelir Kursu — Ücretli Modül**
*Hafta 5-8 | 8 Ders | Çıktı: İlk Gelir Motorun + Motor #2 İçin Plan*

> "Sadece özellik gönderen kod değil, gelir üreten sistemler inşa et."

---

Altyapın hazır (Modül S). Rakiplerin kolayca kopyalayamayacağı bir şeyin var (Modül T). Şimdi tüm bunları paraya çevirme zamanı.

Bu, kurstaki en uzun modül çünkü en çok önemli olan bu. Sekiz gelir motoru. Becerilerini, donanımını ve zamanını gelire dönüştürmenin sekiz farklı yolu. Her biri gerçek kod, gerçek fiyatlandırma, gerçek platformlar ve gerçek matematikle dolu eksiksiz bir oyun kitabı.

{@ insight engine_ranking @}

Sekizini de inşa etmeyeceksin. İkisini seçeceksin.

**1+1 Stratejisi:**
- **Motor 1:** İlk dolara en hızlı yol. Bunu Hafta 5-6'da inşa edeceksin.
- **Motor 2:** Senin özel durumun için en ölçeklenebilir motor. Bunu Hafta 7-8'de planlayacak ve Modül E'de inşa etmeye başlayacaksın.

Neden iki? Çünkü tek gelir akışı kırılgandır. Bir platform şartlarını değiştirir, bir müşteri kaybolur, bir pazar kayar — ve sıfıra dönersin. Farklı müşteri tiplerine farklı kanallardan hizmet veren iki motor sana dayanıklılık kazandırır. Ve Motor 1'de geliştirdiğin beceriler neredeyse her zaman Motor 2'yi hızlandırır.

Bu modülün sonunda şunlara sahip olacaksın:

- Motor 1'den gelen gelir (veya günler içinde gelir üretecek altyapı)
- Motor 2 için detaylı bir inşa planı
- Hangi motorların becerilerine, zamanına ve risk toleransına uyduğuna dair net bir anlayış
- Gerçek, deploy edilmiş kod — sadece planlar değil

{? if progress.completed("T") ?}
Modül T'de hendeklerini inşa ettin. Şimdi o hendekler gelir motorlarının oturduğu temel oluyor — hendeklerin kopyalanması ne kadar zorsa, gelirin o kadar dayanıklı.
{? endif ?}

Teori yok. "Bir gün" yok. Hadi inşa edelim.

---

## Ders 1: Dijital Ürünler

*"Para basmaya en yakın yasal yol."*

**İlk dolara kadar süre:** 1-2 hafta
**Devam eden zaman taahhüdü:** Haftada 2-4 saat (destek, güncellemeler, pazarlama)
**Kar marjı:** %95+ (oluşturduktan sonra maliyetlerin neredeyse sıfır)

### Neden Önce Dijital Ürünler

{@ insight stack_fit @}

Dijital ürünler, geliştiriciler için en yüksek marjlı, en düşük riskli gelir motorudur. Bir kez inşa edersin, sonsuza kadar satarsın. Yönetilecek müşteri yok. Saatlik faturalandırma yok. Kapsam kayması yok. Toplantı yok.

Matematik basit:
- Bir şablon veya başlangıç kiti inşa etmek için 20-40 saat harcarsın
- Fiyatını {= regional.currency_symbol | fallback("$") =}49 koyarsın
- İlk ay 10 kopya satarsın: {= regional.currency_symbol | fallback("$") =}490
- Bundan sonra her ay 5 kopya satarsın: Aylık {= regional.currency_symbol | fallback("$") =}245 pasif gelir
- Oluşturma sonrası toplam maliyet: {= regional.currency_symbol | fallback("$") =}0

O aylık {= regional.currency_symbol | fallback("$") =}245 heyecan verici gelmeyebilir, ama sıfır devam eden zaman gerektiriyor. Üç ürün üst üste koy ve sen uyurken aylık {= regional.currency_symbol | fallback("$") =}735'e ulaşırsın. On tane üst üste koy ve bir junior geliştirici maaşını karşılamış olursun.

### Ne Satılır

{? if stack.primary ?}
İnşa edebileceğin her şey satılmaz. Bir {= stack.primary | fallback("developer") =} geliştiricisi olarak avantajın var: yığınının hangi sorunları olduğunu biliyorsun. İşte geliştiricilerin gerçekte ne için para ödediği, bugün var olan ürünlerden gerçek fiyat noktalarıyla:
{? else ?}
İnşa edebileceğin her şey satılmaz. İşte geliştiricilerin gerçekte ne için para ödediği, bugün var olan ürünlerden gerçek fiyat noktalarıyla:
{? endif ?}

**Başlangıç Kitleri ve Şablonlar**

| Ürün | Fiyat | Neden Satılır |
|------|-------|--------------|
| Auth, DB, otomatik güncelleme ile üretim hazır Tauri 2.0 + React başlangıç kiti | $49-79 | 40+ saatlik şablon koddan tasarruf sağlar. Tauri belgeleri iyi ama üretim kalıplarını kapsamıyor. |
| Stripe faturalandırma, e-posta, auth, yönetici paneli ile Next.js SaaS başlangıç kiti | $79-149 | ShipFast ($199) ve Supastarter ($299) bu pazarın var olduğunu kanıtlıyor. Odaklı, daha ucuz alternatifler için yer var. |
| MCP sunucu şablon paketi (yaygın kalıplar için 5 şablon) | $29-49 | MCP yeni. Çoğu geliştirici henüz bir tane yapmadı. Şablonlar boş sayfa sorununu ortadan kaldırır. |
| Claude Code / Cursor için AI ajan yapılandırma paketi | $29-39 | Alt-ajan tanımları, CLAUDE.md şablonları, iş akışı yapılandırmaları. Yeni pazar, neredeyse sıfır rekabet. |
| Otomatik yayınlama, çapraz derleme, homebrew ile Rust CLI araç şablonu | $29-49 | Rust CLI ekosistemi hızla büyüyor. Doğru yayınlama şaşırtıcı derecede zor. |

**Bileşen Kütüphaneleri ve UI Kitleri**

| Ürün | Fiyat | Neden Satılır |
|------|-------|--------------|
| Koyu mod dashboard bileşen kiti (React + Tailwind) | $39-69 | Her SaaS'ın bir dashboard'a ihtiyacı var. İyi koyu mod tasarım nadir. |
| E-posta şablon paketi (React Email / MJML) | $29-49 | İşlemsel e-posta tasarımı sıkıcı. Geliştiriciler bundan nefret eder. |
| Geliştirici araçları için optimize edilmiş açılış sayfası şablon paketi | $29-49 | Geliştiriciler kod yazabilir ama tasarım yapamaz. Önceden tasarlanmış sayfalar dönüştürür. |

**Dokümantasyon ve Yapılandırma**

| Ürün | Fiyat | Neden Satılır |
|------|-------|--------------|
| Yaygın yığınlar için üretim Docker Compose dosyaları | $19-29 | Docker evrensel ama üretim yapılandırmaları sözlü aktarılan bilgi. |
| 20 yaygın kurulum için Nginx/Caddy ters proxy yapılandırmaları | $19-29 | Kopyala-yapıştır altyapı. Saatlerce Stack Overflow'dan tasarruf sağlar. |
| GitHub Actions iş akışı paketi (10 yaygın yığın için CI/CD) | $19-29 | CI/CD yapılandırması bir kez yazılır, saatlerce Google'lanır. Şablonlar bunu çözer. |

> **Gerçek Konuşma:** En iyi satan ürünler belirli, anlık bir acıyı çözer. "40 saatlik kurulumdan tasarruf et" her zaman "yeni bir framework öğren"i yener. Geliştiriciler ŞU AN sahip oldukları sorunlara çözüm satın alırlar, bir gün sahip olabilecekleri sorunlara değil.

### Nerede Satılır

**Gumroad** — En basit seçenek. 30 dakikada bir ürün sayfası kur, hemen satmaya başla. Her satıştan %10 alır. Aylık ücret yok.
- En iyi: İlk ürünün için. Talebi test etmek için. 100 doların altındaki basit ürünler için.
- Dezavantaj: Sınırlı özelleştirme. Ücretsiz planda yerleşik ortaklık programı yok.

**Lemon Squeezy** — Kayıtlı Satıcı, yani senin için küresel satış vergisi, KDV ve GST'yi halleder. İşlem başına %5 + $0,50 alır.
- En iyi: Uluslararası satışlar için. 50 doların üzerindeki ürünler için. Abonelik ürünleri için.
- Avantaj: KDV için kayıt olman gerekmez. Her şeyi onlar halleder.
- Dezavantaj: Gumroad'dan biraz daha fazla kurulum gerektirir.
{? if regional.country ?}
- *{= regional.country | fallback("your country") =} için, Lemon Squeezy gibi bir Kayıtlı Satıcı sınır ötesi vergi uyumluluğunu halleder, bu özellikle uluslararası satışlar için değerlidir.*
{? endif ?}

**Kendi Siten** — Maksimum kontrol ve marj. Ödemeler için Stripe Checkout kullan, Vercel/Netlify'da ücretsiz barındır.
- En iyi: Trafiğin olduğunda. 100 doların üzerindeki ürünler için. Marka oluşturmak için.
- Avantaj: %0 platform ücreti (sadece Stripe'ın %2,9 + $0,30'u).
- Dezavantaj: Vergi uyumluluğunu sen hallediyorsun (veya Stripe Tax kullanıyorsun).
{? if regional.payment_processors ?}
- *{= regional.country | fallback("your region") =} bölgesinde kullanılabilir ödeme işlemcileri: {= regional.payment_processors | fallback("Stripe, PayPal") =}. Hangisinin {= regional.currency | fallback("local currency") =} desteklediğini doğrula.*
{? endif ?}

> **Yaygın Hata:** Satacak tek bir ürünün olmadan özel bir mağaza cephesi inşa etmek için iki hafta harcamak. İlk ürünün için Gumroad veya Lemon Squeezy kullan. Talebi doğruladıktan ve çabayı haklı çıkaracak gelirin olduktan sonra kendi sitene geç.

### Fikirden Listelenmeye 48 Saatte

İşte tam sıralama. Zamanlayıcı kur. 48 saatin var.

**Saat 0-2: Ürününü Seç**

Modül S'den Egemen Yığın Belgene bak. Birincil becerilerin neler? Günlük hangi framework'ü kullanıyorsun? Son zamanlarda çok uzun süren hangi kurulumu yaptın?

En iyi ilk ürün, zaten kendin için inşa ettiğin bir şeydir. Üç gün harcadığın o Tauri uygulama iskelesi? O bir ürün. Ekibin için yapılandırdığın CI/CD boru hattı? O bir ürün. Düzgün çalıştırmak için bir hafta sonunu harcadığın Docker kurulumu? Ürün.

**Saat 2-16: Ürünü İnşa Et**

Ürünün kendisi temiz, iyi belgelenmiş olmalı ve belirli bir sorunu çözmeli. İşte minimum:

```
my-product/
  README.md           # Kurulum, kullanım, neler dahil
  LICENSE             # Lisansın (aşağıya bakın)
  CHANGELOG.md        # Sürüm geçmişi
  src/                # Asıl ürün
  docs/               # Gerekiyorsa ek dokümantasyon
  examples/           # Çalışan örnekler
  .env.example        # Uygunsa
```

{? if settings.has_llm ?}
**Dokümantasyon ürünün yarısıdır.** İyi belgelenmiş bir şablon, belgesiz daha iyi bir şablonu her zaman geride bırakır. Dokümantasyon taslağı için yerel LLM'ini ({= settings.llm_model | fallback("your configured model") =}) kullan:
{? else ?}
**Dokümantasyon ürünün yarısıdır.** İyi belgelenmiş bir şablon, belgesiz daha iyi bir şablonu her zaman geride bırakır. Dokümantasyon taslağı için yerel bir LLM kullan (henüz kurmadıysan Modül S'den Ollama'yı kur):
{? endif ?}

```bash
# Kod tabanından başlangıç belgelerini oluştur
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

Sonra çıktıyı düzenle. LLM sana belgelerin %70'ini verir. Senin uzmanlığın kalan %30'u sağlar — nüanslar, tuzaklar, "bu yaklaşımı neden seçtim" bağlamı, yani belgeleri gerçekten kullanışlı yapan şey.

**Saat 16-20: Listelemeyi Oluştur**

Lemon Squeezy mağazanı kur. Ödeme entegrasyonu basit — ürününü oluştur, teslimat için bir webhook ayarla ve yayındasın. Kod örnekleriyle birlikte ödeme platformu kurulum kılavuzu için Modül E, Ders 1'e bakın.

**Saat 20-24: Satış Sayfasını Yaz**

Satış sayfan tam olarak beş bölüme ihtiyaç duyar:

1. **Başlık:** Ürünün ne yaptığı ve kimin için olduğu. "Üretime Hazır Tauri 2.0 Başlangıç Kiti — 40 Saatlik Şablon Kodunu Atla."
2. **Acı noktası:** Hangi sorunu çözdüğü. "Yeni bir Tauri uygulaması için auth, veritabanı, otomatik güncelleme ve CI/CD kurmak günler alır. Bu başlangıç kiti hepsini tek bir `git clone` ile verir."
3. **Neler dahil:** Paketteki her şeyin madde işaretli listesi. Spesifik ol. "14 önceden hazırlanmış bileşen, Stripe faturalandırma entegrasyonu, göçlerle SQLite, çapraz platform derlemeleri için GitHub Actions."
4. **Sosyal kanıt:** Varsa. GitHub yıldızları, referanslar veya "[sen] tarafından yapıldı — [X] yıl üretim [framework] uygulamaları inşa ediyorum."
5. **Eylem çağrısı:** Bir buton. Bir fiyat. "$49 — Anında Erişim Al."

Kopya taslağı için yerel LLM'ini kullan, sonra kendi sesine göre yeniden yaz.

**Saat 24-48: Yumuşak Lansman**

Bu yerlere yayınla (ürününle ilgili olanları seç):

- **Twitter/X:** Ne inşa ettiğini ve nedenini açıklayan konu dizisi. Ekran görüntüsü veya GIF ekle.
- **Reddit:** İlgili subreddit'e yayınla (r/reactjs, r/rust, r/webdev, vb.). Satışçı olma. Ürünü göster, çözdüğü sorunu açıkla, linkini ver.
- **Hacker News:** "Show HN: [Ürün Adı] — [tek satırlık açıklama]." Olgusal tut.
- **Dev.to / Hashnode:** Ürününü kullanan bir tutorial yaz. İnce, değerli tanıtım.
- **İlgili Discord sunucuları:** Uygun kanalda paylaş. Çoğu framework Discord sunucusunun bir #showcase veya #projects kanalı var.

### Dijital Ürünlerini Lisanslamak

Bir lisansa ihtiyacın var. İşte seçeneklerin:

**Kişisel Lisans ($49):** Bir kişi, sınırsız kişisel ve ticari proje. Yeniden dağıtılamaz veya yeniden satılamaz.

**Takım Lisansı ($149):** Aynı takımda en fazla 10 geliştirici. Yeniden dağıtım üzerinde aynı kısıtlamalar.

**Genişletilmiş Lisans ($299):** Son kullanıcılara satılan ürünlerde kullanılabilir (örneğin, şablonunu kullanarak müşterilere satılan bir SaaS oluşturmak).

Ürününe bir `LICENSE` dosyası ekle:

```
[Ürün Adı] Lisans Sözleşmesi
Telif Hakkı (c) [Yıl] [Adın/Şirketin]

Kişisel Lisans — Tek Geliştirici

Bu lisans, satın alana şu hakları verir:
- Bu ürünü sınırsız kişisel ve ticari projede kullanmak
- Kaynak kodunu kendi kullanımı için değiştirmek

Bu lisans şunları yasaklar:
- Kaynak kodunun yeniden dağıtımı (değiştirilmiş veya değiştirilmemiş)
- Lisans satın almamış kişilerle erişimi paylaşmak
- Ürünü yeniden satmak veya satış için türev ürünler oluşturmak

Takım veya genişletilmiş lisanslar için [senin-url] adresini ziyaret et.
```

### Gelir Matematiği

{@ insight cost_projection @}

{= regional.currency_symbol | fallback("$") =}49'luk bir ürünün gerçek matematiğini yapalım:

```
Platform ücreti (Lemon Squeezy, %5 + $0,50):  -$2,95
Ödeme işleme (dahil):                          $0,00
Satış başına gelirin:                           $46,05

Aylık $500'a ulaşmak için:   ayda 11 satış (günde 1'den az)
Aylık $1.000'a ulaşmak için: ayda 22 satış (günde 1'den az)
Aylık $2.000'a ulaşmak için: ayda 44 satış (günde yaklaşık 1,5)
```

Bunlar aktif bir nişte iyi konumlanmış bir ürün için gerçekçi rakamlar.

**Gerçek dünya karşılaştırmaları:**
- **ShipFast** (Marc Lou): ~$199-249 fiyatlı bir Next.js şablonu. İlk 4 ayda $528K üretmiş. Marc Lou toplamda ~$83K/ay üreten 10 dijital ürün yönetiyor. (kaynak: starterstory.com/marc-lou-shipfast)
- **Tailwind UI** (Adam Wathan): İlk 3 günde $500K yapan ve ilk 2 yılda $4M'u geçen bir UI bileşen kütüphanesi. Ancak 2025 sonlarına doğru AI tarafından üretilen UI talep azaltınca gelir yıldan yıla ~%80 düşmüş — başarılı ürünlerin bile evrim geçirmesi gerektiğinin bir hatırlatıcısı. (kaynak: adamwathan.me, aibase.com)

O rakamlara ihtiyacın yok. 11 satışa ihtiyacın var.

### Senin Sıran

{? if stack.primary ?}
1. **Ürününü belirle** (30 dk): Egemen Yığın Belgene bak. Bir {= stack.primary | fallback("your primary stack") =} geliştiricisi olarak, kendin için inşa ettiğin 20+ saat süren ne var? İlk ürünün o. Yaz: ürün adı, çözdüğü sorun, hedef alıcı ve fiyat.
{? else ?}
1. **Ürününü belirle** (30 dk): Egemen Yığın Belgene bak. Kendin için inşa ettiğin 20+ saat süren ne var? İlk ürünün o. Yaz: ürün adı, çözdüğü sorun, hedef alıcı ve fiyat.
{? endif ?}

2. **Minimum uygulanabilir ürünü oluştur** (8-16 saat): Mevcut çalışmanı paketle. README'yi yaz. Örnekler ekle. Temiz yap.

3. **Lemon Squeezy mağazası kur** (30 dk): Hesabını oluştur, ürünü ekle, fiyatlandırmayı yapılandır. Yerleşik dosya teslimatını kullan.

4. **Satış sayfasını yaz** (2 saat): Beş bölüm. İlk taslak için yerel LLM'ini kullan. Kendi sesine göre yeniden yaz.

5. **Yumuşak lansman** (1 saat): Ürününün hedef kitlesine uygun 3 yerde yayınla.

---

## Ders 2: İçerik Parasallaştırma

*"Binlerce insanın öğrenmek için para ödeyeceği şeyleri zaten biliyorsun."*

**İlk dolara kadar süre:** 2-4 hafta
**Devam eden zaman taahhüdü:** Haftada 5-10 saat
**Kar marjı:** %70-95 (platforma bağlı)

### İçerik Ekonomisi

{@ insight stack_fit @}

İçerik parasallaştırma diğer tüm motorlardan farklı çalışır. Başlaması yavaştır ve sonra bileşik büyür. İlk ayın $0 üretebilir. Altıncı ayın $500 üretebilir. On ikinci ayın $3.000 üretebilir. Ve büyümeye devam eder — çünkü içeriğin yarı ömrü günlerle değil, yıllarla ölçülür.

Temel denklem:

```
İçerik Geliri = Trafik x Dönüşüm Oranı x Dönüşüm Başına Gelir

Örnek (teknik blog):
  50.000 aylık ziyaretçi x %2 ortaklık tıklama oranı x $5 ortalama komisyon
  = $5.000/ay

Örnek (bülten):
  5.000 abone x %10'u premium'a dönüşür x $5/ay
  = $2.500/ay

Örnek (YouTube):
  10.000 abone, ~aylık 50K görüntülenme
  = $500-1.000/ay reklam geliri
  + $500-1.500/ay sponsorluklar (10K aboneye ulaştıktan sonra)
  = $1.000-2.500/ay
```

### Kanal 1: Ortaklık Gelirli Teknik Blog

**Nasıl çalışır:** Gerçekten faydalı teknik makaleler yaz. Gerçekten kullandığın ve tavsiye ettiğin araçlara ve hizmetlere ortaklık bağlantıları ekle. Okuyucular tıklayıp satın aldığında, komisyon kazanırsın.

**Geliştirici içeriği için iyi ödeme yapan ortaklık programları:**

| Program | Komisyon | Çerez Süresi | Neden İşe Yarar |
|---------|----------|-------------|-----------------|
| Vercel | Yönlendirme başına $50-500 | 90 gün | Deployment makaleleri okuyan geliştiriciler deploy etmeye hazır |
| DigitalOcean | Yeni müşteri başına $200 ($25+ harcayanlar) | 30 gün | Tutorial'lar doğrudan kayıt yönlendirir |
| AWS / GCP | Değişken, genellikle $50-150 | 30 gün | Altyapı makaleleri altyapı alıcılarını çeker |
| Stripe | 1 yıl boyunca tekrarlayan %25 | 90 gün | Her SaaS tutorial'ı ödemeleri içerir |
| Tailwind UI | Satın almanın %10'u ($30-80) | 30 gün | Frontend tutorial'ları = Tailwind UI alıcıları |
| Lemon Squeezy | 1 yıl boyunca tekrarlayan %25 | 30 gün | Dijital ürün satışı hakkında yazıyorsan |
| JetBrains | Satın almanın %15'i | 30 gün | Geliştirici tutorial'larında IDE tavsiyeleri |
| Hetzner | İlk ödemenin %20'si | 30 gün | Bütçe dostu barındırma tavsiyeleri |

**Gerçek gelir örneği — aylık 50K ziyaretçili bir geliştirici blogu:**

```
Aylık trafik: 50.000 benzersiz ziyaretçi (12-18 ayda ulaşılabilir)

Gelir dağılımı:
  Barındırma ortaklığı (DigitalOcean, Hetzner):    $400-800/ay
  Araç ortaklıkları (JetBrains, Tailwind UI):       $200-400/ay
  Hizmet ortaklıkları (Vercel, Stripe):              $300-600/ay
  Görüntülü reklamlar (Carbon Ads geliştiriciler için): $200-400/ay
  Sponsorlu yazılar (ayda 1-2 tane, $500-1.000):    $500-1.000/ay

Toplam: $1.600-3.200/ay
```

**Geliştiriciler için SEO temelleri (gerçekten fark yaratan şey):**

Pazarlama insanlarından SEO hakkında duyduğun her şeyi unut. Geliştirici içeriği için önemli olan şunlar:

1. **Belirli soruları yanıtla.** "Tauri 2.0 ile SQLite nasıl kurulur" her zaman "Tauri'ye Giriş"i yener. Belirli sorgunun daha az rekabeti ve daha yüksek niyeti var.

2. **Uzun kuyruk anahtar kelimeleri hedefle.** Ahrefs (ücretsiz deneme), Ubersuggest (freemium) gibi bir araç kullan veya sadece Google otomatik tamamlama. Konunu yaz ve Google'ın ne önerdiğine bak.

3. **Çalışan kod ekle.** Google, geliştirici sorguları için kod blokları içeren içeriği önceliklendirir. Eksiksiz, çalışan bir örnek, teorik bir açıklamayı her zaman geride bırakır.

4. **Yıllık güncelle.** Gerçekten güncel olan "2026'da X Nasıl Deploy Edilir" makalesi, 10 kat daha fazla geri bağlantısı olan 2023 makalesini geride bırakır. Başlığına yılı ekle ve güncel tut.

5. **Dahili bağlantılar.** Makalelerini birbirine bağla. Tauri kurulum makalenin altına "İlgili: Tauri uygulamana auth nasıl eklenir." Google bu bağlantıları takip eder.

**İçerik oluşturmayı hızlandırmak için LLM kullanımı:**

4 adımlı süreç: (1) Yerel LLM ile taslak oluştur, (2) Her bölümü yerel olarak yaz (bedava), (3) SENİN uzmanlığını ekle — tuzaklar, görüşler ve LLM'in sağlayamayacağı "üretimde gerçekten ne kullanıyorum" bilgisi, (4) Müşteriye yönelik kalite için API modeliyle cilala.

LLM işin %70'ini halleder. Senin uzmanlığın, insanların okumasını, güvenmesini ve ortaklık bağlantılarına tıklamasını sağlayan %30'dur.

> **Yaygın Hata:** LLM tarafından üretilen içeriği önemli düzenleme yapmadan yayınlamak. Okuyucular anlayabilir. Google anlayabilir. Ve ortaklık bağlantılarının dönüştürmesini sağlayan güveni oluşturmaz. LLM olmadan üzerine adını koymazdıysan, LLM ile de koyma.

**Beklentilerini kalibre etmek için gerçek dünya bülten karşılaştırmaları:**
- **TLDR Newsletter** (Dan Ni): 1,2M+ abone, yılda $5-6,4M üretiyor. Sponsor yerleştirme başına $18K'a kadar ücret alıyor. Orijinal habercilik değil, kürasyon üzerine kurulu. (kaynak: growthinreverse.com/tldr)
- **Pragmatic Engineer** (Gergely Orosz): 400K+ abone, yalnızca $15/ay abonelikten yılda $1,5M+. Sıfır sponsor — saf abone geliri. (kaynak: growthinreverse.com/gergely)
- **Cyber Corsairs AI** (Beehiiv vaka çalışması): 1 yıldan kısa sürede 50K aboneye ve aylık $16K'a ulaştı, odaklı nişlerde yeni girenlerin hâlâ atılım yapabileceğini gösteriyor. (kaynak: blog.beehiiv.com)

Bunlar tipik sonuçlar değil — en iyi performans gösterenler. Ama modelin büyük ölçekte çalıştığını ve gelir tavanının gerçek olduğunu kanıtlıyorlar.

### Kanal 2: Premium Katmanlı Bülten

**Platform karşılaştırması:**

| Platform | Ücretsiz Katman | Ücretli Özellikler | Ücretli Aboneliklerden Pay | En İyi |
|----------|----------------|--------------------|-----------------------------|--------|
| **Substack** | Sınırsız abone | Ücretli abonelikler yerleşik | %10 | Maksimum erişim, kolay kurulum |
| **Beehiiv** | 2.500 abone | Özel alan adları, otomasyonlar, yönlendirme programı | %0 (her şeyi sen alırsın) | Büyüme odaklı, profesyonel |
| **Buttondown** | 100 abone | Özel alan adları, API, markdown-native | %0 | Geliştiriciler, minimalistler |
| **Ghost** | Self-hosted (ücretsiz) | Tam CMS + üyelik | %0 | Tam kontrol, SEO, uzun vadeli marka |
| **ConvertKit** | 10.000 abone | Otomasyonlar, diziler | %0 | Kurs/ürün de satıyorsan |

**Geliştiriciler için tavsiye:** Beehiiv (büyüme özellikleri, gelir payı yok) veya Ghost (tam kontrol, en iyi SEO).

**LLM destekli bülten boru hattı:**

```python
#!/usr/bin/env python3
"""newsletter_pipeline.py — Yarı otomatik bülten üretimi."""
import requests, json
from datetime import datetime

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
NICHE = "Rust ecosystem and systems programming"  # ← Bunu değiştir

def fetch_hn_stories(limit=30) -> list[dict]:
    """En popüler HN haberlerini getir. RSS feed'leri, Reddit API vb. ile değiştir/genişlet."""
    story_ids = requests.get("https://hacker-news.firebaseio.com/v0/topstories.json").json()[:limit]
    return [requests.get(f"https://hacker-news.firebaseio.com/v0/item/{sid}.json").json()
            for sid in story_ids]

def classify_and_summarize(items: list[dict]) -> list[dict]:
    """Yerel LLM kullanarak alaka düzeyini puanla ve özetler oluştur."""
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
    """Bülten iskeletini oluştur — sen düzenle ve uzmanlığını ekle."""
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
    print(f"Taslak: {filename} — ŞİMDİ uzmanlığını ekle, hataları düzelt, yayınla.")
```

**Zaman yatırımı:** Boru hattı kurulduktan sonra haftada 3-4 saat. LLM kürasyon ve taslağı halleder. Sen düzenleme, içgörü ve abonelerin para ödediği kişisel sesi hallediyorsun.

### Kanal 3: YouTube

YouTube parasallaştırması en yavaş ama en yüksek tavana sahip. YouTube'daki geliştirici içeriği kronik olarak yetersiz — talep arzı çok aşıyor.

**Gelir zaman çizelgesi (gerçekçi):**

```
Ay 1-3:    $0 (kütüphane oluşturma, henüz parasallaştırılmamış)
Ay 4-6:    $50-200/ay (1.000 abone + 4.000 izlenme saatinde reklam geliri başlar)
Ay 7-12:   $500-1.500/ay (reklam geliri + ilk sponsorluklar)
Yıl 2:     $2.000-5.000/ay (tekrarlayan sponsorlu yerleşik kanal)
```

**2026'da geliştirici YouTube'da ne işe yarıyor:**

1. **"Y ile X İnşa Et" tutorial'ları** (15-30 dk) — "Rust'ta CLI Aracı İnşa Et," "Yerel AI API'si İnşa Et"
2. **Araç karşılaştırmaları** — "2026'da Tauri vs Electron — Hangisini Kullanmalısın?"
3. **"X'i 30 gün denedim"** — "Tüm Bulut Hizmetlerimi Kendi Barındırdığım Alternatiflerle Değiştirdim"
4. **Mimari derinlemesine incelemeler** — "Günde 1M Etkinlik İşleyen Bir Sistem Nasıl Tasarladım"
5. **"Ne Öğrendim" retrospektifleri** — "6 Ay Dijital Ürün Satışı — Gerçek Rakamlar"

**İhtiyacın olan ekipman:**

```
Minimum (buradan başla):
  Ekran kaydı: OBS Studio ($0)
  Mikrofon: Herhangi bir USB mikrofon ($30-60) — veya kulaklık mikrofonun
  Düzenleme: DaVinci Resolve ($0) veya CapCut ($0)
  Toplam: $0-60

Konforlu (gelir haklı çıkardığında yükselt):
  Mikrofon: Blue Yeti veya Audio-Technica AT2020 ($100-130)
  Kamera: Logitech C920 ($70) — istersen yüz kamerası için
  Toplam: $170-200
```

> **Gerçek Konuşma:** Geliştirici içeriği için ses kalitesi, video kalitesinden 10 kat daha önemli. Çoğu izleyici dinliyor, izlemiyor. $30'luk USB mikrofon + OBS başlamak için yeterli. İlk 10 videon iyi içerik ve makul ses ise, abone kazanırsın. Kötü içerik ve $2.000'lık kamera kurulumuyla olmaz.

### Senin Sıran

1. **İçerik kanalını seç** (15 dk): Blog, bülten veya YouTube. BİRİNİ seç. Üçünü birden yapmaya çalışma. Beceriler farklı ve zaman taahhüdü hızla bileşiyor.

{? if stack.primary ?}
2. **Nişini tanımla** (30 dk): "Programlama" değil. "Web geliştirme" değil. {= stack.primary | fallback("primary stack") =} uzmanlığını kullanan spesifik bir şey. "Backend geliştiriciler için Rust." "Yerel öncelikli masaüstü uygulamaları oluşturmak." "Küçük işletmeler için AI otomasyonu." Ne kadar spesifik olursa, o kadar hızlı büyürsün.
{? else ?}
2. **Nişini tanımla** (30 dk): "Programlama" değil. "Web geliştirme" değil. Spesifik bir şey. "Backend geliştiriciler için Rust." "Yerel öncelikli masaüstü uygulamaları oluşturmak." "Küçük işletmeler için AI otomasyonu." Ne kadar spesifik olursa, o kadar hızlı büyürsün.
{? endif ?}

3. **İlk içerik parçanı oluştur** (4-8 saat): Bir blog yazısı, bir bülten sayısı veya bir YouTube videosu. Gönder. Mükemmelliği bekleme.

4. **Parasallaştırma altyapısını kur** (1 saat): 2-3 ilgili ortaklık programına kaydol. Bülten platformunu kur. Veya sadece yayınla ve parasallaştırmayı sonra ekle — önce içerik, sonra gelir.

5. **Bir programa bağlan** (5 dk): Haftalık herhangi bir içerik kanalı için minimum. Yaz: "Her [gün] [saatte] yayınlıyorum." Kitlen tutarlılıkla büyür, kaliteyle değil.

---

## Ders 3: Mikro-SaaS

*"Belirli bir insan grubu için tek bir sorunu çözen ve ayda $9-29 ödemeye istekle razı olacakları küçük bir araç."*

**İlk dolara kadar süre:** 4-8 hafta
**Devam eden zaman taahhüdü:** Haftada 5-15 saat
**Kar marjı:** %80-90 (barındırma + API maliyetleri)

### Mikro-SaaS'ı Farklı Kılan Ne

{@ insight stack_fit @}

Bir mikro-SaaS bir startup değil. Risk sermayesi aramıyor. Bir sonraki Slack olmaya çalışmıyor. Bir mikro-SaaS, küçük, odaklı bir araçtır:

- Tam olarak bir sorunu çözer
- Ayda $9-29 ücretlendirir
- Tek bir kişi tarafından inşa edilip bakımı yapılabilir
- Çalıştırması ayda $20-100'e mal olur
- Ayda $500-5.000 gelir üretir

Güzellik kısıtlamalarda. Bir sorun. Bir kişi. Bir fiyat noktası.

**Gerçek dünya mikro-SaaS karşılaştırmaları:**
- **Pieter Levels** (Nomad List, PhotoAI, vb.): Sıfır çalışanla yılda ~$3M. Tek başına PhotoAI ayda $132K'a ulaşmış. Solo kurucu mikro-SaaS modelini ölçekte kanıtlıyor. (kaynak: fast-saas.com)
- **Bannerbear** (Jon Yongfook): Tek bir kişi tarafından $50K+ MRR'a çıkarılmış bir görüntü üretim API'si. (kaynak: indiepattern.com)
- **Gerçeklik kontrolü:** Mikro-SaaS ürünlerinin %70'i ayda $1K'nın altında gelir üretiyor. Yukarıdaki hayatta kalanlar aykırı değerler. İnşa etmeden önce doğrula ve ödeme yapan müşterilerin olana kadar maliyetlerini sıfıra yakın tut. (kaynak: softwareseni.com)

### Mikro-SaaS Fikrini Bulmak

{? if dna.top_engaged_topics ?}
En çok zaman harcadığın konulara bak: {= dna.top_engaged_topics | fallback("your most-engaged topics") =}. En iyi mikro-SaaS fikirleri, o alanlarda kişisel olarak yaşadığın sorunlardan gelir. Ama onları bulmak için bir çerçeveye ihtiyacın varsa, işte bir tane:
{? else ?}
En iyi mikro-SaaS fikirleri, kişisel olarak yaşadığın sorunlardan gelir. Ama onları bulmak için bir çerçeveye ihtiyacın varsa, işte bir tane:
{? endif ?}

**"Elektronik Tablo Değişimi" Yöntemi:**

Birinin bir elektronik tablo, manuel bir süreç veya bir araya getirilmiş ücretsiz araçlar kullandığı ve tek basit bir uygulama olması gereken herhangi bir iş akışını bul. Mikro-SaaS'ın o.

Örnekler:
- Freelancer'lar müşteri projelerini Google Sheets'te takip ediyor → **Freelancer'lar için proje takipçisi** ($12/ay)
- Geliştiriciler yan projelerinin hâlâ çalışıp çalışmadığını manuel kontrol ediyor → **Bağımsız geliştiriciler için durum sayfası** ($9/ay)
- İçerik oluşturucular birden fazla platforma manuel çapraz paylaşım yapıyor → **Çapraz paylaşım otomasyonu** ($15/ay)
- Küçük takımlar API anahtarlarını Slack mesajlarında paylaşıyor → **Takım gizli bilgi yöneticisi** ($19/ay)

**"Berbat Ücretsiz Araç" Yöntemi:**

İnsanların ücretsiz olduğu için gönülsüzce kullandığı ama kötü olduğu için nefret ettiği ücretsiz bir araç bul. Daha iyi bir sürümünü $9-29/ay'a inşa et.

**"Forum Madenciliği" Yöntemi:**

Reddit, HN ve niş Discord sunucularında şunları ara:
- "Şöyle bir araç var mı..."
- "Keşke şöyle bir şey olsa..."
- "Şunu arıyordum..."
- "İyi bir ... bilen var mı..."

50+ kişi soruyorsa ve cevaplar "pek sayılmaz" veya "bir elektronik tablo kullanıyorum" ise, o bir mikro-SaaS.

### Gelir Potansiyelli Gerçek Mikro-SaaS Fikirleri

| Fikir | Hedef Kullanıcı | Fiyat | 100 Müşteride Gelir |
|-------|----------------|-------|---------------------|
| GitHub PR analitik dashboard'u | Mühendislik yöneticileri | $19/ay | $1.900/ay |
| Güzel durum sayfalarıyla çalışma süresi monitörü | Bağımsız geliştiriciler, küçük SaaS | $9/ay | $900/ay |
| Git commit'lerinden değişiklik günlüğü oluşturucu | Geliştirici takımları | $12/ay | $1.200/ay |
| Geliştirici dostu analitiğe sahip URL kısaltıcı | Teknoloji şirketlerindeki pazarlamacılar | $9/ay | $900/ay |
| Küçük takımlar için API anahtar yöneticisi | Startup'lar | $19/ay | $1.900/ay |
| Cron görevi izleme ve uyarı | DevOps mühendisleri | $15/ay | $1.500/ay |
| Webhook test ve hata ayıklama aracı | Backend geliştiriciler | $12/ay | $1.200/ay |
| MCP sunucu dizini ve pazarı | AI geliştiriciler | Reklam destekli + öne çıkan listeleme $49/ay | Değişken |

### Mikro-SaaS İnşa Etmek: Tam Yol Haritası

Gerçek bir tane inşa edelim. Basit bir çalışma süresi izleme hizmeti inşa edeceğiz — çünkü basit, kullanışlı ve tam yığını gösteriyor.

**Teknoloji yığını (solo geliştirici için optimize edilmiş):**

```
Backend:    Hono (hafif, hızlı, TypeScript)
Veritabanı: Turso (SQLite tabanlı, cömert ücretsiz katman)
Auth:       Lucia (basit, kendi barındırmalı auth)
Ödemeler:   Stripe (abonelikler)
Barındırma: Vercel (fonksiyonlar için ücretsiz katman)
Açılış:     Aynı Vercel projesinde statik HTML
İzleme:     Kendi ürünün (kendi mamanı ye)
```

**Lansmanda aylık maliyetler:**
```
Vercel:       $0 (ücretsiz katman — aylık 100K fonksiyon çağrısı)
Turso:        $0 (ücretsiz katman — 9GB depolama, aylık 500M satır okuma)
Stripe:       İşlem başına %2,9 + $0,30 (sadece ödeme aldığında)
Alan adı:     $1/ay (yılda $12)
Toplam:       Ölçeklendirme gerekene kadar $1/ay
```

**Temel API kurulumu:**

```typescript
// src/index.ts — Çalışma süresi monitörü için Hono API'si
import { Hono } from "hono";
import { cors } from "hono/cors";
import { jwt } from "hono/jwt";
import Stripe from "stripe";

const app = new Hono();
const stripe = new Stripe(process.env.STRIPE_SECRET_KEY!);
const PLAN_LIMITS = { free: 3, starter: 10, pro: 50 };

app.use("/api/*", cors());
app.use("/api/*", jwt({ secret: process.env.JWT_SECRET! }));

// Monitör oluştur (plan bazlı limitlerle)
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

// Kullanıcının tüm monitörlerini getir
app.get("/api/monitors", async (c) => {
  const userId = c.get("jwtPayload").sub;
  return c.json(await db.getMonitors(userId));
});

// Abonelik yönetimi için Stripe webhook
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

// İzleme worker'ı — cron zamanlamasıyla çalışır (Vercel cron, Railway cron, vb.)
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

  // Sonuçları depola ve durum değişikliklerinde uyar (çalışıyor → durdu veya durdu → çalışıyor)
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

**Stripe abonelik kurulumu (bir kez çalıştır):**

```typescript
// stripe-setup.ts — Ürününü ve fiyatlandırma katmanlarını oluştur
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

  console.log(`Starter: ${starter.id} ($9/ay) | Pro: ${pro.id} ($19/ay)`);

  // Checkout'unda kullan:
  // const session = await stripe.checkout.sessions.create({
  //   mode: 'subscription',
  //   line_items: [{ price: starter.id, quantity: 1 }],
  //   success_url: 'https://yourapp.com/dashboard?upgraded=true',
  //   cancel_url: 'https://yourapp.com/pricing',
  // });
}
createPricing().catch(console.error);
```

### Birim Ekonomisi

Herhangi bir mikro-SaaS inşa etmeden önce, rakamları hesapla:

```
Müşteri Edinme Maliyeti (CAC):
  Organik pazarlama yapıyorsan (blog, Twitter, HN): ~$0
  Reklam veriyorsan: deneme kaydı başına $10-50, ödeme yapan müşteri başına $30-150

  Hedef: CAC < 3 aylık abonelik geliri
  Örnek: $30 CAC, $12/ay fiyat → 2,5 ayda geri ödeme ✓

Müşteri Yaşam Boyu Değeri (LTV):
  LTV = Aylık Fiyat x Ortalama Müşteri Ömrü (ay)

  Mikro-SaaS için ortalama kayıp oranı aylık %5-8
  Ortalama ömür = 1 / kayıp oranı
  %5 kayıpta: 1/0,05 = 20 ay → $12/ay'da LTV = $240
  %8 kayıpta: 1/0,08 = 12,5 ay → $12/ay'da LTV = $150

  Hedef: LTV/CAC oranı > 3

Aylık Yakma:
  Barındırma (Vercel/Railway): $0-20
  Veritabanı (Turso/PlanetScale): $0-20
  E-posta gönderimi (Resend): $0
  İzleme (kendi ürünün): $0
  Alan adı: $1

  Toplam: $1-41/ay

  Başabaş: 1-5 müşteri ($9/ay'da)
```

> **Yaygın Hata:** Başabaşa ulaşmak için 500 müşteri gerektiren bir mikro-SaaS inşa etmek. Altyapın ayda $200'a mal oluyorsa ve ayda $9 ücretlendiriyorsan, sadece maliyetleri karşılamak için 23 müşteriye ihtiyacın var. Her şey için ücretsiz katmanlarla başla. İlk müşterinin ödemesi saf kâr olmalı, altyapıyı karşılamak değil.

### Senin Sıran

1. **Fikrini bul** (2 saat): "Elektronik Tablo Değişimi" veya "Forum Madenciliği" yöntemini kullan. 3 potansiyel mikro-SaaS fikri belirle. Her biri için yaz: sorun, hedef kullanıcı, fiyat ve aylık $1.000 gelir için kaç müşteriye ihtiyacın olduğu.

2. **İnşa etmeden önce doğrula** (1-2 gün): En iyi fikrin için 5-10 potansiyel müşteri bul ve onlara sor: "[X] yapıyorum. Bunun için ayda $[Y] öder misiniz?" Çözümü tarif etme — sorunu tarif et ve gözlerinin parladığını gör.

3. **MVP'yi inşa et** (2-4 hafta): Sadece temel işlevsellik. Auth, aracının yaptığı tek şey ve Stripe faturalandırma. Başka bir şey yok. Yönetici paneli yok. Takım özellikleri yok. API yok. Bir kullanıcı, bir fonksiyon, bir fiyat.

{? if computed.os_family == "windows" ?}
4. **Deploy et ve lansmanı yap** (1 gün): Vercel veya Railway'e deploy et. Windows'ta, gerekirse Docker tabanlı deployment'lar için WSL kullan. Alan adını satın al. Açılış sayfası kur. 3-5 ilgili toplulukta paylaş.
{? elif computed.os_family == "macos" ?}
4. **Deploy et ve lansmanı yap** (1 gün): Vercel veya Railway'e deploy et. macOS, Docker Desktop aracılığıyla Docker deployment'ını basit hale getirir. Alan adını satın al. Açılış sayfası kur. 3-5 ilgili toplulukta paylaş.
{? else ?}
4. **Deploy et ve lansmanı yap** (1 gün): Vercel veya Railway'e deploy et. Alan adını satın al. Açılış sayfası kur. 3-5 ilgili toplulukta paylaş.
{? endif ?}

5. **Birim ekonomilerini takip et** (sürekli): Birinci günden itibaren CAC, kayıp oranı ve MRR'ı takip et. Rakamlar 10 müşteride işe yaramıyorsa, 100'de de yaramaz.

---

## Ders 4: Hizmet Olarak Otomasyon

*"İşletmeler araçlarını birbirine bağlaman için sana binlerce dolar ödeyecek."*

**İlk dolara kadar süre:** 1-2 hafta
**Devam eden zaman taahhüdü:** Değişken (proje bazlı)
**Kar marjı:** %80-95 (zamanın ana maliyet)

### Otomasyon Neden Bu Kadar İyi Ödüyor

{@ insight stack_fit @}

Çoğu işletmenin, çalışan zamanından haftada 10-40 saate mal olan manuel iş akışları var. Bir resepsiyonist form gönderimlerini CRM'e manuel giriyor. Bir muhasebeci e-postalardan fatura verilerini QuickBooks'a kopyalayıp yapıştırıyor. Bir pazarlama müdürü içeriği beş platforma manuel olarak çapraz yayınlıyor.

Bu işletmeler otomasyonun var olduğunu biliyor. Zapier'i duymuşlar. Ama bunu kendileri kuramıyorlar — ve Zapier'in önceden hazır entegrasyonları nadiren onların spesifik iş akışını mükemmel şekilde karşılıyor.

İşte burada sen devreye giriyorsun. Onlara haftada 10-40 saat tasarruf sağlayan özel bir otomasyon inşa etmek için $500-$5.000 ücretlendiriyorsun. O çalışanın saatlik maliyeti $20 bile olsa, onlara ayda $800-$3.200 tasarruf ettiriyorsun. Tek seferlik $2.500'luk ücretin kendini bir ayda amorti ediyor.

Bu, tüm kurstaki en kolay satışlardan biri.

### Gizlilik Satış Argümanı

{? if settings.has_llm ?}
İşte Modül S'deki yerel LLM yığının bir silaha dönüştüğü yer. Zaten {= settings.llm_model | fallback("a model") =} modelini yerel olarak çalıştırıyorsun — çoğu otomasyon ajansının sahip olmadığı altyapı bu.
{? else ?}
İşte Modül S'deki yerel LLM yığının bir silaha dönüştüğü yer. (Henüz yerel bir LLM kurmadıysan, Modül S, Ders 3'e geri dön. Bu, premium fiyatlı otomasyon çalışmasının temelidir.)
{? endif ?}

Çoğu otomasyon ajansı bulut tabanlı AI kullanır. Müşterinin verisi Zapier'den, sonra OpenAI'ya, sonra geri gider. Birçok işletme için — özellikle hukuk firmaları, sağlık kuruluşları, finansal danışmanlar ve AB merkezli herhangi bir şirket — bu başlangıçta reddedilir.

{? if regional.country == "US" ?}
Senin sunumun: **"Verilerinizi gizli olarak işleyen otomasyonlar inşa ediyorum. Müşteri kayıtlarınız, faturalarınız ve iletişimleriniz asla altyapınızdan ayrılmaz. Üçüncü taraf AI işlemcisi yok. Tam HIPAA/SOC 2 uyumluluğu."**
{? else ?}
Senin sunumun: **"Verilerinizi gizli olarak işleyen otomasyonlar inşa ediyorum. Müşteri kayıtlarınız, faturalarınız ve iletişimleriniz asla altyapınızdan ayrılmaz. Üçüncü taraf AI işlemcisi yok. GDPR ve yerel veri koruma düzenlemeleriyle tam uyumluluk."**
{? endif ?}

Bu sunum, bulut otomasyon ajanslarının dokunamayacağı anlaşmaları kapatır. Ve bunun için prim ücretlendirebilirsin.

### Fiyatlandırmalı Gerçek Proje Örnekleri

**Proje 1: Bir Emlak Ajansı İçin Lead Değerlendirici — $3.000**

```
Sorun: Ajans, web sitesi, e-posta ve sosyal medya aracılığıyla haftada 200+
       sorgulama alıyor. Ajanlar niteliksiz lead'lere (geziciler, bölge dışı,
       ön onaylı değil) yanıt vererek zaman harcıyor.

Çözüm:
  1. Webhook tüm sorgulama kaynaklarını tek bir kuyrukta toplar
  2. Yerel LLM her lead'i sınıflandırır: Sıcak / Ilık / Soğuk / Spam
  3. Sıcak lead'ler: atanmış ajana SMS ile anında bildirim
  4. Ilık lead'ler: ilgili ilanlarla otomatik yanıt ve takip planla
  5. Soğuk lead'ler: besleyici e-posta dizisine ekle
  6. Spam: sessizce arşivle

Araçlar: n8n (kendi barındırmalı), Ollama, Twilio (SMS için), mevcut CRM API'leri

İnşa süresi: 15-20 saat
Senin maliyetin: ~$0 (kendi barındırmalı araçlar + onların altyapısı)
Onların tasarrufu: ~haftada 20 saat ajan zamanı = ayda $2.000+
```

**Proje 2: Bir Hukuk Firması İçin Fatura İşleyici — $2.500**

```
Sorun: Firma ayda 50-100 satıcı faturası alıyor, PDF ekleri olarak.
       Hukuk asistanı her birini faturalandırma sistemlerine manuel giriyor.
       Ayda 10+ saat sürüyor. Hataya açık.

Çözüm:
  1. E-posta kuralı faturaları işleme gelen kutusuna yönlendirir
  2. PDF çıkarma metni çeker (pdf-extract veya OCR)
  3. Yerel LLM çıkarır: satıcı, tutar, tarih, kategori, faturalandırma kodu
  4. Yapılandırılmış veri, faturalandırma sistemi API'sine gönderilir
  5. İstisnalar (düşük güvenilirlikli çıkarmalar) inceleme kuyruğuna gider
  6. Yönetici ortağa haftalık özet e-posta

Araçlar: Özel Python script'i, Ollama, e-posta API'leri, faturalandırma sistemi API'si

İnşa süresi: 12-15 saat
Senin maliyetin: ~$0
Onların tasarrufu: ~ayda 10 saat hukuk asistanı zamanı + daha az hata
```

**Proje 3: Bir Pazarlama Ajansı İçin İçerik Yeniden Kullanım Boru Hattı — $1.500**

```
Sorun: Ajans her müşteri için haftada bir uzun biçimli blog yazısı oluşturuyor.
       Sonra her makaleden sosyal medya parçacıkları, e-posta özetleri ve
       LinkedIn paylaşımları manuel olarak oluşturuyor. Makale başına 5 saat sürüyor.

Çözüm:
  1. Yeni blog yazısı boru hattını tetikler (RSS veya webhook)
  2. Yerel LLM oluşturur:
     - 5 Twitter/X paylaşımı (farklı açılar, farklı kancalar)
     - 1 LinkedIn paylaşımı (daha uzun, profesyonel ton)
     - 1 e-posta bülteni özeti
     - 3 Instagram açıklama seçeneği
  3. Tüm oluşturulan içerik inceleme dashboard'una gider
  4. İnsan inceler, düzenler ve Buffer/Hootsuite üzerinden zamanlar

Araçlar: n8n, Ollama, Buffer API

İnşa süresi: 8-10 saat
Senin maliyetin: ~$0
Onların tasarrufu: ~makale başına 4 saat x haftada 4 makale = haftada 16 saat
```

### Otomasyon İnşa Etmek: n8n Örneği

n8n, kendi barındırabileceğin açık kaynaklı bir iş akışı otomasyon aracıdır (`docker run -d --name n8n -p 5678:5678 n8nio/n8n`). Profesyonel seçimdir çünkü müşteri verileri senin/onların altyapısında kalır.

{? if stack.contains("python") ?}
Daha basit deployment'lar için, işte aynı fatura işleme saf Python script'i olarak — tam senin uzmanlık alanın:
{? else ?}
Daha basit deployment'lar için, işte aynı fatura işleme saf Python script'i olarak (Python, birincil yığının olmasa bile otomasyon çalışması için standart):
{? endif ?}

```python
#!/usr/bin/env python3
"""
invoice_processor.py — Otomatik fatura veri çıkarma.
Yerel LLM kullanarak PDF faturaları işler, yapılandırılmış veri çıktısı verir.
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
    if not pdfs: return print("İşlenecek fatura yok.")

    for pdf_path in pdfs:
        text = extract_text_from_pdf(pdf_path)
        if not text.strip():
            pdf_path.rename(REVIEW_DIR / pdf_path.name); continue

        invoice = extract_invoice_data(text, pdf_path.name)
        dest = REVIEW_DIR if invoice.needs_review else PROCESSED_DIR
        pdf_path.rename(dest / pdf_path.name)

        with open("./invoices/extracted.jsonl", "a") as f:
            f.write(json.dumps(asdict(invoice)) + "\n")
        print(f"  {'İnceleme' if invoice.needs_review else 'Tamam'}: "
              f"{invoice.vendor} ${invoice.amount:.2f} ({invoice.confidence:.0%})")

if __name__ == "__main__":
    process_invoices()
```

### Otomasyon Müşterisi Bulmak

**LinkedIn (otomasyon müşterisi bulmak için en iyi yatırım getirisi):**

1. Başlığını şu şekilde değiştir: "Sıkıcı iş süreçlerini otomatikleştiriyorum | Gizlilik öncelikli AI otomasyon"
2. Haftada 2-3 kez otomasyon sonuçları hakkında paylaşım yap: "[müşteri tipi] için [süreci] otomatikleştirerek haftada 15 saat tasarruf sağladım. Hiçbir veri altyapılarından çıkmadı."
3. Hedef sektörlerin için LinkedIn gruplarına katıl (emlakçılar, hukuk firması yöneticileri, pazarlama ajansı sahipleri)
4. Bölgendeki küçük işletme sahiplerine günde 5-10 kişiselleştirilmiş bağlantı isteği gönder

**Yerel iş ağları:**

- Ticaret Odası etkinlikleri (birine katıl, "iş süreçlerini otomatikleştirdiğinden" bahset)
- BNI (Business Network International) grupları
- Ortak çalışma alanı toplulukları

**Upwork (ilk 2-3 projen için):**

Şunları ara: "automation," "data processing," "workflow automation," "Zapier expert," "API integration." Günde 5 projeye spesifik, ilgili tekliflerle başvur. İlk 2-3 projen değerlendirme oluşturmak için daha düşük ücretlerde ($500-1.000) olacak. Sonrasında piyasa fiyatı ücretlendir.

### Otomasyon Sözleşme Şablonu

Her zaman bir sözleşme kullan. Sözleşmenin minimum şu 7 bölüme ihtiyacı var:

1. **İş Kapsamı** — Spesifik açıklama + çıktı listesi + dokümantasyon
2. **Zaman Çizelgesi** — Tahmini tamamlanma günleri, başlangıç tarihi = ön ödeme alındığında
3. **Fiyatlandırma** — Toplam ücret, %50 peşin (iade edilemez), %50 teslimatta
4. **Veri İşleme** — "Tüm veriler yerel olarak işlenir. Üçüncü taraf hizmet yok. Geliştirici, tamamlanmadan sonra 30 gün içinde tüm müşteri verilerini siler."
5. **Revizyonlar** — 2 tur dahil, ek $150/saat
6. **Bakım** — Hata düzeltme ve izleme için opsiyonel retainer
7. **Fikri Mülkiyet** — Müşteri otomasyonun sahibidir. Geliştirici genel kalıpları yeniden kullanma hakkını saklı tutar.

{? if regional.business_entity_type ?}
Başlangıç noktası olarak Avodocs.com veya Bonsai'den ücretsiz bir şablon kullan, sonra veri işleme maddesini (bölüm 4) ekle — çoğu şablonun kaçırdığı ve senin rekabet avantajın olan madde bu. {= regional.country | fallback("your country") =} için, sözleşme başlığında {= regional.business_entity_type | fallback("business entity") =} kullan.
{? else ?}
Başlangıç noktası olarak Avodocs.com veya Bonsai'den ücretsiz bir şablon kullan, sonra veri işleme maddesini (bölüm 4) ekle — çoğu şablonun kaçırdığı ve senin rekabet avantajın olan madde bu.
{? endif ?}

> **Gerçek Konuşma:** %50 peşin ön ödeme tartışılmaz. Seni kapsam kaymasından ve teslimat sonrası ortadan kaybolan müşterilerden korur. Bir müşteri %50 peşin ödemeyecekse, sonra %100 ödeyecek bir müşteri de değildir.

### Senin Sıran

1. **3 potansiyel otomasyon projesi belirle** (1 saat): Etkileşimde olduğun işletmeleri düşün (dişçin, ev sahibinin yönetim şirketi, gittiğin kahve dükkanı, berberin). Otomatikleştirebileceğin hangi manuel süreçleri yapıyorlar?

2. **Birini fiyatlandır** (30 dk): Hesapla: inşa etmek kaç saatini alacak, müşteri için değeri nedir (tasarruf edilen saatler x o saatlerin saatlik maliyeti) ve adil bir fiyat nedir? Fiyatın, yarattığın tasarrufun 1-3 ayı olmalı.

3. **Bir demo oluştur** (4-8 saat): Yukarıdaki fatura işleyiciyi al ve hedef sektörün için özelleştir. Onu çalışırken gösteren 2 dakikalık bir ekran kaydı çek. Bu demo senin satış aracın.

4. **5 potansiyel müşteriye ulaş** (2 saat): LinkedIn, e-posta veya yerel bir işletmeye gir. Onlara demoyu göster. Manuel süreçleri hakkında sor.

5. **Sözleşme şablonunu hazırla** (30 dk): Yukarıdaki şablonu kendi bilgilerinle özelleştir. Bir müşteri evet dediği gün gönderebilecek şekilde hazır tut.

---

## Ders 5: API Ürünleri

*"Yerel LLM'ini gelir üreten bir uç noktaya dönüştür."*

**İlk dolara kadar süre:** 2-4 hafta
**Devam eden zaman taahhüdü:** Haftada 5-10 saat (bakım + pazarlama)
**Kar marjı:** %70-90 (işlem maliyetlerine bağlı)

### API Ürün Modeli

{@ insight stack_fit @}

Bir API ürünü, bir yeteneklik — genellikle özel işleme ile yerel LLM'ini — diğer geliştiricilerin kullanmak için ödediği temiz bir HTTP uç noktasının arkasına sarar. Sen altyapıyı, modeli ve alan uzmanlığını hallediyorsun. Onlar basit bir API çağrısı alıyor.

Bu, backend çalışmasına rahat olan geliştiriciler için bu kurstaki en ölçeklenebilir motor. İnşa edildikten sonra, her yeni müşteri minimum ek maliyetle gelir ekler.

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("GPU") =} ile, geliştirme sırasında ve ilk müşterilerin için çıkarım katmanını yerel olarak çalıştırabilir, ölçeklendirme gerekene kadar maliyetleri sıfırda tutabilirsin.
{? endif ?}

### İyi Bir API Ürününü Ne Yapar

Her API'ye para ödenmeye değmez. Geliştiriciler bir API'ye şu durumlarda para öder:

1. **Maliyetinden daha fazla zaman tasarruf sağladığında.** Aylık $29'luk özgeçmiş ayrıştırma API'in takımlarına ayda 20 saatlik manuel işten tasarruf sağlıyor. Kolay satış.
2. **Kendi başlarına kolayca yapamayacakları bir şey yaptığında.** İnce ayarlı model, özel veri seti veya karmaşık işleme boru hattı.
3. **Şirket içi inşa etmekten daha güvenilir olduğunda.** Bakımlı, belgelenmiş, izlenen. Bir LLM deployment'ına bakıcılık yapmak istemiyorlar.

**Fiyatlandırmalı gerçek API ürün fikirleri:**

| API Ürünü | Hedef Müşteri | Fiyatlandırma | Neden Öderler |
|-----------|--------------|---------------|---------------|
| Kod inceleme API'si (özel standartlara karşı kontrol) | Geliştirici takımları | Takım başına $49/ay | Kıdemli geliştirici darboğazı olmadan tutarlı incelemeler |
| Özgeçmiş ayrıştırıcı (PDF özgeçmişlerden yapılandırılmış veri) | İK teknoloji şirketleri, ATS geliştiricileri | 500 ayrıştırma başına $29/ay | Özgeçmişleri güvenilir şekilde ayrıştırmak şaşırtıcı derecede zor |
| Belge sınıflandırıcı (hukuki, finansal, tıbbi) | Belge yönetim sistemleri | 1000 belge başına $99/ay | Alana özgü sınıflandırma uzmanlık gerektirir |
| İçerik moderasyon API'si (yerel, gizli) | Bulut AI kullanamayan platformlar | 10K kontrol başına $79/ay | Gizlilik uyumlu moderasyon nadir |
| SEO içerik puanlayıcı (taslağı rakiplerle analiz eder) | İçerik ajansları, SEO araçları | 100 analiz başına $39/ay | Yazarken gerçek zamanlı puanlama |

### API Ürünü İnşa Etmek: Tam Örnek

Bir belge sınıflandırma API'si inşa edelim — bir hukuk teknolojisi startup'ının ayda $99 ödeyeceği türden.

**Yığın:**

```
Çalışma zamanı:  Hono (TypeScript) Vercel Edge Functions üzerinde
LLM:             Ollama (yerel, geliştirme için) + Anthropic API (üretim yedek)
Auth:            API anahtar tabanlı (basit, geliştirici dostu)
Hız Sınırlama:   Upstash Redis (ücretsiz katman: günde 10K istek)
Faturalandırma:  Stripe kullanım bazlı faturalandırma
Dokümantasyon:   OpenAPI şeması + barındırılan belgeler
```

**Tam API uygulaması:**

```typescript
// src/api.ts — Belge Sınıflandırma API'si
import { Hono } from "hono";
import { cors } from "hono/cors";
import { Ratelimit } from "@upstash/ratelimit";
import { Redis } from "@upstash/redis";

const app = new Hono();
const ratelimit = new Ratelimit({
  redis: new Redis({ url: process.env.UPSTASH_REDIS_URL!, token: process.env.UPSTASH_REDIS_TOKEN! }),
  limiter: Ratelimit.slidingWindow(100, "1 h"),
});

// Auth middleware: API anahtarı → kullanıcı sorgusu → hız sınırı → kullanım takibi
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

// Ana sınıflandırma uç noktası
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
    // Önce yerel Ollama'yı dene, yedek olarak Anthropic API
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

**API'in için fiyatlandırma sayfası içeriği:**

```
Ücretsiz Katman:    Ayda 100 istek, 5K karakter limiti       $0
Başlangıç:          Ayda 2.000 istek, 50K karakter limiti     $29/ay
Profesyonel:        Ayda 10.000 istek, 50K karakter limiti    $99/ay
Kurumsal:           Özel limitler, SLA, özel destek            Bize ulaşın
```

### Stripe ile Kullanım Bazlı Faturalandırma

```typescript
// billing.ts — Ölçülen faturalandırma için Stripe'a kullanım bildir

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

// Bunu cron ile saatlik çalıştır
// Vercel: vercel.json cron yapılandırması
// Railway: railway cron
// Kendi barındırma: sistem cron'u
```

### Çekiş Kazandığında Ölçeklendirme

{? if profile.gpu.exists ?}
API'in gerçek kullanım almaya başladığında, {= profile.gpu.model | fallback("GPU") =} sana avantaj sağlar — bulut çıkarımı için ödeme yapmadan önce ilk müşterilerine kendi donanımından hizmet verebilirsin. İşte ölçeklendirme yolu:
{? else ?}
API'in gerçek kullanım almaya başladığında, işte ölçeklendirme yolu. Özel bir GPU olmadan, ölçeklendirme eğrisinde daha erken bulut çıkarımına (Replicate, Together.ai) geçmek isteyeceksin:
{? endif ?}

```
Aşama 1: 0-100 müşteri
  - Yerel Ollama + Vercel edge fonksiyonları
  - Toplam maliyet: $0-20/ay
  - Gelir: $0-5.000/ay

Aşama 2: 100-500 müşteri
  - LLM çıkarımını özel bir VPS'e taşı (Hetzner GPU, {= regional.currency_symbol | fallback("$") =}50-150/ay)
  - Tekrarlanan sorgular için Redis önbellekleme ekle
  - Toplam maliyet: $50-200/ay
  - Gelir: $5.000-25.000/ay

Aşama 3: 500+ müşteri
  - Yük dengeleyici arkasında birden fazla çıkarım düğümü
  - Taşma için yönetilen çıkarım düşün (Replicate, Together.ai)
  - Toplam maliyet: $200-1.000/ay
  - Gelir: $25.000+/ay
```

> **Yaygın Hata:** 10 müşterin olmadan ölçek için fazla mühendislik yapmak. İlk sürümün ücretsiz katmanlarda çalışmalı. Ölçeklendirme sorunları İYİ sorunlardır. Geldiklerinde çöz, öncesinde değil.

### Senin Sıran

1. **API nişini belirle** (1 saat): Hangi alanı yeterince iyi biliyorsun? Hukuk? Finans? Sağlık? E-ticaret? En iyi API ürünleri derin alan bilgisinin AI yeteneğiyle eşleştirilmesinden gelir.

2. **Konsept kanıtı oluştur** (8-16 saat): Bir uç nokta, bir fonksiyon, auth yok (sadece yerel test). 10 örnek belge için sınıflandırma/çıkarma/analizi doğru çalışır hale getir.

3. **Auth ve faturalandırma ekle** (4-8 saat): API anahtar yönetimi, Stripe entegrasyonu, kullanım takibi. Yukarıdaki kod sana bunun %80'ini veriyor.

4. **API dokümantasyonu yaz** (2-4 saat): Stoplight kullan veya sadece elle bir OpenAPI şeması yaz. İyi dokümantasyon, API ürün benimsenmesinde 1 numaralı faktör.

5. **Geliştirici pazarında lansmanı yap** (1 saat): Product Hunt, Hacker News, ilgili subreddit'lerde paylaş. Geliştiriciden geliştiriciye pazarlama, API ürünleri için en etkili olanıdır.

---

## Ders 6: Danışmanlık ve Parça Zamanlı CTO

*"Başlaması en hızlı motor ve diğer her şeyi finanse etmenin en iyi yolu."*

**İlk dolara kadar süre:** 1 hafta (ciddi)
**Devam eden zaman taahhüdü:** Haftada 5-20 saat (kadranı sen kontrol ediyorsun)
**Kar marjı:** %95+ (zamanın tek maliyet)

### Neden Danışmanlık Çoğu Geliştirici İçin Motor #1

{@ insight stack_fit @}

Bu ay değil, bu çeyrek gelire ihtiyacın varsa, danışmanlık cevap. İnşa edecek ürün yok. Büyütecek kitle yok. Kuracak pazarlama hunisi yok. Sadece sen, uzmanlığın ve ona ihtiyaç duyan biri.

Matematik:

```
$200/saat x haftada 5 saat = $4.000/ay
$300/saat x haftada 5 saat = $6.000/ay
$400/saat x haftada 5 saat = $8.000/ay

Bu, tam zamanlı işinin yanı sıra.
```

"Ama saati $200 ücretlendiremem." Evet ücretlendirebilirsin. Birazdan buna geleceğiz.

### Gerçekte Ne Satıyorsun

{? if stack.primary ?}
"{= stack.primary | fallback("programming") =}" satmıyorsun. Bunlardan birini satıyorsun:
{? else ?}
"Programlama" satmıyorsun. Bunlardan birini satıyorsun:
{? endif ?}

1. **Zaman tasarrufu sağlayan uzmanlık.** "Kubernetes kümenizi takımınızın 80 saat uğraşması yerine 10 saatte doğru şekilde kuracağım."
2. **Risk azaltan bilgi.** "Lansmanınızdan önce mimarinizi denetleyeceğim, böylece birinci günde 10.000 kullanıcıyla ölçeklendirme sorunları keşfetmezsiniz."
3. **Karar veren muhakeme.** "Üç satıcı seçeneğinizi değerlendirip kısıtlamalarınıza uygun olanı tavsiye edeceğim."
4. **Takımları açan liderlik.** "Mühendislik takımınızı [yeni teknoloji]'ye geçiş sürecinde özellik geliştirmesini yavaşlatmadan yöneteceğim."

Çerçeveleme önemli. "Python yazıyorum" saati $50 değerinde. "Veri boru hattı işleme sürenizi iki hafta içinde %60 azaltacağım" saati $300 değerinde.

**Bağlam için gerçek ücret verileri:**
- **Rust danışmanlığı:** Ortalama $78/saat, deneyimli danışmanlar standart çalışma için $143/saate kadar alıyor. Mimari ve göç danışmanlığı bunun çok üstüne çıkıyor. (kaynak: ziprecruiter.com)
- **AI/ML danışmanlığı:** Uygulama çalışması için $120-250/saat. Stratejik AI danışmanlığı (mimari, deployment planlaması) kurumsal ölçekte $250-500/saat alıyor. (kaynak: debutinfotech.com)

### 2026'daki Sıcak Danışmanlık Nişleri

{? if stack.contains("rust") ?}
Rust uzmanlığın seni mevcut en yüksek talep, en yüksek ücret danışmanlık nişlerinden birine sokuyor. Rust göç danışmanlığı, arz ciddi şekilde kısıtlı olduğu için premium ücret alıyor.
{? endif ?}

| Niş | Ücret Aralığı | Talep | Neden Sıcak |
|-----|--------------|-------|-------------|
| Yerel AI deployment'ı | $200-400/saat | Çok yüksek | AB AI Yasası + gizlilik endişeleri. Bu beceriye sahip az danışman var. |
| Gizlilik öncelikli mimari | $200-350/saat | Yüksek | Düzenleme talebi artırıyor. "OpenAI'ya veri göndermeyi durdurmamız lazım." |
| Rust göçü | $250-400/saat | Yüksek | Şirketler Rust'ın güvenlik garantilerini istiyor ama Rust geliştiricileri yok. |
| AI kodlama aracı kurulumu | $150-300/saat | Yüksek | Mühendislik takımları Claude Code/Cursor'u benimsemek istiyor ama ajanlar, iş akışları, güvenlik konusunda rehberlik gerekiyor. |
| Veritabanı performansı | $200-350/saat | Orta-Yüksek | Ezeli ihtiyaç. AI araçları 3 kat daha hızlı teşhis etmene yardımcı olur. |
| Güvenlik denetimi (AI destekli) | $250-400/saat | Orta-Yüksek | AI araçları seni daha kapsamlı yapar. Şirketler buna yatırım turlarından önce ihtiyaç duyar. |

### Bu Hafta İlk Danışmanlık Müşterini Nasıl Alırsın

**Gün 1:** LinkedIn başlığını güncelle. KÖTÜ: "BigCorp'ta Kıdemli Yazılım Mühendisi." İYİ: "Mühendislik takımlarının AI modellerini kendi altyapılarında deploy etmesine yardım ediyorum | Rust + Yerel AI."

**Gün 2:** 3 LinkedIn paylaşımı yaz. (1) Gerçek rakamlarla teknik bir içgörü paylaş. (2) Elde ettiğin somut bir sonucu paylaş. (3) Doğrudan yardım teklif et: "Bu ay [nişin] konusunda yardım arayan takımlar için 2 danışmanlık angajmanı alıyorum. Ücretsiz 30 dakikalık değerlendirme için DM."

**Gün 3-5:** CTO'lara ve Mühendislik Müdürlerine 10 kişiselleştirilmiş ulaşım mesajı gönder. Şablon: "[Şirket]'in [spesifik gözlem] yaptığını fark ettim. Takımların [değer önerisi] konusunda yardım ediyorum. Yakın zamanda [benzer şirket]'in [sonuç] elde etmesine yardım ettim. 20 dakikalık bir görüşme faydalı olur mu?"

**Gün 5-7:** Danışmanlık platformlarına başvur: **Toptal** (premium, $100-200+/saat, 2-4 hafta değerlendirme), **Arc.dev** (uzaktan odaklı, daha hızlı katılım), **Lemon.io** (Avrupa odaklı), **Clarity.fm** (dakika başı danışmanlık).

### Ücret Müzakeresi

**Ücretini nasıl belirlersin:**

```
Adım 1: Nişin için piyasa ücretini bul
  - Toptal'ın yayınlanan aralıklarını kontrol et
  - Geliştirici Slack/Discord topluluklarında sor
  - Benzer danışmanların kamuya açık ücretlerine bak

Adım 2: Aralığın tepesinden başla
  - Piyasa $150-300/saat ise, $250-300 teklif et
  - Müzakere ederlerse, piyasa ücretine inersin
  - Müzakere etmezlerse, piyasanın üstünde kazanıyorsundur

Adım 3: Ücretini asla düşürme — bunun yerine kapsam ekle
  KÖTÜ:  "$300 yerine $200 yapabilirim."
  İYİ:   "$200/saat için X ve Y yapabilirim. $300/saat için
          Z'yi de yapar ve sürekli destek sağlarım."
```

**Değer çıpası tekniği:**

Ücretini teklif etmeden önce, teslim edeceğin değeri ölçümle:

```
"Anlattıklarınıza dayanarak, bu göç önümüzdeki çeyrekte takımınıza
yaklaşık 200 mühendislik saati tasarruf sağlayacak. Takımınızın
yüklü maliyetini saati $150 olarak hesaplarsak, bu $30.000 tasarruf demek.
Bu projeyi yönetmek için ücretim $8.000."

(Müşteri için $30.000 tasarrufa karşı $8.000 = 3,75x yatırım getirisi)
```

### Maksimum Kaldıraç İçin Danışmanlığı Yapılandırmak

Danışmanlığın tuzağı zamanı parayla takas etmek. Bundan kurtul:

1. **Her şeyi belgele** — Her angajman göç kılavuzları, mimari belgeler, kurulum prosedürleri üretir. Müşteriye özgü detayları çıkar ve bir ürünün (Ders 1) veya blog yazısın (Ders 2) var.
2. **Tekrarlanan çalışmayı şablonla** — 3 müşteri için aynı sorun mu? O bir mikro-SaaS (Ders 3) veya dijital ürün (Ders 1).
3. **Konuşma yap, müşteri kazan** — Bir buluşmada 30 dakikalık konuşma 2-3 müşteri sohbeti üretir. Faydalı bir şey öğret; insanlar sana gelir.
4. **Yaz, sonra ücretlendir** — Belirli bir teknik zorluk hakkında blog yazısı, tam olarak o soruna sahip olan ve yardıma ihtiyaç duyan insanları çeker.

### 4DA'yı Gizli Silahın Olarak Kullanmak

{@ mirror feed_predicts_engine @}

İşte çoğu danışmanın sahip olmadığı bir rekabet avantajı: **nişinde neler olduğunu müşterilerinden önce biliyorsun.**

4DA sinyalleri yüzeye çıkarır — yeni güvenlik açıkları, trend teknolojiler, önemli değişiklikler, düzenleyici güncellemeler. Bir müşteriye "Bu arada, [kullandıkları kütüphanede] dün açıklanan yeni bir güvenlik açığı var ve bunu ele almak için tavsiyem şu" dediğinde, doğaüstü bir farkındalığın varmış gibi görünürsün.

O farkındalık premium ücretleri haklı çıkarır. Müşteriler proaktif olarak bilgilendirilmiş danışmanlara, reaktif olarak Google'layan danışmanlara göre daha fazla öder.

> **Gerçek Konuşma:** Danışmanlık diğer motorlarını finanse etmenin en iyi yoludur. 1-3. aylardan danışmanlık gelirini mikro-SaaS'ını (Ders 3) veya içerik operasyonunu (Ders 2) bankacılık yapmak için kullan. Amaç sonsuza kadar danışmanlık yapmak değil — zamanın olmadan gelir üreten şeyler inşa etmek için pist oluşturmak amacıyla şimdi danışmanlık yapmak.

### Senin Sıran

1. **LinkedIn'ini güncelle** (30 dk): Yeni başlık, yeni "Hakkımda" bölümü ve uzmanlığın hakkında öne çıkan bir paylaşım. Bu senin vitrin.

2. **Bir LinkedIn paylaşımı yaz ve yayınla** (1 saat): Teknik bir içgörü, bir sonuç veya bir teklif paylaş. Satış değil — önce değer.

3. **5 doğrudan ulaşım mesajı gönder** (1 saat): Kişiselleştirilmiş, spesifik, değer odaklı. Yukarıdaki şablonu kullan.

4. **Bir danışmanlık platformuna başvur** (30 dk): Toptal, Arc veya Lemon.io. Süreci başlat — zaman alır.

5. **Ücretini belirle** (15 dk): Nişin için piyasa ücretlerini araştır. Ücretini yaz. Aşağı yuvarlama.

---

## Ders 7: Açık Kaynak + Premium

*"Açıkça inşa et, güven kazan, piramidin tepesini paraya çevir."*

**İlk dolara kadar süre:** 4-12 hafta
**Devam eden zaman taahhüdü:** Haftada 10-20 saat
**Kar marjı:** %80-95 (barındırılan sürümler için altyapı maliyetlerine bağlı)

### Açık Kaynak İş Modeli

{@ insight stack_fit @}

Açık kaynak bir hayır işi değil. Bir dağıtım stratejisi.

İşte mantık:
1. Bir araç inşa edip açık kaynak yaparsın
2. Geliştiriciler onu bulur, kullanır ve ona bağımlı hale gelir
3. Bu geliştiricilerin bazıları şirketlerde çalışır
4. O şirketlerin bireylerin ihtiyaç duymadığı özelliklere ihtiyacı var: SSO, takım yönetimi, denetim günlükleri, öncelikli destek, SLA'lar, barındırılan sürüm
5. O şirketler premium sürüm için sana ödeme yapar

Ücretsiz sürüm senin pazarlaması. Premium sürüm senin gelirin.

### Lisans Seçimi

Lisansın hendekini belirler. Dikkatli seç.

| Lisans | Ne Anlama Geliyor | Gelir Stratejisi | Örnek |
|--------|-------------------|------------------|-------|
| **MIT** | Herkes her şeyi yapabilir. Fork'la, sat, seninle rekabet et. | Premium özellikler / barındırılan sürüm, kendi başına yapmanın buna değmeyeceği kadar çekici olmalı. | Express.js, React |
| **AGPLv3** | Ağ üzerinden kullanan herkes değişikliklerini açık kaynak yapmalı. Şirketler bundan nefret eder — bunun yerine ticari lisans için ödeme yaparlar. | Çift lisans: açık kaynak için AGPL, AGPL istemeyen şirketler için ticari lisans. | MongoDB (başlangıçta), Grafana |
| **FSL (Functional Source License)** | 2-3 yıl boyunca kaynak görünür ama açık kaynak değil. Bu süre sonunda Apache 2.0'a dönüşür. Kritik büyüme aşamanda doğrudan rakipleri engeller. | Pazar pozisyonunu oluştururken doğrudan rekabet engellenir. Ek gelir için premium özellikler. | 4DA, Sentry |
| **BUSL (Business Source License)** | FSL'e benzer. Belirli bir süre boyunca rakipler tarafından üretim kullanımını kısıtlar. | FSL ile aynı. | HashiCorp (Terraform, Vault) |

**Solo geliştiriciler için tavsiye:** FSL veya AGPL.

{? if regional.country == "US" ?}
- Şirketlerin kendi barındıracağı bir şey inşa ediyorsan: **AGPL** (AGPL yükümlülüklerinden kaçınmak için ticari lisans satın alırlar). ABD şirketleri özellikle ticari ürünlerde AGPL'ye karşı isteksizdir.
{? else ?}
- Şirketlerin kendi barındıracağı bir şey inşa ediyorsan: **AGPL** (AGPL yükümlülüklerinden kaçınmak için ticari lisans satın alırlar)
{? endif ?}
- 2 yıl boyunca tamamen kontrol etmek istediğin bir şey inşa ediyorsan: **FSL** (pazar pozisyonunu oluştururken fork'ların seninle rekabet etmesini engeller)

> **Yaygın Hata:** "Açık kaynak ücretsiz olmalı" diye MIT seçmek. MIT cömert ve bu takdire değer. Ama risk sermayesi destekli bir şirket MIT projeni fork'lar, ödeme katmanı ekler ve seni pazarlamada geçerse, çalışmalarını onların yatırımcılarına bağışlamış olursun. İşini, iş kuracak kadar uzun süre koru, sonra aç.

### Açık Kaynak Projeyi Pazarlamak

GitHub yıldızları gösteriş metrikleri ama aynı zamanda benimsemeyi yönlendiren sosyal kanıt. İşte nasıl alınır:

**1. README senin açılış sayfan**

README'nde şunlar olmalı:
- **Tek cümlelik açıklama** aracın ne yaptığını ve kimin için olduğunu açıklar
- **Ekran görüntüsü veya GIF** aracı çalışırken gösterir (bu tek başına tıklanma oranını iki katına çıkarır)
- **Hızlı başlangıç** — `npm install x` veya `cargo install x` ve ilk komut
- **Özellik listesi** ücretsiz vs. premium için net etiketlerle
- **Rozet duvarı** — derleme durumu, sürüm, lisans, indirmeler
- **"Bu araç neden?"** — 3-5 cümle onu farklı yapan şey hakkında

**2. Show HN paylaşımı (lansman günün)**

Hacker News "Show HN" paylaşımları, geliştirici araçları için en etkili lansman kanalıdır. Net, olgusal bir başlık yaz: "Show HN: [Araç Adı] — [<10 kelimede ne yaptığı]." Yorumlarda motivasyonunu, teknik kararlarını ve ne hakkında geri bildirim aradığını açıkla.

**3. Reddit lansman stratejisi**

İlgili subreddit'e paylaş (Rust araçları için r/rust, kendi barındırma araçları için r/selfhosted, web araçları için r/webdev). Çözdüğün sorun ve nasıl çözdüğün hakkında samimi bir yazı yaz. GitHub'a bağla. Satışçı olma.

**4. "Awesome" liste gönderimleri**

Her framework ve dilin GitHub'da bir "awesome-X" listesi var. Oraya eklenmek sürdürülebilir trafik yönlendirir. İlgili listeyi bul, kriterleri karşılayıp karşılamadığını kontrol et ve bir PR gönder.

### Gelir Modeli: Açık Çekirdek

Solo geliştiriciler için en yaygın açık kaynak gelir modeli:

```
ÜCRETSİZ (açık kaynak):
  - Temel işlevsellik
  - CLI arayüzü
  - Yerel depolama
  - Topluluk desteği (GitHub issues)
  - Sadece kendi barındırma

PRO (kullanıcı başına $12-29/ay):
  - Ücretsiz'deki her şey
  - GUI / dashboard
  - Bulut senkronizasyonu veya barındırılan sürüm
  - Öncelikli destek (24 saat yanıt süresi)
  - Gelişmiş özellikler (analitik, raporlama, entegrasyonlar)
  - E-posta desteği

TAKIM (takım başına $49-99/ay):
  - Pro'daki her şey
  - SSO / SAML kimlik doğrulama
  - Rol tabanlı erişim kontrolü
  - Denetim günlükleri
  - Paylaşılan çalışma alanları
  - Takım yönetimi

KURUMSAL (özel fiyatlandırma):
  - Takım'daki her şey
  - Yerinde deployment desteği
  - SLA (%99,9 çalışma süresi garantisi)
  - Özel destek kanalı
  - Özel entegrasyonlar
  - Fatura faturalandırma (net-30)
```

### Gerçek Gelir Örnekleri

**Kalibrasyon için gerçek dünya açık kaynak işletmeleri:**
- **Plausible Analytics:** Gizlilik öncelikli web analitiği, AGPL lisanslı, tamamen bootstrapped. 12K abone ile $3,1M ARR'ye ulaştı. Risk sermayesi yok. AGPL çift lisans modelinin solo/küçük takım ürünleri için çalıştığını kanıtlıyor. (kaynak: plausible.io/blog)
- **Ghost:** Açık kaynak yayıncılık platformu. 2024'te $10,4M gelir, 24K müşteri. Açık çekirdek projesi olarak başladı ve topluluk öncelikli stratejiyle büyüdü. (kaynak: getlatka.com)

İşte premium katmanlı daha küçük bir açık kaynak projesi için büyümenin tipik görünümü:

| Aşama | Yıldızlar | Pro Kullanıcılar | Takım/Kurumsal | MRR | Senin Zamanın |
|-------|-----------|-----------------|----------------|-----|---------------|
| 6 ay | 500 | 12 ($12/ay) | 0 | $144 | haftada 5 saat |
| 12 ay | 2.000 | 48 ($12/ay) | 3 takım ($49/ay) | $723 | haftada 8 saat |
| 18 ay | 5.000 | 150 ($19/ay) | 20 takım + 2 kurumsal | $5.430 | haftada 15 saat |

Kalıp: yavaş başlangıç, bileşik büyüme. 18 ayda $5.430/ay MRR = yılda $65K. İşin çoğu 1-6. aylarda. Sonrasında topluluk büyümeyi yönlendiriyor. Plausible'ın yörüngesi, bileşiğin 18 ayın ötesinde devam ettiğinde ne olduğunu gösteriyor.

### Lisanslama ve Özellik Kapılama Kurulumu

```typescript
// license.ts — Açık çekirdek için basit özellik kapılama
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
      // Bu özelliği içeren minimum planı bul
      const requiredPlan = (Object.entries(PLAN_CONFIG) as [Plan, any][])
        .find(([_, config]) => config.features.has(feature))?.[0] || "enterprise";
      throw new Error(
        `"${feature}" requires ${requiredPlan} plan. ` +
        `You're on ${this.plan}. Upgrade at https://yourapp.com/pricing`
      );
    }
  }
}

// Kullanım: const license = new LicenseManager(user.plan);
//           license.requireFeature("cloud_sync"); // doğru planda değilse hata fırlatır
```

### Senin Sıran

1. **Açık kaynak projeni belirle** (1 saat): Kendin hangi aracı kullanırdın? Bir script ile çözdüğün hangi sorun düzgün bir araç olmayı hak ediyor? En iyi açık kaynak projeleri kişisel yardımcı araçlar olarak başlar.

2. **Lisansını seç** (15 dk): Gelir koruması için FSL veya AGPL. Parasallaştırma planı olmadan topluluk iyiliği için inşa ediyorsan sadece MIT.

3. **Çekirdeği inşa et ve gönder** (1-4 hafta): Çekirdeği açık kaynak yap. README'yi yaz. GitHub'a push et. Mükemmel olmasını bekleme.

4. **Fiyatlandırma katmanlarını tanımla** (1 saat): Ücretsiz / Pro / Takım. Her katmanda hangi özellikler var? Premium özellikleri inşa etmeden önce yaz.

5. **Lansman** (1 gün): Show HN paylaşımı, 2-3 ilgili subreddit ve "Awesome" liste PR'ı.

---

## Ders 8: Veri Ürünleri ve İstihbarat

*"Bilgi sadece işlendiğinde, filtrelendiğinde ve bağlam içinde teslim edildiğinde değerlidir."*

**İlk dolara kadar süre:** 4-8 hafta
**Devam eden zaman taahhüdü:** Haftada 5-15 saat
**Kar marjı:** %85-95

### Veri Ürünleri Nedir

{@ insight stack_fit @}

Bir veri ürünü, ham bilgiyi — kamusal veriler, araştırma makaleleri, pazar trendleri, ekosistem değişiklikleri — belirli bir kitle için eyleme dönüştürülebilir bir şeye dönüştürür. Yerel LLM'in işlemeyi halleder. Senin uzmanlığın küratörlüğü halleder. Kombinasyon ödemeye değer.

Bu, içerik parasallaştırmadan (Ders 2) farklı. İçerik "işte React trendleri hakkında bir blog yazısı." Bir veri ürünü ise "işte React ekosistemi karar vericileri için puanlanmış sinyaller, trend analizi ve spesifik eyleme dönüştürülebilir tavsiyelerle yapılandırılmış haftalık rapor."

### Veri Ürün Türleri

**1. Küratörlü İstihbarat Raporları**

| Ürün | Kitle | Format | Fiyat |
|------|-------|--------|-------|
| "Uygulama notlarıyla Haftalık AI Makale Özeti" | ML mühendisleri, AI araştırmacıları | Haftalık e-posta + aranabilir arşiv | $15/ay |
| "Rust Ekosistemi İstihbarat Raporu" | Rust geliştiricileri, Rust'ı değerlendiren CTO'lar | Aylık PDF + haftalık uyarılar | $29/ay |
| "Geliştirici İş Piyasası Trendleri" | İşe alım müdürleri, iş arayanlar | Aylık rapor | Tek seferlik $49 |
| "Gizlilik Mühendisliği Bülteni" | Gizlilik mühendisleri, uyumluluk takımları | İki haftalık e-posta | $19/ay |
| "Bağımsız SaaS Karşılaştırmaları" | Bootstrapped SaaS kurucuları | Aylık veri seti + analiz | $29/ay |

**2. İşlenmiş Veri Setleri**

| Ürün | Kitle | Format | Fiyat |
|------|-------|--------|-------|
| Açık kaynak proje metrikleri küratörlü veritabanı | VC'ler, OSS yatırımcıları | API veya CSV dışa aktarımı | $99/ay |
| Şehir, rol ve şirkete göre teknoloji maaş verileri | Kariyer koçları, İK | Çeyreklik veri seti | Veri seti başına $49 |
| 100 popüler hizmette API çalışma süresi karşılaştırmaları | DevOps, SRE takımları | Dashboard + API | $29/ay |

**3. Trend Uyarıları**

| Ürün | Kitle | Format | Fiyat |
|------|-------|--------|-------|
| Düzeltme kılavuzlarıyla kritik bağımlılık güvenlik açıkları | Geliştirici takımları | Gerçek zamanlı e-posta/Slack uyarıları | Takım başına $19/ay |
| Göç kılavuzlarıyla yeni framework sürümleri | Mühendislik yöneticileri | Anında uyarılar | $9/ay |
| AI/gizliliği etkileyen düzenleyici değişiklikler | Hukuk takımları, CTO'lar | Haftalık özet | $39/ay |

### Veri Boru Hattını İnşa Etmek

{? if settings.has_llm ?}
İşte haftalık istihbarat raporu üretmek için eksiksiz bir boru hattı. Bu gerçek, çalıştırılabilir kod — ve {= settings.llm_model | fallback("a local model") =} kurulumun olduğundan, bu boru hattını sıfır marjinal maliyetle çalıştırabilirsin.
{? else ?}
İşte haftalık istihbarat raporu üretmek için eksiksiz bir boru hattı. Bu gerçek, çalıştırılabilir kod. Öğeleri sıfır maliyetle işlemek için yerel olarak çalışan Ollama'ya ihtiyacın olacak (Modül S'e bakın).
{? endif ?}

```python
#!/usr/bin/env python3
"""
intelligence_pipeline.py — Haftalık istihbarat rapor üreticisi.
Getir → Puanla → Biçimlendir → Teslim Et. Alanın için NICHE ve RSS_FEEDS'i özelleştir.
"""
import requests, json, time, feedparser
from datetime import datetime, timedelta
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "llama3.1:8b"

# ── Aşama 1: RSS + HN'den Getir ─────────────────────────────────

def fetch_items(feeds: list[dict], hn_min_score: int = 50) -> list[dict]:
    items = []
    cutoff = datetime.now() - timedelta(days=7)

    # RSS feed'leri
    for feed_cfg in feeds:
        try:
            for entry in feedparser.parse(feed_cfg["url"]).entries[:20]:
                items.append({"title": entry.get("title", ""), "url": entry.get("link", ""),
                    "source": feed_cfg["name"], "content": entry.get("summary", "")[:2000]})
        except Exception as e:
            print(f"  Uyarı: {feed_cfg['name']}: {e}")

    # Hacker News (Algolia API, zaman filtrelenmiş)
    week_ago = int(cutoff.timestamp())
    resp = requests.get(f"https://hn.algolia.com/api/v1/search?tags=story"
        f"&numericFilters=points>{hn_min_score},created_at_i>{week_ago}&hitsPerPage=30")
    for hit in resp.json().get("hits", []):
        items.append({"title": hit.get("title", ""), "source": "Hacker News",
            "url": hit.get("url", f"https://news.ycombinator.com/item?id={hit['objectID']}"),
            "content": hit.get("title", "")})

    # Çoğaltmayı kaldır
    seen = set()
    return [i for i in items if i["title"][:50].lower() not in seen and not seen.add(i["title"][:50].lower())]

# ── Aşama 2: Yerel LLM ile Puanla ────────────────────────────────

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

# ── Aşama 3: Markdown Rapor Oluştur ─────────────────────────────

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

# ── Çalıştır ───────────────────────────────────────────────────────

if __name__ == "__main__":
    NICHE = "Rust Ecosystem"  # ← Bunu değiştir
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
    print(f"Rapor kaydedildi: {output}")
```

### Veri Ürününü Teslim Etmek

**Teslimat:** Resend (aylık 3.000 e-posta ücretsiz) veya Buttondown kullan. Markdown raporunu `marked` ile HTML'ye dönüştür, Resend'in toplu API'si ile gönder. Toplam teslimat kodu: ~15 satır.

**Veri ürünleri için fiyatlandırma stratejisi:**

```
Ücretsiz katman:   Aylık özet (tanıtım) — kitle oluşturur
Bireysel:          $15-29/ay — tam haftalık rapor + arşiv erişimi
Takım:             $49-99/ay — birden fazla koltuk + ham veriye API erişimi
Kurumsal:          $199-499/ay — özel sinyaller, özel analist zamanı
```

### Gelir Projeksiyonu

```
Ay 1:    $15/ay'da 10 abone  = $150/ay   (arkadaşlar, erken benimseyenler)
Ay 3:    $15/ay'da 50 abone  = $750/ay   (organik büyüme, HN/Reddit paylaşımları)
Ay 6:    $15/ay'da 150 abone = $2.250/ay  (SEO + yönlendirmeler devreye giriyor)
Ay 12:   $15/ay'da 400 abone = $6.000/ay  (yerleşik marka + takım planları)

Çalıştırma maliyeti:  ~$10/ay (e-posta gönderimi + alan adı)
Senin zamanın:        haftada 5-8 saat (çoğu otomatik, sen uzmanlık ekliyorsun)
```

{@ temporal revenue_benchmarks @}

**Bağlam için gerçek dünya içerik üreticisi karşılaştırmaları:**
- **Fireship** (Jeff Delaney): 4M YouTube abonesi, sadece reklamlardan yılda ~$550K+. Geliştirici odaklı, kısa biçimli içerik. (kaynak: networthspot.com)
- **Wes Bos:** Toplam kurs satışlarında $10M+, 55K ödeme yapan öğrenci. Teknik eğitimin bülten gelirinin çok ötesine ölçeklenebileceğini kanıtlıyor. (kaynak: foundershut.com)
- **Josh Comeau:** CSS kursu ön siparişlerinin ilk haftasında $550K. Odaklı, yüksek kaliteli teknik eğitimin premium fiyatlar alabileceğini gösteriyor. (kaynak: failory.com)

Bunlar üst düzey sonuçlar, ama yukarıdaki boru hattı yaklaşımı birçoğunun başladığı şekil: tutarlı, niş odaklı, net değerli içerik.

{? if profile.gpu.exists ?}
Anahtar: boru hattı ağır işi yapıyor. {= profile.gpu.model | fallback("GPU") =} çıkarımı yerel olarak hallediyor, rapor başına maliyetini sıfıra yakın tutuyor. Senin uzmanlığın hendek. Başka hiç kimse senin spesifik alan bilgisi + kürasyon yargısı + işleme altyapısı kombinasyonuna sahip değil.
{? else ?}
Anahtar: boru hattı ağır işi yapıyor. Yalnızca CPU çıkarımıyla bile, haftada 30-50 makale işlemek toplu boru hatları için pratik. Senin uzmanlığın hendek. Başka hiç kimse senin spesifik alan bilgisi + kürasyon yargısı + işleme altyapısı kombinasyonuna sahip değil.
{? endif ?}

### Senin Sıran

1. **Nişini seç** (30 dk): Hangi alanı görüş sahibi olacak kadar iyi biliyorsun? Veri ürün nişin o.

2. **5-10 veri kaynağı belirle** (1 saat): RSS feed'leri, API'ler, subreddit'ler, HN aramaları, şu anda okuduğun bültenler. Bunlar ham girdilerin.

3. **Boru hattını bir kez çalıştır** (2 saat): Yukarıdaki kodu nişin için özelleştir. Çalıştır. Çıktıya bak. Faydalı mı? Bunun için para öder miydin?

4. **İlk raporunu üret** (2-4 saat): Boru hattı çıktısını düzenle. Analizini, görüşlerini, "peki ne olmuş"unu ekle. Bu, ödemeye değer yapan %20.

5. **10 kişiye gönder** (30 dk): Ürün olarak değil — örnek olarak. "Haftalık [niş] istihbarat raporu başlatmayı düşünüyorum. İşte ilk sayı. Bu sana faydalı olur mu? Ayda $15 öder misin?"

---

## Motor Seçimi: İkini Seçmek

*"Artık sekiz motoru biliyorsun. İkisine ihtiyacın var. İşte nasıl seçersin."*

### Karar Matrisi

{@ insight engine_ranking @}

Her motoru bu dört boyutta 1-5 puanla, SENİN spesifik durumuna dayanarak:

| Boyut | Ne Anlama Geliyor | Nasıl Puanlanır |
|-------|-------------------|-----------------|
| **Beceri uyumu** | Bu motor zaten bildiğin şeyle ne kadar uyuşuyor? | 5 = mükemmel uyum, 1 = tamamen yeni alan |
| **Zaman uyumu** | Bu motoru mevcut saatlerinle yürütebilir misin? | 5 = mükemmel uyuyor, 1 = işimi bırakmam gerekir |
| **Hız** | İlk dolarını ne kadar hızlı göreceksin? | 5 = bu hafta, 1 = 3+ ay |
| **Ölçek** | Bu motor, orantılı olarak daha fazla zaman harcamadan ne kadar büyüyebilir? | 5 = sonsuz (ürün), 1 = doğrusal (zamanı parayla takas) |

**Bu matrisi doldur:**

```
Motor                          Beceri  Zaman  Hız  Ölçek  TOPLAM
─────────────────────────────────────────────────────────────────
1. Dijital Ürünler               /5     /5    /5    /5     /20
2. İçerik Parasallaştırma        /5     /5    /5    /5     /20
3. Mikro-SaaS                    /5     /5    /5    /5     /20
4. Hizmet Olarak Otomasyon       /5     /5    /5    /5     /20
5. API Ürünleri                  /5     /5    /5    /5     /20
6. Danışmanlık                   /5     /5    /5    /5     /20
7. Açık Kaynak + Premium         /5     /5    /5    /5     /20
8. Veri Ürünleri                 /5     /5    /5    /5     /20
```

### 1+1 Stratejisi

{? if dna.identity_summary ?}
Geliştirici profiline dayanarak — {= dna.identity_summary | fallback("your unique combination of skills and interests") =} — hangi motorların zaten yaptığın şeyle en doğal şekilde uyuştuğunu düşün.
{? endif ?}

{? if computed.experience_years < 3 ?}
> **Senin deneyim seviyenle:** **Dijital Ürünler** (Motor 1) veya **İçerik Parasallaştırma** (Motor 2) ile başla — en düşük risk, en hızlı geri bildirim döngüsü. Portföyünü oluştururken pazarın ne istediğini öğrenirsin. Daha fazla gönderilmiş çalışma gösterene kadar Danışmanlık ve API Ürünlerinden kaçın. Şu anki avantajın enerji ve hız, derinlik değil.
{? elif computed.experience_years < 8 ?}
> **Senin deneyim seviyenle:** 3-8 yıllık deneyimin **Danışmanlık** ve **API Ürünleri**ni açıyor — derinliği ödüllendiren daha yüksek marjlı motorlar. Müşteriler sadece çıktıya değil, muhakemeye para öder. Danışmanlığı (hızlı nakit) Mikro-SaaS veya API Ürünleri (ölçeklenebilir) ile eşleştirmeyi düşün. Deneyimin hendek — gerçekten neyin işe yaradığını bilecek kadar üretim sistemi gördün.
{? else ?}
> **Senin deneyim seviyenle:** 8+ yılda, zamanla bileşik büyüyen motorlara odaklan: **Açık Kaynak + Premium**, **Veri Ürünleri** veya **Premium ücretlerde Danışmanlık** ($250-500/saat). Premium fiyatlar almak için güvenilirliğin ve ağın var. Avantajın güven ve itibar — kaldıraç olarak kullan. Seçtiğin motorlar ne olursa olsun, bir güçlendirici olarak içerik markası (blog, bülten, YouTube) oluşturmayı düşün.
{? endif ?}

{? if stack.contains("react") ?}
> **React geliştiricileri** için güçlü talep var: UI bileşen kütüphaneleri, Next.js şablonları ve başlangıç kitleri, tasarım sistemi araçları ve Tauri masaüstü uygulama şablonları. React ekosistemi, niş ürünlerin kitle bulacağı kadar büyük. Yığının için doğal uyumlar olarak Motor 1 (Dijital Ürünler) ve Motor 3 (Mikro-SaaS) düşün.
{? endif ?}
{? if stack.contains("python") ?}
> **Python geliştiricileri** için güçlü talep var: veri boru hattı araçları, ML/AI yardımcı programları, otomasyon script'leri ve paketleri, FastAPI şablonları ve CLI araçları. Python'un veri bilimi ve ML'ye uzanması premium danışmanlık fırsatları yaratır. Danışmanlık ile birlikte Motor 4 (Hizmet Olarak Otomasyon) ve Motor 5 (API Ürünleri) düşün.
{? endif ?}
{? if stack.contains("rust") ?}
> **Rust geliştiricileri** arz kısıtlamaları nedeniyle premium ücret alır. Güçlü talep: CLI araçları, WebAssembly modülleri, sistem programlama danışmanlığı ve performans kritik kütüphaneler. Rust ekosistemi hâlâ iyi inşa edilmiş crate'lerin önemli dikkat çekecek kadar genç. Motor 6 (Danışmanlık $250-400/saat) ve Motor 7 (Açık Kaynak + Premium) düşün.
{? endif ?}
{? if stack.contains("typescript") ?}
> **TypeScript geliştiricileri** en geniş pazar erişimine sahip: npm paketleri, VS Code uzantıları, full-stack SaaS ürünleri ve geliştirici araçları. Rekabet Rust veya Python-ML'den daha yüksek, bu yüzden farklılaşma daha önemli. Genel amaçlı araçlar yerine spesifik bir nişe odaklan. Odaklı bir dikeyde Motor 1 (Dijital Ürünler) ve Motor 3 (Mikro-SaaS) düşün.
{? endif ?}

**Motor 1: HIZLI motorun** — En yüksek Hız puanına sahip motoru seç (beraberlikte: en yüksek Toplam). Bunu Hafta 5-6'da inşa ediyorsun. Hedef 14 gün içinde gelir.

**Motor 2: ÖLÇEK motorun** — En yüksek Ölçek puanına sahip motoru seç (beraberlikte: en yüksek Toplam). Bunu Hafta 7-8'de planlıyor ve Modül E boyunca inşa ediyorsun. Hedef 6-12 ay boyunca bileşik büyüme.

**Birlikte iyi çalışan yaygın eşleşmeler:**

| Hızlı Motor | Ölçek Motoru | Neden İyi Eşleşirler |
|-------------|-------------|----------------------|
| Danışmanlık | Mikro-SaaS | Danışmanlık geliri SaaS geliştirmesini finanse eder. Müşteri sorunları SaaS özellikleri olur. |
| Dijital Ürünler | İçerik Parasallaştırma | Ürünler içerik için güvenilirlik verir. İçerik ürün satışlarını yönlendirir. |
| Hizmet Olarak Otomasyon | API Ürünleri | Müşteri otomasyon projeleri yaygın kalıpları ortaya çıkarır → API ürünü olarak paketle. |
| Danışmanlık | Açık Kaynak + Premium | Danışmanlık uzmanlık ve itibar inşa eder. Açık kaynak bunu bir ürün olarak yakalar. |
| Dijital Ürünler | Veri Ürünleri | Şablonlar niş uzmanlığını oluşturur. İstihbarat raporları onu derinleştirir. |

### Gelir Projeksiyon Çalışma Sayfası

{@ insight cost_projection @}

{? if regional.electricity_kwh ?}
Yerel çıkarıma dayanan motorlar için aylık maliyetleri hesaplarken yerel elektrik maliyetini ({= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh) hesaba katmayı unutma.
{? endif ?}

Seçtiğin iki motor için bunu doldur:

```
MOTOR 1 (Hızlı): _______________________________

  İlk dolara kadar süre: _____ hafta
  Ay 1 geliri:            $________
  Ay 3 geliri:            $________
  Ay 6 geliri:            $________

  Aylık gereken zaman:    _____ saat
  Aylık maliyetler:       $________

  İlk kilometre taşı:    $________ tarihine kadar __________

MOTOR 2 (Ölçek): _______________________________

  İlk dolara kadar süre: _____ hafta
  Ay 1 geliri:            $________
  Ay 3 geliri:            $________
  Ay 6 geliri:            $________
  Ay 12 geliri:           $________

  Aylık gereken zaman:    _____ saat
  Aylık maliyetler:       $________

  İlk kilometre taşı:    $________ tarihine kadar __________

KOMBİNE PROJEKSİYON:

  Ay 3 toplamı:    $________/ay
  Ay 6 toplamı:    $________/ay
  Ay 12 toplamı:   $________/ay

  Toplam aylık zaman:     _____ saat
  Toplam aylık maliyetler: $________
```

> **Gerçek Konuşma:** Bu projeksiyonlar yanlış olacak. Sorun değil. Mesele doğruluk değil — inşa etmeye başlamadan önce seni matematik üzerinde düşünmeye zorlamak. Haftada 30 saatini gerektiren ama ayda $200 üreten bir gelir motoru kötü bir anlaşma. Bunu zamanı yatırmadan önce kağıt üzerinde görmen gerekiyor.

### Platform Riski ve Çeşitlendirme

Her gelir motoru kontrol edemediğin platformların üzerinde oturur. Gumroad ücret yapısını değiştirebilir. YouTube kanalını parasallaştırmayı kapatabilir. Vercel ortaklık programını sonlandırabilir. Stripe bir inceleme sırasında hesabını dondurabilir. Bu varsayımsal değil — düzenli olarak oluyor.

**%40 Kuralı:** Gelirinizin %40'ından fazlasının tek bir platforma bağımlı olmasına asla izin verme. Gumroad gelirinizin %60'ını üretiyorsa ve onlar ücretleri bir gecede %5'ten %15'e yükseltirse (2023 başında yapıp sonra geri aldıkları gibi), marjların çöker. YouTube gelirinin %70'i ise ve bir algoritma değişikliği görüntülemelerini yarıya indirirse, başındasın.

**Platform riskinin gerçek örnekleri:**

| Yıl | Platform | Ne Oldu | Geliştiriciler Üzerindeki Etkisi |
|-----|----------|---------|----------------------------------|
| 2022 | Heroku | Ücretsiz katman kaldırıldı | Binlerce hobi projesi ve küçük işletme göç etmek veya ödeme yapmak zorunda kaldı |
| 2023 | Gumroad | %10 sabit ücret duyurusu (sonra geri alındı) | İçerik üreticileri alternatifleri değerlendirmek için çabaladı; Lemon Squeezy veya Stripe yedekleri olanlar etkilenmedi |
| 2023 | Twitter/X API | Ücretsiz katman kaldırıldı, ücretli katmanlar yeniden fiyatlandırıldı | Bot geliştiricileri, içerik otomasyon araçları ve veri ürünleri bir gecede bozuldu |
| 2024 | Unity | Geriye dönük yükleme başına ücret duyuruldu (sonra değiştirildi) | Yıllarca Unity yatırımı olan oyun geliştiricileri ani maliyet artışlarıyla karşılaştı |
| 2025 | Reddit | API fiyatlandırma değişiklikleri | Üçüncü taraf uygulama geliştiricileri işletmelerini tamamen kaybetti |

**Kalıp:** Platformlar kendi büyümeleri için optimize eder, seninki için değil. Bir platformun yaşam döngüsünün başlarında, arzı çekmek için üreticileri sübvanse ederler. Yeterli arza sahip olduktan sonra değer çıkarırlar. Bu kötü niyet değil — iş. Senin işin bundan asla şaşırmamak.

**Platform Bağımlılık Denetimi:**

Bu denetimi çeyreklik çalıştır. Her gelir akışı için yanıtla:

```
PLATFORM BAĞIMLILIK DENETİMİ

Akış: _______________
Bağımlı olduğu platform(lar): _______________

1. Bu akışın gelirinin yüzde kaçı bu platformdan geçiyor?
   [ ] <%25 (düşük risk)  [ ] %25-40 (orta)  [ ] >%40 (yüksek — çeşitlendir)

2. 30 gün içinde alternatif bir platforma geçebilir misin?
   [ ] Evet, alternatifler var ve göç basit
   [ ] Kısmen — bir miktar kilitlenme (kitle, itibar, entegrasyonlar)
   [ ] Hayır — derin kilitlenme (tescilli format, veri dışa aktarma yok)

3. Bu platformun olumsuz değişiklik geçmişi var mı?
   [ ] Zararlı değişiklik geçmişi yok  [ ] Küçük değişiklikler  [ ] Büyük olumsuz değişiklikler

4. Müşteri ilişkisine sahip misin?
   [ ] Evet — e-posta adreslerim var ve müşterilerle doğrudan iletişim kurabilirim
   [ ] Kısmen — bazı müşteriler keşfedilebilir, bazıları değil
   [ ] Hayır — platform tüm müşteri erişimini kontrol ediyor

Eylem öğeleri:
- >%40 bağımlılık ise: bu ay bir alternatif belirle ve test et
- Veri dışa aktarma yoksa: yapabildiğin her şeyi ŞİMDİ dışa aktar, aylık hatırlatıcı koy
- Müşteri ilişkisine sahip değilsen: hemen e-posta toplamaya başla
```

**Motor bazında çeşitlendirme stratejileri:**

| Motor | Birincil Platform Riski | Azaltma |
|-------|------------------------|---------|
| Dijital Ürünler | Gumroad/Lemon Squeezy ücret değişiklikleri | Yedek olarak kendi Stripe checkout'unu koru. Müşteri e-posta listene sahip ol. |
| İçerik Parasallaştırma | YouTube parasallaştırma kapatma, algoritma kaymaları | E-posta listesi oluştur. Birden fazla platforma çapraz paylaşım yap. Kendi alan adında bloguna sahip ol. |
| Mikro-SaaS | Ödeme işlemcisi blokajları, barındırma maliyetleri | Çoklu sağlayıcı ödeme kurulumu. Altyapı maliyetlerini gelirin %10'unun altında tut. |
| API Ürünleri | Bulut barındırma fiyat değişiklikleri | Taşınabilirlik için tasarla. Container kullan. Göç runbook'unu belgele. |
| Danışmanlık | LinkedIn algoritması, iş panosu değişiklikleri | Doğrudan yönlendirme ağı kur. Portföylü kişisel web siteni koru. |
| Açık Kaynak | GitHub politika değişiklikleri, npm kayıt kuralları | Sürümleri yansıla. Kendi proje web sitene ve dokümantasyon alan adına sahip ol. |

> **Platform çeşitlendirmenin altın kuralı:** Müşterilerine doğrudan e-posta gönderemiyorsan, müşterilerin yok — platformun müşterileri var. Hangi motoru çalıştırırsan çalıştır, birinci günden e-posta listeni oluştur.

### Anti-Kalıplar

{? if dna.blind_spots ?}
Belirlenmiş kör noktaların — {= dna.blind_spots | fallback("areas you haven't explored") =} — seni "yenilikçi" hissettiren motorlara çekebilir. Buna diren. Mevcut güçlerin için işe yarayanı seç.
{? endif ?}

Bunları yapma:

1. **3+ motor seçme.** İki maksimum. Üç dikkatini çok fazla böler ve hiçbir şey iyi yapılmaz.

2. **İki yavaş motor seçme.** Her iki motor da gelir üretmek için 8+ hafta sürüyorsa, sonuçları görmeden motivasyonunu kaybedersin. En az bir motor 2 hafta içinde gelir üretmeli.

3. **Aynı kategoride iki motor seçme.** Bir mikro-SaaS ve bir API ürünü ikisi de "bir ürün inşa et" — çeşitlendirmiyorsun. Bir ürün motorunu bir hizmet motoru veya bir içerik motoruyla eşleştir.

4. **Matematiği atlama.** "Fiyatlandırmayı sonra hallederim" çalıştırma maliyetinden daha az kazanan bir ürünle sonuçlanmanın yolu.

5. **En etkileyici motor için optimize etme.** Danışmanlık gösterişli değil. Dijital ürünler "yenilikçi" değil. Ama para kazanırlar. Durumuna uygun olanı seç, Twitter'da iyi görüneni değil.

6. **Platform yoğunlaşmasını görmezden gelme.** Yukarıdaki Platform Bağımlılık Denetimini çalıştır. Herhangi bir platform gelirinin %40'ından fazlasını kontrol ediyorsa, çeşitlendirmek bir sonraki önceliğin olmalı — yeni bir motor eklemeden önce.

---

## 4DA Entegrasyonu

{@ mirror feed_predicts_engine @}

> **4DA Modül R ile nasıl bağlanır:**
>
> 4DA'nın sinyal tespiti, gelir motorlarının doldurduğu pazar boşluklarını bulur. Başlangıç kiti olmayan trend framework mü? Bir tane inşa et (Motor 1). Tutorial olmayan yeni LLM tekniği mi? Bir tane yaz (Motor 2). Göç kılavuzu olmayan bağımlılık güvenlik açığı mı? Birini oluştur ve ücretlendir (Motor 1, 2 veya 8).
>
> 4DA'nın `get_actionable_signals` aracı içeriği aciliyete göre sınıflandırır (taktiksel vs. stratejik) ve öncelik seviyeleriyle. Her sinyal türü doğal olarak gelir motorlarına eşlenir:
>
> | Sinyal Sınıflandırması | Öncelik | En İyi Gelir Motoru | Örnek |
> |----------------------|--------|-------------------|-------|
> | Taktiksel / Yüksek Öncelik | Acil | Danışmanlık, Dijital Ürünler | Yeni güvenlik açığı açıklandı — göç kılavuzu yaz veya iyileştirme danışmanlığı sun |
> | Taktiksel / Orta Öncelik | Bu hafta | İçerik Parasallaştırma, Dijital Ürünler | Trend kütüphane sürümü — ilk tutorial'ı yaz veya başlangıç kiti inşa et |
> | Stratejik / Yüksek Öncelik | Bu çeyrek | Mikro-SaaS, API Ürünleri | Birden fazla sinyal genelinde ortaya çıkan kalıp — pazar olgunlaşmadan araç inşa et |
> | Stratejik / Orta Öncelik | Bu yıl | Açık Kaynak + Premium, Veri Ürünleri | Bir teknoloji alanında anlatı kayması — açık kaynak çalışma veya istihbarat raporları aracılığıyla kendinizi uzman olarak konumlandır |
>
> Daha derine inmek için `get_actionable_signals`'ı diğer 4DA araçlarıyla eşleştir:
> - **`daily_briefing`** — AI tarafından oluşturulan yönetici özeti her sabah en yüksek öncelikli sinyalleri yüzeye çıkarır
> - **`knowledge_gaps`** — projenin bağımlılıklarındaki boşlukları bulur, bu boşlukları dolduran ürünler için fırsatları ortaya çıkarır
> - **`trend_analysis`** — istatistiksel kalıplar ve tahminler hangi teknolojilerin hızlandığını gösterir
> - **`semantic_shifts`** — bir teknolojinin "deneysel"den "üretim" benimsemesine geçtiğini tespit eder, pazar zamanlamasını işaret eder
>
> Kombinasyon geri bildirim döngüsüdür: **4DA fırsatı tespit eder. STREETS sana onu uygulamak için oyun kitabını verir. Gelir motorun sinyali gelire dönüştürür.**

---

## Modül R: Tamamlandı

### Dört Haftada Ne İnşa Ettin

Geri dön ve bu modülün başında nerede olduğuna bak. Altyapın (Modül S) ve savunulabilirliğin (Modül T) vardı. Şimdi şunlara sahipsin:

1. **Çalışan bir Motor 1** gelir üretiyor (veya günler içinde gelir üretecek altyapı)
2. **Motor 2 için detaylı bir plan** — zaman çizelgesi, gelir projeksiyonları ve ilk adımlarla
3. **Gerçek, deploy edilmiş kod** — sadece fikirler değil, çalışan ödeme akışları, API uç noktaları, içerik boru hatları veya ürün listeleri
4. **Bir karar matrisi** yeni bir fırsat belirdiğinde başvurabileceğin
5. **Gelir matematiği** hedeflerine ulaşmak için tam olarak kaç satış, müşteri veya aboneye ihtiyacın olduğunu söyleyen

### Temel Çıktı Kontrolü

Modül E'ye (Uygulama Oyun Kitabı) geçmeden önce doğrula:

- [ ] Motor 1 yayında. Bir şey deploy edilmiş, listelenmiş veya satın alma/kiralama için hazır.
- [ ] Motor 1 en az $1 gelir üretmiş (veya 7 gün içinde $1'a net bir yolun var)
- [ ] Motor 2 planlanmış. Kilometre taşları ve zaman çizelgesi ile yazılı bir planın var.
- [ ] Karar matrisin doldurulmuş. Bu iki motoru NEDEN seçtiğini biliyorsun.
- [ ] Gelir projeksiyon çalışma sayfan tamamlanmış. Ay 1, 3, 6 ve 12 hedeflerini biliyorsun.

Bunlardan herhangi biri eksikse, zamanı ayır. Modül E tüm bunların üzerine inşa eder. Çalışan bir Motor 1 olmadan ilerlemek, var olmayan bir ürünü optimize etmeye çalışmak gibidir.

{? if progress.completed_modules ?}
### STREETS İlerlemen

Şu ana kadar {= progress.total_count | fallback("7") =} modülden {= progress.completed_count | fallback("0") =} tanesini tamamladın ({= progress.completed_modules | fallback("none yet") =}). Modül R dönüm noktası — bundan önceki her şey hazırlıktı. Bundan sonraki her şey uygulama.
{? endif ?}

### Sırada Ne Var: Modül E — Uygulama Oyun Kitabı

Modül R sana motorları verdi. Modül E onları nasıl çalıştıracağını öğretir:

- **Lansman sıralamaları** — her motorun ilk 24 saatinde, ilk haftasında ve ilk ayında tam olarak ne yapılacağı
- **Fiyatlandırma psikolojisi** — neden $49, $39'dan daha iyi satar ve ne zaman indirim yapılır (neredeyse hiç)
- **İlk 10 müşterini bulmak** — her motor türü için spesifik, uygulanabilir taktikler
- **Önemli metrikler** — her aşamada ne takip edilir ve ne göz ardı edilir
- **Ne zaman pivot yapılır** — bir motorun çalışmadığını söyleyen sinyaller ve ne yapılacağı

Motorlar inşa edildi. Şimdi onları sürmeyi öğreniyorsun.

---

*Senin donanımın. Senin kuralların. Senin gelirin.*
