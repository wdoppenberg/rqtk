use pyo3::prelude::*;

mod verification;

use verification::VerificationDecorator;

#[pymodule]
#[pyo3(name = "rqtk")]
fn rqtk(m: &Bound<PyModule>) -> PyResult<()> {
    m.add_class::<VerificationDecorator>()?;
    Ok(())
}
