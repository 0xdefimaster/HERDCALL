# HerdCall – Anchor programı (v0 iskeleti)

Kaynak tasarım: `document/HERDCALL.md`. Yol haritası 2. adım.

## Kapsam (v0)
| Instruction | Ne yapar |
|---|---|
| `initialize_config` | Config + RektPool + HerdVault + ProtocolPool + DaoPool hesaplarını açar. **Sadece programın upgrade authority'si** çağırabilir (`program_data` hesabı seed ile doğrulanır); treasury/builders/ops varsayılan adres olamaz |
| `distribute_protocol` | Sadece authority: ProtocolPool SOL'unu §7'ye göre %50 HerdVault / %30 Builders / %20 Ops böler (rent korunur). Operasyon tabanı zincirde YOK: tutarı authority belirler |
| `update_wallets` | Sadece authority: treasury / builders_fund / ops_wallet günceller |
| `claim_position_fee` | Permissionless. Breakout sonrası kilitli pozisyonun ücretini DAMM'den toplar. Token ve SOL ayrı ayrı `split_lp_fees` ile %60 creator / %20 protokol / %20 `DaoPool`. Creator payı: `creator_eligible()` ise `creator_burn_bps` kısmı yakıma (token: yakılır, SOL: `burn_reserve`'e), kalanı `creator_payout`'a; hak yoksa payın TAMAMI yakıma. Protokol/DAO **token** payı kendi PDA'larının ATA'larında birikir (yakılmaz), SOL payı havuz PDA'larına gider. Dağıtım bakiye farkından hesaplanır. Hesap sayısı fazla: ALT kullan. **Devnet testi yapılmadı** |
| `set_fee_prefs` | Sadece creator: LP ücret payının ödeme cüzdanı ve yakım oranı (0–10 000 bps). Yakım oranı YALNIZCA artırılabilir, cüzdan serbestçe değişir. Varsayılan: creator, yakım 0 |
| `commit_tokens` / `withdraw_commit` | Creator, token'ını program escrow'una kilitler (`commit_auth` PDA'sının ATA'sı). Gelir hakkı: `committed_tokens >= min_commit_tokens` (Config `min_creator_commit_bps`, launch'a snapshot) ve hak kaybı yok. Escrow'dan çekim eşiğin altına indirirse `creator_forfeited` KALICI true olur; sonrasında hem işlem ücreti (`buy`/`sell`) hem LP ücreti payı creator'a değil yakıma gider. Taahhüt dışı token'lar sade holder bakiyesidir. Taahhüt yapılana kadar creator payı da yakıma gider |
| `buyback_burn` | Permissionless. `Launch.burn_reserve`'deki SOL ile DAMM `swap` CPI → gelen token yakılır. min_out zincir üstü spot fiyattan (sqrt_price) hesaplanır, spotun %10 altına inemez; tek işlem havuz SOL rezervinin ≤%1'i. On-run koruması SINIRLIDIR, tam değil. **Devnet testi yapılmadı** |
| `withdraw_dao_tokens` / `withdraw_protocol_tokens` | Sadece authority: havuz PDA'larının token ATA'larından çekim, event'li |
| `withdraw_dao` | Sadece authority: `DaoPool`'dan (kilitli LP ücretinin %20'si, ProtocolPool'dan ayrı) SOL çeker; havuz rent altına inemez, her çekim event'lenir |
| `breakout` | Permissionless. `CurveFull` iken: curve SOL'unu (`accepted_total + vs − vs0`, claim edilmemiş iadeler hariç) + %20 LP token'ını Meteora DAMM v2 havuzuna koyar (payer+creator = `breakout_authority` PDA, pozisyon NFT'sini program tutar), pozisyonu kalıcı kilitler, mint yetkisini iptal eder, artıkları protokol havuzuna yollar. Çağıran `liquidity`/`sqrt_price`'ı zincir dışında hesaplar; havuza giren < planın %99'u ise geri alınır. **Devnet/localnet testi yapılmadı** |
| `update_params` / `propose_authority` / `accept_authority` | Sınırlı parametre güncelleme; iki adımlı yetki devri (multisig → DAO) |
| `create_launch` | Mint (freeze authority YOK) + Launch PDA, Lobby 15/30/60 sn |
| `lobby_deposit` | SOL'ü launch PDA'sında escrow'lar, kullanıcı başına bir `LobbyOrder` |
| `settle_stampede` | Permissionless. Min. katılım tamamsa `Curve` ve Stampede token'larının TAMAMI escrow'a basılır (mint yetkisi claim'e bağlı değil), değilse `Failed` |
| `claim_stampede` | `Curve`, `CurveFull`, `Breakout` ve `Failed` durumlarında açık. Pull modeli: escrow'dan token transferi + cap fazlası iadesi, emri kapatır. Claim ücreti (SOL, kabul edilen tutarın `claim_fee_bps`'i) kullanıcının cüzdanından protokol havuzuna gider; curve rezervine dokunmaz |
| `buy` / `sell` | Bonding curve (x·y=k, sanal rezerv). Başlangıç fiyatı = Stampede clearing price. Satışta token yakılır. `min_*_out` ile slippage koruması |
| fee router (`trade.rs`) | Her işlemde %1 → creator / arayüz / Rekt-back havuzu / protokol havuzu, gerçek SOL hareketiyle, zorunlu |

`curve.rs`: curve matematiği (testli: al-sat kâr ettirmez, her zaman çözücü).
`fees.rs`: %1 ücret bölme (bonded/unbonded), Rekt-back ödeme tavanı, protokol payı 50/30/20.
`breakout.rs`: breakout planlama matematiği (`breakout` kullanır).
`stampede.rs`: tek clearing price matematiği. İkisi de birim testli: `cargo test`.

## Henüz YOK (bilerek)
`distribute_protocol` (protokol havuzu → ops tabanı + 50/30/20), breakout, bond/vesting/escrow/trample, breakout, Rekt-back claim,
CTO, XP (indexer'da, kontratta değil), Metaplex metadata, create fee.

## Curve parametreleri (v0 varsayımı, devnet'te ayarlanır)
Arz: %10 Stampede, %70 curve, %20 breakout LP (mint edilmez). Sanal token rezervi = 2 × curve arzı,
yani curve boyunca fiyat 4× artar. Launch büyüklüğünü belirleyen düğme `stampede_cap_lamports`.

## Bilinen açık noktalar
- Curve dolunca (`CurveFull`) işlem durur ve SOL breakout'u bekler. Breakout yazılana kadar
  mainnet'e çıkma: fonlar takılı kalır.
- Alıcı cüzdanı (creator/arayüz) rent-muafiyetinin altında kalıyorsa o ücret protokol havuzuna yönlendirilir.
- `initialize_config` artık upgrade authority'ye kilitli (ön-çalıştırma/front-run riski kapandı).
- RektPool SOL biriktirir ama çıkış yolu yok (v1 tasarımı, dağıtım kuralları v2'de). Mainnet öncesi karar gerekli.
- `breakout` / `buyback_burn`: CPI'dan önce doğrudan lamport hareketi var; zincirde `UnbalancedInstruction`
  ile düşme ihtimali yüksek ve DAMM havuzu önceden açılarak breakout kilitlenebilir. Ayrıntı: `SECURITY_REVIEW.md`.
- Lobby'de son saniye avantajı var (süre sabit). Çözüm: rastgele bitiş / candle auction.
- Cüzdan başına limit yok; Sybil'e karşı asıl koruma toplam cap + orantılı iade.
- Stampede token'ları claim'de mint edilir; curve eklenince toplam mint ≤ TOTAL_SUPPLY
  garanti edilmeli.

## Derleme ve testler
`cargo test` (Rust 1.97, anchor-lang 0.30.1, `Cargo.lock` sabitli) birim testleri + `tests/` altındaki
instruction seviyesi entegrasyon testlerini çalıştırır. Entegrasyon testleri `solana-program-test` ile
programı NATIVE çalıştırır (SBF değil); SPL Token/ATA paketli BPF sürümleridir. Meteora DAMM v2
yüklü değildir: `breakout`, `claim_position_fee`, `buyback_burn` CPI'ları test EDİLMEZ; breakout sonrası
durum gerektiğinde simüle edilir (`tests/common::simulate_breakout`).

`anchor build` (SBF), IDL üretimi ve devnet/localnet testi YAPILMADI (araç zinciri bu ortamda indirilemedi).
