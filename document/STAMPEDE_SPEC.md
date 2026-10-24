# Stampede — Teknik Şartname (v0.2, taslak)

> Lobby → Stampede açılışının matematiksel ve durum makinesi tanımı.
> Bu belge **kontrat kodu değildir**; kontrat yazılmadan önce kabul edilmesi gereken davranışı tanımlar.
> Referans model ve otomatik testler: [`sim/stampede.js`](../sim/stampede.js), [`sim/stampede.test.js`](../sim/stampede.test.js).
> Çalıştırmak için: `npm run test:sim` (testler), `npm run sim` (senaryo tabloları).

**v0.2'deki değişiklik:** v0.1'de Lobby curve'ün sabit bir dilimini ($S_L$) ilk dilimin ortalama fiyatından
satıyordu. Simülasyon, gerçekçi talepte paranın %90'ından fazlasının iade edildiğini ve karşılanmayan talebin
curve'ün ilk bloklarına yığıldığını gösterdi. v0.2'de **üst sınır yoktur**: Lobby belirli bir süre açık kalır,
toplanan havuz süre bitince **tek bir alım** yapar, herkes yatırdığıyla orantılı token alır ve **herkes curve'ün
açıldığı fiyattan** girer. İade yoktur (yalnızca boş Lobby'de). Reddedilen alternatif §9.2'de.

---

## 0. Kapsam

- **Kapsamda:** Lobby'nin açılması, yatırma, kapanış, tek fiyatlı settlement, tahsis, iptal, claim/refund,
  yuvarlama tozları, curve'ün settlement sonrası başlangıç durumu ve ödeme gücü (solvency).
- **Kapsam dışı:** Curve açıldıktan sonraki ücretler, breakout, Bonded Creator kasası, Rekt-back.
  Bunlar ayrı şartnamelerde tanımlanacak.
- **Zincirden bağımsız:** Tutarlar zincirin native varlığında (SOL ya da ETH) ve en küçük biriminde
  (lamport / wei) tam sayı olarak ifade edilir. Token tutarları da en küçük biriminde tam sayıdır.
  Belgede "native" kelimesi SOL veya ETH yerine kullanılır.

## 1. Terimler ve parametreler

| Sembol | Anlamı | Örnek değer (simülasyon) |
|---|---|---|
| $V_0$ | Curve'ün başlangıç sanal native rezervi | 30 native |
| $T_0$ | Curve'ün başlangıç sanal token rezervi (CURVE_SPEC: $T_0 \le S_{curve} + S_{LP}$) | 1.000.000.000 token |
| $S_{total}$ | Token'ın toplam arzı (mcap hesabı için) | 1.000.000.000 token |
| $S_{curve}$ | Curve üzerinden satılabilecek arz; **şart: $S_{curve} \ge T_0/2$** | 800.000.000 token |
| $f$ | İşlem ücreti, baz puan (bps) | 100 (= %1) |
| $B$ | Baz puan paydası | 10.000 |
| $d_{min}$ | Tek bir yatırmanın alt sınırı | 0,01 native |
| $D_{min}$ | Curve'ün açılması için toplam talep alt sınırı | 0,1 native (simülasyon); program önerisi 1 SOL (HERDCALL.md §12) |
| $D_{max}$ | Lobby'nin toplam talep üst sınırı | 10.000 native |
| $w_{max}$ | Cüzdan başı üst sınır (0 = yok) | 0 |
| $t_{close}$ | Lobby'nin kapanış zamanı (geri sayımın sonu) | açılış + 15/30/60 sn |
| $t_{deadline}$ | Settlement için son an ($t_{close} < t_{deadline}$) | $t_{close}$ + 300 sn |
| $d_i$ | $i$ adresinin toplam yatırdığı native (aynı adresin yatırmaları toplanır) | — |
| $D$ | Havuz: toplam yatırılan native, $D = \sum_i d_i$ | — |
| $n$ | Yatırma yapmış farklı adres sayısı | — |

Token 6, native 9 ondalık basamakla modellenmiştir.

## 2. Bonding curve

Sanal rezervli sabit çarpım eğrisi kullanılır. Curve'ün durumu $(v, t, k, R, s)$:
$v$ ve $t$ sanal rezervler, $k$ çarpım sabiti, $R$ curve'ün gerçekten tuttuğu native (rezerv),
$s$ curve'den çıkmış ve dolaşımdaki token miktarıdır. **Spot fiyat** $p = v / t$'dir.

**Alım ve satım** (yuvarlama her zaman protokol lehine):

$$\text{alım}(\Delta) = \left\lceil \frac{k}{t - \Delta} \right\rceil - v, \qquad
\text{satım}(\Delta) = v - \left\lceil \frac{k}{t + \Delta} \right\rceil$$

Alımda $v$ ve $R$ ödenen tutar kadar artar, $t$ azalır; satımda tersi. Bu yuvarlama ile her işlemden sonra
$v \cdot t \ge k$ kalır (test: `CurveState`, `solvency`).

**Lobby öncesi durum:** $v = V_0$, $t = T_0$, $k = V_0 T_0$, $R = 0$, $s = 0$.

**Lobby sonrası durum (yeniden çapalama).** Settlement, curve'ü Lobby'nin ödediği fiyattan açar (§4.4):

$$v_1 = V_0 + N, \quad t_1 = T_0 - X, \quad k_1 = v_1 t_1, \quad R_1 = N, \quad s_1 = X$$

$k_1 > V_0 T_0$ olur; curve Lobby'den sonra yeni çarpım sabitiyle devam eder. Bu bilinçli bir tasarımdır ve
ödeme gücü §4.6'da ispatlanır.

## 3. Durum makinesi

```
            deposit()                 settle()  (t_close ≤ now < t_deadline)
  ┌──────┐  now < t_close  ┌──────────┐ ───────────────────────────────────▶ ┌─────────┐
  │ Open │ ───────────────▶│ Settling │                                      │ Settled │
  └──────┘   (zamanla)     └──────────┘ ───────────────────────────────────▶ └─────────┘
                                 │        cancel()  (now ≥ t_deadline)       ┌───────────┐
                                 └──────────────────────────────────────────▶│ Cancelled │
                                                                             └───────────┘
```

| Durum | Nasıl girilir | İzin verilen işlemler |
|---|---|---|
| **Open** | Lobby oluşturulunca; $now < t_{close}$ | `deposit` |
| **Settling** | Zamanla, $t_{close} \le now$ ve henüz nihai durum yok | `settle` (yalnızca $now < t_{deadline}$), `cancel` (yalnızca $now \ge t_{deadline}$) |
| **Settled** | `settle` başarıyla çalışınca (nihai) | `claim`, `sweepDust`, curve işlemleri |
| **Cancelled** | `cancel` çalışınca (nihai) | `refund` |

**Yarış koşulunun önlenmesi.**
1. `settle` ve `cancel` için zaman pencereleri **ayrıktır**: settle $[t_{close}, t_{deadline})$, cancel $[t_{deadline}, \infty)$.
2. İkisi de nihai durum yokken çalışabilir ve nihai durumu **tek bir atomik adımda** yazar.
   Settled bir Lobby iptal edilemez, Cancelled bir Lobby settle edilemez.
3. İadeler (`refund`) yalnızca **Cancelled** durumunda açılır.
4. Open süresince **çekme (withdraw) yoktur.** Sahte talep gösterip son anda çekme oyununu engeller.
   Fonlar yine de kilitli kalmaz: settle edilmezse $t_{deadline}$ sonrası herkes iptal edip tam iade alabilir.
5. `settle` ve `cancel` **izinsizdir** (herkes çağırabilir).
6. Curve, Lobby Settled olmadan **işleme açılmaz**. Settlement ile curve'ün açılışı aynı atomik adımdır.

Zaman damgası notu: Blok zamanı birkaç saniye oynayabilir; $t_{deadline} - t_{close}$ bu sapmadan çok büyük
seçilmelidir (örnekte 300 sn). Test: `state machine`.

**Settling ara durumu.** EVM'de settlement tek işlemde atomiktir; Settling yalnızca zamanla belirlenen bir
aşamadır. Solana'da hesap sınırları nedeniyle birden fazla işleme bölünmesi gerekirse Settling saklanan bir
durum olur: yarım kalan adımlar fonlara dokunmaz, tüm fon hareketleri son adımda tek seferde yapılır ve
`cancel` yalnızca Settling ayrı, daha uzun bir süre yarım kaldıysa çalışır.

## 4. Settlement hesabı

`settle()` yalnızca toplamlarla çalışır ve katılımcılar üzerinde döngü yapmaz.

**4.1 Boş ya da yetersiz talep.** $D < D_{min}$ ise (ya da hesaplanan $X = 0$ ise): hiçbir fon curve'e gitmez,
ücret alınmaz, **curve açılmaz**: launch **Cancelled** olur ve herkes `refund` ile yatırdığının **tamamını** geri alır.
Ad ve ticker rezervasyonu hemen serbest bırakılabilir (HERDCALL.md §12). $D_{min}$ böylece bir adı boş Lobby ile
tutmayı da engeller. Test: `zero and below-minimum demand`.

**4.2 Ücret ve net tutar.** Havuzun tamamı harcanır; iade yoktur:

$$N = \left\lfloor \frac{D \cdot (B - f)}{B} \right\rfloor, \qquad F = D - N$$

$N$ token alımına (curve rezervine), $F$ ücret yönlendiricisine gider (HERDCALL.md §5 dağılımı).
Ücret hiçbir zaman curve rezervinden düşülmez.

**4.3 Lobby'nin aldığı token miktarı.** Hedef: herkes **curve'ün açılacağı fiyattan** girsin. Yani Lobby'nin
birim fiyatı $N/X$, yeniden çapalanmış curve'ün spot fiyatına eşit olmalı:

$$\frac{N}{X} = \frac{V_0 + N}{T_0 - X} \;\Longrightarrow\; X(V_0 + 2N) = N\,T_0 \;\Longrightarrow\;
X = \left\lfloor \frac{N \cdot T_0}{V_0 + 2N} \right\rfloor$$

**Sonuçlar:**
- $X < T_0/2$ her zaman (talep sonsuza giderken $X \to T_0/2$). Bu yüzden Lobby hiçbir zaman curve'ü
  tüketemez; $S_{curve} \ge T_0/2$ şartıyla curve'ün satacak arzı her zaman kalır. Lobby oluşturulurken bu
  şart kontrol edilir. Test: `parameters`.
- Üst sınır olmadığı için **iade yoktur**: Her native token alır. Test: `settlement: no refunds`.

**4.4 Fiyat sürekliliği (yuvarlama sınırı).** $X$ aşağı yuvarlandığı için Lobby'nin net birim fiyatı curve'ün
açılış spot fiyatından en fazla bir yuvarlama adımı kadar yüksektir; asla düşük değildir:

$$0 \;\le\; N \cdot t_1 - X \cdot v_1 \;<\; V_0 + 2N$$

İspat: $N t_1 - X v_1 = N(T_0 - X) - X(V_0 + N) = N T_0 - X(V_0 + 2N)$. $X$'in tanımı gereği bu değer
$\ge 0$ ve $X$'e bir eklendiğinde negatife düşer, dolayısıyla $< V_0 + 2N$. Test: I11.

**4.5 Tahsis tanecikliği koruması.** Geçerli her yatırmanın ($d_i \ge d_{min}$) en az 1 token birimi alması için
settlement $X \cdot d_{min} \ge D$ şartını arar. Sağlanmazsa `settle` geri döner; $t_{deadline}$ sonrası Lobby
iptal edilir ve herkes tam iade alır. Örnek parametrelerde $D = D_{max}$ iken bile 1 lamport ≈ 54 token birimi
alır; koruma yalnızca hatalı parametrelere karşı bir emniyettir. Test: `granularity guard`.

**4.6 Ödeme gücü (solvency) ispatı.** Curve'ün her an, dolaşımdaki **tüm** tokenler geri satılsa bile
ödeme yapabilmesi gerekir. Tüm $s$ token geri satıldığında sanal token rezervi $t + s$'ye döner ve ödeme:

$$P_{sell\text{-}all} = v - \frac{k}{t + s}$$

Settlement anında $t_1 + s_1 = T_0$ olduğundan $P = v_1 - k_1/T_0 = (V_0 + N)\,X / T_0$. Şart $P \le R_1 = N$:

$$(V_0 + N)\,X \le N\,T_0 \iff X \le \frac{N\,T_0}{V_0 + N}$$

$X = \lfloor N T_0/(V_0 + 2N) \rfloor$ bu sınırın her zaman altındadır. ✓

Settlement sonrası her alım ve satımda $R$ ve $v$ aynı miktarda değişir; yuvarlama protokol lehine olduğundan
**fazlalık (slack) $= R - P_{sell\text{-}all}$ hiçbir zaman azalmaz.** Fazlalık, breakout'ta DEX'e ek likidite
olarak taşınır. Test: `solvency` (100 rastgele Lobby × 200 rastgele işlem, ardından herkesin tamamını satması).

**4.7 Atomik fon hareketleri.** Tek adımda: $N$ curve rezervine, $F$ ücret yönlendiricisine, $X$ token Lobby
escrow'una (claim'ler için); curve $(v_1, t_1, k_1, R_1, s_1)$ durumuyla işleme açılır.

