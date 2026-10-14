//! Protokol sabitleri. Kaynak: document/HERDCALL.md (§3, §5, §7, §9).
//! Ayarlanabilir olanlar `Config` hesabinda tutulur (authority multisig/DAO);
//! burada yalnizca kontrata ZORUNLU gomulen kurallar var.

use anchor_lang::prelude::*;

pub const CONFIG_SEED: &[u8] = b"config";
pub const REKT_POOL_SEED: &[u8] = b"rekt_pool";
pub const HERD_VAULT_SEED: &[u8] = b"herd_vault";
pub const DAO_POOL_SEED: &[u8] = b"dao_pool";
pub const LAUNCH_SEED: &[u8] = b"launch";
pub const ORDER_SEED: &[u8] = b"order";

// ---- Token ----
pub const TOKEN_DECIMALS: u8 = 6;
/// 1B token * 10^6
pub const TOTAL_SUPPLY: u64 = 1_000_000_000 * 1_000_000;
/// Stampede'e ayrilan pay: arzin %10'u (v0 varsayimi, devnet'te ayarlanir)
pub const STAMPEDE_SUPPLY_BPS: u64 = 1_000;

// ---- Lobby ----
pub const LOBBY_DURATIONS: [i64; 3] = [15, 30, 60]; // saniye (§3)
pub const MIN_DEPOSIT_LAMPORTS: u64 = 10_000_000; // 0.01 SOL

// ---- Ucret modeli (islem hacminin baz puani, 1 bps = %0.01) §5 ----
pub const FEE_TOTAL_BPS: u64 = 100; // %1
pub const BPS_DENOM: u64 = 10_000;

// Bonded: creator %0.30, arayuz %0.10, rekt %0.15, protokol %0.45
pub const BONDED_CREATOR_BPS: u64 = 30;
pub const BONDED_UI_BPS: u64 = 10;
pub const BONDED_REKT_BPS: u64 = 15;
pub const BONDED_PROTOCOL_BPS: u64 = 45;

// Unbonded: creator %0.10, arayuz %0.10, rekt %0.35, protokol %0.45
pub const UNBONDED_CREATOR_BPS: u64 = 10;
pub const UNBONDED_UI_BPS: u64 = 10;
pub const UNBONDED_REKT_BPS: u64 = 35;
pub const UNBONDED_PROTOCOL_BPS: u64 = 45;

// ---- Protokol gelirinin dagitimi §7 (operasyon tabani sonrasi) ----
pub const VAULT_SHARE_BPS: u64 = 5_000; // Herd Vault %50
pub const BUILDERS_SHARE_BPS: u64 = 3_000; // Builders Fund %30
pub const OPS_SHARE_BPS: u64 = 2_000; // Ops/ekip %20

// ---- Stampede claim ucreti (SOL, Config'te ayarlanir; yonetisim bu siniri asamaz) ----
/// Kabul edilen SOL'un en fazla %3'u (bps). Launch olusturulurken snapshot'lanir.
pub const CLAIM_FEE_BPS_MAX: u16 = 300;

// ---- Breakout sonrasi kilitli LP pozisyonunun ucret geliri (bps) ----
pub const LP_FEE_CREATOR_BPS: u64 = 6_000; // %60 creator
pub const LP_FEE_PROTOCOL_BPS: u64 = 2_000; // %20 protokol havuzu
pub const LP_FEE_DAO_BPS: u64 = 2_000; // %20 DAO karari (DaoPool'da birikir; kullanimi DAO belirler)
const _: () = assert!(LP_FEE_CREATOR_BPS + LP_FEE_PROTOCOL_BPS + LP_FEE_DAO_BPS == BPS_DENOM);

