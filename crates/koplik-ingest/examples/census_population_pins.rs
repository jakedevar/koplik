//! Initial pin discovery only, following #1350's documented process. No bytes are stored,
//! no pins are accepted automatically, and each named URL is requested once in this run.
//! The empty-body digest is a deliberate nonmatching guard, never a claimed source hash.
use koplik_ingest::{
    census_files::NamedFileAllowlist,
    census_population,
    error::IngestError,
    http::UreqClient,
    polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper, contact_from_env},
    store::sha256_of,
};
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = PoliteConfig::live(contact_from_env().as_deref())?;
    let sources = [
        "census-state-population",
        "census-county-population",
        "census-texas-counties",
        "census-states",
    ];
    let specs = sources
        .map(census_population::source_spec)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let manifest = NamedFileAllowlist::new(specs.iter().map(|s| (s.url.clone(), sha256_of(b""))))?;
    let mut fetcher = PoliteFetcher::new(
        UreqClient::new(Duration::from_secs(120), 64 * 1024 * 1024),
        SystemTimekeeper::new(),
        cfg,
    );
    for spec in specs {
        match fetcher.fetch_census_file(&spec.url, &manifest) {
            Err(IngestError::PinMismatch { url, actual, .. }) => {
                println!("{}", serde_json::json!({"url":url,"sha256":actual}))
            }
            Err(e) => return Err(e.into()),
            Ok(_) => return Err("unexpected empty source response".into()),
        }
    }
    Ok(())
}
