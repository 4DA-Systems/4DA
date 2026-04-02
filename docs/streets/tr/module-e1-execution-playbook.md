# Modul E: Uygulama Rehberi

**STREETS Gelistirici Gelir Kursu — Ucretli Modul**
*Hafta 9-10 | 6 Ders | Teslim Edilecek: Ilk Urunun, Canli ve Odeme Kabul Ediyor*

> "Fikirden deploy'a 48 saat icinde. Fazla dusunme yok."

---

Altyapın var (Modul S). Savunma hendegın var (Modul T). Gelir motoru tasarımların var (Modul R). Simdi lansman zamanı.

Bu modul, cogu gelistiricinin asla ulasamadıgı modul — zor oldugu icin degil, hala kod tabanını cilaladıkları, mimarileri yeniden duzenledikleri, renk paletlerini ayarladıkları icin. Onemli olan tek sey disinda her seyi yapıyorlar: bir urunu, para odeyebilecek bir insanın onune koymak.

Lansman bir beceridir. Her beceri gibi, pratikle kolaylasır ve ertelemeyle zorlasır. Ne kadar beklersen, o kadar zorlasır. Ne kadar cok lansırsan, o kadar az korkutucu olur. Ilk lansmanın dagınık olacak. Zaten mesele bu.

Bu iki haftanın sonunda sende su seyler olacak:

- Gercek talep sinyallerine karsı test edilmis, dogrulanmıs bir urun fikri
- Gercek bir domain uzerinden erisilebilir, canli, deploy edilmis bir urun
- Gercek para kabul eden odeme isleme
- Hedef kitlenin toplastıgı bir platformda en az bir acik lansman
- Sonraki adımlarını yonlendirecek bir lansman sonrası metrik sistemi

Varsayımlar yok. "Teoride" yok. Gercek bir urun, internette canli, gelir uretebilir durumda.

{? if progress.completed("R") ?}
Modul R'yi tamamladın — yurume hazır gelir motoru tasarımların zaten var. Bu modul bu tasarımlardan birini canli bir urune donusturur.
{? else ?}
Modul R'yi henuz tamamlamadıysan, bu modulu yine de kullanabilirsin — ama hazır bir gelir motoru tasarımına sahip olmak 48 saatlik sprinti onemli olcude daha puruzsuz yapacaktır.
{? endif ?}

{@ mirror execution_readiness @}

Hadi insa edelim.

---

## Ders 1: 48 Saatlik Sprint

*"Cumartesi sabahından pazar gecesine. Bir urun. Sıfır mazeret."*

### Neden 48 Saat

Parkinson Yasası, isin kendisine ayrılan zamanı dolduracak sekilde genisleyecegini soyler. Kendine bir urun insa etmek icin 6 ay ver ve 5 ayını dusunerek, 1 ayını stresli bir telaş icinde gecireceksin. Kendine 48 saat ver ve kararlar alacak, kapsamı acımasızca kesecek ve gercek bir sey lansıracaksın.

48 saatlik kısıtlama mukemmel bir sey insa etmekle ilgili degil. Var olan bir sey insa etmekle ilgili. Varoluş her zaman mukemmelligi yener, cunku canli bir urun veri uretir — kim ziyaret ediyor, kim tıklıyor, kim odeme yapıyor, kim sikayet ediyor — ve veri sana bundan sonra ne insa edecegini soyler.

Inceledigim her basarılı gelistirici urunu bu kalıbı izledi: hızlı lanse et, hızlı ogren, hızlı iter et. Basarısız olanlar? Hepsinin guzel README dosyaları ve sıfır kullanıcısı var.

Iste dakika dakika eylem planın.

### Gun 1 — Cumartesi

#### Sabah Bloku (4 saat): Talebi Dogrula

Tek bir satır kod yazmadan once, senden baska birinin bunu istedigine dair kanıt bulmaya ihtiyacın var. Kesinlik degil — kanıt. Fark onemli. Kesinlik imkansız. Kanıt 4 saatte elde edilebilir.

**Adım 1: Arama Hacmi Kontrolu (45 dakika)**

Su kaynaklara git ve urun fikrin ve ilgili terimleri ara:

- **Google Trends** (https://trends.google.com) — Ucretsiz. Zaman icindeki goreceli arama ilgisini gosterir. Duz veya yukselis grafigi gormek istiyorsun, dusus degil.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Site dogrulama ile ucretsiz. Anahtar kelime hacimlerini gosterir.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — Ucretsiz katman gunde 3 arama verir. Arama hacmi, zorluk ve ilgili terimleri gosterir.
- **AlsoAsked** (https://alsoasked.com) — Ucretsiz katman. Google'dan "Insanlar Bunu da Sordu" verilerini gosterir. Insanların gercekte hangi soruları sordugunu ortaya cıkarır.

Ne arıyorsun:

```
IYI sinyaller:
- Ana anahtar kelimen icin ayda 500+ arama
- Son 12 ayda yukselis trendi
- Iyi yanıtları olmayan birden fazla "Insanlar Bunu da Sordu" sorusu
- Dusuk rekabetli ilgili uzun kuyruk anahtar kelimeler

KOTU sinyaller:
- Dusen arama ilgisi
- Sıfır arama hacmi (kimse bunu aramıyor)
- Sayfa 1'de dev sirketlerin hakimiyeti
- Arama terimlerinde cesitlilik yok (cok dar)
```

Gercek ornek: Modul R'deki gelir motoru fikrinin "SaaS dashboard'ları icin Tailwind CSS bileşen kutuphanesi" oldugunu varsayalım.

```
Arama: "tailwind dashboard components" — 2.900/ay, yukselis trendi
Arama: "tailwind admin template" — 6.600/ay, stabil
Arama: "react dashboard template tailwind" — 1.300/ay, yukselis
Ilgili: "shadcn dashboard", "tailwind analytics components"

Sonuc: Guclu talep. Birden fazla anahtar kelime acısı. Devam et.
```

Diger ornek: Fikrinin "Rust tabanlı log dosyası anonimlestiricisi" oldugunu varsayalım.

```
Arama: "log file anonymizer" — 90/ay, duz
Arama: "anonymize log files" — 140/ay, duz
Arama: "PII removal from logs" — 320/ay, yukselis
Ilgili: "GDPR log compliance", "scrub PII from logs"

Sonuc: Nis ama buyuyen. "PII removal" acısı "anonymizer" acısından
daha fazla hacme sahip. Konumlandırmanı yeniden duzenle.
```

**Adım 2: Topluluk Baslik Madenciligi (60 dakika)**

Gelistiricilerin istekte bulundugu yerlere git ve problem alanını ara:

- **Reddit:** r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming ve alanınla ilgili nis subreddit'lerde ara
- **Hacker News:** Gecmis tartısmaları aramak icin https://hn.algolia.com kullan
- **GitHub Issues:** Alanınla ilgili populer repo'lardaki issue'ları ara
- **Stack Overflow:** Cok oy almıs ama tatmin edici kabul edilmis yanıtları olmayan soruları ara
- **Discord sunucuları:** Ilgili gelistirici toplulugu sunucularını kontrol et

Ne belgeliyorsun:

```markdown
## Baslik Madenciligi Sonucları

### Baslik 1
- **Kaynak:** Reddit r/reactjs
- **URL:** [link]
- **Baslik:** "Is there a good Tailwind dashboard kit that isn't $200?"
- **Oy:** 147
- **Yorum:** 83
- **Anahtar alıntılar:**
  - "Everything on the market is either free and ugly, or $200+ and overkill"
  - "I just need 10-15 well-designed components, not 500"
  - "Would pay $49 for something that actually looks good out of the box"
- **Cıkarım:** $200+ fiyat hassasiyeti, $29-49 arasında odeme istekliligi

### Baslik 2
- ...
```

En az 5 baslik bul. Urunun alanında insanların bir sey istedigini gosteren 5 baslik bulamıyorsan, bu ciddi bir uyarı isarettir. Ya talep yok, ya da yanlıs terimlerle arıyorsun. Fikirden vazgecmeden once farklı anahtar kelimeler dene.

**Adım 3: Rakip Denetimi (45 dakika)**

Halihazırda ne oldugunu ara. Bu cesaret kırıcı degil — dogrulayıcı. Rakipler bir pazar oldugu anlamına gelir. Rakip olmamasi genellikle pazar olmadıgı anlamına gelir, mavi okyanus bulduguna degil.

Her rakip icin belgele:

```markdown
## Rakip Denetimi

### Rakip 1: [Isim]
- **URL:** [link]
- **Fiyat:** $XX
- **Iyi yaptıkları:** [spesifik seyler]
- **Kotu olan:** [incelemelerden/basliklardan spesifik sikayetler]
- **Incelemeleri:** [G2, ProductHunt incelemeleri, Reddit bahisleri kontrol et]
- **Senin acin:** [nasıl farklı yapardın]

### Rakip 2: [Isim]
- ...
```

Altın "kotu olan"da. Bir rakip hakkındaki her sikayet, senin urunun icin bir ozellik istegi. Insanlar sana tam anlamıyla ne insa edecegini ve ne ucret alacagını soyluyor.

**Adım 4: "10 Kisi Oder" Testi (30 dakika)**

Bu son dogrulama gecidi. Bunun icin en az 10 kisinin para odeyecegine dair kanıt bulman gerekiyor. "Ilgi gosterdiler" degil. "Havalı dediler" degil. Oderlerdi.

Kanıt kaynakları:
- Reddit baslıklarında insanların "X icin oderdim" dedigi yerler (en guclu sinyal)
- Odeyen musterileri olan rakip urunler (pazarın odedigini kanıtlar)
- Alanındaki gorunur satıs sayıları olan Gumroad/Lemon Squeezy urunleri
- 1.000+ yıldızlı ilgili bir problemi cozen GitHub repo'ları (insanlar buna yıldız verecek kadar deger veriyor)
- Varsa kendi kitlen (tweet at, 10 kisiye DM gonder, dogrudan sor)

Bu testi gectiysen: devam et. Insa et.

Bu testi gecemedinse: acını donustur, tum fikrinden vazgecme. Talep bitisik bir alanda olabilir. Vazgecmeden once farklı konumlandırma dene.

> **Acık Konusma:** Cogu gelistirici, kod yazmak istedikleri icin dogrulamayı tamamen atlar. Kimsenin istemedigini insa etmek icin 200 saat harcarlar, sonra neden kimsenin almadıgını merak ederler. Bu 4 saatlik arastırma sana 196 saat bosa harcanan efordan tasarruf ettirecek. Bunu atlama. Kod kolay kısım.

#### Ogle Bloku (4 saat): MVP'yi Insa Et

Talebi dogruladın. Rakip arastırman var. Insanların ne istedigini ve mevcut cozumlerin neyi eksik bıraktıgını biliyorsun. Simdi temel problemi cozen minimum versiyonu insa et.

{? if profile.gpu.exists ?}
Sisteminde GPU ile ({= profile.gpu.model | fallback("GPU'n") =}), yerel AI cıkarımından yararlanan urun fikirlerini dusun — goruntu isleme aracları, kod analiz yardımcıları, icerik uretme boru hatları. GPU destekli ozellikler, cogu bagımsız gelistiricinin sunamadıgı gercek bir farklılastırıcı.
{? endif ?}

**3 Ozellik Kuralı**

v0.1'in tam olarak 3 ozellige sahip. 4 degil. 7 degil. Uc.

Nasıl secilir:
1. Urunun yaptıgı TEK sey ne? (Ozellik 1 — cekirdek)
2. Onu kullanılabilir yapan ne? (Ozellik 2 — genellikle kimlik dogrulama, veya kaydet/dısa aktar, veya yapilandırma)
3. Alternatiflere karsı odemeye deger yapan ne? (Ozellik 3 — farklılastırıcın)

Geri kalan her sey bu hafta sonu dokunmadıgın bir "v0.2" listesine gider.

Gercek ornek — Tailwind dashboard bilesen kutuphanesi:
1. **Cekirdek:** 12 uretim hazır dashboard bileseni (grafikler, tablolar, istatistik kartları, navigasyon)
2. **Kullanılabilir:** Canli onizlemeli copy-paste kod parcaları
3. **Farklılastırıcı:** Yerlesik karanlık mod, birlikte calısmak uzere tasarlanmıs bilesenler (rastgele koleksiyon degil)

Gercek ornek — PII log temizleyici CLI aracı:
1. **Cekirdek:** Log dosyalarından PII tespiti ve sansurlemesi (e-postalar, IP'ler, isimler, SSN'ler)
2. **Kullanılabilir:** CLI pipe olarak calısır (`cat logs.txt | pii-scrub > clean.txt`)
3. **Farklılastırıcı:** Yapilandırılabilir kurallar dosyası, otomatik olarak 15+ log formatını isler

{@ insight stack_fit @}

**Proje Iskeletini Olustur**

LLM'leri isini hızlandırmak icin kullan, degistirmek icin degil. Iste pratik is akısı:

{? if stack.contains("react") ?}
Ana teknoloji yıgının React icerdiginden, asagıdaki web uygulaması iskeleti en hızlı yolun. Araclari zaten biliyorsun — 48 saatini urun mantıgına odakla, yeni bir framework ogrenmek icin degil.
{? elif stack.contains("rust") ?}
Ana teknoloji yıgının Rust icerdiginden, asagıdaki CLI aracı iskeleti en hızlı yolun. Rust CLI aracları mukemmel dagıtıma sahiptir (tek binary, platformlar arası) ve gelistirici kitleleri performans hikayesine saygı duyar.
{? elif stack.contains("python") ?}
Ana teknoloji yıgının Python icerdiginden, bir CLI aracı veya API hizmeti dusun. Python, FastAPI veya Typer ile hızlı lansır ve PyPI ekosistemi sana milyonlarca gelistiriciye anında dagıtım saglar.
{? endif ?}

```bash
# Web uygulaması iskeleti (SaaS aracı, docs siteli bilesen kutuphanesi vb.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Tailwind CSS ekle (gelistirici urunleri icin en yaygın)
pnpm install -D tailwindcss @tailwindcss/vite

# Birden fazla sayfa gerekiyorsa yonlendirme ekle
pnpm install react-router-dom

# Proje yapısı — 48 saatlik insa icin duz tut
mkdir -p src/components src/pages src/lib
```

```bash
# CLI aracı iskeleti (gelistirici yardımcıları icin)
cargo init my-tool
cd my-tool

# CLI aracları icin yaygın bagımlılıklar
cargo add clap --features derive    # Arguman ayristirma
cargo add serde --features derive   # Serializasyon
cargo add serde_json                # JSON isleme
cargo add anyhow                    # Hata isleme
cargo add regex                     # Kalıp eslestirme
```

```bash
# npm paketi iskeleti (kutuphaneler/yardımcılar icin)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**Insa Icin LLM Is Akısı**

{? if settings.has_llm ?}
Bir LLM yapılandırmıs durumdasın ({= settings.llm_provider | fallback("yerel") =} / {= settings.llm_model | fallback("modelin") =}). Sprint sırasında esli programcın olarak kullan — iskelet ve boilerplate uretimini onemli olcude hızlandırır.
{? endif ?}

LLM'den tum urununu insa etmesini isteme. Bu jenerik, kırılgan kod uretir. Bunun yerine:

1. **Sen** mimariyi yaz: dosya yapısı, veri akısı, anahtar arayuzler
2. **LLM** boilerplate uretir: tekrarlanan bilesenler, yardımcı fonksiyonlar, tip tanımları
3. **Sen** cekirdek mantıgı yaz: urununu farklı kılan kısım
4. **LLM** testler uretir: birim testleri, uç durumlar, entegrasyon testleri
5. **Sen** her seyi incele ve duzenle: ismin bu urunde

Kod yazarken paralel is: ikinci bir LLM sohbeti ac ve landing page metnini, README'yi ve belgeleri taslak olarak yazdır. Bunları aksam duzenleyeceksin, ama ilk taslaklar hazır olacak.

**Zaman Disiplini**

```
14:00 — Ozellik 1 (cekirdek islevsellik): 2 saat
         16:00'ya kadar calısmıyorsa, kapsami kes.
16:00 — Ozellik 2 (kullanılabilirlik): 1 saat
         Basit tut. Cilayı sonra lansırsın.
17:00 — Ozellik 3 (farklılastırıcı): 1 saat
         Odemeye deger yapan bu. Buraya odaklan.
18:00 — KOD YAZMAYI BIRAK. Mukemmel olması gerekmiyor.
```

> **Yaygın Hata:** "Durmadan once bir ozellik daha." Hafta sonu projelerinin aylık projelere donusmesi boyle olur. 3 ozellik senin kapsamın. Insa sırasında harika bir fikir gelirse, v0.2 listene yaz ve devam et. Odeyen musterilerin olduktan sonra gelecek hafta ekleyebilirsin.

#### Aksam Bloku (2 saat): Landing Page Yaz

Landing page'in tek bir isi var: bir ziyaretciyi odemeye ikna etmek. Guzel olması gerekmiyor. Net olması gerekiyor.

**5 Bolumlu Landing Page**

Her basarılı gelistirici urunu landing page'i bu yapıyı takip eder. Yeniden icat etme:

```
Bolum 1: BASLIK + ALT BASLIK
  - 8 kelime veya daha azıyla ne yaptıgı
  - Kimin icin ve ne sonuc elde edecekleri

Bolum 2: PROBLEM
  - Hedef musterinin tanıyacagı 3 acı noktası
  - Baslik madenciliginden onların tam dilini kullan

Bolum 3: COZUM
  - Urunun ekran goruntuleri veya kod ornekleri
  - Yukarıdaki 3 acı noktasıyla eslesen 3 ozellik

Bolum 4: FIYATLANDIRMA
  - Bir veya iki katman. v0.1 icin basit tut.
  - Abonelikse yıllık faturalama secenegi.

Bolum 5: CTA (Eyleme Cagrı)
  - Bir buton. "Basla", "Satın Al", "Indir".
  - Temel faydayı tekrarla.
```

**Gercek Metin Ornegi — Tailwind Dashboard Kiti:**

```markdown
# Bolum 1
## DashKit — Uretim Hazır Tailwind Dashboard Bilesenleri
SaaS dashboard'unu saatler icinde lanse et, haftalar degil.
12 copy-paste bilesen. Karanlık mod. $29.

# Bolum 2
## Problem
- Jenerik UI kitleri sana 500 bilesen verir ama sıfır tutarlılık
- Sıfırdan dashboard UI insa etmek 40+ saat surer
- Ucretsiz secenekler 2018'den Bootstrap gibi gorunuyor

# Bolum 3
## Ne Alıyorsun
- **12 bilesen** birlikte calısmak uzere tasarlanmıs (rastgele koleksiyon degil)
- **Karanlık mod** yerlesik — bir prop ile degistir
- **Copy-paste kod** — npm install yok, bagımlılık yok, baglılık yok
[bilesen orneklerinin ekran goruntusu]

# Bolum 4
## Fiyatlandırma
**DashKit** — $29 tek seferlik
- Kaynak kodlu tum 12 bilesen
- 12 ay ucretsiz guncelleme
- Sınırsız projede kullan

**DashKit Pro** — $59 tek seferlik
- DashKit'teki her sey
- 8 tam sayfa sablonu (analitik, CRM, yonetim, ayarlar)
- Figma tasarım dosyaları
- Oncelikli ozellik istekleri

# Bolum 5
## Dashboard'unu bu hafta sonu lanse et.
[DashKit Satın Al — $29]
```

**Gercek Metin Ornegi — PII Log Temizleyici:**

```markdown
# Bolum 1
## ScrubLog — Log Dosyalarından PII'yı Saniyeler Icinde Temizle
Logların icin GDPR uyumlulugu. Tek komut.

# Bolum 2
## Problem
- Logların saklamaman gereken e-postalar, IP'ler ve isimler iceriyor
- Manuel sansurleme saatler suruyor ve bir seyleri kacırıyor
- Kurumsal araclar $500/ay ve yapılandırmak icin PhD gerektiyor

# Bolum 3
## Nasıl Calısır
```bash
cat server.log | scrublog > clean.log
```
- 15+ PII kalıbını otomatik tespit eder
- YAML yapilandırma ile ozel kurallar
- JSON, Apache, Nginx ve duz metin formatlarını isler
[once/sonra gosteren terminal ekran goruntusu]

# Bolum 4
## Fiyatlandırma
**Kisisel** — Ucretsiz
- 5 PII kalıbı, 1 log formatı

**Pro** — $19/ay
- Tum 15+ PII kalıbı
- Tum log formatları
- Ozel kurallar
- Takım yapilandırma paylasımı

# Bolum 5
## Ihtiyacın olmayan PII'yı saklamayı bırak.
[ScrubLog Pro Al — $19/ay]
```

**Metin Icin LLM Is Akısı:**

1. LLM'e rakip denetimini ve baslik madenciligi sonuclarını ver
2. 5 bolum sablonunu kullanarak landing page metni taslak olarak yazmasını iste
3. Acımasızca duzenle: her belirsiz ifadeyi spesifik bir ifadeyle degistir
4. Sesli oku. Herhangi bir cumle seni rahatsız ediyorsa, yeniden yaz.

**Landing Page'i Insa Etmek:**

48 saatlik bir sprint icin sıfırdan ozel bir landing page insa etme. Bunlardan birini kullan:

{? if stack.contains("react") ?}
- **React uygulamanız** — React'te calıstıgın icin, landing page'i uygulamanın cıkıs yapılmıs ana sayfası yap veya Next.js'de bir pazarlama rotası ekle. Sıfır baglam degisim maliyeti.
{? endif ?}
- **Urunun kendi sitesi** — Web uygulamasıysa, landing page'i cıkıs yapılmıs ana sayfa yap
- **Astro + Tailwind** — Statik site, Vercel'e 2 dakikada deploy, asırı hızlı
- **Next.js** — Urunun zaten React ise, bir pazarlama sayfası rotası ekle
- **Framer** (https://framer.com) — Gorsel oluşturucu, temiz kod dısa aktarır, ucretsiz katman mevcut
- **Carrd** (https://carrd.co) — $19/yıl, son derece basit tek sayfalık siteler

```bash
# En hızlı yol: Astro statik site
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Tailwind ekle
pnpm astro add tailwind
```

Cumartesi sonuna kadar metin iceren bir landing page'in olmalı. Ozel illustrasyonlara ihtiyacı yok. Animasyonlara ihtiyacı yok. Net kelimelere ve bir satın alma butonuna ihtiyacı var.

### Gun 2 — Pazar

#### Sabah Bloku (3 saat): Deploy

Urunun gercek bir URL'de internette canli olmali. Localhost degil. Rastgele hash'li Vercel onizleme URL'si degil. Gercek bir domain, HTTPS ile, paylasabilecegin ve insanların ziyaret edebilecegi.

**Adım 1: Uygulamayı Deploy Et (60 dakika)**

{? if computed.os_family == "windows" ?}
Windows'ta oldugundan, deploy araclarin gerektiriyorsa WSL2'nin kullanılabilir oldugundan emin ol. Cogu CLI deploy aracı (Vercel, Fly.io) Windows'ta yerel olarak calısır, ama bazı betikler Unix yollarını varsayar.
{? elif computed.os_family == "macos" ?}
macOS'ta tum deploy CLI'ları Homebrew veya dogrudan indirme ile temiz yuklenir. En puruzsuz deploy yolundasın.
{? elif computed.os_family == "linux" ?}
Linux'ta en esnek deploy ortamına sahipsin. Tum CLI aracları yerel olarak calısır ve statik IP'n varsa ve barındırma maliyetlerinden tasarruf etmek istiyorsan kendi makinende de barındırabilirsin.
{? endif ?}

Insa ettigine gore deploy platformunu sec:

**Statik site / SPA (bilesen kutuphanesi, landing page, docs sitesi):**
```bash
# Vercel — statik siteler ve Next.js icin en hızlı yol
pnpm install -g vercel
vercel

# Sorular soracak. Her seye evet de.
# Siten ~60 saniye icinde canli.
```

**Backend'li web uygulaması (SaaS aracı, API hizmeti):**
```bash
# Railway — basit, iyi ucretsiz katman, veritabanlarını destekler
# https://railway.app
# GitHub reponuzu baglayın ve deploy edin.

# Veya Fly.io — daha fazla kontrol, global edge deploy
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**CLI aracı / npm paketi:**
```bash
# npm kayıt defteri
npm publish

# Veya GitHub Releases uzerinden binary olarak dagıt
# Rust projeleri icin cargo-dist kullan
cargo install cargo-dist
cargo dist init
cargo dist build
# Binary'leri GitHub release'ine yukle
```

**Adım 2: Domain Satın Al (30 dakika)**

Gercek bir domain yılda $12. Isine $12 yatırım yapamıyorsan, is sahibi olma konusunda ciddi degilsin.

**Nereden alınır:**
- **Namecheap** (https://namecheap.com) — .com icin $8-12/yıl, iyi DNS yonetimi
- **Cloudflare Registrar** (https://dash.cloudflare.com) — Maliyet fiyatına (.com icin genellikle $9-10/yıl), mukemmel DNS
- **Porkbun** (https://porkbun.com) — Ilk yıl icin genellikle en ucuz, iyi UI

**Domain isimlendirme ipucları:**
- Kısa daha iyi. 2 hece ideal, maksimum 3.
- `.com` guven icin hala kazanır. `.dev` ve `.io` gelistirici aracları icin uygun.
- Musteriligi kayıt sirketinde kontrol et, GoDaddy'de degil (aramaları on-alım yaparlar).
- Secmek icin 15 dakikadan fazla harcama. Isim dustugunun cok daha az onemli.

```bash
# Domain'ini Vercel'e yonlendir
# Vercel panelinde: Settings > Domains > Domain'ini ekle
# Sonra kayıt sirketi DNS ayarlarında ekle:
# A kaydı: @ -> 76.76.21.21
# CNAME kaydı: www -> cname.vercel-dns.com

# Veya DNS icin Cloudflare kullanıyorsan:
# Aynı kayıtları Cloudflare DNS panelinde ekle
# SSL hem Vercel hem Cloudflare ile otomatik
```

**Adım 3: Temel Izleme (30 dakika)**

Iki seyi bilmen gerekiyor: site ayakta mı ve insanlar ziyaret ediyor mu.

**Calısma suresi izleme (ucretsiz):**
- **Better Uptime** (https://betteruptime.com) — Ucretsiz katman 10 URL'yi her 3 dakikada izler
- **UptimeRobot** (https://uptimerobot.com) — Ucretsiz katman 50 URL'yi her 5 dakikada izler

```
Izleme ayarla:
1. Landing page URL'in
2. Uygulamanın health endpoint'i (uygulanabilirse)
3. Odeme webhook URL'in (kritik — odemelerin bozulup bozulmadıgını bilmen gerek)
```

**Analitik (gizliligi koruyan):**

Google Analytics kullanma. Gelistirici kitlen onu engelliyor, yeni bir urun icin asırı ve gizlilik yukumlulugu.

- **Plausible** (https://plausible.io) — $9/ay, gizlilik oncelikli, tek satır betik
- **Fathom** (https://usefathom.com) — $14/ay, gizlilik oncelikli, hafif
- **Umami** (https://umami.is) — Ucretsiz ve self-hosted, veya $9/ay bulut

```html
<!-- Plausible — <head>'inde tek satır -->
<script defer data-domain="senindomain.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — <head>'inde tek satır -->
<script defer
  src="https://senin-umami-orneginin.com/script.js"
  data-website-id="senin-website-id"></script>
```

> **Acık Konusma:** Evet, henuz para kazanmamıs bir urunde analitik icin $9/ay gereksiz hissettiriyor. Ama olcemedigin seyi gelistiremezsin. Ilk ay analitik verileri, bir ay tahmin etmekten pazarın hakkında daha fazla sey soyleyecek. $9/ay butceni kırarsa, Umami'yi Railway'de ucretsiz barındır.

#### Ogle Bloku (2 saat): Odemeleri Ayarla

Urunun para kabul edemiyorsa, hobi projesi. Odeme ayarlama cogu gelistiricinin dustugunun aksine daha az zaman alır — temel akıs icin yaklaşık 20-30 dakika.

{? if regional.country ?}
> **{= regional.country | fallback("ulken") =} icin onerilen odeme islemcileri:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy, PayPal") =}. Asagıdaki secenekler kuresel olarak kullanılabilir, ama tercih ettigin islemcinin {= regional.currency | fallback("yerel para birimin") =} ile odemeleri destekleyip desteklemedigini kontrol et.
{? endif ?}

**Secenek A: Lemon Squeezy (Dijital Urunler Icin Onerilen)**

Lemon Squeezy (https://lemonsqueezy.com) odeme isleme, satıs vergisi, KDV ve dijital teslimati tek platformda yonetir. Sıfırdan odeme kabul etmeye en hızlı yol.

Ilk urunun icin neden Stripe yerine Lemon Squeezy:
- Merchant of Record olarak hareket eder — satıs vergisi, KDV ve uyumlulugu senin icin yonetirler
- Yerlesik odeme sayfaları — frontend calısması gerekmez
- Yerlesik dijital teslimat — dosyalarını yukle, erisimi yonetirler
- Islem basına %5 + $0.50 (Stripe'dan yuksek, ama saatlerce vergi bas agrısından kurtarır)

Kurulum adımları:
1. https://app.lemonsqueezy.com'a kaydol
2. Magaza olustur (isletme adın)
3. Urun ekle:
   - Isim, acıklama, fiyat
   - Dijital teslimat icin dosya yukle (uygulanabilirse)
   - Lisans anahtarları ayarla (yazılım satıyorsan)
4. Odeme URL'ini al — "Satın Al" butonun buna baglantı verir
5. Satın alma sonrası otomasyon icin webhook ayarla

```javascript
// Lemon Squeezy webhook isleyicisi (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Webhook imzasını dogrula
  const signature = req.headers['x-signature'];
  const hmac = crypto.createHmac('sha256', WEBHOOK_SECRET);
  const digest = hmac.update(JSON.stringify(req.body)).digest('hex');

  if (signature !== digest) {
    return res.status(401).json({ error: 'Invalid signature' });
  }

  const event = req.body;

  switch (event.meta.event_name) {
    case 'order_created': {
      const order = event.data;
      const customerEmail = order.attributes.user_email;
      const productId = order.attributes.first_order_item.product_id;
      const orderId = order.id;

      console.log(`New order: ${orderId} from ${customerEmail}`);

      // Karsilama e-postası gonder, erisim ver, lisans anahtarı olustur vb.
      await grantProductAccess(customerEmail, productId);
      await sendWelcomeEmail(customerEmail, orderId);

      break;
    }

    case 'subscription_created': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`New subscription from ${customerEmail}`);
      await createSubscription(customerEmail, subscription);

      break;
    }

    case 'subscription_cancelled': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`Subscription cancelled: ${customerEmail}`);
      await revokeAccess(customerEmail);

      break;
    }

    default:
      console.log(`Unhandled event: ${event.meta.event_name}`);
  }

  return res.status(200).json({ received: true });
}
```

**Secenek B: Stripe (Daha Fazla Kontrol, Daha Fazla Is)**

Stripe (https://stripe.com) sana daha fazla kontrol verir ama vergi uyumlulugunun ayrıca yonetilmesini gerektirir. Karmasık faturalandırmalı SaaS icin daha iyi.

```javascript
// Stripe Checkout oturumu (Node.js)
// Barındırılan bir odeme sayfası olusturur

import Stripe from 'stripe';

const stripe = new Stripe(process.env.STRIPE_SECRET_KEY);

export async function createCheckoutSession(req, res) {
  const session = await stripe.checkout.sessions.create({
    payment_method_types: ['card'],
    line_items: [
      {
        price_data: {
          currency: 'usd',
          product_data: {
            name: 'DashKit Pro',
            description: '12 Tailwind dashboard components + 8 templates + Figma files',
          },
          unit_amount: 5900, // cent cinsinden $59.00
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // tekrarlayan icin 'subscription'
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Varsa on doldur
  });

  return res.json({ url: session.url });
}

// Stripe webhook isleyicisi
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // ham govde, ayrıstırılmıs JSON degil
      sig,
      process.env.STRIPE_WEBHOOK_SECRET
    );
  } catch (err) {
    console.error(`Webhook signature verification failed: ${err.message}`);
    return res.status(400).send(`Webhook Error: ${err.message}`);
  }

  switch (event.type) {
    case 'checkout.session.completed': {
      const session = event.data.object;
      await fulfillOrder(session);
      break;
    }
    case 'customer.subscription.deleted': {
      const subscription = event.data.object;
      await revokeSubscriptionAccess(subscription);
      break;
    }
  }

  return res.json({ received: true });
}
```

**Her Iki Platform Icin — Lansırlamadan Once Test Et:**

```bash
# Lemon Squeezy: Panelde test modunu kullan
# Lemon Squeezy panelinin sag ustundeki "Test mode"u ac
# Kart numarası: 4242 4242 4242 4242, herhangi gelecek son kullanma, herhangi CVC

# Stripe: Test modu API anahtarlarını kullan
# Test kartı: 4242 4242 4242 4242
# Test reddedilen kart: 4000 0000 0000 0002
# Kimlik dogrulama gerektiren test kartı: 4000 0025 0000 3155
```

Tum satın alma akısını test modunda kendin yap. Satın al butonuna tıkla, odemeyi tamamla, webhook'un tetiklendigini dogrula, erişimin verildigini dogrula. Test modunda herhangi bir adım basarısız olursa, gercek musteriler icin de basarısız olacaktır.

> **Yaygın Hata:** "Odemeleri sonra ayarlarım, biraz kullanıcı edindikten sonra." Bu tersine. Odeme ayarlamak bugun para toplamakla ilgili degil — birinin odeyip odemeyecegini dogrulamakla ilgili. Fiyatsız bir urun ucretsiz bir aractır. Fiyatlı bir urun bir is testidir. Fiyatın kendisi dogrulamanın parcası.

#### Aksam Bloku (3 saat): Lansman

Urunun canli. Odemeler calısıyor. Landing page net. Simdi insanların gormesi gerekiyor.

**Yumuşak Lansman Stratejisi**

Ilk urunun icin "buyuk lansman" yapma. Buyuk lansmanlar mukemmel olma baskısı yaratır ve v0.1'in mukemmel degil. Bunun yerine, yumusak lansman yap: birkaç yerde paylas, geri bildirim topla, kritik sorunları duzelt, sonra 1-2 hafta icinde buyuk lansmanı yap.

**Lansman Platformu 1: Reddit (30 dakika)**

r/SideProject'te ve urunünle ilgili bir nis subreddit'te paylas.

Reddit gonderi sablonu:

```markdown
Baslik: I built [ne yaptıgı] in a weekend — [anahtar fayda]

Govde:
Hey [subreddit],

I've been frustrated with [problem] for a while, so I built
[urun adı] this weekend.

**What it does:**
- [Ozellik 1 — cekirdek deger]
- [Ozellik 2]
- [Ozellik 3]

**What makes it different from [rakip]:**
[Farklılastırıcın hakkında durust bir paragraf]

**Pricing:**
[Seffaf ol. "$29 one-time" veya "Free tier + $19/mo Pro"]

I'd love feedback. What am I missing? What would make this
useful for your workflow?

[Urune baglantı]
```

Reddit gonderileri icin kurallar:
- Gercekten yardımcı ol, satıscı degil
- Her yoruma yanıt ver (bu istege baglı degil)
- Elestiriyi zarifce kabul et — olumsuz geri bildirim en degerli tur
- Astroturfing yapma (sahte oylar, birden fazla hesap). Yakalanır ve ban yersin.

**Lansman Platformu 2: Hacker News (30 dakika)**

Urunun teknik ve ilginc ise, Show HN paylas. "Teknik detaylar" bolumunde yıgınını ({= stack.primary | fallback("ana yıgının") =}) belirt ve neden sectigini acıkla — HN okuyucuları bilgilendirilmis teknik kararları sever.

Show HN sablonu:

```markdown
Baslik: Show HN: [Urun Adı] – [ne yaptıgı <70 karakter]

Govde:
[Urun adı] is [ne yaptıgını acıklayan bir cumle].

I built this because [gercek motivasyon — kendin icin hangi sorunu
cozuyordun].

Technical details:
- Built with [yıgın]
- [Ilginc teknik karar ve neden]
- [Uygulamayı dikkat cekici yapan sey]

Try it: [URL]

Feedback welcome. I'm particularly interested in [HN kitlesi icin
spesifik soru].
```

HN ipucları:
- ABD Dogu Saati ile 7-9 arasında paylas (en yuksek trafik)
- Baslik her seyden daha onemli. Spesifik ve teknik ol.
- HN okuyucuları pazarlama cilasından cok teknik icerigi takdir eder
- Ilk 2 saatte yorumlara anında yanıt ver. Yorum hızı sıralamayı etkiler.
- Oy icin yalvarma. Sadece paylas ve etkilesimde kal.

**Lansman Platformu 3: Twitter/X (30 dakika)**

Build-in-public lansman dizisi yaz:

```
Tweet 1 (Kanca):
I built [urun] in 48 hours this weekend.

It [spesifik problemi cozer] for [spesifik kitle].

Here's what I shipped, what I learned, and the real numbers. Thread:

Tweet 2 (Problem):
The problem:
[Acı noktasını 2-3 cumlede betimle]
[Acıyı gosteren ekran goruntusu veya kod ornegi ekle]

Tweet 3 (Cozum):
So I built [urun adı].

[Urunun aksiyondaki ekran goruntusu/GIF]

It does three things:
1. [Ozellik 1]
2. [Ozellik 2]
3. [Ozellik 3]

Tweet 4 (Teknik Detay):
Tech stack for the nerds:
- [Frontend]
- [Backend]
- [Barındırma — spesifik platform belirt]
- [Odemeler — Lemon Squeezy/Stripe belirt]
- Total cost to run: $XX/month

Tweet 5 (Fiyat):
Pricing:
[Net fiyatlandırma, landing page ile aynı]
[Urune baglantı]

Tweet 6 (Istek):
Would love feedback from anyone who [hedef kullanıcıyı tanımlar].

What am I missing? What would make this a must-have for you?
```

**Lansman Platformu 4: Ilgili Topluluklar (30 dakika)**

Hedef kitlenin takıldıgı 2-3 topluluk belirle:

- Discord sunucuları (gelistirici toplulukları, framework'e ozel sunucular)
- Slack toplulukları (bircok nis gelistirici toplulugu Slack gruplarına sahip)
- Dev.to / Hashnode (kısa bir "Bunu insa ettim" gonderisi yaz)
- Indie Hackers (https://indiehackers.com) — ozellikle bunun icin tasarlanmıs
- Ilgili Telegram veya WhatsApp grupları

**Lansırmadan Sonraki Ilk 48 Saat — Neleri Izlemeli:**

```
Izlenecek metrikler:
1. Benzersiz ziyaretciler (analitikten)
2. Landing page → odeme tıklama oranı (%2-5 olmalı)
3. Odeme → satın alma donusum oranı (%1-3 olmalı)
4. Sekme oranı (%80'in uzerinde basligın/hero'nun yanlıs oldugu anlamına gelir)
5. Trafik kaynakları (ziyaretcilerin nereden geliyor?)
6. Yorumlar ve geri bildirim (nitel — insanlar ne diyor?)

Ornek matematik:
- 48 saatte 500 ziyaretci (Reddit + HN + Twitter'dan makul)
- %3 "Satın Al"a tıklar = 15 odeme ziyareti
- %10 satın almayı tamamlar = 1-2 satıs
- $29/satıs = ilk hafta sonunda $29-58

Bu emeklilik parası degil. DOGRULAMA parası.
Internetteki bir yabancıdan $29, urunun deger taşıdıgını kanıtlar.
```

Ilk 48 saatte sıfır satıs alırsan panik yapma. Hunine bak:
- Sıfır ziyaretci? Dagıtımın problem, urunun degil.
- Ziyaretci var ama "Satın Al"a sıfır tıklama? Metin veya fiyat problem.
- "Satın Al"a tıklama var ama sıfır tamamlama? Odeme akısın bozuk veya fiyat algılanan degere gore cok yuksek.

Her birinin farklı bir cozumu var. Metriklerin onemli olmasının nedeni bu.

### Senin Sıran

1. **Zamanı blokla.** Simdi takvimini ac ve gelecek cumartesiyi 08:00'dan 20:00'a ve pazarı 08:00'dan 20:00'a blokla. "48 Saatlik Sprint" olarak etiketle. Yeniden planlayamayacagın bir ucus gibi davran.

2. **Fikrini sec.** Modul R'den bir gelir motoru sec. v0.1'in icin 3 ozellik kapsamını yaz. Birini secemiyorsan, gelistirici olmayan birine tek cumlede acıklayabilecegin birini sec.
{? if dna.primary_stack ?}
   En guclu yurutme yolun {= dna.primary_stack | fallback("ana yıgının") =} ile bir sey insa etmek — zaten derin uzmanlıga sahip oldugun yerde en hızlı lanse et.
{? endif ?}

3. **On hazırlık.** Cumartesiden once su platformlarda hesap olustur:
   - Vercel, Railway veya Fly.io (deploy)
   - Lemon Squeezy veya Stripe (odemeler)
   - Namecheap, Cloudflare veya Porkbun (domain)
   - Plausible, Fathom veya Umami (analitik)
   - Better Uptime veya UptimeRobot (izleme)

   Bunu hafta ici bir aksam yap, boylece cumartesi saf insa, hesap olusturma degil.

4. **Lansman platformlarını hazırla.** Biraz karması olan bir Reddit hesabın yoksa, bu hafta ilgili subreddit'lere katılmaya basla. Sadece kendi tanıtımını yapan hesaplar isaretlenir. Hacker News hesabın yoksa, bir tane olustur ve once birkaç tartısmaya katıl.

---

## Ders 2: "Lanse Et, Sonra Gelistir" Zihniyeti

*"3 ozellikli v0.1, asla lansırmayan v1.0'ı yener."*

### Mukemmeliyetcilik Tuzagı

Gelistiriciler belirli bir basarısızlık moduna benzersiz bicimde yatkındır: sonsuza kadar gizlice insa etmek. "Iyi kod"un nasıl gorundugunun farkındayız. v0.1'imizin iyi kod olmadıgını biliyoruz. Bu yuzden refaktor ediyoruz. Hata isleme ekliyoruz. Daha fazla test yazıyoruz. Mimariyi gelistiriyoruz. Onemli olan tek sey disında her seyi yapıyoruz: insanlara gostermek.

Iste sana binlerce saat tasarruf edecek bir gercek: **musterilerin kaynak kodunu okumuyor.** Mimarinle ilgilenmiyorlar. Test kapsamınla ilgilenmiyorlar. Tek bir seyle ilgileniyorlar: bu problemimi cozuyor mu?

Gercek bir problemi cozen spagetti kodlu bir urun para kazanacak. Hicbir problemi cozmeyen guzel mimarili bir urun hicbir sey kazanmayacak.

Bu kotu kod yazma bahanesi degil. Oncelik bildirimi. Once lanse et. Sonra refaktor et. Refaktor, gercek kullanım verilerinden yine de daha iyi bilgilendirilecek.

### "Lanse Et, Sonra Gelistir" Pratikte Nasıl Isler

Su senaryoyu dusun: bir gelistirici, yazılım muhendisligi yoneticileri icin bir Notion sablon paketi lansıyor. Lansman anında boyle gorunuyor:

- 5 sablon (50 degil)
- Bir paragraf acıklama ve 3 ekran goruntusu olan bir Gumroad sayfası
- Ozel web sitesi yok
- E-posta listesi yok
- Sosyal medya takipcisi yok
- Fiyat: $29

Reddit ve Twitter'da paylasıyorlar. Tum pazarlama stratejisi bu.

Ay 1 sonucları:
- ~170 satıs x $29 = ~$5.000
- Gumroad komisyonundan sonra (%10): ~$4.500
- Yatırılan zaman: toplam ~30 saat (sablon olusturma + acıklama yazma)
- Etkili saatlik ucret: ~$150/saat

"Mukemmel" miydi? Hayır. Sablonlarda formatlama tutarsızlıkları vardı. Bazı acıklamalar jenerikti. Musteriler umursamadı. Sablonları kendilerinin olusturma is yukunden kurtarmasını umursadılar.

Ay 3'e kadar, musteri geri bildirimine dayalı olarak gelistirici:
- Formatlama sorunlarını duzeltdi
- Daha fazla sablon ekledi (musterilerin ozellikle istedikleri)
- Fiyatı $39'a yukseltdi (mevcut musteriler ucretsiz guncellemeler aldı)
- Eslik eden video incelemesiyle bir "Pro" katmanı olusturdu

Lansırladıkları urun, 90 gun sonraki urunlerinden her acıdan daha kotuydü. Ama 90 gunluk surüm yalnızca, lansman surumunun gelistirmeyi yonlendirmek icin geri bildirim ve gelir uretmesi sayesinde var oldu.

> **NOT:** "Cirkin lanse et, hızlı gelistir" modelinin gercek dunya dogrulaması icin: Josh Comeau, CSS for JavaScript Developers kursunun ilk haftasında $550K on satıs yaptı (Kaynak: failory.com). Wes Bos, yinelemeli lansmanlar kullanarak toplamda $10M+'lık gelistirici kursu satısı uretti (Kaynak: foundershut.com). Ikisi de kusurlu v1 urunlerle baslayıp gercek musteri geri bildirimine dayalı olarak yineledi.

### Ilk 10 Musteri Sana Her Seyi Soyler

Ilk 10 odeyen musterin isindeki en onemli insanlar. Paraları yuzunden degil — 10 satıs x $29 = $290, bu sana marketin alisverisini yaptırır. Onemlidirler cunku urun gelistirme ekibinin gonulluleri.

Ilk 10 musterinle ne yapmalı:

1. **Kisisel tesekkur e-postası gonder.** Otomatik degil. Kisisel. "Merhaba, [urun]'u satın aldıgını gordum. Tesekkurler. Aktif olarak gelistiriyorum — yapmadıgı ama istedigin bir sey var mı?"

2. **Her yanıtı oku.** Bazıları yanıt vermeyecek. Bazıları "harika gorunuyor, tesekkurler" diyecek. Ama 10'dan 2-3'u ne istedikleri hakkında paragraflar yazacak. Bu paragraflar senin yol haritandır.

3. **Kalıpları ara.** 10 kisiden 3'u aynı ozelligi istiyorsa, insa et. Bu, odeyen musterilerden %30'luk bir talep sinyali. Hicbir anket sana bu kadar iyi veri vermez.

4. **Daha fazla odeme istekliligini sor.** "[Ozellik X] ile bir Pro katman planlıyorum. Bu senin icin $49 deger mi?" Dogrudan. Spesifik. Fiyatlandırma verisi verir.

```
Ilk 10 musteri icin e-posta sablonu:

Konu: [urun adı] hakkında hızlı soru

Merhaba [isim],

[Urun adı]'nı edindigini gordum — ilk musterilerden biri
oldugun icin tesekkurler.

Bunu aktif olarak insa ediyorum ve haftalık guncellemeler
cıkarıyorum. Hızlı soru: yapmadıgı ama yapmasını
istedigin TEK sey ne?

Yanlıs yanıt yok. Buyuk bir istek gibi gorunse bile
duymak istiyorum.

Tesekkurler,
[Senin adın]
```

### Olumsuz Geri Bildirimle Nasıl Basedilir

Ilk olumsuz geri bildirim kisisel hissettirecek. Kisisel degil. Veri.

**Olumsuz geri bildirimi isleme cercevesi:**

```
1. DUR. 30 dakika yanıt verme. Duygusal tepkin
   faydali degil.

2. Geri bildirimi KATEGORIZE ET:
   a) Hata raporu — duzelt. Tesekkur et.
   b) Ozellik istegi — backlog'a ekle. Tesekkur et.
   c) Fiyat sikayeti — not al. Kalıp mı kontrol et.
   d) Kalite sikayeti — arastır. Gecerli mi?
   e) Trol/mantıksız — gormezden gel. Devam et.

3. YANIT VER (sadece a, b, c, d icin):
   "Geri bildirim icin tesekkurler. [Spesifik sorunu kabul et].
   [Simdi duzeltiyorum / yol haritasına ekliyorum / arastırıyorum].
   Cozuldugunde haber verecegim."

4. HAREKETE GEC. Bir seyi duzeltmeye soz verdiysen, bir hafta icinde duzelt.
   Musterilere geri bildirimlerinin gercek degisikliklere yol actigını
   gostermekten daha hızlı sadakat insa eden bir sey yok.
```

> **Acık Konusma:** Urunun cop oldugunu soyleyen biriyle karsilasacaksın. Acıtacak. Ama urunun canli ve para kazanıyorsa, cogu gelistiricinin asla yapmadıgı seyi yapmıs oluyorsun. Yorum bolumunden elestiren kisi hicbir sey lansırmamıs. Sen lansırdın. Lansırmaya devam et.

### Haftalık Yineleme Dongusu

Lansırmadan sonra, is akısın sıkı bir dongu haline gelir:

```
Pazartesi: Gecen haftanın metriklerini ve musteri geri bildirimini incele
Salı:      Bu haftanın gelistirmesini planla (BIR sey, bes degil)
Carsamba:  Gelistirmeyi insa et
Persembe:  Gelistirmeyi test et ve deploy et
Cuma:      Degisiklik gunlugu/guncelleme gonderisi yaz
Hafta sonu: Pazarlama — bir blog gonderisi, bir sosyal medya gonderisi, bir topluluk etkilesimi

Tekrarla.
```

Anahtar kelime haftada BIR gelistirme. Ozellik yeniden tasarımı degil. Yeniden tasarım degil. Mevcut musterilerin icin urunu biraz daha iyi yapan bir sey. 12 hafta boyunca, bu gercek kullanım verileriyle yonlendirilmis 12 gelistirme. Bu dongunun 12 haftasından sonraki urunun, izole olarak tasarlayabilecegin her seyden dramatik olarak daha iyi olacak.

### Gelir, Anketlerden Daha Hızlı Dogrular

Anketler yalan soyler. Bilerek degil — insanlar sadece kendi davranıslarını tahmin etmede kotudur. "Bunun icin $29 oder misin?" kolay "evet" yanıtları alır. Ama "iste odeme sayfası, kredi kartını gir" durust yanıtlar alır.

Bu yuzden birinci gunden odemelerle lansırsın:

| Dogrulama Yontemi | Sinyal Suresi | Sinyal Kalitesi |
|---|---|---|
| Anket / oylama | 1-2 hafta | Dusuk (insanlar yalan soyler) |
| E-posta kayıtlı landing page | 1-2 hafta | Orta (ilgi, baglılık degil) |
| Fiyatlı ama odemesiz landing page | 1 hafta | Orta-Yuksek (fiyat kabulü) |
| **Gercek odemeli canli urun** | **48 saat** | **En Yuksek (gercek satın alma davranısı)** |

$0 fiyat hicbir sey ortaya cıkarmaz. $29 fiyat her seyi ortaya cıkarır.

### Senin Sıran

1. **"Cirkin lansman" taahhudunu yaz.** Bir metin dosyası ac ve yaz: "[Urun adı]'nı [tarih]'te mukemmel olmasa bile lansıracagım. v0.1 kapsamı: [3 ozellik]. Lansırmadan once Ozellik 4'u eklemeyecegim." Imzala (mecazi olarak). Cilalama durtusü geldigi anda buna bas vur.

2. **Ilk 10 musteri e-postanı tasla.** Kisisel tesekkur e-postası sablonunu simdi, musterilerin olmadan once yaz. Ilk satıs geldigi anda, bir saat icinde gondermek istiyorsun.

3. **Yineleme takipcini ayarla.** Sutunları olan basit bir elektronik tablo veya Notion sayfası olustur: Hafta | Yapılan Gelistirme | Metrik Etkisi | Musteri Geri Bildirimi. Bu, bundan sonra ne insa edecegine dair karar gunlugun olur.

---

## Ders 3: Gelistirici Urunleri Icin Fiyatlandırma Psikolojisi

*"$0 fiyat degil. Tuzak."*

### Neden Ucretsiz Pahalı

Gelistirici urunleri satmada en sezgiye aykırı gercek: **ucretsiz kullanıcılar sana odeyen musterilerden daha pahalıya mal olur.**

Ucretsiz kullanıcılar:
- Daha fazla destek istegi dosyalar (oyunda riskleri yok)
- Daha fazla ozellik talep eder (odemiyorlar diye hak iddia ederler)
- Daha az faydalı geri bildirim saglar ("havalı" eylemlenebilir degil)
- Daha yuksek oranda terk eder (degistirme maliyeti yok)
- Urununu daha az insana soyler (ucretsiz seylerin dusuk algılanan degeri var)

Odeyen musteriler:
- Basarınla ilgili (satın almalarının iyi bir karar olmasını isterler)
- Spesifik, eylemlenebilir geri bildirim saglar (urunun gelistirilmesini isterler)
- Elde tutmak daha kolay (zaten odemeye karar verdiler; atalet lehine calısır)
- Daha sık yonlendirir (odedigin bir seyi onermek satın almayı dogrular)
- Zamanına saygı gosterir (bir is yuruttugunun farkındalar)

Ucretsiz katman sunmanın tek nedeni, ucretli katman icin lead uretme mekanizması olması. Ucretsiz katmanın, insanların asla yukselme yapmadıgı kadar iyiyse, ucretsiz katmanın yok — bagıs butonu olan ucretsiz urunun var.

> **Yaygın Hata:** "Once kullanıcı kazanmak icin ucretsiz yapacagım, sonra ucret alırım." Bu neredeyse hic islemez. $0'a cektigin kullanıcılar sonsuza kadar $0 bekler. Fiyat ekleyince giderler. Birinci gunden $29 odeyecek kullanıcılar urununu hic bulamadı cunku ucretsiz arac olarak konumlandırdın. Yanlıs kitleyi cektin.

{@ insight cost_projection @}

### Gelistirici Urun Fiyatlandırma Katmanları

Yuzlerce basarılı gelistirici urununu analiz ettikten sonra, bu fiyat noktaları tutarlı bir sekilde calısıyor. Asagıdaki tum fiyatlar USD — {= regional.currency | fallback("yerel para birimin") =} ile fiyatlandırıyorsan, yerel satın alma gucu ve pazar normları icin ayarla.

**Katman 1: $9-29 — Gelistirici Aracları ve Yardımcıları**

Bu aralıktaki urunler spesifik, dar bir problemi cozer. Tek satın alma, bugun kullan.

```
Ornekler:
- Premium ozellikli VS Code uzantısı: $9-15
- Pro ozellikleri olan CLI aracı: $15-19
- Tek amacli SaaS aracı: $9-19/ay
- Kucuk bilesen kutuphanesi: $19-29
- Tarayıcı DevTools uzantısı: $9-15

Alıcı psikolojisi: Durtüsel satın alma bolgesi. Gelistirici gorur,
problemi tanır, yoneticisine sormadan satın alır.
Butce onayı gerekmez. Kredi kartı → bitti.

Anahtar icerik: Bu fiyatta, landing page'in 2 dakikadan
kısa surede donustürmeli. Alıcı uzun ozellik listesi okumaz.
Problemi goster, cozumu goster, fiyatı goster.
```

**Katman 2: $49-99 — Sablonlar, Kitler ve Kapsamlı Araclar**

Bu aralıktaki urunler onemli zaman tasarrufu saglar. Birlikte calısan birden fazla bilesen.

```
Ornekler:
- Tam UI sablon kiti: $49-79
- Auth, faturalama, dashboard'lu SaaS boilerplate: $79-99
- Kapsamlı simge/illustrasyon seti: $49-69
- Cok amacli CLI araç seti: $49
- Genis belgeli API sarmalayıcı kutuphanesi: $49-79

Alıcı psikolojisi: Dusunulmus satın alma. Gelistirici
5-10 dakika degerlendirir. Alternatiflerle karsılastırır. Tasarruf edilen zamanı hesaplar.
"Bu bana 10 saat kazandırıyorsa ve zamanımı saatte $50'dan
degerlendiriyorsam, $79 bariz karar."

Anahtar icerik: Karsılastırma noktasına ihtiyacın var. Bunu
sıfırdan insa etmenin aldıgı zaman/eforu vs. kitini satın almayı goster.
Varsa referansları dahil et.
```

**Katman 3: $149-499 — Kurslar, Kapsamlı Cozumler, Premium Sablonlar**

Bu aralıktaki urunler bir beceriyi donusturur veya eksiksiz bir sistem saglar.

```
Ornekler:
- Video kursu (10+ saat): $149-299
- Tam kaynak kodlu + video incelemeli SaaS baslangıc kiti: $199-299
- Kurumsal bilesen kutuphanesi: $299-499
- Kapsamlı gelistirici araç seti (birden fazla arac): $199
- "X'i Sıfırdan Insa Et" tam kod tabanı + dersler: $149-249

Alıcı psikolojisi: Yatırım satın alması. Alıcı harcamayı
gerekcelendirmeli (kendine veya yoneticisine). Sosyal kanıt,
detaylı onizlemeler ve net bir ROI anlatısı gerek.

Anahtar icerik: Bu katmanda, para iade garantisi sun.
Satın alma kaygısını azaltır ve donusumleri artırır. Dijital
gelistirici urunleri icin iade oranları tipik olarak %3-5.
Artan donusumler iadeleri cok aser.
```

### 3 Katmanlı Fiyatlandırma Stratejisi

Urunun destekliyorsa, uc fiyatlandırma katmanı sun. Bu rastgele degil — "merkez sahne etkisi" denilen iyi belgelenmis bilissel bir yanılgıyı kullanır. Uc secenek sunuldugunda, cogu insan ortadakini secer.

```
Katman yapısı:

TEMEL           PRO (vurgulanan)       TAKIM/KURUMSAL
$29             $59                   $149
Cekirdek ozellikler  Temel'deki her sey    Pro'daki her sey
                + premium ozellikler   + takım ozellikleri
                + oncelikli destek     + ticari lisans

Donusum dagılımı (tipik):
- Temel: %20-30
- Pro: %50-60 ← hedefiniz bu
- Takım: %10-20
```

**Katmanları nasıl tasarlarsın:**

1. **Pro** katmanıyla basla. Bu, gercekten satmak istedigin urun, degerini yansıtan fiyatta. Bunu once tasarla.

2. Pro'dan ozellik cıkararak **Temel** katmanı olustur. Temel'in problemi cozmesi ama Pro'nun *iyi* cozmesi icin yeterince cıkar. Temel hafifce sinir bozucu hissetmeli — kullanılabilir, ama acıkca kısıtlı.

3. Pro'ya ozellik ekleyerek **Takım** katmanı olustur. Cok kullanıcılı lisanslama, ticari kullanım hakları, oncelikli destek, ozel markalaşma, kaynak koduna erisim, Figma dosyaları vb.

**Gercek fiyatlandırma sayfası ornegi:**

```
DashKit

STARTER — $29                    PRO — $59                        TAKIM — $149
                                 ★ En Populer                     Ajanslar icin en iyisi

✓ 12 cekirdek bilesen            ✓ Starter'daki her sey            ✓ Pro'daki her sey
✓ React + TypeScript             ✓ 8 tam sayfa sablonu             ✓ 5'e kadar takım uyesi
✓ Karanlık mod                   ✓ Figma tasarım dosyaları         ✓ Ticari lisans
✓ npm install                    ✓ Gelismis veri tablosu             (sınırsız musteri projesi)
✓ 6 ay guncelleme                ✓ Grafik kutuphanesi              ✓ Oncelikli destek
                                   entegrasyonu                    ✓ Omur boyu guncelleme
                                 ✓ 12 ay guncelleme               ✓ Ozel markalaşma secenekleri
                                 ✓ Oncelikli ozellik istekleri

[Starter Al]                     [Pro Al]                          [Takım Al]
```

### Fiyat Cıpalama

Cıpalama, insanların gordugu ilk sayının sonraki sayıları algılamasını etkileyen bilissel yanılgıdır. Etik olarak kullan:

1. **Pahalı secenegi once goster** (Batı yerlesimlerinde sagda). $149 gormek $59'u makul gosterir.

2. **"Kazanılan saatler" hesaplamalarını goster.**
   ```
   "Bu bilesenleri sıfırdan insa etmek ~40 saat surer.
   Saatte $50 ile bu zamanınızın $2.000'ı.
   DashKit Pro: $59."
   ```

3. **Abonelikler icin "gun basına" yeniden cereveleme kullan.**
   ```
   "$19/ay" → "Gunde $0,63'ten az"
   "$99/yıl" → "Ayda $8,25" veya "Gunde $0,27"
   ```

4. **Yıllık faturalama indirimi.** Yıllık planlarda 2 ay ucretsiz sun. Bu standart ve beklenen. Yıllık faturalama, iptali ayda bir tekrarlanan bir karar yerine tek bir yenileme noktasında bilinçli bir karar gerektirdigi icin kayıp oranını %30-40 azaltır.

```
Aylık: $19/ay
Yıllık: $190/yıl ($38 tasarruf — 2 ay ucretsiz)

Gosterim:
Aylık: $19/ay
Yıllık: $15,83/ay ($190 yıllık fatura)
```

### A/B Fiyat Testi

Fiyat testi degerli ama karmasık. Iste durust olmadan nasıl yapılır:

**Kabul edilebilir yaklasımlar:**
- Farklı lansman kanallarında farklı fiyatlar test et (Reddit $29 alır, Product Hunt $39 alır, hangisinin daha iyi donusturdugunü gor)
- 2 haftadan sonra fiyatını degistir ve donusum oranlarını karsılastır
- Lansman indirimi sun ("Bu hafta $29, sonra $39") ve aciliyetin davranısı degistirip degistirmedigine bak
- Farklı zaman dilimlerinde farklı katman yapılarını (2 katman vs 3 katman) test et

**Kabul edilemez:**
- Aynı sayfada aynı anda farklı ziyaretcilere farklı fiyatlar gostermek (fiyat ayrımcılıgı, guveni asındırır)
- Konum veya tarayıcı algılamasına dayalı olarak daha fazla ucret almak (insanlar konusur ve yakalanırsın)

### Ne Zaman Fiyat Yukseltilir

Bunlardan herhangi biri dogru oldugunda fiyatlarını yukselt:

1. **Donusum oranı %5'in uzerinde.** Cok ucuzsun. Gelistirici urunu landing page'i icin saglıklı donusum oranı %1-3. %5'in uzerinde, fiyatı goren neredeyse herkesin bunun iyi bir anlasma olduguna katıldıgı anlamına gelir — bu da masada para bıraktıgın anlamına gelir.

2. **Fiyattan kimse sikayet etmedi.** 100 kisiden sıfır kisi pahalı diyorsa, ucuzdur. Saglıklı bir urunde ziyaretcilerin yaklasık %20'si fiyatı yuksek bulur. Bu, %80'in adil veya iyi bir anlasma bulması demek.

3. **Lansırmadan bu yana onemli ozellikler ekledin.** 3 ozellikle $29'a lansırdın. Simdi 8 ozelligi ve daha iyi belgelerin var. Urun daha degerli. Daha fazla al.

4. **Referansların ve sosyal kanıtın var.** Algılanan deger sosyal kanıtla artar. 5+ olumlu incelemeye sahip oldugunda, urunun alıcının zihninde daha degerli.

**Fiyat nasıl yukseltilir:**
- Fiyat artısını 1-2 hafta once duyur ("Fiyat [tarih]'te $29'dan $39'a cıkıyor")
- Mevcut musterileri eski fiyatta tut
- Bu ahlaksız degil — standart uygulama ve aynı zamanda kararsızlar icin aciliyet yaratır

> **Acık Konusma:** Cogu gelistirici %50-200 ucuza fiyatlandırır. {= regional.currency_symbol | fallback("$") =}29'luk urunun muhtemelen {= regional.currency_symbol | fallback("$") =}49 deger. {= regional.currency_symbol | fallback("$") =}49'luk urunun muhtemelen {= regional.currency_symbol | fallback("$") =}79 deger. Bunu biliyorum cunku gelistiriciler kendi odeme istekliligine cıpalar (dusuk — araclara para harcamakta cimriyiz), musterinin odeme istekligine degil (daha yuksek — zamanlarına mal olan bir problemin cozumunu satın alıyorlar). Fiyatlarını dustugunun cok once yukselt.

### Senin Sıran

1. **Urununu fiyatlandır.** Yukarıdaki katman analizine dayalı olarak, v0.1 lansmanın icin bir fiyat noktası sec. Yaz. "Cok yuksek" gorundugunun icin rahatsız hissediyorsan, muhtemelen dogru aralıktasın. Rahat hissediyorsan, %50 ekle.

2. **Fiyatlandırma sayfanı tasarla.** 3 katman sablonunu kullanarak, fiyatlandırma sayfası metnini tasarla. Her katmana hangi ozelliklerin gidecegini belirle. "Vurgulanan" katmanını belirle (cogu kisinin satın almasını istedigin).

3. **Matematigini hesapla.** Doldur:
   - Satıs basına fiyat: {= regional.currency_symbol | fallback("$") =}___
   - Hedef aylık gelir: {= regional.currency_symbol | fallback("$") =}___
   - Ayda gereken satıs sayısı: ___
   - Gereken tahmini landing page ziyaretcisi (%2 donusumde): ___
   - Bu ziyaretci sayısı dagıtım planınla ulaşılabilir mi? (Evet/Hayır)

---

## Ders 4: Minimum Uygulanabilir Yasal Kurulum

*"Simdi 30 dakikalık yasal kurulum, daha sonra 30 saatlik panigi onler."*

### Yasal Kurulum Hakkında Durust Gercek

Cogu gelistirici yasalı ya tamamen gormezden gelir (riskli) ya da onun tarafından felc olur (israf). Dogru yaklasım minimum uygulanabilir yasal kurulumdur: $5 kazanmadan once avukata $5.000 harcamadan, meşru bir sekilde calısmak icin yeterli koruma.

Iste ilk satısından once gercekten neye ihtiyacın var, yuzuncu satısından once neye ihtiyacın var ve cok daha sonrasına kadar neye ihtiyacın yok.

### Ilk Satısından Once (Bu Hafta Sonu Yap)

**1. Is Sozlesmeni Kontrol Et (30 dakika)**

Tam zamanlı bir isin varsa, herhangi bir sey insa etmeden once is sozlesmendeki fikri mulkiyet maddesini oku. Ozellikle su seyleri ara:

- **Bulus devri maddeleri:** Bazı sozlesmeler, calısırken olusturdugün her seyin — kendi zamanında dahil — isverene ait oldugunu soyler.
- **Rekabet yasagı maddeleri:** Bazıları yan proje olarak bile aynı sektorde calısmayı kısıtlar.
- **Yan is politikaları:** Bazıları dıs ticari faaliyetler icin yazılı onay gerektirir.

```
Ne arıyorsun:

GUVENLI: "Sirket zamanında veya sirket kaynaklarını kullanarak yapılan
buluslar sirkete aittir." → Kisisel makinende hafta sonu projen
senin.

BELIRSIZ: "Sirketin mevcut veya ongorulen isine ilgili tum
buluslar." → Yan projen isverenle aynı alanda ise,
hukuki danısmanlık al.

KISITLAYICI: "Istihdam suresi boyunca tasarlanan tum buluslar
sirkete aittir." → Bu agresif ama bazı sirketlerde yaygın.
Devam etmeden once hukuki danısmanlık al.
```

Kaliforniya, Delaware, Illinois, Minnesota, Washington ve diger eyaletler, isverenlerin kisisel buluslarını ne kadar genis talep edebilecegini sınırlayan yasalara sahiptir. Ama sozlesmendeki spesifik dil onemli.

> **Yaygın Hata:** "Gizli tutarım." Urunun yeterince basarılı olursa, biri fark edecek. Is sozlesmeni ihlal ediyorsa, urunu VE isini kaybedebilirsin. Simdi 30 dakika sozlesmeni okumak bunu onler.

**2. Gizlilik Politikası (15 dakika)**

Urunun herhangi bir veri topluyorsa — satın alma icin bir e-posta adresi bile olsa — gizlilik politikasına ihtiyacın var. Bu AB'de (GDPR), Kaliforniya'da (CCPA) ve giderek her yerde yasal gerekliliktir.

Sıfırdan yazma. Bir oluşturucu kullan:

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Ucretsiz katman, soruları yanıtla, politika al
- **Avodocs** (https://www.avodocs.com) — Ucretsiz, acık kaynak yasal sablonlar
- **Iubenda** (https://www.iubenda.com) — Ucretsiz katman, teknik yıgınına gore otomatik uretir

Gizlilik politikan su konuları kapsamalı:

```markdown
# [Urun Adı] Gizlilik Politikası
Son guncelleme: [Tarih]

## Ne Topluyoruz
- E-posta adresi (satın alma onayı ve urun guncellemeleri icin)
- Odeme bilgileri ([Lemon Squeezy/Stripe] tarafından islenir,
  kart bilgilerinizi asla gormeyiz veya saklamayız)
- Temel kullanım analitigi (sayfa goruntumeleri, ozellik kullanımı —
  [Plausible/Fathom/Umami] aracılıgıyla, gizliligi koruyan, cerez yok)

## Ne TOPLAMIYORUZ
- Sizi web uzerinde takip etmiyoruz
- Verilerinizi kimseye satmıyoruz
- Reklam cerezleri kullanmıyoruz

## Verilerinizi Nasıl Kullanıyoruz
- Satın aldıgınız urunu teslim etmek icin
- Urun guncellemeleri ve onemli bildirimleri gondermek icin
- Toplam kullanım kalıplarına dayalı urunu gelistirmek icin

## Veri Depolama
- Verileriniz [barındırma saglayıcı] sunucularında [bolge]'de depolanır
- Odeme verileri tamamen [Lemon Squeezy/Stripe] tarafından islenir

## Haklarınız
- Istediginiz zaman verilerinizin bir kopyasını talep edebilirsiniz
- Istediginiz zaman verilerinizin silinmesini talep edebilirsiniz
- Iletisim: [e-postanız]

## Degisiklikler
- Onemli degisiklikleri e-posta ile bildirecegiz
```

Bunu `domain.com/privacy`'e koy. Odeme sayfası altbilginden bagla.

**3. Kullanım Sartları (15 dakika)**

Kullanım sartların seni mantıksız taleplerden korur. Dijital bir urun icin basittirler.

```markdown
# [Urun Adı] Kullanım Sartları
Son guncelleme: [Tarih]

## Lisans
[Urun Adı]'nı satın aldıgınızda, [kisisel/ticari] amaclarla
kullanım lisansı alırsınız.

- **Tek lisans:** Kendi projelerinizde kullanın (sınırsız)
- **Takım lisansı:** [N] takım uyesine kadar kullanım
- Yeniden dagıtamaz, yeniden satamaz veya erisim bilgilerini paylasamazsınız

## Iadeler
- Dijital urunler: [30 gun / 14 gun] para iade garantisi
- Memnun degilseniz, tam iade icin [e-postanız]'a yazın
- Iade penceresi icinde soru sorulmaz

## Sorumluluk
- [Urun Adı] garanti olmaksızın "oldugu gibi" saglamnaktadır
- Urunun kullanımından kaynaklanan zararlardan sorumlu degiliz
- Azami sorumluluk odediginiz tutarla sınırlıdır

## Destek
- Destek [e-postanız] uzerinden e-posta ile saglanır
- [48 saat / 2 is gunu] icinde yanıt vermeyi hedefliyoruz

## Degisiklikler
- Bu sartları bildirimle guncelleyebiliriz
- Surekli kullanım, guncellenmis sartların kabulunu olusturur
```

Bunu `domain.com/terms`'e koy. Odeme sayfası altbilginden bagla.

### Yuzuncu Satısından Once (Ilk Birkaç Ay)

**4. Isletme Tuzel Kisilik (1-3 saat + isleme suresi)**

Bireysel isletmeci olarak calısmak (isletme kurmadan satis yaptıgında varsayılan) ilk satıslar icin ise yarar. Ama gelir buyudukce, sorumluluk koruması ve vergi avantajları istersin.

{? if regional.country ?}
> **{= regional.country | fallback("bolgen") =} icin:** Onerilen tuzel kisilik turu **{= regional.business_entity_type | fallback("LLC veya esdeger") =}**, tipik kayıt maliyetleri {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Spesifik yonlendirme icin asagıda ulke bolumunu bul.
{? endif ?}

**ABD — LLC:**

LLC (Limited Liability Company), solo gelistirici isletmeleri icin standart sectir.

```
Maliyet: Eyalete baglı olarak $50-500 (dosya ucreti)
Sure: Isleme icin 1-4 hafta
Nereye dosyalanır: Yasadıgın eyalet, Delaware veya Wyoming
kullanmak icin spesifik bir neden yoksa

DIY dosyalama (en ucuz):
1. Eyaletinin Secretary of State web sitesine git
2. "Articles of Organization" dosyala (form genellikle 1-2 sayfa)
3. Dosyalama ucretini ode (eyalete baglı $50-250)
4. IRS.gov'dan EIN (vergi kimlik no) al — ucretsiz, anında cevrimici

Solo gelistiriciler icin eyalet karsılastırması:
- Wyoming: $100 dosyalama, $60/yıl yıllık rapor. Eyalet gelir vergisi yok.
             Gizlilik icin iyi (halka acık uye bilgisi gerektirmez).
- Delaware: $90 dosyalama, $300/yıl yıllık vergi. Populer ama
            solo gelistiriciler icin mutlaka daha iyi degil.
- New Mexico: $50 dosyalama, yıllık rapor yok. Bakımı en ucuz.
- California: $70 dosyalama, $800/yıl minimum franchise vergisi.
              Pahalı. $0 kazansan bile bunu odersin.
```

**Stripe Atlas (senin icin yapılmasını istiyorsan):**

Stripe Atlas (https://atlas.stripe.com) $500'a mal olur ve Delaware LLC, ABD banka hesabı (Mercury aracılıgıyla), Stripe hesabı olusturur ve vergi ve yasal rehberler saglar. ABD'den degilsen veya baskasının evrak islerini yapmasını istiyorsan, $500 deger.

**Birlesik Krallık — Ltd Company:**

```
Maliyet: Companies House'da GBP 12 (https://www.gov.uk/set-up-limited-company)
Sure: Genellikle 24-48 saat
Devam eden: Yıllık onay beyanı (GBP 13), yıllık hesap dosyalama

Solo gelistiriciler icin: Ltd company, karlar ~GBP 50.000/yılı
astıgında sorumluluk koruması ve vergi verimliligi saglar.
Bunun altında sole trader daha basit.
```

**Avrupa Birligi:**

Her ulkenin kendi yapısı var. Yaygın secenekler:
- **Almanya:** GmbH (kurması pahalı) veya serbest calısan kaydı (ucuz)
- **Hollanda:** BV veya eenmanszaak (bireysel isletme)
- **Fransa:** auto-entrepreneur (mikro isletme) — solo gelistiriciler arasında cok yaygın, basit sabit vergi
- **Estonya:** e-Residency + Estonya OU (dijital gocebeler arasında populer, ~EUR 190'a tam AB sirketi)

**Avustralya:**

```
Sole trader: ABN basvurusu ile ucretsiz kayıt (https://www.abr.gov.au)
Sirket (Pty Ltd): ASIC'te AUD 538 kayıt
Solo gelistiriciler icin: Sole trader olarak basla. Gelir muhasebe
yukunu haklı çıkardıgında sirket kaydet (~AUD 100K+/yıl).
```

**5. Vergi Yukumlulukleri**

Odeme platformun olarak Lemon Squeezy kullanıyorsan, Merchant of Record olarak satıs vergisi ve KDV'yi yonetirler. Bu devasa bir basitlestirme.

Stripe'ı dogrudan kullanıyorsan, su seylerden sorumlusun:
- **ABD satıs vergisi:** Eyalete gore degisir. Otomatiklestirmek icin Stripe Tax ($0,50/islem) veya TaxJar kullan.
- **AB KDV:** Ulkeye gore %20-27. Nerede olursan ol AB musterilerine dijital satıslar icin gerekli. Lemon Squeezy bunu yonetir; Stripe Tax otomatiklestirebilir.
- **BK KDV:** %20. BK satısların yılda GBP 85.000'i asarsa gerekli.
- **Dijital Hizmet Vergileri:** Cesitli ulkeler bunları dayatıyor. Hacmin kendi basına yonetmeyi haklı cıkarana kadar Lemon Squeezy kullanmak icin bir neden daha.

{? if regional.country ?}
> **{= regional.country | fallback("bolgen") =} icin vergi notu:** {= regional.tax_note | fallback("Yukumlulukleriniz hakkında ayrıntılar icin yerel bir vergi uzmanına danısın.") =}
{? endif ?}

> **Acık Konusma:** Solo gelistirici icin Lemon Squeezy'nin Stripe'a karsı en buyuk avantajı odeme sayfası veya ozellikler degil. Kuresel vergi uyumlulugunun yonetilmesi. Uluslararası satıs vergisi bir kabus. Lemon Squeezy islem basına %5 + $0,50 alır ve kabusu ortadan kaldırır. Aylık {= regional.currency_symbol | fallback("$") =}5.000+ yapana kadar, %5 buna deger. Ondan sonra, Stripe + TaxJar ile vergileri kendin yonetmenin para ve akıl saglıgı tasarruf edip etmedigini degerlendir.

**6. Fikri Mulkiyet Temelleri**

Bilmen gerekenler:

- **Kodun yazdıgın anda otomatik olarak telif hakkıyla korunur.** Kayıt gerekmez. Ama kayıt (ABD: copyright.gov'da $65) ihtilaflarda daha guclu yasal konum verir.
- **Urun adın marka tescili yapılabilir.** Lansman icin gerekli degil, ama urun tutarsa dusun. ABD marka dosyalama: sınıf basına $250-350.
- **Bagımlılıklarındaki acık kaynak lisansları onemli.** MIT lisanslı kod kullanıyorsan, sorun yok. Ticari urunde GPL lisanslı kod kullanıyorsan, urununu acık kaynaga cevirmek zorunda kalabilirsin. Satmadan once bagımlılık lisanslarını kontrol et.

```bash
# Projenin bagımlılık lisanslarını kontrol et (Node.js)
npx license-checker --summary

# Sorunlu lisansları ozellikle kontrol et
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# Rust projeleri icin
cargo install cargo-license
cargo license
```

**7. Sigorta**

$29'luk bilesen kutuphanesi icin sigortaya ihtiyacın yok. Sigortaya ihtiyacın var eger:
- Hataların musteri kayıplarına yol acabileceği hizmetler saglıyorsan (danısmanlık, veri isleme)
- Urunun hassas verilerle ilgileniyorsa (saglık, finans)
- Kurumsal musterilerle sozlesme imzalıyorsan (isteyecekler)

Gerektiginde, mesleki sorumluluk sigortası (hatalar ve eksiklikler / E&O) solo gelistirici isletmesi icin yılda $500-1.500'a mal olur.

### Senin Sıran

1. **Is sozlesmeni oku.** Istihdam ediliyorsan, fikri mulkiyet ve rekabet yasagı maddesini bul. Kategorize et: Guvenli / Belirsiz / Kısıtlayıcı. Belirsiz veya Kısıtlayıcıysa, lansırmadan once bir is hukuku avukatına danıs (cogu 30 dakikalık ucretsiz gorusmeler sunuyor).

2. **Yasal belgelerini olustur.** Termly veya Avodocs'a git ve urunun icin gizlilik politikası ve kullanım sartları olustur. HTML veya Markdown olarak kaydet. Urun domaininde `/privacy` ve `/terms`'e deploy et.

3. **Tuzel kisilik kararını ver.** Yukarıdaki rehberlik ve {= regional.country | fallback("ulke") =}'deki ikametine dayanarak karar ver: bireysel isletmeci olarak lansır (en hızlı) veya once {= regional.business_entity_type | fallback("LLC/Ltd/esdeger") =} kur (daha fazla koruma). Kararını ve zaman cizelgeni yaz.

4. **Bagımlılıklarını kontrol et.** Projende lisans denetleyicisini calıstır. Ticari urun satmadan once herhangi bir GPL/AGPL bagımlılıgını coz.

---

## Ders 5: 2026'da Calısan Dagıtım Kanalları

*"Insa etmek isin %20'si. Insanların onune koymak diger %80'i."*

### Dagıtım Gercekligi

Cogu gelistirici urunu kotu oldugu icin degil, kimsenin var oldugundan haberdar olmadıgı icin basarısız olur. Dagıtım — urununu potansiyel musterilerin onune koymak — cogu gelistiricinin en zayıf oldugu beceri. Ve en cok onem taşıyan beceri.

Iste efor, zaman cizelgesi ve beklenen getiriye gore sıralanmıs yedi dagıtım kanalı. Yedisinin hepsine ihtiyacın yok. Guclu yonlerinle ve kitlenle eslesen 2-3 tane sec.

### Kanal 1: Hacker News

**Efor:** Yuksek | **Zaman Cizelgesi:** Anında (0-48 saat) | **Doga:** Ya hep ya hic

Hacker News (https://news.ycombinator.com) gelistirici urunleri icin en yuksek kaldıraclı tek olaylı dagıtım kanalı. On sayfadaki bir Show HN gonderisi 24 saatte 5.000-30.000 ziyaretci gonderebilir. Ama ongorelemez — cogu gonderi sıfır ilgi gorur.

**HN'de ne ise yarar:**
- Ilginc uygulama detayları olan teknik urunler
- Gizlilik odaklı araclar (HN kitlesi gizliligi derinden onemsler)
- Ucretli katmanı olan acık kaynak araclar
- Bilinen problemlere yenilikci cozumler
- Canli demo'ları olan urunler

**HN'de ne ise yaramaz:**
- Pazarlama agırlıklı lansmanlar ("Devrimci AI destekli...")
- Orijinal deger olmadan diger urunlerin sarmalayıcıları olan urunler
- Reklam gibi gorunen her sey

**Show HN Oyun Planı:**

```
GONDERMEDEN ONCE:
1. Kategorindeki son basarılı Show HN gonderilerini incele
   https://hn.algolia.com — "Show HN" ile filtrele, puanlara gore sırala
2. Gonderi baslıgını hazırla: "Show HN: [Isim] – [ne yaptıgı, <70 karakter]"
   Iyi: "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Kotu: "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Canli demo hazır olsun (HN okuyucuları denemek ister, okumak degil)
4. Olası sorulara yanıtlar hazırla (teknik kararlar, fiyat gerekcelesi)

GONDERME:
5. ABD Dogu Saati 7-9 arası, salıdan persembeye gonder
   (en yuksek trafik, en yuksek ilgi sansı)
6. Gonderi govdesi 4-6 paragraf olmalı:
   - Ne oldugu (1 paragraf)
   - Neden insa ettigin (1 paragraf)
   - Teknik detaylar (1-2 paragraf)
   - Ne aradıgın (geri bildirim, spesifik sorular)

GONDERDIKTEN SONRA:
7. Gonderdikten sonra 4 saat cevrimici kal. HER yoruma yanıt ver.
8. Alçakgonullu ve teknik ol. HN sınırlamalar hakkında durustlugu odullendirir.
9. Biri bir hata bulursa, canli duzelt ve "Fixed, thanks." yanıtla.
10. Arkadaslardan oy isteme. HN'in oy halkası tespiti var.
```

**Beklenen sonuclar (gercekci):**
- Show HN gonderilerinin %70'i: <10 puan, <500 ziyaretci
- Show HN gonderilerinin %20'si: 10-50 puan, 500-3.000 ziyaretci
- Show HN gonderilerinin %10'u: 50+ puan, 3.000-30.000 ziyaretci

Eforla yuklenmis olasılıkları olan bir piyango. Harika bir gonderiyle harika bir urunun anlamlı ilgi icin belki %30 sansı var. Garanti degil. Ama yukselis potansiyeli buyuk.

### Kanal 2: Reddit

**Efor:** Orta | **Zaman Cizelgesi:** 1-7 gun | **Doga:** Surdurulebilir, tekrarlanabilir

Reddit, gelistirici urunleri icin en tutarlı dagıtım kanalı. HN'nin aksine (bir sans), Reddit'te urunun ilgili oldugu yuzlerce nis subreddit var.

**Subreddit secimi:**

```
Genel gelistirici subreddit'leri:
- r/SideProject (140K+ uye) — bunun icin insa edilmis
- r/webdev (2.4M uye) — buyuk, rekabetci
- r/programming (6.3M uye) — cok rekabetci, haber odaklı
- r/selfhosted (400K+ uye) — urunun self-host edilebilirse

Framework/dile ozel:
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — frontend aracları icin
- r/rust, r/golang, r/python — dile ozel araclar icin
- r/node — Node.js aracları ve paketleri icin

Alana ozel:
- r/devops — altyapı/deploy aracları icin
- r/machinelearning — AI/ML aracları icin
- r/datascience — veri aracları icin
- r/sysadmin — yonetim/izleme aracları icin

Uzun kuyruk:
- Spesifik nisinle ilgili subreddit'leri ara
- Daha kucuk subreddit'ler (10K-50K uye) buyuk olanlardan
  genellikle daha iyi donusum oranlarına sahip
```

**Reddit etkilesim kuralları:**

1. Urununu gondermeden once **gercek bir Reddit gecmisine** sahip ol. Sadece kendi tanıtımını yapan hesaplar isaretlenir ve golge ban yer.
2. Kendi tanıtım hakkında **her subreddit'in kurallarına uy.** Cogu katkıda bulunan bir uye oldugun surece izin verir.
3. **Gercekten etkilesimde ol.** Soruları yanıtla, deger sun, diger gonderilerin yorumlarında yardımcı ol. Sonra urununu paylas.
4. Farklı subreddit'ler icin **farklı zamanlarda gonder.** En yoğun aktivite zamanları icin https://later.com/reddit veya benzer aracları kontrol et.

**Beklenen sonuclar (gercekci):**
- r/SideProject gonderisi: 20-100 oy, 200-2.000 ziyaretci
- Nis subreddit (50K uye): 10-50 oy, 100-1.000 ziyaretci
- r/webdev on sayfası: 100-500 oy, 2.000-10.000 ziyaretci

### Kanal 3: Twitter/X

**Efor:** Orta | **Zaman Cizelgesi:** Momentum icin 2-4 hafta | **Doga:** Zamanla birikir

Twitter yavaş insa kanalı. Ilk lansman tweetin arkadaslarından 5 begeni alacak. Ama insa surecini tutarlı paylasırsan, kitlen birikir.

**Build-in-Public Stratejisi:**

```
Hafta 1: Insa surecini paylasmaya basla (lansırmadan once)
- "[Urun tipi] uzerinde calısıyorum. Iste cozdugum problem: [ekran goruntusu]"
- "[Urun] insa etmenin 3. gunu. [Ozellik] calıstırdım: [GIF/ekran goruntusu]"

Hafta 2: Insadan teknik icgoruler paylas
- "TIL [urun tipi] insa ederken [teknik ders] gerekiyor"
- "Mimari karar: [neden] icin [Y] yerine [X]'i sectim"

Hafta 3: Lansman
- Lansman dizisi (Ders 1'deki format)
- Spesifik metrikler paylas: "1. gun: X ziyaretci, Y kayıt"

Hafta 4+: Devam
- Musteri geri bildirimini paylas (izinle)
- Gelir kilometre taslarını paylas (insanlar gercek sayıları seviyor)
- Zorlukları ve nasıl cozdugunun paylas
```

**Kiminle etkilesimde ol:**
- Nisindeki gelistiricileri takip et ve etkilesimde ol
- Buyuk hesapların tweetlerine dusunceli yorumlarla yanıt ver (kendi tanıtımı degil)
- Konun hakkındaki Twitter Spaces'lere katıl
- Ilgili tartısmaları kendi perspektifinle alıntıla

**Beklenen sonuclar (gercekci):**
- 0-500 takipci: Lansman tweetleri 5-20 begeni alır, <100 ziyaretci
- 500-2.000 takipci: Lansman tweetleri 20-100 begeni alır, 100-500 ziyaretci
- 2.000-10.000 takipci: Lansman tweetleri 100-500 begeni alır, 500-5.000 ziyaretci

Twitter 6 aylık bir yatırım, lansman gunu stratejisi degil. Urunun hazır olmadan once bile simdi basla.

### Kanal 4: Product Hunt

**Efor:** Yuksek | **Zaman Cizelgesi:** 1 gunluk yogun aktivite | **Doga:** Tek seferlik destek

Product Hunt (https://producthunt.com) ozel bir lansman platformu. Gunluk ilk 5'e girmek 3.000-15.000 ziyaretci gonderebilir. Ama hazırlık gerektirir.

**Product Hunt Lansman Kontrol Listesi:**

```
2 HAFTA ONCE:
- [ ] Product Hunt maker profili olustur
- [ ] PH listesini kur: slogan, acıklama, gorseller, video
- [ ] 4-5 yuksek kaliteli ekran goruntusu/GIF hazırla
- [ ] Motivasyonunu acıklayan "ilk yorum" yaz
- [ ] Lansman gunu destekleyecek 10-20 kisiyi ayarla (sahte oylar degil —
      urunu deneyecek ve gercek yorumlar bırakacak gercek insanlar)
- [ ] Bir "hunter" bul (urununu gondermek icin buyuk PH takipcisi olan biri)
      veya kendin gonder

LANSMAN GUNU (00:01 Pasifik Saati):
- [ ] Gece yarısı PT'den itibaren cevrimici ol. PH gece yarısı sıfırlanır.
- [ ] "Ilk yorum"unu hemen gonder
- [ ] PH linkini Twitter, LinkedIn, e-posta, Discord'da paylas
- [ ] PH listendeki HER yoruma yanıt ver
- [ ] Gun boyunca guncellemeler gonder ("[X] icin duzeltme yayınladım!")
- [ ] Gece yarısı PT'ye kadar tum gunu izle

SONRA:
- [ ] Destekleyen herkese tesekkur et
- [ ] "Ogrenilen dersler" gonderisi yaz (Twitter/blog icerigi icin iyi)
- [ ] Landing page'e PH rozeti yerlesitir (sosyal kanıt)
```

> **Yaygın Hata:** Product Hunt'ta urun hazır olmadan lansırmak. PH sana bir sans verir. Bir urunu lansırdıgında, yeniden lansıramazsın. Urunun cilalı, landing page'in dönüştürücü ve odeme akısın calısana kadar bekle. PH "buyuk lansmanın" olmalı — yumusak lansmanın degil.

**Beklenen sonuclar (gercekci):**
- Gunluk ilk 5: 3.000-15.000 ziyaretci, 50-200 oy
- Gunluk ilk 10: 1.000-5.000 ziyaretci, 20-50 oy
- Ilk 10'un altı: <1.000 ziyaretci. Minimal kalıcı etki.

### Kanal 5: Dev.to / Hashnode / Teknik Blog Gonderileri

**Efor:** Dusuk-orta | **Zaman Cizelgesi:** 1-3 ayda SEO sonucları | **Doga:** Uzun kuyruk, sonsuza kadar birikir

Urunune ilgili problemleri cozen teknik blog gonderileri yaz ve urununu cozum olarak belirt.

**Icerik stratejisi:**

```
Her urun icin 3-5 blog gonderisi yaz:

1. "2026'da [urunun cozdugu problemi nasıl cozulur]"
   - Manuel yaklasımı ogret, sonra urununu kısa yol olarak belirt

2. "[Urunu] 48 saatte insa ettim — iste ogrendiklerim"
   - Build-in-public icerigi. Teknik detaylar + durust yansıma.

3. "[Rakip] vs [Urunun]: Durust Karsılastırma"
   - Gercekten adil ol. Rakibin nerede kazandıgını belirt.
   - Bu, karsılastırmalı alisveris arama trafigini yakalar.

4. "[Urunune ilgili teknik konsept] acıklaması"
   - Saf egitim. Urununu sonunda bir kez belirt.

5. "[Urun alanın] icin 2026'da kullandıgım araclar"
   - Liste formatı. Urununu digerlerinin yanına dahil et.
```

**Nerede yayımlanır:**
- **Dev.to** (https://dev.to) — Buyuk gelistirici kitlesi, iyi SEO, ucretsiz
- **Hashnode** (https://hashnode.com) — Iyi SEO, ozel domain secenegi, ucretsiz
- **Kendi blogun** — Uzun vadeli SEO icin en iyi, icerigin senin
- **Her yerde capraz yayımla.** Bir kez yaz, uc platformda da yayımla. SEO cezalarından kacınmak icin kanonik URL'ler kullan.

**Gonderi basına beklenen sonuclar:**
- 1. gun: 100-1.000 goruntulenme (platform dagıtımı)
- Ay 1-3: 50-200 goruntulenme/ay (arama trafigi buyuyor)
- Ay 6+: 100-500 goruntulenme/ay (arama trafigi birikiyor)

Tek bir iyi yazılmıs blog gonderisi yıllarca ayda 200+ ziyaretci getirebilir. Bes gonderi ayda 1.000+ getirir. Bu birikir.

### Kanal 6: Dogrudan Ulasım

**Efor:** Yuksek | **Zaman Cizelgesi:** Anında | **Doga:** En yuksek donusum oranı

Soguk e-posta ve DM'ler herhangi bir kanalın en yuksek donusum oranına sahip — ama ayrıca lead basına en yuksek efor. Daha yuksek fiyatlı urunler ($99+) veya B2B satısları icin kullan.

**Potansiyel musterilere ulasma icin e-posta sablonu:**

```
Konu: [Spesifik acı noktaları] hakkında hızlı soru

Merhaba [isim],

[Belirttikleri spesifik problem] hakkındaki [tweet/gonderi/yorum]unu gordum.

Ozellikle bunun icin [urun adı]'nı insa ettim — [ne yaptıgının
tek cumleli acıklaması].

Denemeye acık mısın? Geri bildirim icin ucretsiz erisim
vermekten memnuniyet duyarım.

[Senin adın]
[Urune baglantı]
```

**Soguk ulasım kuralları:**
- Sadece urunun cozdugu problemi alenen ifade etmis insanlara ulas
- Spesifik gonderi/yorumlarına atıfta bulun (toplu e-posta gondermiyor oldugunu kanıtlar)
- Anında para istemek yerine deger sun (ucretsiz erisim, indirim)
- 5 cumlenin altında tut
- Gercek bir e-posta adresinden gonder (sen@senindomain.com, gmail degil)
- 3-4 gun sonra bir kez takip et. Yanıt yoksa, dur.

**Beklenen sonuclar:**
- Yanıt oranı: %10-20 (ilgili alıcılara soguk e-posta)
- Yanıttan denemeye donusum: %30-50
- Denemeden ucretliye donusum: %20-40
- Etkili donusum: E-posta gonderilen insanların %1-4'u musteri olur

$99'luk bir urun icin, 100 kisiye e-posta gondermek = 1-4 satıs = $99-396. Olceklenebilir degil, ama ilk musterileri ve geri bildirimi almak icin mukemmel.

### Kanal 7: SEO

**Efor:** Dusuk surekli | **Zaman Cizelgesi:** Sonuclar icin 3-6 ay | **Doga:** Sonsuza kadar birikir

SEO en iyi uzun vadeli dagıtım kanalı. Baslamak yavaş ama calısmaya basladıgında suresiz ucretsiz trafik gonderir.

**Gelistirici odaklı SEO stratejisi:**

```
1. Uzun kuyruk anahtar kelimeleri hedefle (sıralamak daha kolay):
   Yerine: "dashboard components"
   Hedef: "tailwind dashboard components react typescript"

2. Anahtar kelime basına bir sayfa olustur:
   Her blog gonderisi veya belge sayfası tek bir spesifik arama sorgusunu hedefler

3. Teknik uygulama:
   - Hızlı yuklenme icin statik site uretimi kullan (Astro, Next.js SSG)
   - Her sayfaya meta acıklamaları ekle
   - Anlamsal HTML kullan (h1, h2, h3 hiyerarsisi)
   - Her gorsele alt metin ekle
   - Site haritasını Google Search Console'a gonder

4. Gelistirici aracları icin sıralama yapan icerik:
   - Belge sayfaları (SEO icin sasırtıcı derecede iyi)
   - Karsılastırma sayfaları ("X vs Y")
   - Ogretici sayfaları ("X ile Y nasıl yapılır")
   - Degisiklik gunlugu sayfaları (Google icin taze icerik sinyali)
```

```bash
# Site haritanı Google Search Console'a gonder
# 1. https://search.google.com/search-console adresine git
# 2. Mulkunu ekle (domain veya URL oneki)
# 3. Sahipligi dogrula (DNS TXT kaydı veya HTML dosyası)
# 4. Site haritası URL'ini gonder: senindomain.com/sitemap.xml

# Astro kullanıyorsan:
pnpm add @astrojs/sitemap
# Site haritası /sitemap.xml'de otomatik uretilir

# Next.js kullanıyorsan, next-sitemap.config.js'e ekle:
# pnpm add next-sitemap
```

**Beklenen sonuclar:**
- Ay 1-3: Minimal organik trafik (<100/ay)
- Ay 3-6: Buyuyen trafik (100-500/ay)
- Ay 6-12: Onemli trafik (500-5.000/ay)
- Ay 12+: Efor olmadan buyuyen birikimli trafik

{@ temporal market_timing @}

### Kanal Secim Cercevesi

Yedisini de iyi yapamazsın. Bu matrise dayalı 2-3 sec:

| Eger... | Oncelik ver | Atla |
|---|---|---|
| Bu hafta sonu lansıyorsan | Reddit + HN | SEO, Twitter (cok yavaş) |
| Once kitle insa ediyorsan | Twitter + Blog gonderileri | Dogrudan ulasım, PH |
| $99+ urun satıyorsan | Dogrudan ulasım + HN | Dev.to (kitle ucretsiz bekler) |
| Uzun oyun oynuyorsan | SEO + Blog gonderileri + Twitter | PH (tek sans, sonra kullan) |
| Ingilizce konusmuyorsan | Dev.to + Reddit (kuresel) | HN (ABD merkezli) |

### Senin Sıran

1. **2-3 kanalını sec.** Yukarıdaki matrise ve urun tipine dayalı olarak, odaklanacagın kanalları sec. Her biri icin planlanan zaman cizelgenle birlikte yaz.

2. **Reddit gonderini yaz.** Ders 1'deki sablonu kullanarak, r/SideProject gonderi taslağını simdi yaz. Kaydet. Lansman gununde gondereceksin.

3. **Ilk blog gonderini yaz.** "[Urunun cozdugu problemi nasıl cozulur]" gonderisini tasla. Lansırmadan sonraki ilk hafta icinde Dev.to veya bloguna gider. 1.500-2.000 kelimeyi hedefle.

4. **Google Search Console'u ayarla.** Bu 5 dakika surer ve birinci gunden SEO verisi verir. Temel verilere sahip olmak icin lansırmadan once yap.

---

## Ders 6: Lansman Kontrol Listen

*"Umit lansman stratejisi degildir. Kontrol listeleri oyledir."*

### Lansman Oncesi Kontrol Listesi

Her maddeyi incele. Her "Zorunlu" madde isaretlenene kadar lansırma. "Onerilen" maddeler gerekirse 1. Hafta'da yapılabilir.

**Urun (Zorunlu):**

```
- [ ] Cekirdek ozellik landing page'de tanımlandıgı gibi calısıyor
- [ ] Satın alma → teslimat akısında kritik hata yok
- [ ] Chrome, Firefox ve Safari'da calısıyor (web urunleri icin)
- [ ] Mobil uyumlu landing page (trafigın %50+'sı mobil)
- [ ] Hata mesajları yardımcı, stack trace degil
- [ ] Herhangi bir asenkron islem icin yukleme durumları
```

**Landing Page (Zorunlu):**

```
- [ ] Net baslik: 8 kelime veya daha azıyla ne yaptıgı
- [ ] Problem ifadesi: Musteri dilinde 3 acı noktası
- [ ] Cozum bolumu: Urunun ekran goruntuleri veya demo'ları
- [ ] Fiyatlandırma: Gorunur, net, satın alma butonuyla
- [ ] Eyleme cagrı: Bir ana buton, katlamanın ustunde gorunur
- [ ] Altbilgide baglantılı gizlilik politikası
- [ ] Altbilgide baglantılı kullanım sartları
```

**Odemeler (Zorunlu):**

```
- [ ] Odeme akısı test modunda bastan sona test edildi
- [ ] Odeme akısı canli modda bastan sona test edildi ($1 test satın alması)
- [ ] Webhook odeme onayı alıyor
- [ ] Musteri odemeden sonra urun erisimi alıyor
- [ ] Iade sureci belgelendi (iade istekleri ALACAKSIN)
- [ ] Makbuz/fatura otomatik gonderiliyor
```

**Altyapı (Zorunlu):**

```
- [ ] Ozel domain canli siteye isaret ediyor
- [ ] HTTPS calısıyor (yesil kilit)
- [ ] Calısma suresi izleme aktif
- [ ] Analitik betigi yuklendi ve veri alıyor
- [ ] Iletisim e-postası calısıyor (sen@senindomain.com)
```

**Dagıtım (Zorunlu):**

```
- [ ] Reddit gonderisi taslanıp hazır
- [ ] Show HN gonderisi taslanıp hazır (uygulanabilirse)
- [ ] Twitter lansman dizisi taslanmıs
- [ ] Paylasım icin 2-3 topluluk belirlenmis
```

**Onerilen (1. Hafta):**

```
- [ ] Sosyal paylasım onizlemeleri icin OpenGraph meta etiketleri
- [ ] Ozel 404 sayfası
- [ ] SSS sayfası veya bolumu
- [ ] Musteri alisma e-posta dizisi (karsilama + baslarken)
- [ ] Degisiklik gunlugu sayfası (bos bile olsa — guncelleme taahhudunu gosterir)
- [ ] Blog gonderisi: "[Urunu] 48 saatte insa ettim"
- [ ] Google Search Console dogrulanmıs ve site haritası gonderilmis
```

### Lansman Sonrası Eylem Maddeleri

**1. Gun (Lansman Gunu):**

```
Sabah:
- [ ] Reddit'te gonder (r/SideProject + 1 nis subreddit)
- [ ] Show HN gonder (uygulanabilirse)
- [ ] Twitter lansman dizisini gonder

Tum gun:
- [ ] Reddit, HN ve Twitter'da HER yoruma yanıt ver
- [ ] Hata gunluklerini ve analitigi gercek zamanlı izle
- [ ] Kullanıcılar tarafından kesfedilen hataları anında duzelt
- [ ] Her musteriye kisisel tesekkur e-postası gonder

Aksam:
- [ ] Metrikleri kontrol et: ziyaretciler, donusum oranı, gelir
- [ ] Analitik panelinin ekran goruntusu al (daha sonra isteyeceksin)
- [ ] En yaygın 3 geri bildirimi yaz
```

**1. Hafta:**

```
- [ ] Tum geri bildirimlere ve destek isteklerine 24 saat icinde yanıt ver
- [ ] Lansırmada belirlenen ilk 3 hata/sorunu duzelt
- [ ] Ilk blog gonderini yaz ve yayımla
- [ ] Tum musterilere geri bildirim isteyen takip e-postası gonder
- [ ] Analitigi incele: en yuksek sekme oranları hangi sayfalarda?
- [ ] Basit bir geri bildirim toplama yontemi ayarla (e-posta, Typeform veya Canny)

Kaydedilecek haftalık metrikler:
| Metrik                  | Hedef     | Gercek |
|-------------------------|-----------|--------|
| Benzersiz ziyaretciler  | 500+      |        |
| Odeme tıklama oranı     | %2-5      |        |
| Satın alma donusumu      | %1-3      |        |
| Gelir                   | $50+      |        |
| Destek istekleri         | <10       |        |
| Iade istekleri           | <2        |        |
```

**1. Ay:**

```
- [ ] Musteri geri bildirimine dayalı 4 haftalık gelistirme lansır
- [ ] 2+ blog gonderisi yayımla (SEO insası)
- [ ] Musterilerden 3+ referans topla
- [ ] Landing page'e referanslar ekle
- [ ] Fiyatlandırmayı degerlendir: cok yuksek mi? cok dusuk mu? (donusum verilerini incele)
- [ ] Product Hunt'ta "buyuk lansmanı" planla (uygulanabilirse)
- [ ] Gelecek urun lansırmaları icin e-posta listesi insa etmeye basla
- [ ] Dagıtım kanalı stratejini gozden gecir ve ayarla

Aylık finansal inceleme:
| Kategori                | Tutar     |
|-------------------------|-----------|
| Brut gelir              | $         |
| Odeme islemci ucretleri  | $         |
| Barındırma/altyapı      | $         |
| API maliyetleri          | $         |
| Net kar                 | $         |
| Yatırılan saatler        |           |
| Etkili saatlik ucret     | $         |
```

### Metrik Panosu

Her gun kontrol ettigin basit bir metrik panosu ayarla. Suslü olması gerekmiyor — bir elektronik tablo ise yarar.

```
=== GUNLUK METRIKLER (her sabah kontrol et) ===

Tarih: ___
Dunku ziyaretciler: ___
Dunku yeni musteriler: ___
Dunku gelir: $___
Destek istekleri: ___
Calısma suresi: ___%

=== HAFTALIK METRIKLER (her pazartesi kontrol et) ===

Haftası: ___
Toplam ziyaretciler: ___
Toplam musteriler: ___
Toplam gelir: $___
Donusum oranı: ___% (musteriler / ziyaretciler)
En cok ziyaret edilen sayfa: ___
En onemli trafik kaynagı: ___
En onemli geri bildirim teması: ___

=== AYLIK METRIKLER (ayın 1'inde kontrol et) ===

Ay: ___
Toplam gelir: $___
Toplam giderler: $___
Net kar: $___
Toplam musteriler: ___
Iadeler: ___
Kayıp oranı (abonelikler): ___%
MRR (Aylık Yinelenen Gelir): $___
Gecen aya gore buyume oranı: ___%
```

**Gizliligi koruyan analitik kurulumu:**

```javascript
// Plausible kullanıyorsan, bunların cogunu panellerinde elde edersin.
// Ozel olay izleme icin:

// Odeme tıklamalarını izle
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Basarılı satın almaları izle (webhook basarı isleyicinden cagır)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### Ne Zaman Ikiye Katlamak, Pivot Yapmak veya Sonlandırmak

30 gun veriyle, karar vermek icin yeterli sinyal var:

**Ikiye Katla (devam et, daha fazla yatır):**

```
Sinyaller:
- Gelir hafta hafta buyuyor (yavas bile olsa)
- Musteriler spesifik ozellik istekleri saglıyor (DAHA FAZLA istiyorlar)
- Donusum oranı sabit veya gelistiyor
- Organik trafik alıyorsun (insanlar gonderilerin olmadan seni buluyor)
- En az bir musteri "bu bana [zaman/para] tasarruf ettirdi" dedi

Eylemler:
- Dagıtım eforlarını artır (kanal ekle)
- En cok talep edilen ozelligi lansır
- Fiyatları hafifce yukselt
- Gelecek lansırlar icin e-posta listesi insasına basla
```

**Pivot Yap (acıyı degistir, cekirdegi koru):**

```
Sinyaller:
- Ziyaretci var ama satıs yok (insanlar ilgili ama satın almıyor)
- Beklenmedik kitleden satıslar (hedeflediklerinden farklı insanlar)
- Musteriler urunu beklediginden farklı kullanıyor
- Geri bildirim tutarlı olarak cozdugunden farklı bir probleme isaret ediyor

Eylemler:
- Gercek kitle/kullanım durumu icin landing page'i yeniden yaz
- Fiyatları gercek kitlenin odeme istekliligine gore ayarla
- Insanların gercekten kullandıgına dogru ozellikleri yeniden onceliklendır
- Kodu koru, konumlandırmayı degistir
```

**Sonlandır (dur, ogren, baska bir sey insa et):**

```
Sinyaller:
- Dagıtım eforlarına ragmen ziyaretci yok (talep problemi)
- Ziyaretci var ama odemeye sıfır tıklama (ayarlamalardan sonra
  devam eden konumlandırma/fiyat problemi)
- 4+ hafta buyume trendi olmadan duragan gelir
- Uzerinde calısmaktan korkuyorsun (solo urunler icin motivasyon onemli)
- Pazar degisti (rakip lansırdı, teknoloji degisti)

Eylemler:
- Sonuc raporu yaz: ne ise yaradı, ne yaramadı, ne ogrendin
- Kodu sakla — parcalar sonraki urununde yararlı olabilir
- Insa etmekten bir hafta ara ver
- Yeni bir fikir icin dogrulama surecini basla
- Bu basarısızlık degil. Veri. Cogu urun islemez.
  Para kazanan gelistiriciler bir urunle bir yıl gecirenler degil,
  5 urun lansıranlar.
```

### Lansman Belgesi Sablonu

Bu, Modul E icin teslim edilecek seyin. Bu belgeyi olustur ve lansmanını gerceklestirirken doldur.

```markdown
# Lansman Belgesi: [Urun Adı]

## Lansman Oncesi

### Dogrulama Ozeti
- **Arama hacmi:** [Google Trends/Ahrefs sayıları]
- **Baslik kanıtı:** [talep gosteren 5+ basliga baglantılar]
- **Rakip denetimi:** [guclü/zayıf yanları olan 3+ rakip]
- **"10 kisi oder" kanıtı:** [bunu nasıl dogruladın]

### Urun
- **URL:** [canli urun URL'si]
- **Domain:** [satın alınan domain]
- **Barındırma:** [platform]
- **Cekirdek ozellikler (v0.1):**
  1. [Ozellik 1]
  2. [Ozellik 2]
  3. [Ozellik 3]

### Fiyatlandırma
- **Fiyat:** $[tutar]
- **Katman yapısı:** [Temel/Pro/Takım veya tek katman]
- **Odeme platformu:** [Lemon Squeezy/Stripe]
- **Odeme URL'si:** [baglantı]

### Yasal
- **Gizlilik politikası:** [URL]
- **Kullanım sartları:** [URL]
- **Isletme tuzel kisilik:** [tur veya "bireysel isletmeci"]

## Lansman

### Dagıtım Kanalları
| Kanal   | Gonderi URL | Gonderim Tarihi | Sonuclar |
|---------|-------------|-----------------|---------|
| Reddit  | [baglantı]  | [tarih]         | [ziyaretciler, oylar] |
| HN      | [baglantı]  | [tarih]         | [ziyaretciler, puanlar] |
| Twitter | [baglantı]  | [tarih]         | [goruntulenme, tıklamalar] |

### 1. Gun Metrikleri
- Ziyaretciler: ___
- Odeme tıklamaları: ___
- Satın almalar: ___
- Gelir: $___

### 1. Hafta Metrikleri
- Toplam ziyaretciler: ___
- Toplam satın almalar: ___
- Toplam gelir: $___
- Donusum oranı: ___%
- En onemli geri bildirim: ___

### 1. Ay Metrikleri
- Toplam gelir: $___
- Toplam giderler: $___
- Net kar: $___
- Toplam musteriler: ___
- Karar: [ ] Ikiye katla [ ] Pivot yap [ ] Sonlandır

## Lansman Sonrası Yol Haritası
- Hafta 2: [planlanan gelistirme]
- Hafta 3: [planlanan gelistirme]
- Hafta 4: [planlanan gelistirme]
- Ay 2: [planlanan ozellik/genisleme]

## Ogrenilen Dersler
- Ne ise yaradı: ___
- Ne ise yaramadı: ___
- Neyi farklı yapardım: ___
```

### 4DA Entegrasyonu

> **4DA Entegrasyonu:** 4DA'nın eylemlenebilir sinyalleri icerigi aciliyete gore sınıflandırır. Populer bir paketteki guvenlik acıgı hakkında "kritik" sinyal su anlama gelir: duzeltme veya goc aracını SIMDI, herkesten once insa et. Yeni bir framework hakkında "yukselis trendi" sinyali su anlama gelir: bu hafta sonu rekabet neredeyse sıfırken baslangıc kitini insa et. Ders 1'deki 48 saatlik sprint, fikrin zamana duyarlı bir sinyalden geldigi zaman en iyi calısır. 4DA istihbarat akısını sprint takvimine bagla — yuksek aciliyet fırsatı belirdiginde, sonraki hafta sonunu blokla ve icra et. Fırsatları yakalayan gelistiriciler ile kacıranlar arasındaki fark yetenek degil. Hız. 4DA sana radarı verir. Bu modul sana lansman sırasını verir. Birlikte, sinyalleri gelire cevirirler.

### Senin Sıran

1. **Lansman oncesi kontrol listesini tamamla.** Her maddeyi incele. Her birini yapılmıs olarak isaretle veya ne zaman yapacagını planla. "Zorunlu" maddeleri atlama.

2. **Lansman Belgeni olustur.** Yukarıdaki sablonu tercih ettigin belge aracına kopyala. Simdi bildiklerinin tumunu doldur. Lansırma sırasında ve sonrasında dolduracagın metrikler icin bosluklari bırak.

3. **Lansman tarihini belirle.** Takvimini ac. Onumuzde ki 2 hafta icinde belirli bir cumartesi sec. Yaz. Birine soyle — bir arkadasa, bir partnere, bir Twitter takipcisine. Hesap verebilirlik gercek yapar.

4. **Sonlandırma kriterlerini belirle.** Lansırmadan once karar ver: "[Y] dagıtım eforuna ragmen 30 gun sonra [X]'ten az satısım varsa, [pivot yapacagım/sonlandıracagım]." Bunu Lansman Belgene yaz. Onceden belirlenmis kriterler, batık maliyet yanılgısı nedeniyle olu bir urune aylarca dokmeyi onler.
{? if progress.completed("S") ?}
   Modul S'deki Egemen Yıgın Belgene geri bak — butce kısıtlamaların ve isletme maliyetlerin, spesifik durumun icin "karlı"nın ne anlama geldigini tanımlar.
{? endif ?}

5. **Lansır.** Oyun planın var. Araclarin var. Bilgin var. Geriye kalan tek sey eylem. Internet bekliyor.

---

## Modul E: Tamamlandı

### Iki Haftada Ne Insa Ettin

{? if dna.identity_summary ?}
> **Gelistirici kimligin:** {= dna.identity_summary | fallback("Henuz profillenmedi") =}. Bu modulde insa ettigin her sey bu kimlikten yararlanır — lansman hızın mevcut uzmanlıgının bir fonksiyonu.
{? endif ?}

Bu modulu basladıgında sahip olmadıgın seylere bak:

1. **48 saatlik yurutme cercevesi** her insa ettigin urun icin tekrarlayabilir — dogrulanmıs fikirden canli urune bir hafta sonunda.
2. **Lansman zihniyeti** varolusu mukemmelligi, veriyi tahminlemeyi ve yinelemeyi planlamayı onceliklendiren.
3. **Fiyatlandırma stratejisi** gercek psikoloji ve gercek sayılara dayalı, umit ve dusuk fiyatlandırmaya degil.
4. **Yasal temel** seni felc etmeden koruyan — gizlilik politikası, sartlar, tuzel kisilik planı.
5. **Dagıtım oyun planı** yedi kanal icin spesifik sablonlar, zamanlama ve beklenen sonuclarla.
6. **Lansman kontrol listesi ve izleme sistemi** kaosu surece cevirir — tekrarlanabilir, olculebilir, gelistirilebilir.
7. **Canli bir urun, odemeleri kabul eden, gercek insanların ziyaret ettigi.**

Son olan onemli olan. Geri kalan her sey hazırlık. Urun kanıt.

### Sırada Ne Var: Modul E2 — Gelisen Avantaj

Modul E1 seni lansırmaya gotürdu. Modul E2 seni onde tutar.

Modul E2'nin kapsadıkları:

- **Trend tespit sistemleri** — fırsatları belirginlesmeden 2-4 hafta once nasıl fark edilir
- **Rekabet izleme** — alanındaki digerlerinin ne insa ettigini ve nasıl fiyatlandırdıgını takip etme
- **Teknoloji dalgası surme** — urunlerde yeni teknolojiyi ne zaman benimsenmeli ne zaman beklenmeli
- **Musteri gelistirme** — ilk 10 musterini urun danısma kuruluna cevirme
- **Ikinci urun kararı** — urun #2'yi ne zaman insa etmeli vs. urun #1'i gelistirmeli

Tutarlı gelir elde eden gelistiriciler bir kez lansıranlar degil. Lansıran, yineleyen ve pazarın onunde kalanlar. Modul E2 sana onde kalmak icin sistemi verir.

### Tam STREETS Yol Haritası

| Modul | Baslik | Odak | Sure |
|-------|--------|------|------|
| **S** | Egemen Kurulum | Altyapı, yasal, butce | Hafta 1-2 |
| **T** | Teknik Hendekler | Savunulabilir avantajlar, tescilli varlıklar | Hafta 3-4 |
| **R** | Gelir Motorları | Kodlu spesifik monetizasyon oyun planları | Hafta 5-8 |
| **E** | Uygulama Rehberi | Lansman sıraları, fiyatlandırma, ilk musteriler | Hafta 9-10 (tamamlandı) |
| **E** | Gelisen Avantaj | Onde kalma, trend tespiti, adaptasyon | Hafta 11-12 |
| **T** | Taktiksel Otomasyon | Pasif gelir icin operasyonların otomasyonu | Hafta 13-14 |
| **S** | Akıs Yıgma | Birden fazla gelir kaynagı, portfoy stratejisi | Hafta 15-16 |

Yarısını gectın. Canli bir urunun var. Bu seni bagımsız gelir insa etmek isteyen ama asla bu kadar ileri gelemeyen gelistiricilerin %95'inin onune koyar.

> **STREETS Ilerlemesi:** {= progress.completed_count | fallback("0") =} / {= progress.total_count | fallback("7") =} modul tamamlandı. {? if progress.completed_modules ?}Tamamlananlar: {= progress.completed_modules | fallback("Henuz yok") =}.{? endif ?}

Simdi buyut.

---

**Urunun canli. Odemen calısıyor. Insanlar sana para odeyebilir.**

**Bundan sonraki her sey optimizasyon. Ve optimizasyon eglenceli kısım.**

*Senin kurulumun. Senin kuralların. Senin gelirin.*
