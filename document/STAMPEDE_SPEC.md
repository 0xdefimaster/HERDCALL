# Stampede — Teknik Şartname (v0.1, taslak)

> Lobby → Stampede açılışının matematiksel ve durum makinesi tanımı.
> Bu belge **kontrat kodu değildir**; kontrat yazılmadan önce kabul edilmesi gereken davranışı tanımlar.
> Referans model ve otomatik testler: [`sim/stampede.js`](../sim/stampede.js), [`sim/stampede.test.js`](../sim/stampede.test.js).
> Çalıştırmak için: `npm run test:sim` (testler), `npm run sim` (senaryo tabloları).

---

## 0. Kapsam

- **Kapsamda:** Lobby'nin açılması, yatırma, kapanış, tek fiyatlı settlement, tahsis ve iade, iptal,
  claim/refund, yuvarlama tozları, curve'ün başlangıç durumu.
- **Kapsam dışı:** Curve açıldıktan sonraki işlemler, breakout, Bonded Creator kasası, Rekt-back.
  Bunlar ayrı şartnamelerde tanımlanacak.
- **Zincirden bağımsız:** Tutarlar zincirin native varlığında (SOL ya da ETH) ve en küçük biriminde
  (lamport / wei) tam sayı olarak ifade edilir. Token tutarları da en küçük biriminde tam sayıdır.
  Belgede "native" kelimesi SOL veya ETH yerine kullanılır.

## 1. Terimler ve parametreler

| Sembol | Anlamı | Örnek değer (simülasyon) |
|---|---|---|
| $V_0$ | Curve'ün sanal native rezervi | 30 native |
| $T_0$ | Curve'ün sanal token rezervi | 1.073.000.000 token |
| $S_{curve}$ | Curve üzerinden satılabilecek toplam arz | 800.000.000 token |
| $S_L$ | Lobby'ye ayrılan **azami** token miktarı ($0 < S_L \le S_{curve}$) | 150.000.000 token |
| $f$ | İşlem ücreti, baz puan (bps) | 100 (= %1) |
| $B$ | Baz puan paydası | 10.000 |
| $d_{min}$ | Tek bir yatırma işleminin alt sınırı | 0,01 native |
| $D_{min}$ | Settlement için toplam talep alt sınırı | 0,1 native |
| $D_{max}$ | Lobby'nin toplam talep üst sınırı | 10.000 native |
| $t_{close}$ | Lobby'nin kapanış zamanı | açılış + 15/30/60 sn |
| $t_{deadline}$ | Settlement için son an ($t_{close} < t_{deadline}$) | $t_{close}$ + 300 sn |
| $d_i$ | $i$ adresinin toplam yatırdığı native (aynı adresin yatırmaları toplanır) | — |
| $D$ | Toplam yatırılan native, $D = \sum_i d_i$ | — |
| $n$ | Yatırma yapmış farklı adres sayısı | — |

Token 6, native 9 ondalık basamakla modellenmiştir.

## 2. Bonding curve

Sanal rezervli sabit çarpım eğrisi kullanılır: $k = V_0 \cdot T_0$.

**Rezerv fonksiyonu.** Curve'den $x$ token satıldığında curve'ün tutması gereken **net** native miktarı:

$$C(x) = \left\lceil \frac{V_0 \cdot x}{T_0 - x} \right\rceil, \qquad 0 \le x < T_0$$

Yukarı yuvarlama protokol lehinedir: rezerv hiçbir zaman gereğinden az olmaz.

**Ters fonksiyon.** $n$ net native ile alınabilecek en büyük token miktarı:

$$x_{max}(n) = \left\lfloor \frac{T_0 \cdot n}{V_0 + n} \right\rfloor$$

**İspat (maksimallik).** $x = x_{max}(n)$ için $x(V_0+n) \le T_0 n \Rightarrow V_0 x \le n(T_0-x)
\Rightarrow C(x) \le n$. $x+1 > T_0 n / (V_0+n)$ olduğundan $V_0(x+1)/(T_0-x-1) > n$, dolayısıyla
$C(x+1) > n$. Test: `curve: costOf / maxTokensFor are exact inverses at the boundary` (2.000 rastgele bütçe).