## 5. Claim ve refund

**5.1 Kişi başı tahsis (pull modeli).** Settled durumunda her adres kendi payını talep eder:

$$q_i = \left\lfloor \frac{d_i \cdot X}{D} \right\rfloor \text{ token}$$

Boş Lobby (§4.1) Cancelled olur; Cancelled durumunda `refund` $d_i$'nin tamamını iade eder.
Tahsis yalnızca $d_i$'ye ve toplamlara bağlıdır: **yatırma sırası sonucu değiştirmez.**

**5.2 Çift çekim ve yeniden giriş.**
- Her adres **tek bir kez** çeker; `claim` ya da `refund` ilk çağrıda adresi "çekti" olarak işaretler.
  Durumlar birbirini dışladığı için aynı fon hem iade hem tahsis olarak çekilemez.
- **Önce etki, sonra transfer** (checks-effects-interactions) + yeniden giriş kilidi (reentrancy guard).
- Test: `claims` (transfer sırasında yeniden claim denenir ve reddedilir).

**5.3 Yuvarlama tozu.** Yalnızca token tozu oluşur: $X - \sum q_i < n$ birim. Native tozu yoktur (havuzun
tamamı rezerve ve ücrete gider). Toz escrow'da kalır; **tüm katılımcılar çektikten sonra** `sweepDust` ile
yakılır. Katılımcıların hakkı olan hiçbir tutar zaman aşımıyla süpürülmez.

