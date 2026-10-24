# Mainnet kontrollü deneme (smoke test): plan ve maliyet

Durum: **hazır, onay bekliyor. Mainnet'e hiçbir şey yüklenmedi, hiçbir işlem gönderilmedi.**

Bu belge HerdCall'ın mainnet'teki en küçük anlamlı denemesini tanımlıyor:
- tam adresler;
- adım adım komutlar;
- ölçülmüş maliyetler;
- geri döndürülemez sonuçlar.

Denemeyi `scripts/mainnet-smoke.mjs` çalıştırır. Aynı betik yerel doğrulayıcıda prova yapar; prova, mainnet'teki gerçek
Meteora DAMM v2 ve Metaplex programlarıyla birebir aynı binary'leri kullanır (SHA256'lar eşleşti).

## 1. Neden ayrı bir deneme programı

- Launch parametreleri `initialize` sırasında kalıcı olarak yazılır; sonradan değiştiren bir talimat yok.
- V0 henüz kararlaştırılmadı. CURVE_SPEC §1'deki 30 SOL yalnızca simülasyon örneği. Bu değerle bir curve'ü bitirmek
  yaklaşık 120 SOL ister.
- Bu yüzden deneme, test ölçeğindeki parametrelerle (`devnet` hazır ayarı) ayrı bir program kimliğinde yapılır:
  - V0 0,6 SOL;
  - minimum Lobby 0,1 SOL;
  - ad teminatı 0,01 SOL.
- Üretim programı, V0 ve diğer açık parametreler kararlaştırılınca yeni bir program kimliğiyle ayrıca yüklenecek.
- Bu denemede alınan ad ve ticker'lar yalnızca deneme programında kayıtlıdır. Üretim programının ad alanını etkilemez.

## 2. Adresler

| Rol | Adres | Not |
|---|---|---|
| Program (deneme) | `EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K` | Mainnet'te boş (kontrol edildi). Anahtarı `programs/keys/herdcall-devnet.json`, git dışında |
| Deneme cüzdanı (yönetici, creator, yatıran, alıcı) | `jeNjnzpZtzcnrZ6f6nAWZ4Tmrjb9yAtTzr3dTBFAK1e` | Bu oturumda üretildi, yalnızca bu deneme için. Ana cüzdan değil |
| Keeper | `9VSdNCWyhtxgAnn6kqBxN2UPyaEzDTk8NKDM84QNtwis` | Ayrı anahtar. Yönetici anahtarıyla çalışmayı reddeder |
| Migrator PDA | `eBJPrAAKGWS5xkJDX4vavg9dMQoU2JRcWob6wukf54F` | Meteora özel config'inin havuz yaratıcısı bu olmalı |
| Meteora DAMM v2 | `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` | Testlerdeki binary mainnet'tekiyle aynı |
| Metaplex Token Metadata | `metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s` | Aynı |
| DAMM config | **yok** | Mainnet'te havuz yaratıcısı migrator olan config sayısı: 0 (sorgulandı). 2. aşama bunu bekler |
| Geri dönüş adresi | **sizden** | Yükseltme yetkisi ve kalan SOL buraya gider. Yalnızca açık adres, özel anahtar değil |

Yüklenecek binary: `programs/target/deploy/herdcall.so`. Boyutu 761.552 bayt. SHA256:
`d9aac3e1c5aeb14061ad7c33d9739d67e5998fd75944bb9a162251b0a55921b6`. Mainnet derlemesidir: `devnet` özelliği kapalı,
yani yalnızca özel config kabul eder.

Anahtarlar bu geçici konteynerde duruyor. Konteyner kapanırsa kaybolurlar; o durumda yeni anahtar üretilir ve bu tablo
güncellenir. Bu yüzden:
- cüzdana yalnızca o aşamanın tavanı kadar SOL gönderilir;
- her aşamanın sonunda kalan SOL geri dönüş adresinize süpürülür;
- yükseltme yetkisi kurulumdan hemen sonra sizin adresinize devredilir. Böylece program kirasını (3,87 SOL) konteynerden
  bağımsız olarak siz geri alabilirsiniz.

## 3. Aşamalar

### Aşama 1: şimdi yapılabilir (Meteora gerekmez)

Kapsam: program yükleme, kurulum, keeper atama, launch, Lobby, settle, claim, küçük alım ve satım, Lobby token'larını
curve'e geri satma, creator ücret çekimi.

DAMM config "ayarlanmamış" olarak kaydedilir (System Program adresi). Program breakout'ta bunu reddeder. Kamuya açık
config'e sessizce geçilmez.

Komutlar (onaydan sonra çalıştırılır):

