//! The client crate, the Python package and the npm package release with the CLI's version.

use rqtk_macros::verifies;
use std::path::Path;

fn workspace() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn manifest(rel: &str) -> toml::Table {
    let text = std::fs::read_to_string(workspace().join(rel)).unwrap();
    toml::from_str(&text).unwrap()
}

fn package_version(rel: &str) -> String {
    manifest(rel)["package"]["version"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[verifies("VA-CLIENT-0019-01")]
#[test]
fn every_client_package_has_the_cli_version() {
    let cli = package_version("crates/rqtk/Cargo.toml");
    let mut versions = vec![
        (
            "rqtk-client",
            package_version("crates/rqtk-client/Cargo.toml"),
        ),
        // The `rqtk` Python package takes its version from this crate (`dynamic = ["version"]`).
        (
            "rqtk (Python)",
            package_version("crates/rqtk-py/Cargo.toml"),
        ),
        (
            "workspace dependency on rqtk-client",
            manifest("Cargo.toml")["workspace"]["dependencies"]["rqtk-client"]["version"]
                .as_str()
                .unwrap()
                .to_owned(),
        ),
    ];
    let pyproject =
        std::fs::read_to_string(workspace().join("crates/rqtk-py/pyproject.toml")).unwrap();
    assert!(
        pyproject.contains("dynamic = [\"version\"]"),
        "the Python package must take its version from Cargo"
    );
    // The npm package, wherever it lives in the workspace.
    for entry in std::fs::read_dir(workspace().join("crates")).unwrap() {
        let path = entry.unwrap().path().join("package.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let package: serde_json::Value = serde_json::from_str(&text).unwrap();
            if package["name"] == "@rqtk/client" {
                versions.push((
                    "@rqtk/client",
                    package["version"].as_str().unwrap().to_owned(),
                ));
            }
        }
    }
    for (what, version) in &versions {
        assert_eq!(version, &cli, "{what} must carry the CLI's version");
    }
}