**Spot (marjinal) fiyat.** $x$ token satıldıktan sonraki anlık fiyat: $p_{spot}(x) = \dfrac{V_0 T_0}{(T_0-x)^2}$.

**Başlangıç durumu.** Lobby'den önce curve boştur: satılan $= 0$, rezerv $= 0$.
Settlement sonrası curve **satılan $= X$, rezerv $= C(X)$** durumundan açılır (§4).
Curve'ün değişmezi: **rezerv $\ge C(\text{satılan})$**; settlement'ta bu eşitlikle sağlanır.

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
| **Settled** | `settle` başarıyla çalışınca (nihai) | `claim`, `sweepDust` |
| **Cancelled** | `cancel` çalışınca (nihai) | `refund` |

**Yarış koşulunun önlenmesi.**
1. `settle` ve `cancel` için zaman pencereleri **ayrıktır**: settle $[t_{close}, t_{deadline})$, cancel $[t_{deadline}, \infty)$.
   Aynı anda ikisinin de geçerli olduğu bir an yoktur.
2. Her ikisi de nihai durum yokken çalışabilir ve nihai durumu **tek bir atomik adımda** yazar.
   Settled bir Lobby iptal edilemez, Cancelled bir Lobby settle edilemez.
3. İadeler (`refund`) yalnızca **Cancelled** durumunda açılır; önce durum kalıcı olarak Cancelled'a geçer, sonra iade talep edilir.
4. Open süresince **çekme (withdraw) yoktur.** Son saniyede sahte talep gösterip çekme oyununu engeller.
   Fonlar yine de kilitli kalmaz: settle edilmezse $t_{deadline}$ sonrası herkes iptal edip tam iade alabilir.
5. `settle` ve `cancel` **izinsizdir** (herkes çağırabilir). Settle'ı çağıranı ödüllendiren küçük bir
   "keeper" teşviki açık sorudur (§11).

Zaman damgası notu: Blok zamanı birkaç saniye oynayabilir. $t_{deadline} - t_{close}$ bu sapmadan çok büyük
seçilmelidir (örnekte 300 sn). Testler: `state machine: time windows are disjoint and terminal states are final`.

**Settling ara durumu hakkında.** EVM'de settlement tek işlemde atomiktir ve Settling yalnızca zamanla
belirlenen bir aşamadır. Hesap sınırları nedeniyle settlement birden fazla işleme bölünmek zorunda kalırsa
(Solana'da olabilir), Settling **saklanan** bir durum olur ve şu kural eklenir: Settling başladıktan sonra
`cancel` yalnızca Settling yarım kaldıysa ve ayrı, daha uzun bir süre geçtiyse çalışır; yarım kalan adımlar
fonlara dokunmaz, fon hareketleri son adımda tek seferde yapılır.

## 4. Settlement hesabı

`settle()` yalnızca toplamlarla çalışır ve şu değerleri hesaplar:

**4.1 Boş ya da yetersiz talep.** $D < D_{min}$ ise: $X = 0$, hiçbir fon curve'e gitmez, ücret alınmaz,
$E = D$ (tamamı iade). Lobby yine **Settled** olur (iptal değil) ve curve başlangıç durumundan açılır.
Test: `zero and below-minimum demand`.

**4.2 Ücret ve net bütçe.** Ücret yalnızca **harcanan** miktardan alınır; iade edilen native'den ücret alınmaz.
Curve'e gidebilecek azami net miktar:

$$N_{max} = \left\lfloor \frac{D \cdot (B - f)}{B} \right\rfloor$$

**4.3 Satılan miktar.**

$$X = \min\big(S_L,\ x_{max}(N_{max})\big)$$

- **Fazla talep** ($x_{max}(N_{max}) \ge S_L$, yani yaklaşık $D \ge C(S_L)/(1-f)$): $X = S_L$.
- **Eksik talep**: $X = x_{max}(N_{max}) < S_L$ ve $X$ bütçeyle alınabilecek en büyük miktardır: $C(X+1) > N_{max}$.

**4.4 Fon dağılımı.**

| Değer | Formül | Gider |
|---|---|---|
| $U$ | $C(X)$ | Curve rezervi (net) |
| $G$ | $\lceil U \cdot B / (B - f) \rceil$ | Harcanan brüt native |
| $F$ | $G - U$ | Ücret → ücret yönlendiricisi (HERDCALL.md §5 dağılımı) |
| $E$ | $D - G$ | İade havuzu (katılımcılara orantılı) |

**İspat ($G \le D$).** $U \le N_{max} \le D(B-f)/B \Rightarrow U \cdot B/(B-f) \le D$; $D$ tam sayı olduğundan
$\lceil U B/(B-f) \rceil \le D$. Ücretler hiçbir zaman curve rezervinden düşülmez: rezerv tam olarak $C(X)$'tir.

**4.5 Tahsis tanecikliği koruması.** Geçerli her yatırmanın ($d_i \ge d_{min}$) en az 1 token birimi alması için
settlement şunu şart koşar:

$$X \cdot d_{min} \ge D$$

Sağlanmazsa `settle` geri döner (revert). $t_{deadline}$ geçince Lobby iptal edilir ve herkes tam iade alır.
Makul parametrelerde bu koşul fiilen hep sağlanır: örnek parametrelerde en kötü durumda ($D = D_{max}$,
tamamen dolu Lobby) bile 1 lamport ≥ 15 token birimi alır, $d_{min}$ ise $10^7$ lamport'tur.
Koruma, hatalı parametrelere karşı bir emniyettir. Test: `granularity guard`.

**4.6 Atomik fon hareketleri.** Tek adımda: $U$ curve rezervine, $F$ ücret yönlendiricisine, $X$ token
Lobby escrow'una (claim'ler için); $E$ escrow'da kalır. Curve aynı işlemde **satılan $= X$, rezerv $= U$**
durumundan işleme açılır.

