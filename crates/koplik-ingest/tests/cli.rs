//! The CLI refuses live fetching without a contact, before opening the store or sending a
//! request. These tests never reach the network (the refusal comes first).

use std::process::Command;

fn run(contact: Option<&str>, store: &std::path::Path) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"));
    cmd.args(["fetch", "cdc-cases", "--store"]).arg(store);
    cmd.env_remove("KOPLIK_CONTACT");
    if let Some(c) = contact {
        cmd.env("KOPLIK_CONTACT", c);
    }
    cmd.output().unwrap()
}

#[test]
fn fetch_refuses_without_a_contact_and_touches_nothing() {
    for contact in [Some(""), Some("  "), Some("\t")] {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("snapshots");
        let out = run(contact, &store);
        assert!(!out.status.success(), "{contact:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("KOPLIK_CONTACT"), "{err}");
        assert!(
            !store.exists(),
            "store must not be created before the contact check"
        );
    }
}

#[test]
fn parse_and_list_work_offline_without_a_contact() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("snapshots");
    let out = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"))
        .args(["list", "--store"])
        .arg(&store)
        .env_remove("KOPLIK_CONTACT")
        .output()
        .unwrap();
    assert!(out.status.success());
    // `parse` with no snapshot fails for that reason, not for a missing contact.
    let out = Command::new(env!("CARGO_BIN_EXE_koplik-ingest"))
        .args(["parse", "cdc-cases", "--store"])
        .arg(&store)
        .env_remove("KOPLIK_CONTACT")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(err.contains("no snapshot"), "{err}");
}
