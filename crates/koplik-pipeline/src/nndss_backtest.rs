//! The pseudo-real-time backtest of the published forecast on the CDC NNDSS state series
//! (#1503). It runs `koplik_epi::backtest::truncated` on exactly the rows the pipeline publishes
//! as `weekly-cases` (the same `koplik_ingest::cdc::parse_weekly_cases` on the committed
//! real-byte snapshot), with exactly the published configuration ([`forecast_config`], the
//! pre-registered defaults, seed [`FORECAST_SEED`], the origin rule's provisional weeks).
//!
//! The protocol, its floors and what they mean are pre-registered in
//! `thoughts/shared/research/backtest-cdc-states.md`, committed before any score on these series
//! was computed. The report this writes is committed as `data/reports/backtest/cdc-states.json`
//! and read back by the forecast stage ([`crate::forecast_stage`]) to attach each series' measured
//! skill (or its absence) to the published forecast.
//!
//! Real-time honesty: there is one retrieval of this source and CDC publishes no revision
//! history, so the backtest is **pseudo-real-time (revised counts truncated at each forecast
//! date)**, never real-time. [`PROTOCOL`] and [`SCOPE`] say so in the report itself.

use koplik_epi::backtest::truncated::{
    SkillFloor, TruncatedConfig, TruncatedReport, run_truncated_backtest,
};
use koplik_ingest::cdc;
use koplik_ingest::store::{Retrieval, sha256_of};
use serde::Serialize;

use crate::forecast_stage::{FORECAST_SEED, forecast_config};
use crate::{PipelineError, Result, rt_config};

/// The committed report, relative to the reports root (`Config::reports`).
pub const REPORT_REL: &str = "backtest/cdc-states.json";
/// The same report as the companion cites it.
pub const REPORT_PATH: &str = "data/reports/backtest/cdc-states.json";
/// The committed real-byte snapshot the backtest reads.
pub const FIXTURE_PATH: &str = "data/fixtures/cdc/nndss-measles-weekly.json";
/// The pre-registration, committed before any score was computed.
pub const PRE_REGISTRATION: &str = "thoughts/shared/research/backtest-cdc-states.md";

/// How the information cutoff was respected, in words. The label in quotes is the spec's (E5).
pub const PROTOCOL: &str = "pseudo-real-time (revised counts truncated at each forecast date): CDC publishes no revision history and one retrieval of the NNDSS series is held, so this is not a real-time backtest. At each forecast the method is given only the weeks up to its origin week (the latest week less the two provisional weeks, the rule the site applies) from the one retrieved series, and nothing after; it is scored against the same retrieved series, except that the two newest weeks of that series, which are themselves still provisional, are never used as truth. If CDC rewrote earlier weeks when it republished, the retrieved series carries those later revisions and a forecaster at the time saw different counts.";

/// What was scored, in a sentence (the first clause names the series; the rest says what is not
/// covered).
pub const SCOPE: &str = "the CDC NNDSS weekly measles counts of each state, territory and DC (confirmed or unknown-status cases, by CDC report week), scored wherever the forecast's own minimum-count rule held; not the Texas DSHS county series, not confirmed cases only, not symptom-onset incidence, and not real-time by report vintage";

/// Where the report's inputs came from, so every score traces to a snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Input {
    pub source_id: String,
    pub url: String,
    pub retrieved_at: String,
    /// SHA-256 of the committed snapshot's bytes.
    pub sha256: String,
    pub bytes: u64,
    /// Repository path of that snapshot.
    pub path: &'static str,
    /// Weekly rows the snapshot parses into (every state-level geography, every week).
    pub rows: u64,
}

/// The committed report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub input: Input,
    pub protocol: &'static str,
    pub scope: &'static str,
    pub pre_registration: &'static str,
    pub primary: TruncatedReport,
}

/// The backtest configuration: the published forecast's, unchanged, with the pre-registered
/// floors.
pub fn backtest_config() -> TruncatedConfig {
    TruncatedConfig {
        forecast: forecast_config(),
        seed: FORECAST_SEED,
        provisional_weeks: rt_config().provisional_weeks,
        floor: SkillFloor::PRE_REGISTERED,
    }
}