// ---- Breakout (Meteora DAMM v2 / cp-amm) ----
// Program ID ve pool authority: resmi IDL (MeteoraAg/damm-v2-sdk, cp_amm.json v0.2.5).
pub const DAMM_PROGRAM_ID: Pubkey = anchor_lang::solana_program::pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");
pub const DAMM_POOL_AUTHORITY: Pubkey = anchor_lang::solana_program::pubkey!("HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC");
pub const TOKEN_2022_PROGRAM_ID: Pubkey = anchor_lang::solana_program::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const BREAKOUT_AUTH_SEED: &[u8] = b"breakout_auth";
/// Havuza fiilen girenin, planlananin en az bu kadari (bps) olmali; aksi halde islem geri alinir.
/// Cagiran `liquidity`/`sqrt_price`'i zincir disinda hesaplar; yanlis/kotu niyetli deger fonlari
/// havuz disinda birakamaz. (%1 tolerans yuvarlama icin yeterli; ayarlanabilir sabit)
pub const BREAKOUT_MIN_FILL_BPS: u64 = 9_900;
/// Havuz acma maliyeti (rent vb.) butcesi sinirlari (lamport). NOT: bunlar tahmindir,
/// devnet'te gercek maliyet olculup ayarlanmali.
pub const BREAKOUT_COST_MIN: u64 = 5_000_000; // 0.005 SOL
pub const BREAKOUT_COST_MAX: u64 = 200_000_000; // 0.2 SOL

// ---- Rekt-back (simulasyon: havuz ~%7.7 secim orani tasir, yarisi hedeflendi) ----
/// Config'te ayarlanabilir; yonetisim bu sinirlar disina cikamaz.
pub const REKT_SELECT_BPS_MIN: u16 = 200; // %2  (1/50)
pub const REKT_SELECT_BPS_MAX: u16 = 600; // %6
/// Varsayilan: %3.85 (~1/26) = havuzun tasiyabilecegi oranin yarisi
pub const REKT_SELECT_BPS_DEFAULT: u16 = 385;
/// Bir odeme turunda toplam odeme, havuz bakiyesinin (rezerv ustu) en fazla %50'si
pub const REKT_PAYOUT_CAP_BPS_MIN: u16 = 1_000;
pub const REKT_PAYOUT_CAP_BPS_MAX: u16 = 5_000;

// ---- Curve (v0 varsayimlari; devnet'te ayarlanir) ----
pub const PROTOCOL_POOL_SEED: &[u8] = b"protocol_pool";
/// Curve'de satilan arz payi: %70. Stampede %10, kalan %20 breakout LP'si icin ayrilir (mint edilmez).
pub const CURVE_SUPPLY_BPS: u64 = 7_000;
pub const LP_RESERVE_BPS: u64 = 2_000;
/// Sanal token rezervi = CURVE_VT_MULT * curve_tokens. 2 => curve boyunca fiyat 4x artar
/// (fiyat ~ (vt0/vt)^2). Baslangic fiyati Stampede clearing price'ina esittir.
pub const CURVE_VT_MULT: u64 = 2;
/// Dust / spam alt siniri (0.001 SOL)
pub const MIN_TRADE_LAMPORTS: u64 = 1_000_000;

// ---- Creator taahhudu (commit) ve yakim ----
pub const COMMIT_AUTH_SEED: &[u8] = b"commit_auth";
/// Config'teki minimum taahhut ust siniri: toplam arzin %10'u (bps)
pub const CREATOR_COMMIT_BPS_MAX: u16 = 1_000;
/// Buyback-burn: tek islemde en az / en fazla (havuzun SOL rezervinin bps'i) ve spot fiyattan sapma toleransi.
/// Tolerans havuz ucretini + etkiyi kapsamali; asiri dusuk olursa swap geri alinir (fon kaybi olmaz).
pub const BUYBACK_MIN_LAMPORTS: u64 = 10_000_000; // 0.01 SOL
pub const BUYBACK_MAX_POOL_BPS: u64 = 100; // havuz SOL rezervinin %1'i
pub const BUYBACK_TOLERANCE_BPS: u64 = 1_000; // spot beklentinin %10 altina inemez
/// DAMM v2 `swap` diskriminatoru ve Pool hesabi ofsetleri (IDL cp_amm v0.2.5'ten hesaplandi)
pub const POOL_TOKEN_A_MINT_OFFSET: usize = 168;
pub const POOL_TOKEN_B_MINT_OFFSET: usize = 200;
pub const POOL_TOKEN_A_VAULT_OFFSET: usize = 232;
pub const POOL_TOKEN_B_VAULT_OFFSET: usize = 264;
pub const POOL_SQRT_PRICE_OFFSET: usize = 456;
