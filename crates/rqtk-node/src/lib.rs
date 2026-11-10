//! The native half of `@rqtk/client`; `index.js` wraps it in the typed TypeScript API.
//!
//! Queries return the client's JSON, which is already the shape of the generated types.
//! Opening and reloading run on the libuv thread pool and return Promises. Errors carry the
//! client's error as JSON in their message, which `index.js` turns into an `RqtkError`.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use rqtk_client::{Filter, RemoteOptions};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("client values serialise to JSON")
}

fn error(e: rqtk_client::Error) -> Error {
    Error::from_reason(to_json(&e))
}

fn query<T: Serialize>(result: rqtk_client::Result<T>) -> Result<String> {
    result.map(|value| to_json(&value)).map_err(error)
}

/// An open repository snapshot. Shared, not copied, between the threads that use it.
#[napi]
pub struct Repository {
    inner: Arc<rqtk_client::Repository>,
}

#[napi]
impl Repository {
    #[napi(ts_return_type = "Promise<Repository>")]
    pub fn reload(&self) -> AsyncTask<Open> {
        AsyncTask::new(Open(Source::Reload(Arc::clone(&self.inner))))
    }

    #[napi]
    pub fn info(&self) -> String {
        to_json(&self.inner.info())
    }

    #[napi]
    pub fn requirements(&self) -> String {
        to_json(&self.inner.requirements())
    }

    #[napi]
    pub fn requirement(&self, id: String) -> Option<String> {
        self.inner.requirement(&id).map(|r| to_json(&r))
    }

    /// `filter` is a `Filter` as JSON.
    #[napi]
    pub fn find_requirements(&self, filter: String) -> Result<String> {
        let filter: Filter = serde_json::from_str(&filter)
            .map_err(|e| Error::new(Status::InvalidArg, format!("invalid filter: {e}")))?;
        Ok(to_json(&self.inner.find_requirements(&filter)))
    }

    #[napi]
    pub fn needs(&self) -> String {
        to_json(&self.inner.needs())
    }

    #[napi]
    pub fn need(&self, id: String) -> Option<String> {
        self.inner.need(&id).map(|n| to_json(&n))
    }

    #[napi]
    pub fn stakeholders(&self) -> String {
        to_json(&self.inner.stakeholders())
    }

    #[napi]
    pub fn stakeholder(&self, id: String) -> Option<String> {
        self.inner.stakeholder(&id).map(|s| to_json(&s))
    }

    #[napi]
    pub fn activities(&self) -> String {
        to_json(&self.inner.activities())
    }

    #[napi]
    pub fn context(&self, id: String) -> Result<String> {
        query(self.inner.context(&id))
    }

    #[napi]
    pub fn trace(&self, id: String) -> Result<String> {
        query(self.inner.trace(&id))
    }

    #[napi]
    pub fn coverage(&self) -> String {
        to_json(&self.inner.coverage())
    }

    #[napi]
    pub fn lint(&self) -> String {
        to_json(&self.inner.lint())
    }

    #[napi]
    pub fn impact(&self, base: String) -> Result<String> {
        query(self.inner.impact(&base))
    }

    #[napi]
    pub fn diff(&self, from_ref: String, to_ref: String) -> Result<String> {
        query(self.inner.diff(&from_ref, &to_ref))
    }

    #[napi]
    pub fn baselines(&self) -> Result<String> {
        query(self.inner.baselines())
    }

    #[napi]
    pub fn history(&self, id: String) -> Result<String> {
        query(self.inner.history(&id))
    }
}

pub enum Source {
    Checkout(PathBuf),
    Ref(PathBuf, String),
    Remote(String, String, RemoteOptions),
    Reload(Arc<rqtk_client::Repository>),
}

/// Opening a repository, off the event loop.
pub struct Open(Source);

impl Task for Open {
    type Output = rqtk_client::Repository;
    type JsValue = Repository;

    fn compute(&mut self) -> Result<Self::Output> {
        match &self.0 {
            Source::Checkout(path) => rqtk_client::Repository::open(path),
            Source::Ref(path, reference) => rqtk_client::Repository::open_ref(path, reference),
            Source::Remote(url, reference, options) => {
                rqtk_client::Repository::open_remote_with(url, reference, options)
            }
            Source::Reload(repo) => repo.reload(),
        }
        .map_err(error)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(Repository {
            inner: Arc::new(output),
        })
    }
}

#[napi(ts_return_type = "Promise<Repository>")]
pub fn open(path: String) -> AsyncTask<Open> {
    AsyncTask::new(Open(Source::Checkout(path.into())))
}

#[napi(ts_return_type = "Promise<Repository>")]
pub fn open_ref(path: String, reference: String) -> AsyncTask<Open> {
    AsyncTask::new(Open(Source::Ref(path.into(), reference)))
}

#[napi(ts_return_type = "Promise<Repository>")]
pub fn open_remote(url: String, reference: String, allow_file_url: bool) -> AsyncTask<Open> {
    AsyncTask::new(Open(Source::Remote(
        url,
        reference,
        RemoteOptions { allow_file_url },
    )))
}

#[napi]
pub fn cache_dir() -> String {
    rqtk_client::cache_dir().to_string_lossy().into_owned()
}

#[napi]
pub fn clear_cache() -> Result<()> {
    rqtk_client::clear_cache().map_err(error)
}
