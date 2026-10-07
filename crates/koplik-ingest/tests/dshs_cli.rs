//! Every live DSHS/Census fetch command refuses without a verified contact, before it opens
//! the store or makes a request (offline: nothing here reaches the network).

use std::process::Command;

#[test]
fn dshs_and_census_fetches_refuse_without_a_contact_and_touch_nothing() {
    for source in [
        "dshs-live",
        "dshs-wayback",
        "dshs-reports",
        "census-counties",
    ] {
        for contact in [None, Some(""), Some("  ")] {
            let dir = tempfile::tempdir().unwrap();
            let store = dir.path().join("snapshots");
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"));
            cmd.args(["fetch", source, "--store"]).arg(&store);
            match contact {
                Some(c) => cmd.env("KOPLIK_CONTACT", c),
                None => cmd.env_remove("KOPLIK_CONTACT"),
            };
            let out = cmd.output().unwrap();
            assert!(!out.status.success(), "{source} {contact:?}");
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(err.contains("KOPLIK_CONTACT"), "{source}: {err}");
            assert!(
                !store.exists(),
                "{source}: store created before the contact check"
            );
        }
    }
}