**5.4 Kilitli fon olmaması.** Settled: claim süresiz. Cancelled: refund süresiz. Settle edilmemiş Lobby:
$t_{deadline}$ sonrası herkes `cancel` çağırabilir. Test: `state machine`, `claims`.

## 6. Fiyat: tek fiyat ve canlı gösterge

Stampede'de **tek bir fiyat** vardır:

| Ad | Tanım | Arayüzde |
|---|---|---|
| **Stampede fiyatı** | Net: $N/X$, ücret dahil: $D/X$. Curve'ün açılış spot fiyatına eşittir (§4.4). | "Everyone enters at X mcap. The curve opens at the same price." |
| **Stampede mcap'i** | Stampede fiyatı $\times S_{total}$ | Lobby sayfasında büyük gösterge |

- Zilden hemen sonra alım yapan biri, curve'ün yapısı gereği her zaman Lobby fiyatından **biraz daha
  yüksek** öder (simülasyon §9.5). Yani Lobby fiyatı, o anda kimsenin altına inemeyeceği en düşük fiyattır;
  ama Lobby'dekiler, zilden sonra gelenlere karşı **büyük bir anlık fark** kazanmaz.
- **Canlı gösterge (zorunlu):** Havuzdaki toplam herkese açık olduğu için geri sayım boyunca
  *"Zil şimdi çalsa herkes X mcap'ten girer"* değeri kesin olarak hesaplanır ve gösterilir. Kullanılan fonksiyon
  settlement ile **aynıdır** (`projectedPrice`), yani gösterilen değer ile gerçekleşen değer aynı havuzda
  birebir eşittir. Test: `live projection`.