## 5. Claim ve refund

**5.1 Kişi başı hesap (pull modeli).** Settled durumunda her adres kendi payını kendisi talep eder:

$$q_i = \left\lfloor \frac{d_i \cdot X}{D} \right\rfloor \text{ token}, \qquad r_i = \left\lfloor \frac{d_i \cdot E}{D} \right\rfloor \text{ native iade}$$

İki değer de yalnızca $d_i$'ye ve toplamlara bağlıdır: **yatırma sırası sonucu değiştirmez.**
Cancelled durumunda `refund` adresin tüm yatırdığı tutarı ($d_i$) iade eder.

**5.2 Çift çekim ve yeniden giriş.**
- Her adres **tek bir kez** çeker: `claim` ya da `refund` ilk çağrıda adresi "çekti" olarak işaretler.
  Durumlar birbirini dışladığı için aynı fon hem iade hem tahsis olarak çekilemez.
- **Önce etki, sonra transfer** (checks-effects-interactions): işaretleme ve bakiye düşümü transferden önce yapılır.
  Ek olarak kontratta yeniden giriş kilidi (reentrancy guard) kullanılır.
- Test: `claims: no double withdrawal, re-entrant claim fails` (transfer sırasında yeniden claim denenir ve reddedilir).

**5.3 Yuvarlama tozları.** Aşağı yuvarlama nedeniyle:
- Token tozu: $X - \sum q_i < n$ birim
- Native tozu: $E - \sum r_i < n$ birim

Tozlar escrow'da kalır. **Tüm katılımcılar çektikten sonra** `sweepDust` ile süpürülür:
token tozu yakılır, native tozu Rekt-back havuzuna gider (açık soru §11). Tozlar curve rezervine dokunmaz,
böylece rezerv $= C(X)$ eşitliği bozulmaz. Katılımcıların hakkı olan hiçbir tutar zaman aşımıyla süpürülmez.

**5.4 Kilitli fon olmaması.**
- Settled: claim süresizdir.
- Cancelled: refund süresizdir.
- Settle edilmemiş Lobby: $t_{deadline}$ sonrası herkes `cancel` çağırabilir.
Test: `state machine` (iptal sonrası escrow sıfırlanır), `claims` (sweep sonrası escrow sıfırlanır).

## 6. Fiyatın adı ve anlamı

Stampede'de iki farklı fiyat vardır; arayüz ikisini karıştırmamalıdır:

| Ad | Tanım | Arayüzde |
|---|---|---|
| **Stampede fiyatı** (clearing price) | $G / X$: ilk dilimin **ortalama** maliyeti, ücret dahil | "Everyone in this launch paid X per token (incl. 1% fee)." |
| **Açılış spot fiyatı** | $p_{spot}(X)$: curve açıldığı andaki **marjinal** fiyat | "Curve opens at Y per token." |

Artan bir curve'de her zaman $p_{spot}(X) > U/X$ olur. Yani Stampede'e katılanlar, curve açıldığında ilk alım
yapanlardan daha düşük ortalama fiyat öder; bu tasarım gereğidir ve arayüzde açıkça yazılmalıdır.
Kullanıcıya "tek fiyat" sözü, **Lobby içindeki herkesin aynı fiyatı ödemesi** anlamına gelir.
Her katılımcının ödediği birim fiyat, Stampede fiyatından en fazla bir token birimi ve bir native birimi kadarlık
yuvarlama farkıyla sapar. Test: fuzz testindeki I7 kontrolü.

## 7. Ölçek ve işlem maliyeti

- `settle` katılımcı sayısı üzerinden **döngü yapmaz**; yalnızca $D$, $S_L$ ve curve parametreleriyle sabit
  sayıda işlem yapar. Katılımcı sayısı settlement maliyetini etkilemez.
- `deposit`, `claim` ve `refund` her biri **sabit maliyetlidir** ve maliyeti işlemi yapan katılımcı öder.
  $n$ katılımcılı bir Lobby'de toplam maliyet $n$ ile doğrusal büyür ama tek bir işleme yığılmaz.
- $d_{min}$, binlerce toz yatırmayla Lobby'yi şişirme (griefing) maliyetini artırır.

## 8. Invariant'lar ve kabul kriterleri

Her invariant `sim/stampede.test.js` içinde otomatik olarak kontrol edilir. Fuzz testi 300 rastgele Lobby'de
(1–150 katılımcı, %0–3 ücret, %1–100 Lobby payı, balina senaryoları) bütün invariant'ları sınar.

| # | Invariant | Kabul kriteri | Test |
|---|---|---|---|
| I1 | Arz sınırı | $X \le S_L$ | fuzz, undersubscribed |
| I2 | Curve tutarlılığı | Settlement sonrası rezerv $= C(\text{satılan})$; boş Lobby'de curve'e dokunulmaz | fuzz, zero-demand |
| I3 | Harcama sınırı | $G = U + F \le D$, $E = D - G \ge 0$; ücret $f$ bps'i aşmaz (1 birim yuvarlama payı) | fuzz, boundary |
| I4 | Token tozu | $\sum q_i \le X$ ve $X - \sum q_i < n$ | fuzz |
| I5 | Native tozu | $\sum r_i \le E$ ve $E - \sum r_i < n$ | fuzz |
| I6 | Korunum | $D = U + F + \sum r_i + \text{escrow tozu}$; $X = \sum q_i + \text{escrow token tozu}$ | fuzz |
| I7 | Tek fiyat | Kimse Stampede fiyatının altında ödemez; fazla ödeme yalnızca yuvarlama kadardır | fuzz |
| I8 | Sıra bağımsızlığı | Yatırma sırası değişince hiçbir tahsis değişmez | order independence |
| I9 | Bölme nötrlüğü | Bir yatırmayı $k$ cüzdana bölmek toplamda $< k$ birim fark yaratır (yalnızca yuvarlama) | split neutrality |
| I10 | Durum dışlaması | Cancelled settle edilemez; Settled iptal edilemez; pencereler ayrık | state machine |
| I11 | Maksimallik | Eksik talepte $C(X+1) > N_{max}$ | undersubscribed, curve |
| I12 | Taneciklik | $d_i \ge d_{min}$ olan herkes $q_i \ge 1$ alır; aksi halde settle reddedilir | fuzz, granularity guard |
| I13 | Boş Lobby | $D < D_{min}$: $X = 0$, ücret yok, herkese tam iade | zero-demand |
| I14 | Tek çekim | Her adres bir kez çeker; transfer sırasındaki yeniden claim reddedilir | claims |
| I15 | Kilitli fon yok | Tüm claim/refund'lardan ve sweep'ten sonra escrow $= 0$ | state machine, claims |