/// Run the backtest on a snapshot's bytes with its retrieval record. The bytes are re-hashed
/// against the record first: a report is only ever made from the snapshot it cites.
pub fn run(bytes: &[u8], retrieval: &Retrieval) -> Result<Report> {
    if sha256_of(bytes) != retrieval.sha256 || bytes.len() as u64 != retrieval.bytes {
        return Err(PipelineError::Data(format!(
            "the snapshot bytes do not match their retrieval record (sha256 {})",
            retrieval.sha256
        )));
    }
    let rows = cdc::parse_weekly_cases(bytes, retrieval)?;
    let primary = run_truncated_backtest(&rows, &backtest_config())
        .map_err(|e| PipelineError::Data(format!("backtest: {e}")))?;
    Ok(Report {
        input: Input {
            source_id: retrieval.source_id.clone(),
            url: retrieval.url.clone(),
            retrieved_at: retrieval.retrieved_at.to_rfc3339(),
            sha256: retrieval.sha256.to_string(),
            bytes: retrieval.bytes,
            path: FIXTURE_PATH,
            rows: rows.len() as u64,
        },
        protocol: PROTOCOL,
        scope: SCOPE,
        pre_registration: PRE_REGISTRATION,
        primary,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fixture() -> (Vec<u8>, Retrieval) {
        let dir = repo().join("data/fixtures/cdc");
        let bytes = fs::read(dir.join("nndss-measles-weekly.json")).unwrap();
        let record = serde_json::from_slice(
            &fs::read(dir.join("nndss-measles-weekly.retrieval.json")).unwrap(),
        )
        .unwrap();
        (bytes, record)
    }

    #[test]
    fn the_backtest_runs_with_the_published_configuration_and_the_pre_registered_floors() {
        let cfg = backtest_config();
        assert_eq!(cfg.forecast, forecast_config());
        assert_eq!(cfg.seed, FORECAST_SEED);
        assert_eq!(cfg.provisional_weeks, 2);
        assert_eq!(cfg.floor, SkillFloor::PRE_REGISTERED);
        assert_eq!(
            (cfg.floor.min_targets, cfg.floor.min_origin_weeks),
            (40, 10)
        );
        assert!(
            PROTOCOL
                .starts_with("pseudo-real-time (revised counts truncated at each forecast date)")
        );
    }

    /// The committed report is what the code produces: Texas, re-run here from the committed
    /// snapshot, is the committed report's Texas series to the bit (every series draws from its own
    /// geography's seeds, so one series can be re-run alone). The full re-run is the example, and
    /// is deterministic.
    #[test]
    fn the_committed_report_matches_a_fresh_run_of_one_series() {
        let (bytes, record) = fixture();
        let committed: serde_json::Value =
            serde_json::from_slice(&fs::read(repo().join(REPORT_PATH)).unwrap()).unwrap();
        assert_eq!(committed["input"]["sha256"], record.sha256.to_string());
        assert_eq!(committed["protocol"], PROTOCOL);
        let texas: koplik_contracts::v3::GeoId = "48".parse().unwrap();
        let rows: Vec<_> = cdc::parse_weekly_cases(&bytes, &record)
            .unwrap()
            .into_iter()
            .filter(|r| r.geography == texas)
            .collect();
        // The last week of the whole series, not of Texas alone, bounds the origins and the truth.
        assert_eq!(rows.len(), 91);
        let fresh = run_truncated_backtest(&rows, &backtest_config()).unwrap();
        // Through text and back, as the committed report was read: serde_json's default float
        // parser is not correctly rounded, so comparing a computed value with a parsed one can
        // differ in the last digit while the printed bytes are identical.
        let fresh: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&fresh.series[0]).unwrap()).unwrap();
        let committed = committed["primary"]["series"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["geography"] == "48")
            .unwrap();
        for (key, want) in committed.as_object().unwrap() {
            assert_eq!(&fresh[key], want, "{key} differs");
        }
        assert_eq!(&fresh, committed);
    }

    #[test]
    fn a_report_is_only_made_from_the_snapshot_it_cites() {
        let (mut bytes, record) = fixture();
        bytes.push(b' ');
        let e = run(&bytes, &record).unwrap_err().to_string();
        assert!(e.contains("do not match their retrieval record"), "{e}");
    }
}