```bash
export NODE_USE_ENV_PROXY=1 URL=https://api.mainnet-beta.solana.com   # ya da sizin RPC'niz
W=programs/keys/mainnet-trial-wallet.json K=programs/keys/mainnet-trial-keeper.json

# 1. Yükleme (yükseltme yetkisi geçici olarak W)
solana program deploy -u $URL -k $W --program-id programs/keys/herdcall-devnet.json \
  --with-compute-unit-price 20000 programs/target/deploy/herdcall.so
solana program dump -u $URL EPh2hoECkDTsSeHCriDwsF8DhVGhCjxwrbtC3qqyoJ8K /tmp/deployed.so && sha256sum /tmp/deployed.so

# 2. Plan ve onay belirteci (işlem göndermez)
node scripts/mainnet-smoke.mjs --plan --stage lobby --dex-config none --url $URL --keypair $W --keeper $K \
  --upgrade-authority-to <ADRESİNİZ> --sweep-to <ADRESİNİZ>

# 3. Deneme. Belirteç plandakiyle eşleşmezse veya RPC mainnet değilse hiçbir şey göndermez.
#    Kurulumdan hemen sonra yükseltme yetkisini adresinize devreder; sonda kalan SOL'ü adresinize süpürür.
node scripts/mainnet-smoke.mjs --live --approve <BELİRTEÇ> --stage lobby --dex-config none --url $URL \
  --keypair $W --keeper $K --upgrade-authority-to <ADRESİNİZ> --sweep-to <ADRESİNİZ> --report smoke-stage1.json
```

Yerel provada şunlar doğrulandı:
- Yetki devri yapıldı.
- Süpürmeden sonra iki anahtarda da 0 SOL kaldı.
- Yeni yetkili `solana program close` ile kiranın tamamını (yerelde 5,3016 SOL) geri aldı; yalnızca işlem ücreti
  düşüldü.

Mainnet'te kira 3,8696 SOL ve geri alma işlemini kendi cüzdanınızdan yaparsınız.

### Aşama 2: Meteora özel config'i gelince

Kapsam:
- `set_dex_config`;
- yeni bir launch ile Lobby, settle ve claim;
- curve'ü bitirme;
- breakout: DAMM v2 havuzu açılır, pozisyonun tamamı kalıcı kilitlenir;
- havuzda 7 × 0,5 SOL gidiş-dönüş, ücret birikmesi için;
- `claim_lp_fees`;
- keeper hizmetinin "likiditeye geri" payını yatırması (deneme için 60 saniyelik fiyat penceresi; üretimde 30 dakika);
- token'ları havuza geri satma, creator ücret çekimi ve süpürme.

Gönderim RPC üzerinden yapılır, öncelik ücreti 20.000 mikro-lamport/CU. Gerçek Jito uç noktası bu ortamdan erişilemiyor
(HTTP 403). Bu yüzden Jito yolu yalnızca sahte block engine ile test edildi. Reinvest'i bant ve `min_out` korur;
bu denemedeki tutar yaklaşık 0,012 SOL.

## 4. Maliyet

Ölçümler yerel provadan, mainnet ile aynı programlarla alındı. Mainnet'in kira oranı yerel doğrulayıcıdakinden düşük.
Bu yüzden hesap kiraları üst sınırdır. Öncelik ücretleri eklendi.

### Program yükleme

| Kalem | SOL | Geri alınır mı |
|---|---|---|
| Program verisi kirası (761.597 bayt, mainnet kira oranıyla) | 3,869563 | Evet: `solana program close` ile, yükseltme yetkilisi |
| Program hesabı kirası | 0,000833 | Hayır |
| 762 yazma işlemi + öncelik ücreti | yaklaşık 0,005 | Hayır |
| **Toplam** | **yaklaşık 3,875** | Yerelde ölçüldü: yükleme yalnızca 1× kira ister. Ara tampon hesabının SOL'ü program verisine aktarılır |

### Aşama 1 (yerel prova, adım adım)

| Adım | Cüzdan (SOL) | Not |
|---|---|---|
| initialize | 0,005448 | Config, kasalar, defter kirası |
| keeper atama | 0,000005 | |
| create_launch | 0,037918 | Mint, metadata (Metaplex ücreti dahil), kayıtlar, kasalar; 0,01 teminat |
| Lobby yatırma | 0,101467 | 0,1 SOL, minimum |
| settle | −0,009995 | Teminat geri |
| claim | 0,002044 | Token hesabı kirası |
| 0,01 SOL alım, geri satım | 0,000208 | %2 ücret |
| Lobby token'larını curve'e satma | −0,085846 | Lobby yatırımı, ücretler düşülerek |
| creator ücret çekimi | −0,000511 | |
| **Net** | **0,0507** | En yüksek anlık ihtiyaç: 0,145. Tavan: 0,25 |

### Aşama 2 (yerel prova)

