# HerdCall v8 Anchor programı: inceleme raporu

Kapsam: `herdcall-program/` (v8 zip, 14 Ekim 2026). Ana-ağ (mainnet) işlemi veya canlı dağıtım yapılmadı.

## Doğrulama durumu: neyin gerçekten çalıştırıldığı

| Kontrol | Durum |
|---|---|
| `cargo test`, orijinal kod | **DERLENMİYOR** (E0433, `pubkey!` makro yolu). README'deki "cargo test doğrulandı" notu bu sürüm için geçerli değil |
| Derleme düzeltmesinden sonra orijinal birim testleri | 24/25 geçti. `full_curve_pool_sol_is_15x_accepted` 2 lamport yuvarlama farkıyla kaldı (test beklentisi fazla katıydı) |
| Bu dalda birim testleri | 27/27 geçiyor |
| Bu dalda instruction seviyesi entegrasyon testleri (native `solana-program-test`) | 16/16 geçiyor |
| `anchor build` / `cargo build-sbf` / IDL | **ÇALIŞTIRILAMADI.** Solana/Anza araç zinciri indirme adresleri bu ortamda engelli (HTTP 403) |
| Meteora DAMM v2 CPI'ları (`breakout`, `claim_position_fee`, `buyback_burn`) | **TEST EDİLMEDİ.** DAMM programı test ortamına yüklenemiyor |
| Devnet / localnet | YAPILMADI |

Entegrasyon testleri programı native olarak çalıştırır. Runtime kontrolleri (`UnbalancedInstruction`, rent, imza, sahiplik) yürürlükte, ancak SBF'e özgü sınırlar (compute unit, stack/heap, hesap sayısı) **ölçülmedi**.

## Bulgular (önem sırasına göre)

### Kritik

**C1. Breakout sonrası Stampede claim tamamen kapalı.** DÜZELTİLDİ, test edildi.
`claim_stampede` yalnızca `Curve | CurveFull` kabul ediyordu. Breakout'tan sonra durum `Breakout` olduğu için claim edilmemiş token'lar (escrow) ve cap fazlası iadeler (launch PDA) kalıcı olarak kilitleniyordu. Düzeltmeden önceki test sonucu: `Custom(6005) InvalidState`.
Test: `tests/stampede_claim.rs::claim_works_after_breakout_for_every_depositor`. Breakout **simüle edildi**; SOL çıkışı, instruction'ın kullandığı `breakout_sweep_sol` fonksiyonuyla hesaplanıyor ve mint yetkisi iptal ediliyor.

**C2. İadeli her claim işlemi düşüyor (Curve aşamasında da).** DÜZELTİLDİ, native test edildi.
İade, token CPI'sinden **önce** doğrudan lamport hareketiyle (launch → owner) yapılıyordu. Runtime her CPI'dan önce çağıranın hesaplarının dengeli olduğunu doğrular (`TransactionContext::push` → `UnbalancedInstruction`). CPI'ya giren `launch` eksilmiş, dışarıda kalan `owner` ise artmamış göründüğü için işlem düşüyordu. Native testte birebir yeniden üretildi ("sum of account balances before and after instruction do not match").
Düzeltme: önce tüm CPI'lar, en sonda doğrudan lamport hareketleri. Claim ücreti, yetiyorsa iadeden düşülüyor; yetmiyorsa kalanı CPI ile cüzdandan alınıyor.
Not: SBF/localnet üzerinde ayrıca doğrulanmalı.

**C3. `initialize_config` ön-çalıştırma (front-run) ile ele geçirilebiliyordu.** DÜZELTİLDİ, test edildi.
Deploy ile ilk çağrı arasındaki yarışta herhangi biri authority olabiliyordu. Bu da parametreler, DAO ve protokol çekimleri üzerinde tam kontrol demek. Artık çağıranın, bu programın ProgramData hesabındaki `upgrade_authority_address` olması gerekiyor. ProgramData adresi seed ile türetildiği için başka bir programın ProgramData'sı verilemiyor. Değiştirilemez (immutable) bir program initialize edilemiyor.
Testler: `tests/initialize_config.rs` (5 test). ProgramData, native test ortamında sentetik bir hesap.
İstemci etkisi: `InitializeConfig` artık `program_data` hesabı istiyor.

**C4. `breakout` ve `buyback_burn`, C2 ile aynı kalıbı içeriyor.** DOĞRULANMADI, DÜZELTİLMEDİ.
- `breakout_launch.rs` 1. adım: launch → breakout_authority doğrudan lamport aktarımı yapılıyor, ardından `system_program::transfer(auth → wsol)` CPI'si geliyor. `launch` bu CPI'da yok → büyük olasılıkla `UnbalancedInstruction`.
- `buyback_burn.rs` 1. adım: launch → breakout_wsol doğrudan aktarım, ardından `sync_native(wsol)` CPI'si. `launch` yok → aynı risk.

