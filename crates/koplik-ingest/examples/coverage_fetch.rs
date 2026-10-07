//! Source discovery/fixture capture through the same governed transport and snapshot store.
//! Usage: cargo run -p koplik-ingest --example coverage_fetch -- ID URL LICENCE OUTPUT
use koplik_ingest::{
    http::UreqClient,
    polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper},
    source::{SourceSpec, fetch_to_store},
    store::SnapshotStore,
};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(a.len(), 4, "ID URL LICENCE OUTPUT");
    let spec = SourceSpec {
        source_id: a[0].clone(),
        url: a[1].clone(),
        licence_id: a[2].clone(),
    };
    let cfg = PoliteConfig::live_from_env()?;
    let store = SnapshotStore::open("data/snapshots")?;
    let mut fetcher = PoliteFetcher::new(
        UreqClient::new(Duration::from_secs(90), 64 * 1024 * 1024),
        SystemTimekeeper::new(),
        cfg,
    );
    let (r, _) = fetch_to_store(&mut fetcher, &store, &spec)?;
    // Never overwrite captured fixtures.
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&a[3])?
        .write_all(&store.get_verified(&r.sha256)?)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(format!("{}.retrieval.json", a[3]))?
        .write_all(serde_json::to_string(&r)?.as_bytes())?;
    println!("{} {} bytes", r.sha256, r.bytes);
    Ok(())
}
