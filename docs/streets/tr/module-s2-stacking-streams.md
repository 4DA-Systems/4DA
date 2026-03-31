# Modul S: Akis Yigma

**STREETS Gelistirici Gelir Kursu — Ucretsiz Modul (7 Modulun Hepsi 4DA Icinde Ucretsiz)**
*Hafta 14-16 | 6 Ders | Cikti: Akis Yiginin (12 Aylik Gelir Plani)*

> "Bir akis bir ek is. Uc akis bir isletme. Bes akis ozgurluk."

---

{? if progress.completed("T") ?}
Cogu gelistiricinin asla insa etmedigi bir sey insa etmek icin on uc hafta harcadin: egemen bir gelir operasyonu. Altyapin var. Hendeklerin var. Calisan gelir motorlarin var. Yurutme disiplinin var. Istihbaratin var. Otomasyonun var.
{? else ?}
Cogu gelistiricinin asla insa etmedigi bir sey insa etmek icin on uc hafta harcadin: egemen bir gelir operasyonu. Altyapin var. Calisan gelir motorlarin var. Yurutme disiplinin var. Istihbaratin var. Otomasyonun var. (Bu moduldeki hendek tabanli stratejileri tam olarak etkinlestirmek icin Modul T — Teknik Hendekler'i tamamla.)
{? endif ?}

Simdi ayda ekstra {= regional.currency_symbol | fallback("$") =}2K kazanan gelistiriciyi maasini tamamen degistirebilenden ayiran kisim geliyor: **yigma**.

Tek bir gelir akisi — ne kadar iyi olursa olsun — kirilgandir. En buyuk musterim gider. Platform API fiyatlarini degistirir. Bir algoritma degisikligi trafiginizi dusurur. Bir rakip urunun ucretsiz bir surumunu piyasaya surar.

Birden fazla gelir akisi sadece toplanmaz. Birbirini guclendirir. Herhangi bir tek akisi kaybetmenin felaket degil, rahatsizlik oldugu bir sistem yaratir.

Bu modul o sistemi tasarlamakla ilgili.

Bu uc haftanin sonunda elinde olacak:

- Bes gelir akisi kategorisinin ve nasil etkilestiklerinin net anlasilmasi
- Gercek sayilar ve gercekci zaman cizelgeleri ile ayda $10K'ya birden fazla somut yol
- Dusuk performansli akislari ne zaman oldurecegine karar vermek icin bir cerceve
- Erken geliri hizlanan buyumeye donusturen bir yeniden yatirim stratejisi
- Tamamlanmis bir Akis Yigini belgesi — aylik kilometre taslariyla kisisel 12 aylik gelir planin

Bu son modul. STREETS'te insa ettigin her sey burada yakinlasiyor.

{? if progress.completed_modules ?}
> **STREETS ilerlemeniz:** {= progress.total_count | fallback("7") =} modulden {= progress.completed_count | fallback("0") =} tamamlandi ({= progress.completed_modules | fallback("henuz hicbiri") =}). Bu modul onceki modullerden her seyi bir araya getirir.
{? endif ?}

Yigmaya baslayalim.

---

## Ders 1: Gelir Portfoyu Kavrami

*"Gelirini bir yatirim portfoyu gibi ele al — cunku tam olarak odur."*

### 5 Akis Kategorisi

{@ insight engine_ranking @}

```
Akis 1: Hizli Nakit        — Serbest/danismanlik     — faturalari SIMDI oder
Akis 2: Buyuyen Varlik     — SaaS/urun               — 6 ayda faturalari oder
Akis 3: Icerik Birikimi    — Blog/bulten/YT           — 12 ayda faturalari oder
Akis 4: Pasif Otomasyon    — Botlar/API/veri           — sen uyurken oder
Akis 5: Hisse Oyunu        — Acik kaynak -> sirket     — uzun vadeli servet
```

**Akis 1: Hizli Nakit (Serbest / Danismanlik)**
- Gelir zaman cizelgesi: Ilk dolara 1-2 haftada $0
- Tipik aralik: 10-20 saat/haftada $2.000-15.000/ay
- Risk: musteri yogunlasmasi, solen-veya-kilik donguler

**Akis 2: Buyuyen Varlik (SaaS / Urun)**
- Gelir zaman cizelgesi: Anlamli gelire 3-6 ay
- Tipik aralik: 12-18 ayda $500-5.000/ay

**Akis 3: Icerik Birikimi (Blog / Bulten / YouTube)**
- Gelir zaman cizelgesi: Anlamli gelire 6-12 ay

**Akis 4: Pasif Otomasyon (Botlar / API / Veri Urunleri)**

{? if profile.gpu.exists ?}
> **Donanim avantaji:** {= profile.gpu.model | fallback("GPU") =}'un {= profile.gpu.vram | fallback("ayrilmis") =} VRAM ile LLM tabanli otomasyon akislarini acar — yerel cikarsama API'leri, YZ destekli veri isleme ve akilli izleme hizmetleri — hepsi istek basina neredeyse sifir marjinal maliyetle.
{? endif ?}

- Tipik aralik: {= regional.currency_symbol | fallback("$") =}300-3.000/ay

**Akis 5: Hisse Oyunu (Acik Kaynaktan Sirkete)**
- Gelir zaman cizelgesi: Anlamli gelire 12-24 ay
- Risk: tum kategorilerin en yuksegi

### Zaman Dagitimi

| Akis Kategorisi | Bakim Asamasi | Buyume Asamasi | Insa Asamasi |
|-----------------|---------------|----------------|--------------|
| Hizli Nakit | 2-5 saat/hafta | 5-10 saat/hafta | 10-20 saat/hafta |
| Buyuyen Varlik | 3-5 saat/hafta | 8-15 saat/hafta | 15-25 saat/hafta |
| Icerik Birikimi | 3-5 saat/hafta | 5-10 saat/hafta | 8-15 saat/hafta |
| Pasif Otomasyon | 1-2 saat/hafta | 3-5 saat/hafta | 8-12 saat/hafta |
| Hisse Oyunu | 5-10 saat/hafta | 15-25 saat/hafta | 30-40 saat/hafta |

> **Yaygin Hata:** Ay 2'ni baskasinin Ay 24'u ile karsilastirmak. Her akisin bir yukselis donemı var. Bunun icin planla. Butcele.

---

## Ders 2: Akislar Nasil Etkilesir (Volan Etkisi)

*"Akislar toplanmaz — carpar. Etkilesim icin tasarla, bagimsizlik icin degil."*

### Baglanti 1: Danismanlik Urun Fikirlerini Besler

Her danismanlik isi pazar arastirmasidir. Musteriler sana — parayla — tam olarak hangi sorunlarin var oldugunu soyluyor.

**"Uc Kisi Kurali":** Uc farkli musteri ayni seyi istiyorsa, bunu urun olarak insa et.

### Baglanti 2: Icerik Danismanlik Potansiyel Musterileri Ceker

### Baglanti 3: Urunler Icerik Yaratir

### Baglanti 4: Otomasyon Her Seyi Destekler

### Baglanti 5: Istihbarat Her Seyi Baglar

{? if settings.has_llm ?}
> **LLM'in ({= settings.llm_provider | fallback("Yerel") =} / {= settings.llm_model | fallback("modelin") =}) bu baglantiyi guclendirir.** Sinyal algilama, icerik ozeti, potansiyel musteri nitelendirme ve firsat siniflandirma — LLM'in ham bilgiyi tum akislarda ayni anda uygulanabilir istihbarata donusturur.
{? endif ?}

> **Yaygin Hata:** Akislari maksimum etkilesim yerine maksimum gelir icin tasarlamak. Ayda {= regional.currency_symbol | fallback("$") =}800 ureten VE diger iki akisi besleyen bir akis, izole olarak ayda {= regional.currency_symbol | fallback("$") =}2.000 ureten bir akistan daha degerlidir.

---

## Ders 3: Ayda $10K Kilometre Tasi

*"Ayda $10K bir hayal degil. Matematik problemi. Iste cozmek icin dort yol."*

### Yol 1: Danismanlik Agirlikli
| Akis | Matematik | Aylik |
|------|----------|-------|
| Danismanlik | 10 saat/hafta x $200/saat | $8.000 |
| Urunler | 50 musteri x $15/ay | $750 |
| Icerik | Bulten ortaklik geliri | $500 |
| Otomasyon | API urunu | $750 |
| **Toplam** | | **$10.000** |

### Yol 2: Urun Agirlikli
| Akis | Matematik | Aylik |
|------|----------|-------|
| SaaS | 200 musteri x $19/ay | $3.800 |
| Dijital urunler | 100 satis/ay x $29 | $2.900 |
| Icerik | YouTube + bulten | $2.000 |
| Danismanlik | 3 saat/hafta x $250/saat | $3.000 |
| **Toplam** | | **$11.700** |

### Yol 3: Icerik Agirlikli
| Akis | Matematik | Aylik |
|------|----------|-------|
| YouTube | 50K abone, reklamlar + sponsorlar | $3.000 |
| Bulten | 10K abone, %5 ucretli x $8/ay | $4.000 |
| Kurs | 30 satis/ay x $99 | $2.970 |
| Danismanlik | 2 saat/hafta x $300/saat | $2.400 |
| **Toplam** | | **$12.370** |

### Yol 4: Otomasyon Agirlikli
| Akis | Matematik | Aylik |
|------|----------|-------|
| Veri urunleri | 200 abone x $15/ay | $3.000 |
| API hizmetleri | 100 musteri x $29/ay | $2.900 |
| Otomasyon-Hizmet-Olarak | 2 musteri x $1.500/ay | $3.000 |
| Dijital urunler | Pasif satislar | $1.500 |
| **Toplam** | | **$10.400** |

{? if stack.primary ?}
> **Yiginin ({= stack.primary | fallback("ana yiginin") =}) temelinde:** Mevcut becerilerini en iyi kullanan yolu dusun.
{? endif ?}

{@ temporal market_timing @}

---

## Ders 4: Bir Akisi Ne Zaman Oldurmeli

*"Isteki en zor beceri ne zaman birakilacagini bilmektir. Ikinci en zor olanı gercekten yapmaktir."*

### Dort Oldurme Kurali

**Kural 1: $100 Kurali**
Bir akis 6 ay tutarli caba sonrasinda ayda $100'dan az uretiyorsa, oldur veya dramatik sekilde pivot yap.

**Kural 2: ROI Kurali**
Zamaninin ROI'si diger akislarına kiyasla negatifse, otomatlestir veya oldur.

**Kural 3: Enerji Kurali**
Isi yapmaktan nefret ediyorsan, akisi oldur — karli olsa bile.

**Kural 4: Firsat Maliyeti Kurali**
Akis A'yi oldurmek Akis B'yi 3 katina cikarmak icin zaman serbest birakiyorsa, Akis A'yi oldur.

### Gelistiriciler Icin Batik Maliyet Tuzagi

200 saat harcadin bir sey insa ederek. Kod zarif. Mimari temiz. Ve kimse almiyor.

Kodun degerli degil. Zamanin degerli. 200 saat bundan sonra ne yaparsan yap gitti.

> **Yaygin Hata:** Oldurmek yerine pivot yapmak. Bazen pivot calisir. Ama cogu zaman pivot sadece daha yavas bir olumdur.

---

## Ders 5: Yeniden Yatirim Stratejisi

*"Ilk $500 ile ne yaptigin, ilk $50.000 ile ne yaptigindan daha onemli."*

### Seviye 1: Ilk {= regional.currency_symbol | fallback("$") =}500/Ay
**Vergi rezervi: {= regional.currency_symbol | fallback("$") =}150/ay (%30)**
**Yeniden yatirim: $100-150/ay**
**Cebin: $200-250/ay**

### Seviye 2: Ilk $2.000/Ay
**Yeniden yatirim: $400-600/ay**
- Teknik olmayan gorevler icin sanal asistan: $500-800/ay

### Seviye 3: Ilk $5.000/Ay
**Yeniden yatirim: $1.000-1.500/ay**

### Seviye 4: Ilk {= regional.currency_symbol | fallback("$") =}10.000/Ay

{@ insight cost_projection @}

Soru: **"Sonraki {= regional.currency_symbol | fallback("$") =}10K'ya darbogaz nedir?"**

### Evrensel vergi tavsiyeleri:
1. Brut gelirin %30'unu geldigi gun ayir.
2. Her is harcamasini birinci gunden itibaren izle.
3. Ayda $5K'yi gectiginde muhasebeci al.
4. Kisisel ve is fonlarini asla karistirma.

---

## Ders 6: Akis Yiginini (12 Aylik Plan)

*"Plansiz hedef bir dilektir. Kilometre tassiz plan bir fantazi. Iste gerceklik."*

### Cikti

Tum STREETS kursunun son alıstirmasi. Insa ettigin her sey — altyapi, hendekler, gelir motorlari, yurutme disiplini, istihbarat, otomasyon — tek bir belgede birlesiyor: Akis Yiginini.

### Aylik Inceleme Kadansi

**Aylik inceleme (30 dakika, her ayin ilk Pazartesi):**
1. Her akis icin gercek gelirleri guncelle
2. Her akis icin gercek saatleri guncelle
3. Her akis icin saat basina ROI hesapla
4. Oldurme kriterlerini gercek verilere karsi kontrol et
5. Bu ay ele alinacak bir darbogaz belirle

> **Yaygin Hata:** Tum akislari ayni anda baslatmak. Bir tanesinde anlamli ilerleme yerine hepsinde sifir ilerleme yapacaksin. Sirayla baslatma, paralel degil.

---

## STREETS Mezunu

### Tam Yolculuk

**S — Egemen Kurulum:** Altyapin bir is varligina donustu.
**T — Teknik Hendekler:** Uzmanlıgin bir hendege donustu.
**R — Gelir Motorlari:** Becerilerin urunlere donustu.
**E — Yurutme Oyun Kitabi:** Urunlerin tekliflere donustu.
**E — Gelisen Avantaj:** Istihbaratin bir avantaja donustu.
**T — Taktik Otomasyon:** Sistemlerin otonom hale geldi.
**S — Akis Yigma:** Akislarin bir isletmeye donustu.

### Uzun Oyun

STREETS "cabuk zengin ol" sistemi degil. "12-24 ayda ekonomik egemenlige ulas" sistemidir.

Ekonomik egemenlik su anlama gelir:
- Herhangi bir tek gelir kaynagindan — isverenin dahil — finansal panik yasamadan ayrilabilirsin
- Altyapini, verilerini, musteri iliskilerini ve zamanini kontrol edersin
- Hicbir tek platform, musteri, algoritma veya sirket gelirini bir gecede yikamaz

Sistemler piyango biletlerini yener. Her zaman. Her zaman diliminde.

---

## Son Soz

On alti hafta once, bir bilgisayari ve becerileri olan bir gelistiricidin.

Simdi egemen altyapin, teknik hendeklerin, gelir motorlarin, yurutme disiplinin, istihbarat katmanin, taktik otomasyonun ve 12 aylik planli yigilmis akis portfoyun var.

Bunlarin hicbiri girisim sermayesi, kurucu ortak, bilgisayar bilimi diplomasi veya kimsenin izni gerektirmedi.

Sistem insa edildi. Oyun kitabi tamam. Gerisi yurutme.

---

> "Sokak bilgisayar bilimi diplanini umursamiyor. Neyi insa edebileceginizi, piyasaya surebileceginizi ve satabileceginizi umursuyor. Becerilerin zaten var. Rigin zaten var. Simdi oyun kitabin da var."

---

*Rigin. Kurallarin. Gelirin.*

**STREETS Gelistirici Gelir Kursu — Tamamlandi.**
*Modul S (Egemen Kurulum) ile Modul S (Akis Yigma) arasi*
*16 hafta. 7 modul. 42 ders. Bir oyun kitabi.*

*Yillik guncellenir. Sonraki baski: Ocak 2027.*
*4DA'dan sinyal istihbarati ile insa edildi.*
