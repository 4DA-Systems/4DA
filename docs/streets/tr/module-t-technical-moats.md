# Modul T: Teknik Hendekler

**STREETS Gelistirici Gelir Kursu — Ucretli Modul**
*Haftalar 3-4 | 6 Ders | Teslim Edilecek: Hendek Haritan*

> "Metalasamayan beceriler. Rekabet edilemeyen nisler."

---

{? if progress.completed("S") ?}
Modul S sana altyapiyi verdi. Bir donanim setin, yerel bir LLM yigini, hukuki temeller, bir butce ve bir Egemen Yigin Belgen var. Bu temel. Ama duvarsiz bir temel sadece bir beton plakadan ibarettir.
{? else ?}
Modul S altyapiyi kapsiyor — donanim setin, yerel bir LLM yigini, hukuki temeller, bir butce ve bir Egemen Yigin Belge. Bu temel. Ama duvarsiz bir temel sadece bir beton plakadan ibarettir. (Bu modulden maksimum deger elde etmek icin once Modul S'yi tamamla.)
{? endif ?}

Bu modul duvarlar hakkinda. Ozellikle, rakipleri disarida tutan ve surekli omzunun uzerinden bakmadan premium fiyatlar talep etmeni saglayan turden duvarlar.

Is dunyasinda bu duvarlara "hendekler" denir. Warren Buffett terimi sirketler icin populerlestirdi — bir isletmeyi rekabetten koruyan surdurulebilir bir rekabet avantaji. Ayni konsept bireysel gelistiriciler icin de gecerlidir, ama kimse bundan bu sekilde bahsetmiyor.

Bahsetmeliler.

Yan projelerden {= regional.currency_symbol | fallback("$") =}500/ay kazanan bir gelistirici ile {= regional.currency_symbol | fallback("$") =}5.000/ay kazanan arasindaki fark neredeyse hicbir zaman ham teknik beceri degildir. Konumlanmadir. Hendektir. {= regional.currency_symbol | fallback("$") =}5.000/ay'lik gelistirici bir seyler insa etmistir — bir itibar, bir veri seti, bir arac takimi, bir hiz avantaji, baska kimsenin zahmet etmedigi bir entegrasyon — ki bu, rakip ayni donanima ve ayni modellere sahip olsa bile teklifini kopyalanmasi zor kilar.

Bu iki haftanin sonunda sende sunlar olacak:

- T-seklindeki beceri profilinin ve nerede benzersiz deger yarattigi hakkinda net bir harita
- Bes hendek kategorisinin ve hangilerinin sana uygulandigi hakkinda anlayis
- Nis secimi ve dogrulama icin pratik bir cerceve
- Su anda mevcut olan 2026'ya ozgu hendekler hakkinda bilgi
- Pahali araclar gerektirmeyen bir rekabet istihbarati is akisi
- Tamamlanmis bir Hendek Haritasi — kisisel konumlanma belgen

Belirsiz strateji laflarindan eser yok. "Tutkunu bul" kliselerinden eser yok. Somut cerceveler, gercek rakamlar, gercek ornekler.

{? if dna.is_full ?}

{@ mirror blind_spot_moat @}

{? endif ?}

Hadi duvarlarini insa edelim.

---

## Ders 1: T-Seklinde Gelir Gelistiricisi

*"Bir alanda derin, bircogunda yetkin. Emtia fiyatlandirmasindan boyle kacilir."*

### Neden Jeneralistler Ac Kalir

Eger "herseyi biraz" yapabiliyorsan — biraz React, biraz Python, biraz DevOps, biraz veritabani isi — herseyi biraz yapabilen diger tum gelistiricilerle rekabet ediyorsun. Bu milyonlarca kisi demek. Arz bu kadar buyuk oldugunda fiyat duser. Basit ekonomi.

2026'da jeneralistler icin serbest calisan piyasasi soyle gorunuyor:

| Beceri Tanimi | Tipik Serbest Calisan Ucreti | Mevcut Rekabet |
|---|---|---|
| "Full-stack web gelistirici" | $30-60/sa | Sadece Upwork'te 2M+ |
| "Python gelistirici" | $25-50/sa | 1,5M+ |
| "WordPress gelistirici" | $15-35/sa | 3M+ |
| "Her seyi yapabilirim" | $20-40/sa | Herkes |

Bu ucretler yazim hatasi degil. Bu, kuresel bir pazarda farklilastirilmamis teknik becerinin gercekligi. Bangalore, Krakow, Lagos ve Buenos Aires'teki yetenekli gelistiricilerle rekabet ediyorsun; onlar ayni "full-stack web uygulamasini" senin yasam maliyetinin bir kesri karsiliginda teslim edebiliyorlar.

Jeneralistlerin fiyat gucu yoktur. Fiyat alanlardir, fiyat yapanlar degil. Ve 2025-2026'da ortaya cikan yapay zeka kodlama araclari bunu iyilestirmedi, kotulesirdi — Cursor kullanan bir gelistirici olmayan biri artik bir ogleden sonrada temel bir CRUD uygulamasi olusturabiliyor. Emtia gelistirme isinin altindan zemin kaydi.

### Neden Ultra-Uzmanlar Platoya Cikar

Ters uce sallanmak da ise yaramaz. Tum kimligin "Webpack 4 yapilandirmada dunyanin en iyisiyim" ise bir sorunun var. Webpack 4 kullanimi azaliyor. Hedeflenebilir pazarin her yil kuculuyor.

Ultra-uzmanlar uc riskle karsilasir:

1. **Teknoloji eskimesi.** Becerin ne kadar darsa, o teknolojinin degistirilmesine karsi o kadar savunmasizsin.
2. **Pazar tavani.** Tam olarak o tek seye ihtiyac duyan sinirli sayida insan var.
3. **Komsu firsat yakalama eksikligi.** Bir musteri ilgili ama biraz farkli bir seye ihtiyac duydugunda, ona hizmet edemezsin. Baskasina gider.

### T-Sekli: Paranin Oldugu Yer

{@ insight t_shape @}

T-seklindeki gelistirici modeli yeni degil. IDEO'dan Tim Brown tasarimda populerlestirdi. Ama gelistiriciler neredeyse hicbir zaman bunu gelir stratejisine uygulamiyor. Uygulamalari gerekirdi.

T'nin yatay cubugu senin genisligin — yetkin oldugunda komsu beceriler. Yapabilirsin. Kavramlari anliyorsun. Onlar hakkinda akilli bir konusma yuriyebilirsin.

Dikey cubuk senin derinligin — gercekten uzman oldugun bir (veya iki) alan. "Bir projede kullandim" uzmani degil. "Gece 3'te kenar vakalari ayikladim ve bunun hakkinda yazdim" uzmani.

```
Genislik (bircogunda yetkin)
←————————————————————————————————→
  Docker  |  SQL  |  APIs  |  CI/CD  |  Testing  |  Cloud
          |       |        |         |           |
          |       |        |    Derinlik (birinde uzman)
          |       |        |         |
          |       |        |         |
          |       |   Rust + Tauri   |
          |       |  Masaustu Uyg.   |
          |       |  Yerel AI Altyapi|
          |       |        |
```

{? if stack.primary ?}
**Sihir kesisimde gerceklesir.** Birincil yiginin {= stack.primary | fallback("birincil yigininin") =}. {= stack.adjacent | fallback("komsu alanlarindaki") =} komsu becerilerinle birlesirildiginde, bu bir konumlanma temeli olusturur. Soru su: senin spesifik kombinasyonun ne kadar nadir? Bu nadirlik fiyat gucu yaratir.
{? else ?}
**Sihir kesisimde gerceklesir.** "Yerel AI yeteneklerine sahip Rust tabanli masaustu uygulamalar insa ediyorum" binlerce kisinin sahip oldugu bir beceri degil. Yuzlerce olabilir. Belki onlarca. Bu nadirlik fiyat gucu yaratir.
{? endif ?}

Premium ucretler komuta eden T-seklinde konumlanmanin gercek ornekleri:

| Derin Uzmanlik | Komsu Beceriler | Konumlanma | Ucret Araligi |
|---|---|---|---|
| Rust sistem programlama | Docker, Linux, GPU hesaplama | "Yerel AI altyapi muhendisi" | $200-350/sa |
| React + TypeScript | Tasarim sistemleri, erisilebilirlik, performans | "Kurumsal UI mimari" | $180-280/sa |
| PostgreSQL internals | Veri modelleme, Python, ETL | "Veritabani performans uzmani" | $200-300/sa |
| Kubernetes + ag | Guvenlik, uyumluluk, izleme | "Bulut guvenlik muhendisi" | $220-350/sa |
| NLP + machine learning | Saglik alani, HIPAA | "Saglik AI uygulama uzmani" | $250-400/sa |

Son sutunda neler olduguna dikkat et. Bunlar "gelistirici" ucretleri degil. Uzman ucretleri. Ve konumlanma bir yalan ya da abartma degil — gercek, nadir bir beceri kombinasyonunun dogru bir tanimi.

{? if stack.contains("rust") ?}
> **Yigin Avantajin:** Rust gelistiricileri sektordeki en yuksek serbest calisan ucretlerinden bazilarina sahiptir. Rust'in ogrenme egrisi senin hendegin — daha az gelistirici Rust'a ozgu projelerde seninle rekabet edebilir. Maksimum nadirlik icin Rust derinligini yerel AI, gomulu sistemler veya WebAssembly gibi bir alanla eslestirmeyi dusun.
{? endif ?}
{? if stack.contains("python") ?}
> **Yigin Avantajin:** Python genis capta biliniyor, ama spesifik alanlarda Python uzmanligi (ML pipeline'lari, veri muhendisligi, bilimsel hesaplama) hala premium ucretler komuta ediyor. Hendegin tek basina Python'dan gelmeyecek — bir alan eslestirmesine ihtiyaci var. T-seklinin dikeyine odaklan: Python'u hangi alanda uyguluyorsun da baskalar uygulamiyor?
{? endif ?}
{? if stack.contains("typescript") ?}
> **Yigin Avantajin:** TypeScript becerileri yuksek talepte ama ayni zamanda genis capta mevcut. Hendegin TypeScript ile ne insa ettiginden gelmeli, TypeScript'in kendisinden degil. Bir framework nisinde uzmanlasmay dusun (Tauri frontend'leri, ozel tasarim sistemleri, gelistirici araclari) burada TypeScript arac, hedef degil.
{? endif ?}

### Benzersiz Kombinasyon Ilkesi

Hendegin bir seyde en iyi olmaktan gelmiyor. Cok az insanin paylastigi bir beceri kombinasyonuna sahip olmaktan geliyor.

Matematiksel olarak dusun. Diyelim ki:
- React'i iyi bilen 500.000 gelistirici var
- Saglik veri standartlarini anlayan 50.000 gelistirici var
- Yerel AI modellerini dagitabilen 10.000 gelistirici var

Bunlardan herhangi biri kalabalk bir pazar. Ama:
- React + saglik + yerel AI? Bu kesisim dunya capinda 50 kisi olabilir.

Ve bu kombinasyona tam olarak ihtiyac duyan hastaneler, klinikler, saglik teknolojisi sirketleri ve sigorta firmalari var. 3 aylik uyum suresi gerektirmeyen birini bulmak icin ne gerekiyorsa odeyecekler.

> **Acik Konusma:** "Benzersiz kombinasyonun" egzotik olmak zorunda degil. "Python + onceki kariyerden dolayi ticari gayrimenkulun nasil isledigini biliyor" yikici olcude etkili bir kombinasyondur cunku neredeyse hicbir gelistirici ticari gayrimenkul anlamaz ve neredeyse hicbir gayrimenkul profesyoneli kodlayamaz. Iki dunya arasindaki tercumansin. Tercumanlara odenir.

### Alistirma: Kendi T-Seklini Haritalandir

Bir kagit al veya bir metin dosyasi ac. Bu 20 dakika surer. Fazla dusunme.

{? if dna.is_full ?}
> **On Avantaj:** Developer DNA'na gore, birincil yiginin {= dna.primary_stack | fallback("henuz tanimlanmadi") =} ve en cok etkilesime girdigi konular {= dna.top_engaged_topics | fallback("cesitli teknolojiler") =} iceriyor. Asagida bunlari baslangic noktasi olarak kullan — ama kendini 4DA'nin tespit ettikleriyle sinrlama. Teknik olmayan bilgin ve onceki kariyer deneyimin genellikle en degerli girdilerdir.
{? endif ?}

**Adim 1: Derin becerilerini listele (dikey cubuk)**

Bir atolye verebilecegin 1-3 beceri yaz. Belirgin olmayan sorunlari cozdugun yerler. Varsayilan tavsiyelerden farkli goruslerin oldugu yerler.

```
Derin becerilerim:
1. _______________
2. _______________
3. _______________
```

**Adim 2: Komsu becerilerini listele (yatay cubuk)**

Yetkin ama uzman olmadgin 5-10 beceri yaz. Bunlari uretimde kullandin. Bunlari kullanan bir projeye katkida bulunabilirsin. Gerekirse derin kisimlarini ogrenebilirsin.

```
Komsu becerilerim:
1. _______________     6. _______________
2. _______________     7. _______________
3. _______________     8. _______________
4. _______________     9. _______________
5. _______________     10. ______________
```

**Adim 3: Teknik olmayan bilgini listele**

Bu, cogu gelistiricinin atladigi ve en degerli olan adim. Onceki islerden, hobilerden, egitimden veya yasam deneyiminden kodlamayla hicbir ilgisi olmayan ne biliyorsun?

```
Teknik olmayan bilgim:
1. _______________  (orn: "3 yil lojistikte calistim")
2. _______________  (orn: "kucuk isletme yonettigimden muhasebe temellerini anliyorum")
3. _______________  (orn: "Almanca ve Portekizce akici")
4. _______________  (orn: "yarisma bisikleti — spor analizini anliyorum")
5. _______________  (orn: "ozel ihtiyacli cocuk ebeveyni — erisilebirlilik konusunu derinden anliyorum")
```

**Adim 4: Kesisimlerini bul**

Simdi uc listeden de ogeleri birlestir. Baska bir insanda bulmana sasracagin 3-5 olagan disi kombinasyon yaz.

```
Benzersiz kesisimlerim:
1. [Derin beceri] + [Komsu beceri] + [Teknik olmayan bilgi] = _______________
2. [Derin beceri] + [Teknik olmayan bilgi] = _______________
3. [Derin beceri] + [Derin beceri] + [Komsu beceri] = _______________
```

**Adim 5: Fiyatlandirma testi**

Her kesisim icin sor: "Bir sirket tam olarak bu kombinasyona sahip birine ihtiyac duysa, kac kisi bulabilir? Ve ne odemek zorunda kalir?"

Cevap "binlerce kisi, emtia ucretleriyle" ise, kombinasyon yeterince spesifik degil. Daha derine in. Baska bir boyut ekle.

Cevap "belki 50-200 kisi ve muhtemelen {= regional.currency_symbol | fallback("$") =}150+/sa oderler" ise, potansiyel bir hendek buldun.

### Ders 1 Kontrol Noktasi

Simdi sende sunlar olmali:
- [ ] 1-3 derin beceri tanimlanmis
- [ ] 5-10 komsu beceri listelenmis
- [ ] 3-5 teknik olmayan bilgi alani belgelenmis
- [ ] 3+ benzersiz kesisim kombinasyonu yazilmis
- [ ] Hangi kesiismlerin en az rakibe sahip oldugu hakkinda kaba bir fikir

Bu T-sekli haritasini sakla. Ders 2'deki hendek kategorinle birlestirecek ve Ders 6'da Hendek Haritani olusturacaksin.

---

## Ders 2: Gelistiriciler Icin 5 Hendek Kategorisi

*"Sadece bes tur duvar var. Hangilerini insa edebileceğini bil."*

Her gelistirici hendegi bes kategoriden birine girer. Bazilari hizli insa edilir ama kolayca asinir. Digerleri aylar alir ama yillarca dayanir. Kategorileri anlamak, sinirli zamanini nereye yatirman gerektigini secmende yardimci olur.

{@ insight stack_fit @}

### Hendek Kategorisi 1: Entegrasyon Hendekleri

**Nedir:** Birbirleriyle konusmayan sistemleri bagliyorsun. Iki ekosistem, iki API, her birinin kendi dokumantasyonu, kurallari ve tuhafliklari olan iki dunya arasinda koprusun.

**Neden hendek:** Kimse iki set dokumantasyon okumak istemez. Ciddi. Sistem A'nin 200 sayfalik API dokumantasyonu ve Sistem B'nin 300 sayfalik API dokumantasyonu varsa, ikisini de derinlemesine anlayan ve birlikte calismasini saglayan kisi gelecekteki her musteri icin 500 sayfalik okumayi ortadan kaldirmistir. Bunun icin odemeye deger.

**Gercek gelirli gercek ornekler:**

**Ornek 1: Nis Zapier/n8n entegrasyonlari**

Bu senaryoyu dusun: bir gelistirici Clio'yu (hukuk burosu yonetimi) Notion, Slack ve QuickBooks ile baglayan ozel Zapier entegrasyonlari insa ediyor. Hukuk burolari her hafta saatlerce bu sistemler arasinda veriyi manuel kopyaliyor.

- Entegrasyon basina gelistirme suresi: 40-80 saat
- Fiyat: $3.000-5.000 entegrasyon basina
- Devam eden bakim ucreti: $500/ay
- Ilk yil gelir potansiyeli: 8 musteriden $42.000

Hendek: hukuk burosu yonetimi is akislarini anlamak ve hukuk burosu operasyonlarinin dilinden konusmak. Baska bir gelistirici Clio API'sini ogrenebilir, tabi. Ama API'yi ogrenip VE bir hukuk burosunun neden belirli verilerin belirli bir sirada belirli bir zamanda dava yasam dongusu icinde akmasi gerektigini anlamak? Bu, cogu gelistiricinin sahip olmadigi alan bilgisi gerektirir.

> **NOT:** Nis entegrasyonlar icin gercek bir referans noktasi olarak, Plausible Analytics, baskin bir rakibe (Google Analytics) karsi tek bir spesifik kama (gizlilik) sahiplenerek $3,1M ARR'ye ve 12K odeme yapan aboneye ulasti. Nis entegrasyon hamleleris ayni kaliba uyar: baska kimsenin zahmet etmedigi kopruyu sahiplen. (Kaynak: plausible.io/blog)

**Ornek 2: Ekosistemleri baglayan MCP server'lar**

Nasil calistigini gor: bir gelistirici Claude Code'u Pipedrive'a (CRM) baglayan bir MCP server insa ediyor, anlasma arama, asama yonetimi ve tam anlasma baglami alma araclari sunuyor. Server 3 gunde insa ediliyor.

Gelir modeli: kullanici basina $19/ay veya $149/yil. Pipedrive'in 100.000'den fazla odeme yapan sirketi var. %0,1 benimseme bile = 100 musteri = $1.900/ay MRR.

> **NOT:** Bu fiyatlandirma modeli gercek gelistirici araci ekonomisini yansitiyor. Marc Lou'nun ShipFast'i (bir Next.js sablonu) spesifik bir gelistirici ihtiyacini odaklanmis bir urunle hedefleyerek 4 ayda $199-249 fiyat noktasinda $528K'ye ulasti. (Kaynak: starterstory.com)

**Ornek 3: Veri pipeline entegrasyonu**

Bu senaryoyu dusun: bir gelistirici Shopify magazalarindan veri alan ve urun aciklamasi olusturma, SEO optimizasyonu ve musteri e-posta kisiselletirmesi icin yerel LLM'lere besleyen bir hizmet insa ediyor. Entegrasyon Shopify webhook'larini, urun sema eslestirmesini, goruntu islesemi ve cikti bicimlendirmesini hallediyor — hepsi yerel olarak.

- Aylik ucret: magaza basina $49/ay
- 4 ay sonra 30 magaza = $1.470 MRR
- Hendek: Shopify'in veri modelinin derin anlayisi VE yerel LLM dagitimi VE e-ticaret metin yazarligi kaliplari. Uc alan. Bu kesisimde cok az insan var.

> **NOT:** Coklu-alan kesisim hamlelerinin gercek dunya dogrulamasi icin, Pieter Levels Nomad List, PhotoAI ve diger urunleri yonetiyor ve yaklasik $3M/yil uretiyoruz sifir calisanla — her urun, az sayida rakibin kopyalayabilecegi teknik beceri ve nis alan bilgisinin kesisiminde oturuyor. (Kaynak: fast-saas.com)

**Entegrasyon hendegi nasil insa edilir:**

1. Hedef pazarinin birlikte kullandigi iki sistemi sec
2. Su anda nasil baglantiklari hakkindaki aci noktayi bul (genellikle: bagli degillerdir veya CSV disari aktarma ve manuel kopyala-yapistir kullaniyorlardir)
3. Kopruyu insa et
4. Calistan saatlere degil, tasarruf edilen zamana gore fiyatlandir

{? if settings.has_llm ?}
> **LLM Avantajin:** Zaten yapilandirilmis bir yerel LLM'in var. Entegrasyon hendekleri, sistemler arasi AI destekli veri donusumu eklediginde daha da guclu hale gelir. Veriyi A'dan B'ye basitce aktarmak yerine, koprun transit halindeki veriyi akilli bir sekilde esleyebilir, kategorize edebilir ve zenginlestirebilir — tamamen yerel, tamamen gizli.
{? endif ?}

> **Yaygin Hata:** Kurumsal satilarin zaten cozumleri oldugu iki dev platform arasinda (Salesforce ve HubSpot gibi) entegrasyonlar insa etmek. Nise git. Clio + Notion. Pipedrive + Linear. Xero + Airtable. Nisler paranin oldugu yer cunku buyuk oyuncular zahmet etmiyor.

---

### Hendek Kategorisi 2: Hiz Hendekleri

**Nedir:** Ajanslarin 2 hafta surdugu seyi 2 saatte yapiyorsun. Araclarin, is akislarin ve uzmanligin, ayni yatirimi yapmadan rakiplerin eslestiremedigi bir teslim hizi yaratiyor.

**Neden hendek:** Hiz taklit etmesi zordur. Bir musteri kodunun baskasinin kodundan daha iyi olup olmadigini soyleyemez (kolayca degil, yine de). Ama kesinlikle son kisinin 3 hafta icin teklif ettigi seyi 3 gunde teslim ettigini soyleyebilir. Hiz guven, tekrarlanan is ve yonlendirmeler yaratir.

**2026 hiz avantaji:**

Bu kursu 2026'da okuyorsun. Claude Code, Cursor, yerel LLM'ler ve Modul S'de yapilandirdigin Egemen Yigina erisiminiz var. Derin uzmanliginla birlesirildiginde, 18 ay once imkansiz olacak bir hizda is teslim edebilirsin.

{? if profile.gpu.exists ?}
{= profile.gpu.model | fallback("GPU") =} ve {= profile.gpu.vram | fallback("ozel") =} VRAM'in sana donanim hiz avantaji veriyor — yerel cikarim, API hiz limitleri beklemedigin veya hizli yineleme dongulerinde token basina maliyet odemedigin anlamina gelir.
{? endif ?}

Gercek matematik:

| Gorev | Ajans Suresi | Senin Suren (AI araclariyla) | Hiz Carpani |
|---|---|---|---|
| Kopya ile landing page | 2-3 hafta | 3-6 saat | 15-20x |
| API entegrasyonlu ozel dashboard | 4-6 hafta | 1-2 hafta | 3-4x |
| Veri isleme pipeline'i | 3-4 hafta | 2-4 gun | 5-7x |
| Teknik blog yazisi (2.000 kelime) | 3-5 gun | 3-6 saat | 8-12x |
| Belirli bir API icin MCP server | 2-3 hafta | 2-4 gun | 5-7x |
| Chrome extension MVP | 2-4 hafta | 2-5 gun | 4-6x |

**Ornek: Landing page hiz kosucusu**

Nasil calistigini gor: bir serbest calisan gelistirici 6 saatten kisa surede tam landing page'ler teslim etme itibar kuruyor — tasarim, kopya, duyarli duzenleme, iletisim formu, analitik, dagitim — sayfa basina $1.500 alarak.

Yigini:
- Musteri brifinginden baslangic duzeni ve kopya olusturmak icin Claude Code
- 6 ayda olusan kisisel bir bilesen kutuphanesi (50+ onceden insa edilmis bolum)
- Aninda dagitim icin Vercel
- Her proje icin klonladigi onceden yapilandirilmis bir analitik kurulumu

Bir ajans ayni teslim icin $3.000-8.000 aliyor ve 2-3 hafta suruyor cunku toplantilari, revizyonlari, tasarimci ve gelistirici arasinda birden fazla el degistirmesi ve proje yonetimi ek yuku var.

Bu gelistirici: $1.500, ayni gun teslim, musteri cok memnun.

Sadece landing page'lerden aylik gelir: $6.000-9.000 (ayda 4-6 sayfa).

Hendek: bilesen kutuphanesi ve dagitim is akisi insa etmek 6 ay aldi. Yeni bir rakibin ayni hiza ulasmak icin ayni 6 aya ihtiyaci olacak. O zamana kadar, gelistiricinin 6 aylik musteri iliskileri ve yonlendirmeleri var.

> **NOT:** Bilesen kutuphanesi yaklasimi Adam Wathan'in Tailwind UI'sini yansitiyor; onceden olusturulmus CSS bilesenlerini $149-299'a satarak ilk 2 yilda $4M+ uretmistir. Yeniden kullanilabilir varliklar uzerine insa edilen hiz hendeklerinin kanitlanmis ekonomisi var. (Kaynak: adamwathan.me)

**Hiz hendegi nasil insa edilir:**

1. **Bir sablon/bilesen kutuphanesi insa et.** Her projeden yeniden kullanilabilir parcalari cikar. 10 projeden sonra bir kutuphanein var. 20'den sonra super bir gucun var.

```bash
# Example: a project scaffolding script that saves 2+ hours per project
#!/bin/bash
# scaffold-client-project.sh

PROJECT_NAME=$1
TEMPLATE=${2:-"landing-page"}

echo "Scaffolding $PROJECT_NAME from template: $TEMPLATE"

# Clone your private template repo
git clone git@github.com:yourusername/templates-${TEMPLATE}.git "$PROJECT_NAME"
cd "$PROJECT_NAME"

# Remove git history (fresh start for client)
rm -rf .git
git init

# Configure project
sed -i "s/{{PROJECT_NAME}}/$PROJECT_NAME/g" package.json
sed -i "s/{{PROJECT_NAME}}/$PROJECT_NAME/g" src/config.ts

# Install dependencies
pnpm install

# Set up deployment
vercel link --yes

echo "Project $PROJECT_NAME is ready. Start with: pnpm run dev"
echo "Template: $TEMPLATE"
echo "Deploy with: vercel --prod"
```

2. **Onceden yapilandirilmis AI is akislari olustur.** En yaygin gorevlerin icin ayarlanmis sistem istemleri ve ajan yapilandirmalari yaz.

3. **Sikici kisimlari otomatiklestir.** Bir seyi 3'ten fazla yapiyorsan, betiklestir. Dagitim, test, musteri raporlamasi, faturalandirma.

4. **Hizi kamusal olarak goster.** 2 saatte bir sey insa etmenin hizlandirilmis kaydini yap. Paylas. Musteriler seni bulacak.

> **Acik Konusma:** AI araclari iyilestikce ve daha fazla gelistirici bunlari benimsedikce hiz hendekleri asinir. "Ben Claude Code kullaniyorum sen kullanmiyorsun"un saf hiz avantaji onumuzdeki 12-18 ayda benimseme yayildikca azalacak. Hiz hendegin hizin uzerine insa edilmeli — alan bilgin, bilesen kutuphanein, is akisi otomasyonun. AI araclari motor. Senin biriktirdigin sistemler sanziman.

{? if stack.primary ?}
> **Hiz Taban Cizgin:** {= stack.primary | fallback("birincil yigininin") =} birincil yigin olarak, hiz hendegi yatirimlarin o ekosistemdeki yeniden kullanilabilir varliklar insa etmeye odaklanmali — bilesen kutuphaneleri, proje iskeleleri, test sablonlari ve {= stack.primary | fallback("yiginina") =} ozgu dagitim pipeline'lari.
{? endif ?}

---

### Hendek Kategorisi 3: Guven Hendekleri

**Nedir:** Belirli bir niste taninmis uzmansin. O nisteki insanlarin bir sorunu oldugunda senin adin geliyor. Etrafta aramazlar. Sana gelirler.

**Neden hendek:** Guven insa etmesi zaman alir ve satim almak imkansizdir. Bir rakip kodunu kopyalayabilir. Fiyatini kirabilir. 500 kisinin bir nis toplulukta adini bildigi, blog yazilarini okudugu ve son 18 aydir sorulara cevap verdigini gordugu gercegini kopyalayamaz.

**"3 Blog Yazisi" kurali:**

Iste internetteki en az takdir edilen dinamiklerden biri: cogu mikro-niste 3'ten az derinlemesine teknik makale vardir. Dar bir teknik konu hakkinda 3 mukemmel yazi yaz, Google bunlari yuzeye cikaracaktir. Insanlar okuyacaktir. 3-6 ay icinde sen "X hakkinda yazan kisi"sin.

Bu bir teori degil. Matematik. Google'in dizini milyarlarca sayfaya sahip, ama "produksiyon icin GPU passthrough ile Hetzner'de Ollama nasil dagitilir" sorgusu icin 2-3 ilgili sonuc olabilir. Kesin kilavuzu yaz ve o sorguyu sahiplenirsin.

**Ornek: Rust + WebAssembly danismani**

Bu senaryoyu dusun: bir gelistirici 6 ay boyunca ayda bir blog yazisi yaziyor Rust + WebAssembly hakkinda. Konular sunlari iceriyor:

1. "Rust'i WASM'a Derleme: Tam Produksiyon Kilavuzu"
2. "WASM Performans Kiyas Testleri: 2026'da Rust vs. Go vs. C++"
3. "Rust ile WebAssembly Kullanarak Tarayici Eklentileri Olusturma"
4. "WASM Bellek Sizintilarini Ayiklama: Kesin Sorun Giderme Kilavuzu"
5. "Produksiyonda Rust + WASM: 1M Kullaniciya Gondermenin Dersleri"
6. "WebAssembly Bilesen Modeli: Rust Gelistiricileri Icin Ne Anlama Geliyor"

6 ay sonra tahmin edilen sonuclar:
- Birlesik aylik goruntulemeler: ~15.000
- Gelen danismanlik sorusturmalari: ayda 4-6
- Danismanlik ucreti: $300/sa (blogdan once $150/sa'den yukseldi)
- Aylik danismanlik geliri: $6.000-12.000 (20-40 faturalanabilir saat)
- Konusma davetiyeleri: 2 konferans

Toplam yazma zaman yatirimi: 6 ay boyunca yaklasik 80 saat. Bu 80 saatin ROI'si absurt.

> **NOT:** Ortalama $78/sa olan Rust gelistirici danismanlik ucretleri (ZipRecruiter verilerine gore ust sinirda $143/sa'ye kadar) taban cizgidir. Guven hendegi konumlanmasi ucretleri $200-400/sa'ye ceker. Guven hendeklerine sahip AI/ML uzmanlari $120-250/sa arasinda komuta eder (Kaynak: index.dev). "3 blog yazisi" stratejisi ise yarar cunku cogu mikro-niste 3'ten az derin teknik makale var.

{? if regional.country ?}
> **Bolgesel Not:** Danismanlik ucret araliklari pazara gore degisir. {= regional.country | fallback("ulkende") =} bu kiyas noktalarini yerel satin alma gucune ayarla — ama guven hendeklerinin kuresel olarak satmani sagladigini unutma. Google'da siralanan bir blog yazisi her yerden musteri ceker, sadece {= regional.country | fallback("yerel pazarindan") =} degil.
{? endif ?}

**Guven hizlandiricisi olarak acik insa:**

"Acik insa" isini, surecini, rakamlarini ve kararlarini acikca paylasma anlamina gelir — genellikle Twitter/X'te, ama ayni zamanda kisisel bloglarda, YouTube'da veya forumlarda.

Ise yarar cunku es zamanli olarak uc seyi gosterir:
1. **Yetkinlik** — calisan seyler insa edebiliyorsun
2. **Seffaflik** — neyin calistigini ve neyin calismadigi hakkinda durustun
3. **Tutarlilik** — duzenli olarak ortaya cikiyorsun

6 ay boyunca her hafta urununu insa etme hakkinda tweet atan bir gelistirici — ekran goruntuleri gostererek, metrikler paylasarak, kararlari tartisarak — dogrudan musterilere, danismanlik firsatlarina ve ortaklik firsatlarina donusen bir takipci kitlesi olusturur.

**Guven hendegi nasil insa edilir:**

| Eylem | Zaman Yatirimi | Beklenen Geri Donus |
|---|---|---|
| Ayda 1 derinlemesine teknik yazi yaz | 6-10 sa/ay | SEO trafigi, gelen firsatlar 3-6 ayda |
| Nis topluluklarda sorulari yanitla | 2-3 sa/hafta | Itibar, dogrudan yonlendirmeler 1-2 ayda |
| Twitter/X'te acik insa et | 30 dk/gun | Takipciler, marka taninma 3-6 ayda |
| Bir bulusmada veya konferansta konusma yap | 10-20 sa hazirlik | Otorite sinyali, ag kurma |
| Nisindeki acik kaynaga katki sag | 2-5 sa/hafta | Diger gelistiricilerle guvenilirlik |
| Ucretsiz bir arac veya kaynak olustur | 20-40 sa bir kerelik | Firsat olusturma, SEO capasi |

**Bilesik etki:**

Guven hendekleri diger hendeklerin birikmedigi sekilde birikir. Blog yazisi #1 500 goruntulenme alir. Blog yazisi #6 5.000 goruntulenme alir cunku Google artik alan adina guveniyordur VE onceki yazilar yenilere baglantI verir VE insanlar icerigini paylasiyor cunku adini taniyorlar.

Ayni dinamik danismanlik icin de gecerli. Musteri #1 seni bir blog yazisi yuzunden ise aldi. Musteri #5 seni Musteri #2 yonlendirdigi icin ise aldi. Musteri #10 seni ise aldi cunku Rust + WASM toplulugundaki herkes adini biliyor.

> **Yaygin Hata:** Yazmaya baslamak icin "uzman" olmayi beklemek. Gercek bir sorunu cozdugun anda insanlarin %99'una gore uzmansin. Bunun hakkinda yaz. Dun cozdugu sorun hakkinda yazan kisi, hicbir sey yayimlamayan teorik uzmandan daha fazla deger saglar.

---

### Hendek Kategorisi 4: Veri Hendekleri

**Nedir:** Rakiplerin kolayca kopyalayamayacagi veri setlerine, pipeline'lara veya veriden turetilmis icgorulere erisiminiz var. Tescilli veriler, gercekten benzersiz olduklari icin mumkun olan en guclu hendeklerden biridir.

**Neden hendek:** AI caginda herkesin ayni modellere erisimi var. GPT-4o, onu sen cagirsan da rakibin cagirsa da GPT-4o'dur. Ama bu modelleri besleyin veriler — farklilas tirilmis cikti yaratan budur. Daha iyi veriye sahip gelistirici daha iyi sonuclar uretir, nokta.

**Ornek: npm trend analitigi**

Nasil calistigini gor: bir gelistirici her JavaScript framework ve kutuphanesi icin npm indirme istatistiklerini, GitHub yildizlarini, StackOverflow soru sikligini ve is ilani bahsetmelerini izleyen bir veri pipeline'i insa ediyor. Bu pipeline'i 2 yil boyunca gunluk calistiriyor ve o formatta baska hicbir yerde bulunmayan bir veri seti biriktiriyor.

Bu veriler uzerine insa edilen urunler:
- Haftalik "JavaScript Ekosistem Nabzi" bulten — $7/ay, 400 abone = $2.800/ay
- Gelistirici araci sirketlerine satilan ceyreklk trend raporlari — her biri $500, ceyrek basina 6-8 = $3.000-4.000/ceyrek
- Arastirmacilar icin ham verilere API erisimi — $49/ay, 20 abone = $980/ay

Toplam aylik gelir potansiyeli: ~$4.500

Hendek: bu veri pipeline'ini kopyalamak baska bir gelistiricinin 2 yillik gunluk toplamasini gerektirir. Tarihsel veriler degistirilemez. Gecen yilin gunluk npm istatistiklerini zamanda geriye gidip toplayamazsin.

> **NOT:** Bu model gercek veri isletmelerini yansitiyor. Plausible Analytics rekabet hendegini kismi olarak yillarlik birikimli operasyonel veri ve guvenle tek gizlilik odakli analitik platformu olarak insa etti ve $3,1M ARR'ye kendi cabalariyla ulasti. Veri hendekleri kopyalanmasi en zor olandir cunku beceri degil, zaman gerektirir. (Kaynak: plausible.io/blog)

**Veri hendekleri etik olarak nasil insa edilir:**

1. **Kamusal verileri sistematik olarak topla.** Teknik olarak kamusal ama pratik olarak ulasilamaz olan veriler (cunku kimse organize etmemistir) gercek degere sahiptir. Basit bir pipeline insa et: SQLite veritabani, gunluk cron gorevi, yildizlar/fork'lar icin GitHub API, indirmeler icin npm API, topluluk duyarliligi icin Reddit API. Gunluk calistir. 6 ayda baska kimsenin sahip olmadigi bir veri setin var.

```python
# Core pattern: daily data collection into SQLite (run via cron)
# 0 6 * * * python3 /path/to/niche_data_collector.py

import requests, json, sqlite3
from datetime import datetime

conn = sqlite3.connect("niche_data.db")
conn.execute("""CREATE TABLE IF NOT EXISTS data_points (
    id INTEGER PRIMARY KEY, source TEXT, metric_name TEXT,
    metric_value REAL, metadata TEXT, collected_at TEXT
)""")

# Collect GitHub stars for repos in your niche
for repo in ["tauri-apps/tauri", "anthropics/anthropic-sdk-python"]:
    resp = requests.get(f"https://api.github.com/repos/{repo}", timeout=10)
    if resp.ok:
        data = resp.json()
        conn.execute("INSERT INTO data_points VALUES (NULL,?,?,?,?,?)",
            ("github", repo, data["stargazers_count"],
             json.dumps({"forks": data["forks_count"]}),
             datetime.utcnow().isoformat()))

# Same pattern for npm downloads, job postings, etc.
conn.commit()
```

{? if settings.has_llm ?}
2. **Turetilmis veri setleri olustur.** Ham verileri al ve zeka ekle — siniflandirmalar, puanlar, trendler, korelasyonlar — ki bunlar veriyi parcalarinin toplamindan daha degerli kilar. Yerel LLM'inle ({= settings.llm_model | fallback("yapilandirilmis modelin") =}), harici API'lere hicbir sey gondermeden ham verileri AI destekli siniflandirmayla zenginlestirebilirsin.
{? else ?}
2. **Turetilmis veri setleri olustur.** Ham verileri al ve zeka ekle — siniflandirmalar, puanlar, trendler, korelasyonlar — ki bunlar veriyi parcalarinin toplamindan daha degerli kilar.
{? endif ?}

3. **Alana ozgu derleme koleksiyonlari olustur.** Tur, risk duzeyi ve yargi yetkisine gore kategorize edilmis 10.000 hukuki sozlesme maddesinden olusan iyi secimlenmis bir veri seti, hukuki teknoloji sirketleri icin gercek para eder. Cogu alan icin temiz bir veri seti mevcut degildir.

4. **Zaman serisi avantaji.** Bugun toplamaya basladigin veriler her gun daha degerli hale gelir cunku kimse geri gidip dunun verilerini toplayamaz. Simdi basla.

**Veri toplama etigi:**

- Yalnizca kamusal olarak mevcut verileri topla
- robots.txt ve hiz limitlerini say
- Asla kisisel veya ozel bilgi toplama
- Bir site acikca kazima yasakliyorsa, kazima
- Sadece toplama degil, organizasyon ve analiz yoluyla deger kat
- Satarken veri kaynaklarin hakkinda seffaf ol

> **Acik Konusma:** Veri hendekleri hizli insa etmek en zor olandir ama rakiplerin kopyalamasi da en zor olandir. Bir rakip ayni blog yazisini yazabilir. Ayni entegrasyonu insa edebilir. Zaman makinesi olmadan 18 aylik gunluk metrik veri setini kopyalayamaz. Baslangic zamanini yatirmaya istekliysen, bu en guclu hendek kategorisidir.

---

### Hendek Kategorisi 5: Otomasyon Hendekleri

**Nedir:** Zamanla biriken bir betik, arac ve otomasyon is akisi kutuphanesi insa ettin. Olusturdugoin her otomasyon kapasite ve hizina eklenir. Bir yil sonra, bir rakibin kopyalamasi aylar surecek bir arac kutun var.

**Neden hendek:** Otomasyon birikir. Betik #1 haftada 30 dakika tasarruf saglar. Betik #20 haftada 15 saat tasarruf saglar. 12 ayda 20 otomasyon insa ettikten sonra, disindan sihir gibi gorunen bir hizda musterilere hizmet edebilirsin. Sonucu gorurler (hizli teslim, dusuk fiyat, yuksek kalite) ama arkasindaki 12 aylik arac yapilandirmasini gormezler.

**Ornek: Otomasyon-oncelikli ajans**

Bir solo gelistirici e-ticaret isletmelerine hizmet veren bir "tek kisilik ajans" kurdu. 18 ay icerisinde biriktirdikleri:

- 12 veri cikartma betigi (cesitli platformlardan urun verileri)
- 8 icerik uretim pipeline'i (urun aciklamalari, SEO meta verileri, sosyal medya paylasimlari)
- 5 raporlama otomasyonu (musteriler icin haftalik analitik ozetleri)
- 4 dagitim betigi (musteri magazalarina guncelleme gonderme)
- 3 izleme botu (fiyat degisiklikleri, stok sorunlari, kirik bagintilar hakkinda uyari)

Toplam betik: 32. Insa suresi: 18 ay boyunca yaklasik 200 saat.

Sonuc: bu gelistirici yeni bir e-ticaret musterisini ekliyebilir ve tam otomasyon paketini 2 gun icinde calistirabilirdi. Rakipler karsslastirabilir kurulum icin 4-6 hafta teklif ediyorlardi.

Fiyatlandirma: musteri basina $1.500/ay (10 musteri = $15.000/ay)
Otomasyondan sonra musteri basina zaman: 4-5 saat/ay (izleme ve ayarlamalar)
Efektif saatlik ucret: $300-375/sa

Hendek: 10 musteride test edilip rafine edilen bu 32 betik, 200+ saatlik gelistirme zamanini temsil eder. Yeni bir rakip sifirdan baslar.

**Otomasyon hendegi nasil insa edilir:**

```
Otomasyon Birikme Kurali:
- Ay 1: 0 otomasyonun var. Her seyi elle yapiyorsun. Yavas.
- Ay 3: 5 otomasyonun var. Manueldan %20 daha hizlisin.
- Ay 6: 12 otomasyonun var. %50 daha hizlisin.
- Ay 12: 25+ otomasyonun var. Manueldan 3-5x daha hizlisin.
- Ay 18: 35+ otomasyonun var. Musterilerine 3 kisilik bir
  ekip gibi gorunen bir seviyede calisiyorsun.
```

**Pratik yaklasim:**

Bir musteri icin her gorev yaptiginizda sor: "Bu gorevi veya cok benzerini tekrar yapacak miyim?"

Evetse:
1. Gorevi ilk seferinde elle yap (teslimatini gonder, otomasyon icin geciktirme)
2. Hemen ardindan, manuel sureci bir betige donusturmek icin 30-60 dakika harca
3. Betigi acik dokumantasyonla ozel bir depoda sakla
4. Bu gorev bir sonraki sefere geldiginde, betigi calistir ve zamanin %80'ini tasarruf et

Ornek: analitik verilerini ceken, yerel LLM'inden geciren ve bicimlendirilmis bir markdown raporu ureten bir `client-weekly-report.sh` betigi. Insa etmek 30 dakika surer, musteri basina haftada 45 dakika tasarruf saglar. 10 musteriyle carp ve 30 dakikalik bir yatirimdan her hafta 7,5 saat tasarruf etmissin.

> **Yaygin Hata:** Tek bir musteriye cok ozgu olan ve yeniden kullanilamayan otomasyonlar insa etmek. Daima sor: "Bunu bu kategorideki herhangi bir musteri icin calismasi icin parametrize edebilir miyim?" Bir Shopify magazasi icin calisan bir betik, minimal degisikliklerle herhangi bir Shopify magazasi icin calismalidir.

---

### Hendek Kategorilerini Birlestirme

En guclu pozisyonlar birden fazla hendek turunu birlestirir. Iste kanitlanmis kombinasyonlar:

{? if radar.has("tauri", "adopt") ?}
> **Radar Sinyalin:** "Adopt" halkanda Tauri var. Bu seni Entegrasyon + Guven hendekleri icin iyi konumlandirir — Tauri tabanli local-first araclar insa etmek ve surec hakkinda yazmak, az sayida gelistiricinin kopyalayabilecegi bilesik bir hendek yaratir.
{? endif ?}

| Hendek Kombinasyonu | Ornek | Guc |
|---|---|---|
| Entegrasyon + Guven | "Clio'yu her seye baglayan kisi" (ve bunun hakkinda yazar) | Cok guclu |
| Hiz + Otomasyon | Birikimli araclarla desteklenen hizli teslim | Guclu, zamanla birikir |
| Veri + Guven | Benzersiz veri seti + yayimlanmis analiz | Cok guclu, kopyalanmasi zor |
| Entegrasyon + Otomasyon | Sistemler arasi otomatik kopru, SaaS olarak paketlenmis | Guclu, olceklenebilir |
| Guven + Hiz | Taninmis uzman, ayni zamanda hizli teslim eden | Premium fiyatlandirma bolgesi |

### Ders 2 Kontrol Noktasi

Simdi sunlari anlamis olmalisin:
- [ ] Bes hendek kategorisi: Entegrasyon, Hiz, Guven, Veri, Otomasyon
- [ ] Hangi kategorilerin mevcut guclerin ve durumunla eslestigi
- [ ] Her hendek turunun gercek gelir rakamlariyla spesifik ornekleri
- [ ] Hendek kategorilerinin daha guclu konumlanma icin nasil birlestigi
- [ ] Oncelikle hangi hendek turunu insa etmek istedigin

---

## Ders 3: Nis Secim Cercevesi

*"Her sorun cozmeye deger degildir. Odeyenleri nasil bulacagin burada."*

### 4 Soru Filtresi

Herhangi bir sey insa etmek icin 40+ saat yatirim yapmadan once, bu dort sorudan gecir. Herhangi bir cevap "hayir" ise, nis muhtemelen takip etmeye deger degildir. Dort cevap da "evet" ise, bir adayin var.

**Soru 1: "Biri bu sorunu cozmek icin {= regional.currency_symbol | fallback("$") =}50 oder mi?"**

Bu minimum gecerli fiyat testidir. {= regional.currency_symbol | fallback("$") =}5 degil. {= regional.currency_symbol | fallback("$") =}10 degil. {= regional.currency_symbol | fallback("$") =}50. Biri bu sorunun kaybolmasi icin {= regional.currency_symbol | fallback("$") =}50 odemezse, sorun etrafinda is kurmaya yetecek kadar aci degil.

Nasil dogrulanir: Sorunu Google'da ara. Mevcut cozumlere bak. En az $50 aliyorlar mi? Mevcut cozum yoksa, bu ya buyuk bir firsat ya da kimsenin odemeye yetecek kadar umursamadiginin bir isareti. Forumlara git (Reddit, HN, StackOverflow) ve bu sorundan sikayet eden insanlari ara. Sikayetleri say. Hayal kirikligini olc.

**Soru 2: "40 saatten kisa surede bir cozum insa edebilir miyim?"**

Kirk saat makul bir ilk surum butcesidir. Tam zamanli calismanin bir haftasi veya 10 saatlik yan haftalarin 4 haftasi. Minimum gecerli urun bundan uzun surerse, nis test eden solo bir gelistirici icin risk-geri donus orani yanlistir.

Not: 40 saat v1 icin. Cilalanmis nihai urun icin degil. Birinin icin odeme yapacagi kadar cekirdek sorunu yeterince iyi cozen sey.

2026'daki AI kodlama araclariyla, bu 40 saat icindeki efektif ciktin 2023'te olacagindan 2-4x fazladir. 2026'da bir 40 saatlik sprint, eskiden 100-160 saat surecek seyi uretir.

**Soru 3: "Bu cozum birikir mi (zamanla daha iyi veya daha degerli mi olur)?"**

Biten bir serbest calisan projesi gelirdir. Her musteriyle daha iyi olan bir urun, gunluk buyuyen bir veri seti veya her yayinlanan icerikle insa edilen bir itibar — bu biriken bir varliktir.

Birikme ornekleri:
- Bir SaaS urunu kullanici geribudirimina dayali ozellikler ekledikce iyilesir
- Bir veri pipeline'i tarihsel veri seti buyudukce daha degerli olur
- Bir sablon kutuphanesi her projeyle daha hizli olur
- Bir itibar her yayinlanan icerikle buyur
- Bir otomasyon kutuphanesi her musteriyle daha fazla kenar vakasini kapsar

Birikmeme ornekleri:
- Ozel tek seferlik gelistirme (teslimde bitmis, yeniden kullanim yok)
- Icerik uretimi olmayan saatlik danismanlik (zaman-icin-para, olceklenmez)
- Kaybolacak bir sorunu cozen bir arac (tek seferlik goc icin goc araclari)

**Soru 4: "Pazar buyuyor mu?"**

Kuculen bir pazar en iyi konumlanmayi bile cezalandirir. Buyuyen bir pazar vasat yurutmeyi bile odullendirir. Akintila yuzmek istiyorsun, akintiya karsi degil.

Nasil kontrol edilir:
- Google Trends: Arama ilgisi artiyor mu?
- npm/PyPI indirmeleri: Ilgili paketler buyuyor mu?
- Is ilanlari: Sirketler bu teknoloji/alan icin ise aliyor mu?
- Konferans konusmalar: Bu konu daha fazla konferansta gorunuyor mu?
- GitHub aktivitesi: Bu alandaki yeni depolar yildiz aliyor mu?

### Nis Puanlama Matrisi

Her potansiyel nisi her boyutta 1-5 arasi puanla. Puanlari carp. Daha yuksek daha iyi.

```
+-------------------------------------------------------------------+
| NIS DEGERLENDIRME KARTI                                            |
+-------------------------------------------------------------------+
| Nis: _________________________________                             |
|                                                                    |
| ACI YOGUNLUGU             (1=hafif rahatsizlik, 5=sac bas yoluyor) [  ] |
| ODEME ISTEKLILIGI          (1=bedava bekler, 5=para saciyor)      [  ] |
| INSA EDILEBILIRLIK (40s)  (1=dev proje, 5=hafta sonu MVP)         [  ] |
| BIRIKME POTANSIYELI        (1=bir seferlik, 5=kartopu etkisi)     [  ] |
| PAZAR BUYUMESI             (1=kuculuyor, 5=patlyor)               [  ] |
| KISISEL UYUM               (1=alandan nefret, 5=takintiili)       [  ] |
| REKABET                    (1=kirmizi okyanus, 5=mavi okyanus)    [  ] |
|                                                                    |
| TOPLAM PUAN (hepsini carp):  ___________                           |
|                                                                    |
| Maksimum mumkun: 5^7 = 78.125                                     |
| Guclu nis: 5.000+                                                 |
| Gecerli nis: 1.000-5.000                                           |
| Zayif nis: 1.000'in altinda                                        |
+-------------------------------------------------------------------+
```

### Detayli Ornekler

Dort gercek nis degerlendirmesini inceleyelim.

**Nis A: Muhasebe yazilimi icin MCP server'lar (Xero, QuickBooks)**

| Boyut | Puan | Gerekce |
|---|---|---|
| Aci yogunlugu | 4 | Muhasebeciler AI'in otomatiklestirebilecegi veri girisi icin saatler harcyor |
| Odeme istekliligi | 5 | Muhasebe firmalari rutinlik olarak yazilim icin oduyor (arac basina $50-500/ay) |
| Insa edilebilirlik | 4 | Xero ve QuickBooks'un iyi API'lari var. MCP SDK basit. |
| Birikme | 4 | Her entegrasyon pakete eklenir. Veri kullanimla iyilesir. |
| Pazar buyumesi | 5 | Muhasebede AI, 2026'nin en sicak buyume alanlarindan biri |
| Kisisel uyum | 3 | Muhasebe konusunda tutkullu degil, ama temelleri anliyor |
| Rekabet | 4 | Muhasebe araclari icin cok az MCP server henuz var |

**Toplam: 4 x 5 x 4 x 4 x 5 x 3 x 4 = 19.200** — Guclu nis.

**Nis B: WordPress tema gelistirme**

| Boyut | Puan | Gerekce |
|---|---|---|
| Aci yogunlugu | 2 | Binlerce tema zaten mevcut. Aci hafif. |
| Odeme istekliligi | 3 | Insanlar temalar icin $50-80 oduyor, ama fiyat baskisi yogun |
| Insa edilebilirlik | 5 | Hizlica tema insa edilebilir |
| Birikme | 2 | Temalar bakim gerektirir ama degerde birikmez |
| Pazar buyumesi | 1 | WordPress pazar payi duz/dusuyor. AI site olusturuculari rekabet ediyor. |
| Kisisel uyum | 2 | WordPress'ten heyecanli degil |
| Rekabet | 1 | ThemeForest'te 50.000+ tema var. Doymus. |

**Toplam: 2 x 3 x 5 x 2 x 1 x 2 x 1 = 120** — Zayif nis. Uzaklas.

**Nis C: Hukuk burolari icin yerel AI dagitim danismanligi**

| Boyut | Puan | Gerekce |
|---|---|---|
| Aci yogunlugu | 5 | Hukuk burolari AI'a IHTIYAC DUYAR ama musteri verilerini bulut API'lerine GONDEREMEZ (etik yukumlulukler) |
| Odeme istekliligi | 5 | Hukuk burolari $300-800/sa aliyor. $5.000'lik AI dagitim projesi yuvarlama hatasidir. |
| Insa edilebilirlik | 3 | Yerinde veya uzaktan altyapi calismasi gerektirir. Basit bir urun degil. |
| Birikme | 4 | Her dagitim uzmanlik, sablonlar ve yonlendirme agi olusturur |
| Pazar buyumesi | 5 | Hukuki AI yillik %30+ buyuyor. AB AI Yasasi talebi artiriyor. |
| Kisisel uyum | 3 | Hukuk sektoru temellerini ogrenmek gerek, ama teknoloji buyuleyici |
| Rekabet | 5 | Neredeyse kimse bunu ozellikle hukuk burolari icin yapmiyor |

**Toplam: 5 x 5 x 3 x 4 x 5 x 3 x 5 = 22.500** — Cok guclu nis.

**Nis D: Kucuk isletmeler icin genel "AI chatbot"**

| Boyut | Puan | Gerekce |
|---|---|---|
| Aci yogunlugu | 3 | Kucuk isletmeler chatbot istiyor ama neden bilmiyor |
| Odeme istekliligi | 2 | Kucuk isletmelerin butceleri dar ve seni ucretsiz ChatGPT ile karsilastirir |
| Insa edilebilirlik | 4 | Teknik olarak insa etmesi kolay |
| Birikme | 2 | Her chatbot ozel, sinirli yeniden kullanim |
| Pazar buyumesi | 3 | Kalabalk, farklilastirilmamis buyume |
| Kisisel uyum | 2 | Sikici ve tekrarlayici |
| Rekabet | 1 | Binlerce "isletmeler icin AI chatbot" ajansi. Dibe dogru yaris. |

**Toplam: 3 x 2 x 4 x 2 x 3 x 2 x 1 = 576** — Zayif nis. Matematik yalan soylemez.

> **Acik Konusma:** Puanlama matrisi sihir degildir. Basariyi garanti etmez. Ama 15 dakika durust degerlendirseydinkayif oldugu acik olan bir nise 3 ay harcamani ONLEYECEKTIR. Gelistirici girisimciliginde en buyuk zaman israfcisi yanlis seyi insa etmek degildir. Dogru seyi yanlis pazar icin insa etmektir.

### Alistirma: 3 Nis Puanla

Ders 1'de tanimladigin T-sekli kesisimlerini al. Bu kesisimlerden ortaya cikan uc olasi nis sec. Her birini yukaridaki matrisi kullanarak puanla. En yuksek puana sahip nisi birincil adayin olarak tut. Ders 6'da dogrulayacaksin.

{? if stack.primary ?}
> **Baslangic Noktasi:** Birincil yigininin ({= stack.primary | fallback("birincil yigininin") =}) komsu becerilerinle ({= stack.adjacent | fallback("komsu becerilerinle") =}) birlesimi kesisimde nis firsatlari oneriyor. Bu spesifik kombinasyonu kullanan en az bir nis puanla — mevcut uzmanligin "Insa Edilebilirlik" engelini dusurur ve "Kisisel Uyum" puanini arttirir.
{? endif ?}

### Ders 3 Kontrol Noktasi

Simdi sende sunlar olmali:
- [ ] 4 soru filtresinin anlayisi
- [ ] En az 3 potansiyel nis icin tamamlanmis puanlama matrisi
- [ ] Puanlara dayanan net bir ust aday
- [ ] Bir nisi guclu vs. zayif yapanin bilgisi
- [ ] Adaylarinin nereye dustugu hakkinda durust degerlendirme

---

## Ders 4: 2026'ya Ozgu Hendekler

*"Bu hendekler su anda var cunku pazar yeni. Sonsuza kadar surmeyecek. Hareket et."*

Bazi hendekler zamansizdir — guven, derin uzmanlik, tescilli veriler. Digerleri zamana duyarlidir. Yeni bir pazar acildigi, yeni bir teknoloji baslatildigi veya yeni bir duzenleme yururluge girdigi icin varlar. Ilk hareket eden gelistiriciler orantisiz deger yakalar.

Iste 2026'da benzersiz olarak mevcut olan yedi hendek. Her biri icin: pazar buyuklugu tahmini, rekabet seviyesi, giris zorlugu, gelir potansiyeli ve bu hafta insa etmeye baslamak icin ne yapabilecegen.

---

### 1. MCP Server Gelistirme

**Nedir:** AI kodlama araclarini harici servislere baglayan Model Context Protocol server'lari insa etmek.

**Neden SIMDI:** MCP 2025'in sonunda baslatildi. Anthropic bunu siki itiyorlar. Claude Code, Cursor, Windsurf ve diger araclar MCP'yi entegre ediyor. Bugun yaklasik 2.000 MCP server var. 50.000+ olmali. Bosluk cok buyuk.

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | AI kodlama araclari kullanan her gelistirici (tahm. 2026'da 5M+) |
| Rekabet | Cok dusuk. Cogu nisin 0-2 MCP server'i var. |
| Giris zorlugu | Dusuk-Orta. MCP SDK iyi belgelenmis. Temel bir server icin 2-5 gun. |
| Gelir potansiyeli | Server basina (urun) $500-5.000/ay veya ozel proje basina $3.000-10.000 |
| Ilk dolara kadar sure | 2-4 hafta |

**Bu hafta nasil baslanir:**

```bash
# Step 1: Set up the MCP SDK
mkdir my-niche-mcp && cd my-niche-mcp
npm init -y
npm install @modelcontextprotocol/sdk

# Step 2: Pick a niche API that developers use but has no MCP server
# Check: https://github.com/modelcontextprotocol/servers
# Find what's MISSING. That's your opportunity.

# Step 3: Build a basic server (2-3 days)
# Step 4: Test with Claude Code
# Step 5: Publish to npm, announce on Twitter and Reddit
# Step 6: Monetize via Pro features, hosted version, or enterprise support
```

**MCP server'i olmayan spesifik nisler (2026 basi):**
- Muhasebe: Xero, FreshBooks, Wave
- Proje yonetimi: Basecamp, Monday.com (temel otesi)
- E-ticaret: WooCommerce, BigCommerce
- Saglik: FHIR API'lari, Epic EHR
- Hukuk: Clio, PracticePanther
- Gayrimenkul: MLS verileri, mulk yonetimi API'lari
- Egitim: Canvas LMS, Moodle

> **Yaygin Hata:** Zaten bir MCP server'i olan bir servis icin (GitHub veya Slack gibi) insa etmek. Once kayit defterini kontrol et. Sifir veya minimum kapsama olan yere git.

---

### 2. Yerel AI Dagitim Danismanligi

**Nedir:** Isletmelerin kendi altyapilarinda AI modelleri calistirmasina yardim etmek.

**Neden SIMDI:** AB AI Yasasi artik uygulanyor. Sirketlerin veri yonetisimi gostermesi gerekiyor. Es zamanli olarak, acik kaynak modeller (Llama 3, Qwen 2.5, DeepSeek) yerel dagitimi gercek is kullanimi icin uygulanabilir kilan kalite seviyelerine ulasti. "AI'yi ozel olarak calistirmamiza yardim edin" talebi tum zamanlarin en yuksek seviyesinde.

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | AI kullanan her AB sirketi (yuz binlerce). ABD saglik, finans, hukuk (on binlerce). |
| Rekabet | Dusuk. Cogu AI danismanlik sirketi bulutu itiyor. Az sayida yerel/ozele uzmanlasiyor. |
| Giris zorlugu | Orta. Ollama/vLLM/llama.cpp uzmanligi, Docker, ag bilgisi gerekli. |
| Gelir potansiyeli | Proje basina $3.000-15.000. Aylik $1.000-3.000 ucret. |
| Ilk dolara kadar sure | 1-2 hafta (kendi aginla baslarsan) |

**Bu hafta nasil baslanir:**

1. Temiz, belgelenmis bir kurulumla bir VPS'te Ollama dagit. Surecini fotografla/ekran goruntusu al.
2. Bir blog yazisi yaz: "[Sektore] 30 Dakikada Ozel Bir LLM Nasil Dagitilir"
3. LinkedIn'de payla ve slogani ekle: "Verileriniz asla sunucularinizi terk etmez."
4. r/LocalLLaMA ve r/selfhosted'da insanlarin kurumsal dagitim sordugu konulara yanit ver.
5. Agindaki 3 isletmeye ucretsiz 30 dakikalik "AI altyapi denetimi" teklif et.

{? if computed.os_family == "windows" ?}
> **Windows Avantaji:** Cogu yerel AI dagitim kilavuzu Linux'u hedefler. {= profile.os | fallback("Windows") =} calistiriyorsan, yararlanilacak bir icerik boslugu var — kesin Windows-yerel dagitim kilavuzunu yaz. Bircok kurumsal ortam Windows calistirir ve isletim sistemlerinin dilinden konusan danismanlara ihtiyac duyarlar.
{? endif ?}
{? if computed.os_family == "linux" ?}
> **Linux Avantaji:** Yerel AI dagitimi icin zaten baskin platformdasin. Linux'a asinalligin Docker, GPU gecisi ve uretim Ollama kurulumlarini dogal hale getirir — bu danismanlik hendeginin ustundeki bir hiz hendegi.
{? endif ?}

---

### 3. Privacy-First SaaS

**Nedir:** Verileri tamamen kullancinin cihazinda islenen yazilimlar insa etmek. Bulut yok. Telemetri yok. Ucuncu taraf veri paylaimi yok.

**Neden SIMDI:** Kullanicilar kaybolan bulut hizmetlerinden (Pocket kapanisi, Google Domains kapanisi, Evernote dususu) bikti. Gizlilik duzenlemeleri kuresel olarak siklasiyor. "Local-first" nis ideolojiden ana akm talebine gecti. Tauri 2.0 gibi cerceveler local-first masaustu uygulamalari insa etmeyi Electron'un hicbir zaman olmadigicadar kolay hale getiriyor.

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | Hizla buyuyor. Gizlilik odakli kullanicilar premium bir segment. |
| Rekabet | Dusuk-Orta. Cogu SaaS varsayilan olarak bulut-oncelikli. |
| Giris zorlugu | Orta-Yuksek. Masaustu uygulama gelistirme web SaaS'ten daha zor. |
| Gelir potansiyeli | $1.000-10.000+/ay. Tek seferlik satin almalar veya abonelikler. |
| Ilk dolara kadar sure | Gercek bir urun icin 6-12 hafta |

**Bu hafta nasil baslanir:**

1. Insanlarin gizliligi hakkinda sikayet ettigi bir bulut SaaS araci sec
2. Reddit ve HN'de "[arac adi] privacy" veya "[arac adi] alternative self-hosted" ara
3. 50+ oy ile ozel bir alternatif isteyen konular bulursan, bir pazarin var
4. SQLite backend'li bir Tauri 2.0 uygulamasi iskelet
5. Minimum kullanisli versiyonu insa et (bulut urunun tam ozellik setini eslemesi gerekmiyor)

---

### 4. AI Agent Orkestrasyonu

**Nedir:** Birden fazla AI ajaninin karmasik gorevleri tamamlamak icin isbirligi yaptigi sistemler insa etmek — yonlendirme, durum yonetimi, hata isleme ve maliyet optimizasyonu ile.

**Neden SIMDI:** Herkes tek bir LLM cagrisi yapabilir. Az sayida kisi cok adimli, cok modelli, cok aracli ajan is akislarini guvenilir sekilde orkestre edebilir. Araclar olgunlasmamis. Kalipler hala olusturuluyor. Ajan orkestrasyonunda simdi ustalasmis gelistiriciler, 2-3 yil icinde bu disiplinin kdemli muhendisleri olacak.

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | AI urunleri insa eden her sirket (hizla buyuyen) |
| Rekabet | Dusuk. Alan yeni. Az sayida gercek uzman. |
| Giris zorlugu | Orta-Yuksek. LLM davranisi, durum makineleri, hata isleme hakkinda derin anlayis gerektirir. |
| Gelir potansiyeli | Danismanlik: $200-400/sa. Urunler: degisken. |
| Ilk dolara kadar sure | 2-4 hafta (danismanlik), 4-8 hafta (urun) |

**Bu hafta nasil baslanir:**

1. Kendi kullnimin icin bir coklu-ajan sistemi insa et (orn: arama, ozet ve yazma alt ajanlarrna delege eden bir arastirma ajani)
2. Mimari kararlari ve odunlesimleri belgele
3. Bir blog yazisi yayimla: "4 Ajanlik Bir Orkestrasyon Sistemi Insa Ederken Ogrendigim Seyler"
4. Bu, guven hendegi + teknik hendek kombine

---

### 5. Nis Alanlar Icin LLM Fine-Tuning

**Nedir:** Temel bir modeli alip alana ozgu verilerle ince ayar yaparak spesifik gorevler icin temel modelden cok daha iyi performans gostermesini saglamak.

{? if profile.gpu.exists ?}
**Neden SIMDI:** LoRA ve QLoRA ince ayari tuketici GPU'larinda (12GB+ VRAM) erisilebilir kildi. {= profile.gpu.model | fallback("GPU") =} ve {= profile.gpu.vram | fallback("ozel") =} VRAM'inla modelleri yerel olarak ince ayar yapabilecek konumdasin. Cogu isletme bunu nasil yapacagini bilmiyor. Sen biliyorsun.
{? else ?}
**Neden SIMDI:** LoRA ve QLoRA ince ayari tuketici GPU'larinda (12GB+ VRAM) erisilebilir kildi. RTX 3060'li bir gelistirci birkac saat icinde 10.000 ornek uzerinde 7B modeli ince ayar yapabilir. Cogu isletme bunu nasil yapacagini bilmiyor. Sen biliyorsun. (Not: ozel bir GPU olmadan, RunPod veya Vast.ai gibi saglayicilardan bulut GPU kiralar kullanarak bu hizmeti sunabilirsin — danismanlik uzmanligi hendektir, donanim degil.)
{? endif ?}

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | Alana ozgu dili olan her sirket (hukuk, tip, finans, teknik) |
| Rekabet | Dusuk. Veri bilimciler teoriyi bilir ama gelistiriciler dagitimi bilir. Kesisim nadir. |
| Giris zorlugu | Orta. ML temelleri, veri hazirlama becerileri, GPU erisimi gerekli. |
| Gelir potansiyeli | Fine-tuning projesi basina $3.000-15.000. Model guncellemeleri icin aylik ucretler. |
| Ilk dolara kadar sure | 4-6 hafta |

**Bu hafta nasil baslanir:**

```bash
# Install the tools
pip install transformers datasets peft accelerate bitsandbytes

# Get a base model
# For a 12GB GPU, start with a 7B model
ollama pull llama3.1:8b

# Prepare training data (the hard part — this is where domain knowledge matters)
# You need 500-10,000 high-quality examples of input→output for your domain
# Example for legal contract analysis:
# Input: "The Licensee shall pay a royalty of 5% of net sales..."
# Output: {"clause_type": "royalty", "percentage": 5, "basis": "net_sales"}

# Fine-tune with LoRA (using Hugging Face + PEFT)
# This runs on a 12GB GPU in 2-4 hours for 5,000 examples
```

---

### 6. Tauri / Masaustu Uygulama Gelistirme

**Nedir:** Tauri 2.0 kullanarak (Rust backend, web frontend) platformlar arasi masaustu uygulamalar insa etmek.

**Neden SIMDI:** Tauri 2.0 olgun ve kararli. Electron yasini gosteriyor (bellek canavar, guvenlik kaygilari). Sirketler daha hafif alternatifler ariyor. Tauri gelistirici havuzu kucuk — dunya capinda belki 10.000-20.000 aktif gelistirici. 2M+ React gelistricisi ile karsilastir.

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | Masaustu uygulamaya ihtiyac duyan her sirket (local-first trendi ile buyuyor) |
| Rekabet | Cok dusuk. Kucuk gelistirici havuzu. |
| Giris zorlugu | Orta. Temel Rust + web frontend becerileri gerekli. |
| Gelir potansiyeli | Danismanlik: $150-300/sa. Urunler: nise bagli. |
| Ilk dolara kadar sure | 2-4 hafta (danismanlik), 6-12 hafta (urun) |

**Bu hafta nasil baslanir:**

1. Gercek bir sorunu cozen kucuk bir Tauri uygulamasi insa et (dosya donusturucu, yerel veri goruntuleyici vb.)
2. Kodu GitHub'da yayimla
3. "2026'da Neden Electron Yerine Tauri Sectim" yaz
4. Tauri Discord'unda ve Reddit'te paylas
5. Artik kamusal bir Tauri portfolyosuna sahip nispeten az sayidaki gelistiriciden birisin

{? if stack.contains("rust") ?}
> **Avantajin:** Yiginda Rust ile Tauri gelistirme dogal bir uzantidir. Backend dilini zaten konusuyorsun. Tauri'yi deneyen cogu web gelistirici Rust ogrenme egrisine duvar olarak carpar. Sen dogrudan gecersin.
{? endif ?}

---

### 7. Gelistirici Araclari (CLI Araclari, Eklentiler, Pluginler)

**Nedir:** Diger gelistiricilerin gunluk is akisinda kullandigi araclar insa etmek.

**Neden SIMDI:** Gelistirici araclari her zaman gecerli bir pazardir, ama 2026'nin spesifik arka ruzgarlari var. AI kodlama araclari yeni uzanti noktalari olusturuyor. MCP yeni bir dagitim kanali olusturuyor. Gelistiriciler artik daha uretken olduklari icin zaman kazandiran araclar icin odemeye istekli ("saatte daha fazla kazaniyorum, dolayisiyla zamanim daha degerli, dolayisiyla gunde 20 dakika kazandirmak icin $10/ay oderim" mantigi).

| Boyut | Degerlendirme |
|---|---|
| Pazar buyuklugu | 28M+ profesyonel gelistirici |
| Rekabet | Orta. Ama cogu arac vasat. Kalite kazanir. |
| Giris zorlugu | Dusuk-Orta. Araca bagli. |
| Gelir potansiyeli | Basarili bir arac icin $300-5.000/ay. |
| Ilk dolara kadar sure | 3-6 hafta |

**Bu hafta nasil baslanir:**

1. SENI rahatsiz eden hangi tekrarlayan gorev var?
2. Bunu cozen bir CLI araci veya eklenti insa et
3. Senin icin cozuyorsa, muhtemelen baskalari icin de cozer
4. npm/crates.io/PyPI'da ucretsiz katman ve {= regional.currency_symbol | fallback("$") =}9/ay Pro katmani ile yayimla

{? if radar.adopt ?}
> **Radarin:** Adopt halkandaki teknolojiler ({= radar.adopt | fallback("benimsenen teknolojilerin") =}) en derin kanaat tasidiginyerdir. Bu ekosistemlerdeki gelistirici araclari, guvenilir ve kullanisli bir araca en hizli yolun — aci noktalari ilk elden biliyorsun.
{? endif ?}

```rust
// Pattern: Free CLI tool with Pro license gating
// Build the core for free, gate batch processing / advanced features behind $9/mo

use clap::Parser;

#[derive(Parser)]
#[command(name = "niche-tool", about = "Does one thing well")]
struct Cli {
    input: String,
    #[arg(short, long, default_value = "json")]
    format: String,
    #[arg(long)]  // Pro feature: batch processing
    batch: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if cli.batch.is_some() && !check_license() {
        eprintln!("Batch processing requires Pro ($9/mo): https://your-tool.dev/pro");
        std::process::exit(1);
    }
    // Free tier: single-item processing. Pro tier: batch.
}
```

> **Acik Konusma:** Yedi hendegin hepsi senin icin degil. Birini sec. Belki ikisin. Yapabilecegin en kotu sey yedisini ayni anda insa etmeye calismak. Hepsini oku, hangisinin Ders 1'deki T-seklinle uyustugunu belirle ve oraya odaklan. Her zaman sonra pivot yapabilirsin.

{? if dna.is_full ?}
> **DNA Icgorusi:** Developer DNA'n {= dna.top_engaged_topics | fallback("cesitli konularla") =} etkilesim gosteriyor. Bu ilgi alanlarini yukaridaki yedi hendekle karsilastir — zaten dikkatini verdiklerin ile ustuste binen hendek, gercek derinlik insa edecek kadar uzun surdurebilecegin henektir.
{? if dna.blind_spots ?}
> **Kor Nokta Uyarisi:** DNA'n ayni zamanda {= dna.blind_spots | fallback("belirli alanlardaki") =} kor noktalari ortaya cikarir. Bu kor noktalardan herhangi birinin cevre gorusunde gizlenen hendek firsatlarini temsil edip etmedigini dusun — bazen dikkatindeki bosluk, pazardaki boslugun oldugu yerdir.
{? endif ?}
{? endif ?}

### Ders 4 Kontrol Noktasi

Simdi sende sunlar olmali:
- [ ] 2026'ya ozgu yedi hendegin tumunun anlayisi
- [ ] T-sekline ve durumuna uyan 1-2 hendek tanimlanmis
- [ ] Insa etmeye baslamak icin BU HAFTA yapilabilecek somut bir eylem
- [ ] Secilen hendek icin zaman cizelgesi ve gelir hakkinda gercekci beklentiler
- [ ] Hangi hendeklerin zamana duyarli (simdi hareket et) vs. dayanikli (zaman icinde insa edilebilir) oldugu farkindaligi

---

## Ders 5: Rekabet Istihbarati (Urkutucu Olmadan)

*"Insa etmeden once neyin var oldugunu, neyin kirik oldugunu ve bosluklarin nerede oldugunu bil."*

### Neden Rekabet Istihbarati Onemlidir

Cogu gelistirici once insa eder sonra arastirir. 3 ay bir sey insa etmeye harcar, baslatir ve sonra 4 baska aracin zaten var oldugunu, birinin ucretsiz oldugunu ve pazarin dusunduklerinden daha kucuk oldugunu kesfeder.

Sirayi ters cevir. Once arastir. Sonra insa et. Otuz dakikalik rekabet arastirmasi seni 300 saatlik yanlis seyi insa etmekten kurtarabilir.

### Arastirma Yigini

Pahali araclara ihtiyacin yok. Asagidakilerin hepsi ucretsiz veya comert bir ucretsiz katmana sahip.

**Arac 1: GitHub — Arz Tarafi**

GitHub nisinde nelerin insa edildigini soyler.

```bash
# Search GitHub for existing solutions in your niche
curl -s "https://api.github.com/search/repositories?q=mcp+server+accounting&sort=stars&order=desc" \
  | python3 -c "
import sys, json; data = json.load(sys.stdin)
print(f'Total results: {data[\"total_count\"]}')
for r in data['items'][:10]:
    print(f'  {r[\"full_name\"]:40} stars:{r[\"stargazers_count\"]:5}')"

# Check how active the competition is (last commit date, issue activity)
curl -s "https://api.github.com/repos/OWNER/REPO/commits?per_page=5" \
  | python3 -c "
import sys, json
for c in json.load(sys.stdin):
    print(f'  {c[\"commit\"][\"author\"][\"date\"][:10]}  {c[\"commit\"][\"message\"][:70]}')"
```

**Ne aranir:**
- Cok yildizli ama az yeni commit'li depolar = terk edilmis firsat. Kullanicilar istiyor ama bakim sorumlusu devam etmis.
- Cok acik issue'lu depolar = karsilanmamis ihtiyaclar. Issue'lari oku. Insanlarin ne istediginin yol haritasi.
- Az yildizli ama yeni commit'li depolar = birisi deniyor ama product-market fit bulamamislar. Hatalarini incele.

**Arac 2: npm/PyPI/crates.io Indirme Trendleri — Talep Tarafi**

Indirmeler insanlarin nisindeki cozumleri gercekten kullanip kullanmadigini soyler.

```python
# niche_demand_checker.py — Check npm download trends for packages in your niche
import requests
from datetime import datetime, timedelta

def check_npm_downloads(package, period="last-month"):
    resp = requests.get(f"https://api.npmjs.org/downloads/point/{period}/{package}", timeout=10)
    return resp.json().get("downloads", 0) if resp.ok else 0

def check_trend(package, months=6):
    """Get monthly download trend to spot growth."""
    today = datetime.now()
    for i in reversed(range(months)):
        start = (today - timedelta(days=30*(i+1))).strftime("%Y-%m-%d")
        end = (today - timedelta(days=30*i)).strftime("%Y-%m-%d")
        resp = requests.get(f"https://api.npmjs.org/downloads/point/{start}:{end}/{package}")
        downloads = resp.json().get("downloads", 0) if resp.ok else 0
        bar = "#" * (downloads // 5000)
        print(f"  {start} to {end}  {downloads:>10,}  {bar}")

# Compare packages in your niche
for pkg in ["@modelcontextprotocol/sdk", "@anthropic-ai/sdk", "ollama", "langchain"]:
    print(f"  {pkg:40} {check_npm_downloads(pkg):>12,} downloads/month")

# Check MCP SDK growth trajectory
print("\nMCP SDK Monthly Trend:")
check_trend("@modelcontextprotocol/sdk", months=6)
```

**Arac 3: Google Trends — Ilgi Tarafi**

Google Trends nisine olan ilginin artip artmadigini, sabit mi yoksa dususte mi oldugunu gosterir.

- [trends.google.com](https://trends.google.com) adresine git
- Nis anahtar kelimelerini ara
- Ilgili terimlerle karsilastir
- Pazarin cografi olarak spesifikse bolgeye gore filtrele

**Ne aranir:**
- Yukselen trend = buyuyen pazar (iyi)
- Duz trend = sabit pazar (tamam, rekabet dusukse)
- Dusen trend = kuculen pazar (kacin)
- Mevsimsel zirveler = lansman zamanlamamini planla

**Arac 4: Similarweb Free — Rekabet Tarafi**

Herhangi bir rakibin web sitesi icin, Similarweb tahmini trafik, trafik kaynaklari ve kitle catismasini gosterir.

- [similarweb.com](https://www.similarweb.com) adresine git
- Bir rakibin alan adini gir
- Not: aylik ziyaretler, ortalama ziyaret suresi, sekme orani, en iyi trafik kaynaklari
- Ucretsiz katman baslangic arastirmasi icin yeterlidir

**Arac 5: Reddit / Hacker News / StackOverflow — Aci Tarafi**

Gercek aci noktalari buradan bulursun. Insanlarin anketlerde ne istedigini soyledikleri degil, gece 2'de bir sey bozuldugunda ne hakkinda sikayet ettikleri.

```python
# pain_point_finder.py — Search Reddit for pain points in your niche
# Uses public Reddit JSON API (no auth needed for read-only)
import requests

def search_reddit(query, subreddit, limit=5):
    url = f"https://www.reddit.com/r/{subreddit}/search.json"
    params = {"q": query, "sort": "relevance", "limit": limit, "restrict_sr": "on"}
    resp = requests.get(url, params=params,
                       headers={"User-Agent": "NicheResearch/1.0"}, timeout=10)
    if not resp.ok: return []
    posts = resp.json()["data"]["children"]
    return sorted([{"title": p["data"]["title"], "score": p["data"]["score"],
                    "comments": p["data"]["num_comments"]}
                   for p in posts], key=lambda x: x["score"], reverse=True)

# Customize these queries for YOUR niche
for query, sub in [("frustrated with", "selfhosted"), ("alternative to", "selfhosted"),
                    ("how to deploy local LLM", "LocalLLaMA"), ("MCP server for", "ClaudeAI")]:
    print(f"\n=== '{query}' in r/{sub} ===")
    for r in search_reddit(query, sub):
        print(f"  [{r['score']:>4} pts, {r['comments']:>3} comments] {r['title'][:80]}")
```

### Bosluklari Bulmak

Yukaridaki arastirma sana uc gorunum saglar:

1. **Arz** (GitHub): Ne insa edilmis
2. **Talep** (npm/PyPI, Google Trends): Insanlar ne ariyor
3. **Aci** (Reddit, HN, StackOverflow): Ne kirik veya eksik

Bosluklar talebin var oldugu ama arzin olmadigi yerlerdir. Veya arzin var oldugu ama kalitenin zayif oldugu yerler.

**Aranacak bosluk turleri:**

| Bosluk Turu | Sinyal | Firsat |
|---|---|---|
| **Hicbir sey yok** | Arama spesifik bir entegrasyon veya arac icin 0 sonuc dondurur | Ilkini insa et |
| **Var ama terk edilmis** | 500 yildizli GitHub deposu, son commit 18 ay once | Fork'la veya yeniden insa et |
| **Var ama berbat** | Arac var, 3 yildizli incelemeler, "bu sinir bozucu" yorumlari | Daha iyi versiyonunu insa et |
| **Var ama pahali** | Basit bir sorun icin $200/ay kurumsal arac | $19/ay indie versiyonunu insa et |
| **Var ama sadece bulut** | Verilerin sunuculara gonderilmesini gerektiren SaaS araci | Local-first versiyonunu insa et |
| **Var ama manuel** | Surec calisiyor ama saatlerce insan cabasi gerektiriyor | Otomatiklestir |

### Rekabet Ortami Belgesi Olusturma

Sectigin nis icin tek sayfalik bir rekabet ortami olustur. Bu 1-2 saat surer ve seni pazari olmayan bir sey insa etmekten kurtarir.

```markdown
# Competitive Landscape: [Your Niche]
# Date: [Today]

## The Problem
[1-2 sentences describing the pain point]

## Existing Solutions

### Direct Competitors
| Solution | Price | Stars/Users | Last Updated | Strengths | Weaknesses |
|----------|-------|-------------|-------------|-----------|------------|
| [Name]   | $/mo  | count       | date        | ...       | ...        |
| [Name]   | $/mo  | count       | date        | ...       | ...        |

### Indirect Competitors (solve it differently)
| Solution | Approach | Why it's not ideal |
|----------|----------|--------------------|
| [Name]   | ...      | ...                |

### The Gap
[What's missing? What's broken? What's overpriced? What's cloud-only
but should be local? What's manual but should be automated?]

## My Positioning
[How will your solution be different? Pick ONE angle:
better, cheaper, faster, more private, more specific to a niche]

## Validation Next Steps
1. [Who will you talk to this week?]
2. [Where will you post to test demand?]
3. [What's the smallest thing you can build to prove the concept?]
```

{@ insight competitive_position @}

### 4DA Rekabet Istihbaratina Nasil Yardimci Olur

4DA calistiriyorsan, zaten bir rekabet istihbarati motoru var.

- **Bilgi boslugu analizi** (`knowledge_gaps` araci): Projendeki bagimliliklarin nereye yoneldigini ve ekosistemdeki bosluklarin nerede oldugunu gosterir
- **Sinyal siniflandirmasi** (`get_actionable_signals` araci): HN, Reddit ve RSS akislarindan trend olan teknolojileri ve talep sinyallerini yuzeye cikarir
- **Konu baglantilari** (`topic_connections` araci): Beklenmedik nis kesiisimleri bulmak icin teknolojiler arasindaki iliskileri haritalar
- **Trend analizi** (`trend_analysis` araci): Ortaya cikan firsatlari ortaya cikaranicerik akisindaki istatistiksel kaliplan

Manuel rekabet arastirmasi ile surekli calisan 4DA arasindaki fark, havayi bir kez kontrol etmek ile bir radara sahip olmak arasindaki farktir. Ikisi de faydali. Radar kacirabileceklerin yakalagir.

> **4DA Entegrasyonu:** 4DA'yi sectigin nisla ilgili subreddit'lerin, HN konularinin ve GitHub konularinin icerigini izlemek icin ayarla. Bir hafta icinde insanlarin ne istedigini, neden sikayet ettigini ve ne insa ettigini gosterern kaliplari goreceksin. Bu, 7/24 calisan firsat radarin.

### Alistirma: En Iyi Nisini Arastir

Ders 3'ten en yuksek puanli nisini al. Yukarida ana hatlariyla belirtilen arastirmayi yapmak icin 90 dakika harca. Rekabet ortami belgesini doldur. Arastirma boslugun dusundugunden daha kucuk oldugunu ortaya koyarsa, ikinci en yuksek puanli nisine don ve onu arastir.

Amac sifir rekabete sahip bir nis bulmak degil. Bu muhtemelen sifir talep demektir. Amac, talebin kaliteli cozumlerin mevcut arzini geride biraktigi bir nis bulmak.

### Ders 5 Kontrol Noktasi

Simdi sende sunlar olmali:
- [ ] Nisindeki mevcut cozumler icin GitHub arama sonuclari
- [ ] Ilgili paketler icin indirme/benimseme trendleri
- [ ] Nis anahtar kelimelerin icin Google Trends verileri
- [ ] Reddit/HN aci noktasi kaniti (isaretlenmis konular)
- [ ] En iyi nisin icin tamamlanmis bir rekabet ortami belgesi
- [ ] Tanimlanmis bosluklar: neyin var oldugu ama kirik, neyin tamamen eksik

---

## Ders 6: Hendek Haritan

*"Haritasiz hendek sadece bir hendektir. Belgele. Dogrula. Yurutu."*

### Hendek Haritasi Nedir?

Hendek Haritan bu modulun teslim edilecek sonucudur. Ders 1-5'teki her seyi tek bir belgede birlestirir ve su soruyu yanitlar: "Pazardaki savunulabilir konumum nedir ve onu nasil insa edip surdurecegim?"

Bu bir is plani degil. Bir sunum destesi degil. Sana soyleyen bir calisma belgesi:
- Kim oldugun (T-sekli)
- Duvarlarin ne (hendek kategorileri)
- Nerede savastigin (nis)
- Arenada baska kim var (rekabet ortami)
- Bu ceyrekte ne insa ettigin (eylem plani)

### Hendek Haritasi Sablonu

{? if progress.completed("S") ?}
Bu sablonu kopyala. Her bolumu doldur. Bu, Modul S'deki Egemen Yigin Belgenden sonra ikinci onemli teslim edilecek sonucun. T-Sekli ve altyapi bolumlerini doldurmak icin tamamlanmis Egemen Yigin Belgenden dogrudan veri cek.
{? else ?}
Bu sablonu kopyala. Her bolumu doldur. Bu ikinci onemli teslim edilecek sonucun. (Modul S'deki Egemen Yigin Belgen bunu tamamlayacak — tam bir konumlanma temeli icin ikisini de tamamla.)
{? endif ?}

```markdown
# HENDEK HARITASI
# [Adin / Isletme Adin]
# Olusturulma: [Tarih]
# Son Guncelleme: [Tarih]

---

## 1. T-SEKLIM

### Derin Uzmanlik (dikey cubuk)
1. [Birincil derin beceri] — [deneyim yillari, dikkat cekici basarilar]
2. [Ikincil derin beceri, uygulanabilirse] — [yillar, basarilar]

### Komsu Beceriler (yatay cubuk)
1. [Beceri] — [yetkinlik seviyesi: Yetkin / Guclu / Gelisen]
2. [Beceri] — [yetkinlik seviyesi]
3. [Beceri] — [yetkinlik seviyesi]
4. [Beceri] — [yetkinlik seviyesi]
5. [Beceri] — [yetkinlik seviyesi]

### Teknik Olmayan Bilgi
1. [Alan / sektoru / yasam deneyimi]
2. [Alan / sektor / yasam deneyimi]
3. [Alan / sektor / yasam deneyimi]

### Benzersiz Kesisimim
[Cok az insanin paylastigi beceri ve bilgi kombinasyonunu
anlatan 1-2 cumle. Bu senin temel konumlanman.]

Ornek: "Derin Rust sistem programlamayi 4 yillik saglik
sektoru deneyimi ve guclu yerel AI dagitimi bilgisiyle birlestiriyorum.
Dunya capinda 100'den az gelistiricinin bu spesifik kombinasyonu
paylasitigini tahmin ediyorum."

---

## 2. BIRINCIL HENDEK TURUM

### Birincil: [Entegrasyon / Hiz / Guven / Veri / Otomasyon]
[Neden bu hendek turu? T-seklini nasil kullaniyor?]

### Ikincil: [Insa ettigin ikinci bir hendek turu]
[Birincili nasil tamamliyor?]

### Nasil Birikiyor
[Birincil ve ikincil hendeklerinin birbirini nasil guclendirdigini tanimla.
Ornek: "Guven hendegim (blog yazilari) gelen firsatlari getiriyor ve
hiz hendegim (otomasyon kutuphanesi) daha hizli teslim etmemi sagliyor,
bu da daha fazla guven yaratiyor."]

---

## 3. NISIM

### Nis Tanimi
[Bu cumleyi tamamla: "Ben [spesifik kitleye] [spesifik sorunla]
[spesifik yaklasimimla] yardim ediyorum."]

Ornek: "Orta olcekli hukuk burolarina musteri verilerini
harici sunuculara asla gondermeyen yerel LLM altyapisi kurarak
ozel AI belge analizi dagitmalarinda yardim ediyorum."

### Nis Puan Karti
| Boyut | Puan (1-5) | Notlar |
|-----------|-------------|-------|
| Aci Yogunlugu | | |
| Odeme Istekliligi | | |
| Insa Edilebilirlik (40s altinda) | | |
| Birikme Potansiyeli | | |
| Pazar Buyumesi | | |
| Kisisel Uyum | | |
| Rekabet | | |
| **Toplam (carp)** | **___** | |

### Neden Bu Nis, Neden Simdi
[Bu nisi simdi cezbedici kilan spesifik 2026 kosullari hakkinda
2-3 cumle. Uygulanabilirse Ders 4'teki 2026'ya ozgu hendeklere referans ver.]

---

## 4. REKABET ORTAMI

### Dogrudan Rakipler
| Rakip | Fiyat | Kullanicilar/Cekim | Guclu Yonler | Zayif Yonler |
|-----------|-------|---------------|-----------|------------|
| | | | | |
| | | | | |
| | | | | |

### Dolayli Rakipler
| Cozum | Yaklasim | Neden Yetersiz |
|----------|----------|--------------------|
| | | |
| | | |

### Doldurdugum Bosluk
[Mevcut cozumlerde spesifik olarak ne eksik, kirik, asiri pahali veya
yetersiz? Bu senin pazara giris noktandaki kaman.]

### Farklilasitirmam
[BIR birincil farklilastirici sec. Uc degil. Bir.]
- [ ] Daha hizli
- [ ] Daha ucuz
- [ ] Daha gizli / local-first
- [ ] Nisime daha spesifik
- [ ] Daha iyi kalite
- [ ] [spesifik aracla] daha iyi entegre
- [ ] Diger: _______________

---

## 5. GELIR MODELI

### Nasil Odenecegim
[Birincil gelir modelini sec. Ikincilleri sonra ekleyebilirsin,
ama BIR ile basla.]

- [ ] Urun: Tek seferlik satin alma ($_____)
- [ ] Urun: Aylik abonelik ($___/ay)
- [ ] Hizmet: Danismanlik ($___/sa)
- [ ] Hizmet: Sabit fiyatli projeler ($____ proje basina)
- [ ] Hizmet: Aylik ucret ($___/ay)
- [ ] Icerik: Kurs / dijital urun ($_____)
- [ ] Icerik: Ucretli bulten ($___/ay)
- [ ] Melez: ________________

### Fiyatlandirma Gerekciesi
[Neden bu fiyat? Rakipler ne aliyor? Musteriye ne deger
yaratiyor? "10x kurali"ni kullan: fiyatin yarattigin
degerin 1/10'undan az olmali.]

### Ilk Dolar Hedefi
- **Ilk satacagim sey:** [Spesifik teklif]
- **Kime:** [Spesifik kisi veya sirket turu]
- **Hangi fiyatta:** $[Spesifik rakam]
- **Ne zamana kadar:** [Spesifik tarih, 30 gun icinde]

---

## 6. 90 GUNLUK HENDEK INSA PLANI

### Ay 1: Temel
- Hafta 1: _______________
- Hafta 2: _______________
- Hafta 3: _______________
- Hafta 4: _______________
**Ay 1 kilometre tasi:** [Ay 1 sonunda bugun dogru olmayan ne dogru?]

### Ay 2: Cekim
- Hafta 5: _______________
- Hafta 6: _______________
- Hafta 7: _______________
- Hafta 8: _______________
**Ay 2 kilometre tasi:** [Ay 2 sonunda ne dogru?]

### Ay 3: Gelir
- Hafta 9: _______________
- Hafta 10: _______________
- Hafta 11: _______________
- Hafta 12: _______________
**Ay 3 kilometre tasi:** [Gelir hedefi ve dogrulama kriterleri]

### Vazgecme Kriterleri
[Hangi kosullar altinda bu nisten vazgecip baska bir tane deneyeceksin?
Spesifik ol. "30 gun icinde 3 kisiyi 'bunun icin oderdim' demeye
ikna edemessem, ikinci secenegim olan nise pivot yapacagim."]

---

## 7. HENDEK BAKIMI

### Hendegimi Ne Asindirir
[Rekabet konumunu ne zayiflatabilir?]
1. [Tehdit 1] — [Nasil izleyeceksin]
2. [Tehdit 2] — [Nasil karsilik vereceksin]
3. [Tehdit 3] — [Nasil uyum saglayacaksin]

### Zamanla Hendegimi Ne Guclendirir
[Hangi faaliyetler avantajini biriktirir?]
1. [Faaliyet] — [Siklik: gunluk/haftalik/aylik]
2. [Faaliyet] — [Siklik]
3. [Faaliyet] — [Siklik]

---

*Bu belgeyi aylik gozden gecir. Her ayin 1'inde guncelle.
Yeniden degerlendirmede nis puanin 1.000'in altina duserse,
pivot yapmayi dusunme zamani.*
```

### Tamamlanmis Bir Ornek

Hendek Haritan dolduruldugunda nasil gorunebilecegi burada. Bu bir sablon ornegi — beklenen spesifiklik seviyesi icin referans olarak kullan.

{? if dna.is_full ?}
> **Kissellestirilmis Ipucu:** Developer DNA'n birincil yiginini {= dna.primary_stack | fallback("henuz belirlenmedi") =} olarak ve {= dna.interests | fallback("cesitli alanlardaki") =} ilgi alanlariyla tanimliyor. Bunu Hendek Haritana yazdiklarinin gerceklik kontrolu olarak kullan — gercek davranislarin (ne kodlarsin, ne okursun, neyle etkilesirsin) genellikle hedeflerinden daha durust bir sinyaldir.
{? endif ?}

**[Adin] — [Isletme Adin]**

- **T-Sekli:** Rust + yerel AI dagitiminda derin. Komsu: TypeScript, Docker, teknik yazarlik. Teknik olmayan: bir hukuk burosunda 2 yil BT calismasi.
- **Benzersiz Kesisim:** "Rust + yerel AI + hukuk burosu operasyonlari. Dunya capinda 50'den az gelistirici bunu paylasiyor."
- **Birincil Hendek:** Entegrasyon (Ollama'yi Clio gibi hukuki uygulama yonetim araclarma baglama)
- **Ikincil Hendek:** Guven (hukuki teknolojide AI hakkinda aylik blog yazilari)
- **Nis:** "Orta olcekli hukuk burolarina (10-50 avukat) ozel AI belge analizi dagitmalarinda yardimci oluyorum. Musteri verileri asla sunucularini terk etmez."
- **Nis Puani:** Aci 5, OI 5, Insa Edilebilirlik 3, Birikme 4, Buyume 5, Uyum 4, Rekabet 5 = **7.500** (guclu)
- **Rakipler:** Harvey AI (sadece bulut, pahali), CoCounsel (kullanici basina $250/ay, bulut), genel serbest calisanlar (hukuk bilgisi yok)
- **Bosluk:** Hicbir cozum yerel AI + hukuki PMS entegrasyonu + hukuki is akisi anlayisini birlestirmiyor
- **Farklilastirma:** Gizlilik / local-first (veriler asla buroyu terk etmez)
- **Gelir:** Sabit fiyatli dagitimlar ($5.000-15.000) + aylik ucretler ($1.000-2.000)
- **Fiyatlandirma gerekciesi:** 40 avukat x $300/sa x haftalik 2 saat tasarruf = $24.000/hafta kurtarilan faturalanabilir zaman. $10.000'lik dagitim 3 gunde kendini oduyor.
- **Ilk dolar:** Eski isveren icin "Ozel AI Belge Analizi Pilotu", $5.000, 15 Mart'a kadar
- **90 gunluk plan:**
  - Ay 1: Blog yazisi yayimla, referans dagitimi insa et, 5 buroyla iletisime gec, ucretsiz denetimler sun
  - Ay 2: Pilotu teslim et, vaka calismasi yaz, 10 buroyla daha iletisime gec, yonlendirmeler al
  - Ay 3: 2-3 proje daha teslim et, 1'ini aylik ucrete donustur, urun olarak Clio MCP server'ini baslat
  - Hedef: 90. gune kadar $15.000+ toplam gelir
- **Vazgecme kriterleri:** Hicbir buro 45 gun icinde ucretli pilotu kabul etmezse, sagliga pivot yap
- **Hendek bakimi:** Aylik blog yazilari (guven), her projeden sonra sablon kutuphanesi (hiz), anonimlastirilmis kiyas testleri (veri)

### Hendegini Dogrulama

Hendek Haritan bir hipotezdir. Yurutmek icin 3 ay yatirim yapmadan once temel varsayimi dogrula: "Insanlar bunun icin odeyecek."

**3 Kisi Dogrulama Yontemi:**

1. Hedef kitlene uyan 5-10 kisi tanimla
2. Onlara dogrudan ulas (e-posta, LinkedIn, topluluk forumu)
3. Teklifini 2-3 cumlede tanimla
4. Sor: "Bu var olsa, bunun icin $[fiyatin] oder misin?"
5. 5'ten en az 3'u evet derse ("belki" degil — evet), nisin dogrulanmistir

**"Landing page" dogrulamasi:**

1. Teklifini tanimlayan tek sayfalik bir web sitesi olustur (AI araclariyla 2-3 saat)
2. Bir fiyat ve "Basla" veya "Bekleme Listesine Katil" butonu ekle
3. Trafik yonlendir (ilgili topluluklarda paylas, sosyal medyada paylas)
4. Insanlar butona tiklayip e-postalarini girerse, talep gercektir

**"Hayir" nasil gorunur ve bu konuda ne yapilmali:**

- "Ilginc, ama bunun icin odemem." → Aci yeterince guclu degil. Daha akut bir sorun bul.
- "Bunun icin oderdim, ama $[fiyatini] degil." → Fiyat yanlis. Asagi ayarla veya daha fazla deger kat.
- "Birisi zaten bunu yapiyor." → Kacirdigin bir rakibin var. Arastir ve farkllas.
- "Bunun ne oldugunu anlamiyorum." → Konumlanman bulanik. Tanimlamayi yeniden yaz.
- Tam sessizlik (yanitama yok) → Hedef kitlen baktigin yerde takilmiyor. Onlari baska yerde bul.

> **Yaygin Hata:** Arkadaslardan ve aileden dogrulama istemek. "Harika fikir!" diyecekler cunku seni seviyorlar, satin alacaklari icin degil. Hedef kitlene uyan yabancilara sor. Yabancilarin kibar olma nedeni yok. Durust geri bildirimleri, annenin cesaretlendirmesinden 100 kat daha degerlidir.

### Alistirma: Hendek Haritani Tamamla

Bir zamanlayici 90 dakikaya ayarla. Yukaridaki sablonu kopyala ve her bolumu doldur. T-sekli analizinden (Ders 1), hendek kategorisi seciminden (Ders 2), nis puanlamasindan (Ders 3), 2026 hendek firsatlarindan (Ders 4) ve rekabet arastirmasindan (Ders 5) verileri kullan.

Mukemmellik hedefleme. Tamllik hedefle. Kaba ama tam bir Hendek Haritasi, mukemmel ama yari bitmis olandan sonsuz derecede daha faydalidir.

Bitirdiginde, dogrulama surecini hemen basla. Bu hafta 3-5 potansiyel musteriyle iletisime gec.

### Ders 6 Kontrol Noktasi

Simdi sende sunlar olmali:
- [ ] Egemen Yigin Belgenin yaninda kaydedilmis tam bir Hendek Haritasi belgesi
- [ ] Gercek verilerle doldurulmus 7 bolumun tamami (hedefsel tahminler degil)
- [ ] Spesifik haftalik eylemlerle 90 gunluk bir yurutme plani
- [ ] Tanimlanmis vazgecme kriterleri (ne zaman pivot yapilir, ne zaman devam edilir)
- [ ] Bir dogrulama plani: bu hafta iletisime gececegin 3-5 kisi
- [ ] Ilk aylik Hendek Haritasi gozden geciirme icin belirlenmis tarih (simdidem 30 gun sonra)

---

## Modul T: Tamamlandi

### Iki Haftada Ne Insa Ettin

{? if progress.completed_modules ?}
> **Ilerleme:** {= progress.total_count | fallback("7") =} STREETS modulunden {= progress.completed_count | fallback("0") =} tanesini tamamladin ({= progress.completed_modules | fallback("henuz hicbiri") =}). Modul T tamamlanmis setine katiliyor.
{? endif ?}

Simdi neler olduguna bak:

1. **T-seklinde beceri profili** — pazardaki benzersiz degerini tanimlayan — sadece "ne bildigin" degil "hangi bilgi kombinasyonu seni nadir kildigin."

2. **Bes hendek kategorisinin anlayisi** ve hangi tur duvar insa ettigin hakkinda net bir secim. Entegrasyon, Hiz, Guven, Veri veya Otomasyon — hangisinin guclu yonlerini kullandigini biliyorsun.

3. **Dogrulanmis bir nis** — ic gudulere degil, titiz bir puanlama cerceivesi araciligiyla secilmis. Matematigi yaptin. Aci yogunlugunu, odeme istekliligini ve rekabet seviyesini biliyorsun.

4. **2026'ya ozgu firsat farkindaligi** — hangi hendeklerin pazar yeni oldugu icin su anda mevcut oldugunu biliyorsun ve pencerenin sonsuza kadar acik kalmayacagini biliyorsun.

5. **Gercek arastirmaya dayanan bir rekabet ortami belgesi.** Neyin var oldugunu, neyin kirik oldugunu ve bosluklarin nerede oldugunu biliyorsun.

6. **Bir Hendek Haritasi** — yukaridakilerin tumunu 90 gunluk bir zaman cizelgesi ve net vazgecme kriterleriyle eyleme donusturulebilir bir plana birlestiren kisisel konumlanma belgen.

Bu, cogu gelistricinin asla olusturmadigi belge. "Becerilerim var"dan "bir seyler insa edecegim"e kritik ara adim olan "ne insa etmeliyim, kimin icin ve neden beni secsinler?" olmadan dogrudan atlarlar.

Isi yaptin. Haritan var. Simdi motorlara ihtiyacin var.

### Sirada Ne Var: Modul R — Gelir Motorlari

Modul T sana nereye nisanlayacagini soyledi. Modul R sana silahlari veriyor.

Modul R sunlari kapsiyor:

- **8 spesifik gelir motoru plani** — her motor turu icin (dijital urunler, SaaS, danismanlik, icerik, otomasyon hizmetleri, API urunleri, sablonlar ve egitim) kod sablonlari, fiyatlandirma rehberleri ve lansman siralari ile birlikte
- **Birlikte insa projeleri** — nisinde gercek, gelir ureten urunler insa etmek icin adim adim talimatlar
- **Fiyatlandirma psikolojisi** — musterileri korkutmadan maksimum gelir icin tekliflerini nasil fiyatlandirmali
- **Lansman siralari** — her gelir motoru turu icin "insa edildi"den "satildi"ya tam adimlar
- **Finansal modelleme** — gelir, maliyet ve karlilik tahminleri icin tablolar ve hesap makineleri

Modul R 5-8. haftalardır ve STREETS'in en yogun moduludur. Gercek paranin kazanildigi yer burasidır.

### Tam STREETS Yol Haritasi

| Modul | Baslik | Odak | Sure | Durum |
|--------|-------|-------|----------|--------|
| **S** | Egemen Kurulum | Altyapi, hukuk, butce | Haftalar 1-2 | Tamamlandi |
| **T** | Teknik Hendekler | Savunulabilir avantajlar, konumlanma | Haftalar 3-4 | Tamamlandi |
| **R** | Gelir Motorlari | Kodlu spesifik monetizasyon planlari | Haftalar 5-8 | Sirada |
| **E** | Yurutme Plani | Lansman siralari, fiyatlandirma, ilk musteriler | Haftalar 9-10 | |
| **E** | Gelisen Avantaj | Onde kalma, trend tespiti, adaptasyon | Haftalar 11-12 | |
| **T** | Taktik Otomasyon | Pasif gelir icin operasyonlari otomatiklestirme | Haftalar 13-14 | |
| **S** | Akislari Yigma | Birden fazla gelir kaynagi, portfoy stratejisi | Haftalar 15-16 | |

### 4DA Entegrasyonu

Hendek Haritan bir anlik goruntumdur. 4DA onu yasayan bir radara donusturur.

**`developer_dna` kullan** — gercek teknoloji kimligini gormek icin — becerilerinin ne oldugunu dusundugunu degil, kodunun, proje yapinin ve arac kullaniminin gercek guclu yonlerin hakkinda ne ortaya koydugunu. Bu, kendi beyanina dayali anketlerden degil, gercek projelerini tarayarak insa edilmistir.

**`knowledge_gaps` kullan** — talebin arzi astigi nisleri bulmak icin. 4DA bir teknolojinin artan benimsemesie ama az kaliteli kaynak veya arac oldugunu gosterdiginde, bu senin insa etme sinyalindir.

**`get_actionable_signals` kullan** — nisini gunluk izlemek icin. Yeni bir rakip gorundugunde, talep degistiginde, bir duzenleme degistiginde — 4DA icerigi taktik ve stratejik sinyallere oncelik seviyeleriyle siniflandirir ve rakiplerin farketmesinden once onemlileri gosterir.

**`semantic_shifts` kullan** — teknolojilerin deneysel olandan uretim benimsemesine ne zaman gectigini tespit etmek icin. Bu, 2026'ya ozgu hendeklerin icin zamanlama sinyalidir — bir teknolojinin "ilginc"ten "sirketler bunun icin ise aliyor"a gecis esigini ne zaman astigini bilmek sana ne zaman insa edeceğini soyler.

Egemen Yigin Belgen (Modul S) + Hendek Haritan (Modul T) + 4DA'nin surekli istihbarati = her zaman acik bir konumlanma sistemi.

{? if dna.is_full ?}
> **DNA Ozetin:** {= dna.identity_summary | fallback("Teknik kimliginin kisisellesirilmis bir ozetini burada gormek icin Developer DNA profilini tamamla.") =}
{? endif ?}

---

**Temeli insa ettin. Hendegini tanimlandin. Simdi konumlanmayi gelire donusturecek motorlari insa etme zamani.**

Modul R gelecek hafta basliyor. Hendek Haritani getir. Ihtiyacin olacak.

*Senin donanimn. Senin kurallarin. Senin gelirin.*
