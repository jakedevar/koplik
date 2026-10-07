use std::io::Write;

use chrono::Utc;
use koplik_ingest::http::{HttpClient, UreqClient};
use koplik_ingest::store::{RetrievalMeta, SnapshotStore};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Refuse to replace historical fixtures. A newer capture must use new output names.
    for path in [
        "data/fixtures/census/robots.txt",
        "data/fixtures/census/robots.retrieval.json",
    ] {
        if std::path::Path::new(path).exists() {
            return Err(format!(
                "historical fixture already exists: {path}; choose new output names"
            )
            .into());
        }
    }
    // /robots.txt is implicitly allowed under RFC 9309 §2.2.2. Diagnostic capture only.
    let c = UreqClient::new(std::time::Duration::from_secs(30), 1024 * 1024);
    let url = "https://www2.census.gov/robots.txt";
    let r = c.get(
        url,
        &koplik_ingest::polite::live_user_agent(
            koplik_ingest::polite::contact_from_env().as_deref(),
        )?,
    )?;
    assert_eq!(r.status, 200);
    let store = SnapshotStore::open("data/snapshots")?;
    let (retrieval, _) = store.record(
        RetrievalMeta {
            source_id: "census-robots-diagnostic".into(),
            url: url.into(),
            retrieved_at: Utc::now(),
            licence_id: "us-census-public-domain".into(),
            http_status: r.status,
            content_type: r.content_type,
        },
        &r.body,
    )?;
    std::fs::create_dir_all("data/fixtures/census")?;
    std::fs::File::create_new("data/fixtures/census/robots.txt")?.write_all(&r.body)?;
    std::fs::File::create_new("data/fixtures/census/robots.retrieval.json")?
        .write_all(&serde_json::to_vec_pretty(&retrieval)?)?;
    println!("{}", serde_json::to_string(&retrieval)?);
    Ok(())
}
