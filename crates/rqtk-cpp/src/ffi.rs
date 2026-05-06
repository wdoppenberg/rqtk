#[cxx::bridge(namespace = "rqtk::codegen")]
mod ffi_bridge {
    extern "Rust" {
        fn generate_verifies_header_to_file(
            repo_root: &CxxString,
            output: &CxxString,
            macro_name: &CxxString,
        ) -> Result<usize>;
    }
}

pub fn generate_verifies_header_to_file(
    repo_root: &cxx::CxxString,
    output: &cxx::CxxString,
    macro_name: &cxx::CxxString,
) -> Result<usize, String> {
    crate::generate_header_file(
        std::path::Path::new(repo_root.to_str().map_err(|e| e.to_string())?),
        std::path::Path::new(output.to_str().map_err(|e| e.to_string())?),
        macro_name.to_str().map_err(|e| e.to_string())?,
    )
}
