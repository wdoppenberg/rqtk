use pyo3::prelude::*;

mod verification;

use verification::VerificationDecorator;

/// Run the rqtk command line with `argv` (`argv[0]` is the program name) and return its exit
/// code. Runs in-process and releases the GIL while it works.
#[pyfunction]
fn run_cli(py: Python<'_>, argv: Vec<String>) -> u8 {
    py.detach(|| rqtk::cli::run_with_args(argv))
}

/// The native part of the `rqtk` Python package; import from `rqtk` instead.
#[pymodule]
#[pyo3(name = "_rqtk")]
fn rqtk_native(m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<VerificationDecorator>()?;
    m.add_function(wrap_pyfunction!(run_cli, m)?)?;
    Ok(())
}
