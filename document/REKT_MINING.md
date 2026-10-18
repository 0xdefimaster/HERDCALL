# Rekt Mining — Ekonomik Model ve Farm Simülasyonu (v0.1)

> "Kaybetsen bile bir şey kazanırsın" psikolojisini (Papertrade, EOS betting mining), kayba göre sınırsız token
> basmadan HerdCall'a taşıyan sezonluk puan sisteminin ekonomik tanımı.
> Bu belge **kontrat kodu değildir**; parametreler kesinleşmeden önce sağlanması gereken koşulları tanımlar.
> Model ve testler: [`sim/rekt-mining.js`](../sim/rekt-mining.js), [`sim/rekt-mining.test.js`](../sim/rekt-mining.test.js).
> Çalıştırmak için: `npm run test:sim` (testler), `npm run sim:rekt` (senaryo tabloları).

**Cevaplanan soru:** Farmcı, kendi ödülünü büyütmek için ne kadar ücret yakmak zorunda ve bu sırada dürüst
kullanıcıların ödül payını ne kadar azaltıyor?

**Kısa cevap:**
- Ödül havuzunun değeri, o sezon toplanan protokol gelirinin sabit bir payıyla ($\alpha$) sınırlanırsa ve
  §4'teki koşul sağlanırsa, **hiçbir farm büyüklüğü kâr etmez**. Bu yalnızca simülasyon sonucu değil,
  ispatlanan bir sınırdır ve testle doğrulanır.
- Bütçe yalnızca token adediyle sabitlenirse ($B_t$), $HERD fiyatı yükseldikçe farm kârlı hâle gelir. Örnek:
  havuz değeri dürüst ücretlerin 2 katına çıkınca dürüst kullanıcılar ödülün yalnızca %63'ünü alır (§5.3).
- Kayba bağlı çarpan **tutma şartı olmadan** farmcıya bedavadır: her al-sat turu zararla kapanır (§5.1).
- **v0.1 ekonomi politikası kabul edildi (§7):** havuz SOL cinsinden ve gelirle sınırlı, puanlar göreli, $HERD sezon
  sonunda gelirle kontrollü alınır, yeni token basılmaz. Bu politikayla $HERD fiyatı farm ekonomisini değiştirmez (§5.7).

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
  $V_t = \min\left(\alpha R_t,\ R_t - R_t/50 - O_t\right)$. $R_t$, sezonda tahsil edilen protokol geliridir
  (farmcının ödedikleri dahil), $R_t/50$ Diamond Hooves bütçesi, $O_t$ operasyon tabanıdır. Havuz önce SOL olarak
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

| Tasarım | $\alpha$ | Güvenlik marjı | En iyi farm kârı | $1\times H$ yakılırsa dürüste kalan |
|---|---|---|---|---|
| Zarar 2x, şart yok | 0,25 | 0,84 | 0 | %78 |
| Zarar 2x, şart yok | 1,00 | −0,35 | 15 SOL | %78 |
| Zarar 2x, 7 gün şartı | 0,25 | 0,72 | 0 | %65 |
| Zarar 2x, 7 gün şartı | 1,00 | −0,84 | 54 SOL | %65 |
| **Öneri** | 0,10 | 1,07 | 0 | %75 |
| **Öneri** | **0,25** | **0,82** | **0** | **%75** |
| **Öneri** | 0,50 | 0,40 | 0 | %75 |
| **Öneri** | 1,00 | −0,43 | 22 SOL | %75 |

- $\alpha \le 0{,}5$ olduğunda hiçbir tasarımda farm kâr etmez. $\alpha = 1$ (gelirin tamamı ödül) her tasarımda
  kârlı farm açar.
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