| Adım | Cüzdan (SOL) | Not |
|---|---|---|
| set_dex_config, keeper'a 0,06 SOL | 0,060010 | Keeper SOL'ü sonda süpürülür |
| create_launch … claim | 0,131 | Aşama 1 ile aynı |
| curve'ü bitirme | 2,386039 | |
| breakout | −0,007389 | Havuz kirasını öder, breakout ücretini (0,03) geri alır |
| 7 havuz gidiş-dönüşü | 0,502114 | 0,5 SOL WSOL'e çevrilir (sonda geri), ücretler |
| claim_lp_fees | 0,000005 | 0,0525 SOL ücret toplandı ve bölündü |
| keeper reinvest | keeper 0,000005 | 0,0121 SOL kilitli pozisyona eklendi |
| token'ları havuza satma | −2,454690 | |
| creator ücret çekimi | −0,032451 | |
| **Net (iki anahtar birlikte)** | **0,525** | En yüksek anlık ihtiyaç: 3,07. Tavan: 3,3 |

Net maliyetin büyük kısmı:
- havuzda kalıcı kilitli kalan yaklaşık 0,405 SOL;
- protokol, Rekt-back ve creator escrow kasalarında kalan toplam yaklaşık 0,032 SOL (dağıtım talimatı yok);
- Meteora'nın ücret payı;
- kiralar.

### Toplam azami SOL

| | Gönderilecek | Beklenen geri dönmeyen |
|---|---|---|
| Yükleme + aşama 1 | **4,2 SOL** (3,875 + 0,25 tavan + ücret payı) | yaklaşık 0,056 SOL. Ayrıca 3,87 SOL kira, program kapatılana kadar kilitli |
| Aşama 2 (ayrıca, config gelince) | **3,3 SOL** | yaklaşık 0,53 SOL |
| **İkisi birlikte** | **en fazla 7,5 SOL** | yaklaşık 0,58 SOL. Program kapatılırsa 3,87 SOL geri döner |

Betik şu korumalarla çalışır:
- Cüzdan, aşamanın tavanından azını tutuyorsa başlamaz.
- Bilinen tutarı olan bir adımı göndermeden önce tavanı kontrol eder. Bir adımdan sonra tavan aşıldıysa durur.
- Prova bunu iki kez doğruladı: 0,05 SOL tavanla Lobby yatırması gönderilmeden durdu.

## 5. Geri döndürülemez sonuçlar

1. **Program mainnet'te olur.** Kapatılırsa (`solana program close`) aynı kimlik bir daha kullanılamaz. Kapatma ayrıca
   şunları kalıcı olarak erişilmez yapar: tüm PDA'lar, kilitli pozisyonun ileride birikecek LP ücretleri, kasalardaki
   SOL.
2. **İki token oluşur:** `HCSMOKE1` ("HerdCall Smoke Test 1") ve `HCSMOKE2` ("HerdCall Smoke Test 2").
   - Her birinin sabit 1 milyar arzı var; mint ve freeze yetkisi yok.
   - Metaplex metadata'ları değiştirilemez ve URI boş.
   - Token'lar zincirde kalır. Aşama 2'nin token'ı DAMM havuzunda herkese açık olarak işlem görebilir.
3. **Kalıcı kilit:** Aşama 2 havuzundaki pozisyon kalıcı kilitli. Yaklaşık 0,40 SOL ve karşılığı token'lar geri
   alınamaz.
4. **Çıkışı olmayan kasalar:** Protokol, Rekt-back ve bonded creator escrow kasalarındaki SOL'ü (iki aşamada toplam
   yaklaşık 0,034 SOL) çıkaran bir talimat yok. Ancak bir program yükseltmesiyle çıkar.
5. **Aşama 1'in curve'ünde kalan pay:** Yaklaşık 0,012 SOL. Bu, Lobby'nin curve'de bıraktığı Stampede fazlası;
   breakout olmadan çıkmaz.
6. **Parametreler sabit:** Bu programın parametreleri test ölçeğinde ve değiştirilemez. Program yalnızca deneme içindir.
7. **Yönetici değişmez:** Yönetici rolü deneme cüzdanında kalır; `set_admin` talimatı yok.

## 6. Sizden gerekenler

1. **Onay:** Bu planın 1. aşaması için tek bir onay.
2. **Geri dönüş adresi:** Kendi cüzdanınızın açık adresi.
3. **Fon:** Onaydan sonra `jeNjnzpZtzcnrZ6f6nAWZ4Tmrjb9yAtTzr3dTBFAK1e` adresine 4,2 SOL. Ana cüzdan anahtarı
   gerekmez ve istenmez.
4. **İsteğe bağlı: kendi RPC'niz.** Herkese açık mainnet RPC'si yüklemenin 762 yazma işlemini yavaşlatabilir.
5. **Aşama 2 için:** Meteora'dan, havuz yaratıcısı migrator PDA'sı (`eBJPrAAKGWS5xkJDX4vavg9dMQoU2JRcWob6wukf54F`)
   olan özel bir DAMM v2 config'i. Şartlar:
   - statik;
   - ücret yalnızca SOL tarafında;
   - tam fiyat aralığı;
   - sabit ücret %2 veya daha az.

   Config gelince 3,3 SOL ve ikinci bir onay gerekir.
