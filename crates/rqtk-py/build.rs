fn main() {
    // A Python extension module leaves the interpreter's symbols undefined; macOS needs to be
    // told so explicitly, or a plain `cargo build` of the workspace fails to link.
    pyo3_build_config::add_extension_module_link_args();
}
