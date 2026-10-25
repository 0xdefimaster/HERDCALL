# Mainnet provası (dar kapsam): onay paketi

Durum: **hazır; program adresinizi, ardından onayınızı ve fonlamanızı bekliyor.** Mainnet'e hiçbir şey yüklenmedi,
hiçbir işlem gönderilmedi.

## 0. Bu ne, ne değil

Bu bir **mainnet denemesi**: tam sistemin ya da üretim launch'ının doğrulaması değil. Kapsamı:
- test ölçekli bir program yükleme;
- küçük bir launch akışı: kurulum, launch, Lobby, settle, claim, küçük alım ve satım;
- programı kapatıp kirayı geri alma.

Sınanmayanlar:
- Meteora breakout, kalıcı kilitli havuz, LP ücretleri ve keeper. Mainnet'te HerdCall migrator'ı için özel DAMM v2
  config'i yok; DAMM config "ayarlanmamış" kurulur ve program breakout'ta bunu reddeder.
- **Üretim ekonomisi.** Test ölçeği parametreleri üretim parametrelerini doğrulamaz; V0 gibi değerler kararlaştırılmadı.
- **Güvenlik.** Harici denetim yapılmadı.

## 1. Anahtarlar: hepsi sizin bilgisayarınızda

| Anahtar | Ne işe yarar | Nerede |
|---|---|---|
| Test cüzdanı | Ücretleri öder; program yükseltme yetkilisi, HerdCall yöneticisi, creator, yatıran, alıcı | Sizin bilgisayarınızda. Ana cüzdanınız değil |
| Program anahtarı | Yalnızca ilk yüklemede program adresini imzalar; sonra yetkisi yoktur | Sizin bilgisayarınızda |

Bana gereken yalnızca **program anahtarının açık adresi**; özel anahtarlar hiçbir yere gönderilmez.
- Bu depoda, sohbette veya bulut konteynerinde anahtar yok.
- Konteynerde daha önce üretilen ve hiç fonlanmamış deneme anahtarları silindi.
- Keeper bu provada gerekmez.

## 2. Kaynak, araçlar ve binary hash: kendiniz derleyip karşılaştırırsınız

Program adresi binary'ye derlenir (`HERDCALL_PROGRAM_ID`). Kesin hash, sizin adresinizle yapılan derlemenin hash'idir.

**Derleme komutu:**
```bash
HERDCALL_PROGRAM_ID=<program adresi> scripts/build-release.sh
```

Binary'nin baytlarını belirleyen her şey sabitlenmiştir:

| | |
|---|---|
| Derleyici | platform-tools **v1.57** (`--tools-version v1.57`; eksikse `cargo build-sbf` kurar) |
| Hedef | SBPF **v3** (`--arch v3`). Mainnet'te SBPF v3 yükleme ve çalıştırma özelliği **epoch 993'ten beri açık** (mainnet'ten okundu). Program mainnet'in özellik setiyle çalışan bir yerel doğrulayıcıya yüklendi ve çalıştı |
| Bağımlılıklar | `programs/Cargo.lock`, `--locked` |
| Profil | `programs/Cargo.toml` release: `opt-level = "s"`, `overflow-checks = true`. `devnet` özelliği **kapalı**: yalnızca özel DAMM config kabul eder |
| Burada kullanılan araçlar | solana-cli 4.3.1 (Agave, `src:487f1df2`, stable kanalı), cargo-build-sbf 4.4.0 |

**Tekrarlanabilirlik:** Binary, derleyen makinenin hiçbir yolunu içermiyor. Yalnızca göreli yollar (`herdcall/src/lib.rs`)
ve platform-tools'un kendi sabit yolu var. Burada sınandı:
- aynı commit'in ayrı bir klonu, başka bir dizinde ve temiz bir derleme klasörüyle;
- aynı derlemenin tekrarı;
- sabitlenmiş betikle derleme.

Üçü de aynı hash'i verdi (örnek kimlik `CvM8…dYw` → `f1ccc630…f9b660`).

Başka bir işletim sisteminde aynı çıktı bu ortamda sınanamadı. Docker servisi çalışmıyor; macOS da yok. Hash'iniz
farklı çıkarsa yüklemeyin; `scripts/build-release.sh` çıktısının tamamını bana gönderin.

**Süreç:**
1. Program adresinizi gönderirsiniz.
2. Ben derlerim ve şunları veririm: commit, araç sürümleri, boyut (761.552 bayt), SHA256. Binary'yi de
   `programs/release/mainnet-rehearsal/` altına commit'lerim.
3. Siz aynı commit'i derleyip hash'leri karşılaştırırsınız. **Eşleşmeden yükleme yapmayın, cüzdanı fonlamayın.**
4. Yüklemeden sonra `solana program dump` + `sha256sum` ile zincirdeki baytları kontrol edersiniz. Deneme betiği de
   aynısını kendisi kontrol eder; eşleşmezse hiçbir işlem göndermez (provada yanlış binary reddedildi).

## 3. Launch modu: unbonded

Program, bonded creator escrow'unu serbest bırakan bir talimata henüz sahip değil. Bonded bir launch, creator
ücretlerinin yarısını kalıcı kilitlerdi. Prova bu yüzden **unbonded** çalışır; bu betiğin varsayılanı. Provada creator
escrow'u 0 lamport kaldı.

Parametreler: test ölçeği (`devnet` hazır ayarı), programda kalıcı.

| Parametre | Değer |
|---|---|
| V0 | 0,6 SOL |
| Minimum Lobby | 0,1 SOL |
| Ad teminatı | 0,01 SOL |
| İşlem ücreti | %1 |
| Arz | 1 milyar |

Launch: ticker `HCSMOKE1`, ad "HerdCall Smoke Test 1", metadata URI'si boş.

## 4. İşlem sırası

| # | İşlem | İmzalayan |
|---|---|---|
| 1 | `scripts/build-release.sh`, hash karşılaştırması | yok |
| 2 | `solana program deploy` (762 yazma + 1 yükleme işlemi) | test cüzdanı, program anahtarı |
| 3 | `solana program dump` + `sha256sum` | yok (okuma) |
| 4 | `mainnet-smoke.mjs --plan`: plan ve onay belirteci | yok (okuma) |
| 5 | `mainnet-smoke.mjs --live` | test cüzdanı (+ yeni mint) |

5. adımın içi, sırasıyla:
- `initialize`;
- `create_launch` (unbonded);
- `deposit` 0,1 SOL;
- `settle`;
- `claim`;
- 0,01 SOL alım ve geri satım;
- Lobby token'larını curve'e satma;
- boşalan token hesabını kapatma;
- `withdraw_creator_fees`;
- programı kapatma (`--close-program`);
- isteğe bağlı: ana adresinize süpürme (`--sweep-to`).

Betiğin korumaları:
- RPC mainnet değilse durur.
- Zincirdeki baytlar binary ile eşleşmezse durur.
- `--approve` plandaki belirteçle eşleşmezse durur. Belirteç şunların hash'i: cüzdan, program, binary hash'i, launch
  modu, config, ad, tavan, kapatma ve süpürme seçimleri.
- Akış için 0,25 SOL tavanı var: cüzdan bunun altındaysa başlamaz, tavanı aşacak adımı göndermez.

## 5. Satır satır SOL hesabı

Prova, sizin komut sıranızla ve mainnet'in özellik setiyle çalışan yerel doğrulayıcıda yapıldı:
- ayrı program anahtarıyla derleme;
- `solana program deploy`;
- dump ve hash kontrolü;
- `--plan`, ardından akış ve programı kapatma.

Öncelik ücreti 20.000 mikro-lamport/CU. Hesap kuruşu kuruşuna kapandı.

Yükleme ve kapatma satırları **mainnet'in gerçek kirasıyla** gösterildi: program verisi 3,869563, program hesabı
0,000833 (mainnet RPC'sinden). Akış satırları yerel ölçüm. Yerelde hesap kirası yaklaşık %37 daha yüksek; bu satırlar
mainnet için üst sınırdır.

| Satır | SOL | Ne |
|---|---|---|
| Test cüzdanına gönderilen | **+4,150000** | |
| Yükleme | −3,874429 | 3,869563 kira (geri gelir) + 0,000833 program hesabı + 0,004033 ücretler |
| = yüklemeden sonra | 0,275571 | Akışın 0,25 tavanının üstünde |
| initialize | −0,005456 | Config, kasalar, defter kirası |
| create_launch | −0,037926 | 0,01 teminat (geri gelir) + mint, metadata (Metaplex ücreti dahil), kayıtlar, kasalar |
| deposit (Lobby) | −0,101475 | 0,1 SOL (çoğu geri gelir) + yatırma hesabı kirası |
| settle | +0,009987 | Teminat geri |
| claim | −0,002052 | Token hesabı kirası (geri gelir) |
| 0,01 SOL alım | −0,010013 | |
| geri satım | +0,009789 | %2 ücret |
| Lobby token'larını curve'e satma | +0,085838 | 0,1'lik yatırımın geri gelen kısmı |
| boş token hesabını kapatma | +0,002026 | Kirası geri |
| creator ücret çekimi | +0,000607 | Creator payı geri |
| = akış bitince | 0,226896 | En düşük an: 0,130714 (yatırma sonrası) |
| programı kapatma | +3,869550 | Kira geri, ücret düşülerek |
| **= cüzdanda kalan** | **4,096446** | Süpürme yoksa sizin test cüzdanınızda kalır |
| süpürme (`--sweep-to`, isteğe bağlı) | −4,096433 | Ana adresinize gönderilen; 0,000013 işlem ücreti |
| **Kayıp (gönderilen − kalan − süpürülen)** | **yaklaşık 0,0536** | Üst sınır |

Kayıp şunlardan oluşuyor:
- yükleme ücretleri ve program hesabı: 0,0049;
- programın kalıcı hesap kiraları (config, kasalar, defter, launch, mint, metadata ve Metaplex ücreti, kayıtlar,
  yatırma hesabı): 0,0349;
- işlem ücretleri ve curve'de kalan Stampede fazlası: 0,0138. Bu kalem içinde curve fazlası 0,0123;
  protokol ve Rekt-back payları da kasalarda kalır, çünkü çıkış talimatı yok.

## 6. Prova sonunda yetki kimde kalır

- **Varsayılan plan (`--close-program`):** Program kapanır. Yükseltme yetkisi diye bir şey kalmaz; kira test
  cüzdanınıza döner. HerdCall yöneticisi rolü de programla birlikte biter.
- **Kapatmazsanız:** Yükseltme yetkisi, yönetici ve keeper rolü test cüzdanınızda kalır (`set_admin` talimatı yok).
  Bu durumda 3,8696 SOL kira program kapatılana kadar kilitli kalır. Yükseltme yetkisini isterseniz ana cüzdanınızın
  açık adresine devredebilirsiniz (`--upgrade-authority-to`; kapatma ile birlikte kullanılamaz).
- Hiçbir aşamada yetki konteynerde ya da bende değildir.

## 7. Geri döndürülemez sonuçlar

1. **Token:** `HCSMOKE1` mint'i zincirde kalır. Sabit 1 milyar arzı var, mint ve freeze yetkisi yok, metadata'sı
   değiştirilemez. Program kapanınca token'la curve'de işlem yapılamaz.
2. **Program kapatma kalıcıdır:** Aynı adres bir daha kullanılamaz. PDA'lardaki bakiyeler (protokol 0,0011,
   Rekt-back 0,0003, curve fazlası 0,0123, hesap kiraları) erişilmez olur. Bunlar §5'teki kayba dahil.
3. **Ad ve ticker kayıtları:** Yalnızca bu prova programındadır. Üretim programını etkilemez.
4. **Üretim yüklemesi ayrıdır:** Kesinleşmiş parametrelerle ve yeni bir program kimliğiyle yapılır.

## 8. Sizin adımlarınız

1. Bilgisayarınızda anahtarları üretin ve **yalnızca program adresini** gönderin:
   ```bash
   solana-keygen new -o ~/herdcall-rehearsal/wallet.json
   solana-keygen new -o ~/herdcall-rehearsal/program.json
   solana-keygen pubkey ~/herdcall-rehearsal/program.json
   ```
2. Benim vereceğim commit, araç sürümleri ve SHA256'yı bekleyin.
3. Aynı commit'i derleyip karşılaştırın:
   ```bash
   git fetch origin claude/solana-launchpad-alternative-uxyipv && git checkout <commit>
   HERDCALL_PROGRAM_ID=<program adresi> scripts/build-release.sh
   ```
   Eşleşmezse durun.
4. Planı inceleyin. Bu komut hiçbir işlem göndermez:
   ```bash
   export HERDCALL_PROGRAM_ID=<program adresi> URL=<mainnet RPC> W=~/herdcall-rehearsal/wallet.json
   node scripts/mainnet-smoke.mjs --plan --stage lobby --dex-config none --url $URL --keypair $W --close-program [--sweep-to <ana adres>]
   ```
5. Onay verirseniz test cüzdanına 4,15 SOL gönderin, yükleyin ve çalıştırın:
   ```bash
   solana program deploy -u $URL -k $W --program-id ~/herdcall-rehearsal/program.json --with-compute-unit-price 20000 programs/target/deploy/herdcall.so
   solana program dump -u $URL $HERDCALL_PROGRAM_ID /tmp/deployed.so && sha256sum /tmp/deployed.so
   node scripts/mainnet-smoke.mjs --live --approve <BELİRTEÇ> --stage lobby --dex-config none --url $URL --keypair $W \
     --priority-micro-lamports 20000 --funded-sol 4.15 --close-program [--sweep-to <ana adres>] --report rehearsal.json
   ```
6. `rehearsal.json` raporunu isterseniz bana gönderin. Satır satır hesabı içerir; provayla karşılaştırırım.

Ortam: Node 22, depoda `npm ci`, solana-cli 4.3.x.

Meteora özel config'i gelince breakout, LP ücreti ve keeper için ikinci aşama ayrı bir paket olarak hazırlanır.
`--stage full` yerelde prova edildi: net 0,525 SOL, en yüksek ihtiyaç 3,07 SOL. O aşama ayrı bir keeper anahtarı
gerektirir.
