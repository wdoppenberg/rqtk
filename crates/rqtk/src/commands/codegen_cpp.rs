use std::{error::Error, path::Path, path::PathBuf};

use rqtk_cpp::generate_header_file;

use crate::output;

pub fn run(repo_root: &Path, output: PathBuf, macro_name: String) -> Result<(), Box<dyn Error>> {
    let count = generate_header_file(repo_root, &output, &macro_name)?;
    output::success(
        "C++ header generated",
        &[
            ("file", &output.display().to_string()),
            ("macro", &macro_name),
            ("identifiers", &count.to_string()),
        ],
    );
    Ok(())
}
