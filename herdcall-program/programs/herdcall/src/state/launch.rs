use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum LaunchState {
    /// Emirler toplanir, islem yok (snipe edilecek bir sey yok)
    Lobby,
    /// Stampede tek clearing price ile uzlasti; token'lar claim edilebilir.
    /// (v0'da curve henuz yok; sonraki adim)
    Curve,
    /// Min. katilim saglanamadi: herkes tam iade alir
    Failed,
    /// Curve tamamen satildi: islem durur, breakout bekler (v0'da breakout yok)
    CurveFull,
    /// Reserved: DEX'e tasindi, mint yetkisi kaldirildi (v1+)
    Breakout,
}

#[account]
#[derive(InitSpace)]
pub struct Launch {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub state: LaunchState,
    pub bonded: bool,
    pub lobby_start: i64,
    pub lobby_end: i64,
    /// Config'ten snapshot
    pub min_lamports: u64,
    pub cap_lamports: u64,
    /// Config'ten snapshot: claim ucreti (bps, kabul edilen SOL uzerinden)
    pub claim_fee_bps: u16,
    /// Lobby'ye yatirilan toplam
    pub total_deposited: u64,
    pub depositors: u32,
    /// settle sonrasi: min(total_deposited, cap)
    pub accepted_total: u64,
    /// Stampede'de dagitilacak token miktari (sabit)
    pub stampede_tokens: u64,
    /// Curve sanal rezervleri (x*y=k). Baslangic fiyati = clearing price.
    pub virtual_sol: u64,
    pub virtual_token: u64,
    /// virtual_token bu degere inince curve dolmustur
    pub virtual_token_floor: u64,
    /// Solvency kontrolu icin baslangic sanal SOL'u
    pub initial_virtual_sol: u64,
    /// Breakout sonrasi: DAMM v2 havuzu, pozisyonu ve pozisyon NFT mint'i
    /// (NFT'yi breakout_authority PDA'si tutar; LP kalici kilitli).
    pub dex_pool: Pubkey,
    pub dex_position: Pubkey,
    pub position_nft_mint: Pubkey,
    /// Creator'in LP ucret payinin (%60) gidecegi cuzdan. Baslangicta creator'in kendisi;
    /// `set_fee_prefs` ile degistirilir. Asla default pubkey olamaz.
    pub creator_payout: Pubkey,
    /// Creator payindaki TOKEN ucretinin yakilacak kismi (bps, 0..=10_000). SOL tarafina uygulanmaz.
    pub creator_burn_bps: u16,
    /// Creator taahhudu: `commit_tokens` ile program escrow'una kilitlenen token. Gelir hakki icin
    /// `committed_tokens >= min_commit_tokens` olmali.
    pub committed_tokens: u64,
    /// Config'ten snapshot: gereken minimum taahhut (token). 0 => zorunluluk yok.
    pub min_commit_tokens: u64,
    /// Taahhut esigin altina inince KALICI olarak true olur; creator geliri sonsuza dek yakima doner.
    pub creator_forfeited: bool,
    /// Launch PDA'sinda tutulan, buyback-burn icin ayrilmis SOL (lamport). Curve rezervi DEGIL.
    pub burn_reserve: u64,
    /// Buyback ile yakilan toplam token (seffaflik)
    pub total_bought_back: u64,
    pub bump: u8,
}

/// Kullanici basina bir emir. Claim'de kapanir (rent kullaniciya doner).
#[account]
#[derive(InitSpace)]
pub struct LobbyOrder {
    pub launch: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub bump: u8,
}

impl Launch {
    /// Creator geliri alabilir mi? Taahhut tamam ve hak kaybedilmemis olmali.
    pub fn creator_eligible(&self) -> bool {
        !self.creator_forfeited && self.committed_tokens >= self.min_commit_tokens
    }

    /// Cap asildiginda her emrin `accepted` payi asagi yuvarlanir; toplam kabul edilen SOL
    /// `accepted_total`'dan en fazla `depositors` lamport az olur ve fark IADE olarak odenir.
    /// Bu tampon curve rezervinden sayilmaz (satis ve breakout dokunamaz), boylece son
    /// claim edenin iadesi eksik kalmaz.
    pub fn refund_rounding_buffer(&self) -> u64 {
        if self.total_deposited > self.cap_lamports {
            self.depositors as u64
        } else {
            0
        }
    }
}