C2'deki mekanizma doğruysa breakout **hiçbir zaman** başarılı olamaz ve her launch `CurveFull` durumunda, tüm SOL'u içinde kalarak kilitlenir. DAMM yüklenemediği için test edilemedi; bu yüzden kod değiştirilmedi.
Öneri: Doğrudan lamport hareketlerini, ilgili bütün hesapların (launch dahil) yazılabilir olarak geçtiği bir CPI'dan önce yapın ya da hepsini CPI'lardan sonraya taşıyın. Ardından localnet'te DAMM ile entegrasyon testi yapın.

**C5. DAMM havuzu önceden açılarak breakout kalıcı olarak kilitlenebilir.** DOĞRULANMADI, DÜZELTİLMEDİ.
Meteora dokümanına göre config tabanlı havuzun adresi `["pool", config, sıralı mint'ler]`. `pool_creator_authority` atanmamış (public) bir config ile havuzu herkes açabilir. `Config.damm_config` açıklamasında da config'in "havuz oluşturma izni herkese açık olmalı" denmiş. Curve'den token alan bir saldırgan, breakout'tan önce aynı config ile havuzu açarsa `initialize_pool` CPI'si "hesap zaten var" hatasıyla düşer ve launch `CurveFull` durumunda kalır.
Öneri: `pool_creator_authority` = program PDA'sı olan özel (private) bir config (Meteora ile partner config) kullanın ya da var olan havuza likidite ekleyen bir geri dönüş yolu tasarlayın.

### Yüksek

**H1. ProtocolPool SOL'unun çıkış yolu yoktu.** DÜZELTİLDİ, test edildi.
İşlem ücreti, claim ücreti, breakout artığı ve LP SOL ücreti ProtocolPool'a giriyordu, ama hiçbir instruction SOL çıkaramıyordu (`total_distributed` hiç yazılmıyordu). Yeni `distribute_protocol(amount)` instruction'ı yalnızca authority tarafından çağrılabiliyor. Tutarı %50 HerdVault / %30 Builders / %20 Ops olarak bölüyor (`split_protocol_surplus`); hedef adresler Config'ten zorunlu tutuluyor ve rent korunuyor.
Spec'te "operasyon tabanı" aylık tutarı hâlâ açık soru. Bu yüzden tutarı authority belirliyor; taban zincirde modellenmedi.
Testler: `tests/protocol_pool.rs` (5 test).

**H2. Orantılı yuvarlama yüzünden son claim edenin iadesi takılıyordu.** DÜZELTİLDİ, test edildi.
Kabul edilen tutarların toplamı (`Σ accepted_i`), `accepted_total`'dan en fazla `depositors` lamport az olabiliyor ve breakout bu farkı curve SOL'u diye süpürüyordu. Son claim eden kişi rent'in altına düşüyordu (`InsufficientFundsForRent`). `Launch::refund_rounding_buffer()` artık breakout süpürmesinden ve satış solvency hesabından hariç tutuluyor.
Mutasyon kontrolü: tampon kapatıldığında test kırılıyor. Sınırın doğruluğu birim test özelliğiyle kontrol edildi.

**H3. Kod derlenmiyordu.** DÜZELTİLDİ (`pubkey!` makrosu artık prelude'dan geliyor).

### Orta

**M1. RektPool SOL biriktiriyor ama çıkış yolu yok.** DÜZELTİLMEDİ (tasarım kararı).
Spec'e göre v1'de bilinçli bir seçim ("dağıtım kuralları v2 ile tanımlanacak"). Yine de birikim, program yükseltmesi dışında erişilemez durumda. `rekt_select_bps`, `rekt_payout_cap_bps`, `rekt_payable` ve `prorate` kullanılmıyor. Mainnet öncesinde ya Rekt-back v1 dağıtımı ya da yönetişimli bir acil çıkış yolu kararlaştırılmalı. Mevcut durumu belgeleyen test: `rekt_pool_accrues_with_no_outflow_path`.

**M2. Config cüzdanları kullanılmıyor ve değiştirilemiyordu.** DÜZELTİLDİ (`update_wallets`, test edildi).

**M3. Curve dolmazsa SOL süresiz kalıyor.** DÜZELTİLMEDİ (tasarım). Breakout zaman aşımı veya geri sarma (unwind) yok. Satışlar açık, ama `burn_reserve` breakout olmadan harcanamıyor.

**M4. HerdVault'tan çıkış yok.** Dokümana göre $HERD çıkana kadar böyle olacak. Artık `distribute_protocol` ile besleniyor.

### Düşük

- **L1.** Claim yuvarlaması nedeniyle escrow'da depositor başına 1 token'dan az toz kalıyor ve geri alınamıyor (testte kesin miktar doğrulandı).
- **L2.** `claim_position_fee`: `breakout_wsol` hesabında önceden duran bağış wSOL'u, hesap kapatılırken çağırana gidiyor. Etkisi önemsiz.
- **L3.** Önceden var olan clippy stil uyarıları (`div_ceil`, doküman girintisi).

## Bu dalda olmayanlar / açık kalanlar

- C4, C5 ve tüm DAMM CPI'ları için localnet/devnet entegrasyon testi (SBF derlemesi + Meteora programı + SDK ile üretilen instruction karşılaştırması).
- Compute unit ve hesap sayısı ölçümü (ALT gereksinimi).
- RektPool dağıtım tasarımı ve operasyon tabanı tutarı.
