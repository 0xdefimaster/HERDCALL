# Curve ve Breakout — Teknik Şartname (v0.1, taslak)

> Stampede'den sonra curve üzerindeki alım-satımın, ücretlerin ve Breakout'un (DEX'e geçiş) matematiksel tanımı.
> Bu belge **kontrat kodu değildir**; kontrat yazılmadan önce kabul edilmesi gereken davranışı tanımlar.
> Referans model ve testler: [`sim/curve.js`](../sim/curve.js), [`sim/curve.test.js`](../sim/curve.test.js).
> Çalıştırmak için: `npm run test:sim` (testler), `npm run sim:curve` (senaryo tabloları).
> Önceki aşama: [`STAMPEDE_SPEC.md`](STAMPEDE_SPEC.md).

**Özet kararlar (v0.1):**
- **Breakout, satılan token miktarıyla tetiklenir**, SOL hedefiyle değil: curve'den $S_{curve}$ kadar token satıldığında.
  SOL hedefi, büyük bir Lobby'de zil çalar çalmaz breakout'a yol açardı (§5.2).
- **DEX, curve'ün son fiyatından açılır; asla altından açılmaz.** Bunun için parametre kuralı
  $T_0 \le S_{curve} + S_{LP}$; LP'ye gerekmeyen token yakılır (§5.5).
- **Ücret rezerve girmez.** Curve'ün ödeme gücü (her holder'ın her an satabilmesi) ücretlerden etkilenmez (§3.5).
- **Yönetici yok.** Kimse işlemleri durduramaz, rezervi çekemez; breakout'u herkes çağırabilir (§5.4).

---

## 0. Kapsam

- **Kapsamda:** Curve'de alım (iki biçim) ve satım, ücretin hesabı ve dağıtımı, kayma (slippage) koruması,
  breakout tetikleyicisi, DEX'e geçiş, arz muhasebesi.
- **Kapsam dışı:** Lobby ve Stampede ([`STAMPEDE_SPEC.md`](STAMPEDE_SPEC.md)); creator escrow'unun serbest bırakılması ve
  trample (Bonded Creator şartnamesi); DEX seçimi ve DEX ücretinin LP'ye geri eklenmesi; MEV koruması (ayrı şartname);
  Rekt-back havuzunun dağıtımı.
- Tutarlar Stampede şartnamesindeki gibi native'in ve token'ın en küçük biriminde tam sayıdır.

## 1. Parametreler

| Sembol | Anlamı | Örnek değer (simülasyon) |
|---|---|---|
| $V_0$ | Başlangıç sanal native rezervi | 30 native |
| $T_0$ | Başlangıç sanal token rezervi; **şart: $T_0 \le S_{curve} + S_{LP}$** ve $S_{curve} \ge T_0/2$ | 1.000.000.000 |
| $S_{total}$ | Toplam arz $= S_{curve} + S_{LP}$ | 1.000.000.000 |
| $S_{curve}$ | Curve üzerinden satılabilecek arz (Lobby dahil) | 800.000.000 |
| $S_{LP}$ | Breakout'ta DEX likiditesi için ayrılan arz | 200.000.000 |
| $f$ | İşlem ücreti (bps) | 100 (= %1) |
| $m$ | Breakout'u çağırana ödenen geçiş ücreti (havuz kurulum masrafları) | 0,02 native |

**Stampede şartnamesiyle ilişki:** Stampede örnekleri $T_0 = 1{,}073$ milyar ile hesaplandı (pump.fun benzeri). Bu
şartname $T_0 = S_{total} = 1$ milyar önerir (§5.5). Stampede formülleri değişmez; yalnızca o belgedeki tablolar yeni
$T_0$ ile güncellenmelidir (§10).

## 2. Curve durumu ve değişmezler

