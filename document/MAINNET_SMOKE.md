# Mainnet provası (dar kapsam): onay paketi

Durum: **hazır, onay ve fonlama bekliyor.** Mainnet'e hiçbir şey yüklenmedi, hiçbir işlem gönderilmedi.

## 0. Bu ne, ne değil

Bu tam bir launch değil, dar kapsamlı bir mainnet provası. Kapsamı:
- test ölçekli bir program yükleme;
- küçük bir launch akışı: kurulum, launch, Lobby, settle, claim, küçük alım ve satım;
- programı kapatıp kirayı geri alma.

Kapsam dışı ve hâlâ eksik:
- **Meteora breakout:** Mainnet'te HerdCall migrator'ı için özel DAMM v2 config'i yok. DAMM config "ayarlanmamış"
  olarak kurulur (System Program adresi). Program breakout'ta bunu reddeder.
- **Üretim parametreleri** kesinleşmedi; deneme test ölçeğini kullanır (§3).
- **Harici güvenlik denetimi** yapılmadı.
- **Doğrulanabilir derleme** (`solana-verify`) yapılmadı.

## 1. Anahtarlar: hepsi sizin bilgisayarınızda

| Anahtar | Ne işe yarar | Nerede |
|---|---|---|
| Test cüzdanı | Ücretleri öder; program yükseltme yetkilisi, HerdCall yöneticisi, creator, yatıran, alıcı | Sizin bilgisayarınızda. Ana cüzdanınız değil |
| Program anahtarı | Yalnızca ilk yüklemede program adresini imzalar; sonra yetkisi yoktur | Sizin bilgisayarınızda |

