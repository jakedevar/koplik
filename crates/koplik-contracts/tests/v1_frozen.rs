//! Released contract versions are immutable: the committed v1 JSON Schema files must stay
//! byte-for-byte what v1 released with. A shape change is a new version (`v2`, ...), never an
//! edit here. (These digests are the v1 files as released, verified against `git` history.)

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

const V1_RELEASED: &[(&str, &str)] = &[
    (
        "CountyFips.schema.json",
        "7484018f3820cd01365ae3f14ed24c0da733ca8e1bc43cab513942c88ecbbf35",
    ),
    (
        "Forecast.schema.json",
        "60215be6b1be3e726eda94a98be940905955aeb843b6a20706e8256d4d8f78bf",
    ),
    (
        "Geography.schema.json",
        "9f180fef1a4eb7accc447b17597f6f0ed9bc574933aa1510678092cb9c4ce93a",
    ),
    (
        "GeoId.schema.json",
        "521c0b0c4f81015c8324b1f00ad858a65b6ec35cb68d54acef21de774856db1b",
    ),
    (
        "KindergartenMmrCoverage.schema.json",
        "4fcbcae30667a5b7f2659b20602a89a155ec38a0d371c8f10e1edea4c858fdb5",
    ),
    (
        "MmwrWeek.schema.json",
        "35334d124ebe07b5024aa5fa1dbae6b8bedf3b3deac10965c6b5c0fea011173f",
    ),
    (
        "Population.schema.json",
        "a718f87b3765291fe1d9dabe3f6a9d8071da9629f033389018820b847fa1d844",
    ),
    (
        "Provenance.schema.json",
        "421c05ca2518bd14966933dbd02cf374a52c3f6e464436fe568268469c80c0b6",
    ),
    (
        "RtEstimate.schema.json",
        "17d8e380c7c1e5b61ab9f3c7d9d25480b5ba411dbb30cba2462bd66c2cd5b45b",
    ),
    (
        "ScenarioInput.schema.json",
        "1f7181c420c18a93b4240b07ebbb0437e689d143406e7e69c88d8420490d8006",
    ),
    (
        "StateFips.schema.json",
        "8ace44167c1dcaa6a0b1c3443db05f27535c29e9151e298e86ecac1ac3e25703",
    ),
    (
        "WeeklyCaseCount.schema.json",
        "6db139ef9c55376aac1d01fbdfcd0a325a8dfdc41973c90706a58537ebe86284",
    ),
];

#[test]
fn v1_schema_files_are_unchanged() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/v1");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut released: Vec<String> = V1_RELEASED.iter().map(|(n, _)| n.to_string()).collect();
    released.sort();
    assert_eq!(names, released, "v1 schema file set changed");
    for (name, want) in V1_RELEASED {
        let got = hex::encode(Sha256::digest(fs::read(dir.join(name)).unwrap()));
        assert_eq!(&got, want, "{name}: released v1 schema was edited");
    }
}
