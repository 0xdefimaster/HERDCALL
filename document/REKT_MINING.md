# Rekt Mining — Ekonomik Model ve Farm Simülasyonu (v0.1)

> "Kaybetsen bile bir şey kazanırsın" psikolojisini (Papertrade, EOS betting mining), kayba göre sınırsız token
> basmadan HerdCall'a taşıyan sezonluk puan sisteminin ekonomik tanımı.
> Bu belge **kontrat kodu değildir**; parametreler kesinleşmeden önce sağlanması gereken koşulları tanımlar.
> Model ve testler: [`sim/rekt-mining.js`](../sim/rekt-mining.js), [`sim/rekt-mining.test.js`](../sim/rekt-mining.test.js).
> Çalıştırmak için: `npm run test:sim` (testler), `npm run sim:rekt` (senaryo tabloları).

**Cevaplanan soru:** Farmcı, kendi ödülünü büyütmek için ne kadar ücret yakmak zorunda ve bu sırada dürüst
kullanıcıların ödül payını ne kadar azaltıyor?

> **Durum: model varsayımı.** Bu belgedeki ekonomik güvenlik iddiaları, gerçek likidite ve zincir üstü verilerle
> doğrulanana kadar model varsayımıdır. Testlerin geçmesi, kodun yazılan kuralları uyguladığını gösterir;
> ekonomik güvenliğin gerçek piyasa koşullarında kanıtlandığını göstermez.

**Kısa cevap (model içinde):**
- Ödül havuzunun değeri, o sezonun uygun net protokol gelirinin sabit bir payıyla ($\alpha$) sınırlanırsa ve
  §4'teki koşul sağlanırsa, test edilen senaryolarda modelin öngördüğü **ödül kaynaklı net farm kârı sıfır veya
  negatiftir**. Bu, §4'teki varsayımlar altında ispatlanmış bir sınırdır; çoklu hesap, wash trading, dışarıdan
  sübvanse edilen ücretler ve MEV gibi davranışlar gerçek sistemde sonucu değiştirebilir (§6).
- Bütçe yalnızca token adediyle sabitlenirse ($B_t$), $HERD fiyatı yükseldikçe farm kârlı hâle gelir. Örnek:
  havuz değeri dürüst ücretlerin 2 katına çıkınca dürüst kullanıcılar ödülün yalnızca %63'ünü alır (§5.3).
- Kayba bağlı çarpan **tutma şartı olmadan** farmcıya bedavadır: her al-sat turu zararla kapanır (§5.1).
- **v0.1 ekonomi politikası kabul edildi (§7):** yeni tahsis SOL cinsinden ve uygun net gelirin en fazla %25'i,
  puanlar göreli, $HERD sezon sonunda gelirle kontrollü alınır, yeni token basılmaz. Modelde $HERD fiyatı ödül
  kaynaklı farm ekonomisini değiştirmez (§5.7).
- **Alımı parçalara bölmek toplam fiyat etkisini ortadan kaldırmaz.** Arbitraj fiyatı parçalar arasında geri
  getirmezse 28 parçalık alım, tek alımla aynı toplam fiyata gelir (§5.8). Parçalama işlem başına etkiyi ve
  sandwich alanını küçültür; toplam maliyeti garanti etmez.

---

## 0. Kapsam

- **Kapsamda:** Sezonluk puan hesabı, ödül havuzunun değeri, zarar ve tutma çarpanları, Diamond Hooves ile
  ayrı muhasebe, farmcının en iyi stratejisi ve dürüst kullanıcıya etkisi.
- **Kapsam dışı:** $HERD'ün piyasa fiyatı ve likiditesi, puanların token'a dönüşüm takvimi, hukuki
  değerlendirme, MEV (sandwich) koruması. Sandwich yalnızca Rekt Mining'den ayrı bir sorun olduğunu göstermek
  için ölçülür (§5.1).
- **Puan ≠ itibar:** Bu puanlar ekonomik ödüldür. Fair Score ve Herd rank / Herd Caller (itibar) ayrı kalır;
  hacim itibar kazandırmaz (HERDCALL.md §9).

## 1. Terimler