| Dürüst hacim (SOL) | Protokol geliri $H$ | Havuz (SOL) | Havuz / gelir | En iyi farm kârı | $1\times H$ yakılırsa dürüste kalan |
|---|---|---|---|---|---|
| 10.000 | 45 | 0 | %0 | 0 | — |
| 50.000 | 225 | 56,2 | %25 | 0 | %75 |
| 100.000 | 450 | 112,5 | %25 | 0 | %75 |
| 1.000.000 | 4.500 | 1.125 | %25 | 0 | %75 |
| 10.000.000 | 45.000 | 11.250 | %25 | 0 | %75 |

- Koşul ölçekten bağımsızdır: her gelir seviyesinde farm kâr etmez.
- Gelir operasyon tabanını karşılamıyorsa o sezon havuz yoktur. Taban dolarken havuz, yakılan her ücretle
  $\alpha$'dan hızlı büyüyebilir, ama $V_t \le \alpha R_t$ her zaman geçerli olduğu için koşul bozulmaz (test R10).

### 5.7 $HERD fiyat senaryoları ($H = 450$ SOL)

Sabit token bütçesi, 1x fiyatta politikadaki havuzla aynı değere gelecek şekilde seçildi.

| $HERD fiyatı | Sabit $B_t$: havuz değeri | Sabit: farm kârı | Sabit: dürüste kalan | Politika: havuz | Politika: farm kârı | Politika: alınan $HERD (M) |
|---|---|---|---|---|---|---|
| 0,1x | 11 | 0 | %100 | 112,5 | 0 | 11,23 |
| 0,5x | 56 | 0 | %100 | 112,5 | 0 | 2,25 |
| 1x | 113 | 0 | %100 | 112,5 | 0 | 1,12 |
| 2x | 225 | 0 | %100 | 112,5 | 0 | 0,56 |
| 5x | 563 | 27 | %79 | 112,5 | 0 | 0,22 |
| 10x | 1.125 | 221 | %56 | 112,5 | 0 | 0,11 |

- Sabit bütçede fiyat 5 katına çıkınca farm kârlı olur ve dürüst pay düşer. Politikada havuz değeri fiyattan
  bağımsızdır; fiyat yalnızca dağıtılan token adedini değiştirir.

### 5.8 Kontrollü alımın fiyat etkisi

Sabit çarpımlı $HERD/SOL havuzu; parçalar arasında arbitrajın fiyatı geri getirdiği varsayılır. Tablo, alım
fiyatının başlangıç fiyatına göre ne kadar yüksek ödendiğini gösterir. Bu fark aynı zamanda alımın önüne
geçen (front-run) birinin en fazla yakalayabileceği alandır.

| Havuz / $HERD likiditesi | Tek alım | 7 parça | 28 parça (günde 4, 7 gün) | Parça başı ≤ %0,5 için gereken parça |
|---|---|---|---|---|
| 0,01 | %1,00 | %0,14 | %0,04 | 2 |
| 0,05 | %5,00 | %0,71 | %0,18 | 10 |
| 0,10 | %10,00 | %1,43 | %0,36 | 20 |
| 0,50 | %50,00 | %7,14 | %1,79 | 100 |
| 1,00 | %100,00 | %14,29 | %3,57 | 200 |

- Parça başı etki = parça / likidite. Kural: parça başı etki en fazla %0,5; parça sayısı buna göre seçilir,
  zamanlar önceden ilan edilmez.
- Havuz likiditeye göre büyükse (oran 0,5 ve üstü) alım haftalara yayılmalı ya da bir kısmı sonraki sezona
  devretmeli. Bu, $HERD likiditesi bilinmeden kesinleştirilemez.

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
  farklı olabilir. $HERD'ün
  likiditesi ve fiyat davranışı bu modelin dışındadır.
- **Alımın önüne geçme:** Havuzun büyüklüğü herkese açıktır. Alımı bilen biri önceden $HERD alıp alıma satabilir.
  Kazancı en fazla alımın fiyat etkisi kadardır (§5.8); parça başı etki sınırı ve önceden ilan edilmeyen
  zamanlama bunu küçük tutar.
