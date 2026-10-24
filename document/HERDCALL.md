# HerdCall — Ürün Tasarımı

> **No snipers. Just the herd.**
> Solana üzerinde, sniper ve rug'a karşı tasarlanmış memecoin launchpad'i.
> Resmi adres: **herdcall.fun** (Netlify)

Tasarım kanvası (landing + uygulama ekranları): https://claude.ai/artifact/SWHaqagUV2LVYHTUufPo7f

---

## 0. Durum özeti

**Elimizde olanlar**
- Bu doküman: ürün mekanikleri, ücret modeli, gelir dağıtımı, marka.
- [`STAMPEDE_SPEC.md`](STAMPEDE_SPEC.md): Lobby → Stampede açılışının teknik şartnamesi (formüller, durum makinesi,
  invariant'lar) ve [`sim/`](../sim) altında referans model + otomatik testler (`npm run test:sim`, `npm run sim`).
- [`CURVE_SPEC.md`](CURVE_SPEC.md): Curve'de alım-satım, ücret dağıtımı ve Breakout (DEX'e geçiş) şartnamesi
  (`npm run sim:curve`).
- [`REKT_MINING.md`](REKT_MINING.md): "Kaybetsen bile kazanırsın" sezon puanlarının ekonomik modeli ve farm
  simülasyonu (`npm run sim:rekt`).
- Tasarım kanvası: Landing, Lobby (çalışan geri sayım), Token sayfası (Fair Score),
  Token oluşturma (Bellwether anahtarı), Rekt-back profili, Community Takeover oylaması.
- React / Vite arayüzü (`src/`): kanvastaki 6 ekran, piksel tema, mock veri ve
  Phantom cüzdan bağlantısı. Henüz on-chain işlem göndermiyor.
- Eski PyrePad EVM kontratları (`contract/`, `scripts/`, `hardhat.config.js`) hâlâ repo'da;
  on-chain kısım Anchor (Rust) ile yeniden yazılınca kaldırılacak.
