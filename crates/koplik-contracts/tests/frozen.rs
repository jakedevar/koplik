//! Released contract versions are immutable: the committed JSON Schema files of v1 and v2 and
//! the Rust sources that define them must stay byte-for-byte what they released with. A shape
//! change is a new version (`v3`, ...), never an edit here. (v3 is the newest version and is
//! covered by `schema_v3.rs`; freeze it here when v4 is added.)

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

const V1_SCHEMA: &[(&str, &str)] = &[
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

const V2_SCHEMA: &[(&str, &str)] = &[
    (
        "CountyFips.schema.json",
        "7484018f3820cd01365ae3f14ed24c0da733ca8e1bc43cab513942c88ecbbf35",
    ),
    (
        "EnsembleResult.schema.json",
        "bcb421a23e9f79c93d9bc56cdf0027fda2e4e547131737d166b114066e0ac106",
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
        "TrajectoryResult.schema.json",
        "31af8e5252b663c01e54ce9764e2f80b1412c9fe9201efae297473600d9006d6",
    ),
    (
        "WeeklyCaseCount.schema.json",
        "6db139ef9c55376aac1d01fbdfcd0a325a8dfdc41973c90706a58537ebe86284",
    ),
];

/// `src/` files that define v1 and v2 (v2's `mod.rs` is the whole of v2).
const SOURCES: &[(&str, &str)] = &[
    (
        "v1/coverage.rs",
        "f5a0a7218b3fc6fee0d9319b4dd0f253f3ace2dc5fc21923b497180a9b4c4ce6",
    ),
    (
        "v1/fips.rs",
        "68c9436343c77de6a98ecbdfac79d1e92fdaa3c6e229d3cf8c06f020c91836f0",
    ),
    (
        "v1/forecast.rs",
        "3fff4b97e0e1edef0e2a82952c0126ab12556af6ef6e4db21115802edb82010d",
    ),
    (
        "v1/geography.rs",
        "5ecbb02155102c90aee02819f3b97bef6f8a42a06b59623312feb0e45472acc6",
    ),
    (
        "v1/mmwr.rs",
        "313a2070ae07259efddfb97fe2fb1f6de7985ad660a5ebfbc33f228e03d3ed85",
    ),
    (
        "v1/mod.rs",
        "c6a9fd5afc41956f2d470434f23adacd6f21407db19bae203f491c7611c8981c",
    ),
    (
        "v1/population.rs",
        "19847e25f9748695cfdd1ca7726362ad5e1d5bd4adee04e43e2fee9eff27cfea",
    ),
    (
        "v1/provenance.rs",
        "7e7e81b722668556fa52a95b93e30c2e2577fd075e8c911c62c89a0eb2ecea5c",
    ),
    (
        "v1/rt.rs",
        "377b78310da64f596bc5e551b512904b69b058bac7895413ffaa9571c5ada595",
    ),
    (
        "v1/scenario.rs",
        "623fea0e8dc6a1b2da44c398aef924357b8cf2390a2649a2f890e31270980802",
    ),
    (
        "v1/weekly_cases.rs",
        "d04b2ebe30a994f040cc3a5cb79b07cfca8a52faafbad29425c360e52efd22d1",
    ),
    (
        "v2/mod.rs",
        "242fd789505a4e50913e571d76f568c4ee80e7903d579d05889f1e88b77a90ed",
    ),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn digest(path: PathBuf) -> String {
    hex::encode(Sha256::digest(
        fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")),
    ))
}

fn check_schema_dir(version: &str, released: &[(&str, &str)]) {
    let dir = root().join("schema").join(version);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut want: Vec<String> = released.iter().map(|(n, _)| n.to_string()).collect();
    want.sort();
    assert_eq!(names, want, "{version} schema file set changed");
    for (name, want) in released {
        assert_eq!(
            &digest(dir.join(name)),
            want,
            "{version}/{name}: released schema was edited"
        );
    }
}

#[test]
fn v1_schema_files_are_unchanged() {
    check_schema_dir("v1", V1_SCHEMA);
}

#[test]
fn v2_schema_files_are_unchanged() {
    check_schema_dir("v2", V2_SCHEMA);
}

#[test]
fn v1_and_v2_sources_are_unchanged() {
    for (name, want) in SOURCES {
        assert_eq!(
            &digest(root().join("src").join(name)),
            want,
            "src/{name}: released contract source was edited"
        );
    }
}