- **Bu sonuçların geçerlilik alanı:** "Hiçbir farm kâr etmez" sonucu §4'teki varsayımlar altında geçerlidir:
  farmcının tek kazancı ödüldür, ücretler gerçekten tahsil edilir ve havuz $\alpha R_t$'yi aşmaz. Gerçek sistemde
  wash trading, çoklu hesaplar, MEV, fiyat oynaklığı ve alımın piyasa etkisi ayrıca test edilmelidir. Bu
  modelde çoklu hesap (S2, S3), sandwich (S4) ve alım etkisi (§5.8) ölçüldü; zincir üstü davranış ve
  gerçek likidite ölçülmedi.
- **Trend ve sıralama:** Board, trend ya da leaderboard ham hacme göre görünürlük veya ödül verirse, Rekt
  Mining'den bağımsız olarak manipülasyon teşviki yaratır. v0.1'de sıralama uygun protokol ücreti ve
  doğrulanmış aktiviteyle yapılır (§7).
- **Sermaye ve fiyat riski:** S3'te fiyat riski ortalamada sıfıra yakındır ama sapması büyüktür (5 SOL'lük
  bacaklarda ±1,4 SOL). Riskten kaçınan farmcı için gerçek maliyet tablodakinden yüksektir; model bunu
  farmcı lehine yok sayar.

## 7. v0.1 ekonomi politikası (karar)