- Kullanıcı son fiyatı zil çalana kadar kesin bilemez; havuz büyüdükçe fiyat yükselir. Bunun için v1'de
  $D_{max}$ ve isteğe bağlı $w_{max}$ var; "en fazla şu mcap'e kadar alırım" limiti açık sorudur (§11).

## 7. Ölçek ve işlem maliyeti

- `settle` katılımcı sayısı üzerinden **döngü yapmaz**; sabit sayıda işlemle $N$, $F$, $X$ ve curve durumunu
  hesaplar. Katılımcı sayısı settlement maliyetini etkilemez.
- `deposit`, `claim` ve `refund` her biri **sabit maliyetlidir**; maliyeti işlemi yapan katılımcı öder.
  Toplam maliyet $n$ ile doğrusal büyür ama tek bir işleme yığılmaz.
- $d_{min}$, binlerce toz yatırmayla Lobby'yi şişirme (griefing) maliyetini artırır.

## 8. Invariant'lar ve kabul kriterleri

Her invariant `sim/stampede.test.js` içinde otomatik kontrol edilir (15 test). Fuzz testi 300 rastgele Lobby'de
(1–150 katılımcı, %0–3 ücret, balina senaryoları), solvency testi 100 Lobby × 200 rastgele işlemde çalışır.