Bu depoda, sohbette veya bulut konteynerinde hiçbir anahtar bulunmaz. Konteynerde daha önce üretilen deneme
anahtarları hiç fonlanmamıştı (mainnet'te boş oldukları kontrol edildi) ve silindi. Keeper anahtarı bu provada
gerekmez: breakout olmadığı için yeniden yatırma da olmaz.

İmzalar kendi bilgisayarınızda, bir anahtar dosyasıyla atılır:
- `solana program deploy`, `dump` ve `close`;
- `node scripts/mainnet-smoke.mjs`.

Program yüklemesi 762 yazma işlemidir. Bunu donanım cüzdanıyla tek tek onaylamak pratik değil; bu yüzden ayrı bir test
cüzdanı dosyası kullanılır.

## 2. Derleme ve hash

Program kimliği binary'nin içine derlenir: `HERDCALL_PROGRAM_ID` ortam değişkeni. Bu yüzden kesin hash, sizin
program anahtarınızın **açık adresiyle** derlenen binary'nin hash'idir. Süreç:

1. Siz `solana-keygen new -o program.json` ile anahtarı üretir, yalnızca açık adresini gönderirsiniz.
2. Ben o adresle derler, binary'yi ve SHA256'sını bu dala commit'lerim:
   `programs/release/mainnet-rehearsal/herdcall-<adres>.so`.
3. Deneme betiği, yüklenen programın baytlarını bu dosyayla karşılaştırır; eşleşmezse hiçbir işlem göndermez. Bu
   kontrol provada denendi: yanlış binary reddedildi.

Derleme ortamı ve bugünkü durum:

| | |
|---|---|
| Araçlar | solana-cli 4.3.1, cargo-build-sbf 4.4.0, platform-tools v1.57, rustc 1.99.1 |
| Profil | release, `opt-level = "s"`, LTO, `overflow-checks = true`; `devnet` özelliği **kapalı** (yalnızca özel DAMM config kabul eder) |
| Varsayılan kimlikle derleme (`EPh2…oJ8K`) | 761.552 bayt, SHA256 `4f591fff31d792ec03dc4fd0a73229a822838b5742129e57184a5cea0dda1b9e`. Yeniden derlemede aynı çıktı |
| Sizin kimliğinizle derleme | Açık adres gelince. Boyut aynı (761.552 bayt), hash farklı olur |

Aynı kaynaktan başka bir makinede yapılan derleme, dosya yolları binary'ye girdiği için farklı bir hash verebilir.
Kıyas bu yüzden commit'lenen binary ile zincirdeki baytlar arasında yapılır.

## 3. Launch modu: unbonded

Program, bonded bir creator'ın escrow'unu serbest bırakan bir talimata henüz sahip değil. Bonded bir launch creator
ücretlerinin yarısını kalıcı olarak kilitlerdi. Bu yüzden prova **unbonded** çalışır. Bu betiğin varsayılanı;
provada creator escrow'u 0 lamport kaldı.

Parametreler: `devnet` hazır ayarı, test ölçeği.

| Parametre | Değer |
|---|---|
| V0 | 0,6 SOL |
| Minimum Lobby | 0,1 SOL |
| Ad teminatı | 0,01 SOL |
| İşlem ücreti | %1 |
| Arz | 1 milyar |

Bu değerler programda kalıcıdır; bu program yalnızca prova içindir. Launch'ın ticker'ı `HCSMOKE1`, adı "HerdCall Smoke
Test 1", metadata URI'si boş.

## 4. İşlem sırası

| # | İşlem | İmzalayan | Tutar |
|---|---|---|---|
| 1 | `solana program deploy` (762 yazma + 1 yükleme işlemi) | test cüzdanı, program anahtarı | 3,8696 SOL kira + yaklaşık 0,0046 ücret ve program hesabı |
| 2 | `solana program dump` + `sha256sum` | yok (okuma) | 0 |
| 3 | `mainnet-smoke.mjs --plan`: plan ve onay belirteci | yok (okuma) | 0 |
| 4 | `initialize` | test cüzdanı (yükseltme yetkilisi olduğu için) | 0,0054 kira |
| 5 | `create_launch` (unbonded) | test cüzdanı + yeni mint | 0,0379 (0,01 teminat dahil) |
| 6 | `deposit` 0,1 SOL | test cüzdanı | 0,1015 |
| 7 | `settle` | test cüzdanı | −0,0100 (teminat geri) |
| 8 | `claim` | test cüzdanı | 0,0020 (token hesabı kirası) |
| 9 | `buy_exact_in` 0,01 SOL, sonra `sell` | test cüzdanı | 0,0002 net |
| 10 | Lobby token'larını curve'e `sell` | test cüzdanı | −0,0858 |
| 11 | `withdraw_creator_fees` | test cüzdanı | −0,0006 |
| 12 | `solana program close` (önerilen) | test cüzdanı | −3,8696 (kira geri) |

4–11 arası adımları betik çalıştırır. Betiğin korumaları:
- RPC mainnet değilse durur.
- Yüklenen baytlar binary ile eşleşmezse durur.
- `--approve` plandaki belirteçle eşleşmezse durur. Belirteç şunların hash'i: cüzdan, program, binary hash'i, launch
  modu, config, ad ve tavan.
- Cüzdan 0,25 SOL tavanın altındaysa başlamaz.
- Tavanı aşacak bir adımı göndermez.

Öncelik ücreti 20.000 mikro-lamport/CU; tüm adımlar için toplam 0,001 SOL'ün altında.

## 5. Azami harcama ve geri alınabilirlik

| | SOL |
|---|---|
| **Test cüzdanına gönderilecek en fazla** | **4,15** (3,875 yükleme + 0,25 betik tavanı + ücret payı) |
| En yüksek anlık ihtiyaç (yükleme + akış) | yaklaşık 4,02 |
| Geri alınabilir: program kirası, `program close` ile | 3,8696 |
| Geri alınabilir: ad teminatı, settle'da otomatik | 0,01 |
| Geri alınabilir: Lobby yatırımı, token'lar curve'e satılarak | 0,0858 / 0,1 |
| **Geri dönmeyen** | **yaklaşık 0,055** |

Geri dönmeyen kısmın dökümü:
- yükleme ücretleri ve program hesabı kirası: 0,0046;
- programın hesap kiraları (launch, mint, metadata ve Metaplex ücreti, kasalar);
- %1 işlem ücretlerinin protokol ve Rekt-back payı (kasalardan çıkış yok);
- curve'de kalan Stampede fazlası (0,0123).

Bu rakamlar yerel provadan. Mainnet'in kira oranı daha düşük, yani gerçek tutar bunun altında kalır.

Kalan bakiye test cüzdanınızda kalır. Bu zaten sizin cüzdanınız; süpürme gerekmez, isterseniz `--sweep-to <adres>`
ile ana adresinize gönderilir.

Prova yerel doğrulayıcıda, sizin tarafınız taklit edilerek koşuldu:
- ayrı bir program anahtarıyla derleme;
- unbonded Lobby aşaması: net 0,0506 SOL, en yüksek ihtiyaç 0,145 SOL;
- `program close` ile kiranın tamamı geri geldi; yalnızca işlem ücreti düşüldü.

## 6. Prova sonunda yetki kimde kalır

- **Yükseltme yetkisi:** test cüzdanınız. Program sizin cüzdanınızla yüklenir; yetki hiçbir zaman konteynerde olmaz.
- **HerdCall yöneticisi ve keeper:** test cüzdanınız. `set_admin` talimatı yok, bu yüzden yönetici değişmez.
- **Önerilen son adım:** `solana program close`. Kira geri gelir ve yetki diye bir şey kalmaz.
- **Kapatmadan önce isterseniz** yetkiyi ana cüzdanınızın açık adresine devredebilirsiniz: `--upgrade-authority-to`
  ya da `solana program set-upgrade-authority`.

## 7. Geri döndürülemez sonuçlar

1. **Token:** `HCSMOKE1` mint'i zincirde kalır. Sabit 1 milyar arzı var, mint ve freeze yetkisi yok, metadata'sı
   değiştirilemez. Herkes görebilir; curve'de alıp satabilir (program kapatılana kadar).
2. **Program kapatma kalıcıdır:** Aynı program adresi bir daha kullanılamaz. Program PDA'larındaki küçük bakiyeler
   (protokol, Rekt-back, curve fazlası; toplam yaklaşık 0,014 SOL) erişilmez olur.
3. **Ad ve ticker kayıtları:** Yalnızca bu prova programında kalıcıdır. Üretim programını etkilemez.
4. **Parametreler:** Prova programının parametreleri değiştirilemez. Üretim için yeni bir program kimliğiyle ayrı
   yükleme gerekir.

## 8. Sizin adımlarınız

1. Bilgisayarınızda iki anahtar üretin; özel anahtarları kimseye vermeyin:
   ```bash
   solana-keygen new -o ~/herdcall-rehearsal/wallet.json
   solana-keygen new -o ~/herdcall-rehearsal/program.json
   ```
2. **Program anahtarının açık adresini** (`solana-keygen pubkey ~/herdcall-rehearsal/program.json`) gönderin. Ben o
   adresle derler, binary'yi ve hash'ini commit'lerim. İsterseniz test cüzdanının açık adresini de gönderin; komutları
   tam adreslerle doldururum.
3. Hash'i ve bu paketi gözden geçirip onay verince test cüzdanına en fazla 4,15 SOL gönderin.
4. Komutları kendi bilgisayarınızda çalıştırın. Gereken: depo bu dalın o commit'inde, Node 22, `npm ci`,
   solana-cli 4.3.x.
   ```bash
   export HERDCALL_PROGRAM_ID=<program adresi> URL=<mainnet RPC> W=~/herdcall-rehearsal/wallet.json
   SO=programs/release/mainnet-rehearsal/herdcall-$HERDCALL_PROGRAM_ID.so
   sha256sum $SO                                     # commit'teki hash ile aynı olmalı
   solana program deploy -u $URL -k $W --program-id ~/herdcall-rehearsal/program.json --with-compute-unit-price 20000 $SO
   node scripts/mainnet-smoke.mjs --plan --stage lobby --dex-config none --url $URL --keypair $W --so $SO
   node scripts/mainnet-smoke.mjs --live --approve <BELİRTEÇ> --stage lobby --dex-config none --url $URL --keypair $W --so $SO --report rehearsal.json
   solana program close -u $URL -k $W $HERDCALL_PROGRAM_ID --bypass-warning    # kirayı geri alır; kalıcıdır
   ```
5. İsterseniz `rehearsal.json` raporunu bana gönderin; adım adım gerçek maliyeti provayla karşılaştırırım.

Meteora özel config'i gelince breakout, LP ücreti ve keeper ile yapılacak ikinci aşama ayrı bir paket olarak
hazırlanır. `scripts/mainnet-smoke.mjs --stage full` hazır ve yerelde prova edildi: net 0,525 SOL, en yüksek ihtiyaç
3,07 SOL. O aşamada ayrı bir keeper anahtarı gerekir.
