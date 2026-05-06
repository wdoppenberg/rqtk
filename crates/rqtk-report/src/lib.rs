mod error;
mod render;

pub use error::ReportError;
pub use render::generate_typst_source;

use rqtk_core::{RequirementSet, Validated};
use std::path::Path;
use std::process::Command;

/// Generate a requirements report from `set` and write it to `output`.
///
/// The output format is determined by the file extension:
/// - `.typ`  — write the raw Typst source (no external tool required)
/// - anything else (`.pdf` recommended) — compile via the `typst` CLI
pub fn generate_report(set: &RequirementSet<Validated>, output: &Path) -> Result<(), ReportError> {
    let source = generate_typst_source(set);

    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("pdf")
        .to_lowercase();

    if ext == "typ" {
        std::fs::write(output, source)?;
        return Ok(());
    }

    // Write Typst source to a temp file then compile with the typst CLI.
    let typ_path = std::env::temp_dir().join("rqtk-report.typ");
    std::fs::write(&typ_path, &source)?;

    let result = Command::new("typst")
        .args([
            "compile",
            typ_path.to_str().unwrap(),
            output.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ReportError::TypstNotFound
            } else {
                ReportError::Io(e)
            }
        });

    let _ = std::fs::remove_file(&typ_path);

    match result? {
        out if out.status.success() => Ok(()),
        out => {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            Err(ReportError::CompilationFailed(stderr))
        }
    }
}
