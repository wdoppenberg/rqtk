//! The public API of rqtk-client, recorded in `public-api.txt`. Any change to a public item
//! fails this test until the record is updated (`RQTK_UPDATE_SNAPSHOTS=1`), so every API
//! change shows up in review.
//!
//! Needs the nightly toolchain named by `RQTK_API_NIGHTLY` (default below) for rustdoc's JSON
//! output; CI installs that exact nightly, so the output format can't drift.

use rqtk_macros::verifies;
use std::path::PathBuf;

const NIGHTLY: &str = "nightly-2026-06-22";

#[verifies("VA-CLIENT-0001-01")]
#[test]
fn public_api_matches_the_record() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let toolchain = std::env::var("RQTK_API_NIGHTLY").unwrap_or_else(|_| NIGHTLY.to_owned());
    let json = rustdoc_json::Builder::default()
        .toolchain(toolchain)
        .manifest_path(manifest_dir.join("Cargo.toml"))
        .all_features(true)
        .quiet(true)
        .build()
        .expect("rustdoc JSON builds; install the nightly toolchain RQTK_API_NIGHTLY names");
    let api = public_api::Builder::from_rustdoc_json(json)
        .build()
        .expect("the public API is read from rustdoc JSON");
    let current = api.to_string();
    let record = manifest_dir.join("public-api.txt");
    if std::env::var_os("RQTK_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&record, &current).unwrap();
        return;
    }
    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    if recorded != current {
        let added: Vec<&str> = current
            .lines()
            .filter(|l| !recorded.lines().any(|r| r == *l))
            .collect();
        let removed: Vec<&str> = recorded
            .lines()
            .filter(|l| !current.lines().any(|c| c == *l))
            .collect();
        panic!(
            "the public API of rqtk-client changed; review it and run with RQTK_UPDATE_SNAPSHOTS=1\nadded:\n  {}\nremoved:\n  {}",
            added.join("\n  "),
            removed.join("\n  ")
        );
    }
}
