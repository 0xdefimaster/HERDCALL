//! Metaplex token metadata, created by `create_launch` through the real Token Metadata program.
mod common;

use common::*;

fn env() -> Env {
    let mut env = Env::new();
    env.initialize(example_params()).unwrap();
    env
}

/// Metaplex pads strings with zero bytes to their maximum length.
fn s(v: &str) -> &str {
    v.trim_end_matches('\0')
}

#[test]
fn every_launch_has_immutable_metadata() {
    let mut env = env();
    let uri = "https://herdcall.fun/t/pepe.json";
    let k = env.create_launch_uri(true, 60, "PEPE", "  Pepe Coin ", uri).unwrap();
    let m = env.metadata(&k).expect("metadata account");
    assert_eq!(m.mint, k.mint);
    assert_eq!((s(&m.name), s(&m.symbol), s(&m.uri)), ("Pepe Coin", "PEPE", uri));
    assert!(!m.is_mutable, "nobody can change name, symbol or image later");
    assert_eq!(m.update_authority, k.launch, "no wallet holds the update authority");
    assert_eq!(m.seller_fee_basis_points, 0);
    assert!(m.creators.is_none() && m.collection.is_none());
    // The mint authority is gone after the metadata was written.
    assert_eq!(env.mint_info(&k.mint), (false, example_params().total_supply));
}

#[test]
fn uri_is_limited_to_200_bytes() {
    let mut env = env();
    let long = format!("https://herdcall.fun/{}", "x".repeat(200 - 21));
    assert_eq!(long.len(), 200);
    let k = env.create_launch_uri(true, 60, "OK", "Ok", &long).unwrap();
    assert_eq!(s(&env.metadata(&k).unwrap().uri), long);
    let too_long = format!("{long}x");
    let r = env.create_launch_uri(true, 60, "NO", "No", &too_long).map(|_| ());
    assert!(r.unwrap_err().contains("InvalidUri"));
}

#[test]
fn metadata_address_must_be_the_mints() {
    let mut env = env();
    let a = env.create_launch(true, 60).unwrap();
    // Building a launch with another mint's metadata address fails on the seeds check.
    let r = env.create_launch_with(|keys| keys.metadata = a.metadata);
    assert!(r.map(|_| ()).unwrap_err().contains("ConstraintSeeds"));
}