| # | Invariant | Kabul kriteri | Test |
|---|---|---|---|
| I1 | Arz sınırı | $X < T_0/2$ ve $X \le S_{curve}$ | fuzz, no refunds |
| I2 | Yeniden çapalama | Settlement sonrası $v = V_0+N$, $t = T_0-X$, $R = N$, $s = X$; boş Lobby'de curve'e dokunulmaz | fuzz, zero-demand |
| I3 | Harcama | $N + F = D$; ücret $f$ bps'i aşmaz (1 birim yuvarlama payı) | fuzz |
| I4 | Token tozu | $\sum q_i \le X$ ve $X - \sum q_i < n$ | fuzz |
| I5 | İade yok | Settle edilmiş, boş olmayan Lobby'de iade $= 0$ | no refunds, fuzz |
| I6 | Korunum | $D = R + F + \text{ödenen iade} + \text{escrow}$; $X = \sum q_i + \text{escrow token tozu}$ | fuzz |
| I7 | Tek fiyat | Kimse Lobby fiyatının altında ödemez; fazla ödeme bir token biriminden az | fuzz |
| I8 | Sıra bağımsızlığı | Yatırma sırası değişince hiçbir tahsis değişmez | order independence |
| I9 | Bölme nötrlüğü | Bir yatırmayı $k$ cüzdana bölmek toplamda $< k$ birim fark yaratır | split neutrality |
| I10 | Durum dışlaması | Cancelled settle edilemez; Settled iptal edilemez; pencereler ayrık | state machine |
| I11 | Fiyat sürekliliği | $0 \le N t_1 - X v_1 < V_0 + 2N$; zilden sonraki ilk alım Lobby fiyatının üstünde | fuzz, continuity |
| I12 | Taneciklik | $d_i \ge d_{min}$ olan herkes $q_i \ge 1$ alır; aksi halde settle reddedilir | fuzz, granularity guard |
| I13 | Boş Lobby | $D < D_{min}$: ücret yok, curve açılmaz (Cancelled), herkese tam iade | zero-demand |
| I14 | Tek çekim | Her adres bir kez çeker; transfer sırasındaki yeniden claim reddedilir | claims |
| I15 | Kilitli fon yok | Tüm claim/refund'lardan ve sweep'ten sonra escrow $= 0$ | state machine, claims |
| I16 | Ödeme gücü | Fazlalık $\ge 0$ ve hiçbir işlemde azalmaz; $v t \ge k$; herkes her şeyi satabilir | solvency, CurveState |
| I17 | Canlı gösterge | Gösterilen fiyat/token, aynı havuzda settlement sonucuna birebir eşit | live projection |
| I18 | Sınırlar | $d_{min}$, $D_{max}$, $w_{max}$ uygulanır; $S_{curve} < T_0/2$ reddedilir | caps, parameters |