Ek doğrulama: Modelde kasıtlı hatalar açıldığında (çift çekim işareti kaldırıldı, $x_{max}$'a +1 eklendi,
erken iptale izin verildi, $S_L$ sınırı kaldırıldı, iade yukarı yuvarlandı, taneciklik koruması kaldırıldı)
testlerin her biri en az bir başarısızlıkla hatayı yakaladı.

## 9. Simülasyon sonuçları

`npm run sim` çıktısı (örnek parametreler).

**9.1 Lobby kapasitesi: farklı $S_L$ dilimleri için $C(S_L)$**

| $S_L$ (curve arzının %'si) | $S_L$ (M token) | $C(S_L)$ net native | Ortalama fiyat / 1M token | Açılış spot / 1M token |
|---|---|---|---|---|
| 10% | 80 | 2,417 | 0,0302 | 0,0326 |
| 20% | 160 | 5,257 | 0,0329 | 0,0386 |
| 30% | 240 | 8,643 | 0,0360 | 0,0464 |
| 50% | 400 | 17,831 | 0,0446 | 0,0711 |
| 100% | 800 | 87,912 | 0,1099 | 0,4319 |

**9.2 Talep senaryoları** ($S_L$ = 150M token, %1 ücret, 100 eşit katılımcı)

| Toplam D | Durum | X (M token) | $S_L$'nin %'si | U → curve | F ücret | İade | İade % | Stampede fiyatı / 1M | Açılış spot / 1M | Toz (token / native birim) |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | boş | 0 | 0% | 0 | 0 | 0 | — | — | 0,0280 | 0 / 0 |
| 0,05 | boş | 0 | 0% | 0 | 0 | 0,050 | 100% | — | 0,0280 | 0 / 0 |
| 1 | eksik talep | 34,28 | 22,9% | 0,990 | 0,010 | 0 | 0% | 0,0292 | 0,0298 | 67 / 0 |
| 3 | eksik talep | 96,66 | 64,4% | 2,970 | 0,030 | 0 | 0% | 0,0310 | 0,0338 | 28 / 0 |
| 4,926 | fazla talep (sınır) | 150 | 100% | 4,875 | 0,049 | 0,001 | 0% | 0,0328 | 0,0378 | 0 / 87 |
| 10 | fazla talep | 150 | 100% | 4,875 | 0,049 | 5,075 | 50,8% | 0,0328 | 0,0378 | 0 / 87 |
| 186,4 | fazla talep | 150 | 100% | 4,875 | 0,049 | 181,475 | 97,4% | 0,0328 | 0,0378 | 0 / 87 |
| 1.000 | fazla talep | 150 | 100% | 4,875 | 0,049 | 995,075 | 99,5% | 0,0328 | 0,0378 | 0 / 87 |

**9.3 Balina ve küçük katılımcılar** (20 native toplam): balina (10 native) ve 99 küçük katılımcı (her biri 0,101)
1M token başına **aynı fiyatı (0,0328)** öder; tahsis ve iade yatırılan tutarla orantılıdır.

**9.4 Cüzdan bölme:** 100 native'i 1 / 10 / 100 / 1.000 cüzdana bölmek toplam tahsisi yalnızca 0 / 7 / 57 / 857
token birimi (1 token = 1.000.000 birim) ve 0 / 7 / 47 / 847 lamport **azaltır**. Bölmek avantaj sağlamaz, sadece
yuvarlama kaybettirir.

### ⚠️ En önemli bulgu: Lobby kapasitesi

Pump.fun benzeri curve parametreleriyle ilk 150M token yalnızca **~4,9 native** eder. Gerçekçi bir talepte
(örneğin 186 native) talebin **%97'si iade edilir.** Karşılanmayan talep curve açıldığı anda ilk bloklara
yönelir; yani sniper rekabeti **Lobby'den curve'ün ilk bloklarına taşınır.** Stampede, açılış adaletini ancak
Lobby talebin büyük kısmını karşılayabildiğinde sağlar.

Karar verilmesi gereken seçenekler (kontrat yazılmadan önce):
1. **$S_L$'yi büyütmek** (örneğin curve arzının %50–100'ü). %100'de Lobby tüm curve'ü tek fiyattan doldurabilir;
   talep $C(S_{curve})$'u aşarsa token doğrudan breakout'a gider ve fiyat keşfi DEX'e kayar.
