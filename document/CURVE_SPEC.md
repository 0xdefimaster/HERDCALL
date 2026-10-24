# Curve ve Breakout — Teknik Şartname (v0.1)

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
- **İşlemlerde yönetici yetkisi yok.** Kimse işlemleri durduramaz, rezervi çekemez, ücreti değiştiremez; breakout'u
  herkes çağırabilir (§5.4). Program **yükseltilebilir**, ama yalnızca çoklu imza ve 72 saatlik zaman kilidiyle
  (HERDCALL.md §12, program yükseltme politikası).
- ✅ **Kararlar:** $T_0 = 1$ milyar; DEX **Meteora DAMM v2**; LP **kalıcı kilit** (yakılmaz, §5.4).

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

**Stampede şartnamesiyle ilişki:** $T_0 = S_{total} = 1$ milyar kabul edildi (§5.5). Stampede formülleri aynıdır; o
belgedeki tablolar bu değerle güncellendi.

## 2. Curve durumu ve değişmezler

Durum Stampede şartnamesindeki gibidir: sanal rezervler $v, t$, sabit $k$, gerçek rezerv $R$ ve satılmış token $s$.
Stampede sonunda $v = V_0 + N$, $t = T_0 - X$, $R = N$, $s = X$, $k = v \cdot t$ (boş Lobby'de curve açılmaz, launch iptal olur).

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
| Bonded (ücretin) | %50 (yarısı escrow'a) | %10 | %10 | %30 |
| Unbonded (ücretin) | %30 | %10 | %15 | %45 |

İlke: rakip pump.fun'dır ve HerdCall creator'dan yanadır. Her creator en az pump.fun'ın creator payını (%0,30)
alır, bonded creator %0,50 alır; pay piyasa değeriyle düşmez.

- Her pay aşağı yuvarlanır; **protokol kalanı alır**, böylece paylar her zaman ücrete eşittir.
- İşlem bir arayüz (terminal, bot, referans) olmadan geldiyse arayüz payı protokole gider.
- Bonded token'da creator payının yarısı escrow'a gider; ne zaman serbest kalacağı Bonded Creator şartnamesindedir.
- **Tut, kazan (§5.8):** Bonded creator'ın payı kasa kilit oranıyla çarpılır; kaybedilen kısım Rekt-back havuzuna gider.
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
6. **Meteora DAMM v2** havuzu $L_{SOL}$ ve $L_{tok}$ ile kurulur ve pozisyon **kalıcı olarak kilitlenir**. Kilitli
   likidite hiçbir zaman çekilemez ama işlem ücreti kazanmaya devam eder; ücretler ayrıca toplanabilir. Pozisyonu
   temsil eden NFT bir kişide değil, HerdCall programının hesabında (PDA) durur; ücretleri yalnızca program kurallarına
   göre toplayıp dağıtabilir (§10). **Neden yakılmıyor:** LP'yi yakmak da likiditeyi kilitler, ama ücret hakkını da
   sonsuza kadar yok eder.
7. Durum **Migrated** olur. Curve bir daha açılmaz.

**Uygulama (programda, testli).** `breakout` tek işlemde:
- Meteora DAMM v2 `initialize_pool` çağrısıyla havuzu açar. Token A launch tokeni, token B SOL (WSOL); fiyat
  aralığı tam (DAMM'ın min–max fiyatı).
- Açılış fiyatı $\sqrt{v/t}$'nin yukarı yuvarlanmışıdır, yani curve'ün son fiyatıdır; altına inmez.
- Likidite, ayrılan token ve SOL'e sığan en büyük değerdir. Matematik `herdcall-core::dex`'tedir ve DAMM'ın kendi
  yuvarlamasıyla birebir aynıdır.
- Pozisyonun **tamamı** `permanent_lock_position` ile kalıcı kilitlenir. Pozisyon NFT'si launch PDA'sındadır.
- Havuzun almadığı birkaç token birimi yakılır.
- Fiyat aralığının alt sınırı yüzünden SOL'ün yaklaşık $10^{-8}$'i (120 SOL'de ~1.100 lamport) havuza girmez; bu kısım
  protokol kasasına gider.
- Geçiş ücreti $m$ = 0,03 SOL havuzun kirasını öder; kalanı breakout'u çağırana gider. Yerel doğrulayıcıda
  ölçülen kira 0,0226 SOL, çağıranın net kazancı 0,0074 SOL. Kira değişirse breakout yine de çalışır: çağıran işlem
  süresince 0,05 SOL avans koyar ve aynı işlemde geri alır.
- Breakout tek işlemdir: 174.687 compute birimi, 26 hesap, 977 bayt (sınır 1.232).

**Havuzu önceden açma saldırısı.** DAMM'da config tabanlı havuzun adresi `["pool", config, mint'ler]`'dir. Herkese açık
bir config'te, curve'den birkaç token alan biri bu havuzu breakout'tan önce açıp breakout'u kalıcı olarak kilitleyebilir
(test: `nobody_else_can_create_the_canonical_pool`). Bu yüzden program yalnızca **havuz oluşturma yetkisi HerdCall
migrator PDA'sında olan özel bir config**'i kabul eder. Config ayrıca şunları sağlamalıdır:
- statik olmalı;
- ücreti SOL tarafında toplamalı;
- tam fiyat aralığını kapsamalı;
- en fazla %2 sabit ücret almalı.

Herkese açık config'ler yalnızca `devnet` derlemesinde kabul edilir. Özel config'i Meteora oluşturur (partner config);
mainnet öncesi gereklidir (§10). Yönetici config adresini değiştirebilir (`set_dex_config`), ama yeni config de aynı
kontrollerden geçmek zorundadır.

Mint ve freeze yetkileri breakout'ta değil, **token oluşturulurken** kaldırılır; arz baştan sabittir (§6).

**5.5 Fiyat sürekliliği.** $L_{SOL} \cdot t / v = (v - V_0 - m)\, t / v < t = T_0 - S_{curve}$. Dolayısıyla
$T_0 \le S_{curve} + S_{LP}$ ise $L_{tok}$ hiçbir zaman $S_{LP}$ ile kırpılmaz ve DEX fiyatı

$$p^* \le \frac{L_{SOL}}{L_{tok}} < p^* \cdot \left(1 + \frac{1}{L_{tok}}\right)$$

olur: curve'ün son fiyatına bir token biriminin yuvarlaması kadar eşittir. Kural sağlanmazsa (ör. $T_0 = 1{,}073$ milyar)
LP'ye yetecek token olmayabilir ve DEX **daha yüksek** fiyattan açılır: 186 native'lik Lobby'de %27 (§8.2). İki durumda da
DEX hiçbir zaman curve'ün altından açılmaz; holder breakout'ta değer kaybetmez. v0.1 önerisi $T_0 = S_{total}$:
fiyat sıçraması yok, fazla token yakılır.

**5.6 Breakout'a hiç ulaşmayan token.** Curve süresiz açık kalır; alım ve satım her an açıktır (C4). Süreli bir
kapanış (alımların kapanması) kararlaştırılmadı; istenirse eski token'ın artık breakout yapamayacağı ve holder'ların
çıkış yolunun tanımlandığı ayrı bir terminal durum gerekir. Ad ve ticker curve açıldığında kalıcı olmuştur
(HERDCALL.md §12). Stampede'nin tek
fiyatı, rezervde $N^2/(V_0 + 2N)$ kadar bir fazlalık bırakır (herkes satsa bile curve'de kalır; 80 native'lik
Lobby'de 20 native'in üzerinde). Bu fazlalık breakout'ta LP'ye gider; breakout olmazsa curve'de kalır (§10).

**5.7 Kilitli pozisyonun ücretleri.** Meteora DAMM v2'de ücretler havuza kendiliğinden eklenmez; pozisyon NFT'sini
tutan program onları toplar. Herkes çağırabilen bir `claim_lp_fees` talimatı ücreti toplar ve §5.8'e göre böler;
"likiditeye geri" payı aynı pozisyona likidite olarak eklenir ve o da kalıcı kilitlenir.

**Uygulama (programda, testli).** `claim_lp_fees`:
- Ücretleri launch PDA'sının imzasıyla toplar. Ücret yalnızca SOL'dür, çünkü config ücreti token B'den toplar. Token A
  ücreti gelirse çağrı reddedilir; aksi halde Lobby'nin talep edilmemiş tokenlerine karışırdı.
- Ücreti `split_lp_fees` ile böler (§5.8 tablosu, kilit oranı dahil):
  - creator payı creator ücret kasasına gider ve çekilebilir;
  - Rekt-back ve protokol payları ilgili kasalara gider;
  - "likiditeye geri" payı launch'ın SOL kasasında `lp_reinvest_pending` olarak bekler. Breakout'tan sonra bu kasa
    boştur.
- Çağıran yalnızca işlem ücretini öder; geçici WSOL hesabının kirası geri döner.
- Toplam `lp_fees_claimed`'de tutulur (creator ve keşif sıralaması için).
- Meteora, havuz ücretinin kendi protokol payını (herkese açık config'lerde %20) pozisyona gelmeden alır. %1'lik
  havuzda pozisyona işlem hacminin %0,8'i düşer; yerel doğrulayıcıda 6,06 SOL'lük alım 0,0485 SOL ücret getirdi.

**Likiditeye geri ekleme: tasarım önerisi (henüz uygulanmadı, karar bekliyor).** Ücret SOL olarak gelir. Tam aralıklı
pozisyona ekleme ise SOL ve tokeni havuz oranında ister. Bu yüzden bekleyen SOL'ün yaklaşık yarısıyla aynı havuzdan
token alınıp ikisi birlikte eklenmelidir. Herkes çağırabilen bir işlem sandviçlenebilir: saldırgan önce alır, işlem
pahalı alır, saldırgan sonra satar. Model (`npm run sim:reinvest`, [`sim/lp-reinvest.js`](../sim/lp-reinvest.js))
sabit çarpım havuzu kullanır; tam aralıklı DAMM pozisyonu buna çok yakındır. Ücret SOL tarafından alınır.

| Havuz ücreti | Sandviçin kâr etmediği en büyük ekleme (havuzdaki SOL'ün payı) |
|---|---|
| %0,25 | %0,51 |
| %0,5 | %1,02 |
| %1 | %2,05 |
| %2 | %4,19 |

Sınır yaklaşık **2 × havuz ücreti**dir. 120 SOL'lük ve %1 ücretli havuzda 2 SOL'e kadar her ekleme güvenli; 4 SOL'de
sandviç 0,19, 8 SOL'de 1,57 SOL kazanır.

Öneri, herkes çağırabilen bir `reinvest_lp` talimatı:
1. Tutar: $\min(\text{bekleyen},\ c \cdot \text{havuzdaki SOL})$. Önerilen $c$ = %1, yani %1'lik havuzda güvenli
   sınırın yarısı.
2. Bu tutarın gereken kısmıyla havuzdan token alır.
3. `add_liquidity` ile pozisyona ekler ve `permanent_lock_position` ile ekleneni de kilitler.
4. Kalan, bir sonraki çağrıya bekler.

Ayrıca en küçük parti olmalı (ör. 0,01 SOL), toz eklemeler önlensin. Başka yollar:
- (b) yalnızca protokol keeper'ı özel paketle (Jito) çağırır: merkezî bir adım;
- (c) payın kuralı değişir (ör. geri alım ve yakım): aynı sandviç hesabı geçerli, ürün kararı.

**Karar gereken:** yol (öneri: a), $c$ ve en küçük parti.

**5.8 Breakout sonrası ücret dağılımı ve "tut, kazan"**

| Toplanan ücretin | Likiditeye geri | Creator | Rekt-back havuzu | Protokol |
|---|---|---|---|---|
| Bonded | %25 | %50 | %10 | %15 |
| Unbonded | %25 | %30 | %15 | %30 |

- **Kilit oranı** $\lambda$ = kasada kilitli creator tokeni / breakout anındaki creator tahsisatı (en fazla 1).
  Bonded creator'ın payı (curve'de de, breakout sonrasında da) $\lambda$ ile çarpılır; kalan kısım Rekt-back havuzuna
  gider. Likidite, protokol ve arayüz payları $\lambda$'dan etkilenmez.
- $\lambda$ **yalnızca azalır**: her güncellemede şimdiye kadarki en düşük değer kullanılır. Kasadan çekip satan,
  sonra geri alıp yeniden kilitleyen creator payını geri kazanamaz.
- Vesting süresince creator kasadan çekemez, $\lambda = 1$'dir. Unbonded creator'ın kasası yoktur; payı sabittir.
- Ölçülen tek şey zincir üstündeki kasa bakiyesidir. Creator'ın başka cüzdanlardan satması kanıtlanamaz ve cezaya yol
  açmaz (Fair Score'da uyarı olarak görünür).
- Topluluk devralırsa (CTO, HERDCALL.md §6) creator'ın ücret hakkı ve kilit oranı topluluk multisig'ine geçer.
- Farm güvenliği değişmez: en ucuz farmcının (kendi creator'ı ve arayüzü) protokol payı başına maliyeti iki token
  türünde de 1,33'tür (REKT_MINING.md §2).

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
| C15 | Kilit oranı yalnızca azalır; yeniden kilitlemek geri getirmez | `creator lock ratio only goes down` |
| C16 | Kilit oranı yalnızca creator ile Rekt-back arasındaki bölüşümü değiştirir; paylar her zaman ücrete eşit; unbonded etkilenmez; her creator en az %0,30 alır | `hold to earn` |

Mutasyon kontrolü: ilk sürümde 8 kasıtlı hatanın 8'i, tut-kazan ve yeni oranlar için 5 hatanın 5'i yakalandı. Bozulan kurallar: kırpma, LP sınırı, geçiş ücreti, ücret kalanı,
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
| Bonded, arayüz üzerinden | 0,250 | 0,250 | 0,100 | 0,100 | 0,300 |
| Bonded, doğrudan | 0,250 | 0,250 | 0 | 0,100 | 0,400 |
| Unbonded, arayüz üzerinden | 0,300 | 0 | 0,100 | 0,150 | 0,450 |

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
| Yönetici rug'ı | Talimatlarda yönetici rolü yok: işlem durdurma, rezerv çekme, ücret değiştirme yetkisi yok. Program yükseltilebilir; kötü niyetli bir yükseltme rezervlere dokunabilirdi. Savunma: yükseltme yetkisi çoklu imzada, her yükseltme 72 saat önceden zincirde ilan edilir, kullanıcılar bu sürede çıkabilir; Meteora'daki kilitli likidite Meteora programı tarafından korunur ve yükseltmeyle çekilemez (HERDCALL.md §12). |

## 10. Açık sorular

- ✅ **DEX:** Meteora DAMM v2; LP kalıcı kilit. ✅ **Kilitli pozisyonun ücretleri:** %25 likiditeye, kalanı creator,
  Rekt-back ve protokole (§5.8). ✅ **Program yükseltilebilir** (çoklu imza + 72 saat zaman kilidi, HERDCALL.md §12).
- **Havuz ayarları:** ✅ ücret yalnızca SOL'den, tam fiyat aralığı, sabit ücret en fazla %2 (§5.4). **Açık:** ücret oranı
  (test config'i %1) ve Meteora'nın bu ücretteki protokol payı; ikisi de Meteora'nın oluşturacağı özel config'te
  belirlenir.
- **Geçiş ücreti $m$:** ✅ 0,03 SOL; ölçülen havuz kirası 0,0226 SOL, kalanı breakout'u çağıranın (§5.4).
- **Breakout'a ulaşmayan token'daki Stampede fazlalığı** (§5.6): curve'de mi kalsın, belirli bir süre işlem görmeyen
  tokenlerde holder'lara mı dağıtılsın? Dağıtım, ölü token tanımı ve Diamond Hooves ile birlikte düşünülmeli.
- ✅ **Program yükseltme:** Yükseltilebilir; çoklu imza + 72 saat zaman kilidi, doğrulanabilir derleme, hedef: denetim ve
  kararlı bir dönemden sonra değiştirilemez yapmak (HERDCALL.md §12). **Açık:** çoklu imzanın üyeleri ve eşiği.
- **Kayma varsayılanları ve MEV koruması** (ayrı şartname).