| Sembol | Anlamı | Simülasyondaki değer |
|---|---|---|
| Puan tabanı | Bir işlemin ürettiği **protokol ücreti** (işlem tutarının %0,45'i) | — |
| $H$ | Sezonda dürüst kullanıcıların ödediği protokol ücreti | 450 SOL (100k SOL hacim) |
| $x$ | Farmcının ödediği protokol ücreti | farmcı seçer |
| $m_i$ | Pozisyon çarpanı: zarar ($m_r$), tutma ($m_h$), üst sınır ($M$) | §3 |
| $\bar m$ | Dürüst kullanıcıların taban ağırlıklı ortalama çarpanı | §5.2 |
| $m_F$ | Farmcının ortalama çarpanı | en kötü durumda $M$ |
| $c_F$ | Farmcının 1 birim puan tabanı için net maliyeti (geri dönen ücretler düşülmüş) | §5.1 |
| $V_t$ | Sezon ödül havuzunun değeri | §2 |
| $\alpha$ | Havuz değerinin sezon protokol gelirine oranı (üst sınır) | 0,1 – 1 |

## 2. Puan ve dağıtım

- Kullanıcı puanı: $S_i = \sum_{\text{pozisyon}} \text{protokol ücreti} \times m_{\text{pozisyon}}$.
- Ödül: $R_i = V_t \cdot S_i / \sum_j S_j$.
- **Puan tabanı yalnızca protokol payıdır, ücretin tamamı değil.** Creator payı (%0,30) ve arayüz payı (%0,10)
  farmcının kendi cüzdanlarına dönebilir. Taban tüm ücret olsaydı en ucuz farmcının birim maliyeti
  1,33'ten 0,60'a düşerdi ve §4'teki marj neredeyse kapanırdı.
- **Havuz değeri gelirle sınırlı ve SOL cinsinden (v0.1 politikası, §7):**
  $V_t = \min\left(\alpha R_t,\ R_t - R_t/50 - O_t\right)$. $R_t$, sezonun **uygun net protokol geliridir**
  (§7.1; farmcının ödedikleri dahil), $R_t/50$ Diamond Hooves bütçesi, $O_t$ operasyon tabanıdır. Havuz önce SOL olarak
  birikir; $HERD sezon kapandıktan sonra bu SOL ile piyasadan alınır. Token adedi $V_t / P_{HERD}$'dir, yani
  fiyat yalnızca kaç token dağıtılacağını belirler, havuzun değerini değil.
- **Reddedilen: sabit token bütçesi** $V_t = B_t \cdot P_{HERD}$. Enflasyonu sınırlar ama farmı sınırlamaz (§5.3, §5.7).

## 3. Çarpanlar

- **Tutma şartı (7 gün):** Zarar çarpanı yalnızca en az 7 gün tutulup zararla kapanan pozisyonlara uygulanır.
  Şartsız bir zarar çarpanı farmcıya bedavadır: anında al-sat her zaman zararla kapanır (§5.1, S1).
- **Tutma çarpanı:** 7 gün ve üzeri tutulan pozisyonlar $m_h$ alır.
- **Üst sınır:** Çarpanların çarpımı $M$'i aşamaz.
- Öneri (v0.1): $m_r = 1{,}5$, $m_h = 1{,}25$, $M = 2$. Bu değerlerle uzun süre tutup zararla kapanan pozisyon
  1,875x alır. $m_r \ge 1{,}6$ olduğunda üst sınıra takılır, yani $m_r$'yi 2x yapmak sonucu değiştirmez (§5.5).

## 4. Güvenlik koşulu (sınırlı havuz)

Farmcı $x$ kadar protokol ücreti öderse kârı:

$$\pi(x) = \underbrace{V_t \cdot \frac{m_F x}{\bar m H + m_F x}}_{\text{sezon ödülü}} + \underbrace{\text{DH}(x)}_{\text{Diamond Hooves}} - c_F\, x$$

$V_t \le \alpha (H + x)$ olduğundan sezon ödülü $\le x \cdot \alpha \cdot \max(1, m_F/\bar m)$ olur.
Diamond Hooves bütçesi $(H+x)/50$'dir ve uygun ücretlerle orantılı bölünür; farmcının payı
$\le x \cdot \max(1, e_F/e_H)/50$ olur. Burada $e_H$ dürüst tabanın, $e_F$ farmcı tabanının uygun kısmıdır.
Buradan:

$$\boxed{\ \alpha \cdot \max\!\left(1, \frac{m_F}{\bar m}\right) + \frac{\max(1,\ e_F / e_H)}{50} \le c_F \ \Rightarrow\ \pi(x) \le 0 \ \ \forall x\ }$$

Bu bir **model içi** sonuçtur: $\pi$ yalnızca ödülden ve Diamond Hooves'tan gelen kazancı içerir. Varsayımlar:
ücretler gerçekten tahsil edilir, havuz $\alpha R_t$'yi aşmaz, farmcının ödül dışında kazancı yoktur ve $c_F$
§5.1'deki stratejilerin en ucuzundan düşük değildir.

- Koşul **sıkıdır**: farmcı çarpan ve uygunlukta dürüst kullanıcıdan iyiyse ve sol taraf $c_F$'yi aşarsa, küçük
  bir farm kâr eder. Test: `capped pool: the condition is tight`.
- En kötü durum hesabı: $m_F = M$, $e_F = 1$, $c_F$ = en ucuz strateji (kendi creator'ı ve arayüzü, §5.1).
- Koşul, farmcının kendi ödülü dışında hiçbir şey kazanmadığını varsayar. "Trend olmak için" yapılan hacim
  bunun dışındadır (§6).

## 5. Simülasyon sonuçları

Bütün tablolar `npm run sim:rekt` çıktısıdır. Dürüst kullanıcı karışımı bir **varsayımdır**: tabanın %25'i kısa
kârlı, %45'i kısa zararlı, %10'u uzun kârlı, %20'si uzun zararlı pozisyonlardan gelir. Gerçek dağılım test ağı
veya mainnet verisiyle güncellenmeli.

### 5.1 Farmcının maliyeti (gerçek curve üzerinde)

Stampede sonrası curve (50 SOL Lobby), 200 dürüst işlemlik geçmiş, 5 SOL'lük bacaklar, 300 tekrar.

| Strateji | 1 SOL taban başına net maliyet | Zararla kapanan pozisyon | Tutma süresince fiyat PnL'i (SOL) |
|---|---|---|---|
| S1 al-sat, tek cüzdan | 2,22 | %100 | — |
| S1, creator ve arayüz farmcının | **1,33** | %100 | — |
| S2 iki cüzdan (A alır, B alır, A satar, B satar) | 2,22 | %50 | — |
| S3 iki cüzdan, 7 gün tutar | 2,37 | %49 | 0,28 ± 1,37 |
| S3, creator ve arayüz farmcının | 1,48 | %51 | 0,26 ± 1,38 |
| S4 sandwich (%1 kayma toleransı) | **−3,77** | %0 | — |

- Her al-sat turu ücretlerin tamamını öder; curve'de "bedava zarar" yoktur. Ama **zararla kapanmak bedavadır**.
- 7 gün tutma maliyeti çok az artırır (sermaye maliyeti, yıllık %7). Asıl etkisi, farmı ücretle sınırlı olmaktan
  çıkarıp **sermayeyle sınırlı** hâle getirmesidir: aynı sermaye haftada yalnızca bir tur döner.
- Sandwich, ödül olmadan da kârlıdır (taban başına 3,77 kazanç). Bu MEV sorunudur, Rekt Mining'in değil. Ayrıca
  ele alınmalı (kayma varsayılanları, işlem sıralaması).

### 5.2 Çarpanlar: dürüst kullanıcı ve farmcı

| Tasarım | Dürüst $\bar m$ | Farmcı S1 | Farmcı S3 | En kötü ($M/\bar m$) |
|---|---|---|---|---|
| Çarpansız | 1,00 | 1,00 | 1,00 | 1,00 |
| Zarar 2x, tutma şartı yok | 1,90 | 2,00 | 2,20 | 1,58 |
| Zarar 2x, 7 gün şartı | 1,45 | 1,00 | 2,20 | 2,07 |
| **Öneri:** zarar 1,5x, tutma 1,25x, şart, üst sınır 2 | 1,20 | 1,00 | 1,54 | 1,67 |

### 5.3 Sabit token bütçesi (reddedilen)

Öneri tasarımı, en ucuz farmcı ($c_F = 1{,}33$, $m_F = 2$), $H = 450$ SOL.

| $V/H$ | $V$ (SOL) | Farmcının yaktığı ($\times H$) | Farmcı kârı (SOL) | Dürüst kullanıcıya kalan |
|---|---|---|---|---|
| 0,1 | 45 | 0 | 0 | %100 |
| 0,5 | 225 | 0 | 0 | %100 |
| 1 | 450 | 0,07 | 5 | %89 |
| 2 | 900 | 0,35 | 122 | %63 |
| 5 | 2.250 | 0,90 | 810 | %40 |
| 10 | 4.500 | 1,52 | 2.314 | %28 |

Farm, $V > c_F \bar m H / m_F$ olduğunda başlar (burada yaklaşık 360 SOL). Bütçe token adediyle sabitse bu eşik
$HERD fiyatıyla birlikte aşılır: fiyat 5 katına çıkarsa dürüst kullanıcıların ödül payı %40'a düşer.

### 5.4 Sınırlı havuz (önerilen)

En kötü durum farmcısı. Son sütun: farmcı kâr etmese bile (ör. trend olmak için) $1 \times H$ yakarsa dürüst
kullanıcının ödülünün ne kadarı kalır.

| Tasarım | $\alpha$ | Güvenlik marjı | Model: ödül kaynaklı en iyi farm kârı | $1\times H$ yakılırsa dürüste kalan |
|---|---|---|---|---|
| Zarar 2x, şart yok | 0,25 | 0,84 | 0 | %78 |
| Zarar 2x, şart yok | 1,00 | −0,35 | 15 SOL | %78 |
| Zarar 2x, 7 gün şartı | 0,25 | 0,72 | 0 | %65 |
| Zarar 2x, 7 gün şartı | 1,00 | −0,84 | 54 SOL | %65 |
| **Öneri** | 0,10 | 1,07 | 0 | %75 |
| **Öneri** | **0,25** | **0,82** | **0** | **%75** |
| **Öneri** | 0,50 | 0,40 | 0 | %75 |
| **Öneri** | 1,00 | −0,43 | 22 SOL | %75 |

- $\alpha \le 0{,}5$ olduğunda test edilen tasarımlarda ödül kaynaklı net farm kârı sıfır veya negatiftir.
  $\alpha = 1$ (gelirin tamamı ödül) her tasarımda kârlı farm açar.
- Kâr etmeyen ama yine de yakan farmcı, havuzu büyüttüğü hâlde dürüst payını %22–35 azaltır. Bunun maliyeti
  farmcıya $1{,}33 \times H$'dir. Bu yüzden Board ve trend sıralaması ham hacme göre yapılmamalı (§6).

### 5.5 Zarar çarpanı taraması (7 gün şartı, tutma 1,25x, $\alpha = 0{,}25$)

| Zarar x | Üst sınır | Dürüst $\bar m$ | $M/\bar m$ | Güvenlik marjı | Uzun zararlı pozisyonun ortalamaya göre payı |
|---|---|---|---|---|---|
| 1,00 | 2 | 1,07 | 1,86 | 0,77 | 1,16x |
| 1,25 | 2 | 1,14 | 1,76 | 0,79 | 1,37x |
| **1,50** | **2** | **1,20** | **1,67** | **0,82** | **1,56x** |
| 2,00 | 2 | 1,23 | 1,63 | 0,83 | 1,63x |
| 2,00 | 3 | 1,32 | 2,26 | 0,67 | 1,89x |
| 3,00 | 3 | 1,43 | 2,11 | 0,71 | 2,11x |

- Güvenliği zarar çarpanı değil, **üst sınır $M$ ve $\alpha$** belirler. En kötü durumda farmcı her pozisyonda
  $M$ alır; zarar çarpanını artırmak dürüst ortalamayı da yükselttiği için marj biraz artar bile.
- Bu yüzden "2x zarar çarpanı" sorusu yerine karar verilecek değerler $M$ ve $\alpha$'dır. $M = 2$ ve
  $m_r = 1{,}5$ ile uzun süre tutup zarar eden kullanıcı ortalamanın 1,56 katı ödül alır.

### 5.6 v0.1 politikası: sezon geliri seviyeleri

$\alpha = 0{,}25$, örnek operasyon tabanı $O_t = 100$ SOL / sezon, en kötü durum farmcısı.

| Dürüst hacim (SOL) | Protokol geliri $H$ | Havuz (SOL) | Havuz / gelir | Model: ödül kaynaklı en iyi farm kârı | $1\times H$ yakılırsa dürüste kalan |
|---|---|---|---|---|---|
| 10.000 | 45 | 0 | %0 | 0 | — |
| 50.000 | 225 | 56,2 | %25 | 0 | %75 |
| 100.000 | 450 | 112,5 | %25 | 0 | %75 |
| 1.000.000 | 4.500 | 1.125 | %25 | 0 | %75 |
| 10.000.000 | 45.000 | 11.250 | %25 | 0 | %75 |

- Koşul ölçekten bağımsızdır: test edilen her gelir seviyesinde ödül kaynaklı net farm kârı sıfır veya negatiftir.
- Gelir operasyon tabanını karşılamıyorsa o sezon havuz yoktur. Taban dolarken havuz, yakılan her ücretle
  $\alpha$'dan hızlı büyüyebilir, ama $V_t \le \alpha R_t$ her zaman geçerli olduğu için koşul bozulmaz (test R10).

### 5.7 $HERD fiyat senaryoları ($H = 450$ SOL)

Sabit token bütçesi, 1x fiyatta politikadaki havuzla aynı değere gelecek şekilde seçildi.

| $HERD fiyatı | Sabit $B_t$: havuz değeri | Sabit: model farm kârı | Sabit: dürüste kalan | Politika: havuz | Politika: model farm kârı | Politika: alınan $HERD (M)* |
|---|---|---|---|---|---|---|
| 0,1x | 11 | 0 | %100 | 112,5 | 0 | 10,63 |
| 0,5x | 56 | 0 | %100 | 112,5 | 0 | 2,13 |
| 1x | 113 | 0 | %100 | 112,5 | 0 | 1,06 |
| 2x | 225 | 0 | %100 | 112,5 | 0 | 0,53 |
| 5x | 563 | 27 | %79 | 112,5 | 0 | 0,21 |
| 10x | 1.125 | 221 | %56 | 112,5 | 0 | 0,11 |

\* 2.000 SOL derinlikli havuzda 28 parça, arbitraj yok (en kötü durum, §5.8).

- Sabit bütçede fiyat 5 katına çıkınca farm kârlı olur ve dürüst pay düşer. Politikada havuz değeri fiyattan
  bağımsızdır; fiyat yalnızca dağıtılan token adedini değiştirir.

### 5.8 Kontrollü alım: toplam fiyat etkisi ve işlem başına etki

Sabit çarpımlı $HERD/SOL havuzu, havuz ücreti %0,25, işlem başına tavan %0,5. **Toplam prim:** ortalama alım
fiyatının alım başlamadan önceki fiyata göre farkı (havuz ücreti hariç). **Maks:** tek bir işlemin, kendi
öncesindeki fiyata göre etkisi. $\rho$: arbitrajın her parçanın etkisinin ne kadarını bir sonraki parçadan önce
geri aldığı ($\rho = 0$: hiç, en kötü durum).

| Havuz / likidite | Tek işlem | 28 parça, $\rho = 0$ | 28 parça, $\rho = 0{,}5$ | 28 parça, $\rho = 1$ | 100 parça, $\rho = 0$ |
|---|---|---|---|---|---|
| 0,01 | durdu | %1,00 / maks %0,04 | %0,10 / maks %0,04 | %0,04 / maks %0,04 | %1,00 / maks %0,01 |
| 0,05 | durdu | %4,99 / maks %0,18 | %0,51 / maks %0,18 | %0,18 / maks %0,18 | %4,99 / maks %0,05 |
| 0,10 | durdu | %9,99 / maks %0,36 | %1,02 / maks %0,36 | %0,36 / maks %0,36 | %9,99 / maks %0,10 |
| 0,20 | durdu | durdu | durdu | durdu | %19,97 / maks %0,20 |
| 0,50 | durdu | durdu | durdu | durdu | %49,93 / maks %0,50 |

- **Parçalamak toplam maliyeti düşürmez.** Arbitraj yoksa ($\rho = 0$) 28 ya da 100 parça, tek işlemle aynı toplam
  fiyata gelir: havuz / likidite oranı 0,10 ise yaklaşık %10. Parçalamanın faydası işlem başına etkiyi ve
  sandwich alanını küçültmektir. Toplam maliyet ancak parçalar arasında fiyat geri gelirse düşer ve bu piyasaya
  bağlıdır, garanti edilemez.
- **%0,5 işlem başına bir toleranstır, toplam fiyat etkisi sınırı değildir.** Her parçadan önce tahmini etki
  hesaplanır; tavanı aşıyorsa işlem yapılmaz. Ayrıca işlem zincir üstünde min-out ile korunur: gerçekleşen etki
  tavanı aşarsa işlem geri döner. İki durumda da alım durur ve SOL sezonun defterinde kalır (§7.5).
- Oran 0,2 ve üstünde 28 parça bile tavanı aşar. Bu büyüklükte bir havuz için ya çok daha uzun bir alım
  penceresi gerekir ya da harcanamayan SOL, sezonun listesine SOL olarak ödenir (§7.4).

### 5.9 Parçaların önüne geçme (sandwich)

Havuz / likidite 0,05, her parçanın önüne geçen bir saldırgan. **Kayma toleransı:** parçanın, işlem öncesi
fiyat teklifinin ne kadar altında dolabileceği. Saldırganın kullanabileceği alan budur.

| Parça | Parça başı beklenen etki | Havuz ücreti | Kayma toleransı | Saldırgan kârı (bütçenin %'si) | Gerçekleşen maks etki | Toplam prim |
|---|---|---|---|---|---|---|
| 20 | %0,25 | %0 | %0,50 | %0,26 | %0,50 | %5,26 |
| 20 | %0,25 | %0 | %0,10 | %0,10 | %0,35 | %5,10 |
| 20 | %0,25 | %0,25 | %0,50 | %0,00 | %0,25 | %4,99 |
| 100 | %0,05 | %0 | %0,50 | %0,45 | %0,50 | %5,45 |
| 100 | %0,05 | %0 | %0,10 | %0,10 | %0,15 | %5,10 |
| 100 | %0,05 | %0,25 | %0,50 | %0,00 | %0,05 | %4,99 |

- Ücretsiz bir havuzda saldırgan, kayma toleransı kadar kâr eder; parça sayısını artırmak bunu düşürmez.
  Toleransı daraltmak (%0,5 → %0,1) bu kazancı sınırlar.
- %0,25 havuz ücreti bu boyutlarda sandwich'i kârsız kılar. Ücret ve havuz derinliği gerçek piyasada farklı
  olacağı için bu sonuç bir varsayımdır.
- Kural: işlem başına tavan %0,5; kayma toleransı tavandan ayrı ve dar tutulur (öneri %0,1).

### 5.10 Likidite çekilirse ve başarısız alım

Havuz / likidite 0,1, 28 parça; 10. parçadan önce likiditenin %50'si çekiliyor.

| Alınan parça | Durma sebebi | Harcanan SOL | Harcanmayan SOL |
|---|---|---|---|
| 10 | Tahmini etki tavanın üstünde | 35,7 | 64,3 |

Harcanmayan SOL bir sonraki sezona tahsis edilmez. Alım penceresinde devam edilemezse sezonun listesine SOL
olarak ödenir (§7.4).

### 5.11 Hazine defterleri: dört sezon örneği

$\alpha = 0{,}25$, $O_t = 100$ SOL, claim süresi 2 sezon.

| Sezon | Uygun net gelir | Yeni tahsis | Gelire oranı | Kullanılan devir (maliyetle) | $HERD için harcanan SOL | SOL olarak borçlu (harcanmayan) | Maliyetle dağıtılan | Protokole iade (kümülatif) |
|---|---|---|---|---|---|---|---|---|
| 1 | 900 | 225,0 | %25 | 0 | 135,0 | 90,0 | 225,0 | 0 |
| 2 | 450 | 112,5 | %25 | 0 | 112,5 | 0 | 112,5 | 0 |
| 3 | 1.200 | 300,0 | %25 | 33,8 | 266,3 | 0 | 300,0 | 56,3 |
| 4 | 300 | 75,0 | %25 | 0 | 75,0 | 0 | 75,0 | 56,3 |

- 1. sezonda alımın %60'ı yapılabildi; kalan 90 SOL o sezonun listesine SOL olarak borçlu, 2. sezona eklenmedi.
- 1. sezonun claim süresi 3. sezonda doldu. Talep edilmeyen $HERD (maliyeti 33,8 SOL) 3. sezonun **kendi tavanı
  içinde** kullanıldı; 3. sezon yeni alımı o kadar azalttı ve aynı tutar protokole döndü. Talep edilmeyen
  22,5 SOL da protokole döndü. Hiçbir sezon tahsisinden fazlasını dağıtmadı.

## 6. Bilinen sınırlar

- **Ödül dışı faydalar:** Koşul, farmcının yalnızca ödülden kazandığını varsayar. Hacimle trend olmak, Board'da
  üste çıkmak, creator ücreti (zaten $c_F$'de) gibi faydalar farmı kâr etmese bile anlamlı kılabilir.
  Önlem: Board ve keşif sıralaması ham hacimle yapılmaz (benzersiz holder, Stampede katılımı, tutma).
- **Hesaplar arası örüntü:** Model, farmcının kendi cüzdanlarını tanıyamadığımızı varsayar ve buna rağmen güvenlidir.
  Örüntü takibi ek bir katman olabilir ama güvenlik ona dayanmaz.
- **Dürüst karışım varsayımdır:** $\bar m$ ve $e_H$ gerçek veriye bağlıdır. $e_H$ küçükse (az kişi uzun tutup
  zararla çıkıyorsa) Diamond Hooves terimi büyür: $e_H = 0{,}05$ için $1/0{,}05/50 = 0{,}4$. $\bar m = 1$ alınsa
  bile marj öneri parametreleriyle pozitif kalır ($1{,}33 - 0{,}25 \cdot 2 - 0{,}4 = 0{,}43$).
- **Satış baskısı:** v0.1'de $HERD piyasadan alınır, yeni token basılmaz. Ödül alanlar satarsa satış baskısı en
  fazla o sezon alınan miktar kadardır; alım baskısı ile satış baskısı aynı büyüklüktedir ama zamanlamaları
  farklı olabilir. $HERD'ün likiditesi ve fiyat davranışı bu modelin dışındadır.
- **Alımın önüne geçme:** Havuzun büyüklüğü herkese açıktır. Her parçanın önüne geçen saldırgan, kayma
  toleransı kadar kazanabilir (§5.9). Sezon boyunca önceden $HERD biriktirip alım dönemine satmak ise toplam
  fiyat etkisinden pay alır (§5.8); bu modelde ölçülmedi. Dar kayma toleransı ve önceden ilan edilmeyen
  zamanlama alanı küçültür, ortadan kaldırmaz.
- **Bu sonuçların geçerlilik alanı:** "Ödül kaynaklı net farm kârı sıfır veya negatif" sonucu §4'teki
  varsayımlar altında ve test edilen senaryolarda geçerlidir: farmcının tek kazancı ödüldür, ücretler gerçekten
  tahsil edilir ve havuz $\alpha R_t$'yi aşmaz. Gerçek sistemde wash trading, çoklu hesaplar, dışarıdan
  sübvanse edilen işlem ücretleri (ör. bir projenin kendi tokeninde hacim için ödediği ücretler), MEV, fiyat
  oynaklığı ve alımın piyasa etkisi sonucu değiştirebilir ve ayrıca test edilmelidir. Bu modelde çoklu hesap
  (S2, S3), sandwich (S4, §5.9) ve alım etkisi (§5.8, §5.10) ölçüldü; zincir üstü davranış, gerçek havuz
  derinliği ve gerçekleşen ortalama alım fiyatı ölçülmedi.
- **Dışarıdan sübvanse edilen ücretler:** Ücretleri başkası öderse (ör. bir proje kendi tokeninin hacmini
  fonlarsa) farmcının $c_F$'si düşer ve §4'teki koşul o farmcı için geçerli olmayabilir. Puan tabanının yalnızca
  protokol payı olması bu riski küçültür, ortadan kaldırmaz.
- **Trend ve sıralama:** Board, trend ya da leaderboard ham hacme göre görünürlük veya ödül verirse, Rekt
  Mining'den bağımsız olarak manipülasyon teşviki yaratır. v0.1'de sıralama uygun protokol ücreti ve
  doğrulanmış aktiviteyle yapılır (§7).
- **Sermaye ve fiyat riski:** S3'te fiyat riski ortalamada sıfıra yakındır ama sapması büyüktür (5 SOL'lük
  bacaklarda ±1,4 SOL). Riskten kaçınan farmcı için gerçek maliyet tablodakinden yüksektir; model bunu
  farmcı lehine yok sayar.

## 7. v0.1 ekonomi politikası (karar)

| Parametre | Karar | Dayanak |
|---|---|---|
| Yeni ödül tahsisi | Uygun net sezon gelirinin en fazla %25'i | En kötü durumda marj 0,82 (§5.4) |
| Gelir kaynağı | Ayrı Rekt Mining gelir payı (Herd Vault'tan ayrı) | §7.1 |
| Puan tabanı | Yalnızca **tahsil edilmiş** %0,45 protokol ücreti | Creator / arayüz payı farmcıya dönebilir (§2) |
| Zarar çarpanı | 1,5x; yalnızca 7+ gün tutulup zararla kapanan uygun pozisyonlarda | Şartsız çarpan farmcıya bedava (§5.1) |
| Tutma çarpanı | 1,25x (7+ gün) | |
| Toplam çarpan tavanı | 2x | Güvenliği belirleyen değer (§5.5) |
| Puanlanan sezon | Pozisyonun **kapandığı** sezon; 7 gün sayacı açılıştan başlar, sezon değişiminde sıfırlanmaz; bir kapanış bir kez puanlanır | §7.2 |
| Ödül zamanı | Sezon kapanışından sonra | §7.2 |
| Claim süresi | Açık ve önceden ilan edilmiş bir süre (öneri: 2 sezon); sonunda ne olacağı §7.4'te | §7.4 |
| Bakiyeler | Yeni tahsis, harcanmayan SOL ve talep edilmemiş $HERD ayrı tutulur | §7.4 |
| $HERD edinimi | Gelirle kontrollü alım; yeni token basımı yok | §5.7 – §5.10 |
| Alım fiyat etkisi | İşlem başına en fazla %0,5; aşılırsa alım durur. Toplam maliyet garantisi değildir | §7.5 |
| Diamond Hooves | Ayrı bütçe (gelir / 50) ve ayrı muhasebe; iade ödenen ücreti aşmaz | Koşulda ayrı terim (§4) |
| Trend / leaderboard | Ham hacim yerine uygun ücret ve doğrulanmış aktivite | §7.3 |
| Güvenlik iddiaları | Gerçek likidite ve zincir üstü verilerle doğrulanana kadar **model varsayımı** | §6 |

### 7.1 Uygun net gelir ve protokol gelirinin sırası

**Uygun net gelir** $R_t$: sezon $t$ içinde **tahsil edilmiş** protokol ücretleri (%0,45 payı), eksi o sezondaki
geri alınan tutarlar (iadeler, başarısız işlemler). Şunlar gelir **sayılmaz**: önceki sezondan kalan SOL,
talep edilmemiş ya da devreden $HERD, $HERD fiyat artışı, kasa faizi ve iç transferler.

Protokol payı şu sırayla dağılır:

1. **Diamond Hooves:** $R_t$ / 50.
2. **Operasyon tabanı** $O_t$ (RPC, indexer, audit, hukuk).
3. **Rekt Mining yeni tahsisi:** $\min(\alpha R_t,\ \text{kalan})$, $\alpha \le 0{,}25$.
4. Kalan: Herd Vault / Builders Fund / Operasyon (50 / 30 / 20).

Rekt Mining payı tabandan sonra gelir; düşük gelirli sezonda operasyonu aç bırakmaz. Tahsis $\alpha R_t$'den
küçük olabilir ama büyük olamaz; güvenlik koşulu yalnızca bu üst sınıra dayanır.

### 7.2 Sezon akışı

1. **Sezon boyunca:** Uygun pozisyonlar kapandıkça puanlanır. Puanlar göreli olduğu için sezon içinde kimseye
   sabit değer vaat edilmez.
2. **Kapanış:** Sezonun son bloğu sabitlenir. Yalnızca tahsil edilmiş ücretler sayılır: başarısız işlemler,
   iptal edilen Lobby'ler ve iadeler puan üretmez.
   - Pozisyon **kapandığı sezonda** puanlanır; kapanışta hâlâ açık olan pozisyon o sezonda puan almaz.
   - 7 günlük tutma süresi pozisyonun **açıldığı andan** itibaren sayılır; sezon değişimi süreyi sıfırlamaz.
   - Her kapanış yalnızca bir kez puanlanır (pozisyon kimliğiyle).
3. **Liste:** Puanlar zincir verisinden açık kaynaklı betikle hesaplanır, Merkle kökü zincire yazılır ve itiraz
   süresi tanınır (Rekt-back v1 ile aynı yöntem). Creator cüzdanları ve kasalar hariçtir.
4. **Kontrollü alım:** §7.5'teki kurallarla, alım penceresi içinde (öneri: kapanıştan sonraki 14 gün).
5. **Claim:** Herkes payını Merkle kanıtıyla, claim süresi içinde çeker (pull-claim).

$HERD çıkmadan önceki sezonlarda 1–3. adımlar aynı işler; tahsis SOL olarak sezonun defterinde bekler ve alım
$HERD lansmanından sonra aynı kurallarla yapılır.

### 7.3 Trend ve leaderboard

- Board ve trend sıralaması: tokenin **uygun protokol ücreti** (tahsil edilmiş) ve doğrulanmış aktivite
  (benzersiz holder, Stampede katılımı, 7+ gün tutan cüzdan sayısı). Ham hacim sıralamada kullanılmaz.
- Kullanıcı leaderboard'u: sezon puanı (yukarıdaki kurallarla) ve Herd Caller / Herd rank (itibar, ayrı).
- Tek başına ham hacim hiçbir yerde ödül veya görünürlük kazandırmaz.

### 7.4 Üç ayrı bakiye

| Bakiye | Ne | Kural |
|---|---|---|
| **Yeni tahsis** | Sezon $t$ için SOL | En fazla $\alpha R_t$; kapanışta sabitlenir, sonradan artmaz |
| **Harcanmayan SOL** | Sezon $t$'nin henüz $HERD'e çevrilmemiş SOL'ü | Yalnızca sezon $t$'ye aittir; $R_{t+1}$'e eklenmez, yeni puan dağıtımına girmez. Alım penceresi biterse sezon $t$'nin listesine SOL olarak, puanla orantılı ödenir |
| **Talep edilmemiş $HERD** | Sezon $t$ için alınmış ama çekilmemiş token | Claim yükümlülüğü olarak tutulur, yeniden satın alınmaz. Claim süresi dolunca devir rezervine geçer |

**Devir rezervi** (süresi dolmuş, talep edilmemiş $HERD): Sonraki bir sezonda yalnızca **o sezonun kendi tavanı
içinde**, alış maliyetiyle sayılarak kullanılabilir. O sezon yeni alımı aynı tutar kadar azaltır ve yerine geçtiği
SOL protokole döner. Talep edilmemiş SOL da süre sonunda protokole döner. Böylece bir sezonun dağıttığı değer
(maliyetle) hiçbir zaman o sezonun tahsisini aşmaz:

$$\text{devir kullanımı}_t + \text{harcanan SOL}_t + \text{SOL borcu}_t = \text{tahsis}_t \le \alpha R_t$$

### 7.5 Kontrollü alım kuralları

- **İşlem başına tavan %0,5.** Her parçadan önce tahmini etki hesaplanır (parça / anlık havuz derinliği). Tahmin
  tavanı aşarsa işlem yapılmaz ve alım o pencere için durur.
- **Zincir üstü koruma:** Her işlemin min-out'u, işlem öncesi fiyatın %0,5 üstüne ve fiyat teklifinin kayma
  toleransı kadar altına göre belirlenir (hangisi daha sıkıysa). Gerçekleşen etki bunu aşarsa işlem geri döner
  ve alım durur.
- **Kayma toleransı** tavandan ayrı ve dar tutulur (öneri %0,1, §5.9).
- **Zamanlama** önceden ilan edilmez; parça büyüklüğü havuz derinliğine göre seçilir.
- **Raporlama:** Her sezon için havuz derinliği, tahmini etki, gerçekleşen ortalama fiyat, başarısız işlemler ve
  harcanmayan SOL yayımlanır.
- **Durma bir güvenlik garantisi değildir.** Parçalamak ve durmak tek işlemdeki ani etkiyi ve sandwich alanını
  sınırlar; toplam maliyeti sınırlamaz (§5.8).

## 8. Değişmezler ve testler

| # | Değişmez | Test |
|---|---|---|
| R1 | Curve'de al-sat her zaman zararla kapanır ve en az ücretin tamamını öder | `round trip on the curve` |
| R2 | İki cüzdanlı turda küme kâr etmez | `pair trip` |
| R3 | 0 gün tutulan pozisyon zarar çarpanı almaz | `hold gate` |
| R4 | Çarpan hiçbir zaman üst sınırı aşmaz | `multipliers never exceed the cap` |
| R5 | Modelde güvenlik marjı ≥ 0 iken hiçbir farm büyüklüğünün ödül kaynaklı net kârı pozitif değildir (300 rastgele parametre seti) | `capped pool: ... no farm size is profitable` |
| R6 | Marj < 0 ise kârlı bir farm vardır (koşul sıkıdır) | `capped pool: the condition is tight` |
| R7 | Sabit havuzda kapalı formül ve ızgara araması aynı sonucu verir | `fixed pool` |
| R8 | Dağıtılan ödül havuzu aşmaz; Diamond Hooves ödenen ücreti aşmaz | `separate accounting` |
| R9 | Sandwich ödülsüz de kârlıdır (MEV ayrı sorun) | `sandwich pays without any reward` |
| R10 | Politika havuzu Diamond Hooves ve operasyon tabanını önce karşılar, $\alpha R_t$'yi asla aşmaz | `policy pool: funds Diamond Hooves ...` |
| R11 | Modelde politika havuzunda marj ≥ 0 iken her gelir seviyesi ve operasyon tabanında ödül kaynaklı net farm kârı pozitif değildir | `policy pool: with the margin holding ...` |
| R12 | Uygun net gelir yalnızca o sezon tahsil edilen ücretlerdir; rezervler ve fiyat artışı sayılmaz | `eligible revenue` |
| R13 | Pozisyon kapandığı sezonda bir kez puanlanır; 7 gün sayacı sezon değişiminde sıfırlanmaz | `scoring` |
| R14 | Devir dahil hiçbir sezon tahsisinden fazlasını dağıtmaz (40 rastgele sezon) | `treasury: a season never distributes more than its cap` |
| R15 | Harcanmayan SOL kendi sezonunda kalır; sonraki tahsisi değiştirmez | `treasury: unspent SOL stays with its season` |
| R16 | Claim bir kez ve süre içinde; süresi dolan $HERD yalnızca sonraki tavanın içinde kullanılır | `treasury: claims are once ...` |
| R17 | Arbitraj yoksa parçalamak toplam fiyatı düşürmez | `purchase: splitting does not lower the total cost` |
| R18 | Saldırgan varken bile gerçekleşen hiçbir işlem tavanı aşmaz | `purchase: no executed trade exceeds the per-trade limit` |
| R19 | Tahmin tavanı aşarsa ya da likidite çekilirse alım durur, SOL harcanmaz | `purchase: stops when ...` |

Bu testler kodun yazılan kuralları uyguladığını gösterir; ekonomik güvenliğin gerçek piyasada kanıtı değildir.

## 9. Açık sorular

- Operasyon tabanı $O_t$'nin tutarı ve sezon süresi.
- Claim süresinin kesin değeri (öneri: 2 sezon) ve alım penceresi (öneri: kapanıştan sonra 14 gün).
- Kontrollü alımın yapılacağı havuz, gerçek derinliği ve ücreti; kayma toleransının kesin değeri (öneri %0,1).
- Gerçek dürüst karışım ($\bar m$, $e_H$): Paper Season / test ağı verisiyle ölçülecek.
- Sandwich / MEV koruması için ayrı şartname.
- Referral payı puan tabanına girer mi? (Girerse arayüz payı gibi farmcıya dönebilir; $c_F$ düşer.)
- Ödül $HERD'üne vesting uygulanır mı? (Satış baskısını azaltır, ödülün çekiciliğini düşürür.)
