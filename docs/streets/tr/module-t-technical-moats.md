# Modul T: Teknik Hendekler

**STREETS Gelistirici Gelir Kursu — Ucretli Modul**
*Hafta 3-4 | 6 Ders | Cikti: Hendek Haritan*

> "Metalasamayan beceriler. Rekabetten korunan nisler."

---

{? if progress.completed("S") ?}
Modul S sana altyapiyi verdi. Rigin, yerel LLM yiginin, yasal temellerin, butcen ve Egemen Yigin Belgen var. Bunlar temel. Ama duvarsiz temel sadece bir beton levha.
{? else ?}
Modul S altyapiyi kapsıyor — rigin, yerel LLM yigini, yasal temeller, butce ve Egemen Yigin Belgesi. Bunlar temel. Ama duvarsiz temel sadece bir beton levha. (Bu modulden maksimum deger icin once Modul S'i tamamla.)
{? endif ?}

Bu modul duvarlar hakkinda. Ozellikle, rakipleri dista tutan ve surekli omzunun uzerinden bakmadan premium fiyatlar almana izin veren tur duvarlar.

Is dunyasinda bu duvarlara "hendekler" denir. Warren Buffett terimi sirketler icin populer hale getirdi — bir isletmeyi rekabetten koruyan kalici rekabet avantaji. Ayni kavram bireysel gelistiriciler icin de gecerli, ama kimse bunu boyle anlatmiyor.

Anlatmaliler.

Yan projelerden {= regional.currency_symbol | fallback("$") =}500/ay kazanan bir gelistirici ile {= regional.currency_symbol | fallback("$") =}5.000/ay kazanan arasindaki fark neredeyse hicbir zaman saf teknik beceri degil. Konumlandirma. Hendek.

Bu iki haftanin sonunda elinde olacak:

- T-seklindeki beceri profilinin ve nerede benzersiz deger yarattginin net bir haritasi
- Bes hendek kategorisinin anlasilmasi ve hangilerinin sana uygun oldugu
- Nisleri secmek ve dogrulamak icin pratik bir cerceve
- 2026'ya ozgu su anda mevcut hendeklerin bilgisi
- Pahali araclar gerektirmeyen bir rekabet istihbarati is akisi
- Tamamlanmis bir Hendek Haritasi — kisisel konumlandirma belgen

{? if dna.is_full ?}

{@ mirror blind_spot_moat @}

{? endif ?}

Duvarlarini insa edelim.

---

## Ders 1: Gelir Icin T-Seklindeki Gelistirici

*"Bir alanda derin, bircogunda yetkin. Emtia fiyatlarindan boyle kacilir."*

### Jeneralistler Neden Ac Kalir

"Her seyden biraz" yapabiliyorsan — biraz React, biraz Python, biraz DevOps — ayni seyi yapabilen diger her gelistiriciyle rekabet ediyorsun. Bu milyonlarca insan demek. Arz bu kadar buyukken fiyat duser.

| Beceri Tanimi | Tipik Serbest Tarife | Mevcut Rekabet |
|---------------|----------------------|----------------|
| "Full-stack web gelistirici" | $30-60/saat | Sadece Upwork'te 2M+ |
| "Python gelistirici" | $25-50/saat | 1.5M+ |
| "WordPress gelistirici" | $15-35/saat | 3M+ |
| "Her seyi yaparim" | $20-40/saat | Herkes |

Jeneralistlerin fiyat gucu yoktur.

### T-Sekli: Paranin Oldugu Yer

{@ insight t_shape @}

T'nin yatay cubugu senin genisligin — yetkin oldugun bitisik beceriler. Dikey cubuk derinligin — gercekten uzman oldugun alan.

{? if stack.primary ?}
**Sihir kesisimde olur.** Ana yiginin {= stack.primary | fallback("ana yiginin") =}. Bitisik becerilerinle ({= stack.adjacent | fallback("bitisik alanlarin") =}) birlestirdiginde, bu bir konumlandirma temeli yaratir. Soru su: senin ozel kombinasyonun ne kadar nadir? Bu kitlik fiyat gucu yaratir.
{? endif ?}

Premium tarifeler komuta eden gercek T-seklindeki konumlandirma ornekleri:

| Derin Uzmanlik | Bitisik Beceriler | Konumlandirma | Tarife Araligi |
|----------------|-------------------|---------------|----------------|
| Rust sistem programlama | Docker, Linux, GPU compute | "Yerel YZ altyapi muhendisi" | $200-350/saat |
| React + TypeScript | Tasarim sistemleri, erisilebilirlik | "Kurumsal UI mimari" | $180-280/saat |
| PostgreSQL dahili | Veri modelleme, Python, ETL | "Veritabani performans uzmani" | $200-300/saat |
| NLP + makine ogrenimi | Saglik domaini, HIPAA | "Saglik YZ uygulama uzmani" | $250-400/saat |

### Benzersiz Kombinasyon Ilkesi

Hendegin bir seyde en iyi olmaktan gelmez. Cok az kisinin paylatigi bir beceri kombinasyonuna sahip olmaktan gelir.

Matematiksel dusun:
- 500.000 gelistirici React'i iyi biliyor
- 50.000 gelistirici saglik veri standartlarini anlıyor
- 10.000 gelistirici yerel YZ modellerini dagitabiliyor

Her biri kalabaliık bir pazar. Ama:
- React + saglik + yerel YZ? Bu kesisim dunyada belki 50 kisi olabilir.

> **Acik Konusalim:** "Benzersiz kombinasyonun" egzotik olmak zorunda degil. "Python + onceki kariyerden ticari gayrimenkulun nasil calistigini anliyor" yikici olcude etkili bir kombinasyondur. Iki dunya arasinda cevirmensin. Cevirmenlere odenir.

### Ders 1 Kontrol Noktasi

Artik elinde olmali:
- [ ] 1-3 derin beceri belirlenmis
- [ ] 5-10 bitisik beceri listelenmis
- [ ] 3-5 teknik olmayan bilgi alani belgelenmis
- [ ] 3+ benzersiz kesisim kombinasyonu yazilmis

---

## Ders 2: Gelistiriciler Icin 5 Hendek Kategorisi

*"Sadece bes tur duvar var. Hangilerini insa edebileceğini bil."*

{@ insight stack_fit @}

### Kategori 1: Entegrasyon Hendekleri
Birbiriyle konusmayan sistemleri bagliyorsun. Iki ekosistem arasinda kopruysun.

### Kategori 2: Hiz Hendekleri
Ajanslarin 2 haftada yaptigini 2 saatte yapiyorsun.

### Kategori 3: Guven Hendekleri
Belirli bir niste tanınan uzmansin. O nisteki insanlarin sorunu oldugunda senin adın gundeme gelir.

**"3 Blog Yazisi" kurali:** Cogu mikro-niste 3'ten az derin teknik makale vardir. Dar bir teknik konuda 3 mukemmel yazi yaz — Google bunlari gosterecek. 3-6 ay icinde "X hakkinda yazan kisi" olursun.

### Kategori 4: Veri Hendekleri
Rakiplerin kolayca kopyalayamayacagi veri setlerine, hatlara veya veri turetilmis ici gorulere erisiminiz var.

### Kategori 5: Otomasyon Hendekleri
Zamanla biriken betikler, araclar ve otomasyon is akislari kutuphanesi insa ettin.

### Hendek Kategorilerini Birlestirme

| Hendek Kombinasyonu | Ornek | Guc |
|---------------------|-------|-----|
| Entegrasyon + Guven | "Clio'yu her seye baglayan kisi" (ve bunun hakkinda yaziyor) | Cok guclu |
| Hiz + Otomasyon | Biriken arac setine dayali hizli teslimat | Guclu, zamanla birikir |
| Veri + Guven | Benzersiz veri seti + yayinlanmis analiz | Cok guclu, cok zor kopyalanir |
| Entegrasyon + Otomasyon | Sistemler arasi otomatik kopru, SaaS olarak paketlenmis | Guclu, olceklenebilir |
| Guven + Hiz | Taninmis uzman ayni zamanda hizli teslim eder | Premium fiyat alanı |

### Ders 2 Kontrol Noktasi

Artik anlamis olmalisin:
- [ ] Bes hendek kategorisi: Entegrasyon, Hiz, Guven, Veri, Otomasyon
- [ ] Hangi kategorilerin mevcut guclerin ile eslestigi
- [ ] Ilk once hangi hendek turunu insa etmek istedigin

---

## Ders 3: Nis Secim Cercevesi

*"Her sorun cozmeye degmez. Odeyenleri boyle bulursun."*

### 4 Soru Filtresi

**Soru 1:** "Biri bu sorunu cozmek icin {= regional.currency_symbol | fallback("$") =}50 oder mi?"
**Soru 2:** "40 saatten kisa surede cozum insa edebilir miyim?"
**Soru 3:** "Bu cozum birikir mi (zamanla daha iyi veya daha degerli olur mu)?"
**Soru 4:** "Pazar buyuyor mu?"

### Nis Puanlama Matrisi

Her potansiyel nisi her boyutta 1-5 arasi puanla. Puanlari carp.

```
+-------------------------------------------------------------------+
| NIS DEGERLENDIRME KARTI                                            |
+-------------------------------------------------------------------+
| Nis: _________________________________                             |
|                                                                    |
| ACI YOGUNLUGU           (1=hafif rahatsizlik, 5=acil durum)  [  ] |
| ODEME ISTEKLILIGI        (1=ucretsiz bekliyor, 5=para atiyor)[  ] |
| INSA EDILEBILIRLIK (<40s)(1=dev proje, 5=hafta sonu MVP)     [  ] |
| BIRIKIM POTANSIYELI      (1=bir defalik, 5=kartopu etkisi)   [  ] |
| PAZAR BUYUMESI           (1=daralıyor, 5=patlıyor)           [  ] |
| KISISEL UYUM             (1=domainı nefret, 5=takilmis)      [  ] |
| REKABET                  (1=kirmizi okyanus, 5=mavi okyanus) [  ] |
|                                                                    |
| TOPLAM PUAN (hepsini carp):  ___________                           |
| Guclu nis: 5.000+                                                 |
| Yasayabilir nis: 1.000-5.000                                      |
| Zayif nis: 1.000 alti                                             |
+-------------------------------------------------------------------+
```

### Ders 3 Kontrol Noktasi

Artik elinde olmali:
- [ ] 4 soru filtresinin anlasilmasi
- [ ] En az 3 potansiyel nis icin tamamlanmis puanlama matrisi
- [ ] Puanlara dayali net bir on aday

---

## Ders 4: 2026'ya Ozgu Hendekler

*"Bu hendekler simdi var cunku pazar yeni. Sonsuza kadar surmeyecek. Hareket et."*

2026'da benzersiz olarak mevcut yedi hendek:

### 1. MCP Sunucu Gelistirme
MCP 2025 sonunda basladi. Bugun yaklasik 2.000 MCP sunucusu var. 50.000+ olmali.

### 2. Yerel YZ Dagitim Danismanligi
AB YZ Yasasi simdi uygulamada. Sirketlerin veri yonetisimini gostermesi gerekiyor.

### 3. Gizlilik-Oncelikli SaaS
Tauri 2.0 gibi cerceveler yerel-oncelikli masaustu uygulamalar olusturamayi dramatik olarak kolaylastiriyor.

### 4. YZ Ajan Orkestrasyonu
Herkes tek bir LLM cagrisi yapabilir. Az kisi cok adimli is akislarini guvenilir sekilde yonetebilir.

### 5. Nis Alanlar Icin LLM Ince Ayari
LoRA ve QLoRA, ince ayari tuketici GPU'larinda (12GB+ VRAM) eriselebilir hale getirdi.

### 6. Tauri / Masaustu Uygulama Gelistirme
Tauri 2.0 olgun ve kararli. Tauri gelistirici havuzu kucuk — dunyada belki 10.000-20.000 aktif.

### 7. Gelistirici Araclari (CLI, Eklentiler, Pluginler)
YZ kodlama araclari yeni uzanti noktalari yaratir. MCP yeni bir dagitim kanali yaratir.

### Ders 4 Kontrol Noktasi

Artik elinde olmali:
- [ ] Yedi 2026'ya ozgu hendegin hepsinin anlasilmasi
- [ ] T-sekline uyan 1-2 hendek belirlenmis
- [ ] BU HAFTA alinacak somut bir eylem

---

## Ders 5: Rekabet Istihbarati (Urkutucu Olmadan)

*"Neyin var oldugunu, neyin kirik oldugunu ve boşluklarin nerede oldugunu bil — insa etmeden once."*

### Arastirma Yigini

**Arac 1: GitHub — Arz Tarafi**
**Arac 2: npm/PyPI/crates.io Indirme Trendleri — Talep Tarafi**
**Arac 3: Google Trends — Ilgi Tarafi**
**Arac 4: Similarweb Free — Rekabet Tarafi**
**Arac 5: Reddit / HN / StackOverflow — Aci Tarafi**

### Boşluklari Bulma

| Bosluk Turu | Sinyal | Firsat |
|-------------|--------|--------|
| **Hicbir sey yok** | Arama 0 sonuc donduruyor | Ilkini insa et |
| **Var ama terk edilmis** | 500 yildizli GitHub reposu, son commit 18 ay once | Fork'la veya yeniden insa et |
| **Var ama berbat** | Arac var, 3 yildiz degerlendirmeler | Daha iyi versiyonu insa et |
| **Var ama pahali** | Basit sorun icin $200/ay kurumsal arac | $19/ay bagimsiz versiyonu insa et |
| **Var ama sadece bulut** | Sunuculara veri gondermeyi gerektiren SaaS araci | Yerel-oncelikli versiyonu insa et |
| **Var ama manuel** | Surec calisiyor ama saatlerce insan cabasi gerektiriyor | Otomatlestir |

{@ insight competitive_position @}

---

## Ders 6: Hendek Haritan

*"Haritasiz hendek sadece bir cayirdır. Belgele. Dogrula. Uygula."*

### Hendeginizi Dogrulama

**3 Kisi Dogrulama Yontemi:**

1. Hedef kitlenden 5-10 kisi belirle
2. Dogrudan iletisime gec
3. Teklifini 2-3 cumlede tanimla
4. Sor: "Bu olsaydi, bunun icin $[fiyatin] oder miydin?"
5. En az 3/5 evet derse (belki degil — evet), nisin dogrulanmistir

> **Yaygin Hata:** Arkadaslar ve aileden dogrulama istemek. "Harika fikir!" diyecekler cunku seni seviyorlar, almak istedikleri icin degil.

### Ders 6 Kontrol Noktasi

Artik elinde olmali:
- [ ] Tamamlanmis Hendek Haritasi belgesi
- [ ] Gercek verilerle doldurulmus 7 bolumun hepsi
- [ ] Haftalik eylemleri olan 90 gunluk yurutme plani
- [ ] Tanimlanmis terk kriterleri
- [ ] Dogrulama plani: bu hafta iletisime gecilinecek 3-5 kisi

---

## Modul T: Tamamlandi

{? if progress.completed_modules ?}
> **Ilerleme:** {= progress.total_count | fallback("7") =} STREETS modulunden {= progress.completed_count | fallback("0") =}'ini tamamladin ({= progress.completed_modules | fallback("henuz hicbiri") =}). Modul T tamamlanan setine katildi.
{? endif ?}

### Iki Haftada Ne Insa Ettin

1. **T-seklindeki beceri profili** — pazardaki benzersiz degerini tanimlayan.
2. **Bes hendek kategorisinin anlasilmasi** ve hangi tur duvar insa ettiginle ilgili net bir secim.
3. **Dogrulanmis bir nis** — titiz bir puanlama cercevesiyle secilmis.
4. **2026'ya ozgu firsat farkindaligi.**
5. **Gercek arastirmaya dayali rekabet ortami belgesi.**
6. **Hendek Haritasi** — kisisel konumlandirma belgen.

### Tam STREETS Yol Haritasi

| Modul | Baslik | Odak | Sure | Durum |
|-------|--------|------|------|-------|
| **S** | Egemen Kurulum | Altyapi, hukuk, butce | Hafta 1-2 | Tamamlandi |
| **T** | Teknik Hendekler | Savunulabilir avantajlar, konumlandirma | Hafta 3-4 | Tamamlandi |
| **R** | Gelir Motorlari | Kodlu belirli monetizasyon oyun kitaplari | Hafta 5-8 | Siradaki |
| **E** | Yurutme Oyun Kitabi | Lansman siralari, fiyatlandirma | Hafta 9-10 | |
| **E** | Gelisen Avantaj | Onde kalmak, trend algilama | Hafta 11-12 | |
| **T** | Taktik Otomasyon | Pasif gelir icin operasyonlari otomatlestirme | Hafta 13-14 | |
| **S** | Akis Yigma | Birden fazla gelir kaynagi, portfoy stratejisi | Hafta 15-16 | |

---

**Temeli insa ettin. Hendeginini belirledin. Simdi konumlandirmayi gelire donusturen motorlari insa etme zamani.**

Modul R gelecek hafta basliyor. Hendek Haritani getir. Ihtiyacin olacak.

*Rigin. Kurallarin. Gelirin.*