**Mutasyon kontrolü:** Modelde kasıtlı olarak 7 hata açıldı: Lobby'nin düz curve'den alması (v0.1/A modeli),
$X$'in yukarı yuvarlanması, curve'ün yeniden çapalanmaması, satışta kullanıcı lehine yuvarlama, çift çekim
korumasının kaldırılması, erken iptal ve cüzdan sınırının kaldırılması. Testler 7'sinin hepsini yakaladı.

## 9. Simülasyon sonuçları

`npm run sim` çıktısı (örnek parametreler).

**9.1 Talep senaryoları** (100 eşit katılımcı, %1 ücret, toplam arz 1 milyar, $T_0$ = 1 milyar)

| Havuz D | Lobby tokenleri (M) | Arzın %'si | Lobby mcap (ücret dahil) | Curve açılış mcap | Ücret | İade | Token tozu (birim) | Satış fazlalığı |
|---|---|---|---|---|---|---|---|---|
| 1 | 31,0 | 3,1% | 32,3 | 32,0 | 0,01 | 0 | 18 | 0,03 |
| 5 | 124,1 | 12,4% | 40,3 | 39,9 | 0,05 | 0 | 39 | 0,61 |
| 20 | 284,5 | 28,4% | 70,3 | 69,6 | 0,20 | 0 | 89 | 5,63 |
| 50 | 383,7 | 38,4% | 130,3 | 129,0 | 0,50 | 0 | 58 | 18,99 |
| 100 | 434,2 | 43,4% | 230,3 | 228,0 | 1,00 | 0 | 89 | 42,99 |
| 186 | 462,3 | 46,2% | 402,3 | 398,3 | 1,86 | 0 | 11 | 85,13 |
| 500 | 485,3 | 48,5% | 1.030,3 | 1.020,0 | 5,00 | 0 | 58 | 240,22 |
| 1.000 | 492,5 | 49,3% | 2.030,3 | 2.010,0 | 10,00 | 0 | 35 | 487,61 |
| 10.000 | 499,2 | 49,9% | 20.030,3 | 19.830,0 | 100,00 | 0 | 57 | 4.942,51 |

Mcap'ler native cinsindendir. Lobby mcap'i ile açılış mcap'i arasındaki fark yalnızca %1 ücrettir.
"Satış fazlalığı", tüm tokenler geri satılsa bile curve'de kalacak native'dir ($N^2/(V_0+2N)$; CURVE_SPEC §5.6).

**9.2 Reddedilen alternatif (v0.1 / A modeli): havuz düz curve'den alır**

| Havuz D | A: havuzun mcap'i | A: curve'ün açıldığı mcap | A: anlık fark | A: iade | **v0.2: herkesin ve curve'ün mcap'i** |
|---|---|---|---|---|---|
| 5 | 35,0 | 40,7 | 1,2x | 0 | 39,9 |
| 20 | 49,8 | 82,7 | 1,7x | 0 | 69,6 |
| 50 | 79,5 | 210,7 | 2,6x | 0 | 129,0 |
| 100 | 129,0 | 554,7 | 4,3x | 0 | 228,0 |
| 186 | 150,0 | anında breakout | — | 64,14 | 398,3 |

A modelinde zil çaldığı an Lobby'deki herkes büyük bir kâğıt kârla başlar ve zilden sonra gelenlere satar;
sniper sorunu çözülmez, Lobby'deki herkes sniper olur. Talep curve'ü aşınca iade de gerekir.
v0.2'de bu fark yoktur ve iade hiç gerekmez.

**9.3 Balina ve küçük katılımcılar** (20 native havuz): balina (10 native) ve 99 küçük katılımcı
(her biri ≈ 0,10) 1M token başına **aynı fiyatı (0,0703)** öder.

**9.4 Cüzdan bölme:** 100 native'i 1 / 10 / 100 / 1.000 cüzdana bölmek toplam tahsisi bu örnekte **hiç
değiştirmez** (322.265.625.000.000 birim). Genel sınır: fark $< k$ birim (I9).

