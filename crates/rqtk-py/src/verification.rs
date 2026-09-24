use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::intern;
use pyo3::prelude::*;
use rqtk_core::verification::{build_verification_doc, find_activity_from_current_dir};

#[pyclass(name = "verifies", module = "rqtk")]
pub struct VerificationDecorator {
    activity_id: String,
    case: Option<String>,
    doc: String,
}

#[pymethods]
impl VerificationDecorator {
    /// Link a test to a verification activity; `case` names one case of a parametrized
    /// test (its pytest id, as in `test_x[case]`).
    #[new]
    #[pyo3(signature = (activity_id, *, case = None))]
    fn __new__(activity_id: String, case: Option<String>) -> PyResult<Self> {
        match find_activity_from_current_dir(&activity_id) {
            Ok(Some(info)) => Ok(Self {
                doc: build_verification_doc(&activity_id, &info),
                activity_id,
                case,
            }),
            Ok(None) => Err(PyValueError::new_err(format!(
                "verification activity `{activity_id}` not found in any requirement file under the configured requirements directory"
            ))),
            Err(e) => Err(PyRuntimeError::new_err(format!(
                "rqtk-py: could not search requirements for `{activity_id}`: {e}"
            ))),
        }
    }

    fn __call__(&self, py: Python<'_>, wraps: Py<PyAny>) -> PyResult<Py<PyAny>> {
        wraps.setattr(py, intern!(py, "__doc__"), &self.doc)?;
        wraps.setattr(py, intern!(py, "__rqtk_verifies__"), &self.activity_id)?;
        if let Some(case) = &self.case {
            wraps.setattr(py, intern!(py, "__rqtk_case__"), case)?;
        }
        Ok(wraps)
    }
}