Durum Stampede şartnamesindeki gibidir: sanal rezervler $v, t$, sabit $k$, gerçek rezerv $R$ ve satılmış token $s$.
Stampede sonunda $v = V_0 + N$, $t = T_0 - X$, $R = N$, $s = X$, $k = v \cdot t$ (boş Lobby'de $N = X = 0$).

| # | Değişmez | Neden geçerli |
|---|---|---|
| C1 | $R = v - V_0$ | Başlangıçta doğru; her alım ve satım $v$ ve $R$'yi aynı miktarda değiştirir |
| C2 | $t + s = T_0$ | Başlangıçta doğru; her işlem $t$ ve $s$'yi aynı miktarda ters yönde değiştirir |
| C3 | $v \cdot t \ge k$ | Yuvarlama her zaman curve lehine (§3.4) |
| C4 | Ödeme gücü: $R \ge$ herkesin hepsini satması hâlinde ödenecek tutar | Stampede şartnamesi §4.6; ücret rezerve girmediği için korunur |
| C5 | $s \le S_{curve}$ | Sınırı aşan alım kırpılır (§5.1) |

## 3. İşlemler

**3.1 Tam token alımı (`buyExactOut`)**: Kullanıcı $\Delta t$ token ve ödemeye razı olduğu en fazla tutarı
($G_{max}$) verir. $\Delta t$, curve'de kalan miktarla kırpılır ($\Delta t \le S_{curve} - s$).

$$c = \left\lceil \frac{k}{t - \Delta t} \right\rceil - v, \qquad \phi = \left\lceil \frac{c \cdot f}{B} \right\rceil, \qquad G = c + \phi \le G_{max}$$

**3.2 Tam tutar alımı (`buyExactIn`)**: Kullanıcı $G$ native ve almak istediği en az token miktarını verir.
$n_0 = \lfloor G \cdot B / (B + f) \rfloor$, $\Delta t = t - \lceil k / (v + n_0) \rceil$, kırpma, ardından $c + \phi \le G$
olana kadar $\Delta t$ bir birim azaltılır. Kullanılmayan tutar ($G - c - \phi$) iade edilir. Sınırda kırpılan bir
alımın fazlası da bu iadeyle döner.

**3.3 Satım (`sell`)**: Kullanıcı $\Delta t$ token ve kabul ettiği en az tutarı verir.

$$o = v - \left\lceil \frac{k}{t + \Delta t} \right\rceil, \qquad \phi = \left\lceil \frac{o \cdot f}{B} \right\rceil, \qquad \text{alınan} = o - \phi$$

**3.4 Yuvarlama:** Curve'e ödenen tutar yukarı, curve'den çıkan tutar aşağı yuvarlanır. Ücret yukarı yuvarlanır.
Aynı miktarı alıp hemen satan biri her zaman ödediğinden az alır (Stampede şartnamesi testleri).

**3.5 Ücret rezerve girmez.** $c$ curve rezervine, $\phi$ ücret yönlendiricisine gider. C1–C4 ücretlerden bağımsızdır.

**3.6 Kayma koruması zorunludur.** Her işlem $G_{max}$ ya da en az alınacak miktarı taşır; aşılırsa işlem geri döner.
Kullanıcının kaybı bu toleransla sınırlıdır; sandwich yapan saldırganın kârı ise tolerans ve işlem büyüklüğüyle
büyür (§8.4).

## 4. Ücret dağıtımı

Ücret ($\phi$, işlem tutarının %1'i) HERDCALL.md §5'teki oranlarla bölünür:

| | Creator | Arayüz | Rekt-back havuzu | Protokol |
|---|---|---|---|---|
| Bonded (ücretin) | %30 (yarısı escrow'a) | %10 | %15 | %45 |
| Unbonded (ücretin) | %10 | %10 | %35 | %45 |

- Her pay aşağı yuvarlanır; **protokol kalanı alır**, böylece paylar her zaman ücrete eşittir.
- İşlem bir arayüz (terminal, bot, referans) olmadan geldiyse arayüz payı protokole gider.
- Bonded token'da creator payının yarısı escrow'a gider; ne zaman serbest kalacağı Bonded Creator şartnamesindedir.
- Stampede ücreti ($F$) de aynı tabloyla bölünür; arayüz payı, yatırmanın yapıldığı arayüze gider.

## 5. Breakout

**5.1 Tetikleyici.** Curve'den satılan token $s$, $S_{curve}$'a ulaştığında. Sınırı geçecek alım kalan miktarla
kırpılır; fazla ödeme iade edilir. Durum **BreakingOut** olur: o andan sonra curve'de alım ve satım yapılmaz.

**5.2 Neden SOL hedefi değil?** Model B'de Lobby'nin net tutarı ($N$) doğrudan rezerve girer. 85 SOL gibi bir hedef
olsaydı, 100 SOL'lük bir Lobby zil çalar çalmaz breakout'a yol açardı (Stampede şartnamesi §9.2'deki "instant
breakout"). Token hedefinde bu olmaz: Lobby en fazla $T_0/2$ token alabilir ve $S_{curve} \ge T_0/2$ olduğu için curve'de
her zaman satılacak token kalır.

**5.3 Breakout'ta rezerv.** C1 ve C2'den, breakout anında $t = T_0 - S_{curve}$ ve

$$R^* = \frac{k}{T_0 - S_{curve}} - V_0 \quad (\text{yuvarlama nedeniyle } \ge)$$

$R^*$ yalnızca Stampede'nin belirlediği $k$'ye bağlıdır; aradaki alım-satım yolundan bağımsızdır (test:
`breakout reserve depends only on the Stampede`). Model B'de $k = T_0 (V_0 + N)^2 / (V_0 + 2N)$ olduğu için **büyük bir
Lobby, breakout için gereken SOL'ü artırır**: Lobby yokken 120 native, 186 native'lik Lobby'den sonra 546 native (§8.1).
Bu bilinçli bir sonuçtur: Lobby fiyatı yükseldikçe curve'ün tamamı ölçeklenir.

**5.4 Geçiş (`breakout`).** BreakingOut durumunda **herkes** çağırabilir; tek seferliktir.

1. Çağırana $m$ ödenir (havuz kurulum kirası ve işlem ücretleri); $m < R^*$ değilse ödenmez.
2. LP native'i: $L_{SOL} = R^* - m$.
3. Curve'ün son fiyatı: $p^* = v / t$.
4. LP tokeni: $L_{tok} = \min\left(S_{LP},\ \lfloor L_{SOL} \cdot t / v \rfloor\right)$.
5. Kalan $S_{LP} - L_{tok}$ token **yakılır**.
6. DEX havuzu $L_{SOL}$ ve $L_{tok}$ ile kurulur; LP payı kalıcı olarak kilitlenir ya da yakılır (DEX'e göre, §10).
7. Durum **Migrated** olur. Curve bir daha açılmaz.

Mint ve freeze yetkileri breakout'ta değil, **token oluşturulurken** kaldırılır; arz baştan sabittir (§6).

**5.5 Fiyat sürekliliği.** $L_{SOL} \cdot t / v = (v - V_0 - m)\, t / v < t = T_0 - S_{curve}$. Dolayısıyla
$T_0 \le S_{curve} + S_{LP}$ ise $L_{tok}$ hiçbir zaman $S_{LP}$ ile kırpılmaz ve DEX fiyatı

$$p^* \le \frac{L_{SOL}}{L_{tok}} < p^* \cdot \left(1 + \frac{1}{L_{tok}}\right)$$

olur: curve'ün son fiyatına bir token biriminin yuvarlaması kadar eşittir. Kural sağlanmazsa (ör. $T_0 = 1{,}073$ milyar)
LP'ye yetecek token olmayabilir ve DEX **daha yüksek** fiyattan açılır: 186 native'lik Lobby'de %27 (§8.2). İki durumda da
DEX hiçbir zaman curve'ün altından açılmaz; holder breakout'ta değer kaybetmez. v0.1 önerisi $T_0 = S_{total}$:
fiyat sıçraması yok, fazla token yakılır.

**5.6 Breakout'a hiç ulaşmayan token.** Curve süresiz açık kalır; herkes her an satabilir (C4). Stampede'nin tek
fiyatı, rezervde $N^2/(V_0 + 2N)$ kadar bir fazlalık bırakır (herkes satsa bile curve'de kalır; 80 native'lik
Lobby'de 20 native'in üzerinde). Bu fazlalık breakout'ta LP'ye gider; breakout olmazsa curve'de kalır (§10).

## 6. Arz

- $S_{total} = S_{curve} + S_{LP}$, token oluşturulurken bir kez basılır; mint ve freeze yetkisi o anda kaldırılır.
- **Creator için ayrıca token basılmaz.** Creator tahsisatı, creator'ın Lobby'de herkesle aynı fiyattan aldığı ve
  vesting kasasına kilitlediği tokenlerdir (öneri; Bonded Creator şartnamesinde kesinleşecek).
- Korunum: breakout öncesi $s + (S_{curve} - s) + S_{LP} = S_{total}$; sonrası $s + L_{tok} + \text{yakılan} = S_{total}$.

## 7. Değişmezler ve testler

| # | Değişmez | Test |
|---|---|---|
| C1–C5 | §2'deki durum değişmezleri, her adımda | `random trading until breakout keeps every invariant` (120 piyasa, 400'e kadar adım) |
| C6 | Ücret payları ücrete eşit; escrow bonded creator payının yarısı; arayüz yoksa pay protokole | `fees` |
| C7 | Native korunumu: giren − çıkan = rezerv + ücretler | rastgele işlem testi |
| C8 | Holder'lardaki token = satılmış token | rastgele işlem testi |
| C9 | Sınırı geçen alım kırpılır, fazlası iade edilir; sonra işlem yok | `the buy that crosses the curve supply is clamped` |
| C10 | Kayma sınırları iki yönde de uygulanır | `slippage limits` |
| C11 | Breakout herkese açık ve tek seferlik; rezervin tamamı taşınır; DEX fiyatı $\ge p^*$ ve en fazla bir birim üstünde; arz korunur | `breakout: permissionless, once ...` (60 piyasa) |
| C12 | Breakout rezervi işlem yolundan bağımsız | `breakout reserve depends only on the Stampede` |
| C13 | $T_0 > S_{curve} + S_{LP}$ reddedilir; izin verilirse DEX yukarıdan açılır | `parameters` |
| C14 | Ölü token: herkes hepsini geri satabilir; Stampede fazlalığı azalmaz | `a dead token` |

Mutasyon kontrolü: 8 kasıtlı hatanın 8'i yakalandı. Bozulan kurallar: kırpma, LP sınırı, geçiş ücreti, ücret kalanı,
breakout eşiği, kayma kontrolü, ücret yuvarlaması, $T_0$ kuralı. Testler kodun yazılan kuralları uyguladığını
gösterir; zincir üstü davranışın kanıtı değildir.

## 8. Simülasyon sonuçları

Tablolar `npm run sim:curve` çıktısıdır. Örnek parametreler §1'deki gibidir.

**8.1 Zilden breakout'a**

| Lobby havuzu | Curve açılış mcap | Breakout'a kadar gereken native | Breakout rezervi | Breakout mcap | Açılış → breakout | LP native | LP token (M) | Yakılan (M) |
|---|---|---|---|---|---|---|---|---|
| 0 | 30,0 | 120,0 | 120,0 | 750 | 25,0x | 120,0 | 160,0 | 40,0 |
| 5 | 39,9 | 118,1 | 123,1 | 765 | 19,2x | 123,1 | 160,8 | 39,2 |
| 20 | 69,6 | 128,4 | 148,2 | 891 | 12,8x | 148,1 | 166,3 | 33,7 |
| 50 | 129,0 | 165,5 | 215,0 | 1.225 | 9,5x | 215,0 | 175,5 | 24,5 |
| 100 | 228,0 | 235,9 | 334,9 | 1.825 | 8,0x | 334,9 | 183,5 | 16,5 |
| 186 | 398,3 | 361,5 | 545,7 | 2.878 | 7,2x | 545,7 | 189,6 | 10,4 |
| 500 | 1.020,0 | 826,1 | 1.321,1 | 6.756 | 6,6x | 1.321,1 | 195,6 | 4,4 |

- Büyük Lobby: daha yüksek açılış, daha yüksek breakout, ama açılıştan breakout'a daha küçük kat (25x → 6,6x).
- Yakılan token Lobby büyüdükçe azalır; her durumda DEX fiyatı curve'ün son fiyatına eşittir.

**8.2 LP boyutu: $T_0 = S_{curve} + S_{LP}$ ile pump.fun benzeri $T_0$**

| Lobby havuzu | $T_0$ = 1 milyar: yakılan (M) | DEX / curve fiyatı | $T_0$ = 1,073 milyar: yakılan (M) | DEX / curve fiyatı |
|---|---|---|---|---|
| 0 | 40,0 | eşit | 0 | +%1,75 |
| 20 | 33,7 | eşit | 0 | +%7,24 |
| 50 | 24,5 | eşit | 0 | +%15,22 |
| 186 | 10,4 | eşit | 0 | +%27,44 |
| 500 | 4,4 | eşit | 0 | +%32,64 |

Pump.fun'ın parametreleri Lobby'siz bir curve için ayarlıdır. Model B'de Lobby $k$'yi büyüttüğü için o parametrelerle
LP tokeni yetmez ve DEX fiyatı sıçrar. Bu holder'a zarar vermez ama breakout anında öngörülemeyen bir fiyat kırılması
yaratır; v0.1 bunun yerine yakmayı seçer.

**8.3 %1'in dağılımı (100 native curve hacmi başına)**

| Durum | Creator (hemen) | Creator escrow | Arayüz | Rekt-back havuzu | Protokol |
|---|---|---|---|---|---|
| Bonded, arayüz üzerinden | 0,150 | 0,150 | 0,100 | 0,150 | 0,450 |
| Bonded, doğrudan | 0,150 | 0,150 | 0 | 0,150 | 0,550 |
| Unbonded, arayüz üzerinden | 0,100 | 0 | 0,100 | 0,350 | 0,450 |

**8.4 Curve'de sandwich** (20 native Lobby sonrası; saldırgan iki yönde de %1 ücret öder)

| Kurbanın alımı | Kayma toleransı | Öne geçen alım | Saldırgan kârı | Kurbanın alımına oranı |
|---|---|---|---|---|
| 1 | %0,5 | 0,13 | 0,0025 | %0,25 |
| 1 | %1 | 0,26 | 0,0049 | %0,49 |
| 1 | %5 | 1,32 | 0,0238 | %2,38 |
| 5 | %1 | 0,26 | 0,0485 | %0,97 |
| 5 | %5 | 1,37 | 0,2409 | %4,82 |
| 20 | %1 | 0,30 | 0,2669 | %1,33 |
| 20 | %5 | 1,52 | 1,3128 | %6,56 |

- **%1 ücret sandwich'i engellemez.** Saldırganın kârı kayma toleransıyla neredeyse doğrusal büyür.
- Kurbanın kaybı toleransla sınırlıdır; saldırganın kârı, kurbanın kendi fiyat etkisinden de pay aldığı için
  toleransı aşabilir.
- Öneri (arayüz): varsayılan kayma toleransı küçük tutulur (%0,5–1), işlemin beklenen fiyat etkisi ve olası kayıp
  işlemden önce gösterilir. Zincir tarafı koruması MEV şartnamesinde.

**8.5 Lobby fiyatından breakout'a**

| Lobby havuzu | Lobby tokenleri (M) | Lobby fiyatı (mcap) | Breakout mcap | Lobby fiyatı → breakout |
|---|---|---|---|---|
| 5 | 124,1 | 40,3 | 765 | 19,0x |
| 20 | 284,5 | 70,3 | 891 | 12,7x |
| 50 | 383,7 | 130,3 | 1.225 | 9,4x |
| 100 | 434,2 | 230,3 | 1.825 | 7,9x |
| 186 | 462,3 | 402,3 | 2.878 | 7,2x |
| 500 | 485,3 | 1.030,3 | 6.756 | 6,6x |

## 9. Saldırılar ve riskler

| Senaryo | Sonuç |
|---|---|
| Breakout anında DEX'te snipe | DEX curve'ün son fiyatından açılır; fiyat boşluğu yoktur. Kazanç yalnızca gerçek talepten gelir. |
| Breakout'u zorlamak için son alım | Son alıcı kalan tokeni curve fiyatından öder; ek bir avantajı yoktur. |
| Breakout'u geciktirmek | Herkes çağırabilir; çağırana $m$ ödenir. Beklerken holder'lar satamaz, bu yüzden keeper teşviki önemli (§10). |
| Sandwich | §8.4. Zararı kullanıcının kayma toleransı sınırlar; tam koruma MEV şartnamesinde. |
| Kendini arayüz göstermek | Bir trader işlemi kendi arayüz hesabıyla gönderip %0,10'u geri alabilir: etkin ücret %0,9. Bilinen ve kabul edilen durum; Rekt Mining puan tabanı yalnızca protokol payı olduğu için oradaki hesabı değiştirmez. |
| Yuvarlama istismarı | Tüm yuvarlamalar curve ve ücret lehine; al-sat her zaman zararla kapanır. |
| Yönetici rug'ı | Yönetici rolü yok: işlem durdurma, rezerv çekme, ücret değiştirme yetkisi yok. Program yükseltme yetkisi ayrı bir karardır (§10). |

## 10. Açık sorular

- **DEX seçimi** (Raydium CPMM, Meteora DAMM v2 vb.), LP payının kilitlenmesi mi yakılması mı, DEX ücretinin bir kısmının
  LP'ye geri eklenmesi (HERDCALL.md §3, öneri %25).
- **Geçiş ücreti $m$**: gerçek havuz kurulum maliyetine göre.
- **Breakout'a ulaşmayan token'daki Stampede fazlalığı** (§5.6): curve'de mi kalsın, belirli bir süre işlem görmeyen
  tokenlerde holder'lara mı dağıtılsın? Dağıtım, ölü token tanımı ve Diamond Hooves ile birlikte düşünülmeli.
- **$T_0$ uyumu:** Stampede şartnamesinin tabloları $T_0 = 1{,}073$ milyar ile; bu şartname 1 milyar öneriyor. Karar
  verilince Stampede tabloları güncellenecek.
- **Program yükseltme yetkisi:** Kontrat değiştirilemez mi olacak, zaman kilitli bir çoklu imza mı?
- **Kayma varsayılanları ve MEV koruması** (ayrı şartname).