**9.5 Zilden hemen sonra** (50 native havuz): Lobby 1M token başına 0,1290 öder. Zilden sonraki ilk alıcı 1M
token için 0,1292 (+%0,2), 10M için 0,1311 (+%1,6), 50M için 0,1404 (+%8,8) öder. Lobby fiyatının altına
inmek mümkün değildir, ama zilden sonra gelenler de Lobby'ye göre büyük bir dezavantajla başlamaz.

## 10. Saldırı senaryoları

| Senaryo | Sonuç | Savunma |
|---|---|---|
| Lobby içinde ilk olmaya çalışmak | Avantaj yok; tahsis sıradan bağımsız | Tek fiyat + orantılı tahsis (I7, I8) |
| Zilden sonra ilk bloğa yüklenmek | Ucuz pencere yok; ilk alım Lobby fiyatının üstünde başlar | Fiyat sürekliliği (I11) |
| Zilde ucuz alıp hemen satmak (A modelindeki flip) | Büyük anlık fark yok; yalnızca normal curve hareketi | Yeniden çapalanmış curve |
| Cüzdanı bölüp fazla tahsis almak | Avantaj yok | Orantılı tahsis (I9) |
| Son saniyede büyük yatırma (balina) | Herkesin fiyatı yükselir; balina da aynı fiyatı öder, kâr edemez | Canlı gösterge, $D_{max}$, $w_{max}$; v2'de limit fiyat (§11) |
| Sahte talep gösterip son anda çekmek | Mümkün değil | Open süresince çekme yok |
| Settle'ı çağırmamak (griefing) | Fonlar kilitlenmez | İzinsiz settle; $t_{deadline}$ sonrası izinsiz cancel |
| Settle ile iptali yarıştırmak | Mümkün değil | Ayrık pencereler, tek atomik nihai durum (I10) |
| Çift çekim / yeniden giriş | Reddedilir | Tek çekim işareti, CEI, reentrancy guard (I14) |
| Toplu satışla curve'ü boşaltmak | Herkes satabilir, curve ödeme gücünü korur | Solvency ispatı ve fazlalık (I16) |
| Yuvarlamadan kazanç | Mümkün değil | Tüm yuvarlamalar protokol/escrow lehine (I4, I16) |
| Hatalı parametrelerle sıfır tahsis | Settle reddedilir, iptal + tam iade | Taneciklik koruması (I12), $S_{curve} \ge T_0/2$ (I18) |

## 11. Açık sorular ve deneyler

- **Limit fiyat (v2 adayı):** Kullanıcı "en fazla şu mcap'e kadar alırım" der; zilde fiyat bunu aşarsa o yatırma
  dışarıda kalır ve iade edilir. Fiyat havuzla birlikte arttığı için bu bir sabit nokta hesabıdır (limitlere göre
  sıralama gerekir). Zincir üstünde döngüsüz yapılabilmesi için önerilen yol: sıralamayı zincir dışında yapıp
  sonucu zincirde doğrulamak. Ayrı inceleme gerekir.
- **$D_{max}$ ve $w_{max}$ değerleri.** GIWA seçilirse Verified Address ile **kişi başı** tavan mümkün olur.
- **Pencere süreleri:** 15/30/60 sn Lobby, settle penceresi (örnekte 300 sn).
- **$d_{min}$ ve $D_{min}$** değerleri.
- **Breakout tanımı:** ✅ [`CURVE_SPEC.md`](CURVE_SPEC.md) v0.1'de tanımlandı: curve'den satılan token $S_{curve}$'a
  ulaşınca (SOL hedefine göre değil); satış fazlalığı DEX'e LP olarak taşınır. ✅ $T_0 = 1$ milyar kabul edildi;
  bu belgedeki tablolar buna göre güncellendi.
- **Keeper teşviki:** Settle'ı çağırana küçük bir ödül verilmeli mi?
- **Creator'ın Lobby'ye katılımı:** Diğerleriyle aynı fiyattan katılabilir; Bonded tahsisatından ayrı tutulmalı.
- **Deney (çekirdeğe dahil değil):** Curve açılışında zamanla azalan anti-snipe ücreti. v0.2'de zilden sonra ucuz
  pencere kalmadığı için ihtiyaç büyük ölçüde azaldı; yine de ayrı simülasyonla değerlendirilebilir.
