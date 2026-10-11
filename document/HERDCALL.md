# HerdCall — Ürün Tasarımı

> **No snipers. Just the herd.**
> Solana üzerinde, sniper ve rug'a karşı tasarlanmış memecoin launchpad'i.
> Resmi adres: **[DOMAIN]** (henüz alınmadı)

Tasarım kanvası (landing + uygulama ekranları): https://claude.ai/artifact/SWHaqagUV2LVYHTUufPo7f

---

## 0. Durum özeti

**Elimizde olanlar**
- Bu doküman: ürün mekanikleri, ücret modeli, gelir dağıtımı, marka.
- Tasarım kanvası: Landing, Lobby (çalışan geri sayım), Token sayfası (Fair Score),
  Token oluşturma (Bellwether anahtarı), Rekt-back profili, Community Takeover oylaması.
- React / Vite arayüzü (`src/`): kanvastaki 6 ekran, piksel tema, mock veri ve
  Phantom cüzdan bağlantısı. Henüz on-chain işlem göndermiyor.
- Eski PyrePad EVM kontratları (`contract/`, `scripts/`, `hardhat.config.js`) hâlâ repo'da;
  on-chain kısım Anchor (Rust) ile yeniden yazılınca kaldırılacak.
- İsim geçmişi: PyrePad → Stampy (stampy.fun başka bir launchpad'e ait olduğu için bırakıldı) → **HerdCall**.

**Karar verilenler**
- İsim ve marka: HerdCall, piksel boğa maskotu, İngilizce / global.
- Lobby → Stampede → Curve → Breakout yaşam döngüsü (batch auction ile anti-sniper).
- Bonded Creator (Bellwether) (isteğe bağlı bond + vesting), Fair Score, Rekt-back, Herd rank.
- Protokol gelirinin 50 / 30 / 20 dağılımı ve token'ın gelirden **sonra** çıkması.
- **CTO kuralı** (bkz. §6).

**Önerilen, kesinleşmeyi bekleyen**
- Creator / arayüz / Rekt-back / protokol oranları (§5).
- Creator escrow oranı (%50 önerildi).
- Breakout sonrası LP'ye geri eklenecek pay (%25 önerildi).

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
| **Stampede** | Sayaç sıfırlanınca pen'deki tüm emirler **tek batch** olarak **tek bir clearing price** üzerinden gerçekleşir. Bloğa ilk giren ile son giren aynı fiyatı öder. Stampede cap'ini aşan kısım iade edilir. |
| **Curve** | Ardından normal bonding curve açılır, anlık al-sat. |
| **Breakout** | Curve dolunca likidite DEX'e (Raydium / Meteora) taşınır ve kilitlenir. Mint ve freeze yetkisi program tarafından **zorunlu** olarak kaldırılır. Breakout sonrası DEX havuzu ücretinin bir kısmı (öneri: %25) likiditeye geri eklenir. |

## 4. Bonded Creator (Bellwether) (ödül, ceza değil)

Damgalanmak **isteğe bağlıdır**. Damgasız token da açılabilir ama açıkça etiketlenir.

| | Bonded | Unbonded |
|---|---|---|
| Teminat (bond) | 2 / 3 / 5 SOL kilit | Yok |
| Dev payı | 30 / 60 / 90 gün lineer vesting | Vesting yok |
| Creator fee | **%0.30** | %0.10 |
| Creator escrow | Gelirin %50'si breakout'a / vesting sonuna kadar bekler | Yok |
| Feed sıralaması | Boost | Standart |
| Rozet | 🔔 Bellwether | Yok |

- Teminat **breakout'ta** deve iade edilir.
- Dev vesting bitmeden satarsa **trampled** olur: teminat **ve escrow'daki birikmiş creator geliri**
  slash edilip Rekt-back havuzuna / holder'lara gider.
- Escrow zamanla büyüyen ikinci bir teminat gibi çalışır: dev ne kadar başarılıysa,
  dump etmenin bedeli o kadar artar.
- Creator geliri en fazla **10 cüzdana** bölünebilir (ekip, sanatçı, KOL); hepsi token sayfasında görünür.
- Piyasa değerine göre kademeli creator fee **kullanılmaz** (pump.fun dersi).

## 5. Ücret modeli

Curve üzerinde sabit **%1** işlem ücreti. Tüm dağılım program tarafından zorunlu tutulur
ve on-chain herkese açıktır.

| | Creator | Arayüz / referans | Rekt-back havuzu | Protokol |
|---|---|---|---|---|
| Bonded token | %0.30 | %0.10 | %0.15 | %0.45 |
| Unbonded token | %0.10 | %0.10 | %0.35 | %0.45 |

- **Arayüz / referans payı**: işlemi getiren terminal, Telegram botu ya da referans linki alır
  (Zora ve Clanker'daki model). Terminallerin entegre olması için ekonomik sebep.
- Unbonded token'lar havuza daha çok öder, çünkü herd daha çok risk taşır.

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

Protokol payı (%0.45) önce **operasyon tabanını** doldurur (aylık sabit bütçe: RPC, indexer,
audit, hukuk). Kalan kısım şöyle dağıtılır:

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

**Rekt-back havuzu**:
- Ücretlerden, trampled bond'lardan ve slash edilen creator escrow'larından beslenir.
- Trampled dev ya da tespit edilen bağlantılı-cüzdan dump'larında holder'lara **orantılı** ödeme yapar.
- Ödemeler havuz bakiyesiyle **sınırlıdır**, sınırsız garanti verilmez.

**Herd rank (sezonluk XP)**: Calf → Yearling → Bull → Herd Leader

| Davranış | XP |
|---|---|
| Stampede'e katılmak | +40 |
| Token'ı 7. güne kadar tutmak | +120 |
| Rekt olduktan sonra geri dönmek | +200 |
| Breakout yapan token'ı almış olmak | +80 |

**Hacim XP kazandırmaz** (wash trading farm'ını engellemek için).

## 10. Dağıtım

- **Trading terminalleri ve Telegram botları** için ilk günden açık IDL / SDK + arayüz payı (§5).
- KOL cüzdanları token sayfasında etiketlenir.
- On-chain ve şeffaf referans ücretleri.
- Önce dar bir nişte hâkimiyet, sonra genişleme.

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
- Alınacaklar: domain (.fun / .xyz / .com), X handle, Telegram kullanıcı adı, marka tescil kontrolü
  ("herdcall launchpad", "herdcall solana" aramaları; "heardcall" yazımı için yedek domain).

## 12. Teknik notlar (Solana)

On-chain kısım **Anchor (Rust)** ile yazılacak:

- `launch` hesabı: token metadata, Stampede penceresi, durum makinesi (Lobby → Curve → Breakout).
- `lobby_order` hesapları: Lobby boyunca emirler (escrow'da SOL).
- `settle_stampede`: pencere bitince permissionless çağrılır, clearing price'ı hesaplar, dağıtımı yapar, fazlayı iade eder.
- `dev_bond` + vesting hesabı: vesting bitmeden dev satışı → slash → Rekt-back havuzuna / holder'lara.
- `creator_escrow`: creator gelirinin bekletilen yarısı; breakout / vesting sonunda serbest, trample'da slash.
- `fee_router`: %1'i §5'teki tabloya göre böler (creator alıcıları en fazla 10 cüzdan, arayüz payı).
- `cto_proposal` / `cto_vote`: tetikleyici kontrolü, holder oylaması, creator hakkının multisig'e devri.
- `breakout`: likiditeyi DEX'e taşır, LP'yi kilitler, mint / freeze yetkisini kaldırır.
- `herd_vault` + `builders_fund`: protokol payının kasaları (token öncesi birikim).

## 13. Yol haritası

1. **Ekonomi simülasyonu**: farklı hacimlerde ücret dağılımı, Rekt-back havuzu yeterliliği, operasyon tabanı.
2. **Anchor programı v0**: launch + Lobby / Stampede + bonding curve + fee router (devnet).
3. **Bonded Creator katmanı**: bond, vesting, creator escrow, trample.
4. **Breakout**: DEX migration + LP kilidi + yetki iptali.
5. **Arayüz**: mevcut React uygulamasını HerdCall tasarımına çevirmek (yapıldı: mock veri + Phantom bağlantısı); gerçek on-chain çağrılar ve wallet-adapter.
   (Tasarım kanvasındaki ücret çubukları eski 0.5 / 0.3 / 0.2 dağılımını gösteriyor, güncellenecek.)
6. **Fair Score indexer**: cluster analizi, dev geçmişi.
7. **CTO modülü + Rekt-back talepleri.**
8. **Audit → mainnet** (dar bir nişle başlangıç).
9. **$HERD + Herd Vault oylaması** (gelir kanıtlandıktan sonra).

## 14. Açık sorular

- Breakout eşiği ve bonding curve parametreleri
- Create fee miktarı
- Stampede cap'i (tek cüzdanın ya da toplamın üst sınırı)
- CTO tetikleyici süresi, oylama eşiği ve multisig yapısı
- Operasyon tabanının aylık tutarı
- Rekt-back'te "bağlantılı cüzdan dump'ı" tespitinin on-chain mi off-chain mi yapılacağı
- Hukuki yapı (kumar / menkul kıymet riski, şirketin kurulacağı ülke)
- Protokol token adı (`$HERD` şimdilik çalışma adı)
