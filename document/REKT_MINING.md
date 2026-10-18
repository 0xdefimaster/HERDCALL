# Rekt Mining — Ekonomik Model ve Farm Simülasyonu (v0.1, taslak)

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
- **Havuz değeri sınırlı (önerilen):** $V_t = \min(B_t \cdot P_{HERD},\ \alpha \cdot R_t)$. $R_t$, sezonda
  toplanan protokol geliridir (farmcının ödedikleri dahil). $B_t$ token adedi olarak tavan olmaya devam eder;
  değeri $\alpha R_t$'yi aşan kısım dağıtılmaz. Havuzun $HERD öncesi puanlardan oluştuğu dönemde de
  puanların dönüşeceği değer bu sınırla ilan edilir.
- **Reddedilen: sabit token bütçesi** $V_t = B_t \cdot P_{HERD}$. Enflasyonu sınırlar ama farmı sınırlamaz (§5.3).

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

## 6. Bilinen sınırlar

- **Ödül dışı faydalar:** Koşul, farmcının yalnızca ödülden kazandığını varsayar. Hacimle trend olmak, Board'da
  üste çıkmak, creator ücreti (zaten $c_F$'de) gibi faydalar farmı kâr etmese bile anlamlı kılabilir.
  Önlem: Board ve keşif sıralaması ham hacimle yapılmaz (benzersiz holder, Stampede katılımı, tutma).
- **Hesaplar arası örüntü:** Model, farmcının kendi cüzdanlarını tanıyamadığımızı varsayar ve buna rağmen güvenlidir.
  Örüntü takibi ek bir katman olabilir ama güvenlik ona dayanmaz.
- **Dürüst karışım varsayımdır:** $\bar m$ ve $e_H$ gerçek veriye bağlıdır. $e_H$ küçükse (az kişi uzun tutup
  zararla çıkıyorsa) Diamond Hooves terimi büyür: $e_H = 0{,}05$ için $1/0{,}05/50 = 0{,}4$. $\bar m = 1$ alınsa
  bile marj öneri parametreleriyle pozitif kalır ($1{,}33 - 0{,}25 \cdot 2 - 0{,}4 = 0{,}43$).
- **Satış baskısı:** Sınırlı havuz, sezon başına dağıtılan değeri $\alpha R_t$ ile sınırlar. Havuz protokol
  geliriyle piyasadan alınan $HERD'le (Herd Vault buyback) fonlanırsa yeni emisyon olmaz. Basılan token'la
  fonlanırsa sezon başına satış baskısı en fazla $\alpha R_t$ değerindedir. $HERD'ün likiditesi ve fiyatı
  bu modelin dışındadır.
- **Sermaye ve fiyat riski:** S3'te fiyat riski ortalamada sıfıra yakındır ama sapması büyüktür (5 SOL'lük
  bacaklarda ±1,4 SOL). Riskten kaçınan farmcı için gerçek maliyet tablodakinden yüksektir; model bunu
  farmcı lehine yok sayar.

## 7. v0.1 önerisi

| Parametre | Değer | Gerekçe |
|---|---|---|
| Puan tabanı | Protokol ücreti (%0,45) | Creator / arayüz payı farmcıya dönebilir (§2) |
| Havuz | $V_t = \min(B_t P,\ \alpha R_t)$, $\alpha = 0{,}25$ | En kötü durumda marj 0,82 (§5.4) |
| Zarar çarpanı | 1,5x, yalnızca 7+ gün tutulan pozisyonlarda | Şartsız çarpan farmcıya bedava (§5.1) |
| Tutma çarpanı | 1,25x (7+ gün) | |
| Üst sınır $M$ | 2 | Güvenliği belirleyen değer (§5.5) |
| Diamond Hooves | Ayrı bütçe (gelir / 50), ayrı muhasebe, ücreti aşmaz | Koşulda ayrı terim (§4) |

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

## 9. Açık sorular

- $\alpha$'nın kaynağı: Herd Vault payından mı (§7, 50/30/20), yoksa ayrı bir kalem mi?
- Puanların $HERD'e dönüşüm takvimi ve vesting'i.
- Gerçek dürüst karışım ($\bar m$, $e_H$): Paper Season / test ağı verisiyle ölçülecek.
- Sandwich / MEV koruması için ayrı şartname.
- Referral payı puan tabanına girer mi? (Girerse arayüz payı gibi farmcıya dönebilir; $c_F$ düşer.)