- İsim geçmişi: PyrePad → Stampy (stampy.fun başka bir launchpad'e ait olduğu için bırakıldı) → **HerdCall**.

**Karar verilenler**
- İsim ve marka: HerdCall, piksel boğa maskotu, İngilizce / global.
- **Zincir: Solana.** DEX: Meteora DAMM v2. Çekirdek matematik Rust'ta yazıldı (`programs/herdcall-core`).
- Lobby → Stampede → Curve → Breakout yaşam döngüsü (batch auction ile anti-sniper).
- **Stampede modeli (STAMPEDE_SPEC v0.2):** Lobby havuzu zilde tek alım yapar, herkes yatırdığı oranda token alır
  ve herkes curve'ün açıldığı fiyattan girer. Üst sınır yok, iade yok; curve aynı fiyattan açılır.
- Bonded Creator (Bellwether) (isteğe bağlı bond + vesting), Fair Score, Rekt-back, Herd rank.
- Protokol gelirinin 50 / 30 / 20 dağılımı ve token'ın gelirden **sonra** çıkması.
- **CTO kuralı** (bkz. §6).
- **İlk sürüm (v1) kapsamı:** Lobby/Stampede, bonding curve, breakout ve temel creator taahhütleri (§13).
- **Sonraki aşamalar:** Diamond Hooves (uzun süre tutup zararına satanlara ücret iadesi; bütçe protokol gelirinin 1/50'si,
  çekiliş yok, bkz. §9) ve Rekt-back v1 aşama 4'te; zarar bazlı Rekt-back v2 ekonomik simülasyon + hukuki inceleme sonrası.

**Önerilen, kesinleşmeyi bekleyen**
- Creator / arayüz / Rekt-back / protokol oranları (§5).
- Creator escrow oranı (%50 önerildi).
- Breakout sonrası LP'ye geri eklenecek pay (%25 önerildi).
- Mirror Mode (§10), Herd Caller (§9), Creator Bounty (§4): tasarım notu var, şartname yok.

---

## 1. Problem

Mevcut launchpad'lerde (pump.fun ve benzerleri) kurallar şunları ödüllendiriyor:

- **Sniper / bundle botları**: token açıldığı ilk blokta en ucuz fiyattan alıp retail'e satıyor.
- **Dump eden devler**: token açmak bedava, dev anonim, satınca hiçbir sonucu yok.
- **Gizli arz**: aynı kaynaktan fonlanmış cüzdan kümeleri görünmüyor.
- **Kaybeden gidiyor**: rekt olan kullanıcının geri dönmesi için hiçbir sebep yok.
- **Ücretle spam**: küçük tokenlere yüksek creator fee vermek (pump.fun'ın 2025 "Dynamic Fees"
  modeli) risk almadan token açıp ücret toplamayı teşvik etti; pump.fun kurucu ortağı modelin
  başarısız olduğunu söyledi.

## 2. Temel ilke

**Sürtünme alıcıya değil, deve ve botlara biner.** Alıcı için deneyim pump.fun kadar
hızlı ve eğlenceli kalır. Koruma arka planda çalışır, uyarır ama engellemez.

Hedef "güvenli" değil, **adil** bir oyun: kaybedebilirsin ama hilesiz kaybedersin.

## 3. Yaşam döngüsü: Lobby → Stampede → Breakout

| Aşama | Ne olur |
|---|---|
| **Lobby** | Token oluşturulur, 15 / 30 / 60 sn'lik pencere açılır (dev seçer). Kullanıcılar SOL yatırır, **işlem yoktur**, snipe edilecek bir şey yoktur. |
| **Stampede** | Sayaç sıfırlanınca havuz **tek bir alım** yapar ve herkes yatırdığı oranda token alır. Herkes **aynı fiyatı** öder: curve'ün açılacağı fiyat. Lobby'ye düşen token $X = N \cdot T_0 / (V_0 + 2N)$ ($N$: ücret sonrası havuz). Talep ne kadar büyük olursa olsun fazlası için **iade yoktur** (yalnızca settle hiç gerçekleşmezse herkes parasını geri çeker); havuz büyüdükçe fiyat yükselir. Geri sayım boyunca "zil şimdi çalsa herkes X mcap'ten girer" canlı gösterilir. |
| **Curve** | Curve, Stampede fiyatından açılır (yeniden çapalanmış rezervler). Zilden sonra ilk alan biraz daha yüksek öder, ucuz pencere kalmaz. Curve her zaman tüm satışları karşılayacak kadar rezerv tutar (STAMPEDE_SPEC §4). |
| **Breakout** | Curve'den satılabilecek tokenlerin tamamı (örnekte 800M) satılınca curve kapanır, rezerv ve LP tokenleri **Meteora DAMM v2**'ye taşınır ve pozisyon kalıcı olarak kilitlenir (yakılmaz; kilitli likidite ücret kazanmaya devam eder). DEX **curve'ün son fiyatından** açılır, asla altından değil; LP'ye gerekmeyen token yakılır. Breakout'u herkes çağırabilir. Mint ve freeze yetkisi token oluşturulurken kaldırılır. Breakout sonrası DEX havuzu ücretinin bir kısmı (öneri: %25) likiditeye geri eklenir. Ayrıntı: [`CURVE_SPEC.md`](CURVE_SPEC.md). |

## 4. Bonded Creator (Bellwether) (ödül, ceza değil)

> **Tasarım ilkesi (v1):** Cezalandırmak yerine ihlali baştan imkânsız kılmak. Creator'ın **taahhüt ettiği token
> tahsisatı** bir vesting kasasında (vault) kilitlenir ve takvime göre serbest bırakılır. Creator'ın cüzdanlarının
> tamamı dondurulmaz: bu, o cüzdanlardaki başka varlıkları etkiler ve entegrasyonları bozabilir. Solana'da
> Token-2022 Transfer Hook kullanımı her transferde ek program mantığı çalıştırır ve gerekli hesapları sağlayamayan
> cüzdan/uygulamalarda transfer başarısız olur; bu yüzden v1'de varsayılan çözüm kasadır, hook ayrıca test edilir.
> Teminat yalnızca **zincir üstünde kanıtlanabilen** koşullarda kesilir. Cüzdan analizi (Fair Score kümeleri)
> uyarı üretir ama tek başına teminat kesintisine sebep olmaz.

Damgalanmak **isteğe bağlıdır**. Damgasız token da açılabilir ama açıkça etiketlenir.

| | Bonded | Unbonded |
|---|---|---|
| Teminat (bond) | 2 / 3 / 5 SOL kilit | Yok |
| Dev payı | 30 / 60 / 90 gün lineer vesting | Vesting yok |
| Creator fee | **%0.50** (curve) / Meteora ücretinin **%50**'si (breakout sonrası) | %0.30 / Meteora ücretinin %30'u |
| Creator escrow | Gelirin %50'si breakout'a / vesting sonuna kadar bekler | Yok |
| Feed sıralaması | Boost | Standart |
| Rozet | 🔔 Bellwether | Yok |

- Teminat **breakout'ta** deve iade edilir.
- Kasadaki tahsisat vesting bitmeden çekilemez; bu kontrat seviyesinde engellenir.
- **Trample** (teminat ve escrow'daki creator gelirinin kesilmesi) yalnızca açıkça listelenmiş, zincir üstünde
  kanıtlanabilir koşullarda olur. Kesin liste ayrı şartnamede tanımlanacak; muğlak ya da cüzdan analizine dayalı
  koşul içermeyecek. Kesilen tutar Rekt-back v1 kurallarına göre holder'lara dağıtılır (§9).
- Escrow zamanla büyüyen ikinci bir teminat gibi çalışır: dev ne kadar başarılıysa,
  dump etmenin bedeli o kadar artar.
- Creator geliri en fazla **10 cüzdana** bölünebilir (ekip, sanatçı, KOL); hepsi token sayfasında görünür.
- Piyasa değerine göre kademeli creator fee **kullanılmaz** (pump.fun dersi): coin büyüdükçe creator'ın payı küçülmez.
- **Tut, kazan:** Bonded creator'ın ücret payı, tahsisatının kasada hâlâ kilitli duran oranıyla çarpılır. Vesting
  bitince creator çekmekte serbesttir, ama çektiği oranda payı **kalıcı olarak** düşer; oran yalnızca azalır (çekip
  satıp geri koymak payı geri getirmez). Kaybedilen pay Rekt-back havuzuna gider. Ölçülen tek şey zincir üstündeki
  kasa bakiyesidir; başka cüzdanlardan satış cezaya yol açmaz (kanıtlanamaz), Fair Score'da uyarı olarak görünür.
  Unbonded creator'ın kasası yoktur; payı sabittir.

### Creator Bounty: creator'ın kendi tahsisatından tutanlara ödül (isteğe bağlı, aşama 2)

Creator, kendi kilitli tahsisatının bir kısmını **tutanlara ödül** olarak ayırabilir. Örnek: "30 gün tutan herkese,
bakiyesiyle orantılı, tahsisatımın %20'si." Protokole maliyeti sıfırdır; ödül creator'ın kendi token'ından gelir.

- **Kilitli ve geri alınamaz:** Bounty launch anında kasaya kilitlenir. Creator sonradan geri çekemez,
  değiştiremez, iptal edemez. Creator trample olsa da bounty ödenir.
- **Hak kazanma:** Pencere boyunca (örneğin launch'tan itibaren 30 gün) tutulan **en düşük bakiye** esas alınır,
  Rekt-back v1'deki gibi. Pencerenin sonunda alıp payı kapmak işe yaramaz. Creator cüzdanları ve kasalar hariç.
- **Dağıtım:** En düşük bakiyeyle orantılı. Cüzdanı bölmek avantaj sağlamaz. Liste zincir verisinden hesaplanır,
  Merkle kökü yazılır, itiraz süresi tanınır, pull-claim ile çekilir.
- **v1'de yalnızca proje token'ı:** SOL bounty yok. Token'ın değeri sıfıra düşebilir; arayüz bunu "getiri vaadi"
  olarak değil, "creator'ın kendi tahsisatından tutanlara bonus" olarak gösterir.
- **Görünürlük:** Token sayfasında ve Lobby'de "Holder bounty: %X, Y gün" etiketi. Fair Score'a sinyal olarak
  girebilir; ama bounty'nin olmaması ceza değildir.
- **Etkisi:** Creator'ın satılabilir arzı azalır, topluluğa "uzun vadede buradayım" sinyali verir. Degen için
  "tutarsan ekstra" kancası.

## 5. Ücret modeli

Curve üzerinde sabit **%1** işlem ücreti (pump.fun: %1,25). Tüm dağılım program tarafından zorunlu tutulur
ve on-chain herkese açıktır.

**Rakip pump.fun, ilke: creator'dan yana olmak.** Her creator en az pump.fun'ın verdiği kadarını (%0,30) alır;
teminat yatıran (bonded) creator %67 fazlasını (%0,50) alır. Pump.fun'da creator payı piyasa değeri büyüdükçe
%0,05'e kadar düşer (kaynaklara göre); HerdCall'da düşmez.

| Curve (%1'in dağılımı) | Creator | Arayüz / referans | Rekt-back havuzu | Protokol |
|---|---|---|---|---|
| Bonded token | %0.50 (yarısı escrow'da) | %0.10 | %0.10 | %0.30 |
| Unbonded token | %0.30 | %0.10 | %0.15 | %0.45 |

**Breakout sonrası** (kilitli Meteora DAMM v2 pozisyonundan toplanan ücretin dağılımı, CURVE_SPEC §5.8):

| | Likiditeye geri | Creator | Rekt-back havuzu | Protokol |
|---|---|---|---|---|
| Bonded token | %25 | %50 | %10 | %15 |
| Unbonded token | %25 | %30 | %15 | %30 |

- Bonded creator'ın iki aşamadaki payı da "tut, kazan" kuralına bağlıdır (§4); kaybedilen pay Rekt-back'e gider.
- Holder'lara doğrudan ücret dağıtılmaz (menkul kıymet riski, her dağıtımda snapshot); holder'ı koruyan kısım Rekt-back havuzudur.

- **Arayüz / referans payı**: işlemi getiren terminal, Telegram botu ya da referans linki alır
  (Zora ve Clanker'daki model). Terminallerin entegre olması için ekonomik sebep.
- Unbonded token'lar Rekt-back havuzuna daha çok öder, çünkü herd daha çok risk taşır.
- v1'de Rekt-back ücret payı ayrı bir havuzda birikir; bu havuzun dağıtım kuralları v2 ile birlikte tanımlanacak.
  v1'de holder'lara yalnızca trample'da kesilen teminat ve escrow dağıtılır (§9).

## 6. ⭐ CTO kuralı (Community Takeover)

> **Dev trampled olursa ya da belirli bir süre hiç aktivite göstermezse, arzın belirli bir
> oranını tutan holder'lar oylamayla creator hakkını bir topluluk multisig'ine taşıyabilir.**

Neden önemli:
- Memecoin kültüründe ölü dev'in token'ını topluluğun sahiplenmesi (CTO) zaten yaygın;
  HerdCall bunu **on-chain ve kurallı** hale getiren ilk launchpad olabilir.
- Dev kaçsa bile token ölmek zorunda kalmaz: creator geliri artık topluluğun projesini finanse eder.
- Trample + CTO birlikte çalışır: dump eden dev hem teminatını hem escrow'unu hem de
  **gelecekteki tüm creator gelirini** kaybeder.

Netleşecek parametreler:
- Tetikleyiciler: trampled olmak (otomatik) / X gün boyunca dev aktivitesi yok (öneri: 14–30 gün).
- Oylama eşiği: teklif için arzın en az %Y'si, kabul için katılanların çoğunluğu.
- Multisig yapısı: kaç imzacı, imzacılar nasıl seçilir, kötüye kullanıma karşı zaman kilidi.
- Saldırı senaryosu: dev aktifken balinaların CTO ile creator hakkını ele geçirmeye çalışması
  → aktif ve Bonded creator'e karşı CTO açılamaz, sadece tetikleyiciler gerçekleşince açılır.

## 7. Protokol gelirinin dağıtımı

Protokol payından (bonded %0.30, unbonded %0.45) önce **Diamond Hooves bütçesi** (protokol gelirinin 1/50'si, §9) ayrılır,
sonra **operasyon tabanı** dolar (aylık sabit bütçe: RPC, indexer, audit, hukuk), ardından **Rekt Mining payı**
(gelirin en fazla %25'i, [`REKT_MINING.md`](REKT_MINING.md) §7.1; aşama 5'ten itibaren). Kalan kısım şöyle dağıtılır:

| Kalem | Pay | Ne yapar |
|---|---|---|
| **Herd Vault** | %50 | ve$HERD sahiplerinin oylamasıyla yönlendirilir |
| **Builders Fund** | %30 | Hackathon ödülleri ve gerçek ürün çıkaran ekiplere hibeler |
| **Operasyon / ekip** | %20 | Taban üstü operasyon ve ekip |

**Herd Vault (ve$HERD oylaması)**
- Oy gücü için $HERD 1–12 ay kilitlenir; uzun kilit = daha çok oy gücü (vote-escrow).
- Tek cüzdanın oy gücüne üst sınır (balina ve oy satın alma riskine karşı).
- Oylama serbest değil, **menüden** seçilir:
  - $HERD buyback & burn
  - Rekt-back havuzuna ek fon
  - Breakout yapan tokenlere likidite desteği
  - Builders Fund'a ek bütçe
- Oylayana doğrudan gelir dağıtılmaz (menkul kıymet riskini azaltmak için; avukatla teyit edilecek).

**Builders Fund**
- Başka projelerin token'ı **satın alınmaz** (pump suçlaması ve çıkar çatışması riski).
- SOL / USDC hibesi: hackathon ödülleri + breakout yapıp gerçek ürün çıkaran ekiplere
  milestone bazlı, vesting'li hibe.
- Ölçülebilir kriterler: on-chain kullanıcı sayısı, ürün yayında mı, kod açık mı.

**Token zamanlaması**
- $HERD ilk gün çıkmaz. Token çıkana kadar Herd Vault payı **on-chain bir kasada birikir**,
  token lansmanında oylamaya açılır. Gelir kanıtlanmış olur, token için beklenti oluşur.

## 8. Fair Score

Her token sayfasında 0–100 arası tek bir skor gösterilir. **Bir uyarıdır, kapı değildir**:
düşük skorlu token da işlem görmeye devam eder.

Girdiler:
- Dev Bonded (Bellwether) mi (bond miktarı, vesting süresi)
- Dev geçmişi (kaç launch, kaçı trampled, kaçı breakout)
- Top 10 holder yoğunluğu
- Bağlantılı cüzdan kümeleri (aynı kaynaktan fonlanan cüzdanlar)
- Stampede dağılımı (kaç farklı cüzdan katıldı)
- CTO durumu (token topluluk yönetimine geçtiyse gösterilir)

Holder tablosunda küme üyeleri etiketlenir ("Cluster A" gibi).

## 9. Rekt-back ve Herd rank

**Rekt-back v1 (ilk sürüm): zarar hesabı yok.**
- Bir creator trample olduğunda kesilen teminat ve escrow, o token'ın **uygun holder'larına token bakiyeleriyle
  orantılı** dağıtılır. Maliyet, kısmi satış ya da gerçekleşmiş zarar hesaplanmaz.
- Şartnamede tanımlanacak noktalar (öneri):
  - **Snapshot zamanı:** trample'ı tetikleyen işlemin bloğu.
  - **Uygunluk:** Trample'dan hemen önce alıp dağıtımdan pay kapmayı önlemek için, bakiye olarak geriye dönük bir
    pencerede (örneğin 24 saat) tutulan **en düşük bakiye** esas alınır. Creator'ın beyan ettiği cüzdanlar ve kasa hariçtir.
  - **Liste:** Zincir verisinden açık kaynaklı, tekrarlanabilir bir betikle hesaplanır, Merkle kökü zincire yazılır ve
    itiraz süresi tanınır. Herkes aynı veriden listeyi yeniden üretip doğrulayabilir.
  - **Dağıtım:** Pull-claim; her holder kendi payını Merkle kanıtıyla talep eder. Tek işlemde toplu gönderim yok.
- Ödemeler kesilen tutarla **sınırlıdır**, sınırsız garanti verilmez; "iade/sigorta" denmez.

**Rekt-back v2 (ertelendi):** Ücretlerden beslenen genel havuz ve zarar bazlı telafi (maliyet, kısmi satış ve
transfer muhasebesi gerektirir). Ekonomik simülasyon ve hukuki inceleme sonrası değerlendirilecek.

### ⭐ Diamond Hooves: uzun süre tutanlara ücret iadesi (çekiliş yok) — aşama 4

Amaç, kısa vadeli al-sat yapanı değil, **zararı uzun süre taşıyanı** ödüllendirmek. İlk tasarımdaki rastgele
çekiliş (Lucky Hoof) **kaldırıldı**: rastgele para ödülü birçok ülkede şans oyunu/çekiliş mevzuatına girebiliyordu
ve fonun yeterliliği öngörülemiyordu. Diamond Hooves'ta şans yoktur; şartı sağlayan her pozisyon payını alır.

**Kim hak kazanır?**
- Bir token'ı en az **7 gün** tutup **zararına satan** cüzdan, o pozisyon için hak kazanır.
- **"Sıfıra kadar tuttu"**: Token 30 gün boyunca işlem görmezse (ölü token), elinde tutanlar satmadan da hak kazanır.
- Yalnızca HerdCall'da satın alınan tokenler sayılır (başka yerden transfer edilenlerin maliyeti bilinmez).
  "Zararına" = satış fiyatı, cüzdanın o token için HerdCall'daki ortalama alış fiyatının altında.
- Creator cüzdanları ve kasası hariç.

**Bütçe hacimle orantılı, sabit pay:**
- Haftalık bütçe = o haftanın **protokol gelirinin 1/50'si** (%2). Protokol payı hacmin %0,30'u (bonded) ile %0,45'i
  (unbonded) arasında olduğu için bütçe hacmin yaklaşık **%0,006–0,009'u** eder. Örnek: 100.000$ unbonded hacimde
  protokol 450$ kazanır, bütçe **9$** olur; bonded hacimde 300$ ve 6$.
- Bütçe protokol payından, 50 / 30 / 20 dağılımından önce ayrılır (§7).
- Program hiçbir zaman protokolün kazandığından fazlasına mal olamaz; zarar etme riski yoktur.

**Dağıtım:**
- Bütçe, hak kazanan pozisyonlar arasında **o pozisyonda ödenen işlem ücretleriyle orantılı** bölünür
  (yani kullanıcının kendi hacmiyle orantılı).
- Bir pozisyonun alacağı iade, o pozisyonda ödediği ücreti **asla aşamaz**. Ücreti ödeyip iade toplamak her zaman
  zararınadır, bu yüzden istismar edilemez.
- Bütçe tüm tavanları dolduracak kadar büyükse artan kısım protokolde kalır.
- Liste Rekt-back v1 ile aynı yöntemle hesaplanır: zincir verisinden açık kaynaklı betik, Merkle kökü,
  itiraz süresi, pull-claim.

**Oyunlaştırma**: Hak kazananlar **Diamond Hooves** rozeti ve XP alır.

**Not**: Tutarlar küçüktür (örnekte ödenen ücretin birkaç %'si). Oran (`revenueDivisor`) tek bir parametredir;
gerçek hacim ve hak kazanan pozisyon verisi geldikçe gözden geçirilecek.

### Rekt Mining: "kaybetsen bile kazanırsın", sınırlı havuzla (v0.1 politikası, aşama 5)

Papertrade / EOS betting mining psikolojisi, kayba göre sınırsız token basmadan. Ayrıntı ve simülasyon:
[`REKT_MINING.md`](REKT_MINING.md).

- Sezon puanı = ödenen **protokol ücreti** × çarpan. Zarar çarpanı (1,5x) yalnızca **7+ gün** tutulup zararla
  kapanan pozisyonlarda; tutma çarpanı 1,25x; toplam çarpan en fazla 2x.
- Yeni ödül tahsisi **SOL cinsinden** ve sezonun uygun net protokol gelirinin en fazla %25'i ($\alpha \le 0{,}25$); ayrı
  bir Rekt Mining gelir payı. Model varsayımları altında, test edilen senaryolarda ödül kaynaklı net farm kârı
  sıfır veya negatiftir (ispat, test ve sınırları şartnamede). Sabit token bütçesi reddedildi: $HERD fiyatı yükselince
  farm kârlı olur.
- Pozisyon kapandığı sezonda bir kez puanlanır; 7 gün sayacı açılıştan başlar ve sezon değişiminde sıfırlanmaz.
- Ödül sezon kapanışından sonra dağıtılır: Merkle listesi, itiraz süresi, ardından $HERD gelirle **kontrollü alınır**
  (yeni token basılmaz; işlem başına en fazla %0,5 fiyat etkisi, aşılırsa alım durur) ve claim süresi içinde
  pull-claim ile çekilir.
- Yeni tahsis, harcanmayan SOL ve talep edilmemiş $HERD ayrı bakiyelerdir; devreden hiçbir bakiye bir sezonun
  %25 tavanını aşmasına yol açmaz.
- Diamond Hooves'tan ayrı bütçe ve ayrı muhasebe. Board ve trend sıralaması ham hacimle değil, uygun ücret ve
  doğrulanmış aktiviteyle yapılır.
- Ekonomik güvenlik iddiaları, gerçek likidite ve zincir üstü verilerle doğrulanana kadar model varsayımıdır.

**Herd rank (sezonluk XP)**: Calf → Yearling → Bull → Herd Leader

| Davranış | XP |
|---|---|
| Stampede'e katılmak | +40 |
| Token'ı 7. güne kadar tutmak | +120 |
| Rekt olduktan sonra geri dönmek | +200 |
| Breakout yapan token'ı almış olmak | +80 |

**Hacim XP kazandırmaz** (wash trading farm'ını engellemek için).

### Herd Caller: sonuca dayalı itibar (aşama 3)

Herd Memory'nin degen yüzü. Hacmi değil, **seçimlerin sonucunu** ölçer: hangi Lobby'lere girdin ve o tokenler
sonra ne yaptı. Örnek kart: *"BELL'i 120 SOL mcap'te çağırdı → 7. günde 40x"*.

- **Neden adil ölçülebilir:** Stampede'de herkes aynı fiyattan girer. İki kişiyi ayıran tek şey seçimdir;
  erken blok, bot veya sniper avantajı yoktur.
- **Ölçüm:** Her "çağrı" bir Lobby katılımıdır (en az bir alt eşik, örneğin 0,1 SOL). Sonuç, Stampede fiyatına
  göre sabit ufuklarda ölçülür (24 saat, 7 gün, 30 gün). Anlık fiyat değil, ufuk etrafındaki **zaman ağırlıklı
  ortalama fiyat** kullanılır; tek bir anda fiyatı şişirmek skoru değiştirmez.
- **Skor:** Çağrıların tavanlı getirilerinin ortalaması (tek bir 1000x skoru ele geçirmesin diye üst sınır),
  isabet oranı ve çağrı sayısı ayrı gösterilir. **Batan tokenler de sayılır**; kayıplar silinmez.
- **Sıralamaya girme şartı:** En az N çağrı (örneğin 10). Her Lobby'ye en küçük tutarla girmek ortalamayı yükseltmez,
  çünkü kaybedenler de ortalamaya girer.
- **Para ödülü yok:** Herd Caller yalnızca itibardır: rozet, sıralama, paylaşılabilir kart. Ekonomik ödül Rekt Mining'de,
  ayrı kurallarla. Böylece itibarı farm etmek için ekonomik teşvik küçük kalır.
- **Bilinen sınır:** Biri çok sayıda cüzdanla farklı seçimler yapıp yalnızca şanslı olanı gösterebilir. En az çağrı şartı
  ve uzun geçmiş bunu zorlaştırır, ortadan kaldırmaz. Kart, cüzdanın tüm çağrı geçmişine bağlantı verir.
- Creator'ın kendi tokenindeki katılımı çağrı sayılmaz.

## 10. Dağıtım

- **Trading terminalleri ve Telegram botları** için ilk günden açık IDL / SDK + arayüz payı (§5).
- KOL cüzdanları token sayfasında etiketlenir.
- On-chain ve şeffaf referans ücretleri.
- Önce dar bir nişte hâkimiyet, sonra genişleme.

### Mirror Mode: "sniper seni nasıl yerdi" kartı (zincir dışı, kontrattan önce yapılabilir)

Kullanıcı başka bir launchpad'de aldığı bir token'ı (ya da kendi cüzdanını) girer. Site zincir verisinden şunu çıkarır:

> *"Bu token'da sen girmeden önce ilk blokta 14 cüzdan aldı. Senin ortalama giriş fiyatın onlarınkinin 3 katıydı.
> HerdCall'da hepsiyle aynı fiyattan girerdin: tahmini Stampede mcap'i 85 SOL."*

- **Tamamen zincir dışı:** Cüzdan bağlamak gerekmez, işlem gönderilmez. Herkese açık zincir verisi (RPC / indexer)
  okunur. Kontrat ve zincir kararından bağımsızdır; lansmandan önce pazarlama aracı olarak çıkabilir.
- **Hesap:** "Senden önce giren" = token'ın ilk N slotunda ve kullanıcıdan önce alan cüzdanlar. Kullanıcının giriş fiyatı
  bu cüzdanların ortalamasıyla karşılaştırılır. "HerdCall'da ne olurdu" kısmı, ilk N slottaki toplam alımı bir Lobby havuzu
  sayıp Stampede formülüyle (`src/lib/stampede.js`) hesaplanır ve **tahmini** olarak etiketlenir.
- **Paylaşılabilir kart:** Piksel tema, maskot, tek tıkla X / Telegram paylaşımı. Viral döngü: herkes kendi acısını paylaşır.
- **Dürüstlük kuralları:**
  - Cüzdanlara "bot" ya da "dolandırıcı" denmez. Tanım açıktır: "ilk N slotta, senden önce alan cüzdanlar". Adresler kısaltılır.
  - Rakip platformun logosu ve markası kullanılmaz; yalnızca token adresi ve zincir verisi.
  - Karşılaştırma bir tahmindir; HerdCall'da kâr edeceğin iddia edilmez.
- **Uzantı:** Aynı kart, HerdCall'daki bir Stampede için de üretilir: "Bu launch'ta senden önce giren: 0."
- **Durum:** Sitede `/mirror` sayfası var. Örnek veriyle çalışır; canlı mod zincir verisini ziyaretçinin tarayıcısından
  herkese açık bir RPC ile okur (geçmişi en fazla 20.000 işlem olan tokenler, ilk 120 işlem çözülür). Kod:
  `src/lib/mirror.js`, `src/lib/mirrorRpc.js`; testler `npm run test:mirror`. Canlı mod mainnet'te yeni çıkmış bir
  token'la tarayıcıda uçtan uca denendi (herkese açık RPC, ~12 sn; 429 yanıtları yeniden denemeyle aşıldı). RPC,
  herdcall.fun kaynağına CORS izni veriyor. SOL tutarları öncelik ücreti ve tip'leri de içerir; sniper'ların yüksek
  tip'leri onların ortalama fiyatını yüksek gösterir, yani "kaç kat ödedin" oranı temkinli bir tahmindir.

## 11. Marka

- **İsim**: HerdCall, çiftçinin sürüyü toplamak için yaptığı çağrı: kullanıcıları bir lansman
  etrafında toplamak. Memecoin'deki ücretli "call" kanallarına karşı duruş:
  *"No paid calls. Just the herd call."* Yedek isim: Bellow.
- **Maskot**: piksel art boğa; lacivert şapka ve hoodie, güneş gözlüğü, hoodie'de boynuzlu "H".
  Görsel `public/herdcall-hero.webp` (AI üretimi; lansmandan önce bir piksel sanatçısına temiz versiyonu çizdirilmeli).
- **Logo işareti**: boynuzlu piksel "H" (`public/favicon.svg`, `src/components/icons.jsx`).
- **Sözlük**: Lobby, Stampede, The Herd, Bonded Creator / **Bellwether** (rozet), Trampled, Breakout,
  Rekt-back ("Back on your hooves"), Community Takeover.
- **Sloganlar**: *No snipers. Just the herd.* · *Get in before the bell.* · *Answer the call.* ·
  *Everyone runs. Nobody front-runs.*
- **Görsel dil**: siyah zemin `#0A0A0B`, turuncu `#FF7A1A`, lacivert `#1F2A44` / `#AFC2EE`, krem metin `#FFF0DC`;
  köşeli kartlar, 3px kenarlık, sert gölgeler. Fontlar: Silkscreen (başlık), VT323 (rakamlar), Pixelify Sans (gövde).
- **Metin kuralı**: Rekt-back için "refund / iade / insurance" denmez; "capped, pool-funded compensation".
- Alındı: herdcall.fun. Alınacaklar: yedek domainler (.xyz / .com), X handle, Telegram kullanıcı adı, marka tescil kontrolü
  ("herdcall launchpad", "herdcall solana" aramaları; "heardcall" yazımı için yedek domain).

## 12. Teknik notlar (Solana)

On-chain kısım **Anchor (Rust)** ile yazılacak:

- `launch` hesabı: token metadata, Stampede penceresi, durum makinesi (Lobby → Curve → Breakout).
- `lobby_order` hesapları: Lobby boyunca emirler (escrow'da SOL).
- `settle_stampede`: pencere bitince permissionless çağrılır, Lobby tahsisini ($X$) hesaplar, curve'ü Stampede fiyatına
  yeniden çapalar, ücreti ayırır; katılımcılar token'larını pull-claim ile alır.
- `dev_bond` + vesting hesabı: vesting bitmeden dev satışı → slash → Rekt-back havuzuna / holder'lara.
- `creator_escrow`: creator gelirinin bekletilen yarısı; breakout / vesting sonunda serbest, trample'da slash.
- `fee_router`: %1'i §5'teki tabloya göre böler (creator alıcıları en fazla 10 cüzdan, arayüz payı).
- `diamond_hooves`: haftalık bütçe (protokol gelirinin 1/50'si), Merkle kökü ile hak kazananlar, pull-claim.
- `cto_proposal` / `cto_vote`: tetikleyici kontrolü, holder oylaması, creator hakkının multisig'e devri.
- `breakout`: curve'den $S_{curve}$ satılınca herkes çağırabilir; rezervi ve LP tokenlerini curve'ün son fiyatından DEX'e
  taşır, fazla LP tokenini yakar, Meteora DAMM v2 pozisyonunu kalıcı kilitler; pozisyon NFT'si programın hesabında durur (CURVE_SPEC §5). Mint / freeze yetkisi token oluşturulurken kaldırılır.
- `herd_vault` + `builders_fund`: protokol payının kasaları (token öncesi birikim).

**Ad ve ticker rezervasyonu** (programda var, testli)

Aynı ad veya aynı ticker'la ikinci bir launch açılamaz. Lobby curve'e geçerse ad ve ticker kalıcı olarak o launch'a
bağlanır; Lobby iptal olursa serbest kalır. Curve'de süre sınırı yok.

- **İkisi de benzersiz.** Ticker: 1–10 karakter, yalnızca A–Z ve 0–9; istemci büyük harfe çevirir, program kanonik
  olmayanı reddeder. Ad: en fazla 32 bayt, kontrol karakteri yok; baştaki ve sondaki boşluklar ile büyük-küçük harf
  farkı yok sayılır (`Pepe Coin`, `  PEPE COIN ` aynı ad; aradaki boşluklar sayılır). Ad kaydının adresi kanonik
  biçimin sha256'sı; program aynı hesabı kendisi yapar.
- **Minimum Lobby** (`min_total`, **karar: 1 SOL**, parametre): katılımcılar birlikte bu tutara ulaşırsa curve açılır;
  ulaşamazsa launch iptal olur, herkes tam iade alır.
- **İade edilebilir teminat** (`name_bond`, **karar: 0,25 SOL**, parametre): yalnızca Lobby aşamasında durur.
  Launch açılırken ticker kaydına kilitlenir; curve açılınca (settlement) creator'a döner, iptalde kayıtlar
  kapanırken kiralarıyla birlikte döner. İadesiz ücret yok; breakout'a ulaşmayan token'da creator'ın parası kilitli
  kalmaz.

| Durum | Ad ve ticker | Teminat |
|---|---|---|
| Lobby açık veya settlement penceresinde | Geçici olarak ayrılmış | Kilitli |
| Lobby minimuma ulaştı, curve açıldı | **Kalıcı** (curve süresiz işlem görür) | Creator'a döner |
| Lobby minimumun altında kaldı (settle → iptal) | Hemen serbest bırakılabilir (`release_name`, herkes çağırabilir) | Serbest bırakmada döner |
| Lobby settle edilmeden süresi doldu (iptal) | Hemen serbest bırakılabilir | Serbest bırakmada döner |

- **Kapatma (squatting) simülasyonu** (`npm run sim:names`, [`sim/name-bond.js`](../sim/name-bond.js)). LiteSVM'de
  ölçülen, geri dönmeyen launch maliyeti 0,0072 SOL. Bir adı kalıcı almanın en ucuz yolu: Lobby'yi tek başına
  minimumla doldurmak, token'ları geri satmak. Geri dönmeyen kısım ücretler (creator ve arayüz payları geri alınmış
  olarak) ve Stampede'nin curve'de bıraktığı $N^2/(V_0+2N)$ fazlası; bu fazla Lobby tutarının karesiyle büyür.

  | Minimum Lobby | Bir adı kalıcı almanın kaybı | 1.000 ad: kalıcı kayıp | 1.000 ad: aynı anda gereken sermaye |
  |---|---|---|---|
  | 0,1 SOL | 0,009 | 9 | 359 |
  | 0,5 SOL | 0,021 | 21 | 759 |
  | **1 SOL** | **0,050** | **50** | **1.259** |
  | 2 SOL | 0,146 | 146 | 2.259 |
  | 3 SOL | 0,287 | 287 | 3.259 |

  Dürüst creator: 0,259 SOL peşin; teminat Lobby bitince döner, kaybı yalnızca launch kirası (~0,0072 SOL).

  **Sonuç:** Kapatmanın kalıcı maliyetini minimum Lobby belirler; teminat yalnızca Lobby sırasında gereken sermayeyi
  büyütür (toplu kapatma, 1.000 ad için +250 SOL). Minimumu yükseltmek kapatmayı karesel pahalılaştırır ama her
  gerçek launch'ın bu tutarı en fazla 1 saatlik Lobby'de toplamasını şart koşar.
- Kayıtlar devredilemez; kapatıcı adı güvenilir biçimde satamaz.
- **Token metadata (Metaplex):** `create_launch` gerçek Token Metadata programıyla metadata hesabını açar: ad (baştaki
  ve sondaki boşluklar atılmış), ticker sembol olarak, `uri` (en fazla 200 bayt; görsel ve açıklama bu JSON'da).
  **Değiştirilemez** (`is_mutable = false`), güncelleme yetkisi launch PDA'sında (hiçbir cüzdanda değil), telif payı
  0. Mint yetkisi metadata yazıldıktan sonra kaldırılır.
- Benzer görünen harfler (ör. Kiril `а` / Latin `a`) ve
  dile özgü harf dönüşümleri (Türkçe `İ`/`i`) ayrı ad sayılır.

**Kod yapısı ve araç zinciri**
- `programs/herdcall-core`: saf Rust, tam sayı ve taşma kontrollü aritmetik. Programdaki tüm hesaplar buradan gelir.
- `programs/herdcall`: Anchor 1.2 programı; hesap yapıları, talimatlar, Metaplex ve Meteora DAMM v2 çağrıları.
- `programs/herdcall-tests`: derlenmiş programı LiteSVM'de çalıştıran uçtan uca testler (`npm run test:program`).
  Metaplex ve Meteora DAMM v2 programlarının gerçek ikili dosyalarını yükler; `programs/fixtures/fetch.sh` bunları
  mainnet'ten indirir ve sha256 ile doğrular.
- `programs/idl/herdcall.json`: Anchor IDL (`npm run idl`, Anchor CLI gerekmez). `scripts/herdcall-client.mjs`: adresler,
  ad hash'i ve hesap listeleri; web istemcisi bunun üzerine kurulacak. Ad hash'i program ile aynı test vektörleriyle
  doğrulanır (`npm run test:client`).
- Yerel tam akış: `npm run validator:local` (mainnet derlemesi + özel config; `DEVNET=1` ile devnet derlemesi), sonra
  `npm run flow -- --url http://127.0.0.1:8899 --keypair <cüzdan> --dex-config AeLtDKgw3XnXbr3Kgfbcb7KiZULVCQ5mXaFDiG9n7EgW
  --params example`. Devnet: `scripts/devnet.sh`.
- Programı derlemek ve yerel olarak test etmek için Solana CLI, `cargo build-sbf` ve Anchor gerekir; LiteSVM 0.18
  Rust 1.97.1 veya üstünü ister. Bulut ortamında bunların indirilmesi için ağ ayarlarında `release.anza.xyz` ve
  `github.com` alan adlarına izin verilmeli.

**Program yükseltme politikası (karar: yükseltilebilir)**

Yükseltilebilir bir program hataların düzeltilmesine izin verir, ama yükseltme anahtarı aynı zamanda bir rug vektörüdür:
kötü niyetli bir sürüm rezervlere ve escrow'lara dokunabilir. Bu yüzden:

- **Tek anahtar yok:** Yükseltme yetkisi bir çoklu imzada (Squads; öneri 3/5, anahtarlar farklı kişi ve cihazlarda).
- **Zaman kilidi:** Her yükseltme zincirde ilan edilir ve en erken **72 saat** sonra uygulanabilir. Bu sürede kullanıcılar
  curve'de satabilir, Lobby'den ve claim'lerden çıkabilir. Kısa yol (acil durum) yok.
- **Doğrulanabilir derleme:** Her sürüm etiketli bir commit'ten tekrar üretilebilir; program hash'i yayımlanır.
- **Şeffaflık:** Sitede yükseltme yetkisinin adresi, bekleyen yükseltmeler ve geçmişi gösterilir.
- **Sınırlar:** Meteora'da kalıcı kilitli likidite Meteora programı tarafından korunur; HerdCall yükseltmesi onu çekemez
  (pozisyon NFT'si yalnızca ücret toplayabilir). Token'ların mint ve freeze yetkisi oluşturulurken kaldırıldığı için
  yükseltme yeni token basamaz.
- **Hedef:** Denetimler ve kararlı bir dönemden sonra yükseltme yetkisini kaldırıp programı değiştirilemez yapmak.
- **Açık:** Çoklu imza üyeleri, eşik ve değiştirilemez hâle geçiş tarihi.

## 13. Yol haritası

Özellikler birbirine bağımlılıklarına göre sıralanır. Bir aşama, başarı kriteri sağlanmadan sonrakine geçilmez.

1. **Çekirdek launch protokolü (ilk öncelik):** Lobby, Stampede fiyatlandırması, tahsis, bonding curve,
   breakout. Başarı kriteri yalnızca kontratın çalışması değil; [`STAMPEDE_SPEC.md`](STAMPEDE_SPEC.md)'deki
   invariant'ların saldırı ve sınır durumlarında korunması.
   - ✅ Şartname v0.1 + referans model + otomatik testler.
   - ✅ **Lobby kapasitesi kararı:** model B (STAMPEDE_SPEC v0.2). Havuz curve'ün açılış fiyatından alır, üst sınır
     ve iade yok; formül, ödeme gücü ispatı ve testler şartnamede.
   - ✅ **Curve + Breakout şartnamesi v0.1** ([`CURVE_SPEC.md`](CURVE_SPEC.md)): işlemler, ücret dağıtımı, token miktarıyla
     breakout, curve fiyatından DEX'e geçiş; referans model ve testler.
   - ✅ **Zincir: Solana** (GIWA elendi).
   - ✅ **Çekirdek matematik Rust'ta** ([`programs/herdcall-core`](../programs/herdcall-core)): Stampede settlement, curve,
     ücretler, "tut, kazan", breakout. Solana bağımlılığı yok; program ve araçlar aynı kodu kullanacak. JS referans
     modelinden üretilen test vektörleriyle birebir aynı sonucu verir (`npm run vectors`, `npm run test:core`).
   - ✅ **Anchor programı, ilk sürüm** ([`programs/herdcall`](../programs/herdcall)): Lobby, Stampede, claim/iade,
     curve işlemleri, ücret dağıtımı, breakout (keeper ücreti, fazla LP token yakımı). Uçtan uca testler
     ([`programs/herdcall-tests`](../programs/herdcall-tests), LiteSVM, `npm run test:program`) derlenmiş programı
     çalıştırır ve her sonucu çekirdekle karşılaştırır: tam yaşam döngüsü, zaman ve aşama kuralları, boş Lobby, kayma
     limiti, sahte hesaplar, cüzdan ve Lobby sınırları, kira tabanı. Programa bilerek eklenen 9 hatanın 9'unu yakalar.
   - ✅ **Ad ve ticker rezervasyonu** (§12): ikisi de benzersiz; curve açılınca kalıcı, Lobby iptalinde serbest;
     curve'de süre sınırı yok; minimum Lobby 1 SOL; teminat 0,25 SOL, yalnızca Lobby boyunca kilitli; kapatma
     simülasyonu. Uçtan uca testler.
   - ✅ **Meteora DAMM v2 breakout** (CURVE_SPEC §5.4): havuz curve'ün son fiyatından açılır, pozisyonun tamamı kalıcı
     kilitlenir, NFT launch PDA'sında kalır. Testler mainnet'ten alınmış gerçek DAMM v2 programıyla: havuzda gerçek
     swap, havuzu önceden açma saldırısı, yanlış config ve sahte hesaplar. Bilerek eklenen 6 hatanın 6'sı yakalanır.
   - ✅ **Gerçek doğrulayıcıda tam akış** (`scripts/launch-flow.mjs`): yerel test doğrulayıcısında gerçek Metaplex ve
     DAMM v2 programlarıyla çalışır. Adımlar: kurulum, launch, Lobby, settle, claim, curve'ün tamamı, breakout, kilitli
     havuz. Hem mainnet derlemesi + özel config hem devnet derlemesi + herkese açık config ile geçti. İstemci IDL'den
     çalışır (`npm run idl`, `programs/idl/herdcall.json`).
   - ⏳ **Devnet:** `scripts/devnet.sh` hazır (config bulma, devnet derlemesi, yükleme, tam akış). Bu oturumun ağ
     politikası `api.devnet.solana.com`'a izin vermediği için henüz çalıştırılmadı; yaklaşık 8 devnet SOL gerekir.
   - ✅ **Kilitli pozisyonun ücretleri** (CURVE_SPEC §5.7): `claim_lp_fees` ücreti PDA'dan toplar ve §5.8
     paylarıyla böler. Creator, Rekt-back ve protokol hemen ödenir; "likiditeye geri" payı kasada bekler. Testler
     gerçek DAMM v2 ile; yerel doğrulayıcıda tam akışın parçası.
   - ⏳ "Likiditeye geri" payının pozisyona eklenmesi (`reinvest_lp`): tasarım ve sandviç simülasyonu hazır
     (CURVE_SPEC §5.7). Uygulama için yol, çağrı başına tavan ve en küçük parti kararı bekliyor.
   - ⏳ Meteora'dan HerdCall migrator PDA'sı için **özel config** (mainnet şartı).
   - ⏳ Protokol kasasının HerdVault gelir sırasına göre dağıtımı (§7) ve creator puanı; ikisi de zincirdeki
     `lp_fees_claimed` ve ücret olaylarını kullanacak.
   - ✅ **Metaplex token metadata** (§12): değiştirilemez ad, sembol, URI; testler gerçek Metaplex programıyla.
2. **Creator taahhütleri:** Bonded Creator, kilitli tahsisat kasası, teminat, ücret yönlendirmesi. Cezalar yalnızca
   açıkça tanımlanmış ve zincir üstünde doğrulanabilir koşullarda. İsteğe bağlı **Creator Bounty** (§4).
3. **Şeffaflık ve keşif deneyimi:** Gerçek token verileri, işlem geçmişi, creator geçmişi, token yetkileri ve
   açıklanabilir Fair Score. Arayüz (Board, token sayfası) mevcut tasarımla gerçek verilere bağlanır.
   **Herd Caller** sıralaması (§9).
   - **Mirror Mode** (§10) zincir dışıdır; bu sıradan bağımsız olarak, kontratlardan önce pazarlama aracı olarak çıkabilir.
4. **Topluluk ve telafi sistemleri:** Community Takeover, Rekt-back v1, Diamond Hooves, ardından Rekt-back v2. Ekonomik
   simülasyonlar, saldırı testleri ve gerekli hukuki inceleme tamamlandıktan sonra.
5. **Gelir ve yönetişim:** Protokol gelirini gerçek hacimle ölçmek, operasyon bütçesini doğrulamak; ürünün
   sürdürülebilirliği kanıtlandıktan sonra $HERD ve Herd Vault.
   - Rekt Mining (sezon puanları) burada, $HERD ile birlikte: ✅ v0.1 ekonomi politikası ve farm simülasyonu
     ([`REKT_MINING.md`](REKT_MINING.md)); parametreler gerçek kullanıcı verisiyle güncellenecek.

**Deneyler (uygulanmış özellik değil):**
- Curve açılışında zamanla azalan anti-snipe ücreti: normal kullanıcıları da etkileyebilir ve botun ücreti
  ödemesini engellemez. v0.2'de zilden sonra ucuz pencere kalmadığı için ihtiyacı azaldı; ayrı simülasyonla değerlendirilecek.

**Notlar:**
- Arayüzdeki Diamond Hooves kartı "Planned" etiketiyle konsept önizlemesi olarak gösteriliyor.
- Eski PyrePad EVM kontratları (`contract/`) çekirdek kontratlar yazılınca kaldırılacak.

## 14. Açık sorular

- **Limit fiyat (v2 adayı):** "En fazla şu mcap'e kadar alırım" emri (STAMPEDE_SPEC §11).
- Curve: kilitli Meteora pozisyonunun ücret dağıtımı (%25 likiditeye önerisi, kalanı?), geçiş ücreti, breakout'a ulaşmayan tokendeki Stampede fazlalığı, program yükseltme yetkisi (CURVE_SPEC §10). ✅ Karar verilen: $T_0$ = 1 milyar, Meteora DAMM v2, LP kalıcı kilit.
- Create fee miktarı
- Likiditeye geri ekleme (CURVE_SPEC §5.7): herkes çağırabilen `reinvest_lp` + çağrı başına tavan (öneri: havuzdaki
  SOL'ün %1'i) mi, protokol keeper'ı mı, yoksa payın kuralı mı değişsin; en küçük parti.
- Meteora DAMM v2: HerdCall migrator PDA'sı için özel (partner) config; havuz ücreti oranı (test config'i %1) ve
  Meteora'nın bu ücretten aldığı protokol payı (herkese açık config'lerde %20).
- Breakout'a ulaşmayan token için süreli kapanış istenirse ayrı bir terminal durum (holder'ların çıkış yolu
  tanımlı) gerekir; şimdilik yok. ✅ Karar verilen: minimum Lobby 1 SOL, ad/ticker teminatı 0,25 SOL (ikisi de parametre).
- Lobby toplam tavanı $D_{max}$ ve isteğe bağlı cüzdan tavanı $w_{max}$
- CTO tetikleyici süresi, oylama eşiği ve multisig yapısı
- Operasyon tabanının aylık tutarı
- Rekt Mining: operasyon tabanı tutarı, claim süresi ve alım penceresi, alım havuzunun derinliği ve kayma toleransı, ödül $HERD'üne vesting, MEV/sandwich koruması
- Herd Caller: ölçüm ufukları (24 saat / 7 gün / 30 gün), getiri tavanı, en az çağrı sayısı, en küçük çağrı tutarı
- Creator Bounty: en fazla tahsisat payı, pencere süreleri, Fair Score'a etkisi
- Mirror Mode: "ilk N slot" tanımı, veri sağlayıcı (RPC / indexer) ve maliyeti
- Diamond Hooves parametreleri: tutma süresi (7 gün), bütçe oranı (1/50), ölü token eşiği (30 gün), "zararına" tanımı (ortalama alış fiyatı)
- Trample koşullarının kesin listesi (yalnızca zincir üstünde kanıtlanabilir olanlar)
- Rekt-back v1 snapshot penceresi (24 saat önerisi) ve itiraz süresi
- Hukuki yapı (kumar / menkul kıymet riski, şirketin kurulacağı ülke)
- Protokol token adı (`$HERD` şimdilik çalışma adı)
