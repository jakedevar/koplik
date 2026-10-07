//! `koplik-pipeline`: one reproducible path from primary sources to the static site's data.
//!
//! ```text
//! koplik-pipeline <ingest|validate|infer|forecast|build|all> [--from-fixtures]
//!                 [--store DIR] [--work DIR] [--out DIR] [--fixtures DIR]
//! ```
//!
//! `ingest` is the only stage that uses the network. It identifies the client with the contact
//! `koplik_ingest::polite::contact_from_env` resolves (`KOPLIK_CONTACT`, or the operator's
//! default when unset); a blank `KOPLIK_CONTACT` is an explicit opt-out and `ingest` refuses
//! before creating anything. `--from-fixtures` makes
//! `ingest` seed a separate store from the committed real-byte snapshots under
//! `data/fixtures/` instead; the two modes never mix, and nothing falls back from one to the
//! other. Every other stage is offline. See the library docs for what each stage writes.

use std::path::PathBuf;
use std::process::ExitCode;

use koplik_pipeline::{
    Config, DEFAULT_FIXTURES, DEFAULT_OUT, DEFAULT_WORK, Mode, PipelineError, Stage,
};

const USAGE: &str = "usage:
  koplik-pipeline <ingest|validate|infer|forecast|build|all> [--from-fixtures]
                  [--store DIR] [--work DIR] [--out DIR] [--fixtures DIR]

  --from-fixtures  ingest seeds the store from data/fixtures/ (offline); the store defaults
                   to <work>/fixture-snapshots so live and fixture snapshots never mix
  --store DIR      snapshot store (default data/snapshots, or <work>/fixture-snapshots)
  --work DIR       stage outputs and manifests (default data/pipeline)
  --out DIR        web artifacts (default web/public/data)
  --fixtures DIR   fixture root for --from-fixtures (default data/fixtures)";

struct Flags(Vec<String>);

impl Flags {
    fn take(&mut self, name: &str) -> Result<Option<String>, PipelineError> {
        match self.0.iter().position(|a| a == name) {
            None => Ok(None),
            Some(i) => {
                if i + 1 >= self.0.len() {
                    return Err(PipelineError::Invalid(format!("{name} needs a value")));
                }
                let v = self.0.remove(i + 1);
                self.0.remove(i);
                Ok(Some(v))
            }
        }
    }
    fn flag(&mut self, name: &str) -> bool {
        match self.0.iter().position(|a| a == name) {
            None => false,
            Some(i) => {
                self.0.remove(i);
                true
            }
        }
    }
    fn done(&self) -> Result<(), PipelineError> {
        match self.0.first() {
            None => Ok(()),
            Some(a) => Err(PipelineError::Invalid(format!("unexpected argument {a:?}"))),
        }
    }
}

fn run(args: Vec<String>) -> Result<(), PipelineError> {
    let mut args = args.into_iter();
    let cmd = args
        .next()
        .ok_or_else(|| PipelineError::Invalid("missing stage".into()))?;
    let stages: Vec<Stage> = if cmd == "all" {
        Stage::ALL.to_vec()
    } else {
        vec![
            Stage::parse(&cmd)
                .ok_or_else(|| PipelineError::Invalid(format!("unknown stage {cmd:?}")))?,
        ]
    };
    let mut flags = Flags(args.collect());
    let mode = if flags.flag("--from-fixtures") {
        Mode::Fixtures
    } else {
        Mode::Live
    };
    let work = PathBuf::from(
        flags
            .take("--work")?
            .unwrap_or_else(|| DEFAULT_WORK.to_owned()),
    );
    let store = match flags.take("--store")? {
        Some(s) => PathBuf::from(s),
        None => Config::default_store(mode, &work),
    };
    let out = PathBuf::from(
        flags
            .take("--out")?
            .unwrap_or_else(|| DEFAULT_OUT.to_owned()),
    );
    let fixtures = PathBuf::from(
        flags
            .take("--fixtures")?
            .unwrap_or_else(|| DEFAULT_FIXTURES.to_owned()),
    );
    flags.done()?;
    let config = Config {
        mode,
        store,
        work,
        out,
        fixtures,
    };
    for stage in stages {
        let manifest = koplik_pipeline::run_stage(stage, &config)?;
        eprintln!("{}", manifest.summary());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            if matches!(e, PipelineError::Invalid(_)) {
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}
