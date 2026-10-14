//! LiteSVM ortami: gercek `herdcall.so` (anchor build) + mainnet'ten dump edilen gercek
//! Meteora DAMM v2 (`cp_amm.so`). Dosya yollari ortam degiskenlerinden gelir (CI ayarlar).
use litesvm::LiteSVM;
use solana_address::Address;

pub const HERDCALL_ID: Address = Address::from_str_const("8XTPwS47mzBUPWP1GhGH7M5dMBwA4pJ8eCJQqffzk4yd");
pub const DAMM_ID: Address = Address::from_str_const("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

pub fn so_path(var: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| panic!("{var} is not set (CI sets it; see .github/workflows/herdcall-program.yml)"))
}

pub fn load_programs(svm: &mut LiteSVM) {
    svm.add_program_from_file(HERDCALL_ID, so_path("HERDCALL_SO")).expect("load herdcall.so");
    svm.add_program_from_file(DAMM_ID, so_path("DAMM_SO")).expect("load cp_amm.so");
}
