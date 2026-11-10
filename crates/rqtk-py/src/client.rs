//! The client's native half: `rqtk._client` wraps it in the typed Python API.
//!
//! Every method returns the client's JSON, which `rqtk.types` decodes into dataclasses, and
//! releases the GIL while the client works.
//!
//! A client repository is `Send` but not `Sync` (git's object database handle isn't), so each
//! handle is behind a mutex, locked only once the GIL is released: queries on one handle run
//! one at a time, while separate handles run in parallel.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rqtk_client::{Filter, RemoteOptions};
use serde::Serialize;
use std::sync::{Mutex, PoisonError};

type Inner = rqtk_client::Repository;

/// An open repository snapshot.
#[pyclass(frozen, module = "rqtk._rqtk", name = "Repository")]
pub struct Repository(Mutex<Inner>);

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("client values serialise to JSON")
}

/// `rqtk.Error(code, message)`.
fn error(py: Python<'_>, e: rqtk_client::Error) -> PyErr {
    let raised = py
        .import("rqtk._client")
        .and_then(|m| m.getattr("Error"))
        .and_then(|class| class.call1((e.code.as_str(), e.message)));
    match raised {
        Ok(value) => PyErr::from_value(value),
        Err(import) => import,
    }
}

fn opened(
    py: Python<'_>,
    f: impl FnOnce() -> rqtk_client::Result<Inner> + Send,
) -> PyResult<Repository> {
    py.detach(f)
        .map(|repo| Repository(Mutex::new(repo)))
        .map_err(|e| error(py, e))
}

impl Repository {
    /// Run `f` on the snapshot without the GIL.
    fn with<T: Send>(&self, py: Python<'_>, f: impl FnOnce(&Inner) -> T + Send) -> T {
        py.detach(|| f(&self.0.lock().unwrap_or_else(PoisonError::into_inner)))
    }

    /// Run query `f` without the GIL and return its JSON.
    fn query<T: Serialize>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&Inner) -> rqtk_client::Result<T> + Send,
    ) -> PyResult<String> {
        self.with(py, |repo| f(repo).map(|value| to_json(&value)))
            .map_err(|e| error(py, e))
    }

    /// The JSON of the item `f` finds, if any.
    fn lookup<T: Serialize>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&Inner) -> Option<T> + Send,
    ) -> Option<String> {
        self.with(py, |repo| f(repo).map(|item| to_json(&item)))
    }
}

#[pymethods]
impl Repository {
    fn reload(&self, py: Python<'_>) -> PyResult<Repository> {
        let reloaded = self.with(py, |repo| repo.reload());
        reloaded
            .map(|repo| Repository(Mutex::new(repo)))
            .map_err(|e| error(py, e))
    }

    fn info(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.info()))
    }

    fn requirements(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.requirements()))
    }

    fn requirement(&self, py: Python<'_>, id: &str) -> Option<String> {
        self.lookup(py, |repo| repo.requirement(id))
    }

    /// `filter` is a `Filter` as JSON.
    fn find_requirements(&self, py: Python<'_>, filter: &str) -> PyResult<String> {
        let filter: Filter = serde_json::from_str(filter)
            .map_err(|e| PyValueError::new_err(format!("invalid filter: {e}")))?;
        self.query(py, |repo| Ok(repo.find_requirements(&filter)))
    }

    fn needs(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.needs()))
    }

    fn need(&self, py: Python<'_>, id: &str) -> Option<String> {
        self.lookup(py, |repo| repo.need(id))
    }

    fn stakeholders(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.stakeholders()))
    }

    fn stakeholder(&self, py: Python<'_>, id: &str) -> Option<String> {
        self.lookup(py, |repo| repo.stakeholder(id))
    }

    fn activities(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.activities()))
    }

    fn context(&self, py: Python<'_>, id: &str) -> PyResult<String> {
        self.query(py, |repo| repo.context(id))
    }

    fn trace(&self, py: Python<'_>, id: &str) -> PyResult<String> {
        self.query(py, |repo| repo.trace(id))
    }

    fn coverage(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.coverage()))
    }

    fn lint(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| Ok(repo.lint()))
    }

    fn impact(&self, py: Python<'_>, base: &str) -> PyResult<String> {
        self.query(py, |repo| repo.impact(base))
    }

    fn diff(&self, py: Python<'_>, from_ref: &str, to_ref: &str) -> PyResult<String> {
        self.query(py, |repo| repo.diff(from_ref, to_ref))
    }

    fn baselines(&self, py: Python<'_>) -> PyResult<String> {
        self.query(py, |repo| repo.baselines())
    }

    fn history(&self, py: Python<'_>, id: &str) -> PyResult<String> {
        self.query(py, |repo| repo.history(id))
    }
}

/// Open the repository at `path`: its working tree, or the revision `reference`.
#[pyfunction]
#[pyo3(signature = (path, reference = None))]
pub fn open(
    py: Python<'_>,
    path: std::path::PathBuf,
    reference: Option<String>,
) -> PyResult<Repository> {
    opened(py, || match &reference {
        Some(r) => rqtk_client::Repository::open_ref(&path, r),
        None => rqtk_client::Repository::open(&path),
    })
}

/// Open the repository at git URL `url`, revision `reference`, through the cache.
#[pyfunction]
#[pyo3(signature = (url, reference, allow_file_url = false))]
pub fn open_remote(
    py: Python<'_>,
    url: &str,
    reference: &str,
    allow_file_url: bool,
) -> PyResult<Repository> {
    let options = RemoteOptions { allow_file_url };
    opened(py, || {
        rqtk_client::Repository::open_remote_with(url, reference, &options)
    })
}

#[pyfunction]
pub fn cache_dir() -> std::path::PathBuf {
    rqtk_client::cache_dir()
}

#[pyfunction]
pub fn clear_cache(py: Python<'_>) -> PyResult<()> {
    py.detach(rqtk_client::clear_cache)
        .map_err(|e| error(py, e))
}