2. **Curve parametrelerini değiştirmek** (daha büyük $V_0$): aynı arz için daha yüksek kapasite, daha yüksek başlangıç fiyatı.
3. **Dinamik $S_L$**: Lobby talebe göre curve'ün ne kadarını tek fiyattan dolduracağını kendisi belirler;
   üst sınır $S_{curve}$ olur. Bu, seçenek 1'in sınırsız hâlidir.

Hangisinin seçileceği gerçek talep dağılımı verisi olmadan kesinleşmemeli; simülasyon parametreleri buna göre
genişletilecek.

## 10. Saldırı senaryoları

| Senaryo | Sonuç | Savunma |
|---|---|---|
| Sniper ilk bloğa girmeye çalışır (Lobby içinde) | Avantaj yok; tahsis sıradan bağımsız | Tek fiyat + orantılı tahsis (I7, I8) |
| Sniper curve açıldığı ilk bloğa yüklenir | **Hâlâ mümkün**; avantajı Lobby kapasitesine bağlı | §9 kapasite kararı; azalan anti-snipe ücreti ayrı deney (§11) |
| Cüzdanı bölüp daha fazla tahsis almak | Avantaj yok, yuvarlama kaybettirir | Orantılı tahsis (I9) |
| Son saniyede yatırma | Avantaj yok; zaman ağırlığı yok | Sıra bağımsızlığı |
| Sahte talep gösterip son anda çekmek | Mümkün değil | Open süresince çekme yok |
| Settle'ı hiç çağırmamak (griefing) | Fonlar kilitlenmez | İzinsiz settle; $t_{deadline}$ sonrası izinsiz cancel |
| Settle ile iptali yarıştırmak | Mümkün değil | Ayrık zaman pencereleri, tek atomik nihai durum (I10) |
| Çift çekim / yeniden giriş | Reddedilir | Tek çekim işareti, CEI, reentrancy guard (I14) |
| Binlerce toz yatırma | Pahalı, settle'ı etkilemez | $d_{min}$; settle'da döngü yok |
| Yuvarlamadan kazanç | Mümkün değil | Her yuvarlama protokol/escrow lehine; tozlar sınırlı (I4, I5) |
| Hatalı parametrelerle sıfır tahsis | Settle reddedilir, iptal + tam iade | Taneciklik koruması (I12) |
| Blok zamanı oynatma | Pencere kenarlarında birkaç saniye | $t_{deadline} - t_{close}$ çok büyük seçilir |

## 11. Açık sorular ve deneyler

- **Lobby kapasitesi** (§9): $S_L$ büyüklüğü, $V_0$/$T_0$ ya da dinamik $S_L$. **En öncelikli karar.**
- **Pencere süreleri:** 15/30/60 sn Lobby, settle penceresi (örnekte 300 sn).
- **$d_{min}$, $D_{min}$, $D_{max}$** değerleri; $D_{max}$'ın kişi başı mı toplam mı olacağı.
  GIWA seçilirse Verified Address ile **kişi başı** tavan mümkün olur (sybil'e karşı gerçek koruma).
- **Keeper teşviki:** Settle'ı çağırana küçük bir ödül verilmeli mi, verilirse ücretten mi alınmalı?
- **Toz hedefi:** Token tozu yakılsın mı, native tozu Rekt-back'e mi gitsin?
- **Creator'ın Lobby'ye katılımı:** Creator diğerleriyle aynı fiyattan katılabilir; Bonded tahsisatından ayrı tutulmalı.
- **Deney (çekirdeğe dahil değil): Curve açılışında zamanla azalan anti-snipe ücreti.** Yüksek ilk ücret normal
  kullanıcıları da etkiler ve botun ücreti ödemesini engellemez. Ayrı bir simülasyonla, Lobby kapasitesi
  seçenekleriyle karşılaştırılarak değerlendirilecek.