| Parametre | Karar | Dayanak |
|---|---|---|
| $\alpha$ | En fazla %25 | En kötü durumda marj 0,82 (§5.4) |
| Gelir kaynağı | Ayrı Rekt Mining gelir payı (Herd Vault'tan ayrı) | §7.1 |
| Puan tabanı | Yalnızca **tahsil edilmiş** %0,45 protokol ücreti | Creator / arayüz payı farmcıya dönebilir (§2) |
| Zarar çarpanı | 1,5x; yalnızca 7+ gün tutulup zararla kapanan uygun pozisyonlarda | Şartsız çarpan farmcıya bedava (§5.1) |
| Tutma çarpanı | 1,25x (7+ gün) | |
| Toplam çarpan tavanı | 2x | Güvenliği belirleyen değer (§5.5) |
| Ödül zamanı | Sezon kapanışından sonra | §7.2 |
| $HERD edinimi | Gelirle kontrollü alım; yeni token basımı yok | §5.7, §5.8 |
| Diamond Hooves | Ayrı bütçe (gelir / 50) ve ayrı muhasebe; iade ödenen ücreti aşmaz | Koşulda ayrı terim (§4) |
| Trend / leaderboard | Ham hacim yerine uygun ücret ve doğrulanmış aktivite | §6 |

### 7.1 Protokol gelirinin sırası

Protokol payı (%0,45) şu sırayla dağılır:

1. **Diamond Hooves:** gelir / 50.
2. **Operasyon tabanı** $O_t$ (RPC, indexer, audit, hukuk).
3. **Rekt Mining payı:** $\min(\alpha R_t,\ \text{kalan})$, $\alpha \le 0{,}25$.
4. Kalan: Herd Vault / Builders Fund / Operasyon (50 / 30 / 20).

Rekt Mining payı tabandan sonra gelir; düşük gelirli sezonda operasyonu aç bırakmaz. Havuz $\alpha R_t$'den
küçük olabilir ama büyük olamaz; güvenlik koşulu yalnızca üst sınıra dayanır.

### 7.2 Sezon akışı

1. **Sezon boyunca:** Uygun pozisyonlar puan biriktirir. Rekt Mining payı SOL olarak ayrı bir kasada birikir.
   Puanlar göreli olduğu için sezon içinde kimseye sabit değer vaat edilmez.
2. **Kapanış:** Sezonun son bloğu sabitlenir. Yalnızca tahsil edilmiş ücretler sayılır: başarısız işlemler,
   iptal edilen Lobby'ler ve iadeler puan üretmez. Bir pozisyon kapandığı sezonda sayılır; kapanışta hâlâ açık
   olan pozisyon, kapandığı sezona kalır.
3. **Liste:** Puanlar zincir verisinden açık kaynaklı betikle hesaplanır, Merkle kökü zincire yazılır ve itiraz
   süresi tanınır (Rekt-back v1 ile aynı yöntem). Creator cüzdanları ve kasalar hariçtir.
4. **Kontrollü alım:** Kasadaki SOL ile $HERD, parça başı fiyat etkisi en fazla %0,5 olacak şekilde, önceden ilan
   edilmeyen zamanlarda parça parça alınır (§5.8).
5. **Claim:** Herkes payını Merkle kanıtıyla çeker (pull-claim). Alınmayan paylar sonraki sezona devreder.

$HERD çıkmadan önceki sezonlarda 1–3. adımlar aynı işler; SOL kasada bekler ve alım $HERD lansmanından sonra
aynı kurallarla yapılır.

### 7.3 Trend ve leaderboard

- Board ve trend sıralaması: tokenin **uygun protokol ücreti** (tahsil edilmiş) ve doğrulanmış aktivite
  (benzersiz holder, Stampede katılımı, 7+ gün tutan cüzdan sayısı). Ham hacim sıralamada kullanılmaz.
- Kullanıcı leaderboard'u: sezon puanı (yukarıdaki kurallarla) ve Herd Caller / Herd rank (itibar, ayrı).
- Tek başına ham hacim hiçbir yerde ödül veya görünürlük kazandırmaz.

## 8. Değişmezler ve testler

| # | Değişmez | Test |
|---|---|---|
| R1 | Curve'de al-sat her zaman zararla kapanır ve en az ücretin tamamını öder | `round trip on the curve` |
| R2 | İki cüzdanlı turda küme kâr etmez | `pair trip` |
| R3 | 0 gün tutulan pozisyon zarar çarpanı almaz | `hold gate` |
| R4 | Çarpan hiçbir zaman üst sınırı aşmaz | `multipliers never exceed the cap` |
| R5 | Güvenlik marjı ≥ 0 iken hiçbir farm büyüklüğü kâr etmez (300 rastgele parametre seti) | `capped pool: ... no farm size is profitable` |
| R6 | Marj < 0 ise kârlı bir farm vardır (koşul sıkıdır) | `capped pool: the condition is tight` |
| R7 | Sabit havuzda kapalı formül ve ızgara araması aynı sonucu verir | `fixed pool` |
| R8 | Dağıtılan ödül havuzu aşmaz; Diamond Hooves ödenen ücreti aşmaz | `separate accounting` |
| R9 | Sandwich ödülsüz de kârlıdır (MEV ayrı sorun) | `sandwich pays without any reward` |
| R10 | Politika havuzu Diamond Hooves ve operasyon tabanını önce karşılar, $\alpha R_t$'yi asla aşmaz | `policy pool: funds Diamond Hooves ...` |
| R11 | Politika havuzunda marj ≥ 0 iken her gelir seviyesi ve operasyon tabanında farm kâr etmez | `policy pool: with the margin holding ...` |
| R12 | Kontrollü alımda parça başı fiyat etkisi parça / likiditedir | `buyback` |

## 9. Açık sorular

- Operasyon tabanı $O_t$'nin tutarı ve sezon süresi.
- Kontrollü alımın yapılacağı havuz ve likidite eşiği; havuz likiditeye göre büyükse devretme kuralı (§5.8).
- Gerçek dürüst karışım ($\bar m$, $e_H$): Paper Season / test ağı verisiyle ölçülecek.
- Sandwich / MEV koruması için ayrı şartname.
- Referral payı puan tabanına girer mi? (Girerse arayüz payı gibi farmcıya dönebilir; $c_F$ düşer.)
- Claim süresi (öneri: alınmayan paylar sonraki sezona devreder).
- Ödül $HERD'üne vesting uygulanır mı? (Satış baskısını azaltır, ödülün çekiciliğini düşürür.)
