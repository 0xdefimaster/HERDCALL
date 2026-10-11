# Stampy — Ürün Tasarımı

> **No snipers. Just the herd.**
> Solana üzerinde, sniper ve rug'a karşı tasarlanmış memecoin launchpad'i.
> Resmi adres: **stampy.meme**

Tasarım kanvası (landing + uygulama ekranları): https://claude.ai/artifact/SWHaqagUV2LVYHTUufPo7f

---

## 1. Problem

Mevcut launchpad'lerde (pump.fun ve benzerleri) kurallar şunları ödüllendiriyor:

- **Sniper / bundle botları**: token açıldığı ilk blokta en ucuz fiyattan alıp retail'e satıyor.
- **Dump eden devler**: token açmak bedava, dev anonim, satınca hiçbir sonucu yok.
- **Gizli arz**: aynı kaynaktan fonlanmış cüzdan kümeleri görünmüyor.
- **Kaybeden gidiyor**: rekt olan kullanıcının geri dönmesi için hiçbir sebep yok.

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
| **Breakout** | Curve dolunca likidite DEX'e (Raydium / Meteora) taşınır ve kilitlenir. Mint ve freeze yetkisi program tarafından **zorunlu** olarak kaldırılır. |

## 4. Stamped devler (ödül, ceza değil)

Damgalanmak **isteğe bağlıdır**. Damgasız token da açılabilir ama açıkça etiketlenir.

| | Stamped | Unstamped |
|---|---|---|
| Teminat (bond) | 2 / 3 / 5 SOL kilit | Yok |
| Dev payı | 30 / 60 / 90 gün lineer vesting | Vesting yok |
| Creator fee | **%0.3** | %0.1 |
| Feed sıralaması | Boost | Standart |
| Rozet | 🛡️ Stamped | Yok |

- Teminat **breakout'ta** deve iade edilir.
- Dev vesting bitmeden satarsa teminat **trampled** olur, yani slash edilip holder'lara dağıtılır.

## 5. Ücret modeli

Curve üzerinde sabit **%1** işlem ücreti. Tüm dağılım program tarafından zorunlu tutulur
ve on-chain herkese açıktır.

| | Protokol | Creator | Rekt-back havuzu |
|---|---|---|---|
| Stamped token | %0.5 | %0.3 | %0.2 |
| Unstamped token | %0.5 | %0.1 | %0.4 |

Unstamped token'lar havuza daha çok öder, çünkü herd daha çok risk taşır.

## 6. Fair Score

Her token sayfasında 0–100 arası tek bir skor gösterilir. **Bir uyarıdır, kapı değildir**:
düşük skorlu token da işlem görmeye devam eder.

Girdiler:
- Dev Stamped mi (bond miktarı, vesting süresi)
- Dev geçmişi (kaç launch, kaçı trampled, kaçı breakout)
- Top 10 holder yoğunluğu
- Bağlantılı cüzdan kümeleri (aynı kaynaktan fonlanan cüzdanlar)
- Stampede dağılımı (kaç farklı cüzdan katıldı)

Holder tablosunda küme üyeleri etiketlenir ("Cluster A" gibi).

## 7. Rekt-back ve Herd rank

**Rekt-back havuzu**:
- Ücretlerden ve trampled bond'lardan beslenir.
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

## 8. Dağıtım

- **Trading terminalleri ve Telegram botları** için ilk günden açık IDL / SDK.
- KOL cüzdanları token sayfasında etiketlenir.
- On-chain ve şeffaf referans ücretleri.
- Önce dar bir nişte hâkimiyet, sonra genişleme.

## 9. Marka

- **İsim**: Stampy, "stampede"den gelir: sürü hep birlikte koşar, sniper ezilir.
  Ayrıca "stamp" (damga, Stamped devler) ve "stomp" (ezmek) çağrışımları var.
- **Maskot**: boğa yavrusu (bull run). Fil kullanılmaz (Simpsons'taki Stampy ile karışmaması için).
- **Sözlük**: Lobby, Stampede, The Herd, Stamped, Trampled, Breakout, Rekt-back ("Back on your hooves").
- **Sloganlar**: *No snipers. Just the herd.* · *Everyone runs. Nobody front-runs.* · *Run with the herd.*
- **Görsel dil**: koyu sıcak zemin `#14120E`, Stampede turuncusu `#FF7A1A`, Stamped mavisi `#5AA9FF`;
  Bricolage Grotesque (başlık), DM Sans (gövde), JetBrains Mono (rakamlar).

## 10. Teknik notlar (Solana)

Mevcut PyrePad kodu EVM / Solidity (Monad). Stampy için on-chain kısım **Anchor (Rust)** ile yeniden yazılacak:

- `launch` hesabı: token metadata, Stampede penceresi, durum makinesi (Lobby → Curve → Breakout).
- `lobby_order` hesapları: Lobby boyunca emirler (escrow'da SOL).
- `settle_stampede`: pencere bitince permissionless çağrılır, clearing price'ı hesaplar, dağıtımı yapar, fazlayı iade eder.
- `dev_bond` + vesting hesabı: vesting bitmeden dev satışı → slash → Rekt-back havuzuna / holder'lara.
- `breakout`: likiditeyi DEX'e taşır, LP'yi kilitler, mint / freeze yetkisini kaldırır.
- Fee router: %1'i yukarıdaki tabloya göre böler.

## 11. Açık sorular

- Breakout eşiği ve bonding curve parametreleri
- Create fee miktarı
- Stampede cap'i (tek cüzdanın ya da toplamın üst sınırı)
- Rekt-back'te "bağlantılı cüzdan dump'ı" tespitinin on-chain mi off-chain mi yapılacağı
- Hukuki yapı (kumar / menkul kıymet riski, şirketin kurulacağı ülke)
