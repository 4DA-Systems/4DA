# Modül S: Gelir Akışlarını İstiflemek

**STREETS Geliştirici Gelir Kursu — Ücretsiz Modül (7 Modülün Tümü 4DA İçinde Ücretsiz)**
*Hafta 14-16 | 6 Ders | Çıktı: Akış Yığının (12 Aylık Gelir Planı)*

> "Tek akış bir yan iştir. Üç akış bir iştir. Beş akış özgürlüktür."

---

{? if progress.completed("T") ?}
On üç hafta boyunca çoğu geliştiricinin asla inşa etmediği bir şey inşa ettin: egemen bir gelir operasyonu. Altyapın var. Hendeklerin var. Gelir motorların çalışıyor. Uygulama disiplinin var. İstihbaratın var. Otomasyonun var.
{? else ?}
On üç hafta boyunca çoğu geliştiricinin asla inşa etmediği bir şey inşa ettin: egemen bir gelir operasyonu. Altyapın var. Gelir motorların çalışıyor. Uygulama disiplinin var. İstihbaratın var. Otomasyonun var. (Bu modüldeki hendek tabanlı stratejileri tam olarak etkinleştirmek için Modül T — Teknik Hendekler'i tamamla.)
{? endif ?}

Şimdi ayda ekstra {= regional.currency_symbol | fallback("$") =}2K kazanan geliştiriciyi maaşını tamamen değiştiren geliştiriciden ayıran kısım geliyor: **istifleme**.

Tek bir gelir akışı — ne kadar iyi olursa olsun — kırılgandır. En büyük müşterin ayrılır. Platform API fiyatlandırmasını değiştirir. Algoritma kayması trafiğini çökertir. Bir rakip ürününün ücretsiz versiyonunu yayınlar. Bunlardan herhangi biri tek akışlı bir geliri bir gecede yerle bir edebilir. Bunun olduğunu gördün. Belki sana da oldu.

Birden fazla gelir akışı sadece toplanmaz. Bileşik olarak büyür. Birbirini güçlendirir. Herhangi bir tek akışı kaybetmenin felaket değil rahatsızlık olduğu bir sistem oluştururlar. Ve doğru tasarlandıklarında, zamanla hızlanan bir volan içinde birbirini beslerler.

Bu modül o sistemi tasarlamakla ilgili. Rastgele yan projeler biriktirmekle değil, bilinçli olarak bir gelir portföyü inşa etmekle — tıpkı akıllı bir yatırımcının finansal portföy inşa ettiği gibi.

Bu üç haftanın sonunda elinde şunlar olacak:

- Beş gelir akışı kategorisi ve nasıl etkileştiği hakkında net bir anlayış
- $10K/ay'a giden birden fazla somut yol, gerçek sayılar ve gerçekçi zaman çizelgeleriyle
- Düşük performanslı akışları ne zaman sonlandıracağına karar vermek için bir çerçeve
- Erken geliri hızlanan büyümeye dönüştüren bir yeniden yatırım stratejisi
- Tamamlanmış bir Stream Stack belgesi — aylık kilometre taşlarıyla kişisel 12 aylık gelir planın

Bu son modül. STREETS'te inşa ettiğin her şey burada birleşiyor.

{? if progress.completed_modules ?}
> **STREETS ilerlemen:** {= progress.completed_count | fallback("0") =} / {= progress.total_count | fallback("7") =} modül tamamlandı ({= progress.completed_modules | fallback("henüz hiçbiri") =}). Bu modül önceki modüllerdeki her şeyi bir araya getiriyor — ne kadar çok tamamladıysan, Stream Stack'in o kadar somut olacak.
{? endif ?}

Hadi istifleyelim.

---

## Ders 1: Gelir Portföyü Kavramı

*"Gelirini bir yatırım portföyü gibi değerlendir — çünkü tam olarak öyle."*

### Geliştiriciler Gelir Hakkında Neden Yanlış Düşünüyor

Çoğu geliştirici gelir hakkında istihdam gibi düşünür: tek kaynak, tek maaş çeki, tek bağımlılık. Bağımsız olarak kazanmaya başladıklarında bile aynı kalıba düşerler — tek bir serbest müşteri, tek bir ürün, tek bir kanal. Miktar değişebilir. Kırılganlık değişmez.

Yatırım profesyonelleri bunu on yıllar önce çözdü. Tüm parayı tek bir hisseye koymassın. Varlık sınıfları arasında çeşitlendirirsin — bazıları istikrar için, bazıları büyüme için, bazıları uzun vadeli değer artışı için. Her biri farklı bir amaca hizmet eder, farklı bir zaman çizelgesinde çalışır ve farklı piyasa koşullarına tepki verir.

Gelirin de aynı şekilde çalışır. Ya da en azından çalışmalı.

### 5 Akış Kategorisi

{@ insight engine_ranking @}

Her geliştirici gelir akışı beş kategoriden birine girer. Her birinin farklı bir risk profili, zaman ufku ve büyüme eğrisi vardır.

```
Akış 1: Hızlı Nakit        — Serbest/danışmanlık — faturaları ŞİMDİ öder
Akış 2: Büyüyen Varlık      — SaaS/ürün          — faturaları 6 ayda öder
Akış 3: İçerik Bileşiği     — Blog/bülten/YT     — faturaları 12 ayda öder
Akış 4: Pasif Otomasyon      — Botlar/API/veri    — sen uyurken öder
Akış 5: Hisse Oyunu          — Open source -> şirket — uzun vadeli zenginlik
```

**Akış 1: Hızlı Nakit (Serbest Çalışma / Danışmanlık)**

Bu paraya giden en doğrudan yol. Birinin sorunu var, sen çözüyorsun, sana ödüyorlar. İnşa edilecek ürün yok, büyütülecek kitle yok, memnun edilecek algoritma yok. Uzmanlaşmış becerilerin olduğu için zamanını para karşılığı premium bir ücretle takas edersin.

- Gelir zaman çizelgesi: 1-2 hafta içinde $0'dan ilk dolara
- Tipik aralık: haftada 10-20 saat ile $2,000-15,000/ay
- Tavan: saatlerinle sınırlı
- Risk: müşteri yoğunlaşması, ziyafet-açlık döngüleri

Hızlı Nakit senin temeldir. Sonunda onun yerini alacak akışları inşa ederken faturaları öder.

**Akış 2: Büyüyen Varlık (SaaS / Ürün)**

Çoğu geliştiricinin hayal ettiği ama çok azının gerçekten yayınladığı akış. Bir kez ürün inşa edersin, birçok kez satarsın. Product-market fit'i bulduğunda marjlar olağanüstü olur. Ama o fit'i bulmak aylar alır ve gelir eğrisi sıfırdan başlayıp, yükselişe geçmeden önce acı verici bir şekilde düz kalır.

- Gelir zaman çizelgesi: İlk anlamlı gelire 3-6 ay
- Tipik aralık: 12-18 ay sonunda $500-5,000/ay
- Tavan: fiilen sınırsız (zamanınla değil müşterilerle ölçeklenir)
- Risk: kimsenin istemediği bir şey inşa etme, destek yükü

**Akış 3: İçerik Bileşiği (Blog / Bülten / YouTube)**

İçerik başlatılması en yavaş ve sürdürülmesi en güçlü akıştır. Yayınladığın her içerik parçası bileşik olarak büyür. Bugün yazılan bir blog yazısı iki yıl sonra trafik çeker. Bu ay yüklenen bir YouTube videosu gelecek yıl önerilir. Bir bülten her hafta abone tabanını büyütür.

- Gelir zaman çizelgesi: İlk anlamlı gelire 6-12 ay
- Tipik aralık: 12-18 ay sonunda $500-5,000/ay
- Tavan: yüksek (kitle bileşik olarak büyür, monetizasyon seçenekleri çoğalır)
- Risk: tutarlılık acımasızdır, algoritma değişiklikleri, platform bağımlılığı

**Akış 4: Pasif Otomasyon (Botlar / API'ler / Veri Ürünleri)**

Bu geliştiricilere özgü bir akıştır. Senin doğrudan müdahalen olmadan değer üreten otomatik sistemler inşa edersin. Veri işleme pipeline'ları, API servisleri, izleme botları, otomatik raporlar. Gelir sistemin çalışmasından gelir, senin çalışmandan değil.

{? if profile.gpu.exists ?}
> **Donanım avantajı:** {= profile.gpu.vram | fallback("ayrılmış") =} VRAM'e sahip {= profile.gpu.model | fallback("GPU") =}'n, LLM destekli otomasyon akışları açar — yerel çıkarım API'leri, AI destekli veri işleme ve akıllı izleme servisleri — tamamı istek başına neredeyse sıfır marjinal maliyetle.
{? endif ?}

- Gelir zaman çizelgesi: İlk gelire 2-4 ay (alanı biliyorsan)
- Tipik aralık: {= regional.currency_symbol | fallback("$") =}300-3,000/ay
- Tavan: orta (niş boyutuyla sınırlı, ama çalışır hale geldiğinde neredeyse sıfır zaman yatırımı)
- Risk: teknik arızalar, nişin kuruması

**Akış 5: Hisse Oyunu (Open Source'dan Şirkete)**

Bu uzun vadeli oyun. Open source olarak bir şey inşa edersin, etrafında bir topluluk büyütürsün, sonra premium özellikler, barındırılan versiyonlar veya risk sermayesi yoluyla monetize edersin. Zaman çizelgesi aylarla değil yıllarla ölçülür. Ama sonuç aylık gelirle değil şirket değerlemeleriyle ölçülür.

- Gelir zaman çizelgesi: Anlamlı gelire 12-24 ay (VC yolu için daha uzun)
- Tipik aralık: öngörülemez — iki yıl $0 olabilir, sonra $50K/ay
- Tavan: devasa (Supabase, PostHog, Cal.com hepsi bu yolu izledi)
- Risk: tüm kategorilerin en yükseği — çoğu open source projesi asla monetize olmaz

### Tek Akışlı Gelir Neden Kırılgandır

Her ay yaşanan üç gerçek senaryo:

1. **Müşteri ayrılır.** İki müşteriye danışmanlıktan $8K/ay kazanıyorsun. Biri satın alınır, yeni yönetim her şeyi şirket içine alır. Anında $4K/ay'dasın. Faturalar yarıya inmez.

2. **Platform kuralları değiştirir.** Bir Chrome uzantısından $3K/ay kazanıyorsun. Google, Web Store politikalarını değiştirir. Uzantın, çözümü 6 hafta süren bir "politika ihlali" nedeniyle listeden çıkarılır. Gelir: 6 hafta boyunca $0.

3. **Algoritma kayar.** Blogun organik arama trafiğinden $2K/ay affiliate geliri üretiyor. Google bir çekirdek güncelleme yayınlar. Trafiğin bir gecede %60 düşer. Yanlış bir şey yapmadın. Algoritma sadece farklı içerik göstermeye karar verdi.

Bunların hiçbiri varsayımsal değil. Üçü de düzenli olarak yaşanır. Bunları finansal panik yaşamadan atlatan geliştiriciler, birden fazla akışa sahip olanlardır.

### İki Zihniyet: Maaş Değiştirme vs. Maaş Takviyesi

Portföyünü tasarlamadan önce hangi oyunu oynadığına karar ver. Farklı stratejiler gerektirirler.

**Maaş Takviyesi ($2K-5K/ay):**
- Hedef: tam zamanlı işin üzerine ekstra gelir
- Zaman bütçesi: haftada 10-15 saat
- Öncelik: düşük bakım, yüksek marj
- En iyi akışlar: 1 Hızlı Nakit + 1 Pasif Otomasyon, veya 1 Büyüyen Varlık + 1 İçerik Bileşiği
- Risk toleransı: orta (güvenlik ağı olarak maaşın var)

**Maaş Değiştirme ($8K-15K+/ay):**
- Hedef: tam zamanlı gelirini tamamen değiştir
- Zaman bütçesi: haftada 25-40 saat (artık bu senin işin)
- Öncelik: önce istikrar, sonra büyüme
- En iyi akışlar: birden fazla kategoride 3-5 akış
- Risk toleransı: temel akışlarda düşük, büyüme akışlarında yüksek
- Ön koşul: sıçramadan önce 6 aylık giderler biriktirilmiş

> **Açık Konuşalım:** Çoğu kişi Maaş Takviyesi ile başlamalı. Çalışırken akışlar inşa et, 6+ ay istikrarlı olduklarını kanıtla, agresif biriktir, sonra geçiş yap. İlk ayda "tam gaza" gitmek için işini bırakan geliştiriciler, 6 ay sonra birikimlerini ve özgüvenlerini tüketmiş olarak istihdama dönenlerdir. Sıkıcı mı? Evet. Etkili mi? Evet, aynı zamanda.

### Gelire Uygulanan Portföy Teorisi

Yatırım portföyleri risk ve getiriyi dengeler. Gelir portföyün de öyle olmalı.

**"Önce Güvenlik" geliştiricisi:** %60 danışmanlık, %30 ürünler, %10 içerik
- Hızlı Nakit ağırlıklı. Güvenilir, öngörülebilir, faturaları öder.
- Ürünler arka planda yavaşça büyür.
- İçerik gelecekteki kaldıraç için bir kitle oluşturur.
- En uygun: aileleri, ipotekleri, düşük risk toleransı olan geliştiriciler.
- Beklenen toplam: kararlı durumda $6K-10K/ay.

**"Büyüme Modu" geliştiricisi:** %20 danışmanlık, %50 ürünler, %30 içerik
- Danışmanlık minimum giderleri karşılar.
- Zamanın çoğu ürün inşa etme ve pazarlamaya gider.
- İçerik ürün hunisini besler.
- En uygun: birikimleri olan, yüksek risk toleranslı, büyük bir şey inşa etmek isteyen geliştiriciler.
- Beklenen toplam: 12 ay boyunca $4K-8K/ay, ürünler tutarsa sonra $10K-20K/ay.

**"Bağımsızlığa Geçiş" geliştiricisi:** %0 danışmanlık, %40 SaaS, %30 içerik, %30 otomasyon
- Para için zaman takası yok. Her şey ölçeklenir.
- 12-18 aylık pist veya mevcut akış geliri gerektirir.
- İçerik ve otomasyon, SaaS için pazarlama motorudur.
- En uygun: ürünlerini zaten valide etmiş ve tam zamanlı geçişe hazır geliştiriciler.
- Beklenen toplam: 6-12 ay dalgalı, sonra $10K-25K/ay.

### Zaman Dağılımı: Her Akışa Ne Kadar Yatırım Yapmalı

Saatlerin senin sermayendir. Bilinçli olarak dağıt.

| Akış Kategorisi | Bakım Aşaması | Büyüme Aşaması | İnşa Aşaması |
|----------------|------------------|-------------|----------------|
| Hızlı Nakit | 2-5 saat/hafta | 5-10 saat/hafta | 10-20 saat/hafta |
| Büyüyen Varlık | 3-5 saat/hafta | 8-15 saat/hafta | 15-25 saat/hafta |
| İçerik Bileşiği | 3-5 saat/hafta | 5-10 saat/hafta | 8-15 saat/hafta |
| Pasif Otomasyon | 1-2 saat/hafta | 3-5 saat/hafta | 8-12 saat/hafta |
| Hisse Oyunu | 5-10 saat/hafta | 15-25 saat/hafta | 30-40 saat/hafta |

Çoğu geliştirici aynı anda birden fazla akışta "İnşa Aşaması"nda olmamalıdır. Bir akışı bakıma geçene kadar inşa et, sonra bir sonrakini inşa etmeye başla.

### Gelir Zaman Çizelgeleri: Gerçekçi Aylık Tablo

Her akış türünün 12 ay boyunca gerçekte nasıl göründüğü. En iyi durum değil. En kötü durum değil. Tutarlı şekilde uygulayan geliştiriciler için en yaygın durum.

**Hızlı Nakit (Danışmanlık):**
```
Ay 1:  $500-2,000   (ilk müşteri, muhtemelen düşük fiyatlanmış)
Ay 3:  $2,000-4,000 (ücretler ayarlanmış, 1-2 düzenli müşteri)
Ay 6:  $4,000-8,000 (tam pipeline, premium ücretler)
Ay 12: $5,000-10,000 (seçici müşteriler, ücretler tekrar artırılmış)
```

**Büyüyen Varlık (SaaS/Ürün):**
```
Ay 1:  $0           (hâlâ inşa ediliyor)
Ay 3:  $0-100       (yayınlandı, ilk birkaç kullanıcı)
Ay 6:  $200-800     (çekiş buluyor, geri bildirime göre iterasyon)
Ay 9:  $500-2,000   (product-market fit belirginleşiyor)
Ay 12: $1,000-5,000 (PMF gerçekse bileşik büyüme)
```

**İçerik Bileşiği (Blog/Bülten/YouTube):**
```
Ay 1:  $0           (yayınlıyorsun, henüz kitle yok)
Ay 3:  $0-50        (küçük kitle, belki ilk affiliate satışı)
Ay 6:  $50-300      (büyüyor, biraz organik trafik)
Ay 9:  $200-1,000   (içerik kütüphanesi bileşik büyüyor)
Ay 12: $500-3,000   (gerçek kitle, birden fazla monetizasyon)
```

**Pasif Otomasyon (Botlar/API'ler/Veri):**
```
Ay 1:  $0           (sistemi inşa ediyorsun)
Ay 3:  $50-300      (ilk ödeme yapan kullanıcılar)
Ay 6:  $200-1,000   (sistem istikrarlı, organik olarak büyüyor)
Ay 12: $500-2,000   (minimum bakımla çalışıyor)
```

> **Yaygın Hata:** Kendi 2. Ayını başkasının 24. Ayıyla karşılaştırmak. Twitter'daki "SaaS'ımdan $15K/ay kazanıyorum" paylaşımları, önceki 18 aylık $0-$200'ü asla söylemez. Her akışın bir rampa dönemi var. Planla. Bütçele. İlk iki ay hiçe benzediği için çalışan bir stratejiyi terk etme.

### Senin Sıran

**Alıştırma 1.1:** Mevcut gelir kaynaklarını yaz. Her biri için beş kategoriden hangisine girdiğini belirle. Sadece tek bir kaynağın varsa (maaşın), onu da yaz. Kırılganlığı kabul et.

**Alıştırma 1.2:** Zihniyetini seç — Maaş Takviyesi veya Maaş Değiştirme. Nedenini yaz ve diğerine geçmeden önce neyin doğru olması gerektiğini belirt.

**Alıştırma 1.3:** Mevcut durumuna en uygun üç portföy profilinden birini seç (Önce Güvenlik, Büyüme Modu, Bağımsızlığa Geçiş). Akış kategorileri arasında hedefleyeceğin yüzdelik dağılımı yaz.

**Alıştırma 1.4:** Gelir projeleri için haftalık kullanılabilir saatlerini hesapla. Dürüst ol. Uyku, ana iş, aile, egzersiz ve en az 5 saatlik "hayat olur" tamponunu çıkar. O sayı senin gerçek sermayendir.

---

## Ders 2: Akışlar Nasıl Etkileşir (Volan Etkisi)

*"Akışlar sadece toplanmaz — çarpılır. Etkileşim için tasarla, bağımsızlık için değil."*

### Volan Kavramı

Volan, dönme enerjisi depolayan mekanik bir cihazdır. Döndürmeye başlamak zordur, ama hareket ettikten sonra her itme momentum ekler. Momentum arttıkça, sonraki her itme daha az çaba gerektirir.

Gelir akışların da aynı şekilde çalışır — etkileşim için tasarlarsan. İzole olarak var olan bir akış sadece bir yan projedir. Diğer akışları besleyen bir akış, bir volan bileşenidir.

$5K/ay ile $20K/ay arasındaki fark neredeyse hiçbir zaman "daha fazla akış" değildir. Daha iyi bağlı akışlardır.

### Bağlantı 1: Danışmanlık Ürün Fikirlerini Besler

Her danışmanlık görevi bir pazar araştırmasıdır. Bir şirketin sorunlarının içinde oturmak için sana ödeme yapılıyor. Seni işe alan müşteriler sana — parayla — hangi sorunların var olduğunu ve hangi çözümlere ödeme yapacaklarını söylüyor.

**Çıkarma süreci:**

Her danışmanlık görevi 2-3 ürün fikri üretmelidir. Belirsiz "keşke olsa" fikirleri değil. Somut, valide edilmiş fikirler:

- **Bu müşteri için hangi tekrarlayan görevi yaptın?** Onlar için yaptıysan, başka şirketlerin de buna ihtiyacı var. Otomatik olarak yapan bir araç inşa et.
- **Müşteri hangi aracın var olmasını istedi?** Görüşme sırasında söylediler. "Keşke ... için bir araç olsa" dediler, sen başını salladın ve devam ettin. Devam etmeyi bırak. Yaz.
- **Görevi kolaylaştırmak için içeride ne inşa ettin?** O iç araç bir üründür. Zaten kendin kullanarak valide ettin.

**"Üç Kuralı":** Üç farklı müşteri aynı şeyi istiyorsa, ondan bir ürün yap. Üç tesadüf değildir. Üç bir pazar sinyalidir.

**Şu senaryoyu düşün:** Üç farklı fintech şirketine danışmanlık yapıyorsun, her birinin banka ekstresi PDF'lerini yapılandırılmış veriye dönüştürmesi gerekiyor. Her seferinde hızlı bir script yazıyorsun. Üçüncü görüşmeden sonra scripti barındırılan bir API servisine dönüştürüyorsun. Bir yıl içinde $25-30/ay'dan 100-200 müşterisi oluyor. Hâlâ danışmanlık yapıyorsun, ama sadece önce API müşterisi olan şirketler için.

Bu kalıbın gerçek dünya örneği: Bannerbear (Jon Yongfook) otomasyon danışmanlığı olarak başladı, tekrarlayan müşteri işini ürünleştirerek $50K+ MRR API ürününe evrildi (kaynak: indiepattern.com).

### Bağlantı 2: İçerik Danışmanlık Müşterisi Getirir

Yazan geliştirici, müşterisi asla bitmeyen geliştiricidir.

Ayda bir derin teknik blog yazısı — çözdüğün gerçek bir sorun hakkında 1,500-2,500 kelime — danışmanlık pipeline'ın için herhangi bir soğuk erişim veya LinkedIn networking'inden daha fazlasını yapar.

**Pipeline nasıl çalışır:**

```
X Sorununu çözmek hakkında bir yazı yazarsın
    -> Y Şirketindeki geliştirici X Sorununa sahip
    -> Google'da arar
    -> Yazını bulur
    -> Yazın gerçekten yardım eder (çünkü işi yapmışsın)
    -> Siteni kontrol eder: "Aa, danışmanlık yapıyormuş"
    -> Gelen müşteri adayı. Sunum yok. Soğuk e-posta yok. Sana geldiler.
```

Bu bileşik büyür. 1. Yazı belki sıfır müşteri adayı üretir. 12. Yazı tutarlı aylık gelen trafik üretir. 24. Yazı alabileceğinden fazla müşteri adayı üretir.

**"Satış ekibi olarak içerik" modeli:**

Geleneksel bir danışmanlık işi iş geliştirme personeli işe alır. Sen blog yazıları işe alırsın. Blog yazıları sağlık sigortasına ihtiyaç duymaz, asla tatile çıkmaz ve her saat diliminde 7/24 çalışır.

**Gerçek örnek:** Bir Rust geliştiricisi ayda iki yazı yazıyor, performans optimizasyonu hakkında. Gösterişli bir şey değil — sadece işte çözdüğü gerçek sorunlar (anonimleştirilmiş, tescilli ayrıntı yok). 8 ay sonra, ayda 3-5 gelen müşteri adayı alıyor. 2-3'ünü alıyor. Danışmanlık ücreti artık $275/saat çünkü talep arzı aşıyor. Blog yazmasına ayda 8 saat maloluyor. Bu 8 saat, ayda $15K danışmanlık geliri üretiyor.

Matematik: 8 saat yazma → $15,000 gelir. Bu yazma saati başına $1,875, tüm işindeki en yüksek ROI'li aktivite.

### Bağlantı 3: Ürünler İçerik Yaratır

İnşa ettiğin her ürün, aktive edilmeyi bekleyen bir içerik motorudur.

**Lansman içeriği (ürün lansmanı başına 3-5 parça):**
1. "X'i neden inşa ettim" — sorun ve çözümün (blog yazısı)
2. "X kaputun altında nasıl çalışıyor" — teknik mimari (blog yazısı veya video)
3. "X'i inşa etmek: ne öğrendim" — dersler ve hatalar (Twitter dizisi + blog)
4. Lansman duyurusu (bülten, Product Hunt, HN Show)
5. Eğitim: "X ile başlamak" (belgelendirme + video)

**Sürekli içerik (kalıcı):**
- Özellik güncelleme yazıları ("V1.2: Neler yeni ve neden")
- Vaka çalışmaları ("Y Şirketi Z yapmak için X'i nasıl kullanıyor")
- Karşılaştırma yazıları ("X vs. Alternatif A: dürüst bir bakış")
- Entegrasyon kılavuzları ("X'i [popüler araç] ile kullanmak")

**İçerik olarak open source:**
Ürününün bir open source bileşeni varsa, her pull request, her sürüm, her mimari karar potansiyel içeriktir. "X'te önbelleğe almayı nasıl yönetiyoruz" aynı anda mühendislik belgeleri, sosyal kanıt, pazarlama içeriği ve topluluk oluşturmadır.

### Bağlantı 4: Otomasyon Her Şeyi Destekler

Otomasyon yoluyla kazandığın her saat, diğer akışları büyütmeye yatırabileceğin bir saattir.

**Her akışın tekrarlayan kısımlarını otomatikleştir:**

- **Danışmanlık:** Faturalamayı, zaman takibini, sözleşme oluşturmayı, toplantı planlamayı otomatikleştir. Ayda 3-5 saat tasarruf.
- **Ürünler:** Hoşgeldin e-postalarını, metrik panolarını, uyarı izlemesini, changelog oluşturmayı otomatikleştir. Ayda 5-10 saat tasarruf.
- **İçerik:** Sosyal medya dağıtımını, bülten formatlamayı, analitik raporlamayı otomatikleştir. Ayda 4-6 saat tasarruf.

**Otomasyonun bileşik etkisi:**

```
Ay 1:  Faturalamayı otomatikleştiriyorsun.           Ayda 2 saat tasarruf.
Ay 3:  İçerik dağıtımını otomatikleştiriyorsun.      Ayda 4 saat tasarruf.
Ay 6:  Ürün izlemeyi otomatikleştiriyorsun.          Ayda 5 saat tasarruf.
Ay 9:  Müşteri katılımını otomatikleştiriyorsun.     Ayda 3 saat tasarruf.
Ay 12: Toplam otomasyon tasarrufu: ayda 14 saat.

14 saat/ay = yılda 168 saat = 4 tam iş haftasından fazla.
O 4 hafta bir sonraki akışı inşa etmeye gider.
```

### Bağlantı 5: İstihbarat Her Şeyi Birbirine Bağlar

Burası sistemin parçalarının toplamından büyük hale geldiği yerdir.

{? if settings.has_llm ?}
> **LLM'in ({= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("modeliniz") =}) bu bağlantıyı güçlendirir.** Sinyal algılama, içerik özetleme, müşteri adayı yeterlilik değerlendirmesi ve fırsat sınıflandırma — LLM'in ham bilgiyi tüm akışlarda eşzamanlı olarak eyleme dönüştürülebilir istihbarata dönüştürür.
{? endif ?}

Yükselen bir framework hakkında bir sinyal sadece bir haber maddesi değildir. Volan üzerinden izlendiğinde şöyle olur:

- Bir **danışmanlık fırsatı** ("Framework X'i benimsemek için yardıma ihtiyacımız var")
- Bir **ürün fikri** ("Framework X kullanıcılarının Y için bir araca ihtiyacı var")
- Bir **içerik konusu** ("Framework X ile başlamak: dürüst kılavuz")
- Bir **otomasyon fırsatı** ("Framework X sürümlerini izle ve otomatik geçiş kılavuzları üret")

İstihbaratı olmayan geliştirici haber görür. İstihbaratı olan geliştirici tüm akışlarda bağlantılı fırsatlar görür.

### Tam Volan

Tamamen bağlı bir akış yığını şöyle görünür:

```
                    +------------------+
                    |                  |
            +------>|   DANIŞMANLIK    |-------+
            |       |  (Hızlı Nakit)   |       |
            |       +------------------+       |
            |              |                   |
            |    müşteri sorunları =            |
            |    ürün fikirleri                |
            |              |                   |
            |              v                   |
   içerikten|       +------------------+       |    vaka çalışmaları
   gelen    |       |                  |       |    ve lansman
   adaylar  +-------|     ÜRÜNLER      |-------+    hikayeleri
            |       | (Büyüyen Varlık) |       |
            |       +------------------+       |
            |              |                   |
            |    ürün lansmanları =             |
            |    içerik parçaları              |
            |              |                   |
            |              v                   v
            |       +------------------+  +------------------+
            |       |                  |  |                  |
            +-------|     İÇERİK       |  |    OTOMASYON     |
                    |  (Bileşik)       |  | (Pasif Gelir)    |
                    +------------------+  +------------------+
                           |                      |
                    kitle oluşturur          diğer tüm akışlar
                    otorite +               için zaman kazandırır
                    güven                         |
                           |                      |
                           v                      v
                    +----------------------------------+
                    |          İSTİHBARAT                |
                    |    (4DA / Sinyal Algılama)         |
                    |  Tüm akışlarda fırsatları          |
                    |        ortaya çıkarır              |
                    +----------------------------------+
```

**Volan hareket halinde — gerçek bir hafta:**

Pazartesi: 4DA brifingn bir sinyal ortaya çıkarır — büyük bir şirket dahili belge işleme pipeline'ını open source yaptı ve geliştiriciler eksik özelliklerden şikayet ediyor.

Salı: Bir blog yazısı yazıyorsun: "[Şirketin] Belge Pipeline'ının Yanlış Yaptığı Şey (Ve Nasıl Düzeltilir)" — belge işleme konusundaki gerçek danışmanlık deneyimine dayalı.

Çarşamba: Yazı HN'de ilgi görüyor. İki CTO, belge işleme altyapısında danışmanlık yapıp yapmadığını soruyor.

Perşembe: Bir danışmanlık görüşmesi yapıyorsun. Görüşme sırasında CTO, verileri harici sunuculara göndermeyen barındırılan bir belge işleme API'sine ihtiyaçları olduğundan bahsediyor.

Cuma: Ürün yol haritana "gizlilik öncelikli belge işleme API'si" ekliyorsun. Mevcut otomasyon sistemin gerekli işlevselliğin yarısını zaten karşılıyor.

O hafta, tek bir istihbarat sinyali üretti: bir blog yazısı (içerik), iki danışmanlık adayı (hızlı nakit) ve valide edilmiş bir ürün fikri (büyüyen varlık). Her akış diğerlerini besledi. İşte bu volandır.

### Bağlantılarını Tasarlamak

Her akış diğer her akışa bağlanmaz. Sorun değil. Volanın çalışması için en az üç güçlü bağlantıya ihtiyacın var.

**Bağlantılarını haritala:**

Yığınındaki her akış için cevapla:
1. Bu akış diğer akışların kullanabileceği ne **üretir**? (müşteri adayları, içerik, veri, fikirler, kod)
2. Bu akış diğer akışlardan ne **tüketir**? (trafik, güvenilirlik, gelir, zaman)
3. Bu akış ile herhangi bir diğeri arasındaki **en güçlü bağlantı** nedir?

Bir akışın diğer akışlarınla sıfır bağlantısı varsa, volanın parçası değildir. Bağlantısız bir yan projedir. Bu onu öldür demek değil — ya bağlantıyı bul ya da bağımsız olduğunu kabul et ve buna göre yönet.

> **Yaygın Hata:** Maksimum etkileşim yerine maksimum gelir için akış tasarlamak. Ayda {= regional.currency_symbol | fallback("$") =}800 üreten VE iki diğer akışı besleyen bir akış, izolasyonda ayda {= regional.currency_symbol | fallback("$") =}2,000 üreten bir akıştan daha değerlidir. İzole akış {= regional.currency_symbol | fallback("$") =}2,000 ekler. Bağlı akış {= regional.currency_symbol | fallback("$") =}800 artı tüm portföyde büyüme ivmesi ekler. 12 ay boyunca bağlı akış her zaman kazanır.

{? if dna.is_full ?}

{@ mirror blind_spot_moat @}

{? endif ?}

### Senin Sıran

**Alıştırma 2.1:** Kendi volanını çiz. Bugün sadece 1-2 akışın olsa bile, inşa etmek istediğin bağlantıları çiz. En az 3 akış dahil et ve aralarında en az 3 bağlantı belirle.

**Alıştırma 2.2:** Mevcut veya planlanan danışmanlık/hizmet işin için, müşteri konuşmalarından çıkan (veya çıkabilecek) üç ürün fikri listele. Üç Kuralını uygula — bunlardan herhangi biri birden fazla müşteride gündeme geldi mi?

**Alıştırma 2.3:** İşte veya kişisel bir projede çözdüğün son 3 teknik sorunu yaz. Her biri için bir blog yazısı başlığı taslağı hazırla. Bunlar ilk içerik parçaların — zaten çözdüğün sorunlar, aynı şeyle karşılaşacak başkaları için yazılmış.

**Alıştırma 2.4:** Herhangi bir akışında tekrar tekrar yaptığın ve bu hafta otomatikleştirilebilecek bir görevi belirle. Gelecek ay değil. Bu hafta. Otomatikleştir.

---

## Ders 3: $10K/Ay Kilometre Taşı

*"$10K/ay bir hayal değil. Bir matematik problemi. İşte dört çözüm yolu."*

### Neden {= regional.currency_symbol | fallback("$") =}10K/Ay

Ayda on bin {= regional.currency | fallback("dolar") =}, her şeyin değiştiği rakamdır. Keyfi değildir.

- **{= regional.currency_symbol | fallback("$") =}10K/ay = {= regional.currency_symbol | fallback("$") =}120K/yıl.** Bu, ABD'deki medyan yazılım geliştiricisi maaşına eşit veya onu aşar.
- **{= regional.currency_symbol | fallback("$") =}10K/ay vergi sonrası (~{= regional.currency_symbol | fallback("$") =}7K net) çoğu ABD şehrinde orta sınıf bir yaşamı** ve dünyanın neredeyse her yerinde rahat bir yaşamı karşılar.
- **Birden fazla akıştan {= regional.currency_symbol | fallback("$") =}10K/ay**, tek bir işverenden {= regional.currency_symbol | fallback("$") =}15K/ay'dan daha istikrarlıdır, çünkü tek bir başarısızlık seni {= regional.currency_symbol | fallback("$") =}10K'dan {= regional.currency_symbol | fallback("$") =}0'a düşüremez.
- **{= regional.currency_symbol | fallback("$") =}10K/ay modeli kanıtlar.** Bağımsız olarak {= regional.currency_symbol | fallback("$") =}10K/ay kazanabiliyorsan, {= regional.currency_symbol | fallback("$") =}20K/ay kazanabilirsin. Sistem çalışıyor. Bundan sonraki her şey optimizasyondur.

{= regional.currency_symbol | fallback("$") =}10K/ay'ın altında takviye yapıyorsun. {= regional.currency_symbol | fallback("$") =}10K/ay'da bağımsızsın. İşte bu yüzden önemli.

İşte dört somut yol. Her biri gerçekçi, spesifik ve tutarlı uygulamanın 12-18 ayı içinde ulaşılabilirdir.

### Yol 1: Danışmanlık Ağırlıklı

**Profil:** Yetenekli, deneyimli ve zamanını premium ücretlerle satmakta rahatsın. Şimdi istikrar ve yüksek gelir istiyorsun, ürünler arka planda büyürken.

| Akış | Matematik | Aylık |
|--------|------|---------|
| Danışmanlık | 10 saat/hafta x $200/saat | $8,000 |
| Ürünler | 50 müşteri x $15/ay | $750 |
| İçerik | Bülten affiliate geliri | $500 |
| Otomasyon | API ürünü | $750 |
| **Toplam** | | **$10,000** |

**Zaman yatırımı:** 15-20 saat/hafta
- Danışmanlık: 10 saat (müşteri işi)
- Ürün: 3-4 saat (bakım + küçük özellikler)
- İçerik: 2-3 saat (haftada bir yazı veya bülten)
- Otomasyon: 1-2 saat (izleme, ara sıra düzeltmeler)

**Gerçekçi zaman çizelgesi:**
- Ay 1-2: İlk danışmanlık müşterisini bul. Referans oluşturmak için gerekirse $150/saat ile başla.
- Ay 3-4: Ücret $175/saat'e çıkar. İkinci müşteri. Danışmanlık içgörülerine dayanarak ürün inşa etmeye başla.
- Ay 5-6: Ücret $200/saat. Ürün 10-20 ücretsiz kullanıcıyla betada. Bülten başlatıldı.
- Ay 7-9: Ürün $15/ay, 20-30 ödeme yapan müşteri. Bülten büyüyor. İlk affiliate geliri.
- Ay 10-12: Ürün 50 müşteride. API ürünü lansmanı (danışmanlık otomasyonundan inşa edildi). Danışmanlık tam ücrette.

**Gereken beceriler:** Bir alanda derin uzmanlık ("React biliyorum" değil — daha çok "ölçekte e-ticaret için React performans optimizasyonu biliyorum" gibi). İletişim becerileri. Teklif yazma yeteneği.

**Risk seviyesi:** Düşük. Danışmanlık geliri hemen ve öngörülebilir. Ürünler ve içerik arka planda büyür.

**Ölçekleme potansiyeli:** Orta. Danışmanlık tavana çarpar (senin saatlerin), ama ürünler ve içerik zamanla o tavanın ötesinde büyüyebilir. 18-24 ayda oranı %80 danışmanlıktan %40 danışmanlık + %60 ürünlere kaydırabilirsin.

### Yol 2: Ürün Ağırlıklı

**Profil:** Bir şeyler inşa etmek ve satmak istiyorsun. Ölçeklenebilir, zamandan bağımsız gelir karşılığında daha yavaş başlangıç gelirini kabul etmeye hazırsın.

| Akış | Matematik | Aylık |
|--------|------|---------|
| SaaS | 200 müşteri x $19/ay | $3,800 |
| Dijital ürünler | 100 satış/ay x $29 | $2,900 |
| İçerik | YouTube + bülten | $2,000 |
| Danışmanlık | 3 saat/hafta x $250/saat | $3,000 |
| **Toplam** | | **$11,700** |

**Zaman yatırımı:** 20-25 saat/hafta
- SaaS: 8-10 saat (geliştirme, destek, pazarlama)
- Dijital ürünler: 3-4 saat (güncellemeler, yeni ürünler, pazarlama)
- İçerik: 5-6 saat (haftada 1 video + 1 bülten)
- Danışmanlık: 3-4 saat (müşteri işi + yönetim)

**Gerçekçi zaman çizelgesi:**
- Ay 1-3: SaaS MVP inşa et. 1. dijital ürünü yayınla (şablon, araç seti veya kılavuz). İnşa aşamasını finanse etmek için danışmanlık başlat.
- Ay 4-6: SaaS 30-50 müşteride. Dijital ürün $500-1,000/ay üretiyor. İçerik kütüphanesi büyüyor.
- Ay 7-9: SaaS 80-120 müşteride. 2. dijital ürün lansmanı. YouTube bileşik büyümeye başlıyor.
- Ay 10-12: SaaS 200 müşteriye yaklaşıyor. Dijital ürünler toplam $2K-3K/ay. İçerik geliri gerçek.

**Gereken beceriler:** Full-stack geliştirme. Ürün sezgisi (ne inşa edileceğini bilmek). Temel pazarlama (landing sayfaları, metin yazarlığı). İlk 6 ay belirsizlikle rahat olma.

**Risk seviyesi:** Orta. Gelir yavaş başlar. Ya birikim ya da danışmanlık geliri ile arayı kapatman gerekir.

**Ölçekleme potansiyeli:** Yüksek. $11K/ay'da bükülme noktasındasın. 400 SaaS müşterisi = sadece SaaS'tan $7,600/ay. İçerik kitlesi bileşik büyür. Ürünler büyürse danışmanlığı tamamen bırakabilirsin.

> **Açık Konuşalım:** $19/ay'dan 200 SaaS müşterisi kağıt üzerinde basit görünür. Gerçekte 200 ödeme yapan müşteriye ulaşmak amansız uygulama gerektirir — gerçekten faydalı bir şey inşa etmek, doğru pazarı bulmak, geri bildirime göre iterasyon yapmak ve 12+ ay tutarlı pazarlama. Kesinlikle ulaşılabilir. Kolay değil. Aksini söyleyen biri sana bir şey satıyor.

### Yol 3: İçerik Ağırlıklı

**Profil:** İyi bir iletişimcisin — yazılı, sözlü veya her ikisi. Öğretmeyi ve açıklamayı seviyorsun. Zamanla azalan çaba gerektiren bileşik getiriler karşılığında 12 ay boyunca kitle oluşturmaya hazırsın.

| Akış | Matematik | Aylık |
|--------|------|---------|
| YouTube | 50K abone, reklam + sponsorlar | $3,000 |
| Bülten | 10K abone, %5 ücretli x $8/ay | $4,000 |
| Kurs | 30 satış/ay x $99 | $2,970 |
| Danışmanlık | 2 saat/hafta x $300/saat | $2,400 |
| **Toplam** | | **$12,370** |

**Zaman yatırımı:** 15-20 saat/hafta
- YouTube: 6-8 saat (senaryo, kayıt, kurgu — veya editör tut)
- Bülten: 3-4 saat (yazma, küratörlük, dağıtım)
- Kurs: 2-3 saat (öğrenci desteği, periyodik güncellemeler, pazarlama)
- Danışmanlık: 2-3 saat (kitle güvenilirlik sağladığı için premium ücret)

**Gerçekçi zaman çizelgesi:**
- Ay 1-3: YouTube kanalı ve bülteni başlat. Tutarlı yayınla — haftada 1 video, haftada 1 bülten. Gelir: $0. Bu çalışma aşaması. Acil gelir için $200/saat ile danışmanlık başlat.
- Ay 4-6: 5K YouTube abonesi, 2K bülten abonesi. İlk sponsorluk anlaşması ($500-1,000). Bülten ücretli katmanı: 50-100 abone. Danışmanlık ücreti $250/saat'e çıktı.
- Ay 7-9: 15K YouTube abonesi, 5K bülten abonesi. YouTube reklam geliri başlıyor ($500-1,000/ay). Bülten ücretli katmanı $1,500-2,000/ay. Kurs inşa etmeye başla.
- Ay 10-12: 30-50K YouTube abonesi, 8-10K bülten abonesi. Kurs $99'dan lansmanı yapıldı. Danışmanlık ücreti $300/saat, çünkü kitleden gelen talep.

**Gereken beceriler:** Yazma veya konuşma yeteneği. Tutarlılık (gerçek beceri budur — ilk 3 ay kimse izlemezken 12 ay boyunca her hafta yayınlamak). Öğretmeye değer alan uzmanlığı. Temel video kurgusu veya editör tutacak bütçe ($200-400/ay).

**Risk seviyesi:** Orta. Yavaş monetizasyon. Platform bağımlılığı (YouTube, Substack). Ama kitle, inşa edebileceğin en dayanıklı varlıktır — platformlar arasında taşınır.

**Ölçekleme potansiyeli:** Çok yüksek. 50K YouTube kitlesi, gelecekte inşa edeceğin her şey için bir lansman platformudur. Kurs geliri bileşik büyür (bir kez inşa et, sonsuza kadar sat). Bülten, aranda algoritma olmadan kitlene doğrudan erişimdir.

**$300/saat danışmanlık ücreti:** Bu yoldaki danışmanlık ücretinin $200/saat değil $300/saat olduğuna dikkat et. Bunun nedeni içerik kitlesinin güvenilirlik ve gelen talep yaratmasıdır. Bir CTO 20 videonu izleyip bültenini okuduysa, ücretini pazarlık etmez. Müsait olup olmadığını sorar.

### Yol 4: Otomasyon Ağırlıklı

**Profil:** Çabadan çok kaldıracı değer veren bir sistem düşünürüsün. Minimum süregelen zaman yatırımıyla gelir üreten makineler inşa etmek istiyorsun.

| Akış | Matematik | Aylık |
|--------|------|---------|
| Veri ürünleri | 200 abone x $15/ay | $3,000 |
| API servisleri | 100 müşteri x $29/ay | $2,900 |
| Hizmet olarak Otomasyon | 2 müşteri x $1,500/ay sabit ücret | $3,000 |
| Dijital ürünler | Pasif satışlar | $1,500 |
| **Toplam** | | **$10,400** |

**Zaman yatırımı:** 10-15 saat/hafta (kararlı durumda dört yolun en düşüğü)
- Veri ürünleri: 2-3 saat (izleme, veri kalite kontrolleri, ara sıra güncellemeler)
- API servisleri: 2-3 saat (izleme, hata düzeltmeleri, müşteri desteği)
- Otomasyon müşterileri: 3-4 saat (izleme, optimizasyon, aylık değerlendirmeler)
- Dijital ürünler: 1-2 saat (müşteri desteği, ara sıra güncellemeler)

**Gerçekçi zaman çizelgesi:**
- Ay 1-3: İlk veri ürününü veya API servisini inşa et. Networking veya soğuk erişim yoluyla ilk 2 sabit ücretli otomasyon müşterisini bul. Gelir: $2,000-3,000/ay (çoğunlukla sabit ücretler).
- Ay 4-6: Veri ürünü 50-80 abonede. API 20-40 müşteride. İlk dijital ürün lansmanı. Gelir: $4,000-6,000/ay.
- Ay 7-9: Organik büyüme ve içerik pazarlama ile veri ürünlerini ve API'yi ölçeklendir. Gelir: $6,000-8,000/ay.
- Ay 10-12: Tam portföy çalışıyor. Çoğu akış sadece izleme gerektiriyor. Gelir: $9,000-11,000/ay.

**Gereken beceriler:** Backend/sistem geliştirme. API tasarımı. Veri mühendisliği. Belirli bir nişi anlama (veri ve otomasyon gerçek bir kitle için gerçek bir ihtiyaca hizmet etmeli).

**Risk seviyesi:** Orta-Düşük. Dört akış arasında çeşitlendirilmiş. Hiçbir akış gelirin %30'unu aşmaz. Sabit ücretli otomasyon müşterileri istikrar sağlar.

**Ölçekleme potansiyeli:** Orta-Yüksek. Zaman verimliliği anahtar avantajdır. Haftada 10-15 saatle akış ekleme, içerik kanalı başlatma veya premium ücretlerle ara sıra danışmanlık alma kapasiten var. Zaman özgürlüğünün kendisi ekonomik değere sahiptir.

> **Yaygın Hata:** Yol 4'e bakıp "Sadece dört otomasyon ürünü yapacağım" diye düşünmek. Otomasyon ağırlıklı yol, insanların hangi veri veya API servisi için ödeme yapacağını belirlemek için derin alan bilgisi gerektirir. Burada listelenen veri ürünleri ve API'ler jenerik değildir — belirli kitleler için belirli sorunları çözerler. Bu sorunları bulmak ya danışmanlık deneyimi (Yol 1) ya da içerik odaklı pazar araştırması (Yol 3) gerektirir. Yol 4'te başarılı olan geliştiricilerin çoğu önce Yol 1 veya 3'te 6-12 ay geçirdi.

### Yolunu Seçmek

Tam olarak bir yol seçmek zorunda değilsin. Bunlar arketipler, reçeteler değil. Çoğu geliştirici bir hibridle sonuçlanır. Ama hangi arketipe eğildiğini anlamak, dağılım kararları almana yardımcı olur.

**Karar çerçevesi:**

| Eğer... | O zaman eğil... |
|-----------|-------------------|
| Güçlü bir profesyonel ağın varsa | Yol 1 (Danışmanlık Ağırlıklı) |
| Ürün inşa etmeyi seviyorsan ve yavaş başlangıca tahammül edebilirsen | Yol 2 (Ürün Ağırlıklı) |
| İyi bir iletişimciysen ve öğretmeyi seviyorsan | Yol 3 (İçerik Ağırlıklı) |
| Zaman özgürlüğünü değer veren bir sistem düşünürüysen | Yol 4 (Otomasyon Ağırlıklı) |
| Hızlı para gerekiyorsa | Önce Yol 1, sonra geçiş |
| 6+ aylık birikimlerin varsa | Yol 2 veya 3 (bileşiğe yatırım) |
| Haftada 10 saat veya daha azın varsa | Yol 4 (saat başına en yüksek kaldıraç) |

{? if stack.primary ?}
> **Yığınına dayalı olarak ({= stack.primary | fallback("birincil yığının") =}):** Mevcut becerilerini en iyi hangi yolun kaldırdığını düşün. Backend/sistem deneyimi olan geliştiriciler genellikle Yol 4'te (Otomasyon Ağırlıklı) başarılı olur. Frontend ve full-stack geliştiriciler genellikle Yol 2'yi (Ürün Ağırlıklı) çekişe en hızlı bulur. Derin alan bilgisine sahip güçlü iletişimciler Yol 3'te (İçerik Ağırlıklı) başarılı olur.
{? endif ?}

{? if computed.experience_years < 3 ?}
> **3 yıldan az deneyime sahip geliştiriciler için:** Yol 2 (Ürün Ağırlıklı) veya Yol 3 (İçerik Ağırlıklı) en iyi başlangıç noktalarındır. Yüksek ücretli danışmanlık için henüz ağın olmayabilir ve bu sorun değil. Ürünler ve içerik, gelir üretirken itibarını inşa eder. Dijital ürünlerle başla (şablonlar, başlangıç kitleri, kılavuzlar) — en az başlangıç güvenilirliği gerektirir ve en hızlı pazar geri bildirimini verir.
{? elif computed.experience_years < 8 ?}
> **3-8 yıl deneyime sahip geliştiriciler için:** Yol 1 (Danışmanlık Ağırlıklı) hızlı nakit motoru olarak mükemmel konumdasın, yan tarafta ürünler inşa ederken. Deneyimin $150-250/saat ücret almak için yeterince derin ama Yol 3'te premium ücretler için henüz itibarın olmayabilir. Ürün geliştirmeyi finanse etmek için danışmanlığı kullan, sonra ürünler büyüdükçe oranı kademeli olarak kaydır.
{? else ?}
> **Kıdemli geliştiriciler (8+ yıl) için:** Dört yolun hepsi sana açık, ama Yol 3 (İçerik Ağırlıklı) ve Yol 4 (Otomasyon Ağırlıklı) en yüksek uzun vadeli kaldıracı sunar. Deneyimin, ödemeye değer fikirler (içerik), otomatikleştirmeye değer kalıplar (veri ürünleri) ve satış sürtünmesini azaltan güvenilirlik (danışmanlık $300+/saat'te) verir. Anahtar karar: itibarınla mı (danışmanlık/içerik) yoksa sistem düşüncenle mi (ürünler/otomasyon) ticaret yapmak istiyorsun?
{? endif ?}

{? if stack.contains("react") ?}
> **React yığını önerisi:** En yaygın başarılı React geliştiricisi gelir portföyü, bir UI bileşen kütüphanesi veya şablon seti (Ürün) ile teknik içerik (Blog/YouTube) ve ara sıra danışmanlığı birleştirir. React ekosistemi yeniden kullanılabilir, iyi belgelenmiş bileşenler yayınlayan geliştiricileri ödüllendirir.
{? endif ?}
{? if stack.contains("python") ?}
> **Python yığını önerisi:** Python geliştiricileri genellikle en yüksek ROI'yi otomasyon servisleri ve veri ürünlerinde bulur. Dilinin veri işleme, ML ve scripting'deki gücü doğrudan Yol 4'e (Otomasyon Ağırlıklı) çevrilir. Veri pipeline danışmanlığı özellikle kârlıdır — şirketlerin işleyebildiklerinden fazla verisi var.
{? endif ?}
{? if stack.contains("rust") ?}
> **Rust yığını önerisi:** Rust yetenek pazarı ciddi şekilde arz kısıtlıdır. Premium ücretlerle ($250-400/saat) Yol 1 (Danışmanlık Ağırlıklı) production Rust deneyimini gösterebilirsen hemen uygulanabilir. Uzun vadeli bileşik için Yol 2 (Open Source + Premium) ile eşleştir — iyi bakılan Rust crate'leri danışmanlık talebini besleyen itibar inşa eder.
{? endif ?}

{@ temporal market_timing @}

### Senin Sıran

**Alıştırma 3.1:** Durumuna en uygun yolu seç. Nedenini yaz. Kısıtlamaların konusunda dürüst ol — zaman, birikim, beceriler, risk toleransı.

**Alıştırma 3.2:** Yolun için matematiği özelleştir. Genel sayıları kendi gerçek ücretlerin, fiyat noktaların ve gerçekçi müşteri sayılarınla değiştir. SENİN $10K/ay versiyonun nasıl görünüyor?

**Alıştırma 3.3:** Seçtiğin yoldaki en büyük riski belirle. En olası yanlış gidecek şey ne? Acil durum planını yaz. (Örnek: "SaaS'ım 9. aya kadar 100 müşteriye ulaşamazsa, danışmanlığı haftada 15 saate çıkarır ve onu 6 ay daha ürün geliştirmeyi finanse etmek için kullanırım.")

**Alıştırma 3.4:** "Köprü rakamını" hesapla — daha yavaş akışlar ivme kazanırken kendini sürdürmek için ihtiyacın olan birikim veya hızlı nakit geliri miktarı. Hızlı Nakit geliri bu boşluğu doldurur. Minimum giderlerini karşılamak için haftada kaç saat danışmanlık gerekiyor?

---

## Ders 4: Bir Akışı Ne Zaman Sonlandırmalı

*"İş dünyasındaki en zor beceri ne zaman bırakacağını bilmektir. İkinci en zoru gerçekten yapmaktır."*

### Sonlandırma Sorunu

Geliştiriciler inşaatçıdır. Bir şeyler yaratırız. İnşa ettiğimiz bir şeyi öldürmek her içgüdümüze aykırıdır. Şöyle düşünürüz: "Bir özellik daha gerekiyor." "Pazar yetişecek." "Durmak için çok fazla yatırım yaptım."

Sonuncunun bir adı var: batık maliyet yanılgısı. Ve kötü kod, kötü pazarlama ve kötü fikirlerden toplu olarak daha fazla geliştirici yan işini öldürmüştür.

Her akış hayatta kalmaz. Sürdürülebilir gelir inşa eden geliştiriciler, asla başarısız olmayanlar değildir — hızlı başarısız olan, kararlılıkla öldüren ve serbest kalan zamanı gerçekten işe yarayan şeylere yeniden yatıran geliştiricilerdir.

### Dört Sonlandırma Kuralı

#### Kural 1: $100 Kuralı

**Bir akış 6 aylık tutarlı çabadan sonra ayda $100'den az üretiyorsa, öldür veya dramatik şekilde pivot yap.**

6 ay sonra $100/ay demek pazar sana bir şey söylüyor. Belki ürün yanlış. Belki pazar yanlış. Belki uygulama yanlış. Ama $100/ay için 6 aylık çaba, artımlı iyileştirmenin düzeltmeyeceğine dair net bir sinyaldir.

"Tutarlı çaba" anahtar ifadedir. Bir ürün yayınladıysan ve sonra 5 ay dokunmadıysan, 6 ay test etmedin — 1 ay test ettin ve 5 ay ihmal ettin. Bu sinyal değil. Bu terktir.

**İstisnalar:**
- İçerik akışları (blog, YouTube, bülten) genellikle $100/aya ulaşmak için 9-12 ay alır. İçerik için $100 kuralı 6 değil 12 ayda geçerlidir.
- Hisse oyunları (open source) aylık gelirle ölçülmez. Topluluk büyümesi ve benimseme metrikleriyle ölçülür.

#### Kural 2: ROI Kuralı

**Zamanın üzerindeki ROI diğer akışlarınla karşılaştırıldığında negatifse, otomatikleştir veya öldür.**

Her akış için saatlik ROI'yi hesapla:

```
Saatlik ROI = Aylık Gelir / Aylık Yatırılan Saatler

Örnek portföy:
Akış A (Danışmanlık):    $5,000 / 40 saat = $125/saat
Akış B (SaaS):           $1,200 / 20 saat = $60/saat
Akış C (Bülten):         $300  / 12 saat  = $25/saat
Akış D (API ürünü):      $150  / 15 saat  = $10/saat
```

$10/saat ile Akış D bir sorun. İlk 6 ayında ve yukarı eğilimdeyse hariç, o aylık 15 saat Akış A'da ($1,875 ek gelir) veya Akış B'de ($900 ek gelir) daha iyi harcanır.

**Ama yörüngeyi düşün.** $10/saat kazanan ama aydan aya %30 büyüyen bir akış tutmaya değer. $25/saat kazanan ama 4 aydır düz bir akış otomasyon veya sonlandırma adayıdır.

#### Kural 3: Enerji Kuralı

**İşi yapmaktan nefret ediyorsan, akışı öldür — kârlı olsa bile.**

Bu sezgiye aykırıdır. Kârlı bir akışı neden öldürelim?

Çünkü tükenmişlik bireysel akışları hedef almaz. Tükenmişlik tüm kapasiteni hedef alır. Yapmaktan nefret ettiğin bir akış, diğer her şeyden enerji çeker. İşten korkmaya başlarsın. Erteleyesin gelir. Kalite düşer. Müşteriler fark eder. Sonra diğer akışlarından da nefret etmeye başlarsın, çünkü "SaaS'ım daha fazla para kazansaydı bu aptal bülteni yapmak zorunda kalmazdım."

Bu tükenmişlik kaskadıdır. Sadece nefret ettiğini değil, TÜM akışları öldürür.

**Test:** Bir akış üzerinde çalışmayı düşündüğünde karnında bir düğüm hissediyorsan, vücudun sana tablonun söylemeyeceği bir şey söylüyordur.

> **Açık Konuşalım:** Bu "sadece eğlenceli olanı yap" anlamına gelmiyor. Her akışın sıkıcı kısımları var. Müşteri desteği sıkıcıdır. Video kurgusu sıkıcıdır. Fatura kesmek sıkıcıdır. Enerji Kuralı sıkıcılıktan kaçınmakla ilgili değil — temel işin kendisiyle ilgili. Kod yazmak? Bazen sıkıcı, ama işçiliğin tadını çıkarıyorsun. Haftalık yatırım bankacılığı bültenleri yazmak çünkü iyi ödüyor ama finans sana dayanılmaz derecede sıkıcı geliyorsa? Bu bir enerji sızıntısıdır. Farkı bil.

#### Kural 4: Fırsat Maliyeti Kuralı

**Akış A'yı öldürmek Akış B'yi 3 katına çıkarmak için zaman açıyorsa, Akış A'yı öldür.**

Bu uygulanması en zor kuraldır çünkü gelecek hakkında bir bahis yapmayı gerektirir.

```
Mevcut durum:
Akış A: $500/ay, haftada 10 saat
Akış B: $2,000/ay, haftada 15 saat, aydan aya %20 büyüyor

Akış A'yı öldürüp o 10 saati Akış B'ye yatırırsan:
Akış B haftada 25 saatle makul şekilde 3 ayda $6,000/aya büyüyebilir

Potansiyel $4,000/ay kazanç için $500/ay'lık bir akışı öldürmek iyi bir bahistir.
```

Anahtar kelime "makul şekilde"dir. Akış B'nin daha fazla zaman absorbe edip gelire dönüştürebileceğine dair kanıta ihtiyacın var. Akış B zaman sınırlıysa (daha fazla saat = daha fazla çıktı = daha fazla gelir), bahis sağlamdır. Akış B pazar sınırlıysa (daha fazla saat benimseme hızını değiştirmez), bahis kötüdür.

### Bir Akışı Doğru Şekilde Öldürmek

Bir akışı öldürmek müşterilerin önünde kaybolmak anlamına gelmez. Bu itibarına zarar verir, ki bu tüm gelecek akışlarına zarar verir. Profesyonellikle öldür.

**Adım 1: Gün Batımı Duyurusu (kapatmadan 2-4 hafta önce)**

```
Konu: [Ürün Adı] — Önemli Güncelleme

Merhaba [Müşteri Adı],

[Ürün Adı]'nın [Tarih, en az 30 gün sonra] tarihinde
kapanacağını bildirmek için yazıyorum.

Geçen [X ay] boyunca, bu ürünü inşa ederken ve geri
bildiriminizden çok şey öğrendim. Daha fazla değer
sunabileceğim [diğer projeler/akışlar]a odaklanma
kararı aldım.

Sizin için bu şu anlama geliyor:
- Hizmetiniz [kapatma tarihine] kadar normal şekilde devam edecek
- [Geçerliyse] Verilerinizi [URL/yöntem] üzerinden dışa aktarabilirsiniz
- [Geçerliyse] Alternatif olarak [alternatif ürün] öneriyorum
- Kullanılmamış abonelik döneminiz için tam iade alacaksınız

Müşterimiz olduğunuz için teşekkür ederim. Desteğinizi gerçekten takdir ediyorum.

Saygılarımla,
[Adınız]
```

**Adım 2: Geçiş Planı**

- Tüm müşteri verilerini taşınabilir formatta dışa aktar
- Alternatifler öner (evet, rakipler bile — itibarın daha önemli)
- İadeleri proaktif olarak işle, müşterilerin istemesini bekleme

**Adım 3: Kurtarılabilir Olanı Kurtar**

Akışla birlikte her şey ölmez:

- **Kod:** Herhangi bir bileşen diğer ürünlerde yeniden kullanılabilir mi?
- **İçerik:** Blog yazıları, belgeler veya pazarlama metni yeniden amaçlanabilir mi?
- **İlişkiler:** Herhangi bir müşteri diğer akışlarının müşterisi olabilir mi?
- **Kitle:** E-posta aboneleri bültenine taşınabilir mi?
- **Bilgi:** Pazar, teknoloji veya kendin hakkında ne öğrendin?

**Adım 4: Ölüm Sonrası Analiz**

Kısa bir ölüm sonrası analiz yaz. Başkası için değil — kendin için. Üç soru:

1. **Ne işe yaradı?** (Başarısız akışlarda bile bir şey işe yaradı.)
2. **Ne işe yaramadı?** (Spesifik ol. "Pazarlama" spesifik değil. "%2'nin üzerinde dönüşüm sağlayan bir kanal bulamadım" spesifiktir.)
3. **Neyi farklı yapardım?** (Bu bir sonraki akışın için girdi olur.)

### Gerçek Örnekler

**Bültenini ($200/ay) öldürüp SaaS'a ($8K/ay) odaklanan geliştirici:**

Bültenin 1,200 abonesi vardı ve ücretli katman ve ara sıra sponsorluklar yoluyla $200/ay üretiyordu. Üretmek haftada 4-5 saat sürüyordu. SaaS aydan aya %15 büyüyordu ve geliştirme ve pazarlamaya yatırılan her saat gelir üzerinde görünür bir etki yapıyordu.

Matematik: haftada 4.5 saatle $200/ay = $11/saat. Aynı saatler SaaS'a yatırıldığında yaklaşık $150/saat artan gelir üretiyordu.

Bülteni öldürdü. Üç ay sonra SaaS $12K/ay'daydı. Bülteni özlemiyor.

**SaaS'ını ($500/ay, tonlarca destek) öldürüp danışmanlığa ($12K/ay) odaklanan geliştirici:**

SaaS'ın 80 kullanıcısı, $500/ay geliri ve haftada 15-20 destek bileti üretimi vardı. Her bilet 20-40 dakika sürüyordu. Geliştirici, $500/ay üreten bir ürüne haftada 10-15 saat harcıyordu.

Bu arada, $200/saat ile danışmanlık için bekleme listesi vardı. Kelimenin tam anlamıyla — müşteriler haftalarca bekliyordu.

SaaS'ı öldürdü, haftada 15 saati danışmanlığa taşıdı ve geliri $12,500/ay'dan $14,500/aya sıçradı. Artı, pazartesi sabahlarından korkmayı bıraktı.

**Danışmanlığı ($10K/ay) öldürüp tamamen ürünlere geçen geliştirici (şimdi $25K/ay):**

Bu cesaret gerektirir. Danışmanlıktan $10K/ay, haftada 20 saat kazanıyordu. Rahat. İstikrarlı. İki ürününe haftada 40 saat yatırım yapmak için tamamen öldürdü.

4 ay boyunca geliri $3K/aya düştü. Birikimlerini harcadı. Partneri gerginleşti.

5. ayda bir ürün bükülme noktasına ulaştı. 8. ayda birleşik ürün geliri $15K/aya ulaştı. 14. ayda $25K/ay. Asla danışmanlığa dönmeyecek.

Bu yol herkes için değil. 8 aylık birikimi, geliri olan bir partneri ve büyüme yörüngesine dayalı ürünlerine yüksek güveni vardı. Bu faktörler olmadan bu bahis cüretkar değil pervasızdır.

### Geliştiriciler İçin Batık Maliyet Tuzağı

Geliştiricilerin benzersiz bir batık maliyet versiyonu vardır: **koda duygusal bağlılık.**

Bir şey inşa etmek için 200 saat harcadın. Kod zarif. Mimari temiz. Test kapsamı mükemmel. Şimdiye kadar yazdığın en iyi kodlardan biri.

Ve kimse satın almıyor.

Kodun kıymetli değil. Zamanın kıymetli. 200 saat, bundan sonra ne yaparsan yap gitmiş durumda. Tek soru şu: SONRAKİ 200 saat nereye gidecek?

Cevap "pazarın reddettiği bir ürünü ayakta tutmak" ise, ısrarcı değilsin. İnatçısın. Israrcılık, geri bildirime göre iterasyon yapmaktır. İnatçılık, geri bildirimi yok sayıp pazarın fikrini değiştirmesini ummaktır.

> **Yaygın Hata:** Öldürmek yerine pivot yapmak. "Sadece yeni bir özellik eklerim." "Farklı bir pazar denerim." "Fiyatlandırmayı değiştiririm." Bazen pivot işe yarar. Ama çoğu zaman, pivot sadece daha yavaş bir ölümdür. Pivot yapacaksan, kesin bir son tarih belirle: "Eğer [belirli metrik] [belirli sürede] [belirli sayıya] ulaşmazsa, bu sefer gerçekten öldürüyorum." Ve sonra gerçekten yap.

### Senin Sıran

**Alıştırma 4.1:** Mevcut veya planlanan portföyündeki her akışa dört sonlandırma kuralını uygula. Her biri için kararı yaz: Tut, Öldür, İzle (belirli bir metrik hedefiyle 3 ay daha ver) veya Otomatikleştir (zaman yatırımını azalt).

**Alıştırma 4.2:** "İzle" olarak işaretlediğin her akış için belirli metriği ve belirli son tarihi yaz. "Eğer [akış] [tarih]e kadar [$X/ay]a ulaşmazsa, öldüreceğim." Göreceğin bir yere koy.

**Alıştırma 4.3:** Daha önce bir projeyi bıraktıysan, geriye dönük bir ölüm sonrası analiz yaz. Ne işe yaradı? Ne yaramadı? Neyi farklı yapardın? Geçmiş başarısızlıklardan çıkardığın dersler gelecek akışlar için yakıttır.

**Alıştırma 4.4:** Günlük işin dahil mevcut her gelir kaynağın için saatlik ROI'yi hesapla. Sırala. Sıralama seni şaşırtabilir.

---

## Ders 5: Yeniden Yatırım Stratejisi

*"İlk $500 ile ne yaptığın, ilk $50,000 ile ne yaptığından daha önemlidir."*

### Yeniden Yatırım İlkesi

Akışlarının ürettiği her doların dört olası varış noktası var:

1. **Cebine** (yaşam giderleri, yaşam tarzı)
2. **Vergiler** (pazarlığı yok — devlet payını alır)
3. **İşe geri** (araçlar, insanlar, altyapı)
4. **Birikim** (pist, güvenlik, gönül rahatlığı)

Çoğu geliştirici kazandığının hepsini harcar (vergiler düşüldükten sonra). Kalıcı gelir operasyonları inşa edenler stratejik olarak yeniden yatırım yapar. Hepsini değil. Çoğunu değil. Ama büyümeyi hızlandıran belirli yatırımlara ayrılmış bilinçli bir yüzde.

### Seviye 1: İlk {= regional.currency_symbol | fallback("$") =}500/Ay

Eşiği geçtin. Para kazanıyorsun. Çok değil, ama gerçek. İşte nereye gidecek:

**Vergi karşılığı: {= regional.currency_symbol | fallback("$") =}150/ay (%30)**
Bu pazarlık götürmez. İş hesabına düşen her {= regional.currency | fallback("doların") =}'ın %30'unu ayrı bir birikim hesabına aktar. "VERGİLER — DOKUNMA" olarak etiketle. Vergi dairesi (IRS, HMRC veya yerel vergi otoriten) bu para için gelecek. Hazır tut.

**Yeniden yatırım: $100-150/ay**
- Daha iyi araçlar: daha hızlı barındırma, müşteriye yönelik kalite için daha fazla API kredisi ($50/ay)
- Düzgün bir alan adı ve profesyonel e-posta için $12/ay
- 4DA Pro için $99/yıl — bu senin istihbarat katmanın. Bir sonraki hangi fırsatı takip edeceğini bilmek, herhangi bir araçtan daha değerlidir. Bu ayda $8.25.
- Sana ayda 3+ saat kazandıran bir iyi araç (dikkatli değerlendir — çoğu araç üretkenlik kılığına girmiş dikkat dağıtıcılardır)

**Cebine: $200-250/ay**
Paranın bir kısmını al. Ciddi olarak. Erken kazanımlar psikolojik olarak önemlidir. Kendine bunun gerçek olduğunu hatırlatan bir şey al. Güzel bir akşam yemeği. Bir kitap. Yeni kulaklıklar. Lamborghini değil. "Bunu kendi operasyonumla kazandım" diyen bir şey.

> **Açık Konuşalım:** $500/ay seviyesi kırılgandır. Heyecan verici hissettiriyor, ama $0'a 2-3 müşteri iptali uzaklıkta. Yaşam tarzını bu rakama göre ölçekleme. İşinden ayrılma. Başardın gibi kutlama. Konsepti kanıtladın gibi kutla. Çünkü yaptığın tam olarak bu — konsepti kanıtlamak.

### Seviye 2: İlk $2,000/Ay

Şimdi konuşuyoruz. $2,000/ay, akışlarının gerçek, tekrarlanabilir gelir ürettiği anlamına gelir. Kaldıraca yatırım zamanı.

**Vergi karşılığı: $600/ay (%30)**

**Yeniden yatırım: $400-600/ay**
- **Teknik olmayan görevler için sanal asistan: $500-800/ay.** Bu aşamada yapabileceğin en yüksek ROI'li işe alım. Uzaktan bir VA (Filipinler, Latin Amerika) ayda 10-15 saat için: e-posta tasnifi, fatura takibi, planlama, veri girişi, sosyal medya paylaşımı, temel müşteri desteği ön taraması. Ayda 10-15 saat kazanırsın. Efektif ücretinle o saatler ayda $500-3,000 değerinde.
- **Profesyonel e-posta ve faturalama altyapısı:** "Faturaları elle gönder"den otomatik faturalamaya geç (Stripe Billing, Lemon Squeezy). Maliyet: $0-50/ay. Zaman tasarrufu: 3-5 saat/ay.
- **Ürünlerin için ücretli tasarım şablonu:** $49-199 tek seferlik. İlk izlenim önemlidir. Profesyonel bir landing sayfası, acele yapılmış birinden 2-3 kat daha iyi dönüştürür.
- **7 STREETS modülünün tümü 4DA içinde ücretsiz.** Tam playbook'u henüz çalışmadıysan, şimdi tam zamanı. $2,000/ay ile uygulayabildiğini kanıtladın. Kalan modüller işe yarayanı hızlandırır. Hesap verebilirlik ve bu aşamadaki diğer geliştiricilerden vaka çalışmaları için Community ($29/ay) üyeliğini düşün.

**Cebine: $800-1,000/ay**

> **Yaygın Hata:** Yanlış şeyler için çok erken işe almak. $2,000/ay ile geliştirici, pazarlamacı, tasarımcı veya sosyal medya yöneticisi GEREKMİYOR. İnşa zamanını çalan idari sürüklenmeyi üstlenen bir VA gerekiyor. Diğer her şey $5K/aya kadar bekleyebilir.

### Seviye 3: İlk $5,000/Ay

$5,000/ay "bağımsızlığa geçmeyi düşün" eşiğidir. "Şimdi yap" değil — "ciddi olarak düşün."

**Vergi karşılığı: $1,500/ay (%30)**

**Bağımsızlığa geçmeden önce — kontrol listesi:**
- [ ] $5K/ay 3+ ardışık ay boyunca sürdürülmüş (bir iyi ay değil)
- [ ] 6 aylık yaşam giderleri biriktirilmiş (iş fonlarından ayrı)
- [ ] 2+ akıştan gelir (hepsi tek müşteriden veya üründen değil)
- [ ] Sağlık sigortası planı belirlenmiş (ABD) veya eşdeğer kapsam
- [ ] Partner/aile uyumlu ve destekleyici
- [ ] Duygusal hazırlık (maaştan ayrılmak Twitter'da göründüğünden daha korkutucu)

**Yeniden yatırım: $1,000-1,500/ay**
- **Yarı zamanlı pazarlamacı veya içerik sorumlusu: $500-1,000/ay.** $5K/ay ile zamanın en değerli varlığındır. Blog yazıları yazan, sosyal varlığını yöneten ve e-posta kampanyaları yürüten yarı zamanlı bir pazarlamacı, inşa etmen için seni serbest bırakır. Upwork'te bul — aylık 10 saatlik denemeyle başla.
- **Ücretli reklam test bütçesi: $500/ay.** Organik büyümeye güveniyordun. Şimdi ücretli kanalları test et. Ürünün için $500 bütçeyle Google Ads veya Reddit ads yayınla. Müşteri edinme maliyeti (CAC) ömür boyu değerden (LTV) düşükse, ölçeklenebilir bir büyüme kanalı buldun. Değilse, organiğin senin kanalın olduğunu öğrenmek için $500 harcadın ve bu da sorun değil.
- **Profesyonel muhasebeci: $200-400/ay.** $5K/ay ($60K/yıl) ile vergi durumu, bir profesyonelin maliyetinden fazla tasarruf ettirecek kadar karmaşık hale gelir. Üç aylık vergi planlaması, kesinti optimizasyonu ve kuruluş yapısı tavsiyeleri. Bu seviyede iyi bir muhasebeci, aksi takdirde fazla ödeyeceğin vergilerde yılda $2,000-5,000 kazandırır.

**Cebine: $2,000-2,500/ay**

### Seviye 4: İlk {= regional.currency_symbol | fallback("$") =}10,000/Ay

Gerçek bir işin var. Ona öyle davran.

**Vergi karşılığı: {= regional.currency_symbol | fallback("$") =}3,000/ay (%30)**

{@ insight cost_projection @}

Bu seviyede yeniden yatırım kararların belirli bir soru tarafından yönlendirilmeli: **"Sonraki {= regional.currency_symbol | fallback("$") =}10K'nın önündeki darboğaz ne?"**

- Darboğaz **geliştirme kapasitesiyse:** bir yüklenici getir ($2,000-4,000/ay, 20-40 saat/ay için)
- Darboğaz **satış/pazarlamaysa:** yarı zamanlı bir büyüme sorumlusu işe al ($1,500-3,000/ay)
- Darboğaz **operasyonlar/destekse:** VA'nı yükselt veya ayrılmış bir destek sorumlusu getir ($1,000-2,000/ay)
- Darboğaz **kendi kapasitense:** teknik kurucu ortağı veya partner düşün (hisse konuşması, gider değil)

**Yapısal yatırımlar:**
- Henüz yapılmadıysa **{= regional.business_entity_type | fallback("LLC") =} kuruluşu**. {= regional.currency_symbol | fallback("$") =}120K/yılda {= regional.business_entity_type | fallback("LLC") =} isteğe bağlı değildir.
- **S-Corp seçimi** (ABD): Serbest çalışmadan tutarlı olarak yılda $40K+ kazanıyorsan, S-Corp seçimi "makul maaş"ın üzerindeki dağıtımlarda %15.3 serbest çalışma vergisi kazandırır. $80K dağıtımda bu yılda $12,240 vergi tasarrufu. Muhasebecin bu konuda seni yönlendirmeli.
- **İş bankası hesabı ve düzgün muhasebe.** Wave (ücretsiz) veya QuickBooks ($25/ay) veya muhasebeci ($200-400/ay).
- **Sorumluluk sigortası.** Mesleki sorumluluk / E&O sigortası yılda $500-1,500. Bir müşteri seni dava ederse, bu kötü bir gün ile iflas arasındaki farktır.

**Zihniyet değişimi:**

$10K/ay'da mevcut $10K hakkında düşünmeyi bırak ve SONRAKİ $10K hakkında düşünmeye başla. İlk $10K 12 ay sürdü. İkinci $10K 6 ay veya daha az sürmeli, çünkü artık elinde:

- Bir kitle
- Bir itibar
- Çalışan sistemler
- Yeniden yatırım yapacak gelir
- Neyin işe yaradığına dair veri

var.

Oyun "nasıl para kazanırım"dan "zaten işe yarayanı nasıl ölçeklendiririm"e dönüşür.

### Vergi Planlaması: Nisana Kadar Kimsenin Okumadığı Bölüm

Bu bölümü şimdi oku. Nisanda değil. Şimdi.

{? if regional.country == "US" ?}
> **ABD'desin.** Aşağıdaki bölüm vergi yükümlülüklerini doğrudan kapsar. Üç aylık tahmini vergilere ve S-Corp seçim eşiğine özellikle dikkat et.
{? elif regional.country == "GB" ?}
> **İngiltere'desin.** Spesifik yükümlülüklerin için Birleşik Krallık bölümüne ilerle. Self Assessment son tarihleri ve Class 4 NIC'ler anahtar kalemlerindir.
{? elif regional.country ?}
> **Konumun: {= regional.country | fallback("ülken") =}.** Genel ilkeler için aşağıdaki tüm bölümleri incele, ardından ayrıntılar için yerel vergi uzmanına danış.
{? endif ?}

**Amerika Birleşik Devletleri:**

- **Üç aylık tahmini vergiler:** Vadeler — 15 Nisan, 15 Haziran, 15 Eylül, 15 Ocak. Yıl için $1,000'dan fazla vergi borçlanıyorsan, IRS üç aylık ödemeler bekler. Eksik ödeme, eksik tutar üzerinden yıllık ~%8 ceza tetikler.
- **Serbest çalışma vergisi:** Net kazanç üzerinden %15.3 (%12.4 Sosyal Güvenlik + %2.9 Medicare). Bu, gelir vergisi diliminin üzerine eklenir. Serbest çalışmadan $80K kazanan bir geliştirici ~$12,240 SE vergisi artı gelir vergisi öder.
- **Geliştiricilerin unuttuğu kesintiler:**
  - Ev ofis: $5/ft kare, 300 ft kareye kadar = $1,500/yıl (basitleştirilmiş yöntem). Veya gerçek giderler (orantılı kira, faturalar, sigorta) ki bu genellikle daha fazla verir.
  - Ekipman: Bilgisayar, monitörler, klavye, fare, masa, sandalye — Section 179 kesintisi. $2,000'lık bilgisayar al, o yıl gelirden $2,000 düş.
  - Yazılım abonelikleri: İş için kullanılan her SaaS aracı. GitHub, Vercel, Anthropic kredileri, Ollama ile ilgili donanım, alan adları, e-posta servisleri.
  - İnternet: İş kullanım yüzdesi. İnterneti %50 iş için kullanıyorsan, internet faturanın %50'sini düş.
  - Sağlık sigortası primleri: Serbest çalışanlar sağlık sigortası primlerinin %100'ünü düşebilir.
  - Eğitim: İş gelirinle ilgili kurslar, kitaplar, konferanslar.
  - Seyahat: Müşteriyle buluşmak veya konferansa katılmak için seyahat ediyorsan, uçuşlar, oteller ve yemekler indirilebilir.

**Avrupa Birliği:**

- **KDV yükümlülükleri:** AB müşterilerine dijital ürünler satıyorsan, ülkende KDV kaydı gerekebilir (veya One-Stop Shop / OSS sistemini kullanabilirsin). Eşikler ülkeye göre değişir. Lemon Squeezy veya Paddle gibi Merchant of Record kullanmak bunu tamamen hallededer.
- **Çoğu AB ülkesinde üç aylık veya altı aylık vergi beyanı vardır.** Son tarihlerini bil.

**Birleşik Krallık:**

- **Self Assessment:** Önceki vergi yılı için 31 Ocak'a kadar. Hesaba ödemeler 31 Ocak ve 31 Temmuz'da.
- **Trading Allowance:** İlk GBP 1,000 ticari gelir vergisizdir.
- **Class 4 NIC'ler:** GBP 12,570 ile GBP 50,270 arasındaki kârda %6. Üzerinde %2.

**Ülkeden bağımsız evrensel vergi tavsiyeleri:**

1. Brüt gelirin %30'unu geldiği gün ayır. %20 değil. %25 değil. %30. Ya borçlu olacaksın ya da vergi zamanında güzel bir sürpriz yaşayacaksın.
2. Her iş giderini ilk günden itibaren takip et. Bir elektronik tablo, Wave veya Hledger kullan. Giderlerini takip eden geliştiriciler, aksi takdirde masada bırakacakları vergilerde yılda $2,000-5,000 tasarruf eder.
3. $5K/ayı geçtiğinde profesyonel muhasebeci edin. ROI anında.
4. Asla kişisel ve iş fonlarını karıştırma. Ayrı hesaplar. Her zaman.

{? if regional.tax_note ?}
> **{= regional.country | fallback("bölgen") =} için vergi notu:** {= regional.tax_note | fallback("Ayrıntılar için yerel vergi uzmanına danış.") =}
{? endif ?}

### Senin Sıran

**Alıştırma 5.1:** Mevcut veya öngörülen gelinine dayanarak hangi Seviyedesin (1-4) belirle. Spesifik dağılımı yaz: vergilere, yeniden yatırıma ve kendine ne kadar.

**Alıştırma 5.2:** Seviye 2+ isen, bu ay yapabileceğin en yüksek ROI'li tek işe alım veya satın almayı belirle. En heyecan verici olanı değil. Harcanan dolar başına en fazla saat veya dolar tasarruf eden veya üreten olanı.

**Alıştırma 5.3:** Mevcut efektif vergi oranını hesapla. Bilmiyorsan, cevabın o — öğrenmen gerekiyor. Bir muhasebeciyle konuş veya ülkenin vergi dairesi web sitesinde bir saat geçir.

**Alıştırma 5.4:** Yoksa ayrı bir "vergi karşılığı" hesabı aç. İş hesabından %30 otomatik transferi ayarla. Bunu bugün yap, "gelir arttığında" değil.

**Alıştırma 5.5:** Muhtemelen kaçırdığın üç kesintiyi yaz. Yukarıdaki listeyi kontrol et. Çoğu geliştirici, küçük giderleri takip etmediği için yılda $1,000-3,000'lık kesintiyi masada bırakır.

---

## Ders 6: Stream Stack'in (12 Aylık Plan)

*"Plansız bir hedef bir dilektir. Kilometre taşı olmayan bir plan bir fantezidir. İşte gerçeklik."*

### Çıktı

İşte bu. Tüm STREETS kursundaki son alıştırma. İnşa ettiğin her şey — altyapı, hendekler, gelir motorları, uygulama disiplini, istihbarat, otomasyon — tek bir belgede birleşir: Stream Stack'in.

Stream Stack yatırımcılar için bir iş planı değildir. Senin için bir operasyon planıdır. Bu ay tam olarak ne üzerinde çalışacağını, neyi ölçeceğini, neyi öldüreceğini ve neyi büyüteceğini söyler. Her pazartesi sabahı sınırlı saatlerini nasıl harcayacağına karar vermek için açtığın belgedir.

### Stream Stack Şablonu

Yeni bir dosya oluştur. Bu şablonu kopyala. Her alanı doldur. Bu senin 12 aylık operasyon planın.

```markdown
# Stream Stack
# [Adın / İşletme Adı]
# Oluşturulma: [Tarih]
# Hedef: [Tarih + 12 ay]a kadar $[X],000/ay

---

## Portföy Profili
- **Arketip:** [Önce Güvenlik / Büyüme Modu / Bağımsızlığa Geçiş]
- **Toplam müsait saat/hafta:** [X]
- **Mevcut aylık gelir:** $[X]
- **12 aylık gelir hedefi:** $[X]
- **Köprü geliri:** $[X]/ay (Hızlı Nakit akışlarından)

---

## Akış 1: [İsim]

**Kategori:** [Hızlı Nakit / Büyüyen Varlık / İçerik Bileşiği /
             Pasif Otomasyon / Hisse Oyunu]

**Açıklama:** [Bir cümle — bu akışın ne olduğu ve kimin ödediği]

### Gelir Hedefleri
| Zaman Dilimi | Hedef | Gerçekleşen |
|-----------|--------|--------|
| Ay 3   | $[X]   |        |
| Ay 6   | $[X]   |        |
| Ay 12  | $[X]   |        |

### Zaman Yatırımı
- **İnşa aşaması:** [X] saat/hafta, [X] ay boyunca
- **Büyüme aşaması:** [X] saat/hafta
- **Bakım aşaması:** [X] saat/hafta

### Anahtar Kilometre Taşları
- **Ay 1:** [Spesifik çıktı — "Landing sayfası ve beta lansmanı"]
- **Ay 3:** [Spesifik metrik — "10 ödeme yapan müşteri"]
- **Ay 6:** [Spesifik metrik — "$500/ay tekrarlayan"]
- **Ay 12:** [Spesifik metrik — "$2,000/ay tekrarlayan"]

### Sonlandırma Kriterleri
[Bu akışı kapatmana neden olacak spesifik koşul]
Örnek: "6 aylık tutarlı haftalık çabadan sonra $100/aydan az"

### Otomasyon Planı
[Bu akışın hangi kısımları otomatikleştirilebilir ve ne zamana kadar]
Örnek: "Hoşgeldin e-postalarını Ay 2'ye kadar otomatikleştir.
Raporlama panosunu Ay 4'e kadar otomatikleştir. Sosyal medya
dağıtımını Ay 3'e kadar otomatikleştir."

### Volan Bağlantısı
[Bu akış diğer akışlarını nasıl besler veya onlardan beslenir]
Örnek: "Bu danışmanlık işinden müşteri sorunları Akış 2 için
ürün fikirleri üretir. Bu işten vaka çalışmaları Akış 3 için
içerik olur."

---

## Akış 2: [İsim]
[Akış 1 ile aynı yapı]

---

## Akış 3: [İsim]
[Akış 1 ile aynı yapı]

---

## [Varsa Akış 4-5]

---

## Aylık İnceleme Şablonu

### Gelir Panosu
| Akış | Hedef | Gerçekleşen | Delta | Trend |
|--------|--------|--------|-------|-------|
| Akış 1 | $[X] | $[X] | +/-$[X] | yukarı/aşağı/düz |
| Akış 2 | $[X] | $[X] | +/-$[X] | yukarı/aşağı/düz |
| Akış 3 | $[X] | $[X] | +/-$[X] | yukarı/aşağı/düz |
| **Toplam** | **$[X]** | **$[X]** | | |

### Zaman Panosu
| Akış | Planlanan saat | Gerçekleşen saat | ROI ($/saat) |
|--------|------------|------------|------------|
| Akış 1 | [X] | [X] | $[X] |
| Akış 2 | [X] | [X] | $[X] |
| Akış 3 | [X] | [X] | $[X] |

### Aylık Sorular
1. Hangi akış zamanın üzerinde en yüksek ROI'ye sahip?
2. Hangi akış en iyi büyüme yörüngesine sahip?
3. Herhangi bir akış sonlandırma kriterlerine mi ulaşıyor?
4. Tüm akışlardaki en büyük darboğaz ne?
5. Gelecek ay en büyük etkiyi yapacak tek şey ne?

---

## 12 Aylık Yol Haritası

### Aşama 1: Temel (Ay 1-3)
- Ay 1: [Birincil odak — genellikle Akış 1 (Hızlı Nakit) lansmanı]
- Ay 2: [Akış 1 gelir üretiyor. Akış 2 inşası başlıyor]
- Ay 3: [Akış 1 istikrarlı. Akış 2 betada. Akış 3 başladı]

### Aşama 2: Büyüme (Ay 4-6)
- Ay 4: [Akış 1 bakımda. Akış 2 yayınlandı. Akış 3 büyüyor]
- Ay 5: [Akış 1 süreçlerinin ilk otomasyonu]
- Ay 6: [Yıl ortası değerlendirme. Tüm akışlar için öldür/büyüt/koru kararları]

### Aşama 3: Optimizasyon (Ay 7-9)
- Ay 7: [İşe yarayanı ölçeklendir. Yaramayanı öldür]
- Ay 8: [Kapasite izin verirse Akış 4 ekle]
- Ay 9: [Volan bağlantıları güçleniyor]

### Aşama 4: Hızlanma (Ay 10-12)
- Ay 10: [Tam portföy çalışıyor]
- Ay 11: [Tüm akışlarda ROI optimize et]
- Ay 12: [Yıllık değerlendirme. Yıl 2'yi planla. Portföyü yeniden dengele]

---

## Üç Aylık Karar Noktaları

### Q1 Değerlendirme (Ay 3)
- [ ] Tüm akışlar yayınlandı veya betada
- [ ] Gelir aylık maliyetleri karşılıyor (minimum)
- [ ] Zaman dağılımı planla uyuşuyor (+/- %20)
- [ ] Her akış için sonlandırma kriterleri değerlendirildi

### Q2 Değerlendirme (Ay 6)
- [ ] En az bir akış hedef gelirde
- [ ] Sonlandırma kriterlerine ulaşan tüm akışlar öldürüldü
- [ ] Volan bağlantıları görünür sonuçlar üretiyor
- [ ] İlk yeniden yatırım kararları alındı

### Q3 Değerlendirme (Ay 9)
- [ ] Toplam gelir 12 aylık hedefin %60+'ında
- [ ] Portföy performansa göre yeniden dengelendi
- [ ] Otomasyon ayda 5+ saat kazandırıyor
- [ ] Mevcut akışlar kapasitedeyse sonraki akışlar belirlendi

### Q4 Değerlendirme (Ay 12)
- [ ] 12 aylık hedefe ulaşıldı (veya nedeninin net anlaşılması)
- [ ] Tam portföy performans analizi
- [ ] Yıl 2 planı taslağı hazırlandı
- [ ] Stream Stack belgesi gerçekleşenler ve öğrenilenlerle güncellendi
```

### Tamamlanmış Bir Stream Stack: Gerçek Örnek

İşte orta seviye bir full-stack geliştirici için tamamlanmış, doldurulmuş bir Stream Stack. Varsayımsal değil. Bu çerçeveyi uygulayan geliştiricilerin kompozitine dayalı.

```markdown
# Stream Stack
# Alex Chen
# Oluşturulma: Şubat 2026
# Hedef: Şubat 2027'ye kadar $8,000/ay

---

## Portföy Profili
- **Arketip:** Önce Güvenlik (Ay 9'da Büyüme Moduna geçiş)
- **Toplam müsait saat/hafta:** 18 (akşamlar + cumartesiler)
- **Mevcut aylık gelir:** $0 ($130K/yıl ile tam zamanlı çalışıyor)
- **12 aylık gelir hedefi:** $8,000/ay
- **Köprü geliri:** $0 (çalışıyor — akışlar 6 ay istikrar
  kanıtlayana kadar maaş takviyesi)

---

## Akış 1: Next.js Performans Danışmanlığı

**Kategori:** Hızlı Nakit

**Açıklama:** Next.js çalıştıran e-ticaret şirketleri için sabit
kapsamlı performans denetimleri. Çıktı: Önceliklendirilmiş
önerilerle 10 sayfalık denetim raporu. Fiyat: denetim başına $2,500.

### Gelir Hedefleri
| Zaman Dilimi | Hedef | Gerçekleşen |
|-----------|--------|--------|
| Ay 3   | $2,500 (1 denetim/ay) |  |
| Ay 6   | $5,000 (2 denetim/ay) |  |
| Ay 12  | $5,000 (2 denetim/ay, ücret artışı olabilir) |  |

### Zaman Yatırımı
- **İnşa aşaması:** 5 saat/hafta, 1 ay (denetim şablonu, landing sayfası)
- **Büyüme aşaması:** 8 saat/hafta (4 saat teslimat, 2 saat pazarlama, 2 saat yönetim)
- **Bakım aşaması:** 6 saat/hafta

### Anahtar Kilometre Taşları
- Ay 1: Denetim şablonu hazır. Landing sayfası canlı. Ajanslara
  ilk 5 soğuk erişim e-postası gönderildi.
- Ay 3: İlk ücretli denetim teslim edildi. 2 referans toplandı.
- Ay 6: Ayda 2 denetim. Bekleme listesi oluşuyor. Ücret artışı $3,000'a.
- Ay 12: Ayda 2 denetim, $3,000'dan. Ürünleştirilmiş hizmet sayfası
  Google'da "Next.js performance audit" için sıralanıyor.

### Sonlandırma Kriterleri
4 aylık aktif erişimden sonra (20+ soğuk e-posta gönderilmiş,
5+ yazı yayınlanmış) tek bir ücretli denetim alınamaması.

### Otomasyon Planı
- Ay 1: Denetim raporu oluşturma şablonunu otomatikleştir (metrikleri
  doldur, PDF olarak otomatik formatla)
- Ay 2: Lighthouse/WebPageTest çalıştırmalarını ve veri toplamayı otomatikleştir
- Ay 3: Denetim teslimatından sonra takip e-posta dizilerini otomatikleştir

### Volan Bağlantısı
Her denetim yaygın Next.js performans kalıplarını ortaya çıkarır →
Akış 3 (blog) için içerik olur. Yaygın denetim bulguları → Akış 2
(SaaS aracı) için özellikler olur. Denetim müşterileri → potansiyel
SaaS müşterileri olur.

---

## Akış 2: PerfKit — Next.js Performans İzleme Panosu

**Kategori:** Büyüyen Varlık

**Açıklama:** Next.js uygulamaları için AI destekli önerilerle
Core Web Vitals izleyen hafif SaaS. $19/ay.

### Gelir Hedefleri
| Zaman Dilimi | Hedef | Gerçekleşen |
|-----------|--------|--------|
| Ay 3   | $0 (hâlâ inşa ediliyor) |  |
| Ay 6   | $190 (10 müşteri) |  |
| Ay 12  | $950 (50 müşteri) |  |

### Zaman Yatırımı
- **İnşa aşaması:** 8 saat/hafta, 4 ay
- **Büyüme aşaması:** 5 saat/hafta
- **Bakım aşaması:** 3 saat/hafta

### Anahtar Kilometre Taşları
- Ay 1: Mimari ve veri modeli. Bekleme listesili landing sayfası.
- Ay 3: MVP 20 beta kullanıcıya (ücretsiz) yayınlandı. Geri bildirim toplama.
- Ay 6: Ücretli lansman. 10 ödeme yapan müşteri.
  Lighthouse CI entegrasyonu teslim edildi.
- Ay 12: 50 müşteri. Aylık kayıp oranı < %5.
  Otomatik uyarı özelliği teslim edildi.

### Sonlandırma Kriterleri
Lansmandan 9 ay sonra (toplam Ay 13) 20'den az ödeme yapan
müşteri. Sonlandırma kriterleri tetiklenirse, kodu open source yap
ve barındırılan versiyonu kapat.

### Otomasyon Planı
- Ay 4: Otomatik hoşgeldin e-postaları (3 e-postalık dizi)
- Ay 5: Müşterilere otomatik haftalık performans raporları
- Ay 6: Otomatik faturalama ve takip (Stripe Billing)

### Volan Bağlantısı
Beslenir: Danışmanlık denetimleri özellik ihtiyaçlarını ortaya çıkarır.
Next.js performansı hakkında blog yazıları → kayıtları artırır.
Besler: Müşteri kullanım verileri → içerik fikirleri.
Müşteri vaka çalışmaları → danışmanlık güvenilirliği.

---

## Akış 3: "Next.js in Production" Blog + Bülten

**Kategori:** İçerik Bileşiği

**Açıklama:** Next.js performansı, mimarisi ve production operasyonları
hakkında haftalık blog yazıları ve iki haftalık bülten.
Ücretsiz blog, $8/ay ücretli bülten katmanı.

### Gelir Hedefleri
| Zaman Dilimi | Hedef | Gerçekleşen |
|-----------|--------|--------|
| Ay 3   | $0 (kitle oluşturma) |  |
| Ay 6   | $80 (10 ücretli abone) |  |
| Ay 12  | $800 (100 ücretli abone) + $400 (affiliate) |  |

### Zaman Yatırımı
- **İnşa aşaması:** 4 saat/hafta, 2 ay (blog kurulumu, ilk 8 yazı,
  e-posta yakalama inşası)
- **Büyüme aşaması:** 4 saat/hafta (haftada 1 yazı + bülten küratörlüğü)
- **Bakım aşaması:** 3 saat/hafta

### Anahtar Kilometre Taşları
- Ay 1: Blog 4 temel yazıyla yayında. Her sayfada bülten
  kayıt formu. Twitter/X hesabı aktif.
- Ay 3: 500 e-posta abonesi. 8+ blog yazısı Google'da indekslendi.
  İlk HN veya Reddit yazısı ilgi gördü.
- Ay 6: 2,000 e-posta abonesi. 100 ücretli. İlk sponsorluk talebi.
- Ay 12: 5,000 e-posta abonesi. 100 ücretli. Tutarlı organik trafik.
  Blog danışmanlık müşteri adayları üretiyor.

### Sonlandırma Kriterleri
6 aylık haftalık yayından sonra 500'den az e-posta abonesi.
(İçerik akışları ürünlerden daha fazla zaman alır çünkü bileşik
büyüme daha yavaştır.)

### Otomasyon Planı
- Ay 1: RSS-sosyal medya otomasyonu (yeni yazı → otomatik tweet)
- Ay 2: Bülten şablonu otomasyonu (son yazıları çek, formatla, planla)
- Ay 3: 4DA entegrasyonu — bülten küratörlüğü için Next.js ile
  ilgili sinyalleri ortaya çıkar

### Volan Bağlantısı
Beslenir: Danışmanlık deneyimleri → blog konuları.
Ürün geliştirme dersleri → "PerfKit İnşa Etmek" serisi.
Besler: Blog yazıları → danışmanlık müşteri adayları. Blog yazıları → ürün kayıtları.
Bülten kitlesi → ürün lansmanı dağıtım kanalı.

---

## 12 Aylık Yol Haritası

### Aşama 1: Temel (Ay 1-3)
- Ay 1: Danışmanlık servisini başlat (landing sayfası, ilk erişimler).
  4 yazıyla blogu başlat. PerfKit mimarisine başla.
- Ay 2: İlk danışmanlık müşterisi. Blog haftalık yayınlanıyor.
  PerfKit MVP sürecinde. Bülten başlatıldı.
- Ay 3: İlk denetim teslim edildi ($2,500). PerfKit 20 kullanıcıyla
  betada. Blog 500 abonede.
  Gelir: ~$2,500 | Saatler: 18/hafta

### Aşama 2: Büyüme (Ay 4-6)
- Ay 4: İkinci danışmanlık müşterisi kazanıldı. PerfKit ücretli lansmanı.
  Blog içeriği bileşik büyüyor.
- Ay 5: Ayda 2 danışmanlık. PerfKit 10 müşteride.
  Blogdan ilk danışmanlık müşteri adayı.
- Ay 6: Yıl ortası değerlendirme. Gelir: ~$5,270 | Saatler: 18/hafta
  Karar: Rotayı koru mu hızlan mı?

### Aşama 3: Optimizasyon (Ay 7-9)
- Ay 7: Danışmanlık ücreti $3,000/denetime artış. PerfKit müşteri
  geri bildirimine göre özellik genişletme.
- Ay 8: Akış 4 eklemeyi değerlendir (otomasyon — bağımsız ürün
  olarak otomatik performans raporları).
- Ay 9: Volan görünür şekilde çalışıyor — blog hem danışmanlık
  hem PerfKit kayıtlarını yönlendiriyor. Gelir: ~$7,000

### Aşama 4: Hızlanma (Ay 10-12)
- Ay 10: Tüm akışlar çalışıyor. PerfKit'i ölçeklendirmeye odaklan.
- Ay 11: Gelir optimizasyonu — fiyatları yükselt, dönüşümü
  iyileştir, kayıp oranını azalt.
- Ay 12: Yıllık değerlendirme. Gelir hedefi: $8,000/ay.
  Yıl 2 planı: danışmanlığı ayda 1'e düşür, PerfKit ve
  içeriği ölçeklendir.
```

### Aylık İnceleme Kadansı

Stream Stack ancak incelersen işe yarar. İşte kadans:

**Aylık inceleme (30 dakika, her ayın ilk pazartesisi):**
1. Her akış için gelir gerçekleşmelerini güncelle
2. Her akış için zaman gerçekleşmelerini güncelle
3. Her akış için saat başına ROI hesapla
4. Sonlandırma kriterlerini gerçekleşmelere karşı kontrol et
5. Bu ay ele alınacak bir darboğaz belirle

**Üç aylık inceleme (2 saat, her 3 ayda):**
1. Her akış için öldür/büyüt/koru kararı
2. Portföy yeniden dengeleme — düşük ROI'den yüksek ROI akışlara zaman kaydır
3. Yeni akış eklemeyi değerlendir (sadece mevcut akışlar bakım aşamasındaysa)
4. Gerçek performansa göre 12 aylık yol haritasını güncelle

**Yıllık inceleme (yarım gün, STREETS Evolving Edge güncellemesiyle çakışır):**
1. Tam portföy performans analizi
2. Yıl 2 planı: ne kalır, ne gider, ne yeni
3. Yıl 2 gelir hedefi (volan çalışıyorsa Yıl 1'in 2-3 katı olmalı)
4. Sovereign Stack Belgesi güncellemesi (donanım, bütçe, yasal durum değişmiş olabilir)
5. Beceri envanteri güncellemesi — bu yıl hangi yeni yetenekleri geliştirdin?

### 12 Aylık Yol Haritası Şablonu (Genel)

Sıfırdan başlıyorsan, varsayılan sıralama budur:

**Ay 1-2: Akış 1 Lansmanı (Gelire En Hızlı)**
Hızlı Nakit akışın. Danışmanlık, serbest çalışma veya hizmetler. Daha yavaş akışları inşa ederken finansal köprüyü sağlar. Fazla düşünme. Zaten bildiğin şey için sana ödeme yapacak birini bul.

**Ay 2-3: Akış 2 İnşasına Başla (Bileşik Varlık)**
Akış 1 nakit üretirken, müsait zamanının %30-40'ını ürün inşa etmeye yatır. Akış 1 müşteri işinden gelen içgörüleri ne inşa edeceğini belirlemek için kullan.

**Ay 3-4: Akış 3'e Başla (İçerik/Kitle)**
Yayınlamaya başla. Blog, bülten, YouTube — bir kanal seç ve haftalık yayına taahhüt et. Bu akışın getiri süresi en uzundur, işte tam da bu yüzden erken başlarsın.

**Ay 5-6: Akış 1'in İlk Otomasyonu**
Bu noktada yeterince danışmanlık/hizmet işi yaptın, tekrarlayan kısımları belirleyebilirsin. Otomatikleştir. Faturalamayı, raporlamayı, müşteri katılımını veya herhangi bir şablon işi otomatikleştir. Serbest kalan zaman Akış 2 ve 3'e gider.

**Ay 7-8: İşe Yarayanı Ölçeklendir, Yaramayanı Öldür**
Yıl ortası hesaplaşma. Her akışı sonlandırma kriterlerine karşı kontrol et. Dürüst ol. Düşük performanslı akışlardan yüksek performanslılara zaman kaydır. Tüm akışlar düşük performans gösteriyorsa, niş seçimini (Modül T) ve uygulamanı (Modül E) yeniden değerlendir.

**Ay 9-10: Kapasite İzin Verirse Akış 4 Ekle**
Sadece Akış 1-3 gelir üretiyorsa ve tüm zamanını tüketmiyorsa. Akış 4 genellikle otomasyon veya pasif bir üründür — minimum süregelen çabayla çalışan bir şey.

**Ay 11-12: Tam Portföy Optimizasyonu, Yıl 2 Planı**
Fiyatlandırmayı optimize et, kayıp oranını azalt, dönüşümü iyileştir, daha fazla otomatikleştir. Yıl 2 planını hazırla. Yıl 2 hedefi Hızlı Nakit bağımlılığını azaltmak ve ürün/içerik/otomasyon gelir payını artırmak.

> **Yaygın Hata:** Tüm akışları aynı anda başlatmak. Birinde anlamlı ilerleme yerine hepsinde sıfır ilerleme kaydedersin. Sıralı lansman, paralel değil. Akış 1 gelir üretmeli, Akış 2 inşası başlamadan. Akış 2 betada olmalı, Akış 3 yayınlamaya başlamadan. Her akış, önceki akışın performansıyla zaman dağılımını hak eder.

### Senin Sıran

**Alıştırma 6.1:** 3-5 akışınla tam Stream Stack şablonunu doldur. Her alan. Yer tutucu yok. Gerçek ücretlerine, gerçekçi müşteri sayılarına ve dürüst zaman müsaitliğine dayalı gerçek sayılar kullan.

**Alıştırma 6.2:** İlk aylık incelemen için bir takvim hatırlatıcısı ayarla — bugünden 30 gün sonra. Şu an takvimine koy. "Sonra yaparım" değil. Şimdi.

**Alıştırma 6.3:** Her akış için sonlandırma kriterlerini yaz. Spesifik ve zamana bağlı yap. Seni sorumlu tutacak biriyle paylaş. O kişin yoksa, monitörüne yapışkan nota yaz.

**Alıştırma 6.4:** Yığınındaki en güçlü tek volan bağlantısını belirle. Bu, en çok yatırım yapman gereken bağlantıdır. Önümüzdeki 30 gün içinde o bağlantıyı güçlendirmek için atacağın üç spesifik eylemi yaz.

---

## STREETS Mezunu

### Tam Yolculuk

{? if progress.completed("R") ?}
Modül S'ye (Egemen Kurulum) bir donanım envanteri ve bir hayalle başladın. Modül R'deki gelir motorların artık daha büyük bir sistemin bileşenleri. Modül S'yi (Akışları İstiflemek) eksiksiz bir gelir operasyonuyla bitiriyorsun.
{? else ?}
Modül S'ye (Egemen Kurulum) bir donanım envanteri ve bir hayalle başladın. Modül S'yi (Akışları İstiflemek) eksiksiz bir gelir operasyonuyla bitiriyorsun.
{? endif ?}

İşte tam STREETS yolculuğunun inşa ettiği:

**S — Egemen Kurulum (Hafta 1-2):** Rig'ini denetledin, yerel LLM'leri kurdun, yasal ve finansal temelleri attın ve Sovereign Stack Belgenizi oluşturdun. Altyapın bir iş varlığı oldu.

**T — Teknik Hendekler (Hafta 3-4):** Benzersiz beceri kombinasyonlarını belirledin, tescilli veri pipeline'ları inşa ettin ve rakiplerin kolayca kopyalayamayacağı savunulabilir avantajlar tasarladın. Uzmanlığın bir hendek oldu.

**R — Gelir Motorları (Hafta 5-8):** Spesifik, kodla desteklenmiş monetizasyon sistemleri inşa ettin. Teori değil — gerçek kod, gerçek fiyatlandırma ve gerçek dağıtım kılavuzlarıyla gerçek ürünler, hizmetler ve otomasyon. Becerilerin ürün oldu.

**E — Uygulama Playbook'u (Hafta 9-10):** Lansman dizilerini, fiyatlandırma stratejilerini ve ilk müşterilerini nasıl bulacağını öğrendin. Teslim ettin. "Teslim etmeyi planladın" değil. Teslim ettin. Ürünlerin teklifler oldu.

**E — Gelişen Avantaj (Hafta 11-12):** Sinyal algılama sistemleri inşa ettin, trend analizini öğrendin ve rakiplerden önce fırsatları görecek şekilde konumlandın. İstihbaratın avantaj oldu.

**T — Taktik Otomasyon (Hafta 13-14):** Operasyonunun tekrarlayan kısımlarını otomatikleştirdin — izleme, raporlama, müşteri katılımı, içerik dağıtımı. Sistemlerin özerk oldu.

**S — Akışları İstiflemek (Hafta 14-16):** Spesifik hedefler, sonlandırma kriterleri ve 12 aylık yol haritasıyla birbirine bağlı gelir akışlarından oluşan bir portföy tasarladın. Akışların bir iş oldu.

### Bir STREETS Mezunu Neye Benzer

Bu kursu tamamlayan ve 12 ay boyunca uygulayan bir geliştiricinin:

**7/24 çalışan egemen altyapısı var.** Çıkarım yapan, veri işleyen ve müşterilere herhangi bir bulut sağlayıcısına bağımlı olmadan hizmet veren yerel bir hesaplama yığını. Rig artık tüketici ürünü değil. Gelir üreten bir varlıktır.

**Fiyatlandırma gücüne sahip net teknik hendekleri var.** Rakiplerin YouTube eğitimi izleyerek kopyalayamayacağı beceri kombinasyonları, tescilli veriler ve özel araç zincirleri. $200/saat teklif ettiğinde müşteriler irkilmez — çünkü sunduğunu $50/saat alternatifinden alamazlar.

**Gelir üreten birden fazla gelir motoru var.** Tek bir kırılgan akış değil. Farklı kategoriler ve risk profillerinde üç, dört, beş akış. Biri düştüğünde diğerleri taşır. Biri yükseldiğinde fazlalık bir sonraki fırsata yeniden yatırılır.

**Uygulama disiplini var.** Haftalık teslim eder. Duygulara değil verilere göre iterasyon yapar. Batık maliyetlere duygusal bağlılık olmadan düşük performanslı akışları öldürür. Rakamları aylık inceler. Zor kararları üç ayda bir alır.

**Güncel istihbaratı var.** Nişinde ne olduğunu her zaman bilir. Twitter'da doom scrolling'den değil. Fırsatları, tehditleri ve trendleri bariz hale gelmeden önce ortaya çıkaran bilinçli bir sinyal algılama sisteminden.

**Taktik otomasyonu var.** Makineler her akışta tekrarlayan işi halleder. Fatura oluşturma, içerik dağıtımı, izleme, müşteri katılımı, raporlama — tamamı otomatik. İnsan saatleri sadece insanların yapabileceği işe gider: strateji, yaratıcılık, ilişkiler, yargılama.

**İstiflenmiş akışları var.** Her akışın diğerlerini beslediği çeşitlendirilmiş, dayanıklı bir gelir portföyü. Volan dönüyor. Her itme daha az çaba gerektirir ve daha fazla momentum üretir.

{? if dna.is_full ?}
> **Developer DNA özet:** {= dna.identity_summary | fallback("Profil mevcut") =}. En çok ilgilendiğin konular ({= dna.top_engaged_topics | fallback("4DA panona bak") =}) doğal akış temelleridir. {? if dna.blind_spots ?}Kör noktalarını izle ({= dna.blind_spots | fallback("tespit edilmedi") =}) — keşfedilmemiş akış kategorilerini temsil edebilirler.{? endif ?}
{? endif ?}

### Uzun Vadeli Oyun

STREETS "hızlı zengin ol" sistemi değildir. "12-24 ayda ekonomik egemenlik elde et" sistemidir.

Ekonomik egemenlik şu anlama gelir:

- Herhangi bir tek gelir kaynağından — işverenin dahil — finansal panik yaşamadan ayrılabilirsin
- Altyapını, verilerini, müşteri ilişkilerini ve zamanını kontrol edersin
- Hiçbir platform, müşteri, algoritma veya şirket gelirini bir gecede çökertemez
- Gelirin daha fazla saati daha fazla dolara takas etmekle değil bileşik büyümeyle artar

Bu zaman alır. 12 aylık tutarlı uygulamadan sonra $10K/ay kazanan geliştiricinin, tek bir şanslı ürün lansmanından $10K kazanan geliştiriciden çok daha değerli bir şeyi var. İlk geliştiricinin sistemi var. İkincinin piyango bileti var.

Sistemler piyango biletlerini yener. Her zaman. Her zaman diliminde.

### Topluluk

STREETS Community üyeleri ($29/ay veya $249/yıl) geliştiricilerin paylaştığı özel topluluğa erişir:

- **Aylık gelir raporları:** Gerçek rakamlar, gerçek akışlar, gerçek zorluklar.
- **Akış lansmanları:** Ne inşa ettiler, nasıl fiyatlandırdılar, ne oldu.
- **Sonlandırma kararları:** Neyi kapattılar ve neden. Bunlar en değerli paylaşımlardan bazıları.
- **Kazanımlar:** Kazanılan ilk dolar. İlk $1K ayı. İlk $10K ayı. Bunlar önemli.
- **Başarısızlıklar:** Çöken ürünler. Kaybolan müşteriler. Kayan algoritmalar. Bunlar daha da önemli.

100 geliştiricinin aynı anda STREETS uygulamasından, herhangi bir kurs, kitap veya podcast'ten — bu dahil — daha hızlı öğrenirsin.

### Yıllık Güncelleme

Teknoloji manzarası değişir. Düzenlemeler evrilir. Yeni platformlar ortaya çıkar. Eskileri ölür. API fiyatlandırması kayar. Model yetenekleri gelişir. Pazarlar açılır ve kapanır.

STREETS yıllık olarak güncellenir. 2027 baskısı şunları yansıtacak:

- 2026'da var olmayan yeni gelir fırsatları
- Ölen veya metalaşan akışlar
- Güncellenmiş fiyatlandırma referansları ve pazar verileri
- Geliştirici gelirini etkileyen düzenleyici değişiklikler
- Yeni araçlar, platformlar ve dağıtım kanalları
- STREETS topluluğunun kolektif deneyiminden çıkarılan dersler

2027 baskısı için Ocak'ta görüşürüz.

---

## 4DA Entegrasyonu: İstihbarat Katmanın

> **4DA Entegrasyonu:** 4DA'nın günlük brifingi sabah iş istihbarat raporun olur. Nişinde ne teslim edildi? Hangi rakip yeni yayınladı? Hangi framework ivme kazanıyor? Hangi düzenleme kabul edildi? Hangi API fiyatlandırmasını değiştirdi?
>
> STREETS'te başarılı olan geliştiriciler en iyi radara sahip olanlardır. Danışmanlık fırsatını Upwork'te görünmeden önce görürler. Ürün boşluğunu bariz hale gelmeden önce görürler. Trendi ana akım olmadan önce görürler.
>
> 4DA o radardır.
>
> Özellikle bu modülde:
> - **Sinyal algılama** volanını besler — tek bir istihbarat sinyali aynı anda her akışta fırsat üretebilir.
> - **Trend analizi** üç aylık öldür/büyüt kararlarını bilgilendirir — nişin genişliyor mu daralıyor mu?
> - **Rekabet istihbaratı** fiyatları ne zaman artıracağını, ne zaman farklılaşacağını ve ne zaman pivot yapacağını söyler.
> - **İçerik küratörlüğü** bülten ve blog araştırma zamanını %60-80 keser.
> - **Günlük brifing** sosyal medyanın gürültüsü olmadan seni güncel tutan 5 dakikalık sabah ritüelindir.
>
> 4DA bağlamını akış yığını anahtar kelimelerinle kur. Her sabah günlük brifingi incele. Önemli sinyallere göre hareket et. Gerisini görmezden gel.
>
> Rig'in istihbarat üretir. Akışların gelir üretir. 4DA onları birbirine bağlar.

---

## Son Söz

On altı hafta önce bilgisayarı ve becerileri olan bir geliştiriciydin.

Şimdi egemen altyapın, teknik hendeklerin, gelir motorların, uygulama disiplinin, istihbarat katmanın, taktik otomasyonun ve 12 aylık planla istiflenmiş akış portföyün var.

Bunların hiçbiri risk sermayesi, kurucu ortak, bilgisayar bilimi diploması veya birinin izni gerektirmedi. Zaten sahip olduğun bir bilgisayar, zaten sahip olduğun beceriler ve rig'ine tüketici ürünü yerine iş varlığı olarak davranma isteği gerektirdi.

Sistem inşa edildi. Playbook tamamlandı. Gerisi uygulama.

---

> "Sokak bilgisayar bilimi diplomanı umursamaz. Ne inşa edebileceğini, teslim edebileceğini ve satabileceğini umursar. Becerilerin zaten var. Rig'in zaten var. Şimdi playbook'un da var."

---

*Rig'in. Kuralların. Gelirin.*

**STREETS Geliştirici Gelir Kursu — Tamamlandı.**
*Modül S (Egemen Kurulum)'dan Modül S (Akışları İstiflemek)'e*
*16 hafta. 7 modül. 42 ders. Tek playbook.*

*Yıllık olarak güncellenir. Sonraki baskı: Ocak 2027.*
*4DA'dan sinyal istihbaratı ile inşa edildi.*
